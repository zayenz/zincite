//! Integer selectors and operations retain eager errors and strict refusal reasons.
use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn integer_selector_safety(
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
        let Some(subject) = children.first().copied().map(unwrap) else {
            return DefinitionSafety::Unsupported(reason);
        };
        let Some(array) = ty(subject) else {
            return DefinitionSafety::Unsupported(reason);
        };
        let TypeKind::Array { indices, element } = &array.kind else {
            return DefinitionSafety::Unsupported(reason);
        };
        if subject.kind() != NodeKind::Expression
            || self.source.reference(file, subject).is_none()
            || !array.known()
            || optional(array)
            || element.kind != TypeKind::Int
            || !matches!(indices.len(), 1..=3)
            || children.len() != indices.len() + 1
            || indices
                .iter()
                .zip(&children[1..])
                .enumerate()
                .any(|(position, (index, actual))| {
                    !index.known()
                        || optional(index)
                        || index.instantiation != Instantiation::Parameter
                        || ty(actual).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || (t.instantiation != Instantiation::Parameter
                                    && !(indices.len() == 3
                                        && position == 2
                                        && t.instantiation == Instantiation::Decision))
                                || t.kind != index.kind
                        })
                })
        {
            return DefinitionSafety::Unsupported(reason);
        }
        if indices.len() == 3 {
            let safety = self.direct_children_safety(file, &children, view, generators);
            if safety != DefinitionSafety::Supported {
                return safety;
            }
            let array = self.source.reference(file, subject).unwrap();
            if self.decision_selector_membership(file, array, &children[1..], view, generators) {
                return DefinitionSafety::Supported;
            }
            return DefinitionSafety::Unknown("numeric array index membership is unproved".into());
        }
        // A checked enum successor is a source inspection, not an
        // index-membership proof for this array.
        if indices.iter().zip(&children[1..]).any(|(axis, selector)| {
            matches!(axis.kind, TypeKind::Enum(_))
                && self
                    .parameter_enum_successor_safety(file, selector, view, generators)
                    .is_some()
        }) {
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || ty(node).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || t.kind != TypeKind::Int
                        || t.instantiation != element.instantiation
                })
            {
                return DefinitionSafety::Unsupported(
                    "enum successor selection type or annotation is unsupported".into(),
                );
            }
            let array_id = self.source.reference(file, subject).unwrap();
            let owner = &self.source.bindings.declarations[array_id.0];
            if owner.role != DeclarationRole::Value
                || !owner.top_level
                || self.source.calls.declarations[array_id.0].ty != *array
            {
                return DefinitionSafety::Unsupported(
                    "enum successor array source identity is unsupported".into(),
                );
            }
            let Some(source) = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            ) else {
                return DefinitionSafety::Unsupported(
                    "enum successor array source unavailable".into(),
                );
            };
            if let unsupported @ DefinitionSafety::Unsupported(_) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return unsupported;
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(owner.file, source, true, true)
            {
                return DefinitionSafety::Unsupported(reason);
            }
            for selector in &children[1..] {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, selector, true, true)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
            }
            if let Domain::Array { indices, .. } =
                &self.source.domains.declarations[array_id.0].domain
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
                return DefinitionSafety::Unsupported(
                    "scalar selection is outside its declared index set".into(),
                );
            }
            return DefinitionSafety::Unknown("numeric array index membership is unproved".into());
        }
        if let Err(reason) = self.child_dependencies(file, &children, view, generators) {
            return DefinitionSafety::Unsupported(reason);
        }
        DefinitionSafety::Unknown("numeric array index membership is unproved".into())
    }
    pub(in crate::callable_definitions) fn integer_operator_safety(
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
        let name =
            operator(self.source.context, file, node).and_then(crate::bindings::symbolic_operator);
        if name == Some("*")
            && children.len() == 2
            && children
                .iter()
                .any(|n| ty(n).is_some_and(|t| t.kind == TypeKind::Bool))
            && self.source.core(file, node, view, "*")
            && crate::definitions::annotations_safe(self.source.context, file, written)
            && children.iter().all(|n| {
                ty(n).is_some_and(|t| {
                    t.known() && !optional(t) && matches!(t.kind, TypeKind::Int | TypeKind::Bool)
                })
            })
            && self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && return_type.known() && !optional(return_type)
                        && return_type.kind == TypeKind::Int && Some(return_type) == ty(node)
                        && parameters.iter().zip(&children).all(|(formal, actual)| formal.known()
                            && !optional(formal) && formal.kind == TypeKind::Int
                            && ty(actual).is_some_and(|t| crate::types::coerces(t, formal))))
                })
        {
            return match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("coerced multiplication value is unproved".into()),
            };
        }
        if !matches!(name, Some("+" | "-" | "*" | "div" | "mod"))
            || children.len() != 2
            || !self.source.core(file, node, view, name.unwrap())
            || children
                .iter()
                .any(|n| ty(n).is_none_or(|t| t.kind != TypeKind::Int || !t.known() || optional(t)))
        {
            return DefinitionSafety::Unsupported(reason);
        }
        if matches!(name, Some("div" | "mod")) {
            let integer = |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || self
                    .source
                    .operation_fact(self.source.view(file, node, view), file, node)
                    .is_none_or(|call| {
                        !matches!(&call.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters.iter().all(integer)
                            && integer(return_type))
                    })
            {
                return DefinitionSafety::Unsupported(reason);
            }
            if self
                .source
                .inactive_integer_body
                .as_ref()
                .is_some_and(|(f, range)| {
                    *f == file && range.start <= node.range().start && node.range().end <= range.end
                })
            {
                return match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown(
                        "inactive parameter integer value is unproved".into(),
                    ),
                };
            }
            let mut safety = DefinitionSafety::Supported;
            let mut unsupported = None;
            let mut values = Vec::new();
            let mut inspected_extremum = false;
            for (position, child) in children.iter().enumerate() {
                let inspected =
                    self.initialized_source_safety(file, child, view, generators, &mut Vec::new());
                // The typed source inspector checks the array and every
                // selector; membership remains unproved. The independent
                // integer interpreter has no array-selection evaluator.
                let symbolic_selection = unwrap(child).kind() == NodeKind::ArrayAccessExpression
                    && !matches!(&inspected, DefinitionSafety::Unsupported(_))
                    && ty(child).is_some_and(|t| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && matches!(
                                t.instantiation,
                                Instantiation::Parameter | Instantiation::Decision
                            )
                    });
                let wrapper = unwrap(child);
                let wrapper_name = operator(self.source.context, file, wrapper)
                    .and_then(crate::bindings::symbolic_operator);
                let operands: Vec<_> = wrapper.child_nodes().collect();
                // Only one inspected selection plus/minus a closed integer
                // can retain an unproved value. A failing closed sibling is
                // left to the independent evaluator, including overflow.
                let wrapped_selection = !matches!(&inspected, DefinitionSafety::Unsupported(_))
                    && wrapper.kind() == NodeKind::BinaryExpression
                    && matches!(wrapper_name, Some("+" | "-"))
                    && operands.len() == 2
                    && self.source.core(file, wrapper, view, wrapper_name.unwrap())
                    && ty(wrapper).is_some_and(integer)
                    && self
                        .source
                        .operation_fact(self.source.view(file, wrapper, view), file, wrapper)
                        .is_some_and(|call| {
                            matches!(&call.outcome,
                            CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2 && parameters.iter().all(integer)
                                    && Some(return_type) == ty(wrapper))
                        })
                    && operands.iter().enumerate().any(|(position, operand)| {
                        unwrap(operand).kind() == NodeKind::ArrayAccessExpression
                            && ty(operand).is_some_and(|t| {
                                integer(t)
                                    && matches!(
                                        t.instantiation,
                                        Instantiation::Parameter | Instantiation::Decision
                                    )
                            })
                            && crate::domains::invariant_expression_integer(
                                self.source.context,
                                self.source.bindings,
                                file,
                                operands[1 - position],
                            )
                            .is_ok_and(|value| value.is_some())
                    });
                // These exact primitives have already had every source
                // inspected. The invariant interpreter does not evaluate
                // set extrema; retain only closed error vetoes here.
                let parameter = TypeInst::par(TypeKind::Int);
                let set = TypeInst::par(TypeKind::Set(Box::new(parameter.clone())));
                let extremum = wrapper.kind() == NodeKind::CallExpression
                    && ty(wrapper) == Some(&parameter)
                    && operands.len() == 1
                    && ty(operands[0]) == Some(&set)
                    && (self.source.core(file, wrapper, view, "min")
                        || self.source.core(file, wrapper, view, "max"));
                // A nested quotient/remainder is inspected only as the
                // dividend. A computed divisor still needs the independent
                // evaluator, which can establish a concrete zero.
                let nested_dividend = position == 0
                    && matches!(&inspected, DefinitionSafety::Unknown(_))
                    && wrapper.kind() == NodeKind::BinaryExpression
                    && matches!(wrapper_name, Some("div" | "mod"))
                    && self.source.prefix_primitive(
                        file,
                        wrapper,
                        self.source.view(file, wrapper, view),
                        wrapper_name.unwrap(),
                        &[parameter.clone(), parameter.clone()],
                        &parameter,
                    );
                if (extremum || nested_dividend)
                    && !matches!(&inspected, DefinitionSafety::Unsupported(_))
                {
                    if !self.source.prefix_primitive(
                        file,
                        node,
                        self.source.view(file, node, view),
                        name.unwrap(),
                        &[parameter.clone(), parameter.clone()],
                        &parameter,
                    ) || extremum
                        && !self.source.prefix_primitive(
                            file,
                            wrapper,
                            self.source.view(file, wrapper, view),
                            if self.source.core(file, wrapper, view, "min") {
                                "min"
                            } else {
                                "max"
                            },
                            std::slice::from_ref(&set),
                            &parameter,
                        )
                    {
                        unsupported = Some(
                            "integer division/remainder selected written primitive is unsupported"
                                .into(),
                        );
                        values.push(None);
                        continue;
                    }
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error_with_branches(file, child, false, true, true)
                    {
                        unsupported = Some(reason);
                        values.push(None);
                        continue;
                    }
                    safety = DefinitionSafety::Unknown(
                        "integer division/remainder operands are unproved".into(),
                    );
                    if extremum {
                        inspected_extremum = true;
                        let interval = crate::domains::index_domain_interval(&expression_domain(
                            self.source.context,
                            self.source.bindings,
                            file,
                            operands[0],
                        ));
                        values.push(interval.map(|(lower, upper)| {
                            if self.source.core(file, wrapper, view, "min") {
                                lower
                            } else {
                                upper
                            }
                        }));
                    } else {
                        values.push(None);
                    }
                    continue;
                }
                match inspected {
                    DefinitionSafety::Unsupported(reason) => {
                        unsupported = Some(reason);
                        values.push(None);
                        continue;
                    }
                    unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                    DefinitionSafety::Supported => {}
                }
                // A selected length has already had its original source
                // inspected above. The definitionless integer interpreter
                // cannot evaluate this call; retain an unproved value.
                let inspected_length = length_argument(
                    self.source.context,
                    self.source.bindings,
                    self.source.view(file, child, view),
                    file,
                    unwrap(child),
                )
                .is_some();
                if symbolic_selection || wrapped_selection || inspected_length {
                    if inspected_length
                        && let Some(reason) = self.source.closed_integer_source_error_with_branches(
                            file, child, false, true, true,
                        )
                    {
                        unsupported = Some(reason);
                    }
                    values.push(None);
                    continue;
                }
                // Inspect each operand independently: an unknown dividend
                // cannot hide a literal zero divisor or invalid sibling.
                match crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    child,
                ) {
                    Ok(value) => values.push(value),
                    Err(reason) => {
                        unsupported = Some(reason);
                        values.push(None);
                    }
                }
            }
            if values[1] == Some(0) {
                unsupported = Some("integer division by zero".into());
            }
            if let Some(reason) = unsupported {
                return DefinitionSafety::Unsupported(reason);
            }
            if let (Some(left), Some(right)) = (values[0], values[1]) {
                if inspected_extremum {
                    let checked = if name == Some("div") {
                        left.checked_div(right)
                    } else {
                        left.checked_rem(right)
                    };
                    if checked.is_none() {
                        return DefinitionSafety::Unsupported("integer arithmetic overflow".into());
                    }
                    return safety;
                }
                match crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    node,
                ) {
                    Ok(Some(_)) => return safety,
                    Err(reason) => return DefinitionSafety::Unsupported(reason),
                    Ok(None) => {}
                }
            }
            return DefinitionSafety::Unknown(
                "integer division/remainder operands are unproved".into(),
            );
        }
        self.direct_children_safety(file, &children, view, generators)
    }
    pub(in crate::callable_definitions) fn integer_call_safety(
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
        let parameter_integer = TypeInst::par(TypeKind::Int);
        let parameter_boolean = TypeInst::par(TypeKind::Bool);
        if self.source.core(file, node, view, "bool2int")
            && children.len() == 1
            && ty(node) == Some(&parameter_integer)
            && ty(children[0]) == Some(&parameter_boolean)
        {
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || !self.source.prefix_primitive(
                    file,
                    node,
                    self.source.view(file, node, view),
                    "bool2int",
                    std::slice::from_ref(&parameter_boolean),
                    &parameter_integer,
                )
                || !self
                    .source
                    .operation_fact(self.source.view(file, node, view), file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { declaration, .. }
                        if self.source.calls.signatures.iter().any(|signature|
                            signature.declaration == *declaration
                                && signature.parameters.len() == 1
                                && !signature.parameters[0].has_default))
                    })
            {
                return DefinitionSafety::Unsupported(
                    "Boolean integer conversion selected written primitive is unsupported".into(),
                );
            }
            return match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => {
                    DefinitionSafety::Unknown("Boolean integer conversion value is unproved".into())
                }
            };
        }
        if self.source.core(file, node, view, "pow") {
            let base_int = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Int
                    && matches!(
                        t.instantiation,
                        Instantiation::Parameter | Instantiation::Decision
                    )
            };
            let parameter_int = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Int
                    && t.instantiation == Instantiation::Parameter
            };
            if children.len() != 2
                || children.iter().any(|child| child.kind() == NodeKind::NamedArgument)
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || ty(children[0]).is_none_or(|t| !base_int(t))
                || ty(children[1]).is_none_or(|t| !parameter_int(t))
                || ty(node).is_none_or(|t| !base_int(t) || ty(children[0]) != Some(t))
                || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && base_int(&parameters[0])
                            && parameter_int(&parameters[1]) && Some(&parameters[0]) == ty(children[0])
                            && Some(&parameters[1]) == ty(children[1]) && Some(return_type) == ty(node))
                })
            {
                return DefinitionSafety::Unsupported(reason);
            }
            if let unsupported @ DefinitionSafety::Unsupported(_) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return unsupported;
            }
            return match crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                file,
                children[1],
            ) {
                Ok(Some(exponent)) if exponent >= 0 => {
                    DefinitionSafety::Unknown("integer power value is unproved".into())
                }
                Err(reason) => DefinitionSafety::Unsupported(reason),
                _ => DefinitionSafety::Unsupported(
                    "integer power requires a closed nonnegative parameter exponent".into(),
                ),
            };
        }
        if parameter_index_extremum(
            self.source.context,
            self.source.bindings,
            self.source.view(file, node, view),
            file,
            node,
        )
        .is_some()
        {
            let source = node
                .child_nodes()
                .next()
                .and_then(|set| unwrap(set).child_nodes().next());
            let Some(source) = source else {
                return DefinitionSafety::Unsupported(reason);
            };
            if let Err(reason) = self.dependencies(file, source, view, generators) {
                return DefinitionSafety::Unsupported(reason);
            }
            return DefinitionSafety::Unknown(
                "integer index-set extremum may be undefined for an empty array".into(),
            );
        }
        if self.source.core(file, node, view, "to_enum_internal") {
            return match self.source.integer_conversion_fallback(file, node, view) {
                Ok(fallback) => self.direct_safety(file, fallback, view, generators),
                Err(reason) => DefinitionSafety::Unsupported(reason),
            };
        }
        if self.source.core(file, node, view, "assert") {
            let args = match self.source.integer_assertion_arguments(file, node, view) {
                Ok(args) => args,
                Err(reason) => return DefinitionSafety::Unsupported(reason),
            };
            for (file, argument) in &args[..2] {
                if let Err(reason) = self.dependencies(*file, argument, view, generators) {
                    return DefinitionSafety::Unsupported(reason);
                }
            }
            let literal = self.source.assertion_literal(args[0].0, args[0].1);
            if literal == Some(TokenKind::False) {
                return DefinitionSafety::Unsupported(
                    "false assertion condition aborts evaluation".into(),
                );
            }
            let safety = self.direct_safety(args[2].0, args[2].1, view, generators);
            if literal == Some(TokenKind::True)
                || matches!(safety, DefinitionSafety::Unsupported(_))
            {
                return safety;
            }
            return DefinitionSafety::Unknown("returning assertion condition is unproved".into());
        }
        if self.source.core(file, node, view, "abs") {
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty(children[0]).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
                || self.source.operation_fact(self.source.view(file, node, view), file, node)
                    .is_none_or(|call| {
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                            if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                && parameters[0].kind == TypeKind::Int)
                    })
            {
                return DefinitionSafety::Unsupported(reason);
            }
            return self.direct_children_safety(file, &children, view, generators);
        }
        if self
            .source
            .is_scalar_integer_extremum(file, written, view, &children)
        {
            return self.direct_children_safety(file, &children, view, generators);
        }
        let integer_array = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && matches!(&t.kind,
                TypeKind::Array { indices, element } if indices.len() == 1
                    && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter
                    && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Int)
        };
        if !(self.source.core(file, node, view, "min") || self.source.core(file, node, view, "max"))
            || children.len() != 1
            || children[0].kind() == NodeKind::NamedArgument
            || ty(children[0]).is_none_or(|t| !integer_array(t))
            || self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .is_none_or(|c| {
                    !matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && integer_array(&parameters[0]))
                })
        {
            return DefinitionSafety::Unsupported(reason);
        }
        let collection = unwrap(children[0]);
        let values: Vec<_> = collection.child_nodes().collect();
        let (safety, nonempty) = match collection.kind() {
            NodeKind::Expression => {
                if let Some(safety) =
                    self.conditional_integer_min_source_safety(file, written, collection, view)
                {
                    return safety;
                }
                let safety = match self.dependencies(file, collection, view, generators) {
                    Ok(_) => DefinitionSafety::Supported,
                    Err(reason) => DefinitionSafety::Unsupported(reason),
                };
                let nonempty = self
                    .source
                    .reference(file, collection)
                    .is_some_and(|array| self.reflection_array_nonempty(file, node, array, view));
                (safety, nonempty)
            }
            NodeKind::ArrayLiteral => (
                self.direct_children_safety(file, &values, view, generators),
                !values.is_empty(),
            ),
            NodeKind::BinaryExpression => {
                if !crate::definitions::annotations_safe(self.source.context, file, written)
                    || !crate::definitions::annotations_safe(self.source.context, file, children[0])
                    || self.source.operation_fact(self.source.view(file, node, view), file, node)
                        .is_none_or(|call| !matches!(&call.outcome,
                            CallOutcome::Resolved { return_type, .. }
                                if return_type.known() && !optional(return_type)
                                    && return_type.kind == TypeKind::Int && Some(return_type) == ty(node)))
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let Some(arguments) = array_concatenation(
                    self.source.context,
                    self.source.bindings,
                    self.source.view(file, collection, view),
                    file,
                    collection,
                ) else {
                    return DefinitionSafety::Unsupported(reason);
                };
                let mut unsupported = None;
                for argument in arguments {
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        argument,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        unsupported = Some(reason);
                    }
                }
                // Source inspection gives neither extent nor extrema value proof.
                return match unsupported {
                    Some(reason) => DefinitionSafety::Unsupported(reason),
                    None => DefinitionSafety::Unknown(
                        "integer extrema concatenation extent and value are unproved".into(),
                    ),
                };
            }
            NodeKind::ArrayComprehension => {
                let Some(list) = values.iter().find(|n| n.kind() == NodeKind::GeneratorList) else {
                    return DefinitionSafety::Unsupported(reason);
                };
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                if let Err(reason) =
                    self.relation_iterations(file, &all, generators.len(), view, false)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let Some(body) = values.iter().find(|n| n.kind() != NodeKind::GeneratorList) else {
                    return DefinitionSafety::Unsupported(reason);
                };
                if ty(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int) {
                    return DefinitionSafety::Unsupported(reason);
                }
                let nonempty = list.child_nodes().all(|g| {
                    !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                        && self.source.nonempty_source(file, g)
                });
                (self.direct_safety(file, body, view, &all), nonempty)
            }
            _ => return DefinitionSafety::Unsupported(reason),
        };
        if safety == DefinitionSafety::Supported && !nonempty {
            DefinitionSafety::Unknown("integer extrema collection may be empty".into())
        } else {
            safety
        }
    }
    pub(in crate::callable_definitions) fn integer_generator_safety(
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
        let extrema =
            self.source.core(file, node, view, "min") || self.source.core(file, node, view, "max");
        if !(extrema || self.source.core(file, node, view, "sum"))
            || self.source.operation_fact(self.source.view(file, node, view), file, node)
                .is_none_or(|c| {
                    !matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                        if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && element.kind == TypeKind::Int))
                })
        {
            return DefinitionSafety::Unsupported(reason);
        }
        let Some(list) = children
            .iter()
            .find(|n| n.kind() == NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported(reason);
        };
        let Some(body) = children
            .iter()
            .find(|n| n.kind() != NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported(reason);
        };
        if ty(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int) {
            return DefinitionSafety::Unsupported(reason);
        }
        let mut all = generators.to_vec();
        all.extend(list.child_nodes());
        if let Err(reason) = self.relation_iterations(file, &all, generators.len(), view, false) {
            return DefinitionSafety::Unsupported(reason);
        }
        let safety = self.direct_safety(file, body, view, &all);
        if extrema
            && safety == DefinitionSafety::Supported
            && !list.child_nodes().all(|g| {
                !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                    && self.source.nonempty_source(file, g)
            })
        {
            DefinitionSafety::Unknown("integer extrema iteration may be empty".into())
        } else {
            safety
        }
    }
}
