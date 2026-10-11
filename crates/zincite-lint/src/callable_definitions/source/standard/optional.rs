use super::*;

impl<'a> SourceInspector<'a> {
    // This demanded relation is inspected without optional-value extraction or outputs.
    pub(in crate::callable_definitions) fn optional_float_weak_equality_safety(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
    ) -> Option<Result<(), UnavailableCallableDefinition>> {
        let node = unwrap(clause.node);
        if node.kind() != NodeKind::BinaryExpression
            || operator(self.context, clause.file, node) != Some(TokenKind::WeakEqual)
            || !self.core(clause.file, node, view, "~=")
        {
            return None;
        }
        let facts = self.view(clause.file, node, view);
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &self.operation_fact(facts, clause.file, node)?.outcome
        else {
            return None;
        };
        let owner = &self.bindings.declarations[declaration.0];
        let operands: Vec<_> = node.child_nodes().collect();
        if owner.role != DeclarationRole::Function
            || owner.name != "~="
            || self.context.files[owner.file].kind != SourceKind::StandardLibrary
            || !self.context.files[owner.file].implicit
            || parameters.len() != 2
            || operands.len() != 2
            || parameters.iter().any(|ty| {
                !ty.known()
                    || ty.kind != TypeKind::Float
                    || !ty.optional
                    || ty.instantiation != Instantiation::Decision
            })
            || !return_type.known()
            || return_type.kind != TypeKind::Bool
            || return_type.optional
            || return_type.instantiation != Instantiation::Decision
            || self
                .expression_type(facts, clause.file, node)
                .is_none_or(|value| value.ty != *return_type)
            || operands.iter().zip(parameters).any(|(operand, parameter)| {
                self.expression_type(facts, clause.file, operand)
                    .is_none_or(|value| {
                        !value.ty.known()
                            || value.ty.kind != TypeKind::Float
                            || !crate::types::coerces(&value.ty, parameter)
                    })
            })
        {
            return None;
        }
        let mut location = self.context.files[clause.file].location(node.range());
        let checked = if !clause.generators.is_empty() {
            Err("optional Float weak equality ambient traversal is unsupported".into())
        } else {
            self.optional_float_body_source_safety(
                (clause.file, clause.node, self.view(clause.file, node, view)),
                &[],
                &mut Vec::new(),
                &mut location,
            )
        };
        Some(checked.map_err(|reason| UnavailableCallableDefinition {
            location,
            targets: Vec::new(),
            reason,
        }))
    }
    // Visible IDs carry lexical source identity, never presence, values or definitions.
    // No ordinary expression fallback may lose this selected-body active stack.
    pub(in crate::callable_definitions) fn optional_float_body_source_safety(
        &self,
        source: (FileId, &'a SyntaxNode, &CallableFacts),
        visible: &[DeclarationId],
        active: &mut Vec<DeclarationId>,
        location: &mut SourceLocation,
    ) -> Result<(), String> {
        let (file, written, view) = source;
        *location = self.context.files[file].location(written.range());
        if written.kind() == NodeKind::ParenthesizedExpression {
            let parts: Vec<_> = written.child_nodes().collect();
            if parts.len() != 1 || !self.source_annotations_safe(file, written) {
                return Err("optional Float parenthesized source is unsupported".into());
            }
            return self.optional_float_body_source_safety(
                (file, parts[0], view),
                visible,
                active,
                location,
            );
        }
        let node = unwrap(written);
        let typed = |node: &SyntaxNode| {
            self.expression_type(view, file, node)
                .map(|value| &value.ty)
        };
        let scalar = |t: &TypeInst, kind, optional, instantiation| {
            t.known()
                && t.kind == kind
                && t.optional == optional
                && t.instantiation == instantiation
        };
        let boolean = |t: &TypeInst| scalar(t, TypeKind::Bool, false, Instantiation::Decision);
        let float = |t: &TypeInst| scalar(t, TypeKind::Float, false, Instantiation::Decision);
        let option = |t: &TypeInst| scalar(t, TypeKind::Float, true, Instantiation::Decision);
        let parameter_float =
            |t: &TypeInst| scalar(t, TypeKind::Float, false, Instantiation::Parameter);
        let tuple = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Decision
                && matches!(&t.kind, TypeKind::Tuple(fields)
                if fields.len() == 2 && boolean(&fields[0]) && float(&fields[1]))
        };
        let bare = |node: &SyntaxNode| {
            let node = unwrap(node);
            (node.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.context.files[file].parsed, node).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                .then(|| self.reference(file, node)).flatten()
        };
        if written.kind() == NodeKind::AnnotatedExpression {
            let parts: Vec<_> = written.child_nodes().collect();
            if parts.len() != 2
                || parts[1].kind() != NodeKind::Annotation
                || !self.atomic_standard_metadata_safe(file, parts[1], "is_reverse_map")
                || typed(parts[0]).is_none_or(|t| !boolean(t))
            {
                return Err("optional Float representation annotation is unsupported".into());
            }
            // This metadata permits inspection only; the equality is never exported.
            return self.optional_float_body_source_safety(
                (file, parts[0], view),
                visible,
                active,
                location,
            );
        }
        if !self.source_annotations_safe(file, written) {
            return Err("optional Float written source annotation is unsupported".into());
        }
        if node.kind() == NodeKind::Declaration {
            let declaration = self
                .bindings
                .declarations
                .iter()
                .find(|d| {
                    d.file == file
                        && d.syntax_range == node.range()
                        && matches!(d.role, DeclarationRole::Value | DeclarationRole::Local)
                })
                .ok_or("optional Float source declaration is unavailable")?;
            let ty = &view.declarations[declaration.id.0].ty;
            let local = declaration.role == DeclarationRole::Local;
            let initializers: Vec<_> = node
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            if !ty.known()
                || initializers.len() > 1
                || !(local && (boolean(ty) || float(ty) || tuple(ty))
                    || !local
                        && ty.kind == TypeKind::Float
                        && (!ty.optional || ty.instantiation == Instantiation::Parameter))
                || tuple(ty) && initializers.is_empty()
                || local && visible.contains(&declaration.id)
                || active.contains(&declaration.id)
            {
                return Err(
                    "optional Float declaration type, initializer or cycle is unsupported".into(),
                );
            }
            active.push(declaration.id);
            let checked = (|| -> Result<(), String> {
                if let Some(reason) = self.closed_integer_source_error(file, node, false, true) {
                    return Err(reason);
                }
                let mut types = vec![
                    node.child_nodes()
                        .next()
                        .ok_or("optional Float written type is unavailable")?,
                ];
                while let Some(ty_node) = types.pop() {
                    if !self.source_annotations_safe(file, ty_node) {
                        return Err("optional Float written type annotation is unsupported".into());
                    }
                    if ty_node.kind() == NodeKind::DomainType {
                        for domain in ty_node.child_nodes() {
                            let domain = unwrap(domain);
                            let bounds: Vec<_> = domain.child_nodes().map(unwrap).collect();
                            let reflected = local
                                && float(ty)
                                && bounds.len() == 2
                                && domain.kind() == NodeKind::RangeExpression
                                && self.core(file, bounds[0], view, "lb")
                                && self.core(file, bounds[1], view, "ub")
                                && bounds.iter().all(|bound| bound.child_nodes().count() == 1)
                                && bounds[0].child_nodes().next().and_then(bare).is_some_and(
                                    |id| {
                                        visible.contains(&id)
                                            && self.bindings.declarations[id.0].role
                                                == DeclarationRole::Parameter
                                            && self.bindings.declarations[id.0].file == file
                                            && option(&view.declarations[id.0].ty)
                                            && bounds[1].child_nodes().next().and_then(bare)
                                                == Some(id)
                                    },
                                );
                            let literal = !local && domain.kind() == NodeKind::RangeExpression
                                && bounds.len() == 2 && bounds.iter().all(|bound| {
                                    bound.kind() == NodeKind::Expression
                                        && matches!(crate::domains::tokens(&self.context.files[file].parsed, bound).as_slice(),
                                            [token] if token.kind == TokenKind::FloatLiteral)
                                });
                            if !reflected && !literal {
                                return Err(
                                    "optional Float computed written domain is unsupported".into(),
                                );
                            }
                            self.optional_float_body_source_safety(
                                (file, domain, view),
                                visible,
                                active,
                                location,
                            )?;
                        }
                    } else {
                        types.extend(ty_node.child_nodes());
                    }
                }
                for initializer in initializers {
                    if typed(initializer).is_none_or(|value| !crate::types::coerces(value, ty)) {
                        return Err("optional Float initializer coercion is unsupported".into());
                    }
                    self.optional_float_body_source_safety(
                        (file, initializer, view),
                        visible,
                        active,
                        location,
                    )?;
                }
                Ok(())
            })();
            active.pop();
            return checked;
        }
        let ty = typed(node).ok_or("optional Float source type is unavailable")?;
        if !ty.known()
            || !(ty.kind == TypeKind::Bool && !ty.optional
                || ty.kind == TypeKind::Float
                || tuple(ty)
                || ty.instantiation == Instantiation::Parameter
                    && !optional(ty)
                    && matches!(&ty.kind, TypeKind::Set(element) if parameter_float(element)))
        {
            return Err("optional Float source type is unsupported".into());
        }
        if node.kind() == NodeKind::Expression {
            if let Some(id) = bare(node) {
                if visible.contains(&id) && view.declarations[id.0].ty == *ty {
                    return Ok(());
                }
                let d = &self.bindings.declarations[id.0];
                if d.top_level && d.role == DeclarationRole::Value && ty.kind == TypeKind::Float {
                    let declaration = find_node(
                        self.context.files[d.file].parsed.tree(),
                        &d.syntax_range,
                        d.role,
                    )
                    .ok_or("optional Float written actual source is unavailable")?;
                    return self.optional_float_body_source_safety(
                        (d.file, declaration, view),
                        &[],
                        active,
                        location,
                    );
                }
                return Err("optional Float reference has no checked owning scope".into());
            }
            if node.child_nodes().next().is_none()
                && matches!(crate::domains::tokens(&self.context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::FloatLiteral && parameter_float(ty)
                        || matches!(token.kind, TokenKind::True | TokenKind::False)
                            && scalar(ty, TypeKind::Bool, false, Instantiation::Parameter))
            {
                return Ok(());
            }
        }
        if node.kind() == NodeKind::TupleLiteral && tuple(ty) {
            let fields: Vec<_> = node.child_nodes().collect();
            if fields.len() != 2
                || typed(fields[0]).is_none_or(|t| !boolean(t))
                || typed(fields[1]).is_none_or(|t| !float(t))
            {
                return Err("optional Float representation tuple is unsupported".into());
            }
            for field in fields {
                self.optional_float_body_source_safety(
                    (file, field, view),
                    visible,
                    active,
                    location,
                )?;
            }
            return Ok(());
        }
        if node.kind() == NodeKind::FieldAccessExpression {
            let fields: Vec<_> = node.child_nodes().collect();
            let field = crate::domains::tokens(&self.context.files[file].parsed, node)
                .iter()
                .find(|token| token.kind == TokenKind::IntegerLiteral)
                .and_then(|token| {
                    self.context.files[file].parsed.source()[token.range.clone()]
                        .parse::<usize>()
                        .ok()
                });
            if fields.len() != 1
                || bare(fields[0]).is_none_or(|id| !visible.contains(&id))
                || typed(fields[0]).is_none_or(|t| !tuple(t))
                || !(field == Some(1) && boolean(ty) || field == Some(2) && float(ty))
            {
                return Err("optional Float owning tuple projection is unsupported".into());
            }
            return self.optional_float_body_source_safety(
                (file, fields[0], view),
                visible,
                active,
                location,
            );
        }
        if node.kind() == NodeKind::LetExpression {
            let parts: Vec<_> = node.child_nodes().collect();
            if parts.len() != 2
                || parts[0].kind() != NodeKind::LetBlock
                || typed(parts[1]) != Some(ty)
            {
                return Err("optional Float let result or shape is unsupported".into());
            }
            let mut locals = visible.to_vec();
            for item in parts[0].child_nodes() {
                if item.kind() == NodeKind::Constraint {
                    let expressions: Vec<_> = item.child_nodes().collect();
                    if expressions.len() != 1
                        || !self.source_annotations_safe(file, item)
                        || typed(expressions[0]).is_none_or(|t| !boolean(t))
                    {
                        return Err("optional Float local constraint is unsupported".into());
                    }
                    self.optional_float_body_source_safety(
                        (file, expressions[0], view),
                        &locals,
                        active,
                        location,
                    )?;
                } else if item.kind() == NodeKind::Declaration {
                    self.optional_float_body_source_safety(
                        (file, item, view),
                        &locals,
                        active,
                        location,
                    )?;
                    let id = self
                        .bindings
                        .declarations
                        .iter()
                        .find(|d| {
                            d.file == file
                                && d.syntax_range == item.range()
                                && d.role == DeclarationRole::Local
                        })
                        .ok_or("optional Float local identity is unavailable")?
                        .id;
                    locals.push(id);
                } else {
                    return Err("optional Float let item is unsupported".into());
                }
            }
            return self.optional_float_body_source_safety(
                (file, parts[1], view),
                &locals,
                active,
                location,
            );
        }
        if !matches!(
            node.kind(),
            NodeKind::CallExpression
                | NodeKind::BinaryExpression
                | NodeKind::UnaryExpression
                | NodeKind::RangeExpression
        ) {
            return Err("optional Float selected body source form is unsupported".into());
        }
        let fact = self
            .operation_fact(view, file, node)
            .ok_or("optional Float selected operation is unavailable")?;
        let CallOutcome::Resolved {
            declaration: id,
            parameters,
            return_type,
        } = &fact.outcome
        else {
            return Err("optional Float selected operation is unsupported".into());
        };
        let owner = &self.bindings.declarations[id.0];
        let children: Vec<_> = node.child_nodes().collect();
        let expected = match owner.name.as_str() {
            "~=" => parameters.len() == 2 && parameters.iter().all(option) && boolean(return_type),
            "occurs" | "absent" | "occurs_float" => {
                parameters.len() == 1 && option(&parameters[0]) && boolean(return_type)
            }
            "deopt" | "deopt_float" => {
                parameters.len() == 1 && option(&parameters[0]) && float(return_type)
            }
            "opt_internal_float" => {
                parameters.len() == 1 && option(&parameters[0]) && tuple(return_type)
            }
            "reverse_map_var_opt" => {
                parameters.len() == 2
                    && boolean(&parameters[0])
                    && float(&parameters[1])
                    && option(return_type)
            }
            "lb" | "ub" => {
                parameters.len() == 1 && option(&parameters[0]) && parameter_float(return_type)
            }
            "not" => parameters.len() == 1 && boolean(&parameters[0]) && boolean(return_type),
            "\\/" => {
                parameters.len() == 2 && parameters.iter().all(boolean) && boolean(return_type)
            }
            "=" => {
                parameters.len() == 2
                    && parameters[0] == parameters[1]
                    && (float(&parameters[0]) || option(&parameters[0]))
                    && boolean(return_type)
            }
            ".." => {
                parameters.len() == 2
                    && parameters.iter().all(parameter_float)
                    && return_type.instantiation == Instantiation::Parameter
                    && !optional(return_type)
                    && matches!(&return_type.kind, TypeKind::Set(element) if parameter_float(element))
            }
            _ => false,
        };
        if !expected
            || fact.generator_argument.is_some()
            || return_type != ty
            || owner.role != DeclarationRole::Function
            || self.context.files[owner.file].kind != SourceKind::StandardLibrary
            || !self.context.files[owner.file].implicit
            || children.len() != parameters.len()
        {
            return Err("optional Float operation identity or tuple is unsupported".into());
        }
        for (position, parameter) in parameters.iter().enumerate() {
            let (actual_file, actual) =
                call_argument(self.context, self.bindings, view, file, node, *id, position)
                    .ok_or("optional Float actual or default is unavailable")?;
            if actual_file != file
                || typed(actual).is_none_or(|t| !crate::types::coerces(t, parameter))
            {
                return Err("optional Float actual coercion is unsupported".into());
            }
            self.optional_float_body_source_safety(
                (file, actual, view),
                visible,
                active,
                location,
            )?;
        }
        let declaration = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("optional Float selected declaration is unavailable")?;
        let bodies: Vec<_> = declaration
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .collect();
        *location = self.context.files[owner.file].location(
            bodies
                .first()
                .map_or(declaration.range(), |body| body.range()),
        );
        let mut signatures = self
            .calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == *id);
        let signature = signatures
            .next()
            .ok_or("optional Float selected signature is unavailable")?;
        if signatures.next().is_some()
            || signature.parameters.len() != parameters.len()
            || signature
                .parameters
                .iter()
                .any(|parameter| parameter.has_default)
            || declaration
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
                .is_none_or(|list| list.child_nodes().count() != parameters.len())
        {
            return Err("optional Float selected signature or defaults are unsupported".into());
        }
        if bodies.is_empty() {
            if owner.name != "reverse_map_var_opt" {
                if !matches!(
                    owner.name.as_str(),
                    "lb" | "ub" | "=" | "not" | "\\/" | ".."
                ) {
                    return Err("optional Float required selected body is unavailable".into());
                }
                return self.selected_union_primitive_safety(*id);
            }
            let mut nodes = vec![declaration];
            while let Some(part) = nodes.pop() {
                if part.kind() == NodeKind::Annotation {
                    if !self.atomic_standard_metadata_safe(owner.file, part, "output_only") {
                        return Err("optional Float reverse-map metadata is unsupported".into());
                    }
                    continue;
                }
                if is_expression(part.kind()) {
                    return Err(
                        "optional Float reverse-map body/default/domain is unsupported".into(),
                    );
                }
                nodes.extend(part.child_nodes());
            }
            return Ok(());
        }
        if bodies.len() != 1
            || !matches!(
                owner.name.as_str(),
                "~=" | "occurs"
                    | "absent"
                    | "occurs_float"
                    | "deopt"
                    | "deopt_float"
                    | "opt_internal_float"
            )
            || !self.callable_annotations_safe(owner.file, declaration)
            || declaration
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::Annotation)
                .any(|annotation| {
                    !self.atomic_standard_metadata_safe(owner.file, annotation, "promise_total")
                        && !self.atomic_standard_metadata_safe(
                            owner.file,
                            annotation,
                            "promise_commutative",
                        )
                })
            || active.contains(id)
        {
            return Err(
                "optional Float selected body, annotation or recursion is unsupported".into(),
            );
        }
        let body_view = instantiated_body(self.context, self.bindings, self.calls, *id, parameters);
        let mut formals = Vec::new();
        let mut types = vec![
            declaration
                .child_nodes()
                .next()
                .ok_or("optional Float selected return type is unavailable")?,
        ];
        for (position, parameter) in parameters.iter().enumerate() {
            let formal = formal_parameter(self.context, self.bindings, *id, position)
                .ok_or("optional Float selected formal is unavailable")?;
            let written_formal = find_node(
                self.context.files[owner.file].parsed.tree(),
                &self.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("optional Float written formal is unavailable")?;
            if body_view.declarations[formal.0].ty != *parameter
                || written_formal
                    .child_nodes()
                    .any(|n| is_expression(n.kind()))
                || !self.source_annotations_safe(owner.file, written_formal)
            {
                return Err(
                    "optional Float concrete formal or written default is unsupported".into(),
                );
            }
            types.extend(written_formal.child_nodes().take(1));
            formals.push(formal);
        }
        while let Some(part) = types.pop() {
            if !self.source_annotations_safe(owner.file, part)
                || part.kind() == NodeKind::DomainType && part.child_nodes().next().is_some()
            {
                return Err("optional Float selected written type is unsupported".into());
            }
            types.extend(part.child_nodes());
        }
        if self
            .expression_type(&body_view, owner.file, bodies[0])
            .is_none_or(|value| value.ty != *return_type)
        {
            return Err("optional Float selected body result type is unsupported".into());
        }
        active.push(*id);
        let checked = self.optional_float_body_source_safety(
            (owner.file, bodies[0], &body_view),
            &formals,
            active,
            location,
        );
        active.pop();
        checked
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    // Inspection only: the complete standard recurrence preserves presence
    // guards, but establishes no output, extent or traversal membership.
    pub(in crate::callable_definitions) fn optional_increasing_body_safety(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
    ) -> Option<DefinitionSafety> {
        if !matches!(clause.kind, ClauseKind::Local) || !clause.generators.is_empty() {
            return None;
        }
        let instance = self.instances.iter().find(|instance| {
            let owner = &self.source.bindings.declarations[instance.id.0];
            std::ptr::eq(&instance.view, view)
                && owner.file == clause.file
                && owner.name == "fzn_increasing_int_opt"
                && owner.role == DeclarationRole::Predicate
                && self.source.context.files[owner.file].kind == SourceKind::StandardLibrary
        })?;
        let checked = (|| -> Result<(), String> {
            let file = clause.file;
            let integer = TypeInst::par(TypeKind::Int);
            let decision_integer = integer.clone().with_inst(Instantiation::Decision);
            let mut optional_integer = decision_integer.clone();
            optional_integer.optional = true;
            let decision_boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
            let array = |element: TypeInst| TypeInst {
                instantiation: element.instantiation,
                optional: false,
                kind: TypeKind::Array {
                    indices: vec![integer.clone()],
                    element: Box::new(element),
                },
            };
            let optional_array = array(optional_integer.clone());
            let present_array = array(decision_integer.clone());
            let integer_set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
            let boolean = TypeInst::par(TypeKind::Bool);
            let mut optional_boolean = decision_boolean.clone();
            optional_boolean.optional = true;
            let boolean_array = array(decision_boolean.clone());
            let optional_boolean_array = array(optional_boolean);
            let owner = &self.source.bindings.declarations[instance.id.0];
            let declaration = find_node(
                self.source.context.files[file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("optional increasing declaration is unavailable")?;
            let formal =
                formal_parameter(self.source.context, self.source.bindings, instance.id, 0)
                    .ok_or("optional increasing formal is unavailable")?;
            let parameters = declaration
                .child_nodes()
                .find(|node| node.kind() == NodeKind::ParameterList)
                .ok_or("optional increasing parameter list is unavailable")?;
            let bodies: Vec<_> = declaration
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .collect();
            let mut signatures = self
                .source
                .calls
                .signatures
                .iter()
                .filter(|signature| signature.declaration == instance.id);
            let signature = signatures
                .next()
                .ok_or("optional increasing signature is unavailable")?;
            if signatures.next().is_some()
                || instance.recursive
                || instance.parameters.as_slice() != [optional_array.clone()]
                || signature.parameters.len() != 1
                || signature.parameters[0].has_default
                || signature.return_type != decision_boolean
                || parameters.child_nodes().count() != 1
                || bodies.len() != 1
                || !std::ptr::eq(bodies[0], clause.node)
                || view.declarations[formal.0].ty != optional_array
                || self
                    .source
                    .expression_type(view, file, clause.node)
                    .is_none_or(|value| value.ty != decision_boolean)
            {
                return Err(
                    "optional increasing selected signature or owning body is unsupported".into(),
                );
            }
            // A closed selected-body failure takes precedence over shape refusal.
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(file, declaration, false, true)
            {
                return Err(reason);
            }
            let parts: Vec<_> = clause.node.child_nodes().collect();
            let [block, _] = parts.as_slice() else {
                return Err("optional increasing let body is unsupported".into());
            };
            let locals: Vec<_> = block.child_nodes().collect();
            let [cells, previous, constraint] = locals.as_slice() else {
                return Err("optional increasing requires its two locals and constraint".into());
            };
            let local = |node: &SyntaxNode| {
                self.source
                    .bindings
                    .declarations
                    .iter()
                    .find(|declaration| {
                        declaration.file == file
                            && declaration.role == DeclarationRole::Local
                            && declaration.syntax_range == node.range()
                    })
            };
            let cells = local(cells).ok_or("optional increasing cell identity is unavailable")?;
            let previous =
                local(previous).ok_or("optional increasing predecessor identity is unavailable")?;
            let mut binders: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Generator
                        && clause.node.range().start <= declaration.syntax_range.start
                        && declaration.syntax_range.end <= clause.node.range().end
                })
                .collect();
            binders.sort_by_key(|declaration| declaration.syntax_range.start);
            if block.kind() != NodeKind::LetBlock
                || constraint.kind() != NodeKind::Constraint
                || binders.len() != 2
                || view.declarations[cells.id.0].ty != optional_array
                || view.declarations[previous.id.0].ty != present_array
                || binders
                    .iter()
                    .any(|binder| view.declarations[binder.id.0].ty != integer)
                || cells.id == previous.id
                || binders[0].id == binders[1].id
            {
                return Err("optional increasing local or binder types are unsupported".into());
            }
            let formal_node = find_node(
                self.source.context.files[file].parsed.tree(),
                &self.source.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("optional increasing written formal is unavailable")?;
            let formal_type = formal_node
                .child_nodes()
                .next()
                .ok_or("optional increasing written formal type is unavailable")?;
            let formal_parts: Vec<_> = formal_type.child_nodes().collect();
            if formal_type.kind() != NodeKind::ArrayType
                || formal_parts.len() != 2
                || formal_parts.iter().any(|node| {
                    node.kind() != NodeKind::ScalarType || node.child_nodes().next().is_some()
                })
            {
                return Err(
                    "optional increasing formal requires its bare integer axis and element".into(),
                );
            }
            let names = (
                &self.source.bindings.declarations[formal.0].name,
                &cells.name,
                &previous.name,
                &binders[0].name,
                &binders[1].name,
            );
            let (xs, xx, y, first, second) = names;
            // Every token is retained except comments/layout. The declaration
            // identities below make equal spellings in the two scopes distinct.
            let expected = format!(
                "let {{
                    array [ int ] of var opt int : {xx} = array1d ( {xs} ) ;
                    array [ 1 .. length ( {xx} ) ] of var int : {y} ;
                    constraint forall ( {first} in 1 .. length ( {xx} ) ) (
                        {y} [ {first} ] = if occurs ( {xx} [ {first} ] )
                        then deopt ( {xx} [ {first} ] )
                        elseif {first} = 1 then lb_array ( {xx} )
                        else {y} [ {first} - 1 ] endif
                    ) ;
                }} in forall ( {second} in 2 .. length ( {y} )
                    where occurs ( {xx} [ {second} ] ) ) (
                    deopt ( {xx} [ {second} ] ) >= {y} [ {second} - 1 ]
                )"
            );
            let parsed = &self.source.context.files[file].parsed;
            let actual: Vec<_> = parsed
                .tokens()
                .iter()
                .filter(|token| {
                    clause.node.range().start <= token.range.start
                        && token.range.end <= clause.node.range().end
                        && !matches!(
                            token.kind,
                            TokenKind::Whitespace
                                | TokenKind::LineComment
                                | TokenKind::BlockComment
                        )
                })
                .map(|token| &parsed.source()[token.range.clone()])
                .collect();
            if actual != expected.split_whitespace().collect::<Vec<_>>() {
                return Err(
                    "optional increasing complete written recurrence is unsupported".into(),
                );
            }
            let body_range = self.source.context.files[file]
                .location(clause.node.range())
                .range;
            let mut scopes = Vec::new();
            let mut nodes = vec![declaration];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, node) {
                    return Err("optional increasing written annotation is unsupported".into());
                }
                if is_expression(node.kind())
                    && self
                        .source
                        .expression_type(view, file, node)
                        .is_none_or(|value| !value.ty.known())
                {
                    return Err("optional increasing written expression type is unsupported".into());
                }
                if matches!(
                    node.kind(),
                    NodeKind::CallExpression
                        | NodeKind::GeneratorCallExpression
                        | NodeKind::BinaryExpression
                ) {
                    let fact = self
                        .source
                        .operation_fact(view, file, node)
                        .ok_or("optional increasing written operation is unavailable")?;
                    if !matches!(
                        fact.name.as_str(),
                        "array1d"
                            | "length"
                            | "forall"
                            | "="
                            | "occurs"
                            | "deopt"
                            | "lb_array"
                            | "-"
                            | ">="
                            | ".."
                    ) {
                        return Err(
                            "optional increasing written operation is outside its source family"
                                .into(),
                        );
                    }
                    let parameters = if node.kind() == NodeKind::GeneratorCallExpression {
                        vec![
                            fact.generator_argument
                                .clone()
                                .ok_or("optional increasing generated collection is unavailable")?,
                        ]
                    } else {
                        node.child_nodes()
                            .map(|child| {
                                self.source
                                    .expression_type(view, file, child)
                                    .map(|value| value.ty.clone())
                                    .ok_or("optional increasing operand type is unavailable")
                            })
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    let result = &self
                        .source
                        .expression_type(view, file, node)
                        .ok_or("optional increasing result type is unavailable")?
                        .ty;
                    let tuple = match fact.name.as_str() {
                        "array1d" => {
                            parameters.as_slice() == [optional_array.clone()]
                                && *result == optional_array
                        }
                        "length" => {
                            (parameters.as_slice() == [optional_array.clone()]
                                || parameters.as_slice() == [present_array.clone()])
                                && *result == integer
                        }
                        "lb_array" => {
                            parameters.as_slice() == [optional_array.clone()] && *result == integer
                        }
                        "-" => {
                            parameters.as_slice() == [integer.clone(), integer.clone()]
                                && *result == integer
                        }
                        ".." => {
                            parameters.as_slice() == [integer.clone(), integer.clone()]
                                && *result == integer_set
                        }
                        "occurs" => {
                            parameters.as_slice() == [optional_integer.clone()]
                                && *result == decision_boolean
                        }
                        "deopt" => {
                            parameters.as_slice() == [optional_integer.clone()]
                                && *result == decision_integer
                        }
                        "forall" => {
                            (parameters.as_slice() == [boolean_array.clone()]
                                || parameters.as_slice() == [optional_boolean_array.clone()])
                                && *result == decision_boolean
                        }
                        "=" => {
                            parameters.as_slice() == [integer.clone(), integer.clone()]
                                && *result == boolean
                                || parameters.as_slice()
                                    == [decision_integer.clone(), decision_integer.clone()]
                                    && *result == decision_boolean
                        }
                        ">=" => {
                            parameters.as_slice()
                                == [decision_integer.clone(), decision_integer.clone()]
                                && *result == decision_boolean
                        }
                        _ => false,
                    };
                    if !tuple {
                        return Err(
                            "optional increasing exact operation tuple is unsupported".into()
                        );
                    }
                    self.optional_increasing_operation(
                        file,
                        node,
                        view,
                        &fact.name,
                        &parameters,
                        result,
                    )?;
                    if node.kind() == NodeKind::GeneratorCallExpression {
                        let header = node
                            .child_nodes()
                            .next()
                            .and_then(|list| list.child_nodes().next())
                            .ok_or("optional increasing header is unavailable")?;
                        let binder = binders
                            .iter()
                            .find(|binder| binder.syntax_range == header.range())
                            .ok_or("optional increasing owned header is unsupported")?;
                        scopes.push((
                            binder.id,
                            self.source.context.files[file].location(node.range()).range,
                        ));
                    }
                }
                nodes.extend(node.child_nodes());
            }
            if scopes.len() != 2 {
                return Err("optional increasing complete generator scopes are unsupported".into());
            }
            for reference in self.source.bindings.references.iter().filter(|reference| {
                reference.file == file
                    && reference.kind == ReferenceKind::Value
                    && body_range.start <= reference.location.range.start
                    && reference.location.range.end <= body_range.end
            }) {
                let BindingResolution::Resolved(id) = reference.resolution else {
                    return Err("optional increasing source reference is unresolved".into());
                };
                if id == formal || id == cells.id || id == previous.id {
                    continue;
                }
                if !scopes.iter().any(|(binder, range)| {
                    *binder == id
                        && range.start <= reference.location.range.start
                        && reference.location.range.end <= range.end
                }) {
                    return Err("optional increasing source reference has no owning scope".into());
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "standard optional increasing relation has no output or membership proof".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn optional_increasing_operation(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
        name: &str,
        parameters: &[TypeInst],
        result: &TypeInst,
    ) -> Result<(), String> {
        let fact = self
            .source
            .operation_fact(view, file, node)
            .ok_or("optional increasing operation is unavailable")?;
        let CallOutcome::Resolved {
            declaration,
            parameters: selected,
            return_type,
        } = &fact.outcome
        else {
            return Err("optional increasing operation selection is unsupported".into());
        };
        let owner = &self.source.bindings.declarations[declaration.0];
        if owner.name != name
            || self.source.context.files[owner.file].kind != SourceKind::StandardLibrary
            || !self.source.context.files[owner.file].implicit
            || selected.as_slice() != parameters
            || return_type != result
            || self
                .source
                .expression_type(view, file, node)
                .is_none_or(|value| value.ty != *result)
        {
            return Err("optional increasing operation identity or tuple is unsupported".into());
        }
        if name == "forall" && node.kind() == NodeKind::GeneratorCallExpression {
            if parameters.len() != 1
                || fact.generator_argument.as_ref() != parameters.first()
                || !self.source.core(file, node, view, name)
            {
                return Err("optional increasing forall collection tuple is unsupported".into());
            }
            if owner.role == DeclarationRole::Function {
                return self.source.selected_union_primitive_safety(*declaration);
            }
            return self.optional_increasing_forall_body(*declaration, &parameters[0], result);
        }
        if matches!(name, "occurs" | "deopt" | "absent") {
            if parameters.len() != 1
                || fact.generator_argument.is_some()
                || node.kind() != NodeKind::CallExpression
                || node.child_nodes().count() != 1
                || !parameters[0].optional
                || parameters[0].instantiation != Instantiation::Decision
                || !matches!(parameters[0].kind, TypeKind::Int | TypeKind::Bool)
                || !(crate::optional::core_optional_call(
                    self.source.context,
                    self.source.bindings,
                    view,
                    file,
                    node,
                    name,
                ) || name == "absent"
                    && owner.role == DeclarationRole::Predicate
                    && parameters[0].kind == TypeKind::Bool)
            {
                return Err("optional increasing scalar optional operation is unsupported".into());
            }
            return self.optional_increasing_scalar_wrapper(*declaration, &parameters[0], result);
        }
        self.source
            .parameter_array_primitive(file, node, view, name, parameters, result)
    }
    // Existing core optional semantics remain the boundary. Inspect its exact
    // immediate forwarding wrapper; do not evaluate the option representation.
    pub(in crate::callable_definitions) fn optional_increasing_scalar_wrapper(
        &self,
        id: DeclarationId,
        parameter: &TypeInst,
        result: &TypeInst,
    ) -> Result<(), String> {
        let owner = &self.source.bindings.declarations[id.0];
        let (formal, body, view) = self.optional_increasing_wrapper_body(id, parameter)?;
        let file = owner.file;
        if self
            .source
            .expression_type(&view, file, body)
            .is_none_or(|value| value.ty != *result)
        {
            return Err("optional increasing scalar wrapper result type is unsupported".into());
        }
        let reference = |node: &SyntaxNode| {
            unwrap(node).kind() == NodeKind::Expression
                && self.source.reference(file, unwrap(node)) == Some(formal)
        };
        let argument = |node: &'a SyntaxNode| -> Result<&'a SyntaxNode, String> {
            if node.kind() != NodeKind::CallExpression || node.child_nodes().count() != 1 {
                return Err("optional increasing scalar wrapper argument is unsupported".into());
            }
            Ok(node.child_nodes().next().unwrap())
        };
        let boundary = |node: &SyntaxNode, name| -> Result<(), String> {
            if !crate::optional::core_optional_call(self.source.context, self.source.bindings, &view, file, node, name)
                || self.source.operation_fact(&view, file, node).is_none_or(|fact| !matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. } if parameters.as_slice() == [parameter.clone()] && return_type == result))
                || self.source.expression_type(&view, file, node).is_none_or(|value| value.ty != *result)
            { return Err("optional increasing core optional boundary is unsupported".into()); }
            Ok(())
        };
        if owner.name == "absent" {
            let children: Vec<_> = body.child_nodes().collect();
            let [occurs] = children.as_slice() else {
                return Err("optional increasing absent wrapper is unsupported".into());
            };
            if body.kind() != NodeKind::UnaryExpression || !reference(argument(occurs)?) {
                return Err("optional increasing absent source identity is unsupported".into());
            }
            self.optional_increasing_operation(
                file,
                occurs,
                &view,
                "occurs",
                std::slice::from_ref(parameter),
                result,
            )?;
            return self.optional_increasing_operation(
                file,
                body,
                &view,
                "not",
                std::slice::from_ref(result),
                result,
            );
        }
        if parameter.kind == TypeKind::Bool {
            if !reference(argument(body)?) {
                return Err("optional increasing Boolean option source is unsupported".into());
            }
            return boundary(
                body,
                if owner.name == "occurs" {
                    "occurs_bool"
                } else {
                    "deopt_bool"
                },
            );
        }
        let integer = TypeInst::par(TypeKind::Int);
        let decision_integer = integer.clone().with_inst(Instantiation::Decision);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer)));
        let check_conversion = |node: &'a SyntaxNode| -> Result<(), String> {
            if !reference(argument(node)?) {
                return Err("optional increasing option conversion source is unsupported".into());
            }
            self.optional_increasing_operation(
                file,
                node,
                &view,
                "enum2int",
                std::slice::from_ref(parameter),
                parameter,
            )
        };
        if owner.name == "occurs" {
            let convert = argument(body)?;
            check_conversion(convert)?;
            return boundary(body, "occurs_int");
        }
        let parts: Vec<_> = body.child_nodes().collect();
        let [enum_type, deopt] = parts.as_slice() else {
            return Err("optional increasing deopt wrapper is unsupported".into());
        };
        if !reference(argument(enum_type)?) {
            return Err("optional increasing deopt enum source is unsupported".into());
        }
        self.optional_increasing_operation(
            file,
            enum_type,
            &view,
            "enum_of",
            std::slice::from_ref(parameter),
            &set,
        )?;
        check_conversion(argument(deopt)?)?;
        boundary(deopt, "deopt_int")?;
        self.optional_increasing_operation(
            file,
            body,
            &view,
            "to_enum_internal",
            &[set, decision_integer],
            result,
        )
    }
    pub(in crate::callable_definitions) fn optional_increasing_wrapper_body(
        &self,
        id: DeclarationId,
        parameter: &TypeInst,
    ) -> Result<(DeclarationId, &'a SyntaxNode, CallableFacts), String> {
        let owner = &self.source.bindings.declarations[id.0];
        let written = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("optional increasing wrapper declaration is unavailable")?;
        let formal = formal_parameter(self.source.context, self.source.bindings, id, 0)
            .ok_or("optional increasing wrapper formal is unavailable")?;
        let parameters = written
            .child_nodes()
            .find(|node| node.kind() == NodeKind::ParameterList)
            .ok_or("optional increasing wrapper parameters are unavailable")?;
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err("optional increasing wrapper body is unsupported".into());
        };
        let mut signatures = self
            .source
            .calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == id);
        let signature = signatures
            .next()
            .ok_or("optional increasing wrapper signature is unavailable")?;
        if signatures.next().is_some()
            || signature.parameters.len() != 1
            || signature.parameters[0].has_default
            || parameters.child_nodes().count() != 1
            || self.source.context.files[owner.file].kind != SourceKind::StandardLibrary
            || !self.source.context.files[owner.file].implicit
        {
            return Err("optional increasing wrapper signature or defaults are unsupported".into());
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(owner.file, written, false, true)
        {
            return Err(reason);
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                let string = node.child_nodes().next().is_some_and(|value| matches!(crate::domains::tokens(&self.source.context.files[owner.file].parsed, value).as_slice(), [token] if token.kind == TokenKind::StringLiteral));
                if !string
                    && !self
                        .source
                        .atomic_standard_metadata_safe(owner.file, node, "promise_total")
                    && !self.source.atomic_standard_metadata_safe(
                        owner.file,
                        node,
                        "promise_commutative",
                    )
                {
                    return Err(
                        "optional increasing wrapper written metadata is unsupported".into(),
                    );
                }
                continue;
            }
            if node.kind() == NodeKind::DomainType && node.child_nodes().next().is_some()
                || node.kind() == NodeKind::Parameter
                    && node.child_nodes().any(|child| is_expression(child.kind()))
            {
                return Err(
                    "optional increasing wrapper written type or default is unsupported".into(),
                );
            }
            nodes.extend(node.child_nodes());
        }
        let view = self
            .instances
            .iter()
            .find(|instance| {
                instance.id == id
                    && instance.parameters.as_slice() == std::slice::from_ref(parameter)
                    && !instance.recursive
            })
            .map(|instance| instance.view.clone())
            .unwrap_or_else(|| {
                instantiated_body(
                    self.source.context,
                    self.source.bindings,
                    self.source.calls,
                    id,
                    std::slice::from_ref(parameter),
                )
            });
        if view.declarations[formal.0].ty != *parameter {
            return Err("optional increasing wrapper formal type is unsupported".into());
        }
        Ok((formal, *body, view))
    }
    pub(in crate::callable_definitions) fn optional_increasing_forall_body(
        &self,
        id: DeclarationId,
        parameter: &TypeInst,
        result: &TypeInst,
    ) -> Result<(), String> {
        let owner = &self.source.bindings.declarations[id.0];
        let (formal, body, view) = self.optional_increasing_wrapper_body(id, parameter)?;
        let file = owner.file;
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let mut optional_boolean = boolean.clone();
        optional_boolean.optional = true;
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let present = TypeInst {
            instantiation: Instantiation::Decision,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(boolean.clone()),
            },
        };
        let optional = TypeInst {
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(optional_boolean.clone()),
            },
            ..present.clone()
        };
        if owner.role != DeclarationRole::Predicate
            || owner.name != "forall"
            || *parameter != optional
            || *result != boolean
            || body.kind() != NodeKind::CallExpression
            || body.child_nodes().count() != 1
        {
            return Err("optional increasing optional forall wrapper is unsupported".into());
        }
        let comprehension = body.child_nodes().next().unwrap();
        let parts: Vec<_> = comprehension.child_nodes().collect();
        let [disjunction, list] = parts.as_slice() else {
            return Err("optional increasing forall comprehension is unsupported".into());
        };
        if comprehension.kind() != NodeKind::ArrayComprehension
            || list.kind() != NodeKind::GeneratorList
            || list.child_nodes().count() != 1
        {
            return Err("optional increasing forall comprehension scope is unsupported".into());
        }
        let generator = list.child_nodes().next().unwrap();
        let sources: Vec<_> = generator.child_nodes().collect();
        let [index_set] = sources.as_slice() else {
            return Err("optional increasing forall index source is unsupported".into());
        };
        let binders: Vec<_> = self
            .source
            .bindings
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.file == file
                    && declaration.role == DeclarationRole::Generator
                    && declaration.syntax_range == generator.range()
            })
            .collect();
        let reference = |node: &SyntaxNode, id| {
            unwrap(node).kind() == NodeKind::Expression
                && self.source.reference(file, unwrap(node)) == Some(id)
        };
        if generator.kind() != NodeKind::Generator || binders.len() != 1 || view.declarations[binders[0].id.0].ty != integer
            || !generator.children().iter().any(|element| matches!(element, SyntaxElement::Token(index) if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In))
            || index_set.kind() != NodeKind::CallExpression || index_set.child_nodes().count() != 1 || !index_set.child_nodes().next().is_some_and(|value| reference(value, formal))
        { return Err("optional increasing forall binder or array identity is unsupported".into()); }
        self.optional_increasing_operation(
            file,
            index_set,
            &view,
            "index_set",
            std::slice::from_ref(parameter),
            &set,
        )?;
        let operands: Vec<_> = disjunction.child_nodes().collect();
        let [absent, deopt] = operands.as_slice() else {
            return Err("optional increasing forall presence disjunction is unsupported".into());
        };
        for (node, name) in [(absent, "absent"), (deopt, "deopt")] {
            if node.kind() != NodeKind::CallExpression || node.child_nodes().count() != 1 {
                return Err("optional increasing forall optional argument is unsupported".into());
            }
            let selection = node.child_nodes().next().unwrap();
            let cells: Vec<_> = selection.child_nodes().collect();
            let [source, index] = cells.as_slice() else {
                return Err("optional increasing forall optional cell is unsupported".into());
            };
            if selection.kind() != NodeKind::ArrayAccessExpression
                || !reference(source, formal)
                || !reference(index, binders[0].id)
            {
                return Err("optional increasing forall cell identity is unsupported".into());
            }
            self.optional_increasing_operation(
                file,
                node,
                &view,
                name,
                std::slice::from_ref(&optional_boolean),
                &boolean,
            )?;
        }
        self.optional_increasing_operation(
            file,
            disjunction,
            &view,
            "\\/",
            &[boolean.clone(), boolean.clone()],
            &boolean,
        )?;
        if self
            .source
            .expression_type(&view, file, comprehension)
            .is_none_or(|value| value.ty != present)
        {
            return Err("optional increasing forall collection type is unsupported".into());
        }
        self.optional_increasing_operation(file, body, &view, "forall", &[present], &boolean)
    }
    // Refusal only: stored body boundaries do not carry the initialized actual
    // through the compiler-switchable symmetry wrapper's Boolean output.
    pub(in crate::callable_definitions) fn optional_decreasing_actual_safety(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Option<DefinitionSafety> {
        let owner = &self.source.bindings.declarations[id.0];
        if owner.name != "decreasing"
            || owner.role != DeclarationRole::Predicate
            || self.source.context.files[owner.file].kind != SourceKind::StandardLibrary
            || clause.node.kind() != NodeKind::CallExpression
            || parameters.len() != 1
        {
            return None;
        }
        let integer = TypeInst::par(TypeKind::Int);
        let mut element = integer.clone().with_inst(Instantiation::Decision);
        element.optional = true;
        let optional_array = TypeInst {
            instantiation: Instantiation::Decision,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer],
                element: Box::new(element),
            },
        };
        if parameters[0] != optional_array {
            return None;
        }
        let (file, actual) = call_argument(
            self.source.context,
            self.source.bindings,
            self.source.view(clause.file, clause.node, view),
            clause.file,
            clause.node,
            id,
            0,
        )?;
        let facts = self.source.view(file, actual, view);
        let ty = &self.source.expression_type(facts, file, actual)?.ty;
        let TypeKind::Array { indices, element } = &ty.kind else {
            return None;
        };
        if !ty.known()
            || optional(ty)
            || ty.instantiation != Instantiation::Decision
            || indices.len() != 1
            || indices[0].kind != TypeKind::Int
            || indices[0].instantiation != Instantiation::Parameter
            || element.kind != TypeKind::Int
            || element.instantiation != Instantiation::Decision
            || !crate::types::coerces(ty, &optional_array)
        {
            return None;
        }
        let lexical = if file == clause.file
            && clause.node.range().start <= actual.range().start
            && actual.range().end <= clause.node.range().end
        {
            &clause.generators[..]
        } else {
            &[]
        };
        Some(
            match self.initialized_source_safety(file, actual, facts, lexical, &mut Vec::new()) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown(
                    "optional decreasing initialized source has no output proof".into(),
                ),
            },
        )
    }
}
