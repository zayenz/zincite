use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn integer_conversion_fallback(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<&'a SyntaxNode, String> {
        let parts: Vec<_> = node.child_nodes().map(unwrap).collect();
        let ty = |node: &SyntaxNode| {
            self.view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range == self.context.files[file].location(node.range()).range
                })
                .map(|e| &e.ty)
        };
        let present_int =
            |node| ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int);
        if parts.len() != 2 || parts.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || !self.core(file, node, view, "to_enum_internal")
            || !present_int(node) || ty(node).is_none_or(|t| t.instantiation != Instantiation::Parameter)
            || !present_int(parts[1]) || ty(parts[1]).is_none_or(|t| t.instantiation != Instantiation::Parameter)
            || parts[0].kind() != NodeKind::CallExpression
            || !self.core(file, parts[0], view, "enum_of")
            || ty(parts[0]).is_none_or(|t| !t.known() || optional(t) || t.instantiation != Instantiation::Parameter
                || !matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element) && element.kind == TypeKind::Int))
        {
            return Err("integer internal conversion requires a checked present Int composition".into());
        }
        let witness: Vec<_> = parts[0].child_nodes().map(unwrap).collect();
        if witness.len() != 1 || witness[0].kind() == NodeKind::NamedArgument || !present_int(witness[0])
            || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(|t| t.known() && !optional(t))
                        && matches!(&parameters[0].kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
                        && parameters[1].kind == TypeKind::Int && parameters[1].instantiation == Instantiation::Parameter
                        && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Int)
            })
            || self.operation_fact(self.view(file, parts[0], view), file, parts[0]).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && parameters[0].kind == TypeKind::Int)
            })
        {
            return Err("integer conversion witness or selected signature is unsupported".into());
        }
        // MiniZinc erases this entire Int composition to its second argument.
        // The witness is checked for type selection; only the fallback is evaluated.
        Ok(parts[1])
    }
    // Inspect disjoint closed arithmetic fragments and initialized integer
    // sources. Collection inspection follows initialized sets and arrays as well.
    // Existing evaluation supplies an error veto, never a value fact.
    pub(in crate::callable_definitions) fn closed_integer_source_error(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        eager_only: bool,
        follow_collections: bool,
    ) -> Option<String> {
        self.closed_integer_source_error_with_branches(
            file,
            node,
            eager_only,
            follow_collections,
            false,
        )
    }
    pub(in crate::callable_definitions) fn closed_integer_source_error_with_branches(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        eager_only: bool,
        follow_collections: bool,
        type_branches: bool,
    ) -> Option<String> {
        // Both scans must omit the same literal-inactive body, including its references.
        fn source_children<'b>(
            inspector: &SourceInspector<'_>,
            file: FileId,
            node: &'b SyntaxNode,
            type_branches: bool,
        ) -> Vec<&'b SyntaxNode> {
            let children: Vec<_> = node.child_nodes().collect();
            if !type_branches || node.kind() != NodeKind::ConditionalExpression {
                return children;
            }
            let [then, otherwise] = children.as_slice() else {
                return children;
            };
            let first: Vec<_> = then.child_nodes().collect();
            let last: Vec<_> = otherwise.child_nodes().collect();
            let ([guard, body], [fallback]) = (first.as_slice(), last.as_slice()) else {
                return children;
            };
            let parsed = &inspector.context.files[file].parsed;
            if then.kind() != NodeKind::ConditionalBranch
                || otherwise.kind() != NodeKind::ElseBranch
                || guard.kind() != NodeKind::Expression
                || ![node, *then, *otherwise, *guard].iter().all(|source| {
                    crate::definitions::annotations_safe(inspector.context, file, source)
                })
                || !matches!(crate::domains::tokens(parsed, guard).as_slice(),
                    [token] if matches!(token.kind, TokenKind::True | TokenKind::False))
                || inspector
                    .expression_type(inspector.view(file, guard, inspector.calls), file, guard)
                    .is_none_or(|expression| {
                        expression.ty
                            != TypeInst {
                                instantiation: Instantiation::Parameter,
                                optional: false,
                                kind: TypeKind::Bool,
                            }
                    })
            {
                return children;
            }
            match inspector.assertion_literal(file, guard) {
                Some(TokenKind::True) => vec![*guard, *body],
                Some(TokenKind::False) => vec![*guard, *fallback],
                _ => children,
            }
        }
        fn literal_parts(
            inspector: &SourceInspector<'_>,
            file: FileId,
            node: &SyntaxNode,
            eager_only: bool,
            type_branches: bool,
        ) -> Result<bool, String> {
            let context = inspector.context;
            let bindings = inspector.bindings;
            if eager_only
                && node.kind() == NodeKind::ConditionalExpression
                && inspector.selected_set_source
            {
                let branches: Vec<_> = node.child_nodes().collect();
                if let [then, otherwise] = branches.as_slice() {
                    let first: Vec<_> = then.child_nodes().collect();
                    let last: Vec<_> = otherwise.child_nodes().collect();
                    if then.kind() == NodeKind::ConditionalBranch
                        && otherwise.kind() == NodeKind::ElseBranch
                        && let ([guard, body], [fallback]) = (first.as_slice(), last.as_slice())
                    {
                        let selected = inspector.assertion_literal(file, guard);
                        literal_parts(inspector, file, guard, eager_only, type_branches)?;
                        for value in [
                            (*body, selected != Some(TokenKind::False)),
                            (*fallback, selected != Some(TokenKind::True)),
                        ] {
                            if value.1
                                && literal_parts(
                                    inspector,
                                    file,
                                    value.0,
                                    eager_only,
                                    type_branches,
                                )?
                            {
                                crate::domains::invariant_expression_integer(
                                    context, bindings, file, value.0,
                                )?;
                            }
                        }
                        return Ok(false);
                    }
                }
            }
            let inspected_conjunction = eager_only
                && inspector.selected_set_source
                && node.kind() == NodeKind::BinaryExpression
                && inspector.core(file, node, inspector.calls, "/\\")
                && {
                    let facts = inspector.view(file, node, inspector.calls);
                    let parameter_bool = |ty: &TypeInst| {
                        ty.known()
                            && !optional(ty)
                            && ty.instantiation == Instantiation::Parameter
                            && ty.kind == TypeKind::Bool
                    };
                    let operands: Vec<_> = node.child_nodes().collect();
                    operands.len() == 2
                        && operands.iter().all(|operand| {
                            inspector
                                .expression_type(facts, file, operand)
                                .is_some_and(|expression| parameter_bool(&expression.ty))
                        })
                        && inspector.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2 && parameters.iter().all(parameter_bool)
                                    && parameter_bool(return_type)
                                    && inspector.expression_type(facts, file, node)
                                        .is_some_and(|expression| &expression.ty == return_type))
                        })
                };
            if eager_only
                && (node.kind() == NodeKind::ConditionalExpression
                    || node.kind() == NodeKind::BinaryExpression
                        && matches!(
                            operator(context, file, node)
                                .and_then(crate::bindings::symbolic_operator),
                            Some("/\\" | "\\/" | "<->" | "->" | "<-" | "default")
                        )
                        && !inspected_conjunction)
            {
                return Err("lazy initialized arithmetic inspection is unavailable".into());
            }
            let mut children = Vec::new();
            for child in source_children(inspector, file, node, type_branches) {
                children.push((
                    child,
                    literal_parts(inspector, file, child, eager_only, type_branches)?,
                ));
            }
            let literal = match node.kind() {
                NodeKind::Expression => {
                    matches!(crate::domains::tokens(&context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                }
                NodeKind::ParenthesizedExpression => children.len() == 1 && children[0].1,
                NodeKind::UnaryExpression => {
                    children.len() == 1
                        && children[0].1
                        && matches!(
                            operator(context, file, node),
                            Some(TokenKind::Plus | TokenKind::Minus)
                        )
                }
                NodeKind::BinaryExpression => {
                    children.len() == 2
                        && children.iter().all(|(_, literal)| *literal)
                        && matches!(
                            operator(context, file, node),
                            Some(
                                TokenKind::Plus
                                    | TokenKind::Minus
                                    | TokenKind::Star
                                    | TokenKind::Div
                                    | TokenKind::Mod
                            )
                        )
                }
                _ => false,
            };
            if literal
                && node.kind() == NodeKind::BinaryExpression
                && matches!(
                    operator(context, file, node),
                    Some(TokenKind::Div | TokenKind::Mod)
                )
                && crate::domains::invariant_expression_integer(
                    context,
                    bindings,
                    file,
                    children[1].0,
                )? == Some(0)
            {
                return Err("integer division by zero".into());
            }
            if !literal {
                for (child, literal) in children {
                    if literal {
                        crate::domains::invariant_expression_integer(
                            context, bindings, file, child,
                        )?;
                    }
                }
            }
            Ok(literal)
        }
        let mut sources = Vec::new();
        let mut pending = vec![(file, node)];
        while let Some((file, root)) = pending.pop() {
            match literal_parts(self, file, root, eager_only, type_branches) {
                Ok(true) => {
                    if let Err(reason) = crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        root,
                    ) {
                        return Some(reason);
                    }
                }
                Err(reason) => return Some(reason),
                Ok(false) => {}
            }
            let mut nodes = vec![root];
            while let Some(node) = nodes.pop() {
                nodes.extend(source_children(self, file, node, type_branches));
                if self.selected_set_source && node.kind() == NodeKind::DomainType {
                    match literal_parts(self, file, node, true, type_branches) {
                        Ok(true) => {
                            if let Err(reason) = crate::domains::invariant_expression_integer(
                                self.context,
                                self.bindings,
                                file,
                                node,
                            ) {
                                return Some(reason);
                            }
                        }
                        Err(reason) => return Some(reason),
                        Ok(false) => {}
                    }
                }
                if node.kind() == NodeKind::Expression
                    && let Some(id) = self.reference(file, node)
                    && !sources.contains(&id)
                {
                    let declaration = &self.bindings.declarations[id.0];
                    let ty = &self.calls.declarations[id.0].ty;
                    if declaration.top_level
                        && declaration.role == DeclarationRole::Value
                        && ty.known()
                        && !optional(ty)
                        && ty.instantiation == Instantiation::Parameter
                        && (ty.kind == TypeKind::Int
                            || follow_collections
                                && matches!(ty.kind, TypeKind::Set(_) | TypeKind::Array { .. }))
                        && let Some(written) = find_node(
                            self.context.files[declaration.file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                    {
                        sources.push(id);
                        pending.push((declaration.file, written));
                    }
                }
            }
        }
        None
    }
    // Written closed enum extent is used only to reject undefined operations.
    pub(in crate::callable_definitions) fn enum_extent(&self, id: DeclarationId) -> Option<i64> {
        let declaration = &self.bindings.declarations[id.0];
        if declaration.role != DeclarationRole::Enum {
            return None;
        }
        let written = find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        let definition = written
            .child_nodes()
            .find(|n| n.kind() == NodeKind::EnumDefinition)?;
        let parts: Vec<_> = definition.child_nodes().collect();
        let [part] = parts.as_slice() else {
            return None;
        };
        if part.kind() == NodeKind::EnumCases
            && part
                .child_nodes()
                .all(|n| n.kind() == NodeKind::EnumCase && n.child_nodes().next().is_none())
        {
            return i64::try_from(part.child_nodes().count()).ok();
        }
        if part.kind() != NodeKind::EnumConstructor {
            return None;
        }
        let tokens = crate::domains::tokens(&self.context.files[declaration.file].parsed, part);
        let name = tokens.first().map(|t| {
            self.context.files[declaration.file].parsed.source()[t.range.clone()].trim_matches('\'')
        });
        if name != Some("anon_enum") {
            return None;
        }
        let children: Vec<_> = part.child_nodes().collect();
        let [count] = children.as_slice() else {
            return None;
        };
        crate::domains::invariant_expression_integer(
            self.context,
            self.bindings,
            declaration.file,
            unwrap(count),
        )
        .ok()
        .flatten()
    }
    pub(in crate::callable_definitions) fn nonempty_conditional_array_source(
        &self,
        file: FileId,
        written: &SyntaxNode,
    ) -> bool {
        let node = unwrap(written);
        match node.kind() {
            NodeKind::ArrayLiteral => {
                node.child_nodes().next().is_some()
                    && node.child_nodes().all(|value| {
                        is_expression(value.kind()) && value.kind() != NodeKind::IndexedArrayEntry
                    })
            }
            NodeKind::ArrayComprehension => {
                let parts: Vec<_> = node.child_nodes().collect();
                let Some(list) = parts
                    .iter()
                    .find(|part| part.kind() == NodeKind::GeneratorList)
                else {
                    return false;
                };
                let generators: Vec<_> = list.child_nodes().collect();
                let [generator] = generators.as_slice() else {
                    return false;
                };
                parts.len() == 2
                    && crate::domains::generator_slots(&self.context.files[file].parsed, generator)
                        == 1
                    && generator.child_nodes().count() == 1
                    && generator
                        .child_nodes()
                        .next()
                        .is_some_and(|source| unwrap(source).kind() == NodeKind::RangeExpression)
                    && self.nonempty_source(file, generator)
            }
            NodeKind::ConditionalExpression => {
                let branches: Vec<_> = node.child_nodes().collect();
                branches.len() >= 2
                    && branches.iter().enumerate().all(|(position, branch)| {
                        let parts: Vec<_> = branch.child_nodes().collect();
                        if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                            self.nonempty_conditional_array_source(file, parts[1])
                        } else {
                            branch.kind() == NodeKind::ElseBranch
                                && parts.len() == 1
                                && position + 1 == branches.len()
                                && self.nonempty_conditional_array_source(file, parts[0])
                        }
                    })
            }
            _ => false,
        }
    }
    pub(in crate::callable_definitions) fn parameter_set_source_empty(
        &self,
        file: FileId,
        source: &SyntaxNode,
    ) -> bool {
        let written_domain = expression_domain(self.context, self.bindings, file, source);
        let mut domain = &written_domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        match domain {
            Domain::LiteralSet(values) => values.is_empty(),
            Domain::Range { lower, upper } => {
                matches!((crate::domains::invariant_integer(lower), crate::domains::invariant_integer(upper)),
                (Ok(Some(lower)), Ok(Some(upper))) if lower > upper)
            }
            Domain::Enum(id) => self.enum_extent(*id).is_some_and(|size| size == 0),
            _ => false,
        }
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn native_fixed_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !self.source.core(file, node, view, "is_fixed")
        {
            return None;
        }
        let arguments: Vec<_> = node.child_nodes().collect();
        let [argument] = arguments.as_slice() else {
            return None;
        };
        let decision = TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision);
        let result = TypeInst::par(TypeKind::Bool);
        let facts = self.source.view(file, node, view);
        if self
            .source
            .expression_type(facts, file, argument)
            .is_none_or(|e| e.ty != decision)
            || self
                .source
                .expression_type(facts, file, node)
                .is_none_or(|e| e.ty != result)
        {
            return None;
        }
        let mut nodes = vec![written];
        while let Some(source) = nodes.pop() {
            if source.kind() == NodeKind::Error
                || !self.source.source_annotations_safe(file, source)
            {
                return Some(DefinitionSafety::Unsupported(
                    "fixedness source annotation or syntax is unsupported".into(),
                ));
            }
            nodes.extend(source.child_nodes());
        }
        if !self
            .source
            .prefix_primitive(file, node, facts, "is_fixed", &[decision], &result)
        {
            return Some(DefinitionSafety::Unsupported(
                "fixedness selected written primitive or tuple is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_source_safety(file, argument, view, generators, &mut Vec::new())
        {
            return Some(unsupported);
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, argument, true, true)
        {
            return Some(DefinitionSafety::Unsupported(reason));
        }
        Some(DefinitionSafety::Unknown(
            "decision fixedness is unproved".into(),
        ))
    }
    pub(in crate::callable_definitions) fn native_dom_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.source.core(file, node, view, "dom") {
            return None;
        }
        let arguments: Vec<_> = node.child_nodes().collect();
        let [argument] = arguments.as_slice() else {
            return None;
        };
        if argument.kind() == NodeKind::NamedArgument
            || unwrap(argument).kind() != NodeKind::ArrayAccessExpression
        {
            return None;
        }
        let decision = TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision);
        let result = TypeInst::par(TypeKind::Set(Box::new(TypeInst::par(TypeKind::Int))));
        let facts = self.source.view(file, node, view);
        if self
            .source
            .expression_type(facts, file, argument)
            .is_none_or(|e| e.ty != decision)
            || self
                .source
                .expression_type(facts, file, node)
                .is_none_or(|e| e.ty != result)
        {
            return None;
        }
        let mut nodes = vec![written];
        while let Some(source) = nodes.pop() {
            if source.kind() == NodeKind::Error
                || !self.source.source_annotations_safe(file, source)
            {
                return Some(DefinitionSafety::Unsupported(
                    "domain reflection source annotation or syntax is unsupported".into(),
                ));
            }
            nodes.extend(source.child_nodes());
        }
        if !self
            .source
            .prefix_primitive(file, node, facts, "dom", &[decision], &result)
        {
            return Some(DefinitionSafety::Unsupported(
                "domain reflection selected written primitive or tuple is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.decision_scalar_bound_operand_safety(file, argument, view, generators)
        {
            return Some(unsupported);
        }
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, argument, true, true)
        {
            return Some(DefinitionSafety::Unsupported(reason));
        }
        Some(DefinitionSafety::Unknown(
            "reflected domain members, nonemptiness and values are unproved".into(),
        ))
    }
    pub(in crate::callable_definitions) fn native_generator_max_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::GeneratorCallExpression
            || !self.source.core(file, node, view, "max")
        {
            return None;
        }
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(integer.clone()),
        });
        let facts = self.source.view(file, node, view);
        if self
            .source
            .expression_type(facts, file, node)
            .is_none_or(|e| e.ty != integer)
            || self
                .source
                .operation_fact(facts, file, node)
                .is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.as_slice() == std::slice::from_ref(&array)
                        && return_type == &integer)
                })
        {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let parts: Vec<_> = node.child_nodes().collect();
            let [list, body] = parts.as_slice() else {
                return Err("native max generator body or headers are unsupported".into());
            };
            if list.kind() != NodeKind::GeneratorList
                || list.child_nodes().count() == 0
                || self
                    .source
                    .expression_type(self.source.view(file, body, view), file, body)
                    .is_none_or(|e| e.ty != integer)
            {
                return Err("native max generator body or headers are unsupported".into());
            }
            let mut nodes = vec![written];
            while let Some(source) = nodes.pop() {
                if source.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, source)
                {
                    return Err("native max source annotation or syntax is unsupported".into());
                }
                nodes.extend(source.child_nodes());
            }
            if !self.source.prefix_primitive(
                file,
                node,
                facts,
                "max",
                std::slice::from_ref(&array),
                &integer,
            ) {
                return Err("native max selected written primitive or tuple is unsupported".into());
            }
            let headers: Vec<_> = list.child_nodes().collect();
            let mut all = generators.to_vec();
            all.extend(headers.iter().copied());
            self.integer_source_headers_safety(file, view, &all)?;
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, body, true, true)
            {
                return Err(reason);
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
            {
                return Err(reason);
            }
            if headers.iter().any(|header| {
                header
                    .child_nodes()
                    .next()
                    .is_some_and(|source| self.source.parameter_set_source_empty(file, source))
                    || header.child_nodes().skip(1).any(|filter| {
                        filter.child_nodes().next().is_some_and(|condition| {
                            self.source.assertion_literal(file, condition) == Some(TokenKind::False)
                        })
                    })
            }) {
                return Err("native max is undefined for an empty iteration".into());
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("native max nonemptiness and value are unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn parameter_test_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> Option<DefinitionSafety> {
        let written_call = node;
        let node = unwrap(node);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let CallOutcome::Resolved {
            declaration: id,
            parameters,
            return_type,
        } = &self.source.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let declaration = &self.source.bindings.declarations[id.0];
        if declaration.role != DeclarationRole::Test
            || !declaration.top_level
            || self.source.context.files[declaration.file].kind != SourceKind::User
        {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let parameter = |t: &TypeInst, kind: TypeKind| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == kind
            };
            let signature = self
                .source
                .calls
                .signatures
                .iter()
                .find(|s| s.declaration == *id)
                .ok_or("parameter test signature unavailable")?;
            if active.contains(id)
                || !self.source.context.files[declaration.file]
                    .parsed
                    .diagnostics()
                    .is_empty()
                || parameters.len() != 1
                || signature.parameters.len() != 1
                || signature.parameters[0].has_default
                || !parameter(&signature.parameters[0].ty, TypeKind::Int)
                || signature.parameters[0].ty != parameters[0]
                || !parameter(&signature.return_type, TypeKind::Bool)
                || &signature.return_type != return_type
                || self
                    .source
                    .expression_type(facts, file, node)
                    .is_none_or(|e| &e.ty != return_type)
            {
                return Err(
                    "parameter test specialization, default or cycle is unsupported".into(),
                );
            }
            let written = find_node(
                self.source.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("parameter test declaration unavailable")?;
            let formal = formal_parameter(self.source.context, self.source.bindings, *id, 0)
                .ok_or("parameter test formal unavailable")?;
            let formal_declaration = &self.source.bindings.declarations[formal.0];
            let formal_node = find_node(
                self.source.context.files[formal_declaration.file]
                    .parsed
                    .tree(),
                &formal_declaration.syntax_range,
                formal_declaration.role,
            )
            .ok_or("parameter test written formal unavailable")?;
            let (actual_file, actual) = call_argument(
                self.source.context,
                self.source.bindings,
                facts,
                file,
                node,
                *id,
                0,
            )
            .ok_or("parameter test actual unavailable")?;
            let body = written
                .child_nodes()
                .find(|n| is_expression(n.kind()))
                .ok_or("parameter test body unavailable")?;
            if node.child_nodes().count() != 1
                || self
                    .source
                    .expression_type(
                        self.source.view(actual_file, actual, view),
                        actual_file,
                        actual,
                    )
                    .is_none_or(|e| !parameter(&e.ty, TypeKind::Int))
                || self
                    .source
                    .expression_type(self.source.calls, declaration.file, body)
                    .is_none_or(|e| !parameter(&e.ty, TypeKind::Bool))
            {
                return Err("parameter test actual or body type is unsupported".into());
            }
            let mut nodes = vec![written];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(
                    self.source.context,
                    declaration.file,
                    current,
                ) {
                    return Err("parameter test annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut nodes = vec![written_call];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, current) {
                    return Err("parameter test call annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            active.push(*id);
            let inspected = (|| -> Result<(), String> {
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(actual_file, actual, view, generators, active)
                {
                    return Err(reason);
                }
                let mut types: Vec<_> = formal_node.child_nodes().take(1).collect();
                while let Some(ty) = types.pop() {
                    if ty.kind() == NodeKind::DomainType {
                        for domain in ty.child_nodes() {
                            if let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    declaration.file,
                                    domain,
                                    self.source.calls,
                                    &[],
                                    active,
                                )
                            {
                                return Err(reason);
                            }
                        }
                    } else {
                        types.extend(ty.child_nodes());
                    }
                }
                match self.initialized_source_safety(
                    declaration.file,
                    body,
                    self.source.calls,
                    &[],
                    active,
                ) {
                    DefinitionSafety::Unsupported(reason) => Err(reason),
                    _ => Ok(()),
                }
            })();
            active.pop();
            inspected
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown("parameter test value is unproved".into()),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Validate a consumer's retained element domain without replacing its facts.
    // Only exact written integer-array extrema can turn a raw range error Unknown.
    pub(in crate::callable_definitions) fn check_array_element_domain(
        &self,
        array: DeclarationId,
        source: &TypeInst,
        element_domain: &Domain,
    ) -> Result<(), String> {
        let Err(reason) = element_domain.numeric_minimum() else {
            return Ok(());
        };
        if !matches!(&source.kind, TypeKind::Array { element, .. }
            if element.kind == TypeKind::Int)
            || source != &self.source.calls.declarations[array.0].ty
            || self.source.bindings.declarations[array.0].role != DeclarationRole::Value
            || !self.source.bindings.declarations[array.0].top_level
        {
            return Err(reason.into());
        }
        let Domain::Range { lower, upper } = crate::domains::bare_index_domain(element_domain)
        else {
            return Err(reason.into());
        };
        let inspected = Domain::Range {
            lower: self.inspect_array_extremum_bound(lower)?,
            upper: self.inspect_array_extremum_bound(upper)?,
        };
        inspected.numeric_minimum().map_err(str::to_owned)?;
        Ok(())
    }
    // A local interpretation only: checked array extrema stay symbolic, while
    // the retained raw bounds and their declaration identities remain unchanged.
    pub(in crate::callable_definitions) fn inspect_array_extremum_bound(
        &self,
        bound: &NumericBound,
    ) -> Result<NumericBound, String> {
        match bound {
            NumericBound::Unsupported(reason) => Err(reason.clone()),
            NumericBound::Arithmetic { operator, operands } => Ok(NumericBound::Arithmetic {
                operator: *operator,
                operands: operands
                    .iter()
                    .map(|operand| self.inspect_array_extremum_bound(operand))
                    .collect::<Result<_, _>>()?,
            }),
            NumericBound::Defined { declaration, value } => {
                let owner = &self.source.bindings.declarations[declaration.0];
                let parameter_int = TypeInst {
                    instantiation: Instantiation::Parameter,
                    optional: false,
                    kind: TypeKind::Int,
                };
                if !owner.top_level
                    || owner.role != DeclarationRole::Value
                    || self.source.calls.declarations[declaration.0].ty != parameter_int
                {
                    return Err("array extrema bound requires a present parameter integer".into());
                }
                let written = find_node(
                    self.source.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("array extrema bound declaration unavailable")?;
                if !self.source.source_annotations_safe(owner.file, written) {
                    return Err("array extrema bound annotation is unsupported".into());
                }
                let initializer = written
                    .child_nodes()
                    .find(|node| is_expression(node.kind()))
                    .map(unwrap)
                    .ok_or("array extrema bound initializer unavailable")?;
                let facts = self.source.view(owner.file, initializer, self.source.calls);
                if self
                    .source
                    .expression_type(facts, owner.file, initializer)
                    .is_none_or(|expression| expression.ty != parameter_int)
                {
                    return Err("array extrema bound initializer type is unsupported".into());
                }
                crate::domains::core_arithmetic(
                    self.source.context,
                    self.source.bindings,
                    facts,
                    owner.file,
                    initializer,
                )?;
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    owner.file,
                    initializer,
                    facts,
                    &[],
                    &mut Vec::new(),
                ) {
                    return Err(reason);
                }
                let checked = if let NumericBound::Unsupported(reason) = value.as_ref() {
                    let arguments: Vec<_> = initializer.child_nodes().collect();
                    let [argument] = arguments.as_slice() else {
                        return Err(reason.clone());
                    };
                    let source = unwrap(argument);
                    let Some(array) = self.source.reference(owner.file, source) else {
                        return Err(reason.clone());
                    };
                    let source_owner = &self.source.bindings.declarations[array.0];
                    let actual = &self.source.calls.declarations[array.0].ty;
                    if initializer.kind() != NodeKind::CallExpression
                        || !(self.source.core(owner.file, initializer, facts, "min")
                            || self.source.core(owner.file, initializer, facts, "max"))
                        || argument.kind() == NodeKind::NamedArgument
                        || source.kind() != NodeKind::Expression
                        || !matches!(crate::domains::tokens(&self.source.context.files[owner.file].parsed, source).as_slice(),
                            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                        || source_owner.role != DeclarationRole::Value
                        || !source_owner.top_level
                        || actual.instantiation != Instantiation::Parameter
                        || optional(actual)
                        || !matches!(&actual.kind, TypeKind::Array { indices, element }
                            if indices.as_slice() == [parameter_int.clone()]
                                && element.as_ref() == &parameter_int)
                        || self.source.expression_type(facts, owner.file, source)
                            .is_none_or(|expression| &expression.ty != actual)
                        || !self.source.operation_fact(facts, owner.file, initializer).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.as_slice() == [actual.clone()] && return_type == &parameter_int)
                        })
                    {
                        return Err(reason.clone());
                    }
                    let Some(CallOutcome::Resolved { declaration, .. }) = self
                        .source
                        .operation_fact(facts, owner.file, initializer)
                        .map(|call| &call.outcome)
                    else {
                        return Err(reason.clone());
                    };
                    let primitive = &self.source.bindings.declarations[declaration.0];
                    let written = find_node(
                        self.source.context.files[primitive.file].parsed.tree(),
                        &primitive.syntax_range,
                        primitive.role,
                    )
                    .ok_or("array extrema primitive declaration unavailable")?;
                    let mut nodes = vec![written];
                    while let Some(node) = nodes.pop() {
                        if is_expression(node.kind())
                            || !self.source.callable_annotations_safe(primitive.file, node)
                        {
                            return Err("array extrema primitive body, default, domain or annotation is unsupported".into());
                        }
                        // Only validated hint subtrees are skipped; inspect every
                        // other descendant for bodies, defaults and domain sources.
                        nodes.extend(
                            node.child_nodes()
                                .filter(|child| child.kind() != NodeKind::Annotation),
                        );
                    }
                    let Domain::Array { indices, element } = crate::domains::bare_index_domain(
                        &self.source.domains.declarations[array.0].domain,
                    ) else {
                        return Err(reason.clone());
                    };
                    if indices.len() != 1 {
                        return Err(reason.clone());
                    }
                    for domain in indices.iter().chain(std::iter::once(element.as_ref())) {
                        domain.numeric_minimum().map_err(str::to_owned)?;
                    }
                    let axis = crate::domains::bare_index_domain(&indices[0]);
                    if crate::domains::index_domain_interval(axis)
                        .is_some_and(|(lower, upper)| upper < lower)
                        || matches!(axis, Domain::LiteralSet(values) if values.is_empty())
                    {
                        return Err("integer extrema source is empty".into());
                    }
                    if let Some(written) = find_node(
                        self.source.context.files[source_owner.file].parsed.tree(),
                        &source_owner.syntax_range,
                        source_owner.role,
                    ) && written
                        .child_nodes()
                        .find(|node| is_expression(node.kind()))
                        .is_some_and(|value| {
                            let value = unwrap(value);
                            value.kind() == NodeKind::ArrayLiteral
                                && value.child_nodes().next().is_none()
                        })
                    {
                        return Err("integer extrema source is empty".into());
                    }
                    NumericBound::Unknown
                } else {
                    self.inspect_array_extremum_bound(value)?
                };
                Ok(NumericBound::Defined {
                    declaration: *declaration,
                    value: Box::new(checked),
                })
            }
            _ => Ok(bound.clone()),
        }
    }
    pub(in crate::callable_definitions) fn parameter_float_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        if node.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            && typed(node).is_some_and(|t| present(t, TypeKind::Float, Instantiation::Parameter))
            && let Some(id) = self.source.reference(file, node)
            && self.source.bindings.declarations[id.0].role == DeclarationRole::Local
        {
            let declaration = &self.source.bindings.declarations[id.0];
            if declaration.file != file
                || declaration.syntax_range.end > node.range().start
                || !present(
                    &facts.declarations[id.0].ty,
                    TypeKind::Float,
                    Instantiation::Parameter,
                )
            {
                return Some(DefinitionSafety::Unsupported(
                    "Float local alias identity or order is unsupported".into(),
                ));
            }
            let Some(local) = find_node(
                self.source.context.files[file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return Some(DefinitionSafety::Unsupported(
                    "Float local declaration unavailable".into(),
                ));
            };
            let lexical: Vec<_> = generators
                .iter()
                .copied()
                .filter(|g| g.range().end <= declaration.syntax_range.start)
                .collect();
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || !crate::definitions::annotations_safe(self.source.context, file, local)
                || self.type_dependencies(file, local, view, &lexical).is_err()
            {
                return Some(DefinitionSafety::Unsupported(
                    "Float local type or annotation is unsupported".into(),
                ));
            }
            let Some(value) = local.child_nodes().find(|n| is_expression(n.kind())) else {
                return Some(DefinitionSafety::Unsupported(
                    "Float local initializer unavailable".into(),
                ));
            };
            if typed(value).is_none_or(|t| !present(t, TypeKind::Float, Instantiation::Parameter)) {
                return Some(DefinitionSafety::Unsupported(
                    "Float local initializer type is unsupported".into(),
                ));
            }
            return Some(
                match self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new()) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("Float local value is unproved".into()),
                },
            );
        }
        let name = match node.kind() {
            NodeKind::BinaryExpression => operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                .filter(|name| matches!(*name, "*" | "/")),
            NodeKind::CallExpression => ["int2float", "ceil", "fix"]
                .into_iter()
                .find(|name| self.source.core(file, node, view, name)),
            _ => None,
        }?;
        let (input, result, instantiation, arity) = match name {
            "int2float" => (TypeKind::Int, TypeKind::Float, Instantiation::Parameter, 1),
            "ceil" => (TypeKind::Float, TypeKind::Int, Instantiation::Parameter, 1),
            "fix" => (TypeKind::Int, TypeKind::Int, Instantiation::Decision, 1),
            "*" | "/"
                if typed(node)
                    .is_some_and(|t| present(t, TypeKind::Float, Instantiation::Parameter)) =>
            {
                (
                    TypeKind::Float,
                    TypeKind::Float,
                    Instantiation::Parameter,
                    2,
                )
            }
            _ => return None,
        };
        let children: Vec<_> = node.child_nodes().collect();
        // Parameter fix retains its existing strict path; only the unproved
        // scalar decision value needs this inspection fallback.
        if name == "fix"
            && (children.len() != 1
                || typed(children[0])
                    .is_none_or(|t| !present(t, TypeKind::Int, Instantiation::Decision)))
        {
            return None;
        }
        if !self.source.core(file, node, view, name)
            || children.len() != arity
            || children.iter().any(|n| {
                n.kind() == NodeKind::NamedArgument
                    || typed(n).is_none_or(|t| {
                        !present(t, input.clone(), instantiation)
                            && !(input == TypeKind::Float
                                && present(t, TypeKind::Int, Instantiation::Parameter))
                    })
            })
            || typed(node).is_none_or(|t| !present(t, result.clone(), Instantiation::Parameter))
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || self
                .source
                .operation_fact(facts, file, node)
                .is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == arity
                        && parameters.iter().all(|t| present(t, input.clone(), instantiation))
                        && present(return_type, result.clone(), Instantiation::Parameter)
                        && Some(return_type) == typed(node))
                })
        {
            return Some(DefinitionSafety::Unsupported(
                "Float conversion or scalar fix signature is unsupported".into(),
            ));
        }
        let coerced_integer = input == TypeKind::Float
            && children.iter().any(|child| {
                typed(child).is_some_and(|ty| present(ty, TypeKind::Int, Instantiation::Parameter))
            });
        if coerced_integer
            && self.source.operation_fact(facts, file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if self.source.prefix_primitive(file, node, facts, name, parameters, return_type))
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "Float coerced selected written primitive is unsupported".into(),
            ));
        }
        // Inspect coerced operands at their written Int type; a Float formal
        // does not hide closed integer errors or supply a converted value.
        if coerced_integer {
            for child in &children {
                if typed(child)
                    .is_some_and(|ty| present(ty, TypeKind::Int, Instantiation::Parameter))
                    && let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, child, false, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
            }
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        if name == "/" {
            if typed(children[1])
                .is_some_and(|ty| present(ty, TypeKind::Int, Instantiation::Parameter))
                && crate::domains::expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    children[1],
                )
                .is_ok_and(|value| value == Some(0))
            {
                return Some(DefinitionSafety::Unsupported(
                    "Float division by closed integer zero is unsupported".into(),
                ));
            }
            let divisor = unwrap(children[1]);
            let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, divisor);
            let literal_zero = divisor.kind() == NodeKind::Expression
                && tokens.len() == 1
                && tokens[0].kind == TokenKind::FloatLiteral
                && self.source.context.files[file].parsed.source()[tokens[0].range.clone()]
                    .split(['e', 'E'])
                    .next()
                    .is_some_and(|mantissa| mantissa.bytes().all(|b| matches!(b, b'0' | b'.')));
            if literal_zero {
                return Some(DefinitionSafety::Unsupported(
                    "Float division by literal zero is unsupported".into(),
                ));
            }
        }
        Some(DefinitionSafety::Unknown(
            "Float conversion, arithmetic or scalar fixedness is unproved".into(),
        ))
    }
    // Inspect these parameter enum-set sources without granting membership or outputs.
    pub(in crate::callable_definitions) fn parameter_enum_set_source_safety(
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
                .map(|expression| &expression.ty)
        };
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        let result = ty(node)?;
        if !parameter(result)
            || !matches!(&result.kind, TypeKind::Set(element)
                if parameter(element) && matches!(element.kind, TypeKind::Enum(_)))
            || !matches!(
                node.kind(),
                NodeKind::BinaryExpression | NodeKind::ArrayAccessExpression
            )
            || node.kind() == NodeKind::BinaryExpression
                && operator(self.source.context, file, node)
                    .and_then(crate::bindings::symbolic_operator)
                    != Some("intersect")
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let checked = (|| -> Result<(), String> {
            let mut annotated = vec![written];
            while let Some(source) = annotated.pop() {
                if source.kind() == NodeKind::NamedArgument
                    || !self.source.source_annotations_safe(file, source)
                {
                    return Err(
                        "parameter enum-set source annotation or argument is unsupported".into(),
                    );
                }
                annotated.extend(source.child_nodes());
            }
            let [left, right] = children.as_slice() else {
                return Err("parameter enum-set source arity is unsupported".into());
            };
            if node.kind() == NodeKind::BinaryExpression {
                if !self.source.core(file, node, view, "intersect")
                    || ty(left) != Some(result)
                    || ty(right) != Some(result)
                    || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(|formal| formal == result)
                                && return_type == result))
                {
                    return Err("parameter enum-set intersection signature is unsupported".into());
                }
            } else {
                let source = unwrap(left);
                let selector = *right;
                if source.kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, source).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                {
                    return Err("parameter enum-set array source is not a bare value".into());
                }
                let id = self
                    .source
                    .reference(file, source)
                    .ok_or("parameter enum-set array source identity is unavailable")?;
                let declaration = &self.source.bindings.declarations[id.0];
                let array = &self.source.calls.declarations[id.0].ty;
                let TypeKind::Array { indices, element } = &array.kind else {
                    return Err("parameter enum-set array type is unsupported".into());
                };
                if declaration.role != DeclarationRole::Value
                    || !declaration.top_level
                    || !parameter(array)
                    || ty(source) != Some(array)
                    || indices.len() != 1
                    || !parameter(&indices[0])
                    || element.as_ref() != result
                    || ty(selector) != Some(&indices[0])
                {
                    return Err("parameter enum-set array or selector type is unsupported".into());
                }
                let TypeKind::Enum(enum_id) = indices[0].kind else {
                    return Err("parameter enum-set array axis identity is unsupported".into());
                };
                let Domain::Array { indices, .. } = &self.source.domains.declarations[id.0].domain
                else {
                    return Err("parameter enum-set array domain is unavailable".into());
                };
                if indices.len() != 1
                    || crate::domains::bare_index_domain(&indices[0]) != &Domain::Enum(enum_id)
                {
                    return Err("parameter enum-set array axis is not its full enum".into());
                }
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            for child in &children {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, child, true, true)
                {
                    return Err(reason);
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "parameter enum-set source membership or value is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn parameter_enum_conversion_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.source.core(file, node, view, "to_enum")
        {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            self.source
                .expression_type(facts, file, value)
                .map(|e| &e.ty)
        };
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        let children: Vec<_> = node.child_nodes().collect();
        let checked = (|| -> Result<(), String> {
            let [set, value] = children.as_slice() else {
                return Err("enum conversion arity is unsupported".into());
            };
            let result = ty(node)
                .filter(|t| parameter(t))
                .ok_or("enum conversion result unavailable")?;
            let TypeKind::Enum(enum_id) = result.kind else {
                return Err("enum conversion result identity is unsupported".into());
            };
            if children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || ty(set).is_none_or(|t| !parameter(t)
                    || !matches!(&t.kind, TypeKind::Set(element)
                        if parameter(element) && element.kind == result.kind))
                || ty(value).is_none_or(|t| !parameter(t) || !matches!(t.kind, TypeKind::Int | TypeKind::Enum(_)))
                || self.source.operation_fact(facts, file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && return_type == result
                            && parameters[0] == *ty(set).unwrap()
                            && parameter(&parameters[1]) && parameters[1].kind == TypeKind::Int
                            && ty(value).is_some_and(|actual| crate::types::coerces(actual, &parameters[1]))))
            {
                return Err("enum conversion selected signature or annotation is unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, value, false, false)
            {
                return Err(reason);
            }
            let ordinal = crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                file,
                value,
            )
            .ok()
            .flatten();
            if ordinal.is_some_and(|n| {
                n < 1
                    || self
                        .source
                        .enum_extent(enum_id)
                        .is_some_and(|size| n > size)
            }) || self.source.enum_extent(enum_id) == Some(0)
            {
                return Err("enum conversion ordinal is outside the declared enum".into());
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("enum conversion membership or value is unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect the bodyless parameter builtin without proving a successor value
    // or that the value belongs to its enum universe.
    pub(in crate::callable_definitions) fn parameter_enum_successor_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !self.source.core(file, node, view, "enum_next")
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [set, value] = children.as_slice() else {
            return None;
        };
        let facts = self.source.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        let result = ty(node).filter(|t| parameter(t) && matches!(t.kind, TypeKind::Enum(_)))?;
        let checked = (|| -> Result<(), String> {
            let call = self
                .source
                .operation_fact(facts, file, node)
                .ok_or("enum successor selected signature unavailable")?;
            let CallOutcome::Resolved {
                declaration: id,
                parameters,
                return_type,
            } = &call.outcome
            else {
                return Err("enum successor selected signature is unsupported".into());
            };
            if children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || ty(set).is_none_or(|t| {
                    !parameter(t)
                        || !matches!(&t.kind, TypeKind::Set(element)
                        if parameter(element) && element.kind == result.kind)
                })
                || ty(value) != Some(result)
                || parameters.len() != 2
                || return_type != result
                || parameters[0] != *ty(set).unwrap()
                || parameters[1] != *result
            {
                return Err("enum successor tuple or annotation is unsupported".into());
            }
            let signature = self
                .source
                .calls
                .signatures
                .iter()
                .find(|s| s.declaration == *id)
                .ok_or("enum successor written signature unavailable")?;
            let generic = &signature.return_type;
            if signature.parameters.len() != 2
                || signature.parameters.iter().any(|p| p.has_default)
                || optional(generic)
                || generic.instantiation != Instantiation::Parameter
                || !matches!(
                    generic.kind,
                    TypeKind::Variable {
                        enum_only: true,
                        any: false,
                        ..
                    }
                )
                || signature.parameters[1].ty != *generic
                || optional(&signature.parameters[0].ty)
                || signature.parameters[0].ty.instantiation != Instantiation::Parameter
                || !matches!(&signature.parameters[0].ty.kind, TypeKind::Set(element) if **element == *generic)
            {
                return Err("enum successor written generic signature is unsupported".into());
            }
            let declaration = &self.source.bindings.declarations[id.0];
            let source = &self.source.context.files[declaration.file];
            let written = find_node(
                source.parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("enum successor declaration unavailable")?;
            if !source.parsed.diagnostics().is_empty()
                || written.child_nodes().any(|n| is_expression(n.kind()))
                || !self
                    .source
                    .callable_annotations_safe(declaration.file, written)
            {
                return Err("enum successor body or annotation is unsupported".into());
            }
            for position in 0..2 {
                let formal_id =
                    formal_parameter(self.source.context, self.source.bindings, *id, position)
                        .ok_or("enum successor formal unavailable")?;
                let owner = &self.source.bindings.declarations[formal_id.0];
                let formal = find_node(
                    self.source.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("enum successor written formal unavailable")?;
                if self.source.calls.declarations[formal_id.0].ty
                    != signature.parameters[position].ty
                    || formal.child_nodes().any(|n| is_expression(n.kind()))
                {
                    return Err("enum successor formal type or default is unsupported".into());
                }
                let mut nodes = vec![formal];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, owner.file, node)
                    {
                        return Err("enum successor formal annotation is unsupported".into());
                    }
                    nodes.extend(node.child_nodes());
                }
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            for argument in &children {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, argument, true, true)
                {
                    return Err(reason);
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("enum successor value or definedness is unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn parameter_set_extremum_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        // Preserve the existing index-set extremum inspection and its owning scope checks.
        if parameter_index_extremum(
            self.source.context,
            self.source.bindings,
            self.source.view(file, node, view),
            file,
            node,
        )
        .is_some()
        {
            return None;
        }
        if node.kind() != NodeKind::CallExpression
            || !(self.source.core(file, node, view, "min")
                || self.source.core(file, node, view, "max"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let facts = self.source.view(file, node, view);
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let parameter_element = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(t.kind, TypeKind::Int | TypeKind::Enum(_))
        };
        let parameter_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if parameter_element(element))
        };
        if typed(argument).is_none_or(|t| !parameter_set(t)) {
            return None;
        }
        if argument.kind() == NodeKind::NamedArgument
            || typed(node).is_none_or(|t| !parameter_element(t))
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || self
                .source
                .operation_fact(facts, file, node)
                .is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameter_set(&parameters[0])
                        && parameter_element(return_type) && Some(return_type) == typed(node)
                        && parameters[0] == *typed(argument).unwrap()
                        && matches!(&parameters[0].kind, TypeKind::Set(element)
                            if element.kind == return_type.kind))
                })
        {
            return Some(DefinitionSafety::Unsupported(
                "parameter set extremum signature is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        let empty = self.source.parameter_set_source_empty(file, argument);
        Some(if empty {
            DefinitionSafety::Unsupported(
                "parameter set extremum is undefined for an empty set".into(),
            )
        } else {
            DefinitionSafety::Unknown(
                "parameter set extremum nonemptiness or value is unproved".into(),
            )
        })
    }
    // Inspect an initialized conditional source without proving its minimum,
    // index membership, extent or output dependencies.
    pub(in crate::callable_definitions) fn conditional_integer_min_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        collection: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DefinitionSafety> {
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(integer.clone()),
        });
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let call = self.source.operation_fact(facts, file, node)?;
        let CallOutcome::Resolved {
            declaration: primitive,
            parameters,
            return_type,
        } = &call.outcome
        else {
            return None;
        };
        let id = self.source.reference(file, collection)?;
        let owner = &self.source.bindings.declarations[id.0];
        if !self.source.core(file, node, facts, "min")
            || parameters.as_slice() != std::slice::from_ref(&array)
            || return_type != &integer
            || self
                .source
                .expression_type(facts, file, collection)
                .is_none_or(|value| value.ty != array)
            || facts.declarations[id.0].ty != array
            || owner.file != file
            || self.source.context.files[file].kind != SourceKind::User
            || !(owner.role == DeclarationRole::Value && owner.top_level
                || owner.role == DeclarationRole::Local
                    && !owner.top_level
                    && owner.syntax_range.end <= collection.range().start)
        {
            return None;
        }
        let declaration = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let initializers: Vec<_> = declaration
            .child_nodes()
            .filter(|part| is_expression(part.kind()))
            .collect();
        let [initializer] = initializers.as_slice() else {
            return None;
        };
        if unwrap(initializer).kind() != NodeKind::ConditionalExpression {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || !self
                    .source
                    .prefix_primitive(file, node, facts, "min", parameters, return_type)
                || !self.source.calls.signatures.iter().any(|signature| {
                    signature.declaration == *primitive
                        && signature.parameters.len() == 1
                        && !signature.parameters[0].has_default
                })
                || self
                    .source
                    .expression_type(self.source.view(file, initializer, view), file, initializer)
                    .is_none_or(|value| value.ty != array)
            {
                return Err(
                    "conditional integer minimum selected primitive or source type is unsupported"
                        .into(),
                );
            }
            let lexical = if owner.role == DeclarationRole::Local {
                local_source_scope(self.source.context.files[file].parsed.tree(), declaration)
                    .ok_or("conditional minimum local owning scope is unavailable")?
            } else {
                Vec::new()
            };
            self.integer_source_headers_safety(file, view, &lexical)?;
            self.type_dependencies(file, declaration, view, &lexical)?;
            let mut active = vec![id];
            let mut types = vec![
                declaration
                    .child_nodes()
                    .next()
                    .ok_or("conditional minimum written type is unavailable")?,
            ];
            if !self.source.source_annotations_safe(file, declaration) {
                return Err("conditional minimum source annotation is unsupported".into());
            }
            while let Some(ty) = types.pop() {
                if ty.kind() == NodeKind::Error || !self.source.source_annotations_safe(file, ty) {
                    return Err("conditional minimum written type is unsupported".into());
                }
                if ty.kind() == NodeKind::DomainType {
                    for source in ty.child_nodes() {
                        if let Some(reason) = self
                            .source
                            .closed_integer_source_error(file, source, false, true)
                        {
                            return Err(reason);
                        }
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(file, source, view, &lexical, &mut active)
                        {
                            return Err(reason);
                        }
                    }
                } else {
                    types.extend(ty.child_nodes());
                }
            }
            // The existing scan checks transitive initialized references before
            // direct inspection, retaining its active-owner cycle guard.
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, initializer, view, &lexical, &mut active)
            {
                return Err(reason);
            }
            if !self
                .source
                .nonempty_conditional_array_source(file, initializer)
            {
                return Err(
                    "conditional integer minimum source has an empty or unsupported branch".into(),
                );
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("conditional integer minimum value is unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn decision_scalar_bound_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !(self.source.core(file, node, view, "lb")
                || self.source.core(file, node, view, "ub"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let selection = unwrap(argument);
        if selection.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let actual = self
            .source
            .expression_type(facts, file, argument)
            .map(|e| &e.ty)?;
        if actual.instantiation != Instantiation::Decision {
            return None;
        }
        let result = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        if argument.kind() == NodeKind::NamedArgument
            || !actual.known() || optional(actual) || actual.kind != TypeKind::Int
            || result.is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int
                || t.instantiation != Instantiation::Parameter)
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || !self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0] == *actual && Some(return_type) == result)
            })
        {
            return Some(DefinitionSafety::Unsupported("scalar reflection selected operand or signature is unsupported".into()));
        }
        Some(self.decision_scalar_bound_operand_safety(file, argument, view, generators))
    }
    pub(in crate::callable_definitions) fn decision_scalar_bound_operand_safety(
        &self,
        file: FileId,
        argument: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let selection = unwrap(argument);
        let subject = selection.child_nodes().next().map(unwrap);
        let array = subject.and_then(|node| self.source.reference(file, node));
        if selection.kind() != NodeKind::ArrayAccessExpression
            || self.source.expression_type(self.source.view(file, argument, view), file, argument)
                .is_none_or(|e| e.ty != TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision))
            || subject.is_none_or(|node| node.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
            || array.is_none_or(|id| self.source.bindings.declarations[id.0].role != DeclarationRole::Value
                || !self.source.bindings.declarations[id.0].top_level)
        {
            return DefinitionSafety::Unsupported("scalar reflection selected operand or signature is unsupported".into());
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &[argument], view, generators)
        {
            return unsupported;
        }
        let owner = &self.source.bindings.declarations[array.unwrap().0];
        if let Some(written) = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) && written
            .child_nodes()
            .find(|n| is_expression(n.kind()))
            .map(unwrap)
            .is_some_and(|value| {
                value.kind() == NodeKind::ArrayLiteral && value.child_nodes().next().is_none()
            })
        {
            return DefinitionSafety::Unsupported("scalar reflection source is empty".into());
        }
        let mut domains = vec![&self.source.domains.declarations[owner.id.0].domain];
        while let Some(domain) = domains.pop() {
            match domain {
                Domain::Named { domain, .. } => domains.push(domain),
                Domain::Array { indices, element } => {
                    domains.extend(indices);
                    domains.push(element);
                }
                Domain::Range { lower, upper }
                    if matches!((crate::domains::invariant_integer(lower), crate::domains::invariant_integer(upper)),
                        (Ok(Some(lower)), Ok(Some(upper))) if upper < lower) =>
                {
                    return DefinitionSafety::Unsupported(
                        "scalar reflection source domain is empty".into(),
                    );
                }
                Domain::LiteralSet(values) if values.is_empty() => {
                    return DefinitionSafety::Unsupported(
                        "scalar reflection source domain is empty".into(),
                    );
                }
                _ => {}
            }
        }
        DefinitionSafety::Unknown("reflected scalar bound is unproved".into())
    }
    pub(in crate::callable_definitions) fn floor_sum_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.source.core(file, node, view, "floor") {
            return None;
        }
        let checked = crate::domains::symbolic_floor_sum_bound(
            self.source.context,
            self.source.bindings,
            self.source.view(file, node, view),
            file,
            node,
        )?;
        let checked = checked.and_then(|()| {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Err("floor sum source annotation is unsupported".into());
            }
            let arguments: Vec<_> = node
                .child_nodes()
                .filter(|child| child.kind() != NodeKind::Annotation)
                .collect();
            let [quotient] = arguments.as_slice() else {
                return Err("floor sum source quotient is unavailable".into());
            };
            let operands: Vec<_> = quotient
                .child_nodes()
                .filter(|child| child.kind() != NodeKind::Annotation)
                .collect();
            // The shared inspector checks these original Int actuals against
            // the selected Float formals. Retain initialized-source vetoes.
            match self.initialized_children_safety(file, &operands, view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            }
        });
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown("parameter floor sum value is unproved".into()),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect the selected integer product implementation without proving its
    // value, bounds, local recurrence outputs or flattened extent.
    pub(in crate::callable_definitions) fn integer_product_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let integer = TypeInst::par(TypeKind::Int);
        let decision_integer = integer.clone().with_inst(Instantiation::Decision);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(decision_integer.clone()),
        })
        .with_inst(Instantiation::Decision);
        let facts = self.source.view(file, node, view);
        let call = self.source.operation_fact(facts, file, node)?;
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &call.outcome
        else {
            return None;
        };
        if node.kind() != NodeKind::CallExpression
            || !self.source.core(file, node, facts, "product")
            || parameters.as_slice() != std::slice::from_ref(&array)
            || return_type != &decision_integer
        {
            return None;
        }
        let checked = (|| {
            let arguments: Vec<_> = node.child_nodes().collect();
            let [actual] = arguments.as_slice() else {
                return Err("integer product requires its single written array actual".into());
            };
            let actual_written = *actual;
            let actual = unwrap(actual_written);
            let actual_type = self
                .source
                .expression_type(facts, file, actual)
                .ok_or("integer product actual type is unavailable")?;
            let TypeKind::Array { indices, element } = &actual_type.ty.kind else {
                return Err("integer product actual is not an array".into());
            };
            // The raw rank belongs to the caller, not to the matched rank-one
            // formal. Restrict this route to a retained top-level array source.
            let source = self
                .source
                .reference(file, actual)
                .filter(|_| {
                    actual.kind() == NodeKind::Expression && actual.child_nodes().next().is_none()
                })
                .ok_or("integer product actual source identity is unsupported")?;
            let owner = &self.source.bindings.declarations[source.0];
            if call.generator_argument.is_some()
                || !actual_type.ty.known()
                || optional(&actual_type.ty)
                || actual_type.ty.instantiation != Instantiation::Decision
                || !matches!(indices.len(), 1 | 2)
                || indices.iter().any(|axis| axis != &integer)
                || element.as_ref() != &decision_integer
                || !owner.top_level
                || owner.role != DeclarationRole::Value
                || facts.declarations[source.0].ty != actual_type.ty
                || self
                    .source
                    .expression_type(facts, file, node)
                    .is_none_or(|value| value.ty != decision_integer)
                || !self.source.source_annotations_safe(file, written)
            {
                return Err(
                    "integer product actual type, scope or annotation is unsupported".into(),
                );
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, written, true, true)
            {
                return Err(reason);
            }
            if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                file,
                actual_written,
                facts,
                generators,
                &mut Vec::new(),
            ) {
                return Err(reason);
            }
            self.integer_product_written_body(*declaration, &array, &decision_integer)
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "integer product value, bounds and output dependencies are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
}
