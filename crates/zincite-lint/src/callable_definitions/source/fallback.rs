//! Ordered source-family fallbacks after strict dependencies refuse the expression.
use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn fallback_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        reason: String,
    ) -> DefinitionSafety {
        let node = unwrap(written);
        if self.source.default_collection(file, node) {
            return match self.default_collection_arguments(file, node, view, generators) {
                Ok((_, fallback, all)) => self.direct_safety(file, fallback, view, &all),
                Err(reason) => DefinitionSafety::Unsupported(reason),
            };
        }
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if let Some(safety) =
            self.concatenated_and_literal_array_safety(file, written, view, generators)
        {
            return safety;
        }
        if let Some(safety) = self.enum_index_conversion_safety(file, written, view, generators) {
            return safety;
        }
        if let Some(safety) =
            self.parameter_test_safety(file, written, view, generators, &mut Vec::new())
        {
            return safety;
        }
        if let Some(safety) = self.conditional_source_safety(file, written, view, generators) {
            return safety;
        }
        if let Some(safety) = self.scalar_selector_source_safety(file, written, view, generators) {
            return safety;
        }
        if let Some(safety) = self.float_comparison_source_safety(file, written, view, generators) {
            return safety;
        }
        if let Some(safety) = self.decision_boolean_source_safety(file, written, view, generators) {
            return safety;
        }
        if let Some(safety) =
            self.constructed_collection_source_safety(file, written, view, generators)
        {
            return safety;
        }
        if ty(node).is_some_and(|t| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && (t.kind == TypeKind::Bool
                    || matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int))
        }) {
            return self.parameter_relation_source_safety(file, written, view, generators, reason);
        }
        if ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int) {
            return DefinitionSafety::Unsupported(reason);
        }
        match node.kind() {
            NodeKind::Expression => {
                if let Some(id) = self.source.reference(file, node) {
                    let declaration = &self.source.bindings.declarations[id.0];
                    if declaration.role == DeclarationRole::Local
                        && declaration.file == file
                        && declaration.syntax_range.end <= node.range().start
                        && view.declarations[id.0].ty.instantiation == Instantiation::Parameter
                        && let Some(local) = find_node(
                            self.source.context.files[file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                        && crate::definitions::annotations_safe(self.source.context, file, local)
                        && let Some(value) = local.child_nodes().find(|n| is_expression(n.kind()))
                    {
                        let Some(lexical) = local_source_scope(
                            self.source.context.files[file].parsed.tree(),
                            local,
                        ) else {
                            return DefinitionSafety::Unsupported(
                                "local initializer owning scope is unavailable".into(),
                            );
                        };
                        return self.direct_safety(file, value, view, &lexical);
                    }
                }
                DefinitionSafety::Unsupported(reason)
            }
            NodeKind::ConditionalExpression => {
                match self.source.integer_conditional_values(file, node, view) {
                    Ok(values) => self.direct_children_safety(file, &values, view, generators),
                    Err(reason) => DefinitionSafety::Unsupported(reason),
                }
            }
            NodeKind::ArrayAccessExpression => {
                self.integer_selector_safety(file, written, view, generators, reason)
            }
            NodeKind::BinaryExpression => {
                self.integer_operator_safety(file, written, view, generators, reason)
            }
            NodeKind::CallExpression => {
                self.integer_call_safety(file, written, view, generators, reason)
            }
            NodeKind::GeneratorCallExpression => {
                self.integer_generator_safety(file, written, view, generators, reason)
            }
            _ => DefinitionSafety::Unsupported(reason),
        }
    }

    fn concatenated_and_literal_array_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if ty(node).is_some_and(|t| {
            matches!(&t.kind, TypeKind::Array { element, .. }
                if matches!(element.kind, TypeKind::Set(_)))
        }) && let Some(arguments) = array_concatenation(
            self.source.context,
            self.source.bindings,
            self.source.view(file, node, view),
            file,
            node,
        ) {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Some(DefinitionSafety::Unsupported(
                    "set-array concatenation annotation is unsupported".into(),
                ));
            }
            let sources = self.initialized_children_safety(file, &arguments, view, generators);
            for argument in arguments {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, argument, true, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
            }
            return Some(match sources {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown(
                    "parameter set-array concatenation extent and values are unproved".into(),
                ),
            });
        }
        if node.kind() == NodeKind::BinaryExpression
            && operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                == Some("++")
            && let Some(result) = ty(node)
            && result.known()
            && !optional(result)
            && result.instantiation == Instantiation::Decision
            && let TypeKind::Array { indices, element } = &result.kind
            && indices.len() == 1
            && indices[0].known()
            && !optional(&indices[0])
            && indices[0].instantiation == Instantiation::Parameter
            && indices[0].kind == TypeKind::Int
            && element.known()
            && !optional(element)
            && element.instantiation == Instantiation::Decision
            && matches!(element.kind, TypeKind::Enum(_) | TypeKind::Bool)
        {
            let arguments: Vec<_> = node.child_nodes().collect();
            let checked = self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call|
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && return_type == result && arguments.len() == 2
                        && parameters.iter().chain(arguments.iter().filter_map(|argument| ty(argument))).all(|t|
                            t.known() && !optional(t) && t.instantiation == Instantiation::Decision
                                && matches!(&t.kind, TypeKind::Array { indices, element: actual }
                                    if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                        && indices[0].instantiation == Instantiation::Parameter
                                        && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
                                        && actual == element))
                        && arguments.iter().all(|argument| ty(argument).is_some())
                        && self.source.prefix_primitive(file, node, self.source.view(file, node, view), "++", parameters, result)));
            if !checked || !crate::definitions::annotations_safe(self.source.context, file, written)
            {
                return Some(DefinitionSafety::Unsupported(
                    "decision array concatenation written primitive or tuple is unsupported".into(),
                ));
            }
            if let unsupported @ DefinitionSafety::Unsupported(_) =
                self.initialized_children_safety(file, &arguments, view, generators)
            {
                return Some(unsupported);
            }
            for argument in arguments {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, argument, true, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
            }
            return Some(DefinitionSafety::Unknown(
                "decision array concatenation extent and values are unproved".into(),
            ));
        }
        if node.kind() == NodeKind::ArrayLiteral
            && let Some(array) = ty(node)
            && array.known()
            && !optional(array)
            && let TypeKind::Array { indices, element } = &array.kind
            && indices.len() == 1
            && indices[0].known()
            && !optional(&indices[0])
            && indices[0].instantiation == Instantiation::Parameter
            && indices[0].kind == TypeKind::Int
            && element.known()
            && !optional(element)
            && element.kind == TypeKind::Int
            && crate::definitions::annotations_safe(self.source.context, file, written)
        {
            let cells: Vec<_> = node.child_nodes().collect();
            if cells.iter().all(|cell| {
                cell.kind() != NodeKind::IndexedArrayEntry
                    && ty(cell).is_some_and(|t| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && crate::types::coerces(t, element)
                    })
            }) {
                if let unsupported @ DefinitionSafety::Unsupported(_) =
                    self.initialized_children_safety(file, &cells, view, generators)
                {
                    return Some(unsupported);
                }
                for cell in cells {
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error_with_branches(file, cell, false, true, true)
                    {
                        return Some(DefinitionSafety::Unsupported(reason));
                    }
                }
                // Inspecting each written cell does not prove its dependencies.
                return Some(DefinitionSafety::Unknown(
                    "integer array literal value is unproved".into(),
                ));
            }
        }
        None
    }
    fn enum_index_conversion_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::CallExpression
            && (self.source.core(file, node, view, "index_set_1of2")
                || self.source.core(file, node, view, "index_set_2of2"))
        {
            let children: Vec<_> = node.child_nodes().collect();
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            let array = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && integer(element))
            };
            let set = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Set(element) if integer(element))
            };
            let subject = children.first().copied().map(unwrap);
            let owner = subject.and_then(|n| self.source.reference(file, n));
            if children.len() != 1
                || subject.is_none_or(|n| n.kind() != NodeKind::Expression)
                || owner.is_none_or(|id| {
                    let declaration = &self.source.bindings.declarations[id.0];
                    !(declaration.role == DeclarationRole::Value && declaration.top_level
                        || self.source.core(file, node, view, "index_set_2of2")
                            && declaration.role == DeclarationRole::Parameter)
                })
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || ty(children[0]).is_none_or(|t| !array(t))
                || ty(node).is_none_or(|t| !set(t))
                || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && array(&parameters[0])
                            && ty(children[0]).is_some_and(|actual| crate::types::coerces(actual, &parameters[0]))
                            && Some(return_type) == ty(node)))
            {
                return Some(DefinitionSafety::Unsupported("rank-two integer index-set source is unsupported".into()));
            }
            if owner.is_some_and(|id| {
                self.source.bindings.declarations[id.0].role == DeclarationRole::Parameter
            }) && let Err(reason) =
                self.rank_two_formal_safety(file, children[0], view, generators)
            {
                return Some(DefinitionSafety::Unsupported(reason));
            }
            return Some(
                match self.initialized_source_safety(
                    file,
                    children[0],
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown(
                        "rank-two index-set membership is unproved".into(),
                    ),
                },
            );
        }
        if node.kind() == NodeKind::CallExpression
            && self.source.core(file, node, view, "index2int")
        {
            let children: Vec<_> = node.child_nodes().collect();
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            let array = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(integer) && integer(element))
            };
            let subject = children.first().copied().map(unwrap);
            let owner = subject.and_then(|node| self.source.reference(file, node));
            if children.len() == 1
                && subject.is_some_and(|node| node.kind() == NodeKind::Expression)
                && owner.is_some_and(|id| {
                    let declaration = &self.source.bindings.declarations[id.0];
                    declaration.role == DeclarationRole::Parameter
                        || declaration.role == DeclarationRole::Value && declaration.top_level
                })
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && ty(children[0]).is_some_and(array)
                && ty(node) == ty(children[0])
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call|
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && Some(&parameters[0]) == ty(children[0])
                            && Some(return_type) == ty(node)))
            {
                if owner.is_some_and(|id| {
                    self.source.bindings.declarations[id.0].role == DeclarationRole::Parameter
                }) && let Err(reason) = self.rank_two_formal_safety(file, children[0], view, generators)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                return Some(match self.initialized_source_safety(
                    file,
                    children[0],
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("rank-two integer conversion value is unproved".into()),
                });
            }
        }
        None
    }
    fn conditional_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::ConditionalExpression
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (matches!(t.kind, TypeKind::Int | TypeKind::Bool)
                        || t.instantiation == Instantiation::Parameter
                            && matches!(&t.kind, TypeKind::Set(element)
                            if element.known() && !optional(element)
                                && element.instantiation == Instantiation::Parameter
                                && element.kind == TypeKind::Int)
                        || t.instantiation == Instantiation::Parameter
                            && matches!(&t.kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && indices[0].kind == TypeKind::Int
                                    && element.known() && !optional(element)
                                    && element.instantiation == Instantiation::Parameter
                                    && element.kind == TypeKind::Int))
            })
        {
            let result = ty(node).unwrap();
            let branches: Vec<_> = node.child_nodes().collect();
            let mut values = Vec::new();
            let mut complete = false;
            for (position, branch) in branches.iter().enumerate() {
                let parts: Vec<_> = branch.child_nodes().collect();
                let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                    if ty(parts[0]).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.kind != TypeKind::Bool
                            || t.instantiation != Instantiation::Parameter
                    }) {
                        return Some(DefinitionSafety::Unsupported(
                            "conditional inspection guard is unsupported".into(),
                        ));
                    }
                    values.push(parts[0]);
                    parts[1]
                } else if branch.kind() == NodeKind::ElseBranch
                    && parts.len() == 1
                    && position + 1 == branches.len()
                {
                    complete = true;
                    parts[0]
                } else {
                    return Some(DefinitionSafety::Unsupported(
                        "conditional inspection branch is unsupported".into(),
                    ));
                };
                if ty(body).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || (t.kind != result.kind
                            && !(matches!(&result.kind, TypeKind::Set(_))
                                && unwrap(body).kind() == NodeKind::SetLiteral
                                && unwrap(body).child_nodes().count() == 0
                                && t.instantiation == Instantiation::Parameter
                                && matches!(&t.kind, TypeKind::Set(element)
                                    if element.known() && !optional(element)
                                        && element.instantiation == Instantiation::Parameter
                                        && element.kind == TypeKind::Bottom)))
                        || !crate::types::coerces(t, result)
                }) {
                    return Some(DefinitionSafety::Unsupported(
                        "conditional inspection body is unsupported".into(),
                    ));
                }
                values.push(body);
            }
            if !complete
                || values.len() < 3
                || !crate::definitions::annotations_safe(self.source.context, file, written)
            {
                return Some(DefinitionSafety::Unsupported(
                    "conditional inspection requires a safe complete else".into(),
                ));
            }
            if self.source.selected_set_source && branches.len() == 2 && values.len() == 3 {
                let inactive = match self.source.assertion_literal(file, values[0]) {
                    Some(TokenKind::False) => Some(1),
                    Some(TokenKind::True) => Some(2),
                    _ => None,
                };
                let mut unsupported = None;
                for (position, value) in values.iter().enumerate() {
                    let checked = if inactive == Some(position) {
                        let inspector = SourceInspector {
                            context: self.source.context,
                            bindings: self.source.bindings,
                            calls: self.source.calls,
                            instantiations: self.source.instantiations,
                            domains: self.source.domains,
                            lookups: None,
                            selected_set_source: true,
                            inactive_integer_body: Some((file, value.range())),
                        };
                        let producer = BodyInterpreter::without_bodies(&inspector);
                        producer.initialized_source_safety(
                            file,
                            value,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    } else {
                        self.initialized_source_safety(
                            file,
                            value,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    };
                    if let DefinitionSafety::Unsupported(reason) = checked {
                        unsupported = Some(reason);
                    }
                }
                return Some(match unsupported {
                    Some(reason) => DefinitionSafety::Unsupported(reason),
                    None => DefinitionSafety::Unknown("conditional value is unproved".into()),
                });
            }
            return Some(match self.initialized_children_safety(file, &values, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("conditional value is unproved".into()),
            });
        }
        None
    }
    fn scalar_selector_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::ArrayAccessExpression
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (matches!(t.kind, TypeKind::Int | TypeKind::Bool | TypeKind::Enum(_))
                        || t.kind == TypeKind::Float && t.instantiation == Instantiation::Parameter)
            })
        {
            let children: Vec<_> = node.child_nodes().collect();
            if let Some(subject) = children.first().copied().map(unwrap)
                && subject.kind() == NodeKind::Expression
                && let Some(array) = self.source.reference(file, subject)
                && let Some(source) = ty(subject)
                && source.known()
                && !optional(source)
                && let TypeKind::Array { indices, element } = &source.kind
                && matches!(indices.len(), 1 | 2)
                && children.len() == indices.len() + 1
                && element.known()
                && !optional(element)
                && Some(&element.kind) == ty(node).map(|t| &t.kind)
                && (element.kind != TypeKind::Float
                    || indices.len() == 1
                        && source.instantiation == Instantiation::Parameter
                        && element.instantiation == Instantiation::Parameter
                        && self.source.bindings.declarations[array.0].role == DeclarationRole::Value
                        && self.source.bindings.declarations[array.0].top_level
                        && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                && indices.iter().zip(&children[1..]).enumerate().all(
                    |(position, (axis, selector))| {
                        axis.known()
                            && !optional(axis)
                            && (axis.kind == TypeKind::Int
                                || matches!(axis.kind, TypeKind::Enum(_))
                                    && matches!(element.kind, TypeKind::Enum(_))
                                    && source.instantiation == Instantiation::Decision
                                    && element.instantiation == Instantiation::Decision
                                    && source == &self.source.calls.declarations[array.0].ty
                                    && self.source.bindings.declarations[array.0].top_level
                                    && self.source.bindings.declarations[array.0].role
                                        == DeclarationRole::Value
                                    && matches!(crate::domains::bare_index_domain(&self.source.domains.declarations[array.0].domain),
                                        Domain::Array { indices, .. } if indices.get(position).is_some_and(|domain|
                                            matches!(crate::domains::bare_index_domain(domain), Domain::Enum(id)
                                                if axis.kind == TypeKind::Enum(*id)))))
                            && axis.instantiation == Instantiation::Parameter
                            && ty(selector).is_some_and(|t| {
                                t.known()
                                    && !optional(t)
                                    && t.kind == axis.kind
                                    && (t.instantiation == Instantiation::Parameter
                                        || element.kind == TypeKind::Int
                                            && t.instantiation == Instantiation::Decision
                                            && (indices.len() == 1
                                                || indices.len() == 2 && position == 1))
                            })
                    },
                )
            {
                if let unsupported @ DefinitionSafety::Unsupported(_) =
                    self.initialized_children_safety(file, &children, view, generators)
                {
                    return Some(unsupported);
                }
                let inspected_selection = (indices.len() == 2
                    || indices
                        .iter()
                        .any(|axis| matches!(axis.kind, TypeKind::Enum(_))))
                    && matches!(element.kind, TypeKind::Enum(_))
                    || children[1..].iter().any(|selector| {
                        ty(selector).is_some_and(|t| t.instantiation == Instantiation::Decision)
                    });
                if inspected_selection {
                    if !crate::definitions::annotations_safe(self.source.context, file, written) {
                        return Some(DefinitionSafety::Unsupported(
                            "scalar selection annotation is unsupported".into(),
                        ));
                    }
                    let (axes, source_element) = match crate::domains::bare_index_domain(
                        &self.source.domains.declarations[array.0].domain,
                    ) {
                        Domain::Array { indices, element }
                            if indices.len() == children.len() - 1 =>
                        {
                            (indices, element)
                        }
                        Domain::Unsupported(reason) => {
                            return Some(DefinitionSafety::Unsupported(reason.clone()));
                        }
                        _ => {
                            return Some(DefinitionSafety::Unsupported(
                                "scalar selection source domain is unsupported".into(),
                            ));
                        }
                    };
                    // The closed source checker does not follow decision arrays.
                    // Inspect every retained axis and element before uncertainty.
                    for domain in axes {
                        if let Err(reason) = domain.numeric_minimum() {
                            return Some(DefinitionSafety::Unsupported(reason.into()));
                        }
                    }
                    if let Err(reason) =
                        self.check_array_element_domain(array, source, source_element)
                    {
                        return Some(DefinitionSafety::Unsupported(reason));
                    }
                    for selector in &children[1..] {
                        if let Some(reason) =
                            self.source.closed_integer_source_error(file, selector, false, false)
                        {
                            return Some(DefinitionSafety::Unsupported(reason));
                        }
                    }
                }
                if matches!(element.kind, TypeKind::Enum(_)) && !inspected_selection {
                    if let Domain::Array { indices, .. } =
                        &self.source.domains.declarations[array.0].domain
                        && indices.iter().any(|axis| axis.numeric_minimum().is_err())
                    {
                        return Some(DefinitionSafety::Unsupported(
                            "enum array source index domain is unsupported".into(),
                        ));
                    }
                    for selector in &children[1..] {
                        if let Some(reason) =
                            self.source.closed_integer_source_error(file, selector, false, false)
                        {
                            return Some(DefinitionSafety::Unsupported(reason));
                        }
                    }
                }
                if let Domain::Array { indices, .. } = &self.source.domains.declarations[array.0].domain
                    && indices.iter().zip(&children[1..]).any(|(axis, selector)| {
                        crate::domains::invariant_expression_integer(
                            self.source.context,
                            self.source.bindings,
                            file,
                            selector,
                        )
                        .is_ok_and(|value| {
                            value.is_some_and(|value| {
                                crate::domains::index_domain_member(axis, value) == Some(false)
                            })
                        })
                    })
                {
                    return Some(DefinitionSafety::Unsupported(
                        "scalar selection is outside its declared index set".into(),
                    ));
                }
                let owner = &self.source.bindings.declarations[array.0];
                if element.kind == TypeKind::Bool
                    && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    && (owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    && let Some(written) = find_node(
                        self.source.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    )
                    && !written.child_nodes().any(|node| is_expression(node.kind()))
                    && !self.literal_bool_index_membership(
                        file,
                        node,
                        array,
                        self.source.view(file, node, view),
                    )
                {
                    return Some(DefinitionSafety::Unsupported(
                        "Boolean selection requires an index-membership proof".into(),
                    ));
                }
                return Some(DefinitionSafety::Unknown("scalar selection membership is unproved".into()));
            }
        }
        None
    }
    fn float_comparison_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::BinaryExpression
            && operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                == Some("<=")
            && self.source.core(file, node, view, "<=")
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Bool
                    && t.instantiation == Instantiation::Parameter
            })
        {
            let children: Vec<_> = node.child_nodes().collect();
            let float = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Float
                    && t.instantiation == Instantiation::Parameter
            };
            if children.len() == 2
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && children.iter().all(|child| ty(child).is_some_and(float))
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters.iter().all(float)
                            && Some(return_type) == ty(node))
                })
            {
                return Some(match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("parameter Float comparison value is unproved".into()),
                });
            }
        }
        None
    }
    fn decision_boolean_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if ty(node).is_some_and(|t| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Bool
                && t.instantiation == Instantiation::Decision
        }) {
            let children: Vec<_> = node.child_nodes().collect();
            let name = operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator);
            if node.kind() == NodeKind::BinaryExpression
                && name == Some("in")
                && children.len() == 2
                && self.source.core(file, node, view, "in")
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && let (Some(value), Some(choices)) = (ty(children[0]), ty(children[1]))
                && value.known()
                && !optional(value)
                && value.instantiation == Instantiation::Parameter
                && value.kind == TypeKind::Int
                && decision_integer_set(choices)
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters[0].known() && !optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Decision
                            && parameters[0].kind == TypeKind::Int && parameters[1] == *choices
                            && crate::types::coerces(value, &parameters[0])
                            && Some(return_type) == ty(node))
                })
            {
                let mut sources = children.clone();
                for generator in generators {
                    for source in generator.child_nodes() {
                        let source = if source.kind() == NodeKind::WhereFilter {
                            let Some(condition) = source.child_nodes().next() else {
                                return Some(DefinitionSafety::Unsupported(
                                    "membership iteration filter unavailable".into(),
                                ));
                            };
                            condition
                        } else {
                            source
                        };
                        sources.push(source);
                    }
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &sources, view, generators)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                for source in sources {
                    if let Some(reason) = self.source.closed_integer_source_error(file, source, true, true) {
                        return Some(DefinitionSafety::Unsupported(reason));
                    }
                }
                return Some(DefinitionSafety::Unknown("integer membership value is unproved".into()));
            }
            if node.kind() == NodeKind::BinaryExpression
                && name == Some("in")
                && children.len() == 2
                && self.source.core(file, node, view, "in")
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && let (Some(value), Some(choices)) = (ty(children[0]), ty(children[1]))
                && value.known()
                && !optional(value)
                && value.instantiation == Instantiation::Decision
                && value.kind == TypeKind::Int
                && choices.known()
                && !optional(choices)
                && choices.instantiation == Instantiation::Parameter
                && matches!(&choices.kind, TypeKind::Set(element)
                    if element.known() && !optional(element)
                        && element.instantiation == Instantiation::Parameter
                        && element.kind == TypeKind::Int)
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters[0] == *value
                            && parameters[1] == *choices && Some(return_type) == ty(node))
                })
            {
                return Some(match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("integer membership value is unproved".into()),
                });
            }
            if node.kind() == NodeKind::BinaryExpression
                && matches!(name, Some("=" | "!=")) && children.len() == 2
                && self.source.core(file, node, view, name.unwrap())
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && let (Some(left), Some(right)) = (ty(children[0]), ty(children[1]))
                && left.known() && !optional(left) && right.known() && !optional(right)
                && matches!(left.kind, TypeKind::Enum(_)) && left.kind == right.kind
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call|
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)|
                                formal.known() && !optional(formal) && formal.kind == left.kind
                                    && ty(actual).is_some_and(|actual| crate::types::coerces(actual, formal)))))
            {
                return Some(match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("enum equality value is unproved".into()),
                });
            }
            let operand_kind = match (node.kind(), name) {
                (NodeKind::BinaryExpression, Some("=" | "!=" | "<" | "<=" | ">" | ">="))
                    if children.len() == 2 =>
                {
                    Some(TypeKind::Int)
                }
                (NodeKind::BinaryExpression, Some("/\\" | "\\/" | "<->" | "->" | "<-"))
                    if children.len() == 2 =>
                {
                    Some(TypeKind::Bool)
                }
                (NodeKind::UnaryExpression, Some("not")) if children.len() == 1 => {
                    Some(TypeKind::Bool)
                }
                _ => None,
            };
            if let Some(kind) = operand_kind
                && self.source.core(file, node, view, name.unwrap())
                && crate::definitions::annotations_safe(self.source.context, file, written)
                && children.iter().all(|n| ty(n).is_some_and(|t| t.known() && !optional(t) && t.kind == kind))
                && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == children.len() && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)| formal.known()
                                && !optional(formal) && formal.kind == kind
                                && ty(actual).is_some_and(|t| crate::types::coerces(t, formal))))
                }) {
                return Some(match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("Boolean operation value is unproved".into()),
                });
            }
        }
        None
    }
    fn constructed_collection_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if (matches!(
            node.kind(),
            NodeKind::ArrayComprehension | NodeKind::SetComprehension
        ) || node.kind() == NodeKind::GeneratorCallExpression
            && (self.source.core(file, node, view, "forall")
                || self.source.core(file, node, view, "exists")
                || self.source.core(file, node, view, "sum")))
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (t.instantiation == Instantiation::Parameter
                        || t.kind == TypeKind::Bool
                        || matches!(&t.kind, TypeKind::Array { element, .. }
                        if element.kind == TypeKind::Bool
                            || node.kind() == NodeKind::ArrayComprehension
                                && (element.kind == TypeKind::Int || decision_integer_set(element)
                                    || element.instantiation == Instantiation::Decision && matches!(element.kind, TypeKind::Enum(_))))
                        || node.kind() == NodeKind::GeneratorCallExpression
                            && self.source.core(file, node, view, "sum"))
            })
        {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Some(DefinitionSafety::Unsupported(
                    "parameter collection annotation is unsupported".into(),
                ));
            }
            return Some(self.collection_construction_safety(file, node, view, generators));
        }
        if node.kind() == NodeKind::ArrayComprehension {
            return Some(self.integer_comprehension_safety(file, node, view, generators));
        }
        None
    }
    fn parameter_relation_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        reason: String,
    ) -> DefinitionSafety {
        let node = unwrap(written);
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        let children: Vec<_> = node.child_nodes().collect();
        if !crate::definitions::annotations_safe(self.source.context, file, written) {
            return DefinitionSafety::Unsupported(
                "parameter collection operation annotation is unsupported".into(),
            );
        }
        let parameter_int = |node: &SyntaxNode| {
            ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            })
        };
        let parameter_set = |node: &SyntaxNode| {
            ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
            })
        };
        let parameter_bool = |node: &SyntaxNode| {
            ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Bool
            })
        };
        let checked_children = || {
            let mut safety = DefinitionSafety::Supported;
            let mut unsupported = None;
            for child in &children {
                match self.initialized_source_safety(file, child, view, generators, &mut Vec::new())
                {
                    DefinitionSafety::Unsupported(reason) => unsupported = Some(reason),
                    unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                    DefinitionSafety::Supported => {}
                }
            }
            match unsupported {
                Some(reason) => DefinitionSafety::Unsupported(reason),
                None => safety,
            }
        };
        if node.kind() == NodeKind::SetLiteral && children.iter().all(|n| parameter_int(n)) {
            return checked_children();
        }
        if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 {
            let subject = unwrap(children[0]);
            if subject.kind() == NodeKind::Expression && self.source.reference(file, subject).is_some()
                && parameter_int(children[1])
                && ty(subject).is_some_and(|t| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                            && Some(&element.kind) == ty(node).map(|t| &t.kind)))
            {
                return match checked_children() {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("parameter selection membership is unproved".into()),
                };
            }
        }
        let name =
            operator(self.source.context, file, node).and_then(crate::bindings::symbolic_operator);
        let supported = match node.kind() {
            NodeKind::RangeExpression => {
                children.len() == 2
                    && name == Some("..")
                    && children.iter().all(|n| parameter_int(n))
            }
            NodeKind::BinaryExpression if children.len() == 2 => match name {
                Some("intersect" | "union" | "diff") => children.iter().all(|n| parameter_set(n)),
                Some("in") => parameter_int(children[0]) && parameter_set(children[1]),
                Some("=" | "!=") => {
                    children.iter().all(|n| parameter_set(n))
                        || children.iter().all(|n| parameter_int(n))
                }
                Some("<" | "<=" | ">" | ">=") => children.iter().all(|n| parameter_int(n)),
                Some("/\\" | "\\/" | "<->" | "->" | "<-") => {
                    children.iter().all(|n| parameter_bool(n))
                }
                _ => false,
            },
            NodeKind::UnaryExpression => {
                children.len() == 1 && name == Some("not") && parameter_bool(children[0])
            }
            _ => false,
        };
        if supported && self.source.core(file, node, view, name.unwrap()) {
            return checked_children();
        }
        DefinitionSafety::Unsupported(reason)
    }
}
