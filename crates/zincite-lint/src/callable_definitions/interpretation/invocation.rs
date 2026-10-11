use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    // Bind written actuals once per selected invocation. Captures retain their
    // defining file; only an exact formal reference follows these bindings.
    pub(in crate::callable_definitions) fn invocation_actuals<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        instance: &'b Instance<'a>,
        invocation: &mut Invocation<'a, 'b>,
    ) -> Result<(), String> {
        let owner = &self.source.bindings.declarations[instance.id.0];
        let written = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("invoked declaration unavailable")?;
        if !self.source.callable_annotations_safe(owner.file, written) {
            return Err("invoked declaration annotation is unsupported".into());
        }
        self.type_dependencies(owner.file, written, &instance.view, &[])?;
        let first = invocation.actuals.len();
        for (position, parameter) in instance.parameters.iter().enumerate() {
            let formal = formal_parameter(
                self.source.context,
                self.source.bindings,
                instance.id,
                position,
            )
            .ok_or("invoked formal identity unavailable")?;
            let formal_node = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &self.source.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("invoked formal declaration unavailable")?;
            self.type_dependencies(owner.file, formal_node, &instance.view, &[])?;
            let mut nodes = vec![formal_node];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, owner.file, node) {
                    return Err("invoked formal annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            let collection =
                clause.node.kind() == NodeKind::GeneratorCallExpression && position == 0;
            let (file, node) = if collection {
                (clause.file, clause.node)
            } else {
                call_argument(
                    self.source.context,
                    self.source.bindings,
                    self.source.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                    instance.id,
                    position,
                )
                .ok_or("invoked actual or default unavailable")?
            };
            let default = file == owner.file
                && formal_node.range().start <= node.range().start
                && node.range().end <= formal_node.range().end;
            let facts = if default {
                &instance.view
            } else {
                self.source.view(file, node, view)
            };
            let ty = if collection {
                self.source
                    .operation_fact(facts, file, node)
                    .and_then(|call| call.generator_argument.as_ref())
            } else {
                self.source
                    .expression_type(facts, file, node)
                    .map(|e| &e.ty)
                    .or_else(|| {
                        self.source
                            .reference(file, unwrap(node))
                            .map(|id| &facts.declarations[id.0].ty)
                    })
            }
            .ok_or("invoked actual type unavailable")?;
            // Match the selected set2array view without replacing the written
            // set or proving its values, extent or index-set correspondence.
            let set_array_view = ty.instantiation == Instantiation::Parameter
                && parameter.instantiation == Instantiation::Parameter
                && !optional(parameter)
                && matches!((&ty.kind, &parameter.kind),
                    (TypeKind::Set(element), TypeKind::Array { indices, element: selected })
                        if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                            && element.instantiation == Instantiation::Parameter
                            && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                            && element == selected);
            if !parameter.known()
                || !ty.known()
                || optional(ty)
                || !(crate::types::coerces(ty, parameter) || set_array_view)
                || instance.view.declarations[formal.0].ty != *parameter
            {
                return Err("invoked actual type or optionality is unsupported".into());
            }
            invocation.actuals.push(InvocationActual {
                formal,
                file,
                node,
                view: facts,
                collection,
                generators: if file == clause.file
                    && clause.node.range().start <= node.range().start
                    && node.range().end <= clause.node.range().end
                {
                    clause.generators.clone()
                } else {
                    Vec::new()
                },
            });
        }
        let mut unsupported = None;
        for actual in &invocation.actuals[first..] {
            let checked = self
                .invocation_source(actual.file, actual.node, invocation)
                .and_then(|source| {
                    let source = source.unwrap_or(actual);
                    match self.invocation_actual_safety(source) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    }
                });
            if let Err(reason) = checked {
                unsupported = Some(reason);
            }
        }
        // Header sources and filters also evaluate, even when no outputs result.
        for (position, generator) in clause.generators.iter().enumerate() {
            for value in generator.child_nodes() {
                let value = if value.kind() == NodeKind::WhereFilter {
                    value
                        .child_nodes()
                        .next()
                        .ok_or("invoked generator filter unavailable")?
                } else {
                    value
                };
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    clause.file,
                    value,
                    view,
                    &clause.generators[..=position],
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
            }
        }
        match unsupported {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
    // A generated array is inspected through its written construction, never
    // through the scalar result type of the retaining call node.
    pub(in crate::callable_definitions) fn invocation_actual_safety(
        &self,
        actual: &InvocationActual<'a, '_>,
    ) -> DefinitionSafety {
        if !actual.collection {
            return self.initialized_source_safety(
                actual.file,
                actual.node,
                actual.view,
                &actual.generators,
                &mut Vec::new(),
            );
        }
        let file = actual.file;
        let view = actual.view;
        let mut nodes = vec![actual.node];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return DefinitionSafety::Unsupported(
                    "invoked collection annotation is unsupported".into(),
                );
            }
            nodes.extend(node.child_nodes());
        }
        let Some(list) = actual
            .node
            .child_nodes()
            .find(|node| node.kind() == NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported("invoked collection headers unavailable".into());
        };
        let mut generators = actual.generators.clone();
        let mut unsupported = None;
        for generator in list.child_nodes() {
            let Some(source) = generator.child_nodes().next() else {
                return DefinitionSafety::Unsupported(
                    "invoked collection source unavailable".into(),
                );
            };
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, source, view, &generators, &mut Vec::new())
            {
                unsupported = Some(reason);
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, source, true, true)
            {
                unsupported = Some(reason);
            }
            generators.push(generator);
            for filter in generator
                .child_nodes()
                .filter(|node| node.kind() == NodeKind::WhereFilter)
            {
                let Some(condition) = filter.child_nodes().next() else {
                    return DefinitionSafety::Unsupported(
                        "invoked collection filter unavailable".into(),
                    );
                };
                if self
                    .source
                    .expression_type(view, file, condition)
                    .is_none_or(|expression| {
                        !expression.ty.known()
                            || optional(&expression.ty)
                            || expression.ty.kind != TypeKind::Bool
                    })
                {
                    unsupported = Some("invoked collection filter type is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    file,
                    condition,
                    view,
                    &generators,
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, condition, true, true)
                {
                    unsupported = Some(reason);
                }
            }
        }
        let Some(body) = actual
            .node
            .child_nodes()
            .find(|node| node.kind() != NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported("invoked collection body unavailable".into());
        };
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_source_safety(file, body, view, &generators, &mut Vec::new())
        {
            unsupported = Some(reason);
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, body, true, true)
        {
            unsupported = Some(reason);
        }
        match unsupported {
            Some(reason) => DefinitionSafety::Unsupported(reason),
            None => DefinitionSafety::Unknown("invoked collection extent is unproved".into()),
        }
    }
    pub(in crate::callable_definitions) fn invocation_source<'b, 'c>(
        &self,
        mut file: FileId,
        mut node: &'a SyntaxNode,
        invocation: &'c Invocation<'a, 'b>,
    ) -> Result<Option<&'c InvocationActual<'a, 'b>>, String> {
        let mut active = Vec::new();
        let mut source = None;
        while let Some(id) = (unwrap(node).kind() == NodeKind::Expression)
            .then(|| self.source.reference(file, unwrap(node)))
            .flatten()
        {
            let Some(actual) = invocation
                .actuals
                .iter()
                .rev()
                .find(|actual| actual.formal == id)
            else {
                break;
            };
            if active.contains(&id) {
                return Err("cyclic invoked actual correspondence".into());
            }
            active.push(id);
            source = Some(actual);
            file = actual.file;
            node = actual.node;
        }
        Ok(source)
    }
    // Only a literal Boolean or the existing exact length(array)==0 form gives
    // branch direction. This never establishes a member, cardinality or value.
    pub(in crate::callable_definitions) fn invocation_guard<'b>(
        &'b self,
        file: FileId,
        guard: &'a SyntaxNode,
        view: &'b CallableFacts,
        invocation: &Invocation<'a, 'b>,
    ) -> Result<Option<bool>, String> {
        let guard = unwrap(guard);
        let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, guard);
        if guard.kind() == NodeKind::Expression && tokens.len() == 1 {
            return Ok(match tokens[0].kind {
                TokenKind::True => Some(true),
                TokenKind::False => Some(false),
                _ => None,
            });
        }
        for length in guard.child_nodes().map(unwrap) {
            if !self.source.core(file, length, view, "length") {
                continue;
            }
            let arguments: Vec<_> = length.child_nodes().collect();
            let [array] = arguments.as_slice() else {
                continue;
            };
            let Some(id) = self.source.reference(file, unwrap(array)) else {
                continue;
            };
            if self.source.length_equals(file, guard, id, 0, view) {
                return self.invocation_array_empty(
                    file,
                    array,
                    self.source.view(file, array, view),
                    invocation,
                    &mut Vec::new(),
                    false,
                );
            }
        }
        Ok(None)
    }
    pub(in crate::callable_definitions) fn invocation_array_empty<'b>(
        &'b self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &'b CallableFacts,
        invocation: &Invocation<'a, 'b>,
        active: &mut Vec<DeclarationId>,
        inspect_adapter: bool,
    ) -> Result<Option<bool>, String> {
        let actual = self.invocation_source(file, node, invocation)?;
        if actual.is_some_and(|actual| actual.collection) {
            return Ok(None);
        }
        let (file, node, view) = actual.map_or((file, node, view), |actual| {
            (actual.file, actual.node, actual.view)
        });
        let actual_generators = actual.map_or(&[][..], |actual| actual.generators.as_slice());
        let node = unwrap(node);
        match node.kind() {
            NodeKind::ArrayLiteral => Ok(
                if node
                    .child_nodes()
                    .any(|n| n.kind() == NodeKind::IndexedArrayEntry)
                {
                    None
                } else {
                    Some(node.child_nodes().count() == 0)
                },
            ),
            NodeKind::ArrayComprehension => {
                let generators: Vec<_> = node
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                    .map(|n| n.child_nodes().collect())
                    .unwrap_or_default();
                let [generator] = generators.as_slice() else {
                    return Ok(None);
                };
                if crate::domains::generator_slots(
                    &self.source.context.files[file].parsed,
                    generator,
                ) != 1
                    || generator
                        .child_nodes()
                        .any(|n| n.kind() == NodeKind::WhereFilter)
                {
                    return Ok(None);
                }
                let source = generator
                    .child_nodes()
                    .next()
                    .ok_or("invoked array extent source unavailable")?;
                // A fully inspected parameter set selection has unproved extent.
                // Retain its written caller's lexical generators for source checks.
                if unwrap(source).kind() == NodeKind::ArrayAccessExpression
                    && self
                        .source
                        .expression_type(view, file, source)
                        .is_some_and(|e| {
                            e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.instantiation == Instantiation::Parameter
                                && matches!(&e.ty.kind, TypeKind::Set(element)
                                if element.instantiation == Instantiation::Parameter
                                    && element.kind == TypeKind::Int)
                        })
                {
                    return match self.initialized_source_safety(
                        file,
                        source,
                        view,
                        actual_generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(None),
                    };
                }
                let domain =
                    expression_domain(self.source.context, self.source.bindings, file, source);
                match crate::domains::bare_index_domain(&domain) {
                    Domain::Range { lower, upper } => {
                        let lower = crate::domains::invariant_integer(lower)?;
                        let upper = crate::domains::invariant_integer(upper)?;
                        Ok(lower.zip(upper).map(|(lower, upper)| lower > upper))
                    }
                    Domain::Unsupported(reason) => Err(reason.clone()),
                    _ => Ok(None),
                }
            }
            NodeKind::Expression => {
                let Some(id) = self.source.reference(file, node) else {
                    return Ok(None);
                };
                let owner = &self.source.bindings.declarations[id.0];
                if owner.role != DeclarationRole::Value || !owner.top_level {
                    return Ok(None);
                }
                if active.contains(&id) {
                    return Err("cyclic invoked array source".into());
                }
                let written = find_node(
                    self.source.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("invoked array declaration unavailable")?;
                let values: Vec<_> = written
                    .child_nodes()
                    .filter(|n| is_expression(n.kind()))
                    .collect();
                if values.is_empty() {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, node, view, &[], &mut Vec::new())
                    {
                        return Err(reason);
                    }
                    let domain = crate::domains::bare_index_domain(
                        &self.source.domains.declarations[id.0].domain,
                    );
                    let Domain::Array { indices, .. } = domain else {
                        return match domain {
                            Domain::Unsupported(reason) => Err(reason.clone()),
                            _ => Ok(None),
                        };
                    };
                    let [axis] = indices.as_slice() else {
                        return Ok(None);
                    };
                    return match crate::domains::bare_index_domain(axis) {
                        Domain::Range { lower, upper } => {
                            let lower = crate::domains::invariant_integer(lower)?;
                            let upper = crate::domains::invariant_integer(upper)?;
                            Ok(lower.zip(upper).map(|(lower, upper)| lower > upper))
                        }
                        Domain::Unsupported(reason) => Err(reason.clone()),
                        _ => Ok(None),
                    };
                }
                let [value] = values.as_slice() else {
                    return Ok(None);
                };
                active.push(id);
                let result = self.invocation_array_empty(
                    owner.file,
                    value,
                    self.source.view(owner.file, value, view),
                    invocation,
                    active,
                    inspect_adapter,
                );
                active.pop();
                result
            }
            NodeKind::CallExpression if inspect_adapter => {
                let Some(argument) = self.source.array_conversion_argument(file, node, view) else {
                    return Ok(None);
                };
                let call = self
                    .source
                    .operation_fact(view, file, node)
                    .ok_or("invoked array adapter selection unavailable")?;
                let CallOutcome::Resolved {
                    declaration,
                    parameters,
                    return_type,
                } = &call.outcome
                else {
                    return Err("invoked array adapter selection unsupported".into());
                };
                let name = &self.source.bindings.declarations[declaration.0].name;
                if !self
                    .source
                    .prefix_primitive(file, node, view, name, parameters, return_type)
                {
                    return Err("invoked array adapter written declaration is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    file,
                    node,
                    view,
                    actual_generators,
                    &mut Vec::new(),
                ) {
                    return Err(reason);
                }
                self.invocation_array_empty(file, argument, view, invocation, active, true)
            }
            _ => Ok(None),
        }
    }
}
