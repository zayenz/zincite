use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn uncertain_integer_equality(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
    ) -> Result<bool, String> {
        let file = clause.file;
        let node = clause.node;
        let operands: Vec<_> = node.child_nodes().collect();
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|expression| &expression.ty)
        };
        let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
        if operands.len() != 2
            || !self.source.core(file, node, view, "=")
            || !crate::definitions::annotations_safe(self.source.context, file, node)
            || operands
                .iter()
                .any(|operand| typed(operand).is_none_or(|ty| !integer(ty)))
            || typed(node).is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
            || self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && Some(return_type) == typed(node)
                        && parameters.iter().zip(&operands).all(|(formal, actual)| integer(formal)
                            && typed(actual).is_some_and(|ty| crate::types::coerces(ty, formal))))
                })
            || self
                .relation_iterations(file, &clause.generators, 0, view, false)
                .is_err()
        {
            return Ok(false);
        }
        // Only the demonstrated bare selection chain enters this uncertainty
        // path. Arithmetic selectors keep their original strict limitation.
        let mut selections = operands.clone();
        while let Some(value) = selections.pop() {
            let value = unwrap(value);
            match value.kind() {
                NodeKind::ArrayAccessExpression => selections.extend(value.child_nodes()),
                NodeKind::Expression => {
                    let tokens =
                        crate::domains::tokens(&self.source.context.files[file].parsed, value);
                    if !matches!(tokens.as_slice(), [token] if matches!(token.kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::IntegerLiteral))
                    {
                        return Ok(false);
                    }
                    let Some(ty) = typed(value) else {
                        return Ok(false);
                    };
                    if integer(ty) {
                        if crate::domains::expression_integer(
                            self.source.context,
                            self.source.bindings,
                            file,
                            value,
                        )
                        .is_err()
                        {
                            return Ok(false);
                        }
                        if let Some(id) = self.source.reference(file, value) {
                            let declaration = &self.source.bindings.declarations[id.0];
                            if declaration.role == DeclarationRole::Value && declaration.top_level {
                                if self.source.domains.declarations[id.0]
                                    .domain
                                    .numeric_minimum()
                                    .is_err()
                                {
                                    return Ok(false);
                                }
                                let Some(written) = find_node(
                                    self.source.context.files[declaration.file].parsed.tree(),
                                    &declaration.syntax_range,
                                    declaration.role,
                                ) else {
                                    return Ok(false);
                                };
                                if written
                                    .child_nodes()
                                    .filter(|child| is_expression(child.kind()))
                                    .any(|initializer| {
                                        crate::domains::expression_integer(
                                            self.source.context,
                                            self.source.bindings,
                                            declaration.file,
                                            initializer,
                                        )
                                        .is_err()
                                    })
                                {
                                    return Ok(false);
                                }
                            } else if declaration.role != DeclarationRole::Generator {
                                return Ok(false);
                            }
                        }
                        continue;
                    }
                    let TypeKind::Array { indices, element } = &ty.kind else {
                        return Ok(false);
                    };
                    if !ty.known()
                        || optional(ty)
                        || !integer(element)
                        || !matches!(indices.len(), 1 | 2)
                        || indices.iter().any(|index| {
                            !integer(index) || index.instantiation != Instantiation::Parameter
                        })
                    {
                        return Ok(false);
                    }
                    let Some(array) = self.source.reference(file, value) else {
                        return Ok(false);
                    };
                    let declaration = &self.source.bindings.declarations[array.0];
                    if declaration.role != DeclarationRole::Value || !declaration.top_level {
                        return Ok(false);
                    }
                    let Domain::Array { element, .. } =
                        &self.source.domains.declarations[array.0].domain
                    else {
                        return Ok(false);
                    };
                    self.check_array_element_domain(array, ty, element)?;
                    let Some(written) = find_node(
                        self.source.context.files[declaration.file].parsed.tree(),
                        &declaration.syntax_range,
                        declaration.role,
                    ) else {
                        return Ok(false);
                    };
                    // A literal source is inspected without treating its written
                    // defaults as values of every instance. The closed evaluator
                    // supplies only a veto, including overflow in a literal cell.
                    for initializer in written
                        .child_nodes()
                        .filter(|child| is_expression(child.kind()))
                    {
                        let initializer = unwrap(initializer);
                        if ty.instantiation != Instantiation::Parameter
                            || initializer.kind() != NodeKind::ArrayLiteral
                            || initializer.child_nodes().any(|cell| {
                                !matches!(crate::domains::tokens(&self.source.context.files[declaration.file].parsed,
                                    unwrap(cell)).as_slice(), [token] if token.kind == TokenKind::IntegerLiteral)
                                    || self.source.expression_type(self.source.calls, declaration.file, cell)
                                    .is_none_or(|expression| {
                                        !integer(&expression.ty)
                                            || expression.ty.instantiation
                                                != Instantiation::Parameter
                                    })
                                    || crate::domains::expression_integer(
                                        self.source.context,
                                        self.source.bindings,
                                        declaration.file,
                                        cell,
                                    )
                                    .is_err()
                            })
                        {
                            return Ok(false);
                        }
                    }
                }
                _ => return Ok(false),
            }
        }
        let mut unknown = false;
        for (position, generator) in clause.generators.iter().enumerate() {
            let Some(source) = generator.child_nodes().next() else {
                return Ok(false);
            };
            match self.initialized_source_safety(
                file,
                source,
                view,
                &clause.generators[..position],
                &mut Vec::new(),
            ) {
                DefinitionSafety::Unsupported(reason) => return Err(reason),
                DefinitionSafety::Unknown(_) => unknown = true,
                DefinitionSafety::Supported => {}
            }
            if !self.source.uncertain_equality_header(file, source, view) {
                return Ok(false);
            }
            // Filter arithmetic remains on the strict route in this local
            // selection-only admission.
            if generator
                .child_nodes()
                .any(|child| child.kind() == NodeKind::WhereFilter)
            {
                return Ok(false);
            }
        }
        match self.initialized_children_safety(file, &operands, view, &clause.generators) {
            DefinitionSafety::Unknown(_) => Ok(true),
            DefinitionSafety::Unsupported(reason) => Err(reason),
            DefinitionSafety::Supported => Ok(unknown),
        }
    }
    pub(in crate::callable_definitions) fn map_boundaries(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        boundaries: &[Boundary],
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        if !matches!(clause.kind, ClauseKind::Call) {
            return;
        }
        let Some((id, parameters)) = self.source.resolved(clause.file, clause.node, view) else {
            return;
        };
        for boundary in boundaries
            .iter()
            .filter(|b| b.callable == id && b.parameters == parameters)
        {
            let mut targets = Vec::new();
            for target in &boundary.unavailable.targets {
                if self.source.bindings.declarations[target.0].role != DeclarationRole::Parameter {
                    extend(&mut targets, vec![*target]);
                } else if let Some((file, node)) = self.source.actual(clause, id, *target, view) {
                    let range = self.source.context.files[file].location(node.range()).range;
                    for reference in &self.source.bindings.references {
                        if reference.file == file
                            && reference.kind == ReferenceKind::Value
                            && range.start <= reference.location.range.start
                            && reference.location.range.end <= range.end
                            && let BindingResolution::Resolved(id) = reference.resolution
                        {
                            extend(&mut targets, vec![id]);
                        }
                    }
                }
            }
            let unavailable = UnavailableCallableDefinition {
                location: boundary.unavailable.location.clone(),
                targets,
                reason: boundary.unavailable.reason.clone(),
            };
            if !out.contains(&unavailable) {
                out.push(unavailable);
            }
        }
    }
    pub(in crate::callable_definitions) fn literal_call_outputs(
        &self,
        clause: &Clause<'a>,
        caller: &CallableFacts,
        instance: &Instance<'a>,
    ) -> Option<Result<Vec<Output>, String>> {
        if instance.recursive {
            return None;
        }
        let declaration = &self.source.bindings.declarations[instance.id.0];
        let file = declaration.file;
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        if !self.source.callable_annotations_safe(file, written) {
            return None;
        }
        let mut body = unwrap(
            written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .last()?,
        );
        let view = &instance.view;
        let type_of = |file, node: &SyntaxNode, view: &CallableFacts| {
            self.source
                .view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range
                            == self.source.context.files[file].location(node.range()).range
                })
                .map(|e| e.ty.clone())
        };
        let boolean = |node: &SyntaxNode| {
            type_of(file, node, view)
                .is_some_and(|ty| ty.known() && !optional(&ty) && ty.kind == TypeKind::Bool)
        };
        if !boolean(body) {
            return None;
        }
        let mut generators = Vec::new();
        let mut old_array = None;
        let mut binder = None;
        if body.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(file, body, view, "forall")
        {
            let list = body
                .child_nodes()
                .find(|n| n.kind() == NodeKind::GeneratorList)?;
            generators.extend(list.child_nodes());
            if generators.len() != 1 {
                return None;
            }
            self.source.iterations(file, &generators, view).ok()?;
            self.relation_iterations(file, &generators, 0, view, false)
                .ok()?;
            let mut bindings = self.source.bindings.declarations.iter().filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == generators[0].range()
            });
            binder = Some(bindings.next()?.id);
            if bindings.next().is_some() {
                return None;
            }
            let source = unwrap(generators[0].child_nodes().next()?);
            if source.kind() != NodeKind::CallExpression
                || !self.source.core(file, source, view, "index_set")
                || source.child_nodes().count() != 1
            {
                return None;
            }
            let array = unwrap(source.child_nodes().next()?);
            if array.kind() != NodeKind::Expression {
                return None;
            }
            old_array = Some(self.source.reference(file, array)?);
            body = unwrap(
                body.child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)?,
            );
        }
        let equality =
            self.source
                .operation_fact(self.source.view(file, body, view), file, body)?;
        let CallOutcome::Resolved {
            declaration: equality_id,
            parameters,
            return_type,
        } = &equality.outcome
        else {
            return None;
        };
        if body.kind() != NodeKind::BinaryExpression
            || !crate::definitions::core_callable(
                self.source.context,
                self.source.bindings,
                *equality_id,
                "=",
            )
            || !boolean(body)
        {
            return None;
        }
        let sides: Vec<_> = body.child_nodes().map(unwrap).collect();
        if sides.len() != 2 {
            return None;
        }
        let formals: Option<Vec<_>> = (0..instance.parameters.len())
            .map(|position| {
                formal_parameter(
                    self.source.context,
                    self.source.bindings,
                    instance.id,
                    position,
                )
            })
            .collect();
        let formals = formals?;
        if formals.iter().any(|id| {
            let ty = &view.declarations[id.0].ty;
            !ty.known() || optional(ty)
        }) {
            return None;
        }
        let reference = |node: &SyntaxNode| {
            (node.kind() == NodeKind::Expression)
                .then(|| self.source.reference(file, node))
                .flatten()
        };
        let array_access = |node: &'a SyntaxNode| {
            if node.kind() != NodeKind::ArrayAccessExpression {
                return None;
            }
            let mut children = node.child_nodes().map(unwrap);
            let array = reference(children.next()?)?;
            let index = reference(children.next()?)?;
            children.next().is_none().then_some((array, index))
        };
        let (output, input, selector, rhs) = if let Some(old) = old_array {
            // Only the written index_set traversal supplies the literal's 1..N axis.
            // The existing two-cell value proof checks both values and its selector.
            let mut found = None;
            for (lhs, rhs) in [(sides[0], sides[1]), (sides[1], sides[0])] {
                let Some((output, index)) = array_access(lhs) else {
                    continue;
                };
                if Some(index) != binder || rhs.kind() != NodeKind::ArrayAccessExpression {
                    continue;
                }
                let parts: Vec<_> = rhs.child_nodes().map(unwrap).collect();
                if parts.len() != 2 || parts[0].kind() != NodeKind::ArrayLiteral {
                    continue;
                }
                let values: Vec<_> = parts[0].child_nodes().map(unwrap).collect();
                if values.len() == 2 && array_access(values[0]) == Some((old, index)) {
                    found = Some((output, old, None, rhs));
                    break;
                }
            }
            found?
        } else {
            let mut found = None;
            for (lhs, rhs) in [(sides[0], sides[1]), (sides[1], sides[0])] {
                if let (Some(output), Some((array, index))) = (reference(lhs), array_access(rhs)) {
                    found = Some((output, array, Some(index), rhs));
                    break;
                }
            }
            found?
        };
        if ![output, input].iter().all(|id| formals.contains(id))
            || selector.is_some_and(|id| !formals.contains(&id))
            || output == input
        {
            return None;
        }
        let TypeKind::Array { indices, element } = &view.declarations[input.0].ty.kind else {
            return None;
        };
        let kind = element.kind.clone();
        if indices.len() != 1
            || indices[0].kind != TypeKind::Int
            || !matches!(kind, TypeKind::Bool | TypeKind::Int)
        {
            return None;
        }
        let actual = |formal| self.source.actual(clause, instance.id, formal, caller);
        let lexical = |actual_file, node: &SyntaxNode| {
            if actual_file == clause.file
                && clause.node.range().start <= node.range().start
                && node.range().end <= clause.node.range().end
            {
                clause.generators.as_slice()
            } else {
                &[]
            }
        };
        let literal = |actual_file, node: &'a SyntaxNode| {
            let node = unwrap(node);
            let ty = type_of(actual_file, node, caller)?;
            if node.kind() != NodeKind::ArrayLiteral
                || !ty.known()
                || optional(&ty)
                || !matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == kind)
            {
                return None;
            }
            let cells: Vec<_> = node.child_nodes().map(unwrap).collect();
            cells
                .iter()
                .all(|cell| {
                    cell.kind() != NodeKind::IndexedArrayEntry
                        && type_of(actual_file, cell, caller)
                            .is_some_and(|ty| ty.known() && !optional(&ty) && ty.kind == kind)
                })
                .then_some(cells)
        };
        let (input_file, input_actual) = actual(input)?;
        let input_cells = literal(input_file, input_actual)?;
        let (output_file, output_actual) = actual(output)?;
        let targets = if let Some(selector) = selector {
            if view.declarations[output.0].ty.kind != kind {
                return None;
            }
            let (index_file, index_actual) = actual(selector)?;
            let index_actual = unwrap(index_actual);
            let ty = type_of(index_file, index_actual, caller)?;
            if !ty.known() || optional(&ty) || ty.kind != TypeKind::Int {
                return None;
            }
            let interval = if index_actual.kind() == NodeKind::Expression {
                if let Some(id) = self.source.reference(index_file, index_actual) {
                    crate::domains::index_domain_interval(
                        &self.source.domains.declarations[id.0].domain,
                    )
                } else {
                    crate::domains::invariant_expression_integer(
                        self.source.context,
                        self.source.bindings,
                        index_file,
                        index_actual,
                    )
                    .ok()?
                    .map(|value| (value, value))
                }
            } else {
                None
            }?;
            if interval.0 < 1
                || interval.0 > interval.1
                || interval.1 > i64::try_from(input_cells.len()).ok()?
            {
                return None;
            }
            vec![unwrap(output_actual)]
        } else {
            let cells = literal(output_file, output_actual)?;
            if cells.len() != input_cells.len() {
                return None;
            }
            self.dependencies(file, rhs, view, &generators).ok()?;
            cells
        };
        // Validate every mapped input, including unused actuals/defaults. Output
        // lvalues enter dependencies only if the written RHS reads them too.
        let mut dependencies = Vec::new();
        for formal in &formals {
            let written = &self.source.bindings.declarations[formal.0];
            let parameter = find_node(
                self.source.context.files[file].parsed.tree(),
                &written.syntax_range,
                written.role,
            )?;
            if !crate::definitions::annotations_safe(self.source.context, file, parameter) {
                return None;
            }
            if matches!(view.declarations[formal.0].ty.kind, TypeKind::Array { .. }) {
                // These calls bind an unconstrained or polymorphic formal axis
                // directly to the literal. Written restricted axes need their
                // own call-contract proof and keep the ordinary fallback.
                let array_type = parameter.child_nodes().next()?;
                let axis = array_type.child_nodes().next()?;
                if array_type.kind() != NodeKind::ArrayType
                    || !(axis.kind() == NodeKind::TypeInstVariable
                        || axis.kind() == NodeKind::ScalarType
                            && crate::domains::tokens(
                                &self.source.context.files[file].parsed,
                                axis,
                            )
                            .iter()
                            .any(|token| token.kind == TokenKind::Int))
                {
                    return None;
                }
            }
            let type_ids = self.type_dependencies(file, parameter, view, &[]).ok()?;
            for id in type_ids {
                if formals.contains(&id) {
                    let (actual_file, value) = actual(id)?;
                    extend(
                        &mut dependencies,
                        self.dependencies(actual_file, value, caller, lexical(actual_file, value))
                            .ok()?,
                    );
                } else {
                    extend(&mut dependencies, vec![id]);
                }
            }
            if *formal != output || selector == Some(*formal) {
                let (actual_file, value) = actual(*formal)?;
                extend(
                    &mut dependencies,
                    self.dependencies(actual_file, value, caller, lexical(actual_file, value))
                        .ok()?,
                );
                let value = unwrap(value);
                if let Some(id) = (value.kind() == NodeKind::Expression)
                    .then(|| self.source.reference(actual_file, value))
                    .flatten()
                {
                    let d = &self.source.bindings.declarations[id.0];
                    let n = find_node(
                        self.source.context.files[d.file].parsed.tree(),
                        &d.syntax_range,
                        d.role,
                    )?;
                    if !crate::definitions::annotations_safe(self.source.context, d.file, n) {
                        return None;
                    }
                    extend(
                        &mut dependencies,
                        self.type_dependencies(d.file, n, caller, lexical(d.file, n))
                            .ok()?,
                    );
                }
            }
        }
        if selector.is_none() {
            for id in self.dependencies(file, rhs, view, &generators).ok()? {
                if !formals.contains(&id) {
                    extend(&mut dependencies, vec![id]);
                } else if id == output {
                    // Reading a destination is an actual dependency, never an anchor.
                    extend(
                        &mut dependencies,
                        self.dependencies(
                            output_file,
                            output_actual,
                            caller,
                            lexical(output_file, output_actual),
                        )
                        .ok()?,
                    );
                }
            }
        }
        // A checked literal result constrains the selected cell but cannot be
        // inverted into a definition of the selector or any array element.
        let output_actual = unwrap(output_actual);
        if selector.is_some()
            && kind == TypeKind::Int
            && output_actual.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.source.context.files[output_file].parsed, output_actual).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            let selector = selector?;
            let integer = TypeInst {
                instantiation: Instantiation::Parameter,
                optional: false,
                kind: TypeKind::Int,
            };
            let decision = integer.clone().with_inst(Instantiation::Decision);
            let source = &self.source.context.files[file];
            if declaration.name != "element"
                || declaration.role != DeclarationRole::Predicate
                || source.kind != SourceKind::StandardLibrary
                || self.source.context.standard_element.as_ref() != Some(&source.canonical_path)
                || formals.as_slice() != [selector, input, output]
                || reference(sides[0]) != Some(output)
                || array_access(sides[1]) != Some((input, selector))
                || view.declarations[selector.0].ty != decision
                || view.declarations[output.0].ty != integer
                || view.declarations[input.0].ty.instantiation != Instantiation::Decision
                || indices[0] != integer
                || **element != decision
                || type_of(output_file, output_actual, caller)? != integer
            {
                return None;
            }
            match crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                output_file,
                output_actual,
            ) {
                Ok(Some(_)) => {}
                Ok(None) => return None,
                Err(reason) => return Some(Err(reason)),
            }
            // The canonical body was checked by identity above. Refuse changed
            // defaults/domains, and validate metadata throughout its written tree.
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if node.kind() == NodeKind::Annotation {
                    let value = node.child_nodes().next()?;
                    let string = matches!(crate::domains::tokens(&source.parsed, value).as_slice(),
                        [token] if token.kind == TokenKind::StringLiteral);
                    if !string
                        && !["promise_total", "promise_commutative"]
                            .iter()
                            .any(|name| self.source.atomic_standard_metadata_safe(file, node, name))
                    {
                        return Some(Err(
                            "literal result element annotation is unsupported".into()
                        ));
                    }
                    continue;
                }
                if node.kind() == NodeKind::DomainType
                    || is_expression(node.kind())
                        && !(body.range().start <= node.range().start
                            && node.range().end <= body.range().end)
                {
                    return Some(Err(
                        "literal result element default or domain is unsupported".into(),
                    ));
                }
                nodes.extend(node.child_nodes());
            }
            if parameters.len() != 2
                || parameters
                    .iter()
                    .any(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Int)
                || !return_type.known()
                || optional(return_type)
                || return_type.instantiation != Instantiation::Decision
                || return_type.kind != TypeKind::Bool
            {
                return None;
            }
            if let Err(reason) = self.source.selected_union_primitive_safety(*equality_id) {
                return Some(Err(reason));
            }
            for formal in &formals {
                let (actual_file, value) = actual(*formal)?;
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    actual_file,
                    value,
                    caller,
                    lexical(actual_file, value),
                    &mut Vec::new(),
                ) {
                    return Some(Err(reason));
                }
            }
            return Some(Ok(Vec::new()));
        }
        let mut outputs = Vec::new();
        for node in targets {
            if node.kind() != NodeKind::Expression
                || type_of(output_file, node, caller)
                    .is_none_or(|ty| !ty.known() || optional(&ty) || ty.kind != kind)
            {
                return None;
            }
            let (target, coverage) =
                self.source
                    .target(output_file, node, caller, lexical(output_file, node))?;
            if coverage != DefinitionCoverage::Scalar {
                return None;
            }
            let d = &self.source.bindings.declarations[target.0];
            let n = find_node(
                self.source.context.files[d.file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )?;
            if !crate::definitions::annotations_safe(self.source.context, d.file, n) {
                return None;
            }
            let mut ids = dependencies.clone();
            extend(
                &mut ids,
                self.type_dependencies(d.file, n, caller, lexical(d.file, n))
                    .ok()?,
            );
            outputs.push(Output {
                target,
                dependencies: ids,
                coverage,
                location: self.source.context.files[clause.file].location(clause.node.range()),
                enforced_boolean: false,
                scoped_local: false,
            });
        }
        Some(Ok(outputs))
    }
    pub(in crate::callable_definitions) fn direct_forall_equality(
        &self,
        file: FileId,
        quantified: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(DeclarationId, &'a SyntaxNode, Vec<&'a SyntaxNode>)> {
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        let quantified = unwrap(quantified);
        if quantified.kind() != NodeKind::GeneratorCallExpression
            || !self.source.core(file, quantified, view, "forall")
            || !self.source.operation_fact(self.source.view(file, quantified, view), file, quantified)
                .is_some_and(|call| matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                        && parameters[0].instantiation == Instantiation::Decision
                        && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && present(&indices[0], TypeKind::Int, Instantiation::Parameter)
                                && present(element, TypeKind::Bool, Instantiation::Decision))
                        && present(return_type, TypeKind::Bool, Instantiation::Decision)))
        { return None; }
        let children: Vec<_> = quantified.child_nodes().collect();
        if children.len() != 2 {
            return None;
        }
        let list = *children
            .iter()
            .find(|n| n.kind() == NodeKind::GeneratorList)?;
        let generators: Vec<_> = list.child_nodes().collect();
        if generators.len() != 1
            || generators[0].child_nodes().count() != 1
            || self.source.iterations(file, &generators, view).is_err()
            || self
                .relation_iterations(file, &generators, 0, view, false)
                .is_err()
            || self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generators[0].range()
                })
                .count()
                != 1
        {
            return None;
        }
        let output =
            self.source
                .integer_index_set(file, generators[0].child_nodes().next()?, view)?;
        let equality = unwrap(
            *children
                .iter()
                .find(|n| n.kind() != NodeKind::GeneratorList)?,
        );
        let sides: Vec<_> = equality.child_nodes().collect();
        if equality.kind() != NodeKind::BinaryExpression || sides.len() != 2
            || !self.source.core(file, equality, view, "=")
            || !self.source.operation_fact(self.source.view(file, equality, view), file, equality)
                .is_some_and(|call| matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2
                        && parameters.iter().all(|ty| present(ty, TypeKind::Int, Instantiation::Decision))
                        && present(return_type, TypeKind::Bool, Instantiation::Decision)))
            || !sides.iter().any(|side| self.source.target(file, side, view, &generators)
                == Some((output, DefinitionCoverage::WholeArray)))
        { return None; }
        Some((output, equality, generators))
    }
    pub(in crate::callable_definitions) fn direct_asserted_forall(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(DeclarationId, [DeclarationId; 2])> {
        let arrays = self.asserted_index_pair(file, assertion, view)?;
        let args = self.source.assertion_arguments(file, assertion, view)?;
        let (output, _, _) = self.direct_forall_equality(file, args[2].1, view)?;
        Some((output, arrays))
    }
    pub(in crate::callable_definitions) fn nested_asserted_forall(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(Output, [DeclarationId; 2])> {
        let arrays = self.asserted_index_pair(file, assertion, view)?;
        let args = self.source.assertion_arguments(file, assertion, view)?;
        let mut call = unwrap(args[2].1);
        let mut weight_guard = None;
        if self.source.core(file, call, view, "assert") {
            // Carry the outer index relation only through this one fully checked
            // returning guard. Its ordered RHS is already inspected by the
            // existing reflection/length consumers; it proves no load extent.
            let inner = self.source.assertion_arguments(file, call, view)?;
            let par_bool = TypeInst::par(TypeKind::Bool);
            let var_bool = par_bool.clone().with_inst(Instantiation::Decision);
            let par_string = TypeInst::par(TypeKind::String);
            let par_int = TypeInst::par(TypeKind::Int);
            let var_int = par_int.clone().with_inst(Instantiation::Decision);
            let var_array = TypeInst::par(TypeKind::Array {
                indices: vec![par_int.clone()],
                element: Box::new(var_int),
            })
            .with_inst(Instantiation::Decision);
            let expected = [par_bool.clone(), par_string.clone(), var_bool.clone()];
            if inner.len() != 3
                || inner.iter().any(|(source, _)| *source != file)
                || self
                    .source
                    .operation_fact(self.source.view(file, call, view), file, call)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == expected && *return_type == var_bool)
                    })
                || inner.iter().zip(&expected).any(|((_, node), ty)| {
                    self.source
                        .expression_type(self.source.view(file, node, view), file, node)
                        .is_none_or(|e| e.ty != *ty)
                })
            {
                return None;
            }
            let condition = unwrap(inner[0].1);
            let sides: Vec<_> = condition.child_nodes().map(unwrap).collect();
            if condition.kind() != NodeKind::BinaryExpression
                || sides.len() != 2
                || !self.source.core(file, condition, view, "\\/")
                || self
                    .source
                    .operation_fact(self.source.view(file, condition, view), file, condition)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [par_bool.clone(), par_bool.clone()]
                            && *return_type == par_bool)
                    })
            {
                return None;
            }
            let bound = sides[1];
            let operands: Vec<_> = bound.child_nodes().map(unwrap).collect();
            if bound.kind() != NodeKind::BinaryExpression || operands.len() != 2
                || !self.source.core(file, bound, view, ">=")
                || self.source.operation_fact(self.source.view(file, bound, view), file, bound)
                    .is_none_or(|fact| !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [TypeInst::par(TypeKind::Int), TypeInst::par(TypeKind::Int)] && *return_type == par_bool))
                || crate::domains::invariant_expression_integer(self.source.context, self.source.bindings, file, operands[1]) != Ok(Some(0))
            {
                return None;
            }
            let reflection = operands[0];
            let subject: Vec<_> = reflection.child_nodes().map(unwrap).collect();
            if reflection.kind() != NodeKind::CallExpression
                || subject.len() != 1
                || !self.source.core(file, reflection, view, "lb_array")
                || subject[0].kind() != NodeKind::Expression
                || self
                    .source
                    .operation_fact(self.source.view(file, reflection, view), file, reflection)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [var_array] && *return_type == par_int)
                    })
                || self
                    .source
                    .expression_type(self.source.view(file, reflection, view), file, reflection)
                    .is_none_or(|e| e.ty != par_int)
            {
                return None;
            }
            let weight = self.source.reference(file, subject[0])?;
            if !arrays.contains(&weight)
                || !self.source.length_equals(file, sides[0], weight, 0, view)
                || inner.iter().take(2).any(|(_, node)| {
                    self.dependencies(file, node, view, &[]).is_err()
                })
                // The exact ordered OR has been checked above. Inspect its
                // operands without asking an eager checker to traverse the OR.
                || [sides[0], sides[1], inner[1].1].into_iter().any(|node| {
                    self.source.closed_integer_source_error(file, node, true, false)
                        .is_some()
                })
            {
                return None;
            }
            weight_guard = Some(weight);
            call = unwrap(inner[2].1);
        }
        if call.kind() != NodeKind::CallExpression {
            return None;
        }
        let (id, parameters) = self.source.resolved(file, call, view)?;
        let array = |ty: &TypeInst, instantiation| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == instantiation
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0] == TypeInst::par(TypeKind::Int)
                        && element.kind == TypeKind::Int && element.instantiation == instantiation)
        };
        if parameters.len() != 3
            || !array(&parameters[0], Instantiation::Decision)
            || !array(&parameters[1], Instantiation::Decision)
            || !array(&parameters[2], Instantiation::Parameter)
            || self.source.expression_type(self.source.view(file, call, view), file, call)
                .is_none_or(|e| e.ty != TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision))
            || self.source.operation_fact(self.source.view(file, call, view), file, call)
                .is_none_or(|fact| !matches!(&fact.outcome,
                    CallOutcome::Resolved { return_type, .. } if *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)))
        {
            return None;
        }
        let owner = &self.source.bindings.declarations[arrays[0].0];
        let child = &self.source.bindings.declarations[id.0];
        let instance = self
            .instances
            .iter()
            .find(|instance| instance.id == id && instance.parameters == parameters)?;
        if instance.recursive
            || child.role != DeclarationRole::Predicate
            || child.file == owner.file && child.item == owner.item
        {
            return None;
        }
        let written = find_node(
            self.source.context.files[child.file].parsed.tree(),
            &child.syntax_range,
            child.role,
        )?;
        if !self.source.callable_annotations_safe(child.file, written) {
            return None;
        }
        let body = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .last()?;
        let (equality_clause, quantified, full_body) = match instance.clauses.as_slice() {
            [equality]
                if matches!(equality.kind, ClauseKind::Equality) && weight_guard.is_none() =>
            {
                (equality, body, false)
            }
            [total, lower, upper, equality]
                if weight_guard.is_some()
                    && matches!(total.kind, ClauseKind::Equality)
                    && matches!(lower.kind, ClauseKind::Relation)
                    && matches!(upper.kind, ClauseKind::Relation)
                    && matches!(equality.kind, ClauseKind::Equality) =>
            {
                let conjunction = |node: &'a SyntaxNode| {
                    let node = unwrap(node);
                    let sides: Vec<_> = node.child_nodes().collect();
                    (node.kind() == NodeKind::BinaryExpression && sides.len() == 2
                        && self.source.core(child.file, node, &instance.view, "/\\")
                        && self.source.operation_fact(&instance.view, child.file, node)
                            .is_some_and(|fact| matches!(&fact.outcome,
                                CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.as_slice() == [TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision), TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)]
                                    && *return_type == parameters[0])))
                        .then_some(sides)
                };
                let outer = conjunction(body)?;
                let first = conjunction(outer[0])?;
                let bounds = unwrap(first[1]);
                let parts: Vec<_> = bounds.child_nodes().collect();
                let list = parts
                    .iter()
                    .find(|node| node.kind() == NodeKind::GeneratorList)?;
                let generators: Vec<_> = list.child_nodes().collect();
                let bound_body = *parts
                    .iter()
                    .find(|node| node.kind() != NodeKind::GeneratorList)?;
                let bound_sides = conjunction(bound_body)?;
                if unwrap(first[0]).range() != total.node.range() || !total.generators.is_empty()
                    || bounds.kind() != NodeKind::GeneratorCallExpression || parts.len() != 2
                    || !self.source.core(child.file, bounds, &instance.view, "forall")
                    || self.source.operation_fact(&instance.view, child.file, bounds)
                        .is_none_or(|fact| !matches!(&fact.outcome,
                            CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                                        && **element == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision))
                                && *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)))
                    || generators.len() != 1 || generators[0].child_nodes().count() != 1
                    || lower.generators.len() != 1 || upper.generators.len() != 1
                    || !std::ptr::eq(lower.generators[0], generators[0])
                    || !std::ptr::eq(upper.generators[0], generators[0])
                    || unwrap(bound_sides[0]).range() != lower.node.range()
                    || unwrap(bound_sides[1]).range() != upper.node.range()
                {
                    return None;
                }
                (equality, outer[1], true)
            }
            _ => return None,
        };
        let (output, equality, generators) =
            self.direct_forall_equality(child.file, quantified, &instance.view)?;
        if equality.range() != equality_clause.node.range()
            || equality_clause.generators.len() != generators.len()
            || equality_clause
                .generators
                .iter()
                .zip(&generators)
                .any(|(clause, source)| !std::ptr::eq(*clause, *source))
        {
            return None;
        }
        let invocation = Clause {
            file,
            item: owner.item,
            node: call,
            generators: Vec::new(),
            kind: ClauseKind::Call,
        };
        let mut mapped = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let formal = formal_parameter(self.source.context, self.source.bindings, id, position)?;
            let (source, written_actual) = self.source.actual(&invocation, id, formal, view)?;
            if source != file {
                return None;
            }
            let mut value = unwrap(written_actual);
            if value.kind() == NodeKind::CallExpression {
                // Only these already-Int views preserve the asserted axis and
                // value identities. Enum-changing views keep ordinary inspection.
                for name in ["index2int", "enum2int"] {
                    if name == "enum2int"
                        && (position != 1 || value.kind() != NodeKind::CallExpression)
                    {
                        break;
                    }
                    if !self.source.core(file, value, view, name) {
                        return None;
                    }
                    let argument = self.source.array_conversion_argument(file, value, view)?;
                    if [value, argument].into_iter().any(|node| {
                        self.source
                            .expression_type(self.source.view(file, node, view), file, node)
                            .is_none_or(|e| e.ty != *parameter)
                    }) {
                        return None;
                    }
                    value = unwrap(argument);
                }
                if self.dependencies(file, written_actual, view, &[]).is_err()
                    || self.initialized_source_safety(
                        file,
                        written_actual,
                        view,
                        &[],
                        &mut Vec::new(),
                    ) != DefinitionSafety::Supported
                    || self
                        .source
                        .closed_integer_source_error(file, written_actual, true, false)
                        .is_some()
                {
                    return None;
                }
            }
            if value.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return None;
            }
            let actual = self.source.reference(file, value)?;
            let declaration = &self.source.bindings.declarations[actual.0];
            if declaration.file != file
                || declaration.item != owner.item
                || declaration.role != DeclarationRole::Parameter
                || instance.view.declarations[formal.0].ty != *parameter
                || self
                    .source
                    .expression_type(self.source.view(file, value, view), file, value)
                    .is_none_or(|e| e.ty != *parameter)
                || mapped.iter().any(|(_, previous)| *previous == actual)
                || self.dependencies(file, value, view, &[]).is_err()
                || self.initialized_source_safety(file, value, view, &[], &mut Vec::new())
                    != DefinitionSafety::Supported
            {
                return None;
            }
            let parameter_node = find_node(
                self.source.context.files[file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )?;
            let formal_declaration = &self.source.bindings.declarations[formal.0];
            let formal_node = find_node(
                self.source.context.files[child.file].parsed.tree(),
                &formal_declaration.syntax_range,
                formal_declaration.role,
            )?;
            if self
                .type_dependencies(file, parameter_node, view, &[])
                .is_err()
                || self
                    .type_dependencies(child.file, formal_node, &instance.view, &[])
                    .is_err()
                || self
                    .source
                    .closed_integer_source_error(file, parameter_node, true, false)
                    .is_some()
            {
                return None;
            }
            let mut nodes = vec![parameter_node];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, node) {
                    return None;
                }
                nodes.extend(node.child_nodes());
            }
            mapped.push((formal, actual));
        }
        let child_pair = [
            mapped.iter().find(|(_, actual)| *actual == arrays[0])?.0,
            mapped.iter().find(|(_, actual)| *actual == arrays[1])?.0,
        ];
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, child.file, node) {
                return None;
            }
            nodes.extend(node.child_nodes());
        }
        if full_body {
            // The complete conjunction is checked as four actual clauses. Keep
            // every clause and generator source in eager arithmetic inspection.
            for clause in &instance.clauses {
                if self
                    .source
                    .closed_integer_source_error(child.file, clause.node, true, false)
                    .is_some()
                    || clause.generators.iter().any(|generator| {
                        generator.child_nodes().any(|node| {
                            self.source
                                .closed_integer_source_error(child.file, node, true, false)
                                .is_some()
                        })
                    })
                {
                    return None;
                }
            }
        } else if self
            .source
            .closed_integer_source_error(child.file, body, true, false)
            .is_some()
        {
            return None;
        }
        if full_body {
            let formals = [
                formal_parameter(self.source.context, self.source.bindings, id, 0)?,
                formal_parameter(self.source.context, self.source.bindings, id, 1)?,
                formal_parameter(self.source.context, self.source.bindings, id, 2)?,
            ];
            if output != formals[0]
                || weight_guard != Some(mapped[2].1)
                || child_pair != [formals[1], formals[2]] && child_pair != [formals[2], formals[1]]
                || !self.full_weighted_siblings(
                    child.file,
                    &instance.clauses[..3],
                    formals,
                    &instance.view,
                )
            {
                return None;
            }
        }
        // This selected invocation keeps its own relation and source checks. A
        // failed prerequisite falls back to ordinary inspection and its full causes.
        let dependencies = self.nested_weighted_dependencies(
            child.file,
            equality,
            output,
            &generators,
            child_pair,
            &instance.view,
        )?;
        let dependencies = dependencies
            .into_iter()
            .map(|formal| {
                mapped
                    .iter()
                    .find(|(child, _)| *child == formal)
                    .map(|(_, actual)| *actual)
            })
            .collect::<Option<Vec<_>>>()?;
        Some((
            Output {
                target: mapped.iter().find(|(child, _)| *child == output)?.1,
                dependencies,
                coverage: DefinitionCoverage::WholeArray,
                location: self.source.context.files[file].location(call.range()),
                enforced_boolean: false,
                scoped_local: false,
            },
            arrays,
        ))
    }
    pub(in crate::callable_definitions) fn full_weighted_siblings(
        &self,
        file: FileId,
        clauses: &[Clause<'a>],
        [load, bin, weight]: [DeclarationId; 3],
        view: &CallableFacts,
    ) -> bool {
        let [total, lower, upper] = clauses else {
            return false;
        };
        let par_int = TypeInst::par(TypeKind::Int);
        let var_int = par_int.clone().with_inst(Instantiation::Decision);
        let var_bool = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let par_set = TypeInst::par(TypeKind::Set(Box::new(par_int.clone())));
        let typed = |node: &SyntaxNode, ty: &TypeInst| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .is_some_and(|e| e.ty == *ty)
        };
        let sides: Vec<_> = total.node.child_nodes().map(unwrap).collect();
        if sides.len() != 2 || !self.source.core(file, total.node, view, "=")
            || !typed(total.node, &var_bool)
            || self.source.operation_fact(self.source.view(file, total.node, view), file, total.node)
                .is_none_or(|fact| !matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.as_slice() == [var_int.clone(), par_int.clone()] && *return_type == var_bool))
        {
            return false;
        }
        // A targetless sum equality still has two evaluated sources. It grants
        // no output; neither aggregate may be skipped because target() fails.
        for (side, expected) in sides.iter().zip([load, weight]) {
            let arguments: Vec<_> = side.child_nodes().map(unwrap).collect();
            if side.kind() != NodeKind::CallExpression
                || arguments.len() != 1
                || !self.source.core(file, side, view, "sum")
                || arguments[0].kind() != NodeKind::Expression
                || self.source.reference(file, arguments[0]) != Some(expected)
                || self.dependencies(file, arguments[0], view, &[]).is_err()
                || self.aggregate_source_safety(file, side, view, &[])
                    != Some(DefinitionSafety::Supported)
            {
                return false;
            }
        }
        for (position, relation) in [lower, upper].iter().enumerate() {
            let operands: Vec<_> = relation.node.child_nodes().map(unwrap).collect();
            let Some(generator) = relation.generators.first() else {
                return false;
            };
            let Some(source) = generator.child_nodes().next() else {
                return false;
            };
            let expected_operands = if position == 0 {
                [par_int.clone(), var_int.clone()]
            } else {
                [var_int.clone(), par_int.clone()]
            };
            if operands.len() != 2
                || !self.source.core(file, relation.node, view, "<=")
                || self.source.integer_index_set(file, source, view) != Some(bin)
                || self
                    .relation_iterations(file, &relation.generators, 0, view, false)
                    .is_err()
                || !typed(relation.node, &var_bool)
                || self
                    .source
                    .operation_fact(
                        self.source.view(file, relation.node, view),
                        file,
                        relation.node,
                    )
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == expected_operands
                            && *return_type == var_bool)
                    })
            {
                return false;
            }
            let extremum = operands[position];
            let arguments: Vec<_> = extremum.child_nodes().collect();
            let name = if position == 0 { "min" } else { "max" };
            let selected = operands[1 - position];
            let selection: Vec<_> = selected.child_nodes().map(unwrap).collect();
            if extremum.kind() != NodeKind::CallExpression
                || arguments.len() != 1
                || !self.source.core(file, extremum, view, name)
                || self.source.integer_index_set(file, arguments[0], view) != Some(load)
                || !typed(extremum, &par_int)
                || self
                    .source
                    .operation_fact(self.source.view(file, extremum, view), file, extremum)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [par_set.clone()] && *return_type == par_int)
                    })
                || selected.kind() != NodeKind::ArrayAccessExpression
                || selection.len() != 2
                || selection
                    .iter()
                    .any(|node| node.kind() != NodeKind::Expression)
                || self.source.reference(file, selection[0]) != Some(bin)
                || !self
                    .source
                    .member_index(file, bin, selection[1], &relation.generators, view)
                || !typed(selected, &var_int)
                || !typed(selection[1], &par_int)
            {
                return false;
            }
            if self
                .boolean_relation_dependencies(file, relation.node, view, &relation.generators)
                .is_err()
                && !matches!(
                    self.initialized_children_safety(
                        file,
                        &[relation.node, source],
                        view,
                        &relation.generators
                    ),
                    DefinitionSafety::Unknown(_)
                )
            {
                return false;
            }
            // This is the existing nondefining relation inspection: checked
            // uncertainty about min/max supplies no nonempty-load certificate.
        }
        true
    }
    pub(in crate::callable_definitions) fn nested_weighted_dependencies(
        &self,
        file: FileId,
        equality: &'a SyntaxNode,
        output: DeclarationId,
        generators: &[&'a SyntaxNode],
        pair: [DeclarationId; 2],
        view: &CallableFacts,
    ) -> Option<Vec<DeclarationId>> {
        let present = |node: &SyntaxNode, kind, instantiation| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .is_some_and(|e| {
                    e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.kind == kind
                        && e.ty.instantiation == instantiation
                })
        };
        let sides: Vec<_> = equality.child_nodes().map(unwrap).collect();
        let position = sides.iter().position(|side| {
            self.source.target(file, side, view, generators)
                == Some((output, DefinitionCoverage::WholeArray))
        })?;
        if sides.len() != 2
            || !present(equality, TypeKind::Bool, Instantiation::Decision)
            || !present(sides[position], TypeKind::Int, Instantiation::Decision)
        {
            return None;
        }
        let sum = sides[1 - position];
        if sum.kind() != NodeKind::GeneratorCallExpression || !self.source.core(file, sum, view, "sum")
            || !present(sum, TypeKind::Int, Instantiation::Decision)
            || !self.source.operation_fact(self.source.view(file, sum, view), file, sum)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if *return_type == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision) && parameters.len() == 1
                        && parameters[0].known() && !optional(&parameters[0])
                        && parameters[0].instantiation == Instantiation::Decision
                        && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                            if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                                && **element == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision))))
        {
            return None;
        }
        let parts: Vec<_> = sum.child_nodes().collect();
        if parts.len() != 2 {
            return None;
        }
        let list = *parts
            .iter()
            .find(|node| node.kind() == NodeKind::GeneratorList)?;
        let inner: Vec<_> = list.child_nodes().collect();
        if inner.len() != 1 || inner[0].child_nodes().count() != 1 {
            return None;
        }
        let source = inner[0].child_nodes().next()?;
        let bin = self.source.integer_index_set(file, source, view)?;
        let mut all = generators.to_vec();
        all.extend(inner.iter().copied());
        if self.source.iterations(file, &all, view).is_err()
            || self
                .relation_iterations(file, &all, generators.len(), view, false)
                .is_err()
            || self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Generator
                        && declaration.syntax_range == inner[0].range()
                })
                .count()
                != 1
        {
            return None;
        }
        let product = unwrap(
            *parts
                .iter()
                .find(|node| node.kind() != NodeKind::GeneratorList)?,
        );
        let operands: Vec<_> = product.child_nodes().map(unwrap).collect();
        if product.kind() != NodeKind::BinaryExpression || operands.len() != 2
            || !self.source.core(file, product, view, "*")
            || !present(product, TypeKind::Int, Instantiation::Decision)
            || !self.source.operation_fact(self.source.view(file, product, view), file, product)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.as_slice() == [TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision), TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision)]
                        && *return_type == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision)
                        && parameters.iter().zip(&operands).all(|(formal, actual)|
                            self.source.expression_type(self.source.view(file, actual, view), file, actual)
                                .is_some_and(|e| crate::types::coerces(&e.ty, formal)))))
        {
            return None;
        }
        let weight_position = operands.iter().position(|node| {
            node.kind() == NodeKind::ArrayAccessExpression
                && present(node, TypeKind::Int, Instantiation::Parameter)
        })?;
        let weight = operands[weight_position];
        let selection: Vec<_> = weight.child_nodes().map(unwrap).collect();
        if selection.len() != 2
            || selection
                .iter()
                .any(|node| node.kind() != NodeKind::Expression)
            || !present(selection[1], TypeKind::Int, Instantiation::Parameter)
            || self.source.reference(file, selection[1]).is_none_or(|id| {
                let d = &self.source.bindings.declarations[id.0];
                d.file != file
                    || d.role != DeclarationRole::Generator
                    || d.syntax_range != inner[0].range()
            })
        {
            return None;
        }
        let weight_array = self.source.reference(file, selection[0])?;
        if pair != [bin, weight_array] && pair != [weight_array, bin] {
            return None;
        }
        let comparison = operands[1 - weight_position];
        let compared: Vec<_> = comparison.child_nodes().map(unwrap).collect();
        if comparison.kind() != NodeKind::BinaryExpression || compared.len() != 2
            || !self.source.core(file, comparison, view, "=")
            || !present(comparison, TypeKind::Bool, Instantiation::Decision)
            || !self.source.operation_fact(self.source.view(file, comparison, view), file, comparison)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
                        && parameters.iter().zip(&compared).all(|(formal, actual)| formal.known()
                            && !optional(formal) && formal.kind == TypeKind::Int
                            && self.source.expression_type(self.source.view(file, actual, view), file, actual)
                                .is_some_and(|e| crate::types::coerces(&e.ty, formal)))))
        {
            return None;
        }
        let bin_position = compared.iter().position(|node| {
            let children: Vec<_> = node.child_nodes().map(unwrap).collect();
            node.kind() == NodeKind::ArrayAccessExpression
                && children.len() == 2
                && children[0].kind() == NodeKind::Expression
                && self.source.reference(file, children[0]) == Some(bin)
                && self.source.reference(file, children[1])
                    == self.source.reference(file, selection[1])
                && self.source.member_index(file, bin, children[1], &all, view)
                && present(node, TypeKind::Int, Instantiation::Decision)
        })?;
        let outer_index = compared[1 - bin_position];
        if outer_index.kind() != NodeKind::Expression
            || !present(outer_index, TypeKind::Int, Instantiation::Parameter)
            || self.source.reference(file, outer_index).is_none_or(|id| {
                let d = &self.source.bindings.declarations[id.0];
                d.file != file
                    || d.role != DeclarationRole::Generator
                    || !generators
                        .iter()
                        .any(|generator| generator.range() == d.syntax_range)
            })
        {
            return None;
        }
        // Only weight[i] needs the invocation's index-set equality. All headers
        // and remaining operands keep strict source/membership dependencies.
        let mut dependencies = self.dependencies(file, selection[0], view, &all).ok()?;
        extend(
            &mut dependencies,
            self.dependencies(file, selection[1], view, &all).ok()?,
        );
        extend(
            &mut dependencies,
            self.dependencies(file, comparison, view, &all).ok()?,
        );
        extend(
            &mut dependencies,
            self.dependencies(file, source, view, generators).ok()?,
        );
        for generator in generators {
            // This exact index_set(output) is inspected as the target's traversal,
            // not a dependency on the output values themselves.
            self.dependencies(file, generator.child_nodes().next()?, view, &[])
                .ok()?;
        }
        Some(dependencies)
    }
    // Only this complete owning assertion can admit a constructed actual pair.
    pub(in crate::callable_definitions) fn asserted_forall_contract(
        &self,
        instance: &Instance<'a>,
    ) -> Option<(DeclarationId, [DeclarationId; 2])> {
        let [body] = instance.clauses.as_slice() else {
            return None;
        };
        if !matches!(body.kind, ClauseKind::Assertion) {
            return None;
        }
        self.direct_asserted_forall(body.file, body.node, &instance.view)
            .or_else(|| {
                self.nested_asserted_forall(body.file, body.node, &instance.view)
                    .map(|(output, arrays)| (output.target, arrays))
            })
    }
    pub(in crate::callable_definitions) fn constructed_array_actual(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        lexical: &[&'a SyntaxNode],
    ) -> Result<Option<ConstructedArray>, String> {
        let written_actual = node;
        let node = unwrap(node);
        if node.kind() != NodeKind::ArrayComprehension {
            return Ok(None);
        }
        // The whole source graph remains an error veto. Its generic collection
        // uncertainty is resolved only by the exact shape/membership checks below.
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_source_safety(file, written_actual, view, lexical, &mut Vec::new())
        {
            return Err(reason);
        }
        if let Some(reason) =
            self.source
                .closed_integer_source_error(file, written_actual, true, true)
        {
            return Err(reason);
        }
        let present_int = |ty: &TypeInst, inst| {
            ty.known() && !optional(ty) && ty.instantiation == inst && ty.kind == TypeKind::Int
        };
        let integer_set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if present_int(element, Instantiation::Parameter))
        };
        let facts = self.source.view(file, node, view);
        let ty = &self
            .source
            .expression_type(facts, file, node)
            .ok_or("constructed array actual type is unavailable")?
            .ty;
        let TypeKind::Array { indices, element } = &ty.kind else {
            return Err("constructed actual must retain an integer array type".into());
        };
        if !ty.known()
            || optional(ty)
            || indices.len() != 1
            || !present_int(&indices[0], Instantiation::Parameter)
            || !element.known()
            || optional(element)
            || element.kind != TypeKind::Int
            || element.instantiation != ty.instantiation
        {
            return Err("constructed array qualifiers or axis are unsupported".into());
        }
        let [outer] = lexical else {
            return Err("constructed array requires one owning row traversal".into());
        };
        let children: Vec<_> = node.child_nodes().collect();
        let [body, list] = children.as_slice() else {
            return Err("constructed array body or headers are unsupported".into());
        };
        if list.kind() != NodeKind::GeneratorList {
            return Err("constructed array headers are unavailable".into());
        }
        let headers: Vec<_> = list.child_nodes().collect();
        let [inner] = headers.as_slice() else {
            return Err("constructed array requires one inner traversal".into());
        };
        let all = [*outer, *inner];
        let mut binders = Vec::new();
        let mut dependencies = Vec::new();
        for (position, generator) in all.iter().enumerate() {
            let declared: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generator.range()
                })
                .collect();
            let [binder] = declared.as_slice() else {
                return Err("constructed traversal binder identity is unsupported".into());
            };
            if crate::domains::generator_slots(&self.source.context.files[file].parsed, generator)
                != 1
                || generator.child_nodes().count() != 1
                || !present_int(
                    &facts.declarations[binder.id.0].ty,
                    Instantiation::Parameter,
                )
            {
                return Err(
                    "constructed traversal is filtered, assigned or not present Int".into(),
                );
            }
            let source = generator
                .child_nodes()
                .next()
                .ok_or("constructed source unavailable")?;
            if self
                .source
                .expression_type(self.source.view(file, source, facts), file, source)
                .is_none_or(|e| !integer_set(&e.ty))
            {
                return Err(
                    "constructed traversal source is not a present parameter Int set".into(),
                );
            }
            match self.initialized_source_safety(
                file,
                source,
                view,
                &all[..position],
                &mut Vec::new(),
            ) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
            extend(
                &mut dependencies,
                self.dependencies(file, source, view, &all[..position])?,
            );
            binders.push(binder.id);
        }
        if binders[0] == binders[1] {
            return Err("constructed row selectors must be distinct binders".into());
        }
        self.source.iterations(file, &all, view)?;
        self.relation_iterations(file, &all, 0, view, false)?;
        let mut inspected = vec![written_actual, *outer];
        while let Some(part) = inspected.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, part) {
                return Err("constructed actual or header annotation is unsupported".into());
            }
            inspected.extend(part.child_nodes());
        }
        let body = unwrap(body);
        if body.kind() != NodeKind::ArrayAccessExpression {
            return Err("constructed array element must retain a direct array selection".into());
        }
        let selected: Vec<_> = body.child_nodes().collect();
        let subject = unwrap(
            selected
                .first()
                .ok_or("constructed selection owner unavailable")?,
        );
        let bare = |value: &SyntaxNode| {
            value.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        };
        let target = self
            .source
            .reference(file, subject)
            .filter(|_| bare(subject))
            .ok_or("constructed selection requires a bare array owner")?;
        let owner = &self.source.bindings.declarations[target.0];
        if owner.file != file || owner.role != DeclarationRole::Value || !owner.top_level {
            return Err("constructed selection owner scope is unsupported".into());
        }
        let owner_type = &facts.declarations[target.0].ty;
        let TypeKind::Array {
            indices: owner_axes,
            element: owner_element,
        } = &owner_type.kind
        else {
            return Err("constructed selection owner type is unavailable".into());
        };
        if !owner_type.known()
            || optional(owner_type)
            || owner_type.instantiation != ty.instantiation
            || owner_element != element
            || !owner_axes
                .iter()
                .all(|axis| present_int(axis, Instantiation::Parameter))
            || self
                .source
                .expression_type(self.source.view(file, body, facts), file, body)
                .is_none_or(|e| &e.ty != element.as_ref())
        {
            return Err("constructed selection type or optionality is unsupported".into());
        }
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("constructed selection declaration unavailable")?;
        self.type_dependencies(file, written, view, &[])?;
        match self.initialized_source_safety(file, subject, view, lexical, &mut Vec::new()) {
            DefinitionSafety::Supported => {}
            DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                return Err(reason);
            }
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, written, true, true)
        {
            return Err(reason);
        }
        let mut declarations = vec![written];
        while let Some(part) = declarations.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, part) {
                return Err("constructed selection declaration annotation is unsupported".into());
            }
            declarations.extend(part.child_nodes());
        }
        let selectors = &selected[1..];
        for selector in selectors {
            let selector = unwrap(selector);
            if !bare(selector)
                || self
                    .source
                    .expression_type(self.source.view(file, selector, facts), file, selector)
                    .is_none_or(|e| !present_int(&e.ty, Instantiation::Parameter))
            {
                return Err(
                    "constructed element selector is not a bare parameter Int binder".into(),
                );
            }
            match self.initialized_source_safety(file, selector, view, &all, &mut Vec::new()) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
        }
        let source = unwrap(
            inner
                .child_nodes()
                .next()
                .ok_or("constructed inner source unavailable")?,
        );
        let (item_source, coverage) = if owner_axes.len() == 1 && selectors.len() == 1 {
            if self.source.reference(file, unwrap(selectors[0])) != Some(binders[1])
                || source.kind() != NodeKind::ArrayAccessExpression
            {
                return Err("constructed item selection lacks its exact set traversal".into());
            }
            let parts: Vec<_> = source.child_nodes().map(unwrap).collect();
            let [set_array, row] = parts.as_slice() else {
                return Err("constructed item set must retain one row selector".into());
            };
            let set_id = self
                .source
                .reference(file, set_array)
                .filter(|_| bare(set_array))
                .ok_or("constructed item set array identity is unavailable")?;
            if !bare(row) || self.source.reference(file, row) != Some(binders[0]) {
                return Err("constructed item set must retain the owning outer binder".into());
            }
            let set_type = &facts.declarations[set_id.0].ty;
            if !set_type.known()
                || optional(set_type)
                || set_type.instantiation != Instantiation::Parameter
                || !matches!(&set_type.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && present_int(&indices[0], Instantiation::Parameter)
                        && integer_set(element))
            {
                return Err(
                    "constructed item source is not a present parameter array of Int sets".into(),
                );
            }
            let set_owner = &self.source.bindings.declarations[set_id.0];
            if set_owner.file != file
                || set_owner.role != DeclarationRole::Value
                || !set_owner.top_level
            {
                return Err("constructed item set declaration scope is unsupported".into());
            }
            let set_written = find_node(
                self.source.context.files[file].parsed.tree(),
                &set_owner.syntax_range,
                set_owner.role,
            )
            .ok_or("constructed item set declaration unavailable")?;
            self.type_dependencies(file, set_written, view, &[])?;
            let Domain::Array { indices, element } = crate::domains::bare_index_domain(
                &self.source.domains.declarations[set_id.0].domain,
            ) else {
                return Err("constructed item set written domains are unavailable".into());
            };
            let Domain::Set(bound) = element.as_ref() else {
                return Err("constructed item set requires a written element bound".into());
            };
            let Domain::Array {
                indices: destination,
                ..
            } = crate::domains::bare_index_domain(
                &self.source.domains.declarations[target.0].domain,
            )
            else {
                return Err("constructed item destination axis unavailable".into());
            };
            if indices.len() != 1
                || destination.len() != 1
                || !matches!(bound.as_ref(), Domain::Named { .. } | Domain::Range { .. })
                || crate::domains::same_members(bound, &destination[0])? != Some(true)
            {
                return Err(
                    "constructed item set bound does not prove destination membership".into(),
                );
            }
            // This is subset membership only. Neither the selected set's count
            // nor complete coverage of the input owner follows from its bound.
            (
                Some((file, set_id, binders[0])),
                DefinitionCoverage::ArrayElement,
            )
        } else if owner_axes.len() == 2
            && selectors.len() == 2
            && ty.instantiation == Instantiation::Decision
            && self.source.reference(file, unwrap(selectors[0])) == Some(binders[0])
            && self.source.reference(file, unwrap(selectors[1])) == Some(binders[1])
            && bare(source)
        {
            if dependencies.contains(&target) {
                return Err("constructed output traversal depends on destination values".into());
            }
            let coverage = complete_array_coverage(
                self.source.context,
                self.source.bindings,
                (self.source.calls, facts),
                self.source.instantiations,
                self.source.domains,
                (file, target),
                (selectors, &all),
            );
            if coverage != DefinitionCoverage::WholeArray {
                return Err("constructed output row lacks complete rectangular traversal".into());
            }
            self.dependencies(file, body, view, &all)?;
            (None, coverage)
        } else {
            return Err(
                "constructed actual is not a supported item array or complete output row".into(),
            );
        };
        extend(&mut dependencies, vec![target]);
        Ok(Some(ConstructedArray {
            target,
            item_source,
            dependencies,
            coverage,
        }))
    }
    pub(in crate::callable_definitions) fn constructed_call_row(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        instance: &Instance<'a>,
        formal: DeclarationId,
    ) -> Result<Option<ConstructedArray>, String> {
        if instance.recursive
            || self
                .asserted_forall_contract(instance)
                .is_none_or(|(output, _)| output != formal)
        {
            return Ok(None);
        }
        self.asserted_call_indices(clause, view, instance, true)?;
        let (file, node) = self
            .source
            .actual(clause, instance.id, formal, view)
            .ok_or("constructed output actual/default correspondence is unavailable")?;
        let lexical = if file == clause.file
            && clause.node.range().start <= node.range().start
            && node.range().end <= clause.node.range().end
        {
            &clause.generators[..]
        } else {
            &[]
        };
        let Some(actual) = self.constructed_array_actual(file, node, view, lexical)? else {
            return Ok(None);
        };
        if actual.item_source.is_some() || actual.coverage != DefinitionCoverage::WholeArray {
            return Ok(None);
        }
        Ok(Some(actual))
    }
    pub(in crate::callable_definitions) fn complete_constructed_call_output(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        target: DeclarationId,
    ) -> bool {
        let Some((id, parameters)) = self.source.resolved(clause.file, clause.node, view) else {
            return false;
        };
        let Some(instance) = self
            .instances
            .iter()
            .find(|i| i.id == id && i.parameters == parameters)
        else {
            return false;
        };
        known
            .iter()
            .filter(|g| {
                g.callable == id
                    && g.parameters == parameters
                    && g.coverage == DefinitionCoverage::WholeArray
                    && self.source.bindings.declarations[g.target.0].role
                        == DeclarationRole::Parameter
            })
            .any(|g| {
                self.constructed_call_row(clause, view, instance, g.target)
                    .is_ok_and(|actual| actual.is_some_and(|actual| actual.target == target))
            })
    }
    pub(in crate::callable_definitions) fn asserted_call_indices(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        instance: &Instance<'a>,
        allow_constructed: bool,
    ) -> Result<(), String> {
        let Some((_, arrays)) = self.asserted_forall_contract(instance) else {
            return Ok(());
        };
        if allow_constructed && !instance.recursive {
            let actuals: Vec<_> = arrays
                .iter()
                .map(|formal| {
                    self.source
                        .actual(clause, instance.id, *formal, view)
                        .ok_or("asserted array actual/default mapping is unavailable")
                })
                .collect::<Result<_, _>>()?;
            if actuals
                .iter()
                .all(|(_, node)| unwrap(node).kind() == NodeKind::ArrayComprehension)
            {
                let mut sources = Vec::new();
                for ((file, node), formal) in actuals.into_iter().zip(arrays) {
                    if self
                        .source
                        .expression_type(self.source.view(file, node, view), file, node)
                        .is_none_or(|e| e.ty != instance.view.declarations[formal.0].ty)
                    {
                        return Err(
                            "constructed asserted actual does not retain its selected formal type"
                                .into(),
                        );
                    }
                    let lexical = if file == clause.file
                        && clause.node.range().start <= node.range().start
                        && node.range().end <= clause.node.range().end
                    {
                        &clause.generators[..]
                    } else {
                        &[]
                    };
                    let actual = self
                        .constructed_array_actual(file, node, view, lexical)?
                        .ok_or("asserted constructed array actual unavailable")?;
                    sources.push(actual.item_source.ok_or(
                        "asserted constructed input requires a checked item-set selection",
                    )?);
                }
                // Both implicit axes are 1..card(this exact inspected set),
                // including zero and noncontiguous members. No count is guessed.
                return if sources[0] == sources[1] {
                    Ok(())
                } else {
                    Err("constructed asserted array axes have different source identities".into())
                };
            }
        }
        let mut axes = Vec::new();
        for formal in arrays {
            let (file, value) = self
                .source
                .actual(clause, instance.id, formal, view)
                .ok_or("asserted array actual/default mapping is unavailable")?;
            let value = unwrap(value);
            if value.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("asserted array actual/default must be a bare array identity".into());
            }
            let id = self
                .source
                .reference(file, value)
                .ok_or("asserted array actual identity is unavailable")?;
            let facts = self.source.view(file, value, view);
            let ty = &facts.declarations[id.0].ty;
            if !ty.known()
                || optional(ty)
                || !matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Int)
                || self
                    .source
                    .expression_type(facts, file, value)
                    .is_none_or(|e| e.ty != *ty)
            {
                return Err("asserted array actual type or optionality is unsupported".into());
            }
            let owner = &self.source.bindings.declarations[id.0];
            if !matches!(
                owner.role,
                DeclarationRole::Value | DeclarationRole::Parameter
            ) || owner.role == DeclarationRole::Value && !owner.top_level
            {
                return Err("asserted array actual declaration scope is unsupported".into());
            }
            let written = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("asserted array actual declaration is unavailable")?;
            let Domain::Array { indices, .. } = &self.source.domains.declarations[id.0].domain
            else {
                return Err("asserted array written axis is unavailable".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("asserted array must retain one written axis".into());
            };
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, owner.file, node) {
                    return Err("asserted array source annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            self.type_dependencies(owner.file, written, view, &[])?;
            match self.initialized_source_safety(file, value, view, &[], &mut Vec::new()) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
            for initializer in written.child_nodes().filter(|n| is_expression(n.kind())) {
                let initializer = unwrap(initializer);
                if initializer.kind() != NodeKind::ArrayLiteral
                    || initializer
                        .child_nodes()
                        .any(|cell| cell.kind() == NodeKind::IndexedArrayEntry)
                {
                    return Err("asserted array initializer shape is unproved".into());
                }
                let TypeKind::Array { element, .. } = &ty.kind else {
                    return Err("asserted array literal element type is unavailable".into());
                };
                for cell in initializer.child_nodes() {
                    if self
                        .source
                        .expression_type(self.source.view(owner.file, cell, view), owner.file, cell)
                        .is_none_or(|e| {
                            !e.ty.known()
                                || optional(&e.ty)
                                || e.ty.kind != TypeKind::Int
                                || !crate::types::coerces(&e.ty, element)
                        })
                    {
                        return Err("asserted array literal cell type is unsupported".into());
                    }
                }
                match self.initialized_source_safety(
                    owner.file,
                    initializer,
                    view,
                    &[],
                    &mut Vec::new(),
                ) {
                    DefinitionSafety::Supported => {}
                    DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                        return Err(reason);
                    }
                }
                if let Some(reason) =
                    self.source
                        .closed_integer_source_error(owner.file, initializer, true, false)
                {
                    return Err(reason);
                }
                let count = i64::try_from(initializer.child_nodes().count())
                    .map_err(|_| "asserted array literal extent is unsupported")?;
                let literal_axis = Domain::Range {
                    lower: crate::NumericBound::Integer(1),
                    upper: crate::NumericBound::Integer(count),
                };
                if crate::domains::same_members(axis, &literal_axis)? != Some(true) {
                    return Err(
                        "asserted array initializer does not retain its written axis".into(),
                    );
                }
            }
            axes.push(axis);
        }
        match crate::domains::same_members(axes[0], axes[1])? {
            Some(true) => Ok(()),
            Some(false) => Err("asserted array index equality is false for these actuals".into()),
            None => Err("asserted array actual index equality is unproved".into()),
        }
    }
}

