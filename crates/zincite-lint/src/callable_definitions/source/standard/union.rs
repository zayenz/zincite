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
    // Finite output checks require the selected standard primitive declaration.
    // A core name does not waive a written body, default or domain.
    pub(in crate::callable_definitions) fn standard_primitive_safety(
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

    // Source views require bodyless selected primitives with checked metadata.
    // Validate each annotation before skipping its subtree.
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
        view: &crate::callables::CallableView<'_>,
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
                                // Retain the selected native set2array matching view only.
                                let set_array_view = e.ty.instantiation == Instantiation::Parameter
                                    && parameter.instantiation == Instantiation::Parameter
                                    && !optional(parameter)
                                    && matches!((&e.ty.kind, &parameter.kind),
                                        (TypeKind::Set(element), TypeKind::Array { indices, element: selected })
                                            if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                                                && element.instantiation == Instantiation::Parameter
                                                && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                                                && element == selected);
                                !e.ty.known()
                                    || optional(&e.ty)
                                    || !(crate::types::coerces(&e.ty, parameter) || set_array_view)
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
}

impl<'a> BodyInterpreter<'a, '_> {
    // Required value-body inspection belongs to source admission, independent
    // of Boolean forwarding. The local failure also retains its written location.

    // This is typed source inspection of the selected union/FZN written family.
    // Visible IDs are checked owning formals, preceding locals and integer binders;
    // they carry neither a known value nor a definition/output guarantee.

    // This selected value body supplies source safety only. Neither its local
    // bound nor set_card proves the actual set's cardinality or any output.
}
