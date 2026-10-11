use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn user_integer_fresh_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let call = self
            .source
            .operation_fact(self.source.view(file, node, view), file, node)?;
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &call.outcome
        else {
            return None;
        };
        let decision = TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision);
        let owner = &self.source.bindings.declarations[declaration.0];
        if owner.role != DeclarationRole::Function
            || !owner.top_level
            || self.source.context.files[owner.file].kind != SourceKind::User
            || parameters.is_empty()
            || parameters.iter().any(|ty| ty != &decision)
            || return_type != &decision
        {
            return None;
        }
        let function = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let body = function
            .child_nodes()
            .filter(|part| is_expression(part.kind()))
            .last()?;
        if unwrap(body).kind() != NodeKind::LetExpression {
            return None;
        }
        Some(
            (|| -> Result<DefinitionSafety, String> {
                if !self.source.source_annotations_safe(file, written)
                    || !self.source.callable_annotations_safe(owner.file, function)
                    || function
                        .child_nodes()
                        .filter(|part| is_expression(part.kind()))
                        .count()
                        != 1
                    || !self.source.calls.signatures.iter().any(|signature| {
                        signature.declaration == *declaration
                            && signature.return_type == decision
                            && signature.parameters.len() == parameters.len()
                            && signature
                                .parameters
                                .iter()
                                .all(|formal| formal.ty == decision && !formal.has_default)
                    })
                {
                    return Err("user integer source signature or metadata is unsupported".into());
                }
                // Only plain Int headers enter this body family; no named domain,
                // default or alias is inferred from the concrete selected tuple.
                for header in function
                    .child_nodes()
                    .filter(|part| !is_expression(part.kind()))
                {
                    let mut types = vec![header];
                    while let Some(part) = types.pop() {
                        if !matches!(
                            part.kind(),
                            NodeKind::ParameterList
                                | NodeKind::Parameter
                                | NodeKind::ScalarType
                                | NodeKind::Annotation
                        ) || !self.source.prefix_primitive_source_safe(owner.file, part)
                        {
                            return Err("user integer written header is unsupported".into());
                        }
                        if part.kind() != NodeKind::Annotation {
                            types.extend(part.child_nodes());
                        }
                    }
                }
                self.user_integer_call_scope(file, written, view, generators)?;
                let instance = Instance {
                    id: *declaration,
                    parameters: parameters.clone(),
                    ancestry: Vec::new(),
                    view: instantiated_body(
                        self.source.context,
                        self.source.bindings,
                        self.source.calls,
                        *declaration,
                        parameters,
                    ),
                    clauses: Vec::new(),
                    recursive: false,
                };
                let clause = Clause {
                    file,
                    item: call.item,
                    node,
                    generators: generators.to_vec(),
                    kind: ClauseKind::Call,
                };
                let mut invocation = Invocation {
                    actuals: Vec::new(),
                    reachable: None,
                };
                self.invocation_actuals(&clause, view, &instance, &mut invocation)?;
                self.fresh_integer_let_safety(
                    owner.file,
                    body,
                    &instance.view,
                    &[],
                    Some(&invocation),
                )
                .ok_or_else(|| "user integer source requires an inspected fresh let body".into())
            })()
            .unwrap_or_else(DefinitionSafety::Unsupported),
        )
    }
    // Caller locals are inspected in declaration order, before inherited headers
    // and actuals use them. Their values and set nonemptiness remain unproved.
    pub(in crate::callable_definitions) fn user_integer_call_scope(
        &self,
        file: FileId,
        call: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let written_operation = |source: &SyntaxNode| -> Result<(), String> {
            if matches!(
                source.kind(),
                NodeKind::CallExpression
                    | NodeKind::GeneratorCallExpression
                    | NodeKind::UnaryExpression
                    | NodeKind::BinaryExpression
                    | NodeKind::RangeExpression
            ) {
                let facts = self.source.view(file, source, view);
                let call = self
                    .source
                    .operation_fact(facts, file, source)
                    .ok_or("user integer caller source selection is unavailable")?;
                let CallOutcome::Resolved {
                    parameters,
                    return_type,
                    ..
                } = &call.outcome
                else {
                    return Err("user integer caller source selection is unsupported".into());
                };
                if !self.source.prefix_primitive(
                    file,
                    source,
                    facts,
                    &call.name,
                    parameters,
                    return_type,
                ) {
                    return Err("user integer caller written primitive is unsupported".into());
                }
            }
            Ok(())
        };
        let mut checked = Vec::new();
        let mut ancestors = vec![self.source.context.files[file].parsed.tree()];
        while let Some(node) = ancestors.pop() {
            if node.range().start > call.range().start || call.range().end > node.range().end {
                continue;
            }
            if node.kind() == NodeKind::LetExpression {
                for entry in node
                    .child_nodes()
                    .filter(|part| part.kind() == NodeKind::LetBlock)
                    .flat_map(|block| block.child_nodes())
                    .take_while(|entry| entry.range().end <= call.range().start)
                {
                    let local = self
                        .source
                        .bindings
                        .declarations
                        .iter()
                        .find(|local| {
                            local.file == file
                                && local.role == DeclarationRole::Local
                                && local.syntax_range == entry.range()
                        })
                        .ok_or("user integer caller local identity is unavailable")?;
                    let ty = &self.source.view(file, entry, view).declarations[local.id.0].ty;
                    let parts: Vec<_> = entry
                        .child_nodes()
                        .filter(|part| part.kind() != NodeKind::Annotation)
                        .collect();
                    let [written_type, initializer] = parts.as_slice() else {
                        return Err(
                            "user integer caller requires initialized parameter locals".into()
                        );
                    };
                    if entry.kind() != NodeKind::Declaration
                        || !ty.known()
                        || optional(ty)
                        || ty.instantiation != Instantiation::Parameter
                        || !(ty.kind == TypeKind::Int
                            || matches!(&ty.kind, TypeKind::Set(element)
                            if **element == TypeInst::par(TypeKind::Int)))
                        || !matches!(
                            written_type.kind(),
                            NodeKind::ScalarType | NodeKind::SetType
                        )
                        || !is_expression(initializer.kind())
                    {
                        return Err(
                            "user integer caller local type or source is unsupported".into()
                        );
                    }
                    let preceding: Vec<_> = generators
                        .iter()
                        .copied()
                        .filter(|header| header.range().end <= entry.range().start)
                        .collect();
                    self.type_dependencies(file, entry, view, &preceding)?;
                    let mut sources = vec![entry];
                    while let Some(source) = sources.pop() {
                        if source.kind() == NodeKind::Error
                            || !self.source.source_annotations_safe(file, source)
                            || source.kind() == NodeKind::Expression
                                && self.source.reference(file, source).is_some_and(|id| {
                                    self.source.bindings.declarations[id.0].role
                                        == DeclarationRole::Local
                                        && !checked.contains(&id)
                                })
                        {
                            return Err(
                                "user integer caller local scope or annotation is unsupported"
                                    .into(),
                            );
                        }
                        written_operation(source)?;
                        sources.extend(source.child_nodes());
                    }
                    if let Some(reason) = self.source.closed_integer_source_error_with_branches(
                        file,
                        initializer,
                        false,
                        true,
                        true,
                    ) {
                        return Err(reason);
                    }
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        initializer,
                        view,
                        &preceding,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    checked.push(local.id);
                }
            }
            ancestors.extend(node.child_nodes());
        }
        // Reuse the same present-Int header contract and source-before-binder,
        // filter-after-binder ordering as the existing array source reader.
        self.integer_source_headers_safety(file, view, generators)?;
        let mut sources = vec![call];
        sources.extend(generators.iter().copied());
        while let Some(source) = sources.pop() {
            if source.kind() == NodeKind::Expression
                && self.source.reference(file, source).is_some_and(|id| {
                    let owner = &self.source.bindings.declarations[id.0];
                    owner.role == DeclarationRole::Local
                        && (!checked.contains(&id) || owner.syntax_range.end > source.range().start)
                })
            {
                return Err("user integer actual local scope is unsupported".into());
            }
            if source.kind() == NodeKind::Error
                || !self.source.source_annotations_safe(file, source)
            {
                return Err("user integer actual source annotation is unsupported".into());
            }
            if source.range() != call.range() {
                written_operation(source)?;
            }
            sources.extend(source.child_nodes());
        }
        Ok(())
    }
    // Inspect this fresh-let source family with retained physical formal-to-actual
    // correspondence. No invocation output or reflected numeric bound is created.
    pub(in crate::callable_definitions) fn fresh_integer_invoked_source(
        &self,
        file: FileId,
        source: &'a SyntaxNode,
        view: &CallableFacts,
        invocation: &Invocation<'a, '_>,
    ) -> Result<(), String> {
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let boolean = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Bool;
        let int = |ty: &TypeInst| ty == &integer || ty == &decision;
        let mut nodes = vec![source];
        while let Some(written) = nodes.pop() {
            let node = unwrap(written);
            if node.kind() == NodeKind::Expression {
                let ty = self
                    .source
                    .expression_type(self.source.view(file, node, view), file, node)
                    .ok_or("invoked integer source type is unavailable")?;
                if !int(&ty.ty) && !boolean(&ty.ty) {
                    return Err("invoked integer source type is unsupported".into());
                }
                if let Some(id) = self.source.reference(file, node) {
                    let owner = &self.source.bindings.declarations[id.0];
                    let own_formal = invocation.actuals.iter().any(|actual| actual.formal == id);
                    let own_local = owner.file == file
                        && owner.role == DeclarationRole::Local
                        && owner.syntax_range.end <= node.range().start
                        && invocation.actuals.iter().any(|actual| {
                            let formal = &self.source.bindings.declarations[actual.formal.0];
                            formal.file == file && formal.item == owner.item
                        });
                    if !own_formal && !own_local {
                        return Err(
                            "invoked integer source reference is outside its owning body".into(),
                        );
                    }
                } else if !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if matches!(token.kind, TokenKind::IntegerLiteral | TokenKind::Infinity | TokenKind::True | TokenKind::False))
                {
                    return Err("invoked integer source literal is unsupported".into());
                }
                continue;
            }
            if !matches!(
                node.kind(),
                NodeKind::CallExpression
                    | NodeKind::BinaryExpression
                    | NodeKind::UnaryExpression
                    | NodeKind::RangeExpression
            ) {
                return Err("invoked integer source expression is unsupported".into());
            }
            let call = self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .ok_or("invoked integer source selection is unavailable")?;
            let CallOutcome::Resolved {
                parameters,
                return_type,
                ..
            } = &call.outcome
            else {
                return Err("invoked integer source selection is unsupported".into());
            };
            let children: Vec<_> = node.child_nodes().collect();
            let allowed = match call.name.as_str() {
                "lb" | "ub" => {
                    if parameters.as_slice() != [decision.clone()]
                        || return_type != &integer
                        || children.len() != 1
                    {
                        return Err("invoked scalar reflection tuple is unsupported".into());
                    }
                    let actual = self
                        .invocation_source(file, children[0], invocation)?
                        .ok_or("invoked scalar reflection requires its own formal")?;
                    if actual.collection {
                        return Err("invoked scalar reflection collection is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) = self
                        .decision_scalar_bound_operand_safety(
                            actual.file,
                            actual.node,
                            actual.view,
                            &actual.generators,
                        )
                    {
                        return Err(reason);
                    }
                    continue;
                }
                "+" | "-" => {
                    matches!(parameters.len(), 1 | 2)
                        && parameters.iter().all(int)
                        && int(return_type)
                }
                "min" | "max" => {
                    parameters.as_slice() == [integer.clone(), integer.clone()]
                        && return_type == &integer
                }
                "=" => parameters.len() == 2 && parameters.iter().all(int) && boolean(return_type),
                "/\\" | "\\/" => {
                    parameters.len() == 2 && parameters.iter().all(boolean) && boolean(return_type)
                }
                ".." => {
                    parameters.as_slice() == [integer.clone(), integer.clone()]
                        && *return_type == TypeInst::par(TypeKind::Set(Box::new(integer.clone())))
                }
                _ => false,
            };
            if !allowed {
                return Err("invoked integer source operation is unsupported".into());
            }
            nodes.extend(children);
        }
        Ok(())
    }
    // A fresh integer choice can have inspected sources without a defining
    // dependency vector. Keep the strict let interpreter and its outputs separate.
    pub(in crate::callable_definitions) fn fresh_integer_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        invocation: Option<&Invocation<'a, '_>>,
    ) -> Option<DefinitionSafety> {
        self.inspect_fresh_integer_let(file, written, view, generators, invocation)
            .map(|checked| match checked {
                Ok(_) => DefinitionSafety::Unknown(
                    "fresh local integer value and domain nonemptiness are unproved".into(),
                ),
                Err(reason) => DefinitionSafety::Unsupported(reason),
            })
    }
    pub(in crate::callable_definitions) fn inspect_fresh_integer_let(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        invocation: Option<&Invocation<'a, '_>>,
    ) -> Option<Result<DeclarationId, String>> {
        let node = unwrap(written);
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let typed = |source: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, source, view), file, source)
                .map(|expression| &expression.ty)
        };
        if node.kind() != NodeKind::LetExpression || typed(node) != Some(&decision) {
            return None;
        }
        let parts: Vec<_> = node.child_nodes().collect();
        let [block, body] = parts.as_slice() else {
            return None;
        };
        if block.kind() != NodeKind::LetBlock {
            return None;
        }
        let mut parameters = Vec::new();
        let mut constraints = Vec::new();
        let mut fresh = None;
        for entry in block.child_nodes() {
            if entry.kind() == NodeKind::Constraint && fresh.is_some() {
                let expressions: Vec<_> = entry.child_nodes().collect();
                let [constraint] = expressions.as_slice() else {
                    return None;
                };
                if typed(constraint)
                    .is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
                {
                    return None;
                }
                constraints.push(*constraint);
                continue;
            }
            if entry.kind() != NodeKind::Declaration || fresh.is_some() {
                return None;
            }
            let owner = self
                .source
                .bindings
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Local
                        && declaration.syntax_range == entry.range()
                })?;
            let ty = &self.source.view(file, entry, view).declarations[owner.id.0].ty;
            let initializers: Vec<_> = entry
                .child_nodes()
                .filter(|part| is_expression(part.kind()))
                .collect();
            if ty == &integer
                && let [initializer] = initializers.as_slice()
            {
                if typed(initializer) != Some(&integer) {
                    return None;
                }
                parameters.push((entry, *initializer));
            } else if ty == &decision && initializers.is_empty() {
                fresh = Some((entry, owner));
            } else {
                return None;
            }
        }
        let (local, owner) = fresh?;
        let body = unwrap(body);
        if local.kind() != NodeKind::Declaration
            || self.source.view(file, node, view).declarations[owner.id.0].ty != decision
            || local.child_nodes().any(|part| is_expression(part.kind()))
            || body.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, body).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || self.source.reference(file, body) != Some(owner.id)
            || typed(body) != Some(&decision)
        {
            return None;
        }
        let types: Vec<_> = local
            .child_nodes()
            .filter(|part| part.kind() != NodeKind::Annotation)
            .collect();
        let [domain] = types.as_slice() else {
            return None;
        };
        let range = match domain.kind() {
            NodeKind::ScalarType
                if domain.child_nodes().next().is_none()
                    && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, domain).as_slice(),
                        [var, int] if var.kind == TokenKind::Var && int.kind == TokenKind::Int) =>
            {
                None
            }
            NodeKind::DomainType => {
                let values: Vec<_> = domain.child_nodes().collect();
                let [range] = values.as_slice() else {
                    return None;
                };
                let range = unwrap(range);
                let bounds: Vec<_> = range.child_nodes().collect();
                let [lower, upper] = bounds.as_slice() else {
                    return None;
                };
                if range.kind() != NodeKind::RangeExpression {
                    return None;
                }
                Some((range, *lower, *upper))
            }
            _ => return None,
        };
        let checked = (|| -> Result<DeclarationId, String> {
            // Include written wrappers and every attached annotation, rather
            // than inspecting only the returned local reference.
            let mut nodes = vec![written];
            nodes.extend(generators.iter().copied());
            while let Some(source) = nodes.pop() {
                if source.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, source)
                {
                    return Err("fresh integer let annotation or source is unsupported".into());
                }
                nodes.extend(source.child_nodes());
            }
            for (position, header) in generators.iter().enumerate() {
                let binders: Vec<_> = self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .filter(|declaration| {
                        declaration.file == file
                            && declaration.role == DeclarationRole::Generator
                            && declaration.syntax_range == header.range()
                    })
                    .collect();
                let parts: Vec<_> = header.child_nodes().collect();
                let Some(source) = parts.first() else {
                    return Err("fresh integer let generator source is unavailable".into());
                };
                if header.kind() != NodeKind::Generator
                    || binders.is_empty()
                    || binders.len() != crate::domains::generator_slots(&self.source.context.files[file].parsed, header)
                    || !header.children().iter().any(|part| matches!(part,
                        SyntaxElement::Token(index) if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In))
                    || binders.iter().any(|binder| self.source.view(file, header, view).declarations[binder.id.0].ty != integer)
                    || typed(source) != Some(&set)
                {
                    return Err("fresh integer let generator identity or type is unsupported".into());
                }
                for (index, value) in parts.iter().enumerate() {
                    let (value, scope) = if index == 0 {
                        (*value, &generators[..position])
                    } else {
                        let conditions: Vec<_> = value.child_nodes().collect();
                        let [condition] = conditions.as_slice() else {
                            return Err("fresh integer let filter is unsupported".into());
                        };
                        if value.kind() != NodeKind::WhereFilter
                            || typed(condition) != Some(&TypeInst::par(TypeKind::Bool))
                        {
                            return Err("fresh integer let filter type is unsupported".into());
                        }
                        (*condition, &generators[..=position])
                    };
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, value, true, true)
                    {
                        return Err(reason);
                    }
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, value, view, scope, &mut Vec::new())
                    {
                        return Err(reason);
                    }
                }
            }
            if let Some((range, lower, upper)) = range
                && (typed(range) != Some(&set)
                    || typed(lower) != Some(&integer)
                    || typed(upper) != Some(&integer)
                    || !self.source.prefix_primitive(
                        file,
                        range,
                        self.source.view(file, range, view),
                        "..",
                        &[integer.clone(), integer.clone()],
                        &set,
                    ))
            {
                return Err("fresh integer let requires a checked parameter integer range".into());
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error_with_branches(file, domain, false, true, true)
            {
                return Err(reason);
            }
            let mut sources = range
                .map(|(_, lower, upper)| vec![lower, upper])
                .unwrap_or_default();
            for (parameter, initializer) in parameters {
                let mut types: Vec<_> = parameter
                    .child_nodes()
                    .filter(|part| {
                        !is_expression(part.kind()) && part.kind() != NodeKind::Annotation
                    })
                    .collect();
                if types.len() != 1 {
                    return Err("fresh integer let parameter type is unsupported".into());
                }
                while let Some(ty) = types.pop() {
                    if ty.kind() == NodeKind::DomainType {
                        for value in ty.child_nodes() {
                            if typed(value) != Some(&set) {
                                return Err(
                                    "fresh integer let parameter domain type is unsupported".into(),
                                );
                            }
                            if let Some(reason) =
                                self.source.closed_integer_source_error_with_branches(
                                    file, value, false, true, true,
                                )
                            {
                                return Err(reason);
                            }
                            sources.push(value);
                        }
                    } else {
                        types.extend(ty.child_nodes());
                    }
                }
                sources.push(initializer);
            }
            sources.extend(constraints);
            // Check every selected written operation and initialized source.
            // Constraint inspection does not define the fresh result local.
            let mut nodes = sources.clone();
            while let Some(source) = nodes.pop() {
                if matches!(
                    source.kind(),
                    NodeKind::CallExpression
                        | NodeKind::GeneratorCallExpression
                        | NodeKind::UnaryExpression
                        | NodeKind::BinaryExpression
                        | NodeKind::RangeExpression
                ) {
                    let facts = self.source.view(file, source, view);
                    let call = self
                        .source
                        .operation_fact(facts, file, source)
                        .ok_or("fresh integer let source selection is unavailable")?;
                    let CallOutcome::Resolved {
                        parameters,
                        return_type,
                        ..
                    } = &call.outcome
                    else {
                        return Err("fresh integer let source selection is unsupported".into());
                    };
                    if !self.source.prefix_primitive(
                        file,
                        source,
                        facts,
                        &call.name,
                        parameters,
                        return_type,
                    ) {
                        return Err(
                            "fresh integer let source written primitive is unsupported".into()
                        );
                    }
                }
                nodes.extend(source.child_nodes());
            }
            for source in sources {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error_with_branches(file, source, false, true, true)
                {
                    return Err(reason);
                }
                if let Some(invocation) = invocation {
                    self.fresh_integer_invoked_source(file, source, view, invocation)?;
                } else if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, source, view, generators, &mut Vec::new())
                {
                    return Err(reason);
                }
            }
            Ok(owner.id)
        })();
        Some(checked)
    }
    // Search can account for these model locals only after the whole owning
    // Value source succeeds. This supplies no definition or coverage fact.
    pub(in crate::callable_definitions) fn model_value_fresh_locals(
        &self,
        id: DeclarationId,
    ) -> Option<Result<Vec<DeclarationId>, String>> {
        let owner = &self.source.bindings.declarations[id.0];
        let file = owner.file;
        let ty = &self.source.calls.declarations[id.0].ty;
        if !owner.top_level
            || owner.role != DeclarationRole::Value
            || self.source.context.files[file].kind != SourceKind::User
            || !ty.known()
            || optional(ty)
        {
            return None;
        }
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let initializers: Vec<_> = written
            .child_nodes()
            .filter(|part| is_expression(part.kind()))
            .collect();
        let [initializer] = initializers.as_slice() else {
            return None;
        };
        let mut lets = Vec::new();
        let mut nodes = vec![*initializer];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::LetExpression
                && let Some(block) = node
                    .child_nodes()
                    .find(|part| part.kind() == NodeKind::LetBlock)
                && let Some(local) = block.child_nodes().find(|part| {
                    part.kind() == NodeKind::Declaration
                        && self.source.bindings.declarations.iter().any(|declaration| {
                            declaration.file == file
                                && declaration.item == owner.item
                                && declaration.role == DeclarationRole::Local
                                && declaration.syntax_range == part.range()
                                && self.source.calls.declarations[declaration.id.0]
                                    .ty
                                    .instantiation
                                    == Instantiation::Decision
                        })
                })
            {
                lets.push((node, local));
            }
            nodes.extend(node.child_nodes());
        }
        if lets.is_empty() {
            return None;
        }
        let checked = (|| -> Result<Vec<DeclarationId>, String> {
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if node.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, node)
                {
                    return Err("model value fresh let source or annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            let mut sources = vec![*initializer];
            let mut types: Vec<_> = written.child_nodes().take(1).collect();
            while let Some(ty) = types.pop() {
                if ty.kind() == NodeKind::DomainType {
                    sources.extend(ty.child_nodes());
                } else {
                    types.extend(ty.child_nodes());
                }
            }
            for source in sources {
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    file,
                    source,
                    self.source.calls,
                    &[],
                    &mut vec![id],
                ) {
                    return Err(reason);
                }
            }
            let mut inspected = Vec::new();
            for (node, local) in lets {
                let scope = local_source_scope(initializer, local)
                    .ok_or("model value fresh let lexical scope is unavailable")?;
                let local_id = self
                    .inspect_fresh_integer_let(file, node, self.source.calls, &scope, None)
                    .ok_or("model value fresh let source family is unsupported")??;
                let declaration = &self.source.bindings.declarations[local_id.0];
                if declaration.file != file
                    || declaration.item != owner.item
                    || declaration.role != DeclarationRole::Local
                    || declaration.syntax_range != local.range()
                {
                    return Err("model value fresh let owner identity is unsupported".into());
                }
                if !inspected.contains(&local_id) {
                    inspected.push(local_id);
                }
            }
            Ok(inspected)
        })();
        Some(checked)
    }
    pub(in crate::callable_definitions) fn parameter_set_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::LetExpression {
            return None;
        }
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let integer_set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if element.known() && !optional(element) && element.kind == TypeKind::Int
                        && element.instantiation == Instantiation::Parameter)
        };
        let bodies: Vec<_> = node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::LetBlock)
            .collect();
        let [body] = bodies.as_slice() else {
            return None;
        };
        if unwrap(body).kind() != NodeKind::ConditionalExpression
            || typed(node).is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
        {
            return None;
        }
        let locals: Vec<_> = node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|block| block.child_nodes())
            .collect();
        // Only initialized parameter integer-set comprehensions are admitted.
        // Other let shapes keep the existing strict Local interpreter.
        if locals.is_empty()
            || locals.iter().any(|local| {
                local.kind() != NodeKind::Declaration
                    || self
                        .source
                        .bindings
                        .declarations
                        .iter()
                        .find(|d| {
                            d.file == file
                                && d.syntax_range == local.range()
                                && d.role == DeclarationRole::Local
                        })
                        .is_none_or(|d| !integer_set(&view.declarations[d.id.0].ty))
                    || local
                        .child_nodes()
                        .find(|n| is_expression(n.kind()))
                        .is_none_or(|value| unwrap(value).kind() != NodeKind::SetComprehension)
            })
        {
            return None;
        }
        let inspect = (|| -> Result<(), String> {
            let mut nodes = vec![written];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, current) {
                    return Err("set let annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut lexical = Vec::new();
            for header in generators {
                if !crate::definitions::annotations_safe(self.source.context, file, header) {
                    return Err("set let generator annotation is unsupported".into());
                }
                let binders: Vec<_> = self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if binders.is_empty()
                    || binders.len()
                        != crate::domains::generator_slots(&self.source.context.files[file].parsed, header)
                    || !header.children().iter().any(|c| {
                        matches!(c, SyntaxElement::Token(i)
                        if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                    })
                    || binders.iter().any(|d| {
                        let ty = &view.declarations[d.id.0].ty;
                        !ty.known()
                            || optional(ty)
                            || ty.instantiation != Instantiation::Parameter
                            || ty.kind != TypeKind::Int
                    })
                {
                    return Err("set let generator identity or type is unsupported".into());
                }
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("set let generator source is unavailable")?;
                if typed(source).is_none_or(|ty| !integer_set(ty)) {
                    return Err("set let generator source type is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, source, view, &lexical, &mut Vec::new())
                {
                    return Err(reason);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, source, true, true)
                {
                    return Err(reason);
                }
                lexical.push(*header);
                for filter in header
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let parts: Vec<_> = filter.child_nodes().collect();
                    let [condition] = parts.as_slice() else {
                        return Err("set let filter is unsupported".into());
                    };
                    if typed(condition).is_none_or(|ty| {
                        !ty.known()
                            || optional(ty)
                            || ty.instantiation != Instantiation::Parameter
                            || ty.kind != TypeKind::Bool
                    }) {
                        return Err("set let filter type is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        condition,
                        view,
                        &lexical,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, condition, true, true)
                    {
                        return Err(reason);
                    }
                }
            }
            // Inspect every initializer, even unused locals. The initialized
            // source walker does not follow Local references, so reject forward
            // or cyclic local aliases before inspecting ordered initializers.
            for local in locals {
                if !crate::definitions::annotations_safe(self.source.context, file, local) {
                    return Err("set local annotation is unsupported".into());
                }
                self.type_dependencies(file, local, view, &lexical)?;
                let value = local
                    .child_nodes()
                    .find(|n| is_expression(n.kind()))
                    .ok_or("set local initializer is unavailable")?;
                let range = self.source.context.files[file]
                    .location(value.range())
                    .range;
                if self.source.bindings.references.iter().any(|reference| reference.file == file
                    && range.start <= reference.location.range.start && reference.location.range.end <= range.end
                    && matches!(reference.resolution, BindingResolution::Resolved(id)
                        if self.source.bindings.declarations[id.0].role == DeclarationRole::Local
                            && self.source.bindings.declarations[id.0].syntax_range.end + self.source.context.files[file].byte_offset
                                > reference.location.range.start))
                { return Err("set local has a cyclic or forward source".into()); }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new())
                {
                    return Err(reason);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, local, true, true)
                {
                    return Err(reason);
                }
            }
            match self.boolean_relation_safety(file, body, view, &lexical) {
                Some(DefinitionSafety::Unsupported(reason)) => Err(reason),
                Some(_) => Ok(()),
                None => Err("set let Boolean relation inspection is unavailable".into()),
            }
        })();
        Some(match inspect {
            Err(reason) => DefinitionSafety::Unsupported(reason),
            Ok(()) => DefinitionSafety::Unknown("set let relation value is unproved".into()),
        })
    }
    pub(in crate::callable_definitions) fn parameter_row_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        inspected_locals: &[DeclarationId],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let integer = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Int
                && t.instantiation == Instantiation::Parameter
        };
        let nested_rows = generators.len() == 3 && inspected_locals.len() == 3;
        if node.kind() != NodeKind::LetExpression
            || ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
            || (!matches!(generators.len(), 1 | 2) && !nested_rows)
            || node
                .child_nodes()
                .filter(|n| n.kind() != NodeKind::LetBlock)
                .count()
                != 1
        {
            return None;
        }
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::LetBlock)?;
        let first = unwrap(body);
        let second = first.child_nodes().nth(1).map(unwrap);
        let nested_body = generators.len() == 1
            && inspected_locals.is_empty()
            && first.kind() == NodeKind::GeneratorCallExpression
            && second.is_some_and(|n| {
                n.kind() == NodeKind::GeneratorCallExpression
                    && n.child_nodes()
                        .nth(1)
                        .is_some_and(|inner| unwrap(inner).kind() == NodeKind::LetExpression)
            });
        let dual_rows = generators.len() == 2;
        let scoped_rows = dual_rows || nested_rows || nested_body;
        if nested_rows
            && inspected_locals.iter().any(|id| {
                let declaration = &self.source.bindings.declarations[id.0];
                declaration.file != file
                    || declaration.role != DeclarationRole::Local
                    || declaration.syntax_range.end > node.range().start
                    || !integer(&view.declarations[id.0].ty)
            })
        {
            return None;
        }
        let integer_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if integer(element))
        };
        let integer_table = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && integer(element))
        };
        let mut sources = Vec::new();
        let mut tables = Vec::new();
        for (position, generator) in generators.iter().enumerate() {
            let source = generator.child_nodes().next()?;
            if (!scoped_rows && generator.child_nodes().count() != 1)
                || (nested_body && generator.child_nodes().count() != 1)
                || (nested_rows
                    && generator.child_nodes().count() != if position == 0 { 1 } else { 2 })
                || (scoped_rows
                    && generator
                        .child_nodes()
                        .skip(1)
                        .any(|n| n.kind() != NodeKind::WhereFilter))
                || !generator.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
                || !self.source.core(file, source, view, "index_set_1of2")
            {
                return None;
            }
            if scoped_rows {
                let binders: Vec<_> = self
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
                let values: Vec<_> = source.child_nodes().collect();
                let [value] = values.as_slice() else {
                    return None;
                };
                let subject = unwrap(value);
                let array = self.source.reference(file, subject)?;
                let owner = &self.source.bindings.declarations[array.0];
                if binders.len() != 1
                    || !integer(&view.declarations[binders[0].id.0].ty)
                    || subject.kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || !(owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    || (dual_rows && tables.first().is_some_and(|previous| *previous != array))
                    || (nested_rows && position == 1 && tables[0] == array)
                    || (nested_rows && position == 2 && tables[1] != array)
                    || ty(value).is_none_or(|t| !integer_table(t))
                    || ty(source).is_none_or(|t| !integer_set(t))
                    || self.source.operation_fact(self.source.view(file, source, view), file, source).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && integer_table(&parameters[0])
                                && ty(value).is_some_and(|actual| crate::types::coerces(actual, &parameters[0]))
                                && integer_set(return_type) && Some(return_type) == ty(source)))
                {
                    return None;
                }
                tables.push(array);
            }
            sources.push(source);
        }
        let mut rows = Vec::new();
        let mut locals = Vec::new();
        let mut initializers = Vec::new();
        for block in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
        {
            for local in block.child_nodes() {
                let declaration = self.source.bindings.declarations.iter().find(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Local
                        && d.syntax_range == local.range()
                })?;
                if local.kind() != NodeKind::Declaration
                    || !integer(&view.declarations[declaration.id.0].ty)
                {
                    return None;
                }
                let mut values = local.child_nodes().filter(|n| is_expression(n.kind()));
                let initializer = values.next()?;
                if values.next().is_some() {
                    return None;
                }
                let access = unwrap(initializer);
                let parts: Vec<_> = access.child_nodes().collect();
                if access.kind() != NodeKind::ArrayAccessExpression
                    || parts.len() != 3
                    || ty(access).is_none_or(|t| !integer(t))
                {
                    return None;
                }
                let subject = unwrap(parts[0]);
                let array = self.source.reference(file, subject)?;
                let owner = &self.source.bindings.declarations[array.0];
                let row = self.source.reference(file, unwrap(parts[1]))?;
                let position = generators.iter().position(|generator| {
                    self.source.bindings.declarations[row.0].syntax_range == generator.range()
                })?;
                let generator = generators[position];
                let source = sources[position];
                if subject.kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || !(owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    || ty(subject).is_none_or(|t| !t.known() || optional(t)
                        || t.instantiation != Instantiation::Parameter
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 2 && indices.iter().all(integer) && integer(element)))
                    || unwrap(parts[1]).kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(parts[1])).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || ty(parts[1]).is_none_or(|t| !integer(t))
                    || ty(parts[2]).is_none_or(|t| !integer(t))
                    || self.source.bindings.declarations[row.0].role != DeclarationRole::Generator
                    || self.source.bindings.declarations[row.0].syntax_range != generator.range()
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(parts[2])).as_slice(),
                        [token] if token.kind == TokenKind::IntegerLiteral)
                    || source.child_nodes().count() != 1
                    || source.child_nodes().next().is_none_or(|n| {
                        let n = unwrap(n);
                        n.kind() != NodeKind::Expression || self.source.reference(file, n) != Some(array)
                            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, n).as_slice(),
                                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    })
                {
                    return None;
                }
                if !crate::definitions::annotations_safe(self.source.context, file, local) {
                    return Some(DefinitionSafety::Unsupported(
                        "row local annotation is unsupported".into(),
                    ));
                }
                if let Err(reason) = self.type_dependencies(file, local, view, generators) {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                if self.source.domains.declarations[declaration.id.0]
                    .domain
                    .numeric_minimum()
                    .is_err()
                {
                    return Some(DefinitionSafety::Unsupported(
                        "row local domain is unsupported".into(),
                    ));
                }
                if !rows.contains(&array) {
                    rows.push(array);
                }
                locals.push(declaration.id);
                initializers.push(initializer);
            }
        }
        if initializers.is_empty()
            || nested_body && locals.len() != 3
            || nested_rows && locals.len() != 1
        {
            return None;
        }
        if !crate::definitions::annotations_safe(self.source.context, file, written)
            || generators.iter().any(|generator| {
                !crate::definitions::annotations_safe(self.source.context, file, generator)
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "row let or iteration annotation is unsupported".into(),
            ));
        }
        // Unknown cell values do not excuse a closed source-axis or element error.
        // Retain the original array identity; neither defaults nor row values are proofs.
        for array in rows {
            let mut domain = &self.source.domains.declarations[array.0].domain;
            while let Domain::Named {
                domain: original, ..
            } = domain
            {
                domain = original;
            }
            let Domain::Array { indices, element } = domain else {
                return Some(DefinitionSafety::Unsupported(
                    "row table domain is unavailable".into(),
                ));
            };
            if indices.len() != 2
                || indices.iter().any(|axis| axis.numeric_minimum().is_err())
                || element.numeric_minimum().is_err()
            {
                return Some(DefinitionSafety::Unsupported(
                    "row table index or element domain is unsupported".into(),
                ));
            }
        }
        // Inspect the exact selected header, every initialized local (including
        // unused ones), and the whole Boolean body in its original lexical scope.
        if scoped_rows {
            // Sources see prior headers; each filter also sees its own binder.
            // Check eager errors only after initialized source inspection.
            for (position, generator) in generators.iter().enumerate() {
                let source = sources[position];
                if let unsupported @ DefinitionSafety::Unsupported(_) = self
                    .initialized_source_safety(
                        file,
                        source,
                        view,
                        &generators[..position],
                        &mut Vec::new(),
                    )
                {
                    return Some(unsupported);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, source, true, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                for filter in generator
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let values: Vec<_> = filter.child_nodes().collect();
                    let [condition] = values.as_slice() else {
                        return Some(DefinitionSafety::Unsupported(
                            "row filter is unsupported".into(),
                        ));
                    };
                    if !crate::definitions::annotations_safe(self.source.context, file, filter)
                        || ty(condition).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || t.kind != TypeKind::Bool
                                || t.instantiation != Instantiation::Parameter
                        })
                    {
                        return Some(DefinitionSafety::Unsupported(
                            "row filter type or annotation is unsupported".into(),
                        ));
                    }
                    if nested_rows {
                        let range = self.source.context.files[file]
                            .location(condition.range())
                            .range;
                        if self.source.bindings.references.iter().any(|reference| {
                            reference.file == file && reference.kind == ReferenceKind::Value
                                && range.start <= reference.location.range.start
                                && reference.location.range.end <= range.end
                                && matches!(reference.resolution, BindingResolution::Resolved(id)
                                    if self.source.bindings.declarations[id.0].role == DeclarationRole::Local
                                        && !inspected_locals.contains(&id))
                        }) {
                            return Some(DefinitionSafety::Unsupported(
                                "nested row filter has an uninspected local source".into(),
                            ));
                        }
                        let condition = unwrap(condition);
                        let atoms: Vec<_> = if self.source.core(file, condition, view, "/\\") {
                            let parts: Vec<_> = condition.child_nodes().collect();
                            if parts.len() != 2 || parts.iter().any(|part| ty(part).is_none_or(|t|
                                !t.known() || optional(t) || t.kind != TypeKind::Bool
                                    || t.instantiation != Instantiation::Parameter)) || self.source.operation_fact(self.source.view(file, condition, view), file, condition)
                                .is_none_or(|call| !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                    if parameters.len() == 2 && parameters.iter().chain(std::iter::once(return_type))
                                        .all(|t| t.known() && !optional(t) && t.kind == TypeKind::Bool
                                            && t.instantiation == Instantiation::Parameter)
                                        && Some(return_type) == ty(condition))) {
                                return Some(DefinitionSafety::Unsupported(
                                    "nested row conjunction signature is unsupported".into(),
                                ));
                            }
                            parts
                        } else {
                            vec![condition]
                        };
                        // Both complete comparison atoms are inspected, but the
                        // closed arithmetic walk never treats a lazy AND as eager.
                        for atom in atoms {
                            let atom = unwrap(atom);
                            if atom.child_nodes().count() != 2
                                || atom.child_nodes().any(|child| ty(child).is_none_or(|t| !integer(t)))
                                || !self.source.core(file, atom, view, "=")
                                || self.source.operation_fact(self.source.view(file, atom, view), file, atom)
                                    .is_none_or(|call| !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                        if parameters.len() == 2 && parameters.iter().all(integer)
                                            && return_type.known() && !optional(return_type)
                                            && return_type.kind == TypeKind::Bool
                                            && return_type.instantiation == Instantiation::Parameter
                                            && Some(return_type) == ty(atom))) {
                                return Some(DefinitionSafety::Unsupported(
                                    "nested row comparison signature is unsupported".into(),
                                ));
                            }
                            if let unsupported @ DefinitionSafety::Unsupported(_) = self
                                .initialized_source_safety(
                                    file,
                                    atom,
                                    view,
                                    &generators[..=position],
                                    &mut Vec::new(),
                                )
                            {
                                return Some(unsupported);
                            }
                            if let Some(reason) = self
                                .source
                                .closed_integer_source_error(file, atom, true, true)
                            {
                                return Some(DefinitionSafety::Unsupported(reason));
                            }
                        }
                    } else {
                        if let unsupported @ DefinitionSafety::Unsupported(_) = self
                            .initialized_source_safety(
                                file,
                                condition,
                                view,
                                &generators[..=position],
                                &mut Vec::new(),
                            )
                        {
                            return Some(unsupported);
                        }
                        if let Some(reason) = self
                            .source
                            .closed_integer_source_error(file, condition, true, true)
                        {
                            return Some(DefinitionSafety::Unsupported(reason));
                        }
                    }
                }
            }
        } else {
            initializers.push(sources[0]);
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &initializers, view, generators)
        {
            return Some(unsupported);
        }
        if nested_body || nested_rows {
            for initializer in &initializers {
                if let Some(reason) =
                    self.source
                        .closed_integer_source_error(file, initializer, true, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
            }
        }
        if nested_body {
            let quantified = |value: &'a SyntaxNode| -> Option<(&'a SyntaxNode, &'a SyntaxNode)> {
                let value = unwrap(value);
                let parts: Vec<_> = value.child_nodes().collect();
                let [headers, body] = parts.as_slice() else {
                    return None;
                };
                if value.kind() != NodeKind::GeneratorCallExpression
                    || headers.kind() != NodeKind::GeneratorList || headers.child_nodes().count() != 1
                    || !self.source.core(file, value, view, "forall")
                    || !crate::definitions::annotations_safe(self.source.context, file, value)
                    || self.source.operation_fact(self.source.view(file, value, view), file, value).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && return_type.known() && !optional(return_type)
                                && return_type.kind == TypeKind::Bool && return_type.instantiation == Instantiation::Decision
                                && Some(return_type) == ty(value) && parameters[0].known() && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && integer(&indices[0]) && element.known() && !optional(element)
                                        && element.kind == TypeKind::Bool && element.instantiation == Instantiation::Decision
                                        && ty(body).is_some_and(|actual| actual.known() && !optional(actual)
                                            && crate::types::coerces(actual, element))))) {
                    return None;
                }
                Some((headers.child_nodes().next()?, *body))
            };
            let Some((first_header, second)) = quantified(body) else {
                return Some(DefinitionSafety::Unsupported(
                    "nested row forall signature is unsupported".into(),
                ));
            };
            let Some((second_header, inner)) = quantified(second) else {
                return Some(DefinitionSafety::Unsupported(
                    "nested row forall signature is unsupported".into(),
                ));
            };
            let mut lexical = generators.to_vec();
            lexical.extend([first_header, second_header]);
            // The inner let may use only these already fully inspected prior
            // Local identities. This is inspection state, never output facts.
            return Some(
                self.parameter_row_let_safety(file, inner, view, &lexical, &locals)
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported("nested row let scope is unsupported".into())
                    }),
            );
        }
        if dual_rows || nested_rows {
            let range = self.source.context.files[file].location(body.range()).range;
            for reference in self.source.bindings.references.iter().filter(|r| {
                r.file == file
                    && r.kind == ReferenceKind::Value
                    && range.start <= r.location.range.start
                    && r.location.range.end <= range.end
            }) {
                if let BindingResolution::Resolved(id) = reference.resolution
                    && self.source.bindings.declarations[id.0].role == DeclarationRole::Local
                    && (!locals.contains(&id) && !inspected_locals.contains(&id)
                        || self.source.bindings.declarations[id.0].syntax_range.end
                            + self.source.context.files[file].byte_offset
                            > reference.location.range.start)
                {
                    return Some(DefinitionSafety::Unsupported(
                        "row body has an uninspected or forward local source".into(),
                    ));
                }
            }
        }
        let inspected = self
            .boolean_relation_safety(file, body, view, generators)
            .unwrap_or_else(|| {
                self.initialized_source_safety(file, body, view, generators, &mut Vec::new())
            });
        Some(match inspected {
            unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
            _ => {
                DefinitionSafety::Unknown("row let selection or relation value is unproved".into())
            }
        })
    }
    // Inspect a checked local body, core forall conditional body or core
    // exists body. Numeric operands retain raw Bool-to-Int coercion partiality.
    pub(in crate::callable_definitions) fn boolean_relation_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        if ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool) {
            return None;
        }
        if node.kind() == NodeKind::ConditionalExpression {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Some(DefinitionSafety::Unsupported(
                    "Boolean conditional annotation is unsupported".into(),
                ));
            }
            return Some((|| -> DefinitionSafety {
                let branches: Vec<_> = node.child_nodes().collect();
                let mut complete = false;
                for (position, branch) in branches.iter().enumerate() {
                    if !crate::definitions::annotations_safe(self.source.context, file, branch) {
                        return DefinitionSafety::Unsupported(
                            "Boolean conditional branch annotation is unsupported".into(),
                        );
                    }
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let branch_body =
                        if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                            let guard = parts[0];
                            if ty(guard).is_none_or(|t| {
                                !t.known()
                                    || optional(t)
                                    || t.instantiation != Instantiation::Parameter
                                    || t.kind != TypeKind::Bool
                            }) {
                                return DefinitionSafety::Unsupported(
                                    "Boolean conditional guard is unsupported".into(),
                                );
                            }
                            if let unsupported @ DefinitionSafety::Unsupported(_) = self
                                .initialized_source_safety(
                                    file,
                                    guard,
                                    view,
                                    generators,
                                    &mut Vec::new(),
                                )
                            {
                                return unsupported;
                            }
                            // Inspect eager guard sources independently of their
                            // value; never check the whole lazy conditional here.
                            if let Some(reason) = self
                                .source
                                .closed_integer_source_error(file, guard, true, true)
                            {
                                return DefinitionSafety::Unsupported(reason);
                            }
                            parts[1]
                        } else if branch.kind() == NodeKind::ElseBranch
                            && parts.len() == 1
                            && position + 1 == branches.len()
                        {
                            complete = true;
                            parts[0]
                        } else {
                            return DefinitionSafety::Unsupported(
                                "Boolean conditional branch is unsupported".into(),
                            );
                        };
                    if ty(branch_body).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.kind != TypeKind::Bool
                            || !crate::types::coerces(t, ty(node).unwrap())
                    }) {
                        return DefinitionSafety::Unsupported(
                            "Boolean conditional body is unsupported".into(),
                        );
                    }
                    let inspected = self
                        .boolean_relation_safety(file, branch_body, view, generators)
                        .unwrap_or_else(|| {
                            self.initialized_source_safety(
                                file,
                                branch_body,
                                view,
                                generators,
                                &mut Vec::new(),
                            )
                        });
                    if let unsupported @ DefinitionSafety::Unsupported(_) = inspected {
                        return unsupported;
                    }
                }
                if complete && branches.len() >= 2 {
                    DefinitionSafety::Unknown(
                        "Boolean conditional relation value is unproved".into(),
                    )
                } else {
                    DefinitionSafety::Unsupported(
                        "Boolean conditional inspection requires a complete else".into(),
                    )
                }
            })());
        }
        let children: Vec<_> = node.child_nodes().collect();
        let name =
            operator(self.source.context, file, node).and_then(crate::bindings::symbolic_operator);
        let logical = match (node.kind(), name) {
            (NodeKind::BinaryExpression, Some("/\\" | "\\/" | "<->" | "->" | "<-" | "xor")) => {
                children.len() == 2
            }
            (NodeKind::UnaryExpression, Some("not")) => children.len() == 1,
            _ => false,
        };
        if logical {
            if !self.source.core(file, node, view, name.unwrap())
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || children.iter().any(|n| ty(n).is_none_or(|t|
                    !t.known() || optional(t) || t.kind != TypeKind::Bool))
                || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == children.len() && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)|
                                formal.known() && !optional(formal) && formal.kind == TypeKind::Bool
                                    && ty(actual).is_some_and(|t| crate::types::coerces(t, formal)))))
            {
                return Some(DefinitionSafety::Unsupported("Boolean relation identity or signature is unsupported".into()));
            }
            for child in children {
                let inspected = self
                    .boolean_relation_safety(file, child, view, generators)
                    .unwrap_or_else(|| {
                        self.initialized_source_safety(
                            file,
                            child,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    });
                if let unsupported @ DefinitionSafety::Unsupported(_) = inspected {
                    return Some(unsupported);
                }
            }
            return Some(DefinitionSafety::Unknown(
                "Boolean relation value is unproved".into(),
            ));
        }
        if node.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let subject = children.first().copied().map(unwrap)?;
        let array = self.source.reference(file, subject)?;
        if subject.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || ty(subject).is_none_or(|t| !t.known() || optional(t)
                || !matches!(&t.kind, TypeKind::Array { indices, element }
                    if matches!(indices.len(), 1 | 2) && children.len() == indices.len() + 1
                        && element.known() && !optional(element) && element.kind == TypeKind::Bool
                        && indices.iter().zip(&children[1..]).all(|(axis, selector)|
                            axis.known() && !optional(axis) && axis.instantiation == Instantiation::Parameter
                                && matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_))
                                && ty(selector).is_some_and(|index| index.known() && !optional(index)
                                    && index.instantiation == Instantiation::Parameter && index.kind == axis.kind))))
        {
            return Some(DefinitionSafety::Unsupported("Boolean relation selection type or source is unsupported".into()));
        }
        let mut domain = &self.source.domains.declarations[array.0].domain;
        while let Domain::Named {
            domain: original, ..
        } = domain
        {
            domain = original;
        }
        let Domain::Array { indices, element } = domain else {
            return Some(DefinitionSafety::Unsupported(
                "Boolean relation source domain is unavailable".into(),
            ));
        };
        if indices.len() + 1 != children.len()
            || indices.iter().any(|axis| axis.numeric_minimum().is_err())
            || element.numeric_minimum().is_err()
        {
            return Some(DefinitionSafety::Unsupported(
                "Boolean relation source domain is unsupported".into(),
            ));
        }
        for selector in &children[1..] {
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, selector, true, false)
            {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
        // Undefined Boolean selection is false only in the checked Boolean
        // relation. Inspect its source and selectors; grant no raw membership.
        Some(
            match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => {
                    DefinitionSafety::Unknown("Boolean relation selection value is unproved".into())
                }
            },
        )
    }
    pub(in crate::callable_definitions) fn float_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::LetExpression
            || self
                .source
                .expression_type(self.source.view(file, node, view), file, node)
                .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
        {
            return None;
        }
        let mut nodes = vec![node];
        let mut parameter_float = false;
        while let Some(current) = nodes.pop() {
            if self
                .source
                .expression_type(self.source.view(file, current, view), file, current)
                .is_some_and(|e| {
                    e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.kind == TypeKind::Float
                        && e.ty.instantiation == Instantiation::Parameter
                })
            {
                parameter_float = true;
            }
            nodes.extend(current.child_nodes());
        }
        if !parameter_float {
            return None;
        }
        if !crate::definitions::annotations_safe(self.source.context, file, written) {
            return Some(DefinitionSafety::Unsupported(
                "Float let annotation is unsupported".into(),
            ));
        }
        // The Local interpreter's strict reference dependencies do not walk a
        // named global initializer. Inspect even unused local sources first;
        // nested lets perform this same check in their own lexical scope.
        for block in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
        {
            for local in block
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::Declaration)
            {
                let lexical: Vec<_> = generators
                    .iter()
                    .copied()
                    .filter(|g| g.range().end <= local.range().start)
                    .collect();
                for value in local.child_nodes().filter(|n| is_expression(n.kind())) {
                    if let unsupported @ DefinitionSafety::Unsupported(_) =
                        self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new())
                    {
                        return Some(unsupported);
                    }
                }
            }
        }
        let Some(item) = self.source.context.files[file]
            .parsed
            .tree()
            .child_nodes()
            .position(|item| {
                item.range().start <= node.range().start && node.range().end <= item.range().end
            })
        else {
            return Some(DefinitionSafety::Unsupported(
                "Float let owning item unavailable".into(),
            ));
        };
        let clause = Clause {
            file,
            item,
            node,
            generators: generators.to_vec(),
            kind: ClauseKind::Local,
        };
        let mut unavailable = Vec::new();
        // Use the existing ordered Local interpreter, and discard every temporary
        // output and inspected-local ID even when its body is supported.
        self.interpret(
            &clause,
            view,
            &[],
            &mut Vec::new(),
            &mut unavailable,
            &mut Vec::new(),
        );
        Some(match unavailable.into_iter().next() {
            Some(unavailable) => DefinitionSafety::Unsupported(unavailable.reason),
            None => {
                DefinitionSafety::Unknown("Float let initialization or value is unproved".into())
            }
        })
    }
    // Inspect this optional constructor without establishing any cell value,
    // presence, extent or local output. Its present sources remain strict.
    pub(in crate::callable_definitions) fn optional_parameter_matrix_safety(
        &self,
        file: FileId,
        local: &'a SyntaxNode,
        id: DeclarationId,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let ty = &view.declarations[id.0].ty;
        let parameter = |ty: &TypeInst| {
            ty.known() && !optional(ty) && ty.instantiation == Instantiation::Parameter
        };
        let TypeKind::Array { indices, element } = &ty.kind else {
            return None;
        };
        if !ty.known()
            || ty.optional
            || ty.instantiation != Instantiation::Parameter
            || indices.len() != 2
            || indices.iter().any(|axis| {
                !parameter(axis) || !matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_))
            })
            || element.instantiation != Instantiation::Parameter
            || !element.optional
            || element.kind != TypeKind::Int
        {
            return None;
        }
        let initializer = unwrap(
            local
                .child_nodes()
                .find(|node| is_expression(node.kind()))?,
        );
        if initializer.kind() != NodeKind::CallExpression
            || self
                .source
                .operation_fact(self.source.view(file, initializer, view), file, initializer)
                .is_none_or(|call| call.name != "array2d")
        {
            return None;
        }
        let checked = (|| {
            let typed = |node: &SyntaxNode| {
                self.source
                    .expression_type(self.source.view(file, node, view), file, node)
                    .map(|value| &value.ty)
            };
            let arguments: Vec<_> = initializer.child_nodes().map(unwrap).collect();
            let [rows, columns, input] = arguments.as_slice() else {
                return Err("optional parameter matrix requires three positional arguments".into());
            };
            if initializer
                .child_nodes()
                .any(|argument| argument.kind() == NodeKind::NamedArgument)
                || typed(initializer) != Some(ty)
            {
                return Err(
                    "optional parameter matrix written type or nominal axes mismatch".into(),
                );
            }
            let integer = TypeInst::par(TypeKind::Int);
            let parameters = vec![
                TypeInst::par(TypeKind::Set(Box::new(indices[0].clone()))),
                TypeInst::par(TypeKind::Set(Box::new(indices[1].clone()))),
                TypeInst::par(TypeKind::Array {
                    indices: vec![integer.clone()],
                    element: element.clone(),
                }),
            ];
            self.source.parameter_array_primitive(
                file,
                initializer,
                view,
                "array2d",
                &parameters,
                ty,
            )?;
            if typed(rows) != Some(&parameters[0]) || typed(columns) != Some(&parameters[1]) {
                return Err("optional parameter matrix axis argument type is unsupported".into());
            }
            let written = local
                .child_nodes()
                .next()
                .filter(|node| node.kind() == NodeKind::ArrayType)
                .ok_or("optional parameter matrix written array type is unavailable")?;
            let parts: Vec<_> = written.child_nodes().collect();
            if parts.len() != 3 || parts.iter().any(|part| part.kind() != NodeKind::DomainType) {
                return Err(
                    "optional parameter matrix requires written named axes and member domain"
                        .into(),
                );
            }
            let mut active = vec![id];
            for (position, (axis, argument)) in parts[..2].iter().zip([*rows, *columns]).enumerate()
            {
                let sources: Vec<_> = axis.child_nodes().map(unwrap).collect();
                let [source] = sources.as_slice() else {
                    return Err(
                        "optional parameter matrix written axis source is unsupported".into(),
                    );
                };
                let identity = self
                    .source
                    .bindings
                    .references
                    .iter()
                    .find(|reference| {
                        reference.file == file
                            && reference.location.range
                                == self.source.context.files[file]
                                    .location(source.range())
                                    .range
                    })
                    .and_then(|reference| match reference.resolution {
                        BindingResolution::Resolved(id) => Some(id),
                        _ => None,
                    });
                if source.kind() != NodeKind::Expression
                    || source.child_nodes().next().is_some()
                    || argument.kind() != NodeKind::Expression
                    || argument.child_nodes().next().is_some()
                    || identity.is_none()
                    || self.source.reference(file, argument) != identity
                {
                    return Err("optional parameter matrix axis must retain its written declaration identity".into());
                }
                if let TypeKind::Enum(enum_id) = indices[position].kind
                    && (identity != Some(enum_id)
                        || !self.source.bindings.declarations[enum_id.0].top_level
                        || self.source.bindings.declarations[enum_id.0].role
                            != DeclarationRole::Enum)
                {
                    return Err(
                        "optional parameter matrix enum axis requires its owning bare enum source"
                            .into(),
                    );
                }
                self.optional_matrix_source_safety(
                    (file, argument, local.range().start),
                    view,
                    generators,
                    &mut active,
                )?;
            }
            let members: Vec<_> = parts[2].child_nodes().collect();
            let [member] = members.as_slice() else {
                return Err("optional parameter matrix member domain source is unsupported".into());
            };
            self.optional_matrix_source_safety(
                (file, member, local.range().start),
                view,
                generators,
                &mut active,
            )?;
            let input_type =
                typed(input).ok_or("optional parameter matrix input type is unavailable")?;
            let TypeKind::Array {
                indices: raw_indices,
                element: raw_element,
            } = &input_type.kind
            else {
                return Err("optional parameter matrix input is not an array".into());
            };
            if !input_type.known()
                || input_type.optional
                || input_type.instantiation != Instantiation::Parameter
                || raw_element != element
                || raw_indices.iter().any(|axis| axis != &integer)
            {
                return Err(
                    "optional parameter matrix raw input or normalized formal is unsupported"
                        .into(),
                );
            }
            let cells: Vec<_> = match input.kind() {
                NodeKind::ArrayLiteral if raw_indices.len() == 1 => {
                    let cells: Vec<_> = input.child_nodes().collect();
                    if cells
                        .iter()
                        .any(|cell| cell.kind() == NodeKind::IndexedArrayEntry)
                    {
                        return Err(
                            "optional parameter matrix indexed literal is unsupported".into()
                        );
                    }
                    cells
                }
                NodeKind::MatrixLiteral if raw_indices.len() == 2 => {
                    let rows: Vec<_> = input.child_nodes().collect();
                    let width = rows.first().map_or(0, |row| row.child_nodes().count());
                    if rows.iter().any(|row| {
                        row.kind() != NodeKind::MatrixRow
                            || row.child_nodes().count() != width
                            || crate::domains::tokens(&self.source.context.files[file].parsed, row)
                                .iter()
                                .any(|token| token.kind == TokenKind::Colon)
                    }) {
                        return Err(
                            "optional parameter matrix requires a plain rectangular literal".into(),
                        );
                    }
                    rows.into_iter().flat_map(|row| row.child_nodes()).collect()
                }
                _ => {
                    return Err("optional parameter matrix input source form is unsupported".into());
                }
            };
            if !self.source.source_annotations_safe(file, initializer)
                || !self.source.source_annotations_safe(file, input)
            {
                return Err("optional parameter matrix source annotation is unsupported".into());
            }
            let Domain::Array {
                indices: domains,
                element: member_domain,
            } = &self.source.domains.declarations[id.0].domain
            else {
                return Err("optional parameter matrix written domains are unavailable".into());
            };
            if domains.len() != 2 {
                return Err("optional parameter matrix written axes are unavailable".into());
            }
            member_domain.numeric_minimum().map_err(str::to_owned)?;
            let mut size = Some(1_i64);
            for domain in domains {
                domain.numeric_minimum().map_err(str::to_owned)?;
                let extent = match crate::domains::bare_index_domain(domain) {
                    Domain::Range { .. } => {
                        crate::domains::index_domain_interval(domain).and_then(|(lower, upper)| {
                            if lower > upper {
                                Some(0)
                            } else {
                                upper.checked_sub(lower)?.checked_add(1)
                            }
                        })
                    }
                    Domain::Enum(id) => self.source.enum_extent(*id),
                    _ => None,
                };
                size = size
                    .zip(extent)
                    .and_then(|(size, extent)| size.checked_mul(extent));
            }
            if size.is_some_and(|size| usize::try_from(size).ok() != Some(cells.len())) {
                return Err(
                    "optional parameter matrix closed axis extent disagrees with literal cells"
                        .into(),
                );
            }
            for written_cell in cells {
                let cell = unwrap(written_cell);
                if !self.source.source_annotations_safe(file, written_cell) {
                    return Err("optional parameter matrix cell annotation is unsupported".into());
                }
                let cell_type =
                    typed(cell).ok_or("optional parameter matrix cell type is unavailable")?;
                let absent = cell.kind() == NodeKind::Expression
                    && cell.child_nodes().next().is_none()
                    && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, cell).as_slice(),
                        [token] if token.kind == TokenKind::Absent)
                    && cell_type.instantiation == Instantiation::Parameter
                    && cell_type.optional
                    && cell_type.kind == TypeKind::Bottom;
                if absent {
                    continue;
                }
                if !parameter(cell_type)
                    || cell_type.kind != TypeKind::Int
                    || !crate::types::coerces(cell_type, element)
                {
                    return Err("optional parameter matrix cell must be typed absent or present Parameter Int".into());
                }
                self.optional_matrix_source_safety(
                    (file, written_cell, local.range().start),
                    view,
                    generators,
                    &mut active,
                )?;
                if let Some(value) = crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    cell,
                )? && (crate::domains::index_domain_member(member_domain, value) == Some(false)
                    || matches!(crate::domains::bare_index_domain(member_domain), Domain::Range { lower, .. }
                        if crate::domains::invariant_integer(lower).is_ok_and(|bound|
                            bound.is_some_and(|bound| value < bound))))
                {
                    return Err(
                        "optional parameter matrix cell is outside its written member domain"
                            .into(),
                    );
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "optional parameter matrix extent and values are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Add earlier lexical locals to the existing source readers. The same
    // graph also retains selected primitive checks in transitive sources.
    pub(in crate::callable_definitions) fn optional_matrix_source_safety(
        &self,
        source: (FileId, &'a SyntaxNode, usize),
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> Result<(), String> {
        let (file, written, before) = source;
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                continue;
            }
            if matches!(
                node.kind(),
                NodeKind::CallExpression
                    | NodeKind::UnaryExpression
                    | NodeKind::BinaryExpression
                    | NodeKind::RangeExpression
            ) && let Some(call) =
                self.source
                    .operation_fact(self.source.view(file, node, view), file, node)
                && let CallOutcome::Resolved {
                    parameters,
                    return_type,
                    ..
                } = &call.outcome
            {
                self.source.parameter_array_primitive(
                    file,
                    node,
                    view,
                    &call.name,
                    parameters,
                    return_type,
                )?;
            }
            nodes.extend(node.child_nodes());
            if node.kind() != NodeKind::Expression || node.child_nodes().next().is_some() {
                continue;
            }
            let range = self.source.context.files[file].location(node.range()).range;
            if self.source.bindings.references.iter().any(|reference| {
                reference.file == file
                    && range.start <= reference.location.range.start
                    && reference.location.range.end <= range.end
                    && matches!(&reference.resolution, BindingResolution::Resolved(id)
                        if self.source.bindings.declarations[id.0].role == DeclarationRole::TypeAlias)
            }) {
                return Err("optional matrix type alias source inspection is unsupported".into());
            }
            let Some(id) = self.source.reference(file, node) else {
                continue;
            };
            let owner = &self.source.bindings.declarations[id.0];
            if owner.role != DeclarationRole::Local
                && !(owner.top_level
                    && matches!(owner.role, DeclarationRole::Value | DeclarationRole::Enum))
            {
                continue;
            }
            if active.contains(&id) {
                return Err("cyclic optional matrix initialized source".into());
            }
            let local = owner.role == DeclarationRole::Local;
            if local && (owner.file != file || owner.syntax_range.end > before) {
                return Err("optional matrix source must be an earlier lexical local".into());
            }
            let declaration = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("optional matrix source declaration is unavailable")?;
            let facts = if local { view } else { self.source.calls };
            let lexical: Vec<_> = if local {
                generators
                    .iter()
                    .copied()
                    .filter(|header| header.range().end <= declaration.range().start)
                    .collect()
            } else {
                Vec::new()
            };
            active.push(id);
            let checked: Result<(), String> = (|| {
                if local {
                    let ty = &facts.declarations[id.0].ty;
                    let initializers: Vec<_> = declaration
                        .child_nodes()
                        .filter(|part| is_expression(part.kind()))
                        .collect();
                    if !ty.known()
                        || optional(ty)
                        || ty.instantiation != Instantiation::Parameter
                        || initializers.len() != 1
                        || !self.source.source_annotations_safe(owner.file, declaration)
                        || self
                            .source
                            .expression_type(facts, owner.file, initializers[0])
                            .is_none_or(|actual| {
                                !actual.ty.known()
                                    || optional(&actual.ty)
                                    || actual.ty.instantiation != Instantiation::Parameter
                                    || !crate::types::coerces(&actual.ty, ty)
                            })
                    {
                        return Err("optional matrix local initializer type, coercion or annotation is unsupported".into());
                    }
                }
                // Existing readers own expression/global/enum checks. The
                // prewalk supplies local order/cycles and selected written guards.
                let mut parts = vec![declaration];
                while let Some(part) = parts.pop() {
                    if part.kind() == NodeKind::Annotation {
                        continue;
                    }
                    if local && !self.source.source_annotations_safe(owner.file, part) {
                        return Err(
                            "optional matrix local written type annotation is unsupported".into(),
                        );
                    }
                    if is_expression(part.kind()) {
                        self.optional_matrix_source_safety(
                            (owner.file, part, declaration.range().start),
                            facts,
                            &lexical,
                            active,
                        )?;
                    } else {
                        parts.extend(part.child_nodes());
                    }
                }
                if local {
                    self.type_dependencies(owner.file, declaration, facts, &lexical)?;
                }
                Ok(())
            })();
            active.pop();
            checked?;
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, written, true, true)
        {
            return Err(reason);
        }
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_source_safety(file, written, view, generators, active)
        {
            return Err(reason);
        }
        Ok(())
    }
}