impl<'a> SourceInspector<'a> {
    // A closed domain error must not disappear behind source-safety Unknown.
    // Plain integer parameters and checked count views permit header inspection,
    // without deriving a count or index membership.
    pub(in crate::callable_definitions) fn uncertain_equality_header(
        &self,
        file: FileId,
        source: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let numeric = |domain: Domain| {
            let mut domains = vec![domain];
            while let Some(domain) = domains.pop() {
                match domain {
                    Domain::Named { domain, .. } | Domain::Set(domain) => domains.push(*domain),
                    Domain::Array { indices, element } => {
                        domains.extend(indices);
                        domains.push(*element);
                    }
                    Domain::Range { lower, upper }
                        if !matches!(
                            (&lower, &upper),
                            (
                                crate::domains::NumericBound::Integer(_),
                                crate::domains::NumericBound::Integer(_)
                            )
                        ) =>
                    {
                        return false;
                    }
                    Domain::LiteralSet(bounds)
                        if bounds.is_empty()
                            || bounds.iter().any(|bound| {
                                !matches!(bound, crate::domains::NumericBound::Integer(_))
                            }) =>
                    {
                        return false;
                    }
                    domain if domain.numeric_minimum().is_err() => return false,
                    _ => {}
                }
            }
            true
        };
        let count = |file: FileId, node: &SyntaxNode| {
            let node = unwrap(node);
            let facts = self.view(file, node, view);
            let Some(argument) = length_argument(self.context, self.bindings, facts, file, node)
                .or_else(|| {
                    crate::domains::parameter_set_cardinality(
                        self.context,
                        self.bindings,
                        facts,
                        file,
                        node,
                    )
                    .and_then(|_| node.child_nodes().next())
                })
            else {
                return false;
            };
            let argument = unwrap(argument);
            if argument.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, argument).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return false;
            }
            let Some(id) = self.reference(file, argument) else {
                return false;
            };
            let declaration = &self.bindings.declarations[id.0];
            if declaration.role != DeclarationRole::Value
                || !declaration.top_level
                || !numeric(self.domains.declarations[id.0].domain.clone())
            {
                return false;
            }
            let Some(written) = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return false;
            };
            if written.child_nodes().take(1).any(|ty| {
                !numeric(expression_domain(
                    self.context,
                    self.bindings,
                    declaration.file,
                    ty,
                ))
            }) {
                return false;
            }
            // Length does not evaluate cells, but its source must not hide a
            // closed failure. Computed/reshaped initializers stay outside this
            // local admission; a literal retains only checked scalar cells.
            for initializer in written
                .child_nodes()
                .filter(|child| is_expression(child.kind()))
            {
                let initializer = unwrap(initializer);
                if matches!(self.calls.declarations[id.0].ty.kind, TypeKind::Set(_)) {
                    if !matches!(initializer.kind(), NodeKind::RangeExpression | NodeKind::SetLiteral)
                        || initializer.child_nodes().any(|bound| {
                            let bound = unwrap(bound);
                            !matches!(crate::domains::tokens(&self.context.files[declaration.file].parsed, bound).as_slice(),
                                [token] if token.kind == TokenKind::IntegerLiteral)
                        })
                    {
                        return false;
                    }
                    continue;
                }
                if initializer.kind() != NodeKind::ArrayLiteral
                    || initializer.child_nodes().next().is_none()
                {
                    return false;
                }
                for cell in initializer.child_nodes() {
                    let cell = unwrap(cell);
                    if cell.kind() != NodeKind::Expression
                        || crate::domains::expression_integer(
                            self.context,
                            self.bindings,
                            declaration.file,
                            cell,
                        )
                        .is_err()
                    {
                        return false;
                    }
                    if let Some(cell_id) = self.reference(declaration.file, cell) {
                        let cell_declaration = &self.bindings.declarations[cell_id.0];
                        if cell_declaration.role != DeclarationRole::Value
                            || !cell_declaration.top_level
                            || !numeric(self.domains.declarations[cell_id.0].domain.clone())
                        {
                            return false;
                        }
                        let Some(cell_written) = find_node(
                            self.context.files[cell_declaration.file].parsed.tree(),
                            &cell_declaration.syntax_range,
                            cell_declaration.role,
                        ) else {
                            return false;
                        };
                        if cell_written.child_nodes().any(|n| is_expression(n.kind())) {
                            return false;
                        }
                    }
                }
            }
            true
        };
        let mut source = unwrap(source);
        let mut file = file;
        let mut active = Vec::new();
        while source.kind() == NodeKind::Expression {
            let Some(id) = self.reference(file, source) else {
                return false;
            };
            let declaration = &self.bindings.declarations[id.0];
            if active.contains(&id)
                || declaration.role != DeclarationRole::Value
                || !declaration.top_level
            {
                return false;
            }
            active.push(id);
            let Some(written) = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return false;
            };
            if written.child_nodes().take(1).any(|ty| {
                !numeric(expression_domain(
                    self.context,
                    self.bindings,
                    declaration.file,
                    ty,
                ))
            }) {
                return false;
            }
            let ty = &self.calls.declarations[id.0].ty;
            if !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int
                    && element.known() && !optional(element))
            {
                return false;
            }
            let Some(initializer) = written
                .child_nodes()
                .find(|child| is_expression(child.kind()))
            else {
                return self.domains.declarations[id.0]
                    .domain
                    .numeric_minimum()
                    .is_ok();
            };
            source = unwrap(initializer);
            file = declaration.file;
        }
        let domain = expression_domain(self.context, self.bindings, file, source);
        let mut constants = vec![source];
        let mut closed = true;
        while let Some(value) = constants.pop() {
            if value.kind() == NodeKind::Expression
                && !matches!(crate::domains::tokens(&self.context.files[file].parsed, value).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
            {
                closed = false;
            }
            constants.extend(value.child_nodes());
        }
        if closed && domain.numeric_minimum().is_ok() {
            return true;
        }
        let bounds: Vec<_> = source.child_nodes().map(unwrap).collect();
        if source.kind() != NodeKind::RangeExpression
            || bounds.len() != 2
            || crate::domains::expression_integer(self.context, self.bindings, file, bounds[0])
                != Ok(Some(1))
        {
            return false;
        }
        let upper = bounds[1];
        if upper.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.context.files[file].parsed, upper).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            && let Some(id) = self.reference(file, upper)
        {
            let declaration = &self.bindings.declarations[id.0];
            if declaration.role == DeclarationRole::Value
                && declaration.top_level
                && self.calls.declarations[id.0].ty == TypeInst::par(TypeKind::Int)
                && self
                    .expression_type(self.view(file, upper, view), file, upper)
                    .is_some_and(|expression| expression.ty == self.calls.declarations[id.0].ty)
                && matches!(
                    self.domains.declarations[id.0].domain,
                    Domain::UnconstrainedInt
                )
                && let Some(written) = find_node(
                    self.context.files[declaration.file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
                && !written
                    .child_nodes()
                    .any(|child| is_expression(child.kind()))
                && self.source_annotations_safe(declaration.file, written)
                && written.child_nodes().next().is_some_and(|ty| {
                    ty.kind() == NodeKind::ScalarType
                        && ty.child_nodes().next().is_none()
                        && self.source_annotations_safe(declaration.file, ty)
                        && matches!(
                            expression_domain(self.context, self.bindings, declaration.file, ty,),
                            Domain::UnconstrainedInt
                        )
                })
            {
                return true;
            }
        }
        if count(file, upper) {
            return true;
        }
        let operands: Vec<_> = upper.child_nodes().collect();
        upper.kind() == NodeKind::BinaryExpression
            && operands.len() == 2
            && self.core(file, upper, view, "div")
            && operands.iter().all(|operand| count(file, operand))
    }
    pub(in crate::callable_definitions) fn scoped_local(
        &self,
        clause: &Clause<'a>,
        target: DeclarationId,
        view: &CallableFacts,
        kind: TypeKind,
    ) -> bool {
        let declaration = &self.bindings.declarations[target.0];
        let ty = &view.declarations[target.0].ty;
        let source = &self.context.files[clause.file];
        if declaration.file != clause.file
            || declaration.item != clause.item
            || declaration.role != DeclarationRole::Local
            || !ty.known()
            || optional(ty)
            || ty.kind != kind
            || ty.instantiation != Instantiation::Decision
            || source.kind != SourceKind::User
        {
            return false;
        }
        let Some(mut node) = source
            .parsed
            .tree()
            .child_nodes()
            .nth(clause.item)
            .filter(|n| n.kind() == NodeKind::Constraint)
        else {
            return false;
        };
        let contains = |outer: &SyntaxNode, inner: &SyntaxNode| {
            outer.range().start <= inner.range().start && inner.range().end <= outer.range().end
        };
        let mut ancestors = Vec::new();
        while node.range() != clause.node.range() {
            ancestors.push(node);
            let Some(child) = node.child_nodes().find(|n| contains(n, clause.node)) else {
                return false;
            };
            node = child;
        }
        let Some(owning) = ancestors.iter().copied().chain([clause.node]).find(|n| {
            n.kind() == NodeKind::LetExpression
                && n.child_nodes().any(|block| {
                    block.kind() == NodeKind::LetBlock
                        && block
                            .child_nodes()
                            .any(|d| d.range() == declaration.syntax_range)
                })
        }) else {
            return false;
        };
        ancestors
            .iter()
            .enumerate()
            .all(|(position, node)| match node.kind() {
                NodeKind::Constraint
                | NodeKind::ParenthesizedExpression
                | NodeKind::LetExpression
                | NodeKind::LetBlock => true,
                NodeKind::BinaryExpression => self.core(clause.file, node, view, "/\\"),
                NodeKind::GeneratorCallExpression | NodeKind::CallExpression
                    if self.core(clause.file, node, view, "forall") =>
                {
                    let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                        Some(*node)
                    } else {
                        node.child_nodes()
                            .next()
                            .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                    };
                    quantified
                        .and_then(|q| {
                            q.child_nodes()
                                .find(|n| n.kind() != NodeKind::GeneratorList)
                        })
                        .is_some_and(|body| {
                            contains(body, owning)
                                || matches!(&ty.kind, TypeKind::Array { indices, element }
                                    if indices.len() == 2 && element.kind == TypeKind::Bool)
                                    && contains(owning, node)
                                    && contains(body, clause.node)
                        })
                }
                NodeKind::ArrayComprehension => {
                    position > 0 && self.core(clause.file, ancestors[position - 1], view, "forall")
                }
                _ => false,
            })
    }
    pub(in crate::callable_definitions) fn actual(
        &self,
        clause: &Clause<'a>,
        id: DeclarationId,
        formal: DeclarationId,
        view: &CallableFacts,
    ) -> Option<(FileId, &'a SyntaxNode)> {
        let d = &self.bindings.declarations[formal.0];
        if d.role == DeclarationRole::Parameter {
            let signature = view.signatures.iter().find(|s| s.declaration == id)?;
            let position = (0..signature.parameters.len())
                .find(|p| formal_parameter(self.context, self.bindings, id, *p) == Some(formal))?;
            call_argument(
                self.context,
                self.bindings,
                view,
                clause.file,
                clause.node,
                id,
                position,
            )
        } else {
            // Captured dependencies keep their defining scope; they are not
            // looked up again in the caller's namespace.
            find_node(
                self.context.files[d.file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )
            .map(|n| (d.file, n))
        }
    }
    pub(in crate::callable_definitions) fn target(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<(DeclarationId, DefinitionCoverage)> {
        let node = unwrap(node);
        if node.kind() == NodeKind::CallExpression && self.core(file, node, view, "array1d") {
            let argument = node.child_nodes().next()?;
            return self.target(file, argument, view, generators);
        }
        let (subject, indices) = if node.kind() == NodeKind::ArrayAccessExpression {
            let n: Vec<_> = node.child_nodes().collect();
            (*n.first()?, n[1..].to_vec())
        } else {
            (node, Vec::new())
        };
        if subject.kind() != NodeKind::Expression {
            return None;
        }
        let id = self.reference(file, subject)?;
        let ty = &self.view(file, subject, view).declarations[id.0].ty;
        if ty.instantiation != Instantiation::Decision || !ty.known() || optional(ty) {
            return None;
        }
        if let TypeKind::Array {
            indices: declared, ..
        } = &ty.kind
        {
            let coverage = if indices.is_empty()
                || declared.len() == 1
                    && indices.len() == 1
                    && self.exact_index(file, id, indices[0], generators, view)
            {
                DefinitionCoverage::WholeArray
            } else {
                complete_array_coverage(
                    self.context,
                    self.bindings,
                    (self.calls, self.view(file, node, view)),
                    self.instantiations,
                    self.domains,
                    (file, id),
                    (&indices, generators),
                )
            };
            Some((id, coverage))
        } else if indices.is_empty() {
            Some((id, DefinitionCoverage::Scalar))
        } else {
            None
        }
    }
    pub(in crate::callable_definitions) fn converted_array_target(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        if let Some(argument) = self.array_conversion_argument(file, node, view) {
            return self.converted_array_target(file, argument, view);
        }
        if node.kind() != NodeKind::Expression {
            return None;
        }
        let id = self.reference(file, node)?;
        let ty = &self.view(file, node, view).declarations[id.0].ty;
        (ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Decision
            && matches!(&ty.kind, TypeKind::Array { indices, .. } if indices.len() == 1))
        .then_some(id)
    }
}
