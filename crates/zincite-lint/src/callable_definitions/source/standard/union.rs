use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn atomic_standard_metadata_safe(
        &self,
        file: FileId,
        annotation: &SyntaxNode,
        name: &str,
    ) -> bool {
        let Some(value) = annotation.child_nodes().next().map(unwrap) else {
            return false;
        };
        let Some(id) = self.reference(file, value) else {
            return false;
        };
        let declaration = &self.bindings.declarations[id.0];
        let annotation_type = TypeInst {
            instantiation: Instantiation::Parameter,
            optional: false,
            kind: TypeKind::Annotation,
        };
        let mut signatures = self.calls.signatures.iter().filter(|s| s.declaration == id);
        let Some(signature) = signatures.next() else {
            return false;
        };
        value.kind() == NodeKind::Expression
            && crate::domains::tokens(&self.context.files[file].parsed, value).len() == 1
            && declaration.role == DeclarationRole::Annotation
            && declaration.name == name
            && self.context.files[declaration.file].kind == SourceKind::StandardLibrary
            && self.context.files[declaration.file].implicit
            && self.calls.declarations[id.0].ty == annotation_type
            && signature.parameters.is_empty()
            && signature.return_type == annotation_type
            && signatures.next().is_none()
            && find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .is_some_and(|written| {
                written.kind() == NodeKind::AnnotationDeclaration
                    && written.child_nodes().next().is_none()
            })
    }
    // Only source views and the checked aggregate use this bodyless boundary.
    // A changed written body/default/domain is never waived by its core name.
    pub(in crate::callable_definitions) fn selected_union_primitive_safety(
        &self,
        id: DeclarationId,
    ) -> Result<(), String> {
        let declaration = &self.bindings.declarations[id.0];
        let written = find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("selected union primitive declaration is unavailable")?;
        if declaration.role != DeclarationRole::Function
            || self.context.files[declaration.file].kind != SourceKind::StandardLibrary
            || !self.context.files[declaration.file].implicit
        {
            return Err("selected union primitive identity is unsupported".into());
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                if ![
                    "mzn_internal_representation",
                    "promise_total",
                    "promise_commutative",
                ]
                .iter()
                .any(|name| self.atomic_standard_metadata_safe(declaration.file, node, name))
                {
                    return Err("selected union primitive annotation is unsupported".into());
                }
                continue;
            }
            if is_expression(node.kind()) {
                return Err("selected union primitive body/default/domain is unsupported".into());
            }
            nodes.extend(node.child_nodes());
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn selected_union_predicate_identity(
        &self,
        id: DeclarationId,
    ) -> bool {
        let declaration = &self.bindings.declarations[id.0];
        declaration.name == "fzn_array_set_union"
            && declaration.role == DeclarationRole::Predicate
            && self.context.files[declaration.file].kind == SourceKind::StandardLibrary
    }
    // Inspect decision sets without proving their values, cardinality or selector
    // membership. Strict dependencies and output contracts remain independent.
    // These selected inner operations must still be written primitives.
    // Validate atomic standard metadata before skipping its annotation subtree.
    pub(in crate::callable_definitions) fn prefix_primitive_source_safe(
        &self,
        file: FileId,
        written: &SyntaxNode,
    ) -> bool {
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                let values: Vec<_> = node.child_nodes().collect();
                let [value] = values.as_slice() else {
                    return false;
                };
                let tokens = crate::domains::tokens(&self.context.files[file].parsed, value);
                if value.kind() != NodeKind::Expression || tokens.len() != 1 {
                    return false;
                }
                if tokens[0].kind == TokenKind::StringLiteral {
                    continue;
                }
                if ![
                    "promise_total",
                    "promise_commutative",
                    "mzn_internal_representation",
                ]
                .iter()
                .any(|name| self.atomic_standard_metadata_safe(file, node, name))
                {
                    return false;
                }
                continue;
            }
            if is_expression(node.kind()) || node.kind() == NodeKind::Error {
                return false;
            }
            nodes.extend(node.child_nodes());
        }
        true
    }
    pub(in crate::callable_definitions) fn prefix_primitive(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
        name: &str,
        parameters: &[TypeInst],
        result: &TypeInst,
    ) -> bool {
        let Some(call) = self.operation_fact(view, file, node) else {
            return false;
        };
        let CallOutcome::Resolved {
            declaration,
            parameters: selected,
            return_type,
        } = &call.outcome
        else {
            return false;
        };
        if !crate::definitions::core_callable(self.context, self.bindings, *declaration, name)
            || selected.as_slice() != parameters
            || return_type != result
            || self
                .expression_type(view, file, node)
                .is_none_or(|e| &e.ty != result)
        {
            return false;
        }
        if node.kind() == NodeKind::GeneratorCallExpression {
            let eligible = if name == "forall" {
                true
            } else if name == "max" {
                let integer = TypeInst::par(TypeKind::Int);
                let array = TypeInst::par(TypeKind::Array {
                    indices: vec![integer.clone()],
                    element: Box::new(integer.clone()),
                });
                parameters == std::slice::from_ref(&array) && result == &integer
            } else {
                false
            };
            if !eligible
                || parameters.len() != 1
                || call.generator_argument.as_ref() != parameters.first()
            {
                return false;
            }
        } else {
            let arguments: Vec<_> = node.child_nodes().collect();
            if call.generator_argument.is_some()
                || arguments.len() != parameters.len()
                || arguments
                    .iter()
                    .zip(parameters)
                    .any(|(argument, parameter)| {
                        argument.kind() == NodeKind::NamedArgument
                            || self.expression_type(view, file, argument).is_none_or(|e| {
                                !e.ty.known()
                                    || optional(&e.ty)
                                    || !crate::types::coerces(&e.ty, parameter)
                            })
                    })
            {
                return false;
            }
        }
        let owner = &self.bindings.declarations[declaration.0];
        find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .is_some_and(|written| {
            let lists: Vec<_> = written
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::ParameterList)
                .collect();
            matches!(lists.as_slice(), [list] if list.child_nodes().count() == parameters.len())
                && self.prefix_primitive_source_safe(owner.file, written)
        })
    }
    pub(in crate::callable_definitions) fn decision_range_body_safety(
        &self,
        id: DeclarationId,
    ) -> Result<(), String> {
        let error = || "decision range selected written body is unsupported".to_owned();
        let owner = &self.bindings.declarations[id.0];
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or_else(error)?;
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err(error());
        };
        if owner.role != DeclarationRole::Function
            || body.kind() != NodeKind::LetExpression
            || written
                .child_nodes()
                .filter(|n| !is_expression(n.kind()))
                .any(|n| !self.prefix_primitive_source_safe(owner.file, n))
        {
            return Err(error());
        }
        let formal_a = formal_parameter(self.context, self.bindings, id, 0).ok_or_else(error)?;
        let formal_b = formal_parameter(self.context, self.bindings, id, 1).ok_or_else(error)?;
        let lists: Vec<_> = written
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::ParameterList)
            .collect();
        if !matches!(lists.as_slice(), [list] if list.child_nodes().count() == 2)
            || formal_a == formal_b
        {
            return Err(error());
        }
        let locals: Vec<_> = self
            .bindings
            .declarations
            .iter()
            .filter(|d| {
                d.file == owner.file
                    && d.role == DeclarationRole::Local
                    && body.range().start <= d.syntax_range.start
                    && d.syntax_range.end <= body.range().end
            })
            .collect();
        let binders: Vec<_> = self
            .bindings
            .declarations
            .iter()
            .filter(|d| {
                d.file == owner.file
                    && d.role == DeclarationRole::Generator
                    && body.range().start <= d.syntax_range.start
                    && d.syntax_range.end <= body.range().end
            })
            .collect();
        let ([local], [binder]) = (locals.as_slice(), binders.as_slice()) else {
            return Err(error());
        };
        let a = &self.bindings.declarations[formal_a.0].name;
        let b = &self.bindings.declarations[formal_b.0].name;
        let s = &local.name;
        let i = &binder.name;
        let names = [a, b, s, i];
        if names
            .iter()
            .enumerate()
            .any(|(position, name)| names[..position].contains(name))
        {
            return Err(error());
        }
        // Exact canonical syntax rejects added locals, defaults, domains,
        // constraints and opaque expressions; references below retain identities.
        let expected = [
            "let",
            "{",
            "var",
            "set",
            "of",
            "lb",
            "(",
            a,
            ")",
            "..",
            "ub",
            "(",
            b,
            ")",
            ":",
            s,
            ";",
            "constraint",
            "forall",
            "(",
            i,
            "in",
            "ub",
            "(",
            s,
            ")",
            ")",
            "(",
            i,
            "in",
            s,
            "<->",
            "(",
            a,
            "<=",
            i,
            "/\\",
            i,
            "<=",
            b,
            ")",
            ")",
            ";",
            "}",
            "in",
            s,
        ];
        let parsed = &self.context.files[owner.file].parsed;
        let range = body.range();
        let tokens: Vec<_> = parsed
            .tokens()
            .iter()
            .filter(|token| {
                range.start <= token.range.start
                    && token.range.end <= range.end
                    && !matches!(
                        token.kind,
                        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                    )
            })
            .collect();
        if tokens.len() != expected.len()
            || tokens
                .iter()
                .zip(expected)
                .any(|(token, expected)| &parsed.source()[token.range.clone()] != expected)
        {
            return Err(error());
        }
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let decision_set = set.clone().with_inst(Instantiation::Decision);
        let boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let array = TypeInst {
            instantiation: Instantiation::Decision,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(boolean.clone()),
            },
        };
        let view = instantiated_body(
            self.context,
            self.bindings,
            self.calls,
            id,
            &[decision.clone(), decision.clone()],
        );
        for (id, ty) in [
            (formal_a, &decision),
            (formal_b, &decision),
            (local.id, &decision_set),
            (binder.id, &integer),
        ] {
            if view
                .declarations
                .get(id.0)
                .is_none_or(|d| d.declaration != id || &d.ty != ty)
            {
                return Err(error());
            }
        }
        if self
            .expression_type(&view, owner.file, body)
            .is_none_or(|e| e.ty != decision_set)
        {
            return Err(error());
        }
        let mut nodes = vec![*body];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, owner.file, node) {
                return Err(error());
            }
            nodes.extend(node.child_nodes());
            if node.kind() == NodeKind::Expression {
                let token = crate::domains::tokens(parsed, node);
                let Some((id, ty)) = [
                    (formal_a, &decision),
                    (formal_b, &decision),
                    (local.id, &decision_set),
                    (binder.id, &integer),
                ]
                .into_iter()
                .find(|(id, _)| self.reference(owner.file, node) == Some(*id)) else {
                    return Err(error());
                };
                if !matches!(token.as_slice(), [token] if token.kind == TokenKind::Identifier
                    && parsed.source()[token.range.clone()] == self.bindings.declarations[id.0].name)
                    || self
                        .expression_type(&view, owner.file, node)
                        .is_none_or(|e| &e.ty != ty)
                {
                    return Err(error());
                }
            } else if matches!(
                node.kind(),
                NodeKind::CallExpression
                    | NodeKind::RangeExpression
                    | NodeKind::BinaryExpression
                    | NodeKind::GeneratorCallExpression
            ) {
                let call = self
                    .operation_fact(&view, owner.file, node)
                    .ok_or_else(error)?;
                let (parameters, result) = match call.name.as_str() {
                    "lb" => (vec![decision.clone()], integer.clone()),
                    "ub" if node
                        .child_nodes()
                        .next()
                        .is_some_and(|n| self.reference(owner.file, n) == Some(local.id)) =>
                    {
                        (vec![decision_set.clone()], set.clone())
                    }
                    "ub" => (vec![decision.clone()], integer.clone()),
                    ".." => (vec![integer.clone(), integer.clone()], set.clone()),
                    "in" => (
                        vec![decision.clone(), decision_set.clone()],
                        boolean.clone(),
                    ),
                    "/\\" | "<->" => (vec![boolean.clone(), boolean.clone()], boolean.clone()),
                    "<=" => {
                        let CallOutcome::Resolved { parameters, .. } = &call.outcome else {
                            return Err(error());
                        };
                        if !matches!(parameters.as_slice(), [left, right]
                            if (left == &decision && (right == &integer || right == &decision))
                                || (left == &integer && right == &decision))
                        {
                            return Err(error());
                        }
                        (parameters.clone(), boolean.clone())
                    }
                    "forall" => (vec![array.clone()], boolean.clone()),
                    _ => return Err(error()),
                };
                if !self.prefix_primitive(owner.file, node, &view, &call.name, &parameters, &result)
                {
                    return Err(error());
                }
            }
        }
        if let Some(reason) = self.closed_integer_source_error(owner.file, body, false, true) {
            return Err(reason);
        }
        Ok(())
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    // Required value-body inspection belongs to source admission, independent
    // of Boolean forwarding. The local failure also retains its written location.
    pub(in crate::callable_definitions) fn decision_set_union_value_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> Result<DefinitionSafety, UnavailableCallableDefinition> {
        let mut location = self.source.context.files[file].location(node.range());
        if !crate::definitions::annotations_safe(self.source.context, file, node) {
            return Err(UnavailableCallableDefinition {
                location,
                targets: Vec::new(),
                reason: "decision set source annotation is unsupported".into(),
            });
        }
        let node = unwrap(node);
        if node.kind() == NodeKind::BinaryExpression
            && self.source.core(file, node, view, "intersect")
        {
            let failure = |reason: String| UnavailableCallableDefinition {
                location: location.clone(),
                targets: Vec::new(),
                reason,
            };
            let result = self
                .source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|value| &value.ty);
            let children: Vec<_> = node.child_nodes().collect();
            if result.is_none_or(|t| !decision_integer_set(t)) || children.len() != 2
                || children.iter().any(|child|
                    self.source.expression_type(self.source.view(file, child, view), file, child)
                        .map(|value| &value.ty) != result)
                || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters.iter().all(|p| Some(p) == result)
                            && Some(return_type) == result))
            {
                return Err(failure("decision set intersection signature is unsupported".into()));
            }
            for child in children {
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, child, view, generators, active)
                {
                    let child = unwrap(child);
                    if child.kind() == NodeKind::GeneratorCallExpression
                        && self.source.core(file, child, view, "array_union")
                        || child.kind() == NodeKind::BinaryExpression
                            && self.source.core(file, child, view, "intersect")
                    {
                        self.decision_set_union_value_safety(
                            file, child, view, generators, active,
                        )?;
                    }
                    return Err(failure(reason));
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, child, true, true)
                {
                    return Err(failure(reason));
                }
            }
            return Ok(DefinitionSafety::Unknown(
                "decision set intersection values are unproved".into(),
            ));
        }
        let mut entered = false;
        let checked = (|| -> Result<DefinitionSafety, String> {
            if node.kind() != NodeKind::GeneratorCallExpression
                || !self.source.core(file, node, view, "array_union")
            {
                return Err("decision set union source identity is unsupported".into());
            }
            let construction = self.collection_construction_safety(file, node, view, generators);
            if let DefinitionSafety::Unsupported(reason) = &construction {
                return Err(reason.clone());
            }
            let call = self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .ok_or("decision set union selection is unavailable")?;
            let CallOutcome::Resolved {
                declaration: id,
                parameters,
                return_type,
            } = &call.outcome
            else {
                return Err("decision set union selection is unsupported".into());
            };
            if !decision_integer_set(return_type)
                || !matches!(parameters.as_slice(), [array]
                    if array.known() && !optional(array) && array.instantiation == Instantiation::Decision
                        && matches!(&array.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                && indices[0].instantiation == Instantiation::Parameter
                                && indices[0].kind == TypeKind::Int && element.as_ref() == return_type))
                || call.generator_argument.as_ref().is_none_or(|actual| {
                    !actual.known()
                        || optional(actual)
                        || !crate::types::coerces(actual, &parameters[0])
                })
                || self
                    .source
                    .expression_type(self.source.view(file, node, view), file, node)
                    .is_none_or(|value| value.ty != *return_type)
            {
                return Err("decision set union selected value tuple is unsupported".into());
            }
            let owner = &self.source.bindings.declarations[id.0];
            let written = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("selected decision set union declaration is unavailable")?;
            let body = written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .last()
                .ok_or("selected decision set union value body is unavailable")?;
            location = self.source.context.files[owner.file].location(body.range());
            if owner.role != DeclarationRole::Function
                || !self.source.callable_annotations_safe(owner.file, written)
            {
                return Err("selected decision set union body or annotation is unsupported".into());
            }
            if active.contains(id) {
                return Err("recursive decision set union value body is unsupported".into());
            }
            active.push(*id);
            entered = true;
            let formal = formal_parameter(self.source.context, self.source.bindings, *id, 0)
                .ok_or("selected decision set union formal is unavailable")?;
            let formal_node = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &self.source.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("selected decision set union written formal is unavailable")?;
            if formal_node.child_nodes().any(|n| is_expression(n.kind())) {
                return Err("selected decision set union formal default is unsupported".into());
            }
            let body_view = instantiated_body(
                self.source.context,
                self.source.bindings,
                self.source.calls,
                *id,
                parameters,
            );
            if body_view.declarations[formal.0].ty != parameters[0] {
                return Err(
                    "selected decision set union concrete formal type is unsupported".into(),
                );
            }
            self.type_dependencies(owner.file, written, &body_view, &[])?;
            self.type_dependencies(owner.file, formal_node, &body_view, &[])?;
            let mut nodes = vec![formal_node];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, owner.file, node) {
                    return Err(
                        "selected decision set union formal annotation is unsupported".into(),
                    );
                }
                nodes.extend(node.child_nodes());
            }
            let written_type = written
                .child_nodes()
                .next()
                .ok_or("selected decision set union return type is unavailable")?;
            for source in [written_type, formal_node] {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(owner.file, source, true, true)
                {
                    return Err(reason);
                }
            }
            let value = self
                .source
                .expression_type(&body_view, owner.file, body)
                .ok_or("decision set union value body type is unavailable")?;
            if !value.ty.known()
                || optional(&value.ty)
                || !crate::types::coerces(&value.ty, return_type)
            {
                return Err("decision set union value body type is unsupported".into());
            }
            // Ordinary source inspectors may start a fresh active vector. Refuse
            // this written union recurrence before entering those inspectors.
            let mut nodes = vec![body];
            while let Some(node) = nodes.pop() {
                if let Some(CallOutcome::Resolved { declaration, .. }) = self
                    .source
                    .operation_fact(&body_view, owner.file, node)
                    .map(|call| &call.outcome)
                    && active.contains(declaration)
                    && crate::definitions::core_callable(
                        self.source.context,
                        self.source.bindings,
                        *declaration,
                        "array_union",
                    )
                {
                    return Err("recursive decision set union value body is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            if !self.source.source_annotations_safe(owner.file, body) {
                return Err("decision set union value body annotation is unsupported".into());
            }
            let conditional = unwrap(body);
            if conditional.kind() == NodeKind::ConditionalExpression {
                let typed = |node: &SyntaxNode| {
                    self.source
                        .expression_type(&body_view, owner.file, node)
                        .map(|value| &value.ty)
                };
                let branches: Vec<_> = conditional.child_nodes().collect();
                let mut values = Vec::new();
                let mut complete = false;
                let mut inactive = None;
                for (position, branch) in branches.iter().enumerate() {
                    if !crate::definitions::annotations_safe(
                        self.source.context,
                        owner.file,
                        branch,
                    ) {
                        return Err(
                            "decision set union conditional branch annotation is unsupported"
                                .into(),
                        );
                    }
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let branch_body = if branch.kind() == NodeKind::ConditionalBranch
                        && parts.len() == 2
                    {
                        let guard = parts[0];
                        if typed(guard).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || t.instantiation != Instantiation::Parameter
                                || t.kind != TypeKind::Bool
                        }) {
                            return Err(
                                "decision set union conditional guard is unsupported".into()
                            );
                        }
                        if let Some(literal) = self.source.assertion_literal(owner.file, guard) {
                            if branches.len() != 2 {
                                return Err(
                                    "decision set union literal conditional shape is unsupported"
                                        .into(),
                                );
                            }
                            inactive = match literal {
                                TokenKind::False => Some(0),
                                TokenKind::True => Some(1),
                                _ => None,
                            };
                        }
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(owner.file, guard, &body_view, &[], active)
                        {
                            return Err(reason);
                        }
                        // Each eager guard has its own closed-error check; the
                        // entire conditional remains lazy and has no known value.
                        if let Some(reason) = self
                            .source
                            .closed_integer_source_error(owner.file, guard, true, true)
                        {
                            return Err(reason);
                        }
                        parts[1]
                    } else if branch.kind() == NodeKind::ElseBranch
                        && parts.len() == 1
                        && position + 1 == branches.len()
                    {
                        complete = true;
                        parts[0]
                    } else {
                        return Err("decision set union conditional branch is unsupported".into());
                    };
                    let empty = unwrap(branch_body).kind() == NodeKind::SetLiteral
                        && unwrap(branch_body).child_nodes().next().is_none()
                        && typed(branch_body).is_some_and(|t| t.known() && !optional(t)
                            && t.instantiation == Instantiation::Parameter
                            && matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element)
                                && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Bottom));
                    if typed(branch_body).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || (t.kind != value.ty.kind && !empty)
                            || !crate::types::coerces(t, &value.ty)
                    }) {
                        return Err(
                            "decision set union conditional body type is unsupported".into()
                        );
                    }
                    values.push(branch_body);
                }
                if !complete || branches.len() < 2 {
                    return Err("decision set union conditional requires a complete else".into());
                }
                for (position, branch_body) in values.iter().enumerate() {
                    let inspected = if inactive == Some(position) {
                        // Reuse the existing bounded literal-inactive source
                        // inspection without evaluating its closed arithmetic.
                        let inspector = SourceInspector {
                            context: self.source.context,
                            bindings: self.source.bindings,
                            calls: self.source.calls,
                            instantiations: self.source.instantiations,
                            domains: self.source.domains,
                            lookups: None,
                            selected_set_source: true,
                            inactive_integer_body: Some((owner.file, branch_body.range())),
                        };
                        let interpreter = BodyInterpreter::without_bodies(&inspector);
                        interpreter.initialized_source_safety(
                            owner.file,
                            branch_body,
                            &body_view,
                            &[],
                            active,
                        )
                    } else {
                        if let Some(reason) = self.source.closed_integer_source_error(
                            owner.file,
                            branch_body,
                            true,
                            true,
                        ) {
                            return Err(reason);
                        }
                        let cell = unwrap(branch_body);
                        let parts: Vec<_> = cell.child_nodes().collect();
                        if cell.kind() == NodeKind::ArrayAccessExpression
                            && parts.len() == 2
                            && self
                                .source
                                .core(owner.file, unwrap(parts[0]), &body_view, "array1d")
                        {
                            let converted = unwrap(parts[0]);
                            let arguments: Vec<_> = converted.child_nodes().collect();
                            let [argument] = arguments.as_slice() else {
                                return Err(
                                    "decision set singleton conversion arity is unsupported".into(),
                                );
                            };
                            let source = unwrap(argument);
                            let selector = parts[1];
                            if converted.kind() != NodeKind::CallExpression
                                || source.kind() != NodeKind::Expression
                                || !matches!(crate::domains::tokens(&self.source.context.files[owner.file].parsed, source).as_slice(),
                                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                                || self.source.reference(owner.file, source) != Some(formal)
                                || typed(argument) != Some(&parameters[0])
                                || typed(converted) != Some(&parameters[0])
                                || typed(cell) != Some(return_type)
                                || typed(selector).is_none_or(|t| {
                                    !t.known()
                                        || optional(t)
                                        || t.instantiation != Instantiation::Parameter
                                        || t.kind != TypeKind::Int
                                })
                            {
                                return Err(
                                    "decision set singleton source or selector type is unsupported"
                                        .into(),
                                );
                            }
                            let conversion = self
                                .source
                                .operation_fact(&body_view, owner.file, converted)
                                .ok_or(
                                    "decision set singleton conversion selection is unavailable",
                                )?;
                            let CallOutcome::Resolved {
                                declaration: primitive,
                                parameters: primitive_parameters,
                                return_type: primitive_result,
                            } = &conversion.outcome
                            else {
                                return Err(
                                    "decision set singleton conversion selection is unsupported"
                                        .into(),
                                );
                            };
                            if primitive_parameters.as_slice()
                                != std::slice::from_ref(&parameters[0])
                                || primitive_result != &parameters[0]
                            {
                                return Err(
                                    "decision set singleton conversion tuple is unsupported".into(),
                                );
                            }
                            let declaration = &self.source.bindings.declarations[primitive.0];
                            let written = find_node(
                                self.source.context.files[declaration.file].parsed.tree(),
                                &declaration.syntax_range,
                                declaration.role,
                            )
                            .ok_or("decision set singleton primitive declaration is unavailable")?;
                            if declaration.role != DeclarationRole::Function {
                                return Err(
                                    "decision set singleton primitive role is unsupported".into()
                                );
                            }
                            // This source path admits only the written bodyless
                            // primitive. Changed bodies/defaults/domains stay refused.
                            let mut nodes = vec![written];
                            while let Some(node) = nodes.pop() {
                                if is_expression(node.kind())
                                    || !crate::definitions::annotations_safe(
                                        self.source.context,
                                        declaration.file,
                                        node,
                                    )
                                {
                                    return Err("decision set singleton primitive body or written source is unsupported".into());
                                }
                                nodes.extend(node.child_nodes());
                            }
                            let mut nodes = vec![*branch_body];
                            while let Some(node) = nodes.pop() {
                                if !self.source.source_annotations_safe(owner.file, node) {
                                    return Err(
                                        "decision set singleton source annotation is unsupported"
                                            .into(),
                                    );
                                }
                                nodes.extend(node.child_nodes());
                            }
                            for source in [*argument, selector] {
                                if let DefinitionSafety::Unsupported(reason) = self
                                    .initialized_source_safety(
                                        owner.file,
                                        source,
                                        &body_view,
                                        &[],
                                        active,
                                    )
                                {
                                    return Err(reason);
                                }
                                if let Some(reason) = self
                                    .source
                                    .closed_integer_source_error(owner.file, source, true, true)
                                {
                                    return Err(reason);
                                }
                            }
                            if !self
                                .source
                                .array_nonempty_branch(owner.file, cell, formal, &body_view)
                                || crate::domains::invariant_expression_integer(
                                    self.source.context,
                                    self.source.bindings,
                                    owner.file,
                                    selector,
                                ) != Ok(Some(1))
                            {
                                return Err("decision set singleton lacks its same-array nonempty branch or first selector".into());
                            }
                            DefinitionSafety::Unknown(
                                "decision set singleton value and membership are unproved".into(),
                            )
                        } else if cell.kind() == NodeKind::LetExpression {
                            self.selected_union_body_source_safety(
                                (owner.file, branch_body, &body_view),
                                &[formal],
                                active,
                                &mut location,
                            )?;
                            DefinitionSafety::Unknown(
                                "decision set local relation value is unproved".into(),
                            )
                        } else {
                            self.initialized_source_safety(
                                owner.file,
                                branch_body,
                                &body_view,
                                &[],
                                active,
                            )
                        }
                    };
                    if let DefinitionSafety::Unsupported(reason) = inspected {
                        return Err(reason);
                    }
                }
                return Ok(construction);
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(owner.file, body, true, true)
            {
                return Err(reason);
            }
            if let Some(arguments) =
                self.source
                    .assertion_arguments(owner.file, unwrap(body), &body_view)
                && arguments.len() == 3
                && self
                    .source
                    .assertion_literal(arguments[0].0, arguments[0].1)
                    == Some(TokenKind::False)
            {
                let assertion = self
                    .source
                    .operation_fact(
                        self.source.view(owner.file, unwrap(body), &body_view),
                        owner.file,
                        unwrap(body),
                    )
                    .ok_or("selected value assertion signature is unavailable")?;
                let CallOutcome::Resolved {
                    parameters,
                    return_type,
                    ..
                } = &assertion.outcome
                else {
                    return Err("selected value assertion signature is unsupported".into());
                };
                if parameters.len() != 3
                    || *return_type != value.ty
                    || parameters[2] != *return_type
                    || parameters
                        .iter()
                        .any(|parameter| !parameter.known() || optional(parameter))
                {
                    return Err("selected value assertion signature is unsupported".into());
                }
                for (position, (file, argument)) in arguments.iter().enumerate() {
                    let ty = self
                        .source
                        .expression_type(
                            self.source.view(*file, argument, &body_view),
                            *file,
                            argument,
                        )
                        .ok_or("selected value assertion argument type is unavailable")?;
                    if !ty.ty.known()
                        || optional(&ty.ty)
                        || !crate::types::coerces(&ty.ty, &parameters[position])
                        || match position {
                            0 => {
                                ty.ty.instantiation != Instantiation::Parameter
                                    || ty.ty.kind != TypeKind::Bool
                            }
                            1 => {
                                ty.ty.instantiation != Instantiation::Parameter
                                    || ty.ty.kind != TypeKind::String
                            }
                            _ => !crate::types::coerces(&ty.ty, &value.ty),
                        }
                    {
                        return Err("selected value assertion argument type is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(*file, argument, &body_view, &[], active)
                    {
                        return Err(reason);
                    }
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(*file, argument, true, true)
                    {
                        return Err(reason);
                    }
                }
                return Err("false assertion condition aborts evaluation".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(owner.file, body, &body_view, &[], active)
            {
                return Err(reason);
            }
            Ok(construction)
        })();
        if entered {
            active.pop();
        }
        checked.map_err(|reason| UnavailableCallableDefinition {
            location,
            targets: Vec::new(),
            reason,
        })
    }
    // This is typed source inspection of the selected union/FZN written family.
    // Visible IDs are checked owning formals, preceding locals and integer binders;
    // they carry neither a known value nor a definition/output guarantee.
    pub(in crate::callable_definitions) fn selected_union_body_source_safety(
        &self,
        source: (FileId, &'a SyntaxNode, &CallableFacts),
        visible: &[DeclarationId],
        active: &mut Vec<DeclarationId>,
        location: &mut SourceLocation,
    ) -> Result<(), String> {
        let (file, written, view) = source;
        let node = unwrap(written);
        let typed = |node: &SyntaxNode| {
            self.source
                .expression_type(view, file, node)
                .map(|value| &value.ty)
        };
        let integer = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let parameter_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if integer(element))
        };
        let set_array = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Decision
                && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && integer(&indices[0])
                        && decision_integer_set(element))
        };
        let bare = |node: &SyntaxNode| {
            let node = unwrap(node);
            (node.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                .then(|| self.source.reference(file, node)).flatten()
        };
        if node.kind() == NodeKind::AnnotatedExpression {
            let parts: Vec<_> = node.child_nodes().collect();
            let [body, annotation] = parts.as_slice() else {
                return Err("selected union annotated constraint shape is unsupported".into());
            };
            let call = annotation
                .child_nodes()
                .next()
                .map(unwrap)
                .ok_or("selected union constraint metadata is unavailable")?;
            let arguments: Vec<_> = call.child_nodes().collect();
            let local = arguments.first().and_then(|argument| bare(argument));
            if annotation.kind() != NodeKind::Annotation
                || call.kind() != NodeKind::CallExpression
                || !self.source.core(file, call, view, "defines_var")
                || arguments.len() != 1
                || local.is_none_or(|id| {
                    !visible.contains(&id)
                        || self.source.bindings.declarations[id.0].role != DeclarationRole::Local
                        || self.source.bindings.declarations[id.0].file != file
                        || !decision_integer_set(&view.declarations[id.0].ty)
                })
                || typed(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
            {
                return Err(
                    "selected union constraint metadata or owning local is unsupported".into(),
                );
            }
            let CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &self
                .source
                .operation_fact(view, file, call)
                .ok_or("selected union constraint metadata selection is unavailable")?
                .outcome
            else {
                return Err("selected union constraint metadata selection is unsupported".into());
            };
            if parameters.len() != 1
                || !decision_integer_set(&parameters[0])
                || typed(arguments[0]).is_none_or(|t| t != &parameters[0])
                || return_type.instantiation != Instantiation::Parameter
                || return_type.optional
                || return_type.kind != TypeKind::Annotation
                || typed(call) != Some(return_type)
            {
                return Err("selected union constraint metadata tuple is unsupported".into());
            }
            self.source.selected_union_primitive_safety(*declaration)?;
            self.selected_union_body_source_safety(
                (file, arguments[0], view),
                visible,
                active,
                location,
            )?;
            return self.selected_union_body_source_safety(
                (file, body, view),
                visible,
                active,
                location,
            );
        }
        if !self.source.source_annotations_safe(file, written) {
            return Err("selected union local source annotation is unsupported".into());
        }
        let ty = typed(node).ok_or("selected union local source type is unavailable")?;
        if !ty.known() || optional(ty) {
            return Err("selected union local source type or optionality is unsupported".into());
        }
        if node.kind() == NodeKind::LetExpression {
            let parts: Vec<_> = node.child_nodes().collect();
            let [block, body] = parts.as_slice() else {
                return Err("selected union let shape is unsupported".into());
            };
            if block.kind() != NodeKind::LetBlock
                || typed(body).is_none_or(|body_type| !crate::types::coerces(body_type, ty))
            {
                return Err("selected union let result type is unsupported".into());
            }
            let mut locals = visible.to_vec();
            for item in block.child_nodes() {
                if item.kind() == NodeKind::Constraint {
                    let expression = item
                        .child_nodes()
                        .next()
                        .ok_or("selected union local constraint is unavailable")?;
                    if item.child_nodes().count() != 1
                        || !self.source.source_annotations_safe(file, item)
                    {
                        return Err("selected union local constraint shape is unsupported".into());
                    }
                    self.selected_union_body_source_safety(
                        (file, expression, view),
                        &locals,
                        active,
                        location,
                    )?;
                    continue;
                }
                let declaration = self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .find(|declaration| {
                        declaration.file == file
                            && declaration.syntax_range == item.range()
                            && declaration.role == DeclarationRole::Local
                    })
                    .filter(|_| item.kind() == NodeKind::Declaration)
                    .ok_or("selected union local declaration is unavailable")?;
                let local_type = &view.declarations[declaration.id.0].ty;
                let initializers: Vec<_> = item
                    .child_nodes()
                    .filter(|node| is_expression(node.kind()))
                    .collect();
                if locals.contains(&declaration.id)
                    || !local_type.known()
                    || optional(local_type)
                    || !(integer(local_type) && initializers.len() == 1
                        || (decision_integer_set(local_type) || set_array(local_type))
                            && initializers.is_empty())
                {
                    return Err("selected union local type or initializer is unsupported".into());
                }
                for annotation in item
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::Annotation)
                {
                    if !decision_integer_set(local_type)
                        || !self.source.atomic_standard_metadata_safe(
                            file,
                            annotation,
                            "is_defined_var",
                        )
                    {
                        return Err(
                            "selected union local declaration metadata is unsupported".into()
                        );
                    }
                }
                let mut types = vec![
                    item.child_nodes()
                        .next()
                        .ok_or("selected union local written type is unavailable")?,
                ];
                while let Some(written_type) = types.pop() {
                    if !self.source.source_annotations_safe(file, written_type) {
                        return Err(
                            "selected union local written type annotation is unsupported".into(),
                        );
                    }
                    if written_type.kind() == NodeKind::DomainType {
                        for domain in written_type.child_nodes() {
                            self.selected_union_body_source_safety(
                                (file, domain, view),
                                &locals,
                                active,
                                location,
                            )?;
                        }
                    } else {
                        types.extend(written_type.child_nodes());
                    }
                }
                for initializer in initializers {
                    if typed(initializer).is_none_or(|t| !integer(t)) {
                        return Err(
                            "selected union local integer initializer type is unsupported".into(),
                        );
                    }
                    self.selected_union_body_source_safety(
                        (file, initializer, view),
                        &locals,
                        active,
                        location,
                    )?;
                }
                locals.push(declaration.id);
            }
            return self.selected_union_body_source_safety(
                (file, body, view),
                &locals,
                active,
                location,
            );
        }
        if node.kind() == NodeKind::ConditionalExpression {
            let branches: Vec<_> = node.child_nodes().collect();
            let mut complete = false;
            for (position, branch) in branches.iter().enumerate() {
                if !self.source.source_annotations_safe(file, branch) {
                    return Err("selected union predicate branch annotation is unsupported".into());
                }
                let parts: Vec<_> = branch.child_nodes().collect();
                let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                    if typed(parts[0]).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.instantiation != Instantiation::Parameter
                            || t.kind != TypeKind::Bool
                    }) || self.source.assertion_literal(file, parts[0]).is_some()
                    {
                        return Err("selected union predicate symbolic guard is unsupported".into());
                    }
                    self.selected_union_body_source_safety(
                        (file, parts[0], view),
                        visible,
                        active,
                        location,
                    )?;
                    parts[1]
                } else if branch.kind() == NodeKind::ElseBranch
                    && parts.len() == 1
                    && position + 1 == branches.len()
                {
                    complete = true;
                    parts[0]
                } else {
                    return Err("selected union predicate conditional shape is unsupported".into());
                };
                if typed(body)
                    .is_none_or(|t| !t.known() || optional(t) || !crate::types::coerces(t, ty))
                {
                    return Err(
                        "selected union predicate conditional body type is unsupported".into(),
                    );
                }
                self.selected_union_body_source_safety(
                    (file, body, view),
                    visible,
                    active,
                    location,
                )?;
            }
            return if complete && branches.len() >= 2 {
                Ok(())
            } else {
                Err("selected union predicate conditional requires complete else".into())
            };
        }
        let decision_bool = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Decision
                && ty.kind == TypeKind::Bool
        };
        // The checked operator path below inspects the written primitive and
        // each decision operand before this unit can be admitted.
        let decision_conjunction = node.kind() == NodeKind::BinaryExpression
            && self.source.core(file, node, view, "/\\")
            && decision_bool(ty)
            && {
                let operands: Vec<_> = node.child_nodes().collect();
                operands.len() == 2
                    && operands.iter().all(|operand| {
                        typed(operand).is_some_and(decision_bool)
                    })
                    && self.source.operation_fact(view, file, node).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(decision_bool)
                                && return_type == ty)
                    })
            };
        if !decision_conjunction
            && let Some(reason) = self
                .source
                .closed_integer_source_error(file, node, true, true)
        {
            return Err(reason);
        }
        if let Some(id) = bare(node) {
            if !visible.contains(&id)
                || self.source.bindings.declarations[id.0].file != file
                || ty != &view.declarations[id.0].ty
                || !(integer(ty) || decision_integer_set(ty) || set_array(ty))
            {
                return Err("selected union source is not a checked owning formal/local".into());
            }
            // These lexical declarations were checked before becoming visible.
            // Re-entering strict dependency extraction would demand a value proof
            // for an inspected symbolic local extremum.
            return Ok(());
        }
        if node.kind() == NodeKind::ArrayAccessExpression {
            let parts: Vec<_> = node.child_nodes().collect();
            let [array, selector] = parts.as_slice() else {
                return Err("selected union local array cell shape is unsupported".into());
            };
            if typed(array).is_none_or(|t| !set_array(t))
                || !decision_integer_set(ty)
                || typed(selector).is_none_or(|t| !integer(t))
                || bare(array).is_none_or(|id| !visible.contains(&id))
            {
                return Err("selected union local array cell type or owner is unsupported".into());
            }
            let id = bare(array).ok_or("selected union local array owner is unavailable")?;
            let Domain::Array { indices, .. } =
                crate::domains::bare_index_domain(&self.source.domains.declarations[id.0].domain)
            else {
                return Err("selected union local array axis is unavailable".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("selected union local array axis rank is unsupported".into());
            };
            // Only a closed exclusion is a veto. Symbolic l/u bounds and min/max
            // selectors keep their inspected Unknown source result.
            let empty = crate::domains::index_domain_interval(axis)
                .is_some_and(|(lower, upper)| lower > upper)
                || matches!(crate::domains::bare_index_domain(axis), Domain::LiteralSet(values)
                    if values.is_empty());
            let outside = crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                file,
                selector,
            )
            .is_ok_and(|value| {
                value.is_some_and(|value| {
                    crate::domains::index_domain_member(axis, value) == Some(false)
                })
            });
            if empty || outside {
                return Err("selected union local array cell is outside its declared axis".into());
            }
            for child in parts {
                self.selected_union_body_source_safety(
                    (file, child, view),
                    visible,
                    active,
                    location,
                )?;
            }
            return Ok(());
        }
        if node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(file, node, view, "forall")
        {
            let parts: Vec<_> = node.child_nodes().collect();
            let [list, body] = parts.as_slice() else {
                return Err("selected union forall shape is unsupported".into());
            };
            let headers: Vec<_> = list.child_nodes().collect();
            let [header] = headers.as_slice() else {
                return Err("selected union forall requires one integer header".into());
            };
            let domains: Vec<_> = header.child_nodes().collect();
            let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, header);
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
            if list.kind() != NodeKind::GeneratorList
                || header.kind() != NodeKind::Generator
                || domains.len() != 1
                || binders.len() != 1
                || tokens.iter().position(|token| token.kind == TokenKind::In) != Some(1)
                || !matches!(
                    tokens[0].kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                )
                || !self.source.source_annotations_safe(file, list)
                || !self.source.source_annotations_safe(file, header)
                || !integer(&view.declarations[binders[0].id.0].ty)
                || typed(domains[0]).is_none_or(|t| !parameter_set(t))
                || typed(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
                || ty.kind != TypeKind::Bool
            {
                return Err("selected union forall header/body type is unsupported".into());
            }
            let CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &self
                .source
                .operation_fact(view, file, node)
                .ok_or("selected union forall selection is unavailable")?
                .outcome
            else {
                return Err("selected union forall selection is unsupported".into());
            };
            if parameters.len() != 1
                || return_type != ty
                || node.child_nodes().count() != 2
                || !parameters[0].known()
                || parameters[0].optional
                || !matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && integer(&indices[0]) && element.known()
                        && element.kind == TypeKind::Bool)
                || self
                    .source
                    .operation_fact(view, file, node)
                    .and_then(|call| call.generator_argument.as_ref())
                    .is_none_or(|actual| {
                        !actual.known()
                            || optional(actual)
                            || !crate::types::coerces(actual, &parameters[0])
                    })
            {
                return Err("selected union forall concrete tuple is unsupported".into());
            }
            self.source.selected_union_primitive_safety(*declaration)?;
            self.selected_union_body_source_safety(
                (file, domains[0], view),
                visible,
                active,
                location,
            )?;
            let mut scoped = visible.to_vec();
            scoped.push(binders[0].id);
            return self.selected_union_body_source_safety(
                (file, body, view),
                &scoped,
                active,
                location,
            );
        }
        if node.kind() == NodeKind::CallExpression
            && self
                .source
                .operation_fact(view, file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { declaration, .. }
                    if self.source.selected_union_predicate_identity(*declaration))
                })
        {
            let arguments: Vec<_> = node.child_nodes().collect();
            if arguments.len() != 2
                || arguments
                    .iter()
                    .any(|argument| argument.kind() == NodeKind::NamedArgument)
                || ty.kind != TypeKind::Bool
                || typed(arguments[0]).is_none_or(|t| !set_array(t))
                || typed(arguments[1]).is_none_or(|t| !decision_integer_set(t))
            {
                return Err("selected union nested predicate actual shape is unsupported".into());
            }
            for argument in arguments {
                self.selected_union_body_source_safety(
                    (file, argument, view),
                    visible,
                    active,
                    location,
                )?;
            }
            if let Err(failure) =
                self.selected_union_predicate_body_safety(file, node, view, active)
            {
                *location = failure.location;
                return Err(failure.reason);
            }
            return Ok(());
        }
        if node.kind() == NodeKind::CallExpression {
            let parts: Vec<_> = node.child_nodes().collect();
            let name = [
                "array1d",
                "enum2int",
                "ub_array",
                "length",
                "index_set",
                "min",
                "max",
            ]
            .into_iter()
            .find(|name| self.source.core(file, node, view, name));
            if let Some(name) = name {
                let [argument] = parts.as_slice() else {
                    return Err("selected union source view arity is unsupported".into());
                };
                let argument_type = typed(argument)
                    .ok_or("selected union source view actual type is unavailable")?;
                let shape = match name {
                    "array1d" => set_array(argument_type) && ty == argument_type,
                    "enum2int" => {
                        (set_array(argument_type) || decision_integer_set(argument_type))
                            && ty == argument_type
                    }
                    "ub_array" => {
                        set_array(argument_type)
                            && parameter_set(ty)
                            && bare(argument).is_some_and(|id| {
                                visible.contains(&id)
                                    && self.source.bindings.declarations[id.0].role
                                        == DeclarationRole::Parameter
                            })
                    }
                    "length" => set_array(argument_type) && integer(ty),
                    "index_set" => set_array(argument_type) && parameter_set(ty),
                    "min" | "max" => {
                        parameter_set(argument_type)
                            && integer(ty)
                            && unwrap(argument).kind() == NodeKind::CallExpression
                            && self.source.core(file, unwrap(argument), view, "index_set")
                            && unwrap(argument)
                                .child_nodes()
                                .next()
                                .and_then(bare)
                                .is_some_and(|id| {
                                    visible.contains(&id)
                                        && self.source.bindings.declarations[id.0].role
                                            == DeclarationRole::Parameter
                                        && self.source.array_nonempty_branch(file, node, id, view)
                                })
                    }
                    _ => false,
                };
                let CallOutcome::Resolved {
                    declaration,
                    parameters,
                    return_type,
                } = &self
                    .source
                    .operation_fact(view, file, node)
                    .ok_or("selected union source view selection is unavailable")?
                    .outcome
                else {
                    return Err("selected union source view selection is unsupported".into());
                };
                if !shape
                    || argument.kind() == NodeKind::NamedArgument
                    || parameters.len() != 1
                    || return_type != ty
                    || !parameters[0].known()
                    || optional(&parameters[0])
                    || !crate::types::coerces(argument_type, &parameters[0])
                {
                    return Err(
                        "selected union source view tuple or nonempty prerequisite is unsupported"
                            .into(),
                    );
                }
                self.source.selected_union_primitive_safety(*declaration)?;
                return self.selected_union_body_source_safety(
                    (file, argument, view),
                    visible,
                    active,
                    location,
                );
            }
        }
        if matches!(
            node.kind(),
            NodeKind::BinaryExpression | NodeKind::RangeExpression
        ) {
            let parts: Vec<_> = node.child_nodes().collect();
            let [left, right] = parts.as_slice() else {
                return Err("selected union source operator arity is unsupported".into());
            };
            let name = operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                .ok_or("selected union source operator is unavailable")?;
            let operand_types = [typed(left), typed(right)];
            let shape = match name {
                "union" => decision_integer_set(ty)
                    && operand_types.iter().all(|t| *t == Some(ty)),
                "=" => ty.kind == TypeKind::Bool
                    && (operand_types.iter().all(|t| t.is_some_and(integer))
                        || operand_types.iter().all(|t| t.is_some_and(|t|
                            decision_integer_set(t) || parameter_set(t)
                                || matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Bottom)))),
                "/\\" => ty.kind == TypeKind::Bool
                    && operand_types.iter().all(|t| t.is_some_and(|t|
                        t.known() && !optional(t) && t.kind == TypeKind::Bool)),
                "+" | "-" => integer(ty)
                    && operand_types.iter().all(|t| t.is_some_and(integer)),
                ".." => parameter_set(ty)
                    && operand_types.iter().all(|t| t.is_some_and(integer)),
                _ => false,
            };
            let call = self
                .source
                .operation_fact(view, file, node)
                .ok_or("selected union source operator selection is unavailable")?;
            let CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &call.outcome
            else {
                return Err("selected union source operator selection is unsupported".into());
            };
            if !shape
                || !self.source.core(file, node, view, name)
                || parameters.len() != 2
                || return_type != ty
                || !parameters
                    .iter()
                    .zip(operand_types)
                    .all(|(formal, actual)| {
                        formal.known()
                            && !optional(formal)
                            && actual.is_some_and(|actual| crate::types::coerces(actual, formal))
                    })
            {
                return Err("selected union source operator identity/type is unsupported".into());
            }
            self.source.selected_union_primitive_safety(*declaration)?;
            for child in parts {
                self.selected_union_body_source_safety(
                    (file, child, view),
                    visible,
                    active,
                    location,
                )?;
            }
            return Ok(());
        }
        if node.kind() == NodeKind::SetLiteral {
            let empty_type = ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if element.known() && !optional(element)
                        && element.instantiation == Instantiation::Parameter
                        && element.kind == TypeKind::Bottom);
            let cells: Vec<_> = node.child_nodes().collect();
            if !parameter_set(ty) && !(empty_type && cells.is_empty())
                || cells
                    .iter()
                    .any(|cell| typed(cell).is_none_or(|t| !integer(t)))
            {
                return Err("selected union integer set literal source is unsupported".into());
            }
            for cell in cells {
                self.selected_union_body_source_safety(
                    (file, cell, view),
                    visible,
                    active,
                    location,
                )?;
            }
            return Ok(());
        }
        // Only childless literals reuse ordinary source inspection. A literal
        // container cannot hide a selected helper or operator's written body.
        if node.kind() == NodeKind::Expression
            && node.child_nodes().next().is_none()
            && self.source.bindings.references.iter().all(|reference| {
                reference.file != file
                    || reference.kind != ReferenceKind::Value
                    || reference.location.range.start
                        < self.source.context.files[file]
                            .location(node.range())
                            .range
                            .start
                    || self.source.context.files[file]
                        .location(node.range())
                        .range
                        .end
                        < reference.location.range.end
            })
        {
            return match self.initialized_source_safety(file, node, view, &[], active) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        Err("selected union local written source form is unsupported".into())
    }
    pub(in crate::callable_definitions) fn selected_union_predicate_body_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        active: &mut Vec<DeclarationId>,
    ) -> Result<(), UnavailableCallableDefinition> {
        let mut location = self.source.context.files[file].location(node.range());
        let mut entered = false;
        let checked = (|| -> Result<(), String> {
            let CallOutcome::Resolved {
                declaration: id,
                parameters,
                return_type,
            } = &self
                .source
                .operation_fact(view, file, node)
                .ok_or("selected union predicate selection is unavailable")?
                .outcome
            else {
                return Err("selected union predicate selection is unsupported".into());
            };
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            let arguments: Vec<_> = node.child_nodes().collect();
            if !self.source.selected_union_predicate_identity(*id)
                || parameters.len() != 2
                || arguments.len() != 2
                || !matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                    if parameters[0].known() && !optional(&parameters[0])
                        && parameters[0].instantiation == Instantiation::Decision
                        && indices.len() == 1 && integer(&indices[0])
                        && decision_integer_set(element) && element.as_ref() == &parameters[1])
                || !decision_integer_set(&parameters[1])
                || !return_type.known()
                || optional(return_type)
                || return_type.instantiation != Instantiation::Decision
                || return_type.kind != TypeKind::Bool
                || self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|t| t.ty != *return_type)
                || arguments.iter().zip(parameters).any(|(argument, formal)| {
                    self.source
                        .expression_type(view, file, argument)
                        .is_none_or(|t| {
                            !t.ty.known()
                                || optional(&t.ty)
                                || !crate::types::coerces(&t.ty, formal)
                        })
                })
            {
                return Err("selected union predicate concrete tuple is unsupported".into());
            }
            let owner = &self.source.bindings.declarations[id.0];
            let written = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("selected union predicate declaration is unavailable")?;
            let body = written
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .last()
                .ok_or("selected union predicate real body is unavailable")?;
            location = self.source.context.files[owner.file].location(body.range());
            if owner.role != DeclarationRole::Predicate
                || !self.source.callable_annotations_safe(owner.file, written)
                || active.contains(id)
            {
                return Err(
                    "selected union predicate body/annotation or recurrence is unsupported".into(),
                );
            }
            active.push(*id);
            entered = true;
            let body_view = instantiated_body(
                self.source.context,
                self.source.bindings,
                self.source.calls,
                *id,
                parameters,
            );
            let mut formals = Vec::new();
            let list = written
                .child_nodes()
                .find(|node| node.kind() == NodeKind::ParameterList)
                .ok_or("selected union predicate written parameters are unavailable")?;
            if list.child_nodes().count() != parameters.len() {
                return Err(
                    "selected union predicate written parameter count is unsupported".into(),
                );
            }
            for (position, parameter_type) in parameters.iter().enumerate() {
                let formal =
                    formal_parameter(self.source.context, self.source.bindings, *id, position)
                        .ok_or("selected union predicate formal is unavailable")?;
                let written_formal = find_node(
                    self.source.context.files[owner.file].parsed.tree(),
                    &self.source.bindings.declarations[formal.0].syntax_range,
                    DeclarationRole::Parameter,
                )
                .ok_or("selected union predicate written formal is unavailable")?;
                if written_formal
                    .child_nodes()
                    .any(|node| is_expression(node.kind()))
                    || body_view.declarations[formal.0].ty != *parameter_type
                {
                    return Err("selected union predicate formal/default is unsupported".into());
                }
                self.type_dependencies(owner.file, written_formal, &body_view, &[])?;
                let mut nodes = vec![written_formal];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, owner.file, node)
                    {
                        return Err(
                            "selected union predicate formal annotation is unsupported".into()
                        );
                    }
                    nodes.extend(node.child_nodes());
                }
                if let Some(reason) =
                    self.source
                        .closed_integer_source_error(owner.file, written_formal, true, true)
                {
                    return Err(reason);
                }
                formals.push(formal);
            }
            let value = self
                .source
                .expression_type(&body_view, owner.file, body)
                .ok_or("selected union predicate body type is unavailable")?;
            if !value.ty.known()
                || optional(&value.ty)
                || !crate::types::coerces(&value.ty, return_type)
            {
                return Err("selected union predicate body type is unsupported".into());
            }
            // This exact selected-body scan precedes inspectors that start fresh
            // vectors, including written recurrence nested under scalar aggregates.
            let mut nodes = vec![body];
            while let Some(node) = nodes.pop() {
                if let Some(CallOutcome::Resolved { declaration, .. }) = self
                    .source
                    .operation_fact(&body_view, owner.file, node)
                    .map(|call| &call.outcome)
                    && active.contains(declaration)
                    && (crate::definitions::core_callable(
                        self.source.context,
                        self.source.bindings,
                        *declaration,
                        "array_union",
                    ) || self.source.selected_union_predicate_identity(*declaration))
                {
                    return Err("recursive selected union predicate body is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            if let Some(arguments) =
                self.source
                    .assertion_arguments(owner.file, unwrap(body), &body_view)
                && arguments.len() == 3
                && self
                    .source
                    .assertion_literal(arguments[0].0, arguments[0].1)
                    == Some(TokenKind::False)
            {
                let CallOutcome::Resolved {
                    parameters,
                    return_type,
                    ..
                } = &self
                    .source
                    .operation_fact(&body_view, owner.file, unwrap(body))
                    .ok_or("selected union predicate assertion selection is unavailable")?
                    .outcome
                else {
                    return Err(
                        "selected union predicate assertion selection is unsupported".into(),
                    );
                };
                if parameters.len() != 3
                    || *return_type != value.ty
                    || parameters[2] != *return_type
                    || parameters.iter().any(|t| !t.known() || optional(t))
                {
                    return Err("selected union predicate assertion tuple is unsupported".into());
                }
                for (position, (file, argument)) in arguments.iter().enumerate() {
                    let actual = self
                        .source
                        .expression_type(&body_view, *file, argument)
                        .ok_or("selected union predicate assertion actual type is unavailable")?;
                    if !actual.ty.known()
                        || optional(&actual.ty)
                        || !crate::types::coerces(&actual.ty, &parameters[position])
                        || match position {
                            0 => {
                                actual.ty.instantiation != Instantiation::Parameter
                                    || actual.ty.kind != TypeKind::Bool
                            }
                            1 => {
                                actual.ty.instantiation != Instantiation::Parameter
                                    || actual.ty.kind != TypeKind::String
                            }
                            _ => !crate::types::coerces(&actual.ty, &value.ty),
                        }
                    {
                        return Err(
                            "selected union predicate assertion actual type is unsupported".into(),
                        );
                    }
                    self.selected_union_body_source_safety(
                        (*file, argument, &body_view),
                        &formals,
                        active,
                        &mut location,
                    )?;
                }
                return Err("false assertion condition aborts evaluation".into());
            }
            self.selected_union_body_source_safety(
                (owner.file, body, &body_view),
                &formals,
                active,
                &mut location,
            )
        })();
        if entered {
            active.pop();
        }
        checked.map_err(|reason| UnavailableCallableDefinition {
            location,
            targets: Vec::new(),
            reason,
        })
    }
    // This selected value body supplies source safety only. Neither its local
    // bound nor set_card proves the actual set's cardinality or any output.
    pub(in crate::callable_definitions) fn decision_enum_card_body_safety(
        &self,
        id: DeclarationId,
        parameters: &[TypeInst],
        result: &TypeInst,
    ) -> Result<(), String> {
        let error = || "decision enum card selected body is unsupported".to_owned();
        let owner = &self.source.bindings.declarations[id.0];
        let written = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or_else(error)?;
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err(error());
        };
        // Inspect closed errors before a changed shape is refused. Both branches
        // of this unknown bound must be defined; the condition proves no truth.
        if let Some(reason) = self
            .source
            .closed_integer_source_error(owner.file, written, false, true)
        {
            return Err(reason);
        }
        if owner.role != DeclarationRole::Function
            || !self.source.callable_annotations_safe(owner.file, written)
            || written
                .child_nodes()
                .filter(|n| !is_expression(n.kind()) && n.kind() != NodeKind::Annotation)
                .any(|n| !self.source.prefix_primitive_source_safe(owner.file, n))
        {
            return Err(error());
        }
        let formal =
            formal_parameter(self.source.context, self.source.bindings, id, 0).ok_or_else(error)?;
        let lists: Vec<_> = written
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::ParameterList)
            .collect();
        if !matches!(lists.as_slice(), [list] if list.child_nodes().count() == 1)
            || parameters.len() != 1
        {
            return Err(error());
        }
        let view = instantiated_body(
            self.source.context,
            self.source.bindings,
            self.source.calls,
            id,
            parameters,
        );
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool);
        let decision_integer = integer.clone().with_inst(Instantiation::Decision);
        let decision_boolean = boolean.clone().with_inst(Instantiation::Decision);
        let integer_set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let decision_integer_set = integer_set.clone().with_inst(Instantiation::Decision);
        let parameter_set = parameters[0].clone().with_inst(Instantiation::Parameter);
        let typed = |node: &SyntaxNode| {
            self.source
                .expression_type(&view, owner.file, node)
                .map(|e| &e.ty)
        };
        let reference = |node: &SyntaxNode| {
            let node = unwrap(node);
            (node.kind() == NodeKind::Expression)
                .then(|| self.source.reference(owner.file, node))
                .flatten()
        };
        if result != &decision_integer
            || view.declarations[formal.0].ty != parameters[0]
            || body.kind() != NodeKind::LetExpression
            || typed(body) != Some(result)
        {
            return Err(error());
        }
        self.type_dependencies(owner.file, written, &view, &[])?;
        let formal_node = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &self.source.bindings.declarations[formal.0].syntax_range,
            DeclarationRole::Parameter,
        )
        .ok_or_else(error)?;
        self.type_dependencies(owner.file, formal_node, &view, &[])?;
        let mut nodes = vec![*body];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Error || !self.source.source_annotations_safe(owner.file, node)
                || self.source.operation_fact(&view, owner.file, node).is_some_and(|call|
                    matches!(call.outcome, CallOutcome::Resolved { declaration, .. } if declaration == id))
            {
                return Err(error());
            }
            nodes.extend(node.child_nodes());
        }
        let parts: Vec<_> = body.child_nodes().collect();
        let [block, returned] = parts.as_slice() else {
            return Err(error());
        };
        let locals: Vec<_> = block.child_nodes().collect();
        let [local, constraint] = locals.as_slice() else {
            return Err(error());
        };
        if block.kind() != NodeKind::LetBlock
            || local.kind() != NodeKind::Declaration
            || constraint.kind() != NodeKind::Constraint
        {
            return Err(error());
        }
        let local_id = self
            .source
            .bindings
            .declarations
            .iter()
            .find(|d| {
                d.file == owner.file
                    && d.role == DeclarationRole::Local
                    && d.syntax_range == local.range()
            })
            .map(|d| d.id)
            .ok_or_else(error)?;
        if local_id == formal
            || view.declarations[local_id.0].ty != decision_integer
            || reference(returned) != Some(local_id)
            || typed(returned) != Some(result)
            || local.child_nodes().count() != 1
        {
            return Err(error());
        }
        let domain = local.child_nodes().next().ok_or_else(error)?;
        let ranges: Vec<_> = domain.child_nodes().collect();
        let [range] = ranges.as_slice() else {
            return Err(error());
        };
        let range = unwrap(range);
        let ends: Vec<_> = range.child_nodes().collect();
        let [zero, upper] = ends.as_slice() else {
            return Err(error());
        };
        if domain.kind() != NodeKind::DomainType
            || range.kind() != NodeKind::RangeExpression
            || !self.source.prefix_primitive(
                owner.file,
                range,
                &view,
                "..",
                &[integer.clone(), integer.clone()],
                &integer_set,
            )
            || !matches!(crate::domains::tokens(&self.source.context.files[owner.file].parsed, unwrap(zero)).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
            || !crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                owner.file,
                zero,
            )
            .is_ok_and(|value| value == Some(0))
        {
            return Err(error());
        }
        let upper = unwrap(upper);
        let branches: Vec<_> = upper.child_nodes().collect();
        let [then, otherwise] = branches.as_slice() else {
            return Err(error());
        };
        let first: Vec<_> = then.child_nodes().collect();
        let second: Vec<_> = otherwise.child_nodes().collect();
        let ([guard, count], [unbounded]) = (first.as_slice(), second.as_slice()) else {
            return Err(error());
        };
        if upper.kind() != NodeKind::ConditionalExpression
            || typed(upper) != Some(&integer)
            || then.kind() != NodeKind::ConditionalBranch
            || otherwise.kind() != NodeKind::ElseBranch
            || !matches!(crate::domains::tokens(&self.source.context.files[owner.file].parsed, unwrap(unbounded)).as_slice(),
                [token] if token.kind == TokenKind::Infinity)
            || typed(unbounded) != Some(&integer)
        {
            return Err(error());
        }
        let guard = unwrap(guard);
        let count = unwrap(count);
        let bounds: Vec<_> = count.child_nodes().collect();
        let [bound] = bounds.as_slice() else {
            return Err(error());
        };
        let bound = unwrap(bound);
        for (node, name, expected, output) in [
            (guard, "has_ub_set", &parameters[0], &boolean),
            (bound, "ub", &parameters[0], &parameter_set),
            (count, "card", &parameter_set, &integer),
        ] {
            if node.kind() != NodeKind::CallExpression
                || !self.source.prefix_primitive(
                    owner.file,
                    node,
                    &view,
                    name,
                    std::slice::from_ref(expected),
                    output,
                )
                || node.child_nodes().count() != 1
            {
                return Err(error());
            }
        }
        if guard.child_nodes().next().and_then(reference) != Some(formal)
            || bound.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err(error());
        }
        let constraints: Vec<_> = constraint.child_nodes().collect();
        let [cardinality] = constraints.as_slice() else {
            return Err(error());
        };
        let cardinality = unwrap(cardinality);
        let arguments: Vec<_> = cardinality.child_nodes().collect();
        let [convert, actual_count] = arguments.as_slice() else {
            return Err(error());
        };
        let convert = unwrap(convert);
        if convert.kind() != NodeKind::CallExpression
            || !self.source.prefix_primitive(
                owner.file,
                convert,
                &view,
                "enum2int",
                parameters,
                &decision_integer_set,
            )
            || convert.child_nodes().next().and_then(reference) != Some(formal)
            || reference(actual_count) != Some(local_id)
            || typed(actual_count) != Some(&decision_integer)
        {
            return Err(error());
        }
        let call = self
            .source
            .operation_fact(&view, owner.file, cardinality)
            .ok_or_else(error)?;
        let CallOutcome::Resolved {
            declaration,
            parameters: selected,
            return_type,
        } = &call.outcome
        else {
            return Err(error());
        };
        let primitive = &self.source.bindings.declarations[declaration.0];
        let primitive_source = &self.source.context.files[primitive.file];
        if cardinality.kind() != NodeKind::CallExpression
            || call.generator_argument.is_some()
            || primitive.name != "set_card"
            || primitive.role != DeclarationRole::Predicate
            || primitive_source.kind != SourceKind::StandardLibrary
            || !primitive_source.implicit
            || selected.as_slice() != [decision_integer_set, decision_integer]
            || return_type != &decision_boolean
            || typed(cardinality) != Some(return_type)
        {
            return Err(error());
        }
        let primitive_node = find_node(
            primitive_source.parsed.tree(),
            &primitive.syntax_range,
            primitive.role,
        )
        .ok_or_else(error)?;
        let primitive_lists: Vec<_> = primitive_node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::ParameterList)
            .collect();
        if !matches!(primitive_lists.as_slice(), [list] if list.child_nodes().count() == 2)
            || !self
                .source
                .prefix_primitive_source_safe(primitive.file, primitive_node)
        {
            return Err(error());
        }
        Ok(())
    }
}
