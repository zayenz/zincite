//! Node-family traversal retains declaration ordering and proof-sensitive source prerequisites.
use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn reference_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let mut ids = Vec::new();
        if let Some(id) = self.source.reference(file, node)
            && self.source.bindings.declarations[id.0].role != DeclarationRole::Generator
        {
            let declaration = &self.source.bindings.declarations[id.0];
            if declaration.role == DeclarationRole::Local
                && declaration.syntax_range.end <= node.range().start
                && facts.declarations[id.0].ty.instantiation == Instantiation::Parameter
            {
                let local = find_node(
                    self.source.context.files[file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
                .ok_or("local declaration unavailable")?;
                if !crate::definitions::annotations_safe(self.source.context, file, local) {
                    return Err("local annotation evaluation is unsupported".into());
                }
                let value = local
                    .child_nodes()
                    .find(|n| is_expression(n.kind()))
                    .ok_or("local initializer unavailable")?;
                // An initializer keeps its declaration's lexical binders,
                // not a later generator that consumes this local value.
                let lexical: Vec<_> = generators
                    .iter()
                    .copied()
                    .filter(|g| g.range().end <= declaration.syntax_range.start)
                    .collect();
                return self.dependencies(file, value, view, &lexical);
            }
            ids.push(id);
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn selector_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        children: Vec<&'a SyntaxNode>,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        let subject = unwrap(children.first().ok_or("array access subject unavailable")?);
        if subject.kind() == NodeKind::ArrayLiteral && children.len() == 2 {
            let values: Vec<_> = subject.child_nodes().collect();
            let index = unwrap(children[1]);
            let parts: Vec<_> = index.child_nodes().map(unwrap).collect();
            let type_of = |node: &SyntaxNode| {
                self.source
                    .expression_type(self.source.view(file, node, facts), file, node)
                    .map(|e| &e.ty)
            };
            let one = parts.iter().position(|part| {
                part.kind() == NodeKind::Expression
                    && crate::domains::invariant_expression_integer(
                        self.source.context,
                        self.source.bindings,
                        file,
                        part,
                    )
                    .is_ok_and(|value| value == Some(1))
            });
            if values.len() != 2
                || values
                    .iter()
                    .any(|value| value.kind() == NodeKind::IndexedArrayEntry)
                || ty.is_none_or(|t| !matches!(t.kind, TypeKind::Bool | TypeKind::Int))
                || type_of(subject).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter
                            && Some(&element.kind) == ty.map(|t| &t.kind))
                })
                || values.iter().any(|value| {
                    type_of(value).is_none_or(|t| {
                        !t.known() || optional(t) || Some(&t.kind) != ty.map(|t| &t.kind)
                    })
                })
                || index.kind() != NodeKind::BinaryExpression
                || parts.len() != 2
                || !self.source.core(file, index, facts, "+")
                || type_of(index)
                    .is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
                || one.is_none_or(|position| {
                    let conversion = parts[1 - position];
                    conversion.kind() != NodeKind::CallExpression
                        || !self.source.core(file, conversion, facts, "bool2int")
                        || conversion.child_nodes().count() != 1
                })
            {
                return Err(
                    "literal array selection is not a supported two-value Boolean choice".into(),
                );
            }
            // The exact core conversion makes the index 1 or 2. Both
            // values and the selector remain forward dependencies.
            return self.child_dependencies(file, &children, view, generators);
        }
        if subject.kind() != NodeKind::Expression {
            return Err(
                "computed array access subject does not establish output dependencies".into(),
            );
        }
        let id = self
            .source
            .reference(file, subject)
            .ok_or("array access identity unavailable")?;
        if self.literal_bool_index_membership(file, node, id, facts) {
            // This exact closed index proves one scalar selection, not
            // whole-array coverage. Keep source and selector dependencies.
            return self.child_dependencies(file, &children, view, generators);
        }
        if matches!(&facts.declarations[id.0].ty.kind,
            TypeKind::Array { indices, .. }
                if matches!(indices.len(), 1 | 2) && indices.len() + 1 == children.len())
            && children[1..].iter().enumerate().all(|(axis, selector)| {
                self.source
                    .named_selector_membership(file, id, axis, selector, facts)
            })
        {
            // Every selector must match its own exact declared axis.
            // Evaluation and all forward selector dependencies remain required.
            if children.len() == 3
                && let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            let mut ids = vec![id];
            for selector in &children[1..] {
                extend(
                    &mut ids,
                    self.dependencies(file, selector, view, generators)?,
                );
            }
            return Ok(ids);
        }
        let supported = children.len() == 2
            && (self
                .source
                .member_index(file, id, children[1], generators, view)
                || self.asserted_index_membership(file, node, id, children[1], generators, facts)
                || self.source.array_bound_index(file, id, children[1], view)
                || self.shifted_index_membership(file, node, id, children[1], generators, facts))
            || self.written_index_membership(file, id, &children[1..], generators, facts)
            || complete_array_coverage(
                self.source.context,
                self.source.bindings,
                (self.source.calls, facts),
                self.source.instantiations,
                self.source.domains,
                (file, id),
                (&children[1..], generators),
            ) == DefinitionCoverage::WholeArray;
        if !supported {
            return Err(
                "output dependency array access has no exact traversal membership proof".into(),
            );
        }
        Ok(vec![id])
    }
    pub(in crate::callable_definitions) fn operator_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        children: Vec<&'a SyntaxNode>,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        let name = crate::bindings::symbolic_operator(
            operator(self.source.context, file, node).ok_or("operator unavailable")?,
        )
        .ok_or("operator unsupported")?;
        if !matches!(
            name,
            "=" | "!="
                | "+"
                | "-"
                | "*"
                | ".."
                | "<"
                | "<="
                | ">"
                | ">="
                | "in"
                | "diff"
                | "/\\"
                | "\\/"
                | "++"
        ) || !self.source.core(file, node, view, name)
        {
            return Err("output dependency operator identity or partiality is unsupported".into());
        }
        if name == "++" {
            let strings = ty.is_some_and(|t| {
                t.instantiation == Instantiation::Parameter && t.kind == TypeKind::String
            });
            let integers = ty.is_some_and(parameter_integers)
                && children.len() == 2
                && children.iter().all(|child| {
                    self.source
                        .expression_type(facts, file, child)
                        .is_some_and(|e| parameter_integers(&e.ty))
                });
            if !strings && !integers {
                // Keep strict dependencies limited to the existing integer shape.
                let Some(arguments) = ty
                    .filter(|t| {
                        matches!(&t.kind, TypeKind::Array { element, .. }
                        if element.kind == TypeKind::Int)
                    })
                    .and_then(|_| {
                        array_concatenation(
                            self.source.context,
                            self.source.bindings,
                            facts,
                            file,
                            node,
                        )
                    })
                else {
                    return Err(
                        "concatenation requires supported parameter strings or integer arrays"
                            .into(),
                    );
                };
                for argument in arguments {
                    match self.initialized_source_safety(
                        file,
                        argument,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Supported => {}
                        DefinitionSafety::Unknown(reason)
                        | DefinitionSafety::Unsupported(reason) => return Err(reason),
                    }
                }
            }
        }
        if name == "diff"
            && ty.is_none_or(|t| {
                t.instantiation != Instantiation::Parameter || !matches!(t.kind, TypeKind::Set(_))
            })
        {
            return Err("set difference requires supported parameter sets".into());
        }
        let scalar_comparison = matches!(name, "=" | "!=" | "<" | "<=" | ">" | ">=")
            && ty.is_some_and(|t| t.kind == TypeKind::Bool)
            && children.len() == 2
            && children.iter().all(|child| {
                self.source
                    .expression_type(facts, file, child)
                    .is_some_and(|e| {
                        e.ty.known()
                            && !optional(&e.ty)
                            && matches!(e.ty.kind, TypeKind::Int | TypeKind::Enum(_))
                    })
            });
        let range_membership = name == "in" && ty.is_some_and(|t| t.kind == TypeKind::Bool) && children.len() == 2
            && children.iter().enumerate().all(|(position, child)| {
                self.source.expression_type(facts, file, child).is_some_and(|e| {
                    e.ty.known() && !optional(&e.ty) && if position == 0 { e.ty.kind == TypeKind::Int }
                    else { e.ty.instantiation == Instantiation::Parameter && matches!(&e.ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int) }
                })
            });
        let mut ids = Vec::new();
        for child in &children {
            let dependencies = match self.dependencies(file, child, view, generators) {
                Ok(ids) => ids,
                Err(reason) if scalar_comparison || range_membership => self
                    .relational_operand_dependencies(file, child, view, generators)
                    .map_err(|_| reason)?,
                Err(reason) => return Err(reason),
            };
            extend(&mut ids, dependencies);
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn call_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        children: Vec<&'a SyntaxNode>,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        if self.source.core(file, node, view, "to_enum_internal") {
            let fallback = self.source.integer_conversion_fallback(file, node, view)?;
            return self.dependencies(file, fallback, view, generators);
        }
        if self.source.core(file, node, view, "assert") {
            let args = self.source.integer_assertion_arguments(file, node, view)?;
            let mut ids = Vec::new();
            for (file, argument) in &args[..2] {
                extend(
                    &mut ids,
                    self.dependencies(*file, argument, view, generators)?,
                );
            }
            if self.source.assertion_literal(args[0].0, args[0].1) != Some(TokenKind::True) {
                return Err("returning assertion condition is unproved".into());
            }
            extend(
                &mut ids,
                self.dependencies(args[2].0, args[2].1, view, generators)?,
            );
            return Ok(ids);
        }
        if self.source.core(file, node, view, "abs") {
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty.is_none_or(|t| t.kind != TypeKind::Int)
                || self.source.expression_type(facts, file, children[0]).is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Int)
                || self.source.operation_fact(facts, file, node).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                        if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                            && parameters[0].kind == TypeKind::Int)
                })
            {
                return Err("abs requires a supported present integer value".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "bool2int") {
            let argument = children
                .first()
                .and_then(|child| self.source.expression_type(facts, file, child));
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty.is_none_or(|t| t.kind != TypeKind::Int)
                || argument
                    .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
                || !self
                    .source
                    .operation_fact(facts, file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                        if parameters.len() == 1 && parameters[0].known()
                            && !optional(&parameters[0]) && parameters[0].kind == TypeKind::Bool)
                    })
            {
                return Err("bool2int requires a supported present Boolean value".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "arrayXd") {
            self.source
                .parameter_array_source(file, node, view)
                .ok_or("parameter reindexing lacks a same-source one-to-one traversal")?;
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "arg_min") {
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty.is_none_or(|t| {
                    t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                })
                || !self
                    .source
                    .operation_fact(facts, file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                        if parameters.len() == 1 && parameter_integers(&parameters[0]))
                    })
            {
                return Err("arg_min requires a supported parameter integer array".into());
            }
            let source = self
                .source
                .parameter_array_source(file, children[0], view)
                .ok_or("arg_min lacks a same-source parameter traversal")?;
            if !self.reflection_array_nonempty(file, node, source, view) {
                return Err("arg_min lacks a scoped nonempty source proof".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "dom") {
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty.is_none_or(|t| t.instantiation != Instantiation::Parameter
                    || !matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element) && element.kind == TypeKind::Int))
                || !self.source.asserted_integer_bounds(file, node, children[0], view)
            {
                return Err("scalar domain reflection lacks a same-operand assertion bounds proof".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if ["lb", "ub", "fix", "is_fixed", "lb_array", "ub_array"]
            .iter()
            .any(|name| self.source.core(file, node, view, name))
        {
            let argument = children
                .first()
                .and_then(|child| self.source.expression_type(facts, file, child));
            let array = self.source.core(file, node, view, "lb_array")
                || self.source.core(file, node, view, "ub_array");
            let result = if self.source.core(file, node, view, "is_fixed") {
                TypeKind::Bool
            } else {
                TypeKind::Int
            };
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty
                    .is_none_or(|t| t.kind != result || t.instantiation != Instantiation::Parameter)
                || argument.is_none_or(|e| {
                    if array {
                        !parameter_integers(&e.ty)
                    } else {
                        e.ty.kind != TypeKind::Int
                            || e.ty.instantiation != Instantiation::Parameter
                                && !(self.source.core(file, node, view, "lb")
                                    && self.source.asserted_integer_bounds(
                                        file,
                                        node,
                                        children[0],
                                        view,
                                    ))
                            || !e.ty.known()
                            || optional(&e.ty)
                    }
                })
            {
                return Err(
                    "reflection requires a supported present parameter integer operand".into(),
                );
            }
            if array && unwrap(children[0]).kind() != NodeKind::Expression {
                return Err("array bounds require a guarded bare array identity".into());
            }
            if array
                && !self
                    .source
                    .reference(file, unwrap(children[0]))
                    .is_some_and(|id| self.reflection_array_nonempty(file, node, id, view))
            {
                return Err("array bounds lack a scoped nonempty proof".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "sum") {
            let argument = children
                .first()
                .and_then(|child| self.source.expression_type(facts, file, child));
            if children.len() != 1
                || children[0].kind() == NodeKind::NamedArgument
                || ty.is_none_or(|t| {
                    t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                })
                || argument.is_none_or(|e| !parameter_integers(&e.ty))
                || !self
                    .source
                    .operation_fact(facts, file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                        if parameters.len() == 1 && parameter_integers(&parameters[0]))
                    })
            {
                return Err("sum requires a supported present parameter integer array".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "set2array") {
            let argument = children
                .first()
                .filter(|n| n.kind() != NodeKind::NamedArgument)
                .and_then(|n| self.source.expression_type(facts, file, n));
            if children.len() != 1
                || argument.is_none_or(|e| {
                    !e.ty.known()
                        || optional(&e.ty)
                        || e.ty.instantiation != Instantiation::Parameter
                        || !matches!(&e.ty.kind, TypeKind::Set(element)
                            if element.known() && !optional(element)
                                && element.instantiation == Instantiation::Parameter
                                && element.kind == TypeKind::Int)
                })
                || ty.is_none_or(|t| !parameter_integers(t))
                || !self.source.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1
                            && argument.is_some_and(|e| parameters[0] == e.ty)
                            && ty == Some(return_type))
                })
            {
                return Err("set2array requires an exact present parameter integer-set conversion".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, children[0], view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(file, children[0], true, true)
            {
                return Err(reason);
            }
            // Inspect the written source without inventing converted values or extent.
            return self.dependencies(file, children[0], view, generators);
        }
        if ["absent", "occurs"].iter().any(|name| {
            crate::optional::core_optional_call(
                self.source.context,
                self.source.bindings,
                facts,
                file,
                node,
                name,
            )
        }) {
            if children.len() != 1 {
                return Err("presence guard operand unavailable".into());
            }
            return self.source.presence_dependencies(file, children[0], view);
        }
        if self.source.core(file, node, view, "min") || self.source.core(file, node, view, "max") {
            if children.len() == 1 {
                let set = unwrap(children[0]);
                let args: Vec<_> = set.child_nodes().collect();
                if args.len() == 1
                    && unwrap(args[0]).kind() == NodeKind::Expression
                    && let Some(array) = self.source.reference(file, unwrap(args[0]))
                    && self.source.array_bound_index(file, array, node, view)
                {
                    return self.dependencies(file, args[0], view, generators);
                }
            } else if self
                .source
                .is_scalar_integer_extremum(file, written, view, &children)
            {
                return self.child_dependencies(file, &children, view, generators);
            }
            return Err(
                "extrema lack a supported scalar or guarded nonempty index-set proof".into(),
            );
        }
        if self.source.core(file, node, view, "length")
            && let Some(argument) =
                length_argument(self.source.context, self.source.bindings, facts, file, node)
            && self
                .source
                .expression_type(facts, file, argument)
                .is_some_and(|e| matches!(e.ty.kind, TypeKind::Set(_)))
        {
            // Inspect the original set; the selected array formal is
            // only the compiler's set2array matching view.
            match self.initialized_source_safety(file, argument, view, generators, &mut Vec::new())
            {
                DefinitionSafety::Supported => {
                    return self.dependencies(file, argument, view, generators);
                }
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
        }
        let metadata = if self.source.core(file, node, view, "show_index_sets") {
            Some((1, TypeKind::String))
        } else if self.source.core(file, node, view, "length") {
            Some((1, TypeKind::Int))
        } else if crate::optional::core_optional_call(
            self.source.context,
            self.source.bindings,
            facts,
            file,
            node,
            "index_sets_agree",
        ) {
            Some((2, TypeKind::Bool))
        } else {
            None
        };
        if let Some((arity, result)) = metadata {
            if children.len() != arity
                || ty
                    .is_none_or(|t| t.kind != result || t.instantiation != Instantiation::Parameter)
                || children.iter().any(|child| {
                    self.source
                        .expression_type(facts, file, child)
                        .is_none_or(|e| !matches!(e.ty.kind, TypeKind::Array { .. }))
                })
            {
                return Err("array metadata type or arity is unsupported".into());
            }
            if self.source.core(file, node, view, "length")
                && self
                    .source
                    .expression_type(facts, file, children[0])
                    .is_some_and(|e| {
                        e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.instantiation == Instantiation::Parameter
                            && matches!(&e.ty.kind, TypeKind::Array { indices, element }
                            if indices.len() == 3
                                && indices.iter().all(|axis| *axis == TypeInst::par(TypeKind::Int))
                                && **element == TypeInst::par(TypeKind::Int))
                    })
                && length_argument(self.source.context, self.source.bindings, facts, file, node)
                    .is_some()
            {
                // This matching view admits source inspection, never an output dependency.
                return Err("multidimensional array length is unproved".into());
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if self.source.core(file, node, view, "dom_array") {
            let array = children
                .first()
                .filter(|n| unwrap(n).kind() == NodeKind::Expression)
                .and_then(|n| self.source.reference(file, unwrap(n)))
                .ok_or("reflected array identity is unsupported")?;
            if children.len() != 1 || !self.source.finite_array_branch(file, node, array, view) {
                return Err(
                    "reflected array domains lack a complete guarded finite-bounds proof".into(),
                );
            }
            return self.child_dependencies(file, &children, view, generators);
        }
        if !(self.source.core(file, node, view, "array1d")
            || self.source.core(file, node, view, "enum2int")
            || self
                .source
                .array_conversion_argument(file, node, view)
                .is_some()
            || self.source.core(file, node, view, "index_set")
            || self.source.core(file, node, view, "has_bounds"))
        {
            return Err("arbitrary value calls do not prove output dependencies".into());
        }
        self.child_dependencies(file, &children, view, generators)
    }
    pub(in crate::callable_definitions) fn iteration_body_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        let parameter_collection = (node.kind() == NodeKind::SetComprehension
            && ty.is_some_and(|t| {
                t.instantiation == Instantiation::Parameter && matches!(t.kind, TypeKind::Set(_))
            }))
            || (node.kind() == NodeKind::ArrayComprehension && ty.is_some_and(parameter_integers));
        let quantifier = (self.source.core(file, node, view, "forall")
            || self.source.core(file, node, view, "exists")
            || self.source.core(file, node, view, "xorall"))
            && ty.is_some_and(|t| t.kind == TypeKind::Bool);
        let count = self.source.core(file, node, view, "count")
            && ty.is_some_and(|t| t.kind == TypeKind::Int)
            && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                        && matches!(&parameters[0].kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Bool))
            });
        let extrema = (self.source.core(file, node, view, "min")
            || self.source.core(file, node, view, "max"))
            && ty.is_some_and(|t| t.kind == TypeKind::Int)
            && self
                .source
                .operation_fact(facts, file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                        && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && element.kind == TypeKind::Int))
                });
        if !parameter_collection
            && !quantifier
            && !count
            && !extrema
            && !self.source.core(file, node, view, "sum")
        {
            return Err("output dependency iteration is unsupported".into());
        }
        let list = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
            .ok_or("sum generators unavailable")?;
        let mut all = generators.to_vec();
        all.extend(list.child_nodes());
        if parameter_collection || quantifier || count || extrema {
            let decision_quantifier = quantifier && self.source.core(file, node, view, "exists") && list.child_nodes().all(|generator| {
                let Some(source) = generator.child_nodes().next() else { return false; };
                self.source.expression_type(self.source.view(file, source, view), file, source).is_some_and(|e| {
                    e.ty.instantiation != Instantiation::Decision
                        || matches!(&e.ty.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                && indices[0].instantiation == Instantiation::Parameter
                                && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
                                && element.known() && !optional(element) && element.kind == TypeKind::Int)
                })
            });
            self.relation_iterations(
                file,
                &all,
                generators.len(),
                view,
                count || decision_quantifier,
            )?;
        } else {
            self.source.iterations(file, &all, view)?;
        }
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
            .ok_or("sum body unavailable")?;
        if quantifier
            && self
                .source
                .expression_type(facts, file, body)
                .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
        {
            return Err("quantifier requires a supported present Boolean body".into());
        }
        let selected_parameter_sum = node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(file, node, view, "sum")
            && self
                .source
                .operation_fact(facts, file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameter_integers(&parameters[0])
                        && call.generator_argument.as_ref() == Some(&parameters[0])
                        && return_type.known() && !optional(return_type)
                        && return_type.instantiation == Instantiation::Parameter
                        && return_type.kind == TypeKind::Int && ty == Some(return_type))
                });
        let mut ids = self.dependencies(file, body, view, &all)?;
        for (position, g) in list.child_nodes().enumerate() {
            let position = generators.len() + position;
            // Dependency identity does not inspect a selected set's initializer.
            if selected_parameter_sum
                && let Some(DefinitionSafety::Unsupported(reason)) = self
                    .source
                    .selected_generator_source_safety(file, g, view, &all[..position])
            {
                return Err(reason);
            }
            extend(
                &mut ids,
                self.dependencies(
                    file,
                    g.child_nodes().next().ok_or("sum domain unavailable")?,
                    view,
                    &all[..position],
                )?,
            );
            for filter in g
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::WhereFilter)
            {
                let condition = filter
                    .child_nodes()
                    .next()
                    .ok_or("iteration filter unavailable")?;
                extend(
                    &mut ids,
                    self.dependencies(file, condition, view, &all[..=position])?,
                );
            }
        }
        if extrema
            && !list.child_nodes().all(|g| {
                !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                    && self.source.nonempty_source(file, g)
            })
        {
            return Err("integer extrema iteration may be empty".into());
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn let_dependencies(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        children: Vec<&'a SyntaxNode>,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(written);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        if ty
            .is_none_or(|t| t.kind != TypeKind::Bool || t.instantiation != Instantiation::Parameter)
            || children.len() != 2
            || children[0].kind() != NodeKind::LetBlock
        {
            return Err("value let requires a supported present parameter Boolean result".into());
        }
        let mut ids = Vec::new();
        let mut locals = Vec::new();
        for local in children[0].child_nodes() {
            let declaration = self.source.bindings.declarations.iter().find(|d| {
                d.file == file
                    && d.syntax_range == local.range()
                    && d.role == DeclarationRole::Local
            });
            let Some(declaration) = declaration.filter(|_| local.kind() == NodeKind::Declaration)
            else {
                return Err("value let supports initialized parameter declarations only".into());
            };
            let local_type = &facts.declarations[declaration.id.0].ty;
            let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
            if !local_type.known()
                || optional(local_type)
                || local_type.instantiation != Instantiation::Parameter
                || initializer.is_none()
                || !crate::definitions::annotations_safe(self.source.context, file, local)
            {
                return Err(
                    "value let declaration, initializer or annotation is unsupported".into(),
                );
            }
            locals.push(declaration.id);
            extend(
                &mut ids,
                self.type_dependencies(file, local, view, generators)?,
            );
            extend(
                &mut ids,
                self.dependencies(file, initializer.unwrap(), view, generators)?,
            );
        }
        extend(
            &mut ids,
            self.dependencies(file, children[1], view, generators)?,
        );
        if ids.iter().any(|id| locals.contains(id)) {
            return Err("value let has an unsupported forward or cyclic local dependency".into());
        }
        Ok(ids)
    }
}
