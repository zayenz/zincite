use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn array_conversion_argument(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<&'a SyntaxNode> {
        let node = unwrap(node);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let facts = self.view(file, node, view);
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &self.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let name = &self.bindings.declarations[declaration.0].name;
        if !matches!(name.as_str(), "enum2int" | "index2int")
            || !crate::definitions::core_callable(self.context, self.bindings, *declaration, name)
        {
            return None;
        }
        let mut children = node.child_nodes();
        let argument = children.next()?;
        if children.next().is_some() || argument.kind() == NodeKind::NamedArgument {
            return None;
        }
        let input = &self
            .expression_type(self.view(file, argument, view), file, argument)?
            .ty;
        let result = &self.expression_type(facts, file, node)?.ty;
        if !input.known()
            || optional(input)
            || parameters.as_slice() != std::slice::from_ref(input)
            || return_type != result
        {
            return None;
        }
        let mut expected = input.clone();
        let TypeKind::Array { indices, element } = &mut expected.kind else {
            return None;
        };
        if indices.len() != 1
            || indices[0].instantiation != Instantiation::Parameter
            || !matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
        {
            return None;
        }
        if name == "enum2int" {
            if !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)) {
                return None;
            }
            element.kind = TypeKind::Int;
        } else {
            indices[0] = TypeInst::par(TypeKind::Int);
        }
        // These standard views retain every value; only the enum representation
        // or index type changes. Keep all other qualifiers and identities exact.
        (expected == *result).then_some(argument)
    }
    pub(in crate::callable_definitions) fn nonempty_source(
        &self,
        file: FileId,
        generator: &SyntaxNode,
    ) -> bool {
        let Some(source) = generator.child_nodes().next() else {
            return false;
        };
        match expression_domain(self.context, self.bindings, file, source) {
            crate::Domain::Range { lower, upper } => {
                matches!((crate::domains::invariant_integer(&lower), crate::domains::invariant_integer(&upper)), (Ok(Some(lower)), Ok(Some(upper))) if lower <= upper)
            }
            crate::Domain::LiteralSet(values) => {
                !values.is_empty()
                    && values
                        .iter()
                        .all(|v| matches!(crate::domains::invariant_integer(v), Ok(Some(_))))
            }
            _ => false,
        }
    }
    pub(in crate::callable_definitions) fn length_equals(
        &self,
        file: FileId,
        guard: &SyntaxNode,
        array: DeclarationId,
        value: i64,
        view: &CallableFacts,
    ) -> bool {
        let guard = unwrap(guard);
        let facts = self.view(file, guard, view);
        let range = self.context.files[file].location(guard.range()).range;
        if !self.core(file, guard, view, "=")
            || !facts.expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.kind == TypeKind::Bool
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
            })
        {
            return false;
        }
        let parts: Vec<_> = guard.child_nodes().collect();
        if parts.len() != 2 {
            return false;
        }
        [(parts[0], parts[1]), (parts[1], parts[0])]
            .into_iter()
            .any(|(length, amount)| {
                let length = unwrap(length);
                let args: Vec<_> = length.child_nodes().collect();
                self.core(file, length, view, "length")
                    && args.len() == 1
                    && unwrap(args[0]).kind() == NodeKind::Expression
                    && self.reference(file, unwrap(args[0])) == Some(array)
                    && crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        amount,
                    ) == Ok(Some(value))
            })
    }
    pub(in crate::callable_definitions) fn array_bound_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let facts = self.view(file, index, view);
        let array_type = &facts.declarations[array.0].ty;
        let TypeKind::Array { indices, .. } = &array_type.kind else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || indices.len() != 1
            || !matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
        {
            return false;
        }
        let parameter = |node: &SyntaxNode, kind: &TypeKind| {
            let range = self.context.files[file].location(node.range()).range;
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
                    && &e.ty.kind == kind
            })
        };
        let args: Vec<_> = index.child_nodes().collect();
        if !(self.core(file, index, view, "min") || self.core(file, index, view, "max"))
            || args.len() != 1
            || !parameter(index, &indices[0].kind)
        {
            return false;
        }
        let set = unwrap(args[0]);
        let subjects: Vec<_> = set.child_nodes().collect();
        if !self.core(file, set, view, "index_set")
            || subjects.len() != 1
            || unwrap(subjects[0]).kind() != NodeKind::Expression
            || self.reference(file, unwrap(subjects[0])) != Some(array)
        {
            return false;
        }
        self.array_nonempty_branch(file, index, array, view)
    }
    pub(in crate::callable_definitions) fn array_nonempty_branch(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut containing = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            let parameter_bool = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && ty.kind == TypeKind::Bool
            };
            if node.kind() == NodeKind::BinaryExpression
                && children.len() == 2
                && contains(children[1])
                && self.core(file, node, view, "\\/")
                && self
                    .operation_fact(self.view(file, node, view), file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome,
                            CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(parameter_bool)
                                && parameter_bool(return_type))
                    })
                && self.length_equals(file, children[0], array, 0, view)
            {
                // This ordered parameter OR evaluates its right operand only after
                // the exact same-array zero-length test has returned false.
                return true;
            }
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
                && self
                    .view(file, unwrap(children[0]), view)
                    .expressions
                    .iter()
                    .any(|e| {
                        e.file == file
                            && e.location.range
                                == self.context.files[file]
                                    .location(unwrap(children[0]).range())
                                    .range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.instantiation == Instantiation::Parameter
                            && e.ty.kind == TypeKind::Bool
                    })
            {
                let mut conjuncts = vec![unwrap(children[0])];
                while let Some(guard) = conjuncts.pop() {
                    let parts: Vec<_> = guard.child_nodes().collect();
                    if parts.len() == 2 && self.core(file, guard, view, "/\\") {
                        conjuncts.extend(parts.into_iter().map(unwrap));
                    } else if self.length_equals(file, guard, array, 1, view) {
                        return true;
                    }
                }
            }
            if node.kind() == NodeKind::ConditionalExpression {
                let mut nonempty = false;
                for branch in &children {
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let body = match branch.kind() {
                        NodeKind::ConditionalBranch if parts.len() == 2 => parts[1],
                        NodeKind::ElseBranch if parts.len() == 1 => parts[0],
                        _ => break,
                    };
                    if contains(branch) {
                        // Reaching this later body makes earlier exact zero-length
                        // guards false. A false conjunction supplies no such proof.
                        if contains(body) && nonempty {
                            return true;
                        }
                        break;
                    }
                    if branch.kind() == NodeKind::ConditionalBranch {
                        nonempty |= self.length_equals(file, parts[0], array, 0, view);
                    }
                }
            }
            containing.extend(children.into_iter().filter(|child| contains(child)));
        }
        false
    }
    pub(in crate::callable_definitions) fn assertion_arguments(
        &self,
        file: FileId,
        call: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<Vec<(FileId, &'a SyntaxNode)>> {
        // MiniZinc 2.10.1 rejects named arguments for this builtin.
        if call
            .child_nodes()
            .any(|n| n.kind() == NodeKind::NamedArgument)
        {
            return None;
        }
        let CallOutcome::Resolved {
            declaration: id,
            parameters,
            return_type,
        } = &self
            .operation_fact(self.view(file, call, view), file, call)?
            .outcome
        else {
            return None;
        };
        if !self.core(file, call, view, "assert") || !return_type.known() || optional(return_type) {
            return None;
        }
        if !matches!(parameters.len(), 2 | 3) || call.child_nodes().count() != parameters.len() {
            return None;
        }
        (0..parameters.len())
            .map(|position| {
                call_argument(
                    self.context,
                    self.bindings,
                    self.view(file, call, view),
                    file,
                    call,
                    *id,
                    position,
                )
            })
            .collect()
    }
    pub(in crate::callable_definitions) fn integer_assertion_arguments(
        &self,
        file: FileId,
        call: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<(FileId, &'a SyntaxNode)>, String> {
        let args = self
            .assertion_arguments(file, call, view)
            .filter(|args| args.len() == 3)
            .ok_or("returning assertion argument correspondence is unsupported")?;
        for (position, (file, node)) in args.iter().enumerate() {
            let range = self.context.files[*file].location(node.range()).range;
            let kind = match position {
                0 => TypeKind::Bool,
                1 => TypeKind::String,
                _ => TypeKind::Int,
            };
            if !self.view(*file, node, view).expressions.iter().any(|e| {
                e.file == *file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == kind
                    && e.ty.instantiation == Instantiation::Parameter
            }) {
                return Err("returning assertion requires present parameter condition, message and integer value".into());
            }
        }
        Ok(args)
    }
    pub(in crate::callable_definitions) fn assertion_literal(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<TokenKind> {
        let node = unwrap(node);
        (node.kind() == NodeKind::Expression)
            .then(|| {
                crate::domains::tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|token| token.kind)
            })
            .flatten()
    }
    pub(in crate::callable_definitions) fn integer_conditional_values(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<&'a SyntaxNode>, String> {
        let mut values = Vec::new();
        let mut has_else = false;
        for branch in node.child_nodes() {
            let children: Vec<_> = branch.child_nodes().collect();
            let body = if branch.kind() == NodeKind::ConditionalBranch && children.len() == 2 {
                let range = self.context.files[file].location(children[0].range()).range;
                if !self
                    .view(file, children[0], view)
                    .expressions
                    .iter()
                    .any(|e| {
                        e.file == file
                            && e.location.range == range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.kind == TypeKind::Bool
                            && e.ty.instantiation == Instantiation::Parameter
                    })
                {
                    return Err("integer conditional guard is unsupported".into());
                }
                values.push(children[0]);
                children[1]
            } else if branch.kind() == NodeKind::ElseBranch && children.len() == 1 {
                has_else = true;
                children[0]
            } else {
                return Err("integer conditional branch form is unsupported".into());
            };
            let range = self.context.files[file].location(body.range()).range;
            if !self.view(file, body, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == TypeKind::Int
                    && e.ty.instantiation == Instantiation::Parameter
            }) {
                return Err("integer conditional body is unsupported".into());
            }
            values.push(body);
        }
        if !has_else || values.len() < 3 {
            return Err("integer conditional requires a complete else".into());
        }
        Ok(values)
    }
    pub(in crate::callable_definitions) fn asserted_integer_bounds(
        &self,
        file: FileId,
        operation: &'a SyntaxNode,
        operand: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let present_integer = |node: &SyntaxNode| {
            let range = self.context.files[file].location(node.range()).range;
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == TypeKind::Int
            })
        };
        let mut operand = unwrap(operand);
        if !present_integer(operand) {
            return false;
        }
        if operand.kind() == NodeKind::CallExpression && self.core(file, operand, view, "enum2int")
        {
            let children: Vec<_> = operand.child_nodes().collect();
            if children.len() != 1 || !present_integer(children[0]) {
                return false;
            }
            operand = unwrap(children[0]);
        }
        if operand.kind() != NodeKind::Expression {
            return false;
        }
        let Some(id) = self.reference(file, operand) else {
            return false;
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut pending = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = pending.pop() {
            if node.kind() == NodeKind::CallExpression
                && self.core(file, node, view, "assert")
                && let Ok(args) = self.integer_assertion_arguments(file, node, view)
                && args[2].0 == file
                && contains(args[2].1)
            {
                let guard = unwrap(args[0].1);
                let children: Vec<_> = guard.child_nodes().collect();
                if args[0].0 == file
                    && self.core(file, guard, view, "has_bounds")
                    && children.len() == 1
                    && present_integer(children[0])
                    && unwrap(children[0]).kind() == NodeKind::Expression
                    && self.reference(file, unwrap(children[0])) == Some(id)
                {
                    return true;
                }
            }
            pending.extend(node.child_nodes().filter(|child| contains(child)));
        }
        false
    }
    pub(in crate::callable_definitions) fn finite_array_branch(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let range = operation.range();
        let mut containing = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && children[1].range().start <= range.start
                && range.end <= children[1].range().end
            {
                let guard = unwrap(children[0]);
                let guard_range = self.context.files[file].location(guard.range()).range;
                let parameter_bool = self.view(file, guard, view).expressions.iter().any(|e| {
                    e.file == file
                        && e.location.range == guard_range
                        && e.ty.kind == TypeKind::Bool
                        && !e.ty.optional
                        && e.ty.instantiation == Instantiation::Parameter
                });
                let list = guard
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList);
                let body = guard
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList);
                if parameter_bool
                    && guard.kind() == NodeKind::GeneratorCallExpression
                    && self.core(file, guard, view, "forall")
                    && let (Some(list), Some(body)) = (list, body)
                {
                    let generators: Vec<_> = list.child_nodes().collect();
                    if generators.len() == 1
                        && crate::domains::generator_slots(
                            &self.context.files[file].parsed,
                            generators[0],
                        ) == 1
                        && self.iterations(file, &generators, view).is_ok()
                        && self.core(file, unwrap(body), view, "has_bounds")
                    {
                        let args: Vec<_> = unwrap(body).child_nodes().collect();
                        if args.len() == 1 {
                            let access = unwrap(args[0]);
                            let parts: Vec<_> = access.child_nodes().collect();
                            if access.kind() == NodeKind::ArrayAccessExpression
                                && parts.len() == 2
                                && unwrap(parts[0]).kind() == NodeKind::Expression
                                && self.reference(file, unwrap(parts[0])) == Some(array)
                                && self.member_index(file, array, parts[1], &generators, view)
                            {
                                // A complete traversal proves each element bounded. An
                                // empty array supplies the finite empty domain union.
                                return true;
                            }
                        }
                    }
                }
            }
            containing.extend(children.into_iter().filter(|child| {
                child.range().start <= range.start && range.end <= child.range().end
            }));
        }
        false
    }
    pub(in crate::callable_definitions) fn parameter_array_source(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let ty = |node: &SyntaxNode| {
            let range = self.context.files[file].location(node.range()).range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        let children: Vec<_> = node.child_nodes().collect();
        if !self.core(file, node, view, "arrayXd")
            || children.len() != 2
            || children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || ty(node).is_none_or(|t| !parameter_integers(t))
            || children
                .iter()
                .any(|n| ty(n).is_none_or(|t| !parameter_integers(t)))
            || unwrap(children[0]).kind() != NodeKind::Expression
        {
            return None;
        }
        let source = self.reference(file, unwrap(children[0]))?;
        let source_type = &facts.declarations[source.0].ty;
        let TypeKind::Array { indices, .. } = &source_type.kind else {
            return None;
        };
        if !parameter_integers(source_type)
            || !matches!(&ty(node)?.kind, TypeKind::Array { indices: result, .. } if result == indices)
        {
            return None;
        }
        let comprehension = unwrap(children[1]);
        if comprehension.kind() != NodeKind::ArrayComprehension {
            return None;
        }
        let list = comprehension
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)?;
        let generators: Vec<_> = list.child_nodes().collect();
        if generators.len() != 1
            || generators[0]
                .child_nodes()
                .any(|n| n.kind() == NodeKind::WhereFilter)
            || self
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
        let domain = unwrap(generators[0].child_nodes().next()?);
        (domain.kind() == NodeKind::Expression && self.reference(file, domain) == Some(source))
            .then_some(source)
    }
    pub(in crate::callable_definitions) fn default_collection(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> bool {
        let node = unwrap(node);
        node.kind() == NodeKind::ArrayComprehension
            && node
                .child_nodes()
                .find(|n| n.kind() != NodeKind::GeneratorList)
                .is_some_and(|body| {
                    operator(self.context, file, unwrap(body)) == Some(TokenKind::Default)
                })
    }
    pub(in crate::callable_definitions) fn enum_collection_header(
        &self,
        file: FileId,
        header: &SyntaxNode,
        view: &CallableFacts,
        preceding: &[&SyntaxNode],
    ) -> bool {
        let Some(source) = header.child_nodes().next().map(unwrap) else {
            return false;
        };
        if source.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, source).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        {
            return false;
        }
        let Some(id) = self.reference(file, source) else {
            return false;
        };
        let declaration = &self.bindings.declarations[id.0];
        let Some(ty) = self.expression_type(view, file, source).map(|e| &e.ty) else {
            return false;
        };
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        if !parameter(ty) || ty != &view.declarations[id.0].ty {
            return false;
        }
        let binders: Vec<_> = self
            .bindings
            .declarations
            .iter()
            .filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == header.range()
            })
            .collect();
        match &ty.kind {
            TypeKind::Array { indices, element } => {
                declaration.top_level
                    && declaration.role == DeclarationRole::Value
                    && indices.len() == 1
                    && parameter(&indices[0])
                    && indices[0].kind == TypeKind::Int
                    && parameter(element)
                    && matches!(&element.kind, TypeKind::Set(member)
                        if parameter(member) && matches!(member.kind, TypeKind::Enum(_)))
                    && binders.len() == 1
                    && &view.declarations[binders[0].id.0].ty == element.as_ref()
            }
            TypeKind::Set(element) => {
                let Some(owning) = preceding.iter().find(|owning| {
                    owning.range() == declaration.syntax_range
                        && crate::domains::generator_slots(&self.context.files[file].parsed, owning)
                            == 1
                        && owning.children().iter().any(|child| {
                            matches!(child, SyntaxElement::Token(i)
                            if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                        })
                }) else {
                    return false;
                };
                let Some(array) = owning.child_nodes().next().map(unwrap) else {
                    return false;
                };
                let Some(array_id) = self.reference(file, array).filter(|id| {
                    array.kind() == NodeKind::Expression
                        && self.bindings.declarations[id.0].top_level
                        && self.bindings.declarations[id.0].role == DeclarationRole::Value
                }) else {
                    return false;
                };
                let array_type = &view.declarations[array_id.0].ty;
                declaration.file == file
                    && declaration.role == DeclarationRole::Generator
                    && parameter(array_type)
                    && self
                        .expression_type(view, file, array)
                        .is_some_and(|e| &e.ty == array_type)
                    && matches!(&array_type.kind, TypeKind::Array { indices, element: member }
                        if indices.len() == 1 && parameter(&indices[0]) && indices[0].kind == TypeKind::Int
                            && member.as_ref() == ty)
                    && parameter(element)
                    && matches!(element.kind, TypeKind::Enum(_))
                    && !binders.is_empty()
                    && binders
                        .iter()
                        .all(|binder| &view.declarations[binder.id.0].ty == element.as_ref())
            }
            _ => false,
        }
    }
    pub(in crate::callable_definitions) fn is_scalar_integer_extremum(
        &self,
        file: FileId,
        written: &SyntaxNode,
        view: &CallableFacts,
        operands: &[&SyntaxNode],
    ) -> bool {
        let node = unwrap(written);
        let facts = self.view(file, node, view);
        let integer = |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
        if operands.len() != 2
            || operands.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || !(self.core(file, node, view, "min") || self.core(file, node, view, "max"))
            || self
                .expression_type(facts, file, node)
                .is_none_or(|e| !integer(&e.ty))
            || operands.iter().any(|operand| {
                self.expression_type(self.view(file, operand, view), file, operand)
                    .is_none_or(|e| !integer(&e.ty))
            })
            || self.operation_fact(facts, file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(integer)
                        && integer(return_type))
            })
        {
            return false;
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return false;
            }
            nodes.extend(node.child_nodes());
        }
        true
    }
    pub(in crate::callable_definitions) fn set_axis_integer_reshape_arguments(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<[&'a SyntaxNode; 3]> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "array2d") {
            return None;
        }
        let mut nodes = vec![written];
        while let Some(current) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, current) {
                return None;
            }
            nodes.extend(current.child_nodes());
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [rows, columns, values] = children.as_slice() else {
            return None;
        };
        let arguments = [*rows, *columns, *values];
        let unsupported_bare_value = |argument: &SyntaxNode| {
            argument.kind() != NodeKind::Expression
                || self.reference(file, argument).is_none_or(|id| {
                    let declaration = &self.bindings.declarations[id.0];
                    !declaration.top_level || declaration.role != DeclarationRole::Value
                })
        };
        let inline = unwrap(values);
        if arguments[..2]
            .iter()
            .any(|argument| unsupported_bare_value(argument))
            || unsupported_bare_value(values) && inline.kind() != NodeKind::ArrayComprehension
        {
            return None;
        }
        let facts = self.view(file, node, view);
        let typed = |value: &SyntaxNode| self.expression_type(facts, file, value).map(|e| &e.ty);
        let parameter_int = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let integer_array = |t: &TypeInst, rank: usize| {
            t.known()
                && !optional(t)
                && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == rank && indices.iter().all(parameter_int)
                        && element.known() && !optional(element) && element.kind == TypeKind::Int)
        };
        let actual = [typed(rows)?, typed(columns)?, typed(values)?];
        let result = typed(node)?;
        if !actual[..2].iter().all(|t| t.known() && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(&t.kind, TypeKind::Set(element) if parameter_int(element)))
            || !integer_array(actual[2], 1) || !integer_array(result, 2)
            || !self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 3 && parameters.iter().zip(actual).all(|(formal, actual)| formal == actual)
                        && return_type == result)
            })
        {
            return None;
        }
        if inline.kind() == NodeKind::ArrayComprehension {
            let parts: Vec<_> = inline.child_nodes().collect();
            let [_, list] = parts.as_slice() else {
                return None;
            };
            if list.kind() != NodeKind::GeneratorList || list.child_nodes().count() != 2 {
                return None;
            }
            for (header, axis) in list.child_nodes().zip(arguments[..2].iter()) {
                let sources: Vec<_> = header.child_nodes().collect();
                let [source] = sources.as_slice() else {
                    return None;
                };
                let source = unwrap(source);
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if !header.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                        if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                }) || crate::domains::generator_slots(&self.context.files[file].parsed, header)
                    != 1
                    || binders.len() != 1
                    || !parameter_int(&facts.declarations[binders[0].id.0].ty)
                    || source.kind() != NodeKind::Expression
                    || self.reference(file, source) != self.reference(file, axis)
                {
                    return None;
                }
            }
        }
        Some(arguments)
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn reflection_array_nonempty(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        if self
            .source
            .array_nonempty_branch(file, operation, array, view)
        {
            return true;
        }
        let TypeKind::Array { indices, .. } = &view.declarations[array.0].ty.kind else {
            return false;
        };
        if indices.len() != 1 {
            return false;
        }
        let identity = |node: &SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::Expression {
                return None;
            }
            let id = self.source.reference(file, node)?;
            let ty = &self.source.view(file, node, view).declarations[id.0].ty;
            (ty.known()
                && !optional(ty)
                && matches!(&ty.kind,
                TypeKind::Array { indices: source, .. } if source.len() == 1
                    && source[0].kind == indices[0].kind
                    && source[0].instantiation == Instantiation::Parameter))
            .then_some(id)
        };
        let index_array = |node: &SyntaxNode| {
            let node = unwrap(node);
            let children: Vec<_> = node.child_nodes().collect();
            if !self.source.core(file, node, view, "index_set") || children.len() != 1 {
                return None;
            }
            identity(children[0])
        };
        let parameter_bool = |node: &SyntaxNode| {
            let range = self.source.context.files[file].location(node.range()).range;
            self.source
                .view(file, node, view)
                .expressions
                .iter()
                .any(|e| {
                    e.file == file
                        && e.location.range == range
                        && e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.kind == TypeKind::Bool
                        && e.ty.instantiation == Instantiation::Parameter
                })
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut nonempty = Vec::new();
        let mut aligned = Vec::new();
        let mut ancestors = vec![self.source.context.files[file].parsed.tree()];
        while let Some(node) = ancestors.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
                && parameter_bool(unwrap(children[0]))
            {
                let mut guards = vec![unwrap(children[0])];
                while let Some(guard) = guards.pop() {
                    let parts: Vec<_> = guard.child_nodes().collect();
                    if parts.len() == 2 && self.source.core(file, guard, view, "/\\") {
                        guards.extend(parts.into_iter().map(unwrap));
                    } else if parts.len() == 2
                        && self.source.core(file, guard, view, ">=")
                        && parameter_bool(guard)
                        && crate::domains::invariant_expression_integer(
                            self.source.context,
                            self.source.bindings,
                            file,
                            parts[1],
                        ) == Ok(Some(1))
                    {
                        let length = unwrap(parts[0]);
                        let arguments: Vec<_> = length.child_nodes().collect();
                        if self.source.core(file, length, view, "length")
                            && arguments.len() == 1
                            && let Some(id) = identity(arguments[0])
                        {
                            nonempty.push(id);
                        }
                    }
                }
            }
            if children.len() == 2 && self.source.core(file, node, view, "/\\") {
                let mut siblings: Vec<_> = children
                    .iter()
                    .copied()
                    .filter(|child| !contains(child))
                    .map(unwrap)
                    .collect();
                while let Some(sibling) = siblings.pop() {
                    let parts: Vec<_> = sibling.child_nodes().collect();
                    if parts.len() == 2 && self.source.core(file, sibling, view, "/\\") {
                        siblings.extend(parts.into_iter().map(unwrap));
                        continue;
                    }
                    if !self.source.core(file, sibling, view, "assert") {
                        continue;
                    }
                    let Some(args) = self
                        .source
                        .assertion_arguments(file, sibling, view)
                        .filter(|args| args.len() == 2 && args.iter().all(|(f, _)| *f == file))
                    else {
                        continue;
                    };
                    let message = unwrap(args[1].1);
                    let tokens =
                        crate::domains::tokens(&self.source.context.files[file].parsed, message);
                    if !parameter_bool(unwrap(args[0].1))
                        || tokens.len() != 1
                        || tokens[0].kind != TokenKind::StringLiteral
                        || !self
                            .source
                            .view(file, message, view)
                            .expressions
                            .iter()
                            .any(|e| {
                                e.file == file
                                    && e.location.range
                                        == self.source.context.files[file]
                                            .location(message.range())
                                            .range
                                    && e.ty.known()
                                    && !optional(&e.ty)
                                    && e.ty.kind == TypeKind::String
                                    && e.ty.instantiation == Instantiation::Parameter
                            })
                    {
                        continue;
                    }
                    let mut conditions = vec![unwrap(args[0].1)];
                    let mut pairs = Vec::new();
                    let mut supported = true;
                    while let Some(condition) = conditions.pop() {
                        let parts: Vec<_> = condition.child_nodes().collect();
                        if parts.len() == 2 && self.source.core(file, condition, view, "/\\") {
                            conditions.extend(parts.into_iter().map(unwrap));
                        } else if parts.len() == 2
                            && self.source.core(file, condition, view, "=")
                            && let (Some(left), Some(right)) =
                                (index_array(parts[0]), index_array(parts[1]))
                        {
                            pairs.push((left, right));
                        } else {
                            supported = false;
                            break;
                        }
                    }
                    // This bounded shape prevents assertion proof checking from
                    // recursively depending on another reflected value.
                    if supported
                        && args.iter().all(|(_, argument)| {
                            self.dependencies(file, argument, view, &[]).is_ok()
                        })
                    {
                        aligned.extend(pairs);
                    }
                }
            }
            ancestors.extend(children.into_iter().filter(|child| contains(child)));
        }
        nonempty.iter().any(|source| {
            *source == array
                || aligned.iter().any(|(left, right)| {
                    (*left == *source && *right == array) || (*right == *source && *left == array)
                })
        })
    }
    // This inspection-only branch never changes strict dependencies or outputs.
    pub(in crate::callable_definitions) fn inspect_private_index_conditional<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) -> Option<Result<(), String>> {
        let (clause, view, known) = inputs;
        if !matches!(clause.kind, ClauseKind::Conditional) {
            return None;
        }
        let file = clause.file;
        let owner = self.source.bindings.declarations.iter().find(|d| {
            d.file == file
                && d.item == clause.item
                && d.role == DeclarationRole::Predicate
                && d.syntax_range.start <= clause.node.range().start
                && clause.node.range().end <= d.syntax_range.end
        })?;
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        if written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .count()
            != 1
            || written
                .child_nodes()
                .find(|n| is_expression(n.kind()))
                .is_none_or(|body| unwrap(body).range() != clause.node.range())
            || !self.instances.iter().any(|i| {
                i.id == owner.id
                    && !i.recursive
                    && i.clauses
                        .iter()
                        .any(|c| c.file == file && c.node.range() == clause.node.range())
            })
        {
            return None;
        }
        let branches: Vec<_> = clause.node.child_nodes().collect();
        if branches.len() != 2
            || branches[0].kind() != NodeKind::ConditionalBranch
            || branches[1].kind() != NodeKind::ElseBranch
        {
            return None;
        }
        let first: Vec<_> = branches[0].child_nodes().collect();
        let second: Vec<_> = branches[1].child_nodes().collect();
        if first.len() != 2
            || second.len() != 1
            || self.source.assertion_literal(file, first[1]) != Some(TokenKind::True)
        {
            return None;
        }
        let guard = unwrap(first[0]);
        let guard_parts: Vec<_> = guard.child_nodes().collect();
        if guard.kind() != NodeKind::BinaryExpression
            || guard_parts.len() != 2
            || !self.source.core(file, guard, view, "=")
        {
            return None;
        }
        let length = unwrap(guard_parts[0]);
        let subject = unwrap(length_argument(
            self.source.context,
            self.source.bindings,
            self.source.view(file, length, view),
            file,
            length,
        )?);
        let source = self.source.reference(file, subject)?;
        if !self.source.length_equals(file, guard, source, 0, view)
            || unwrap(guard_parts[1]).kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(guard_parts[1])).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            return None;
        }
        let source_owner = &self.source.bindings.declarations[source.0];
        if source_owner.file != file
            || source_owner.item != clause.item
            || source_owner.role != DeclarationRole::Parameter
        {
            return None;
        }
        let local_node = unwrap(second[0]);
        if local_node.kind() != NodeKind::LetExpression {
            return None;
        }
        let locals: Vec<_> = local_node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|n| n.child_nodes())
            .collect();
        if locals.len() != 4 || locals.iter().any(|n| n.kind() != NodeKind::Declaration) {
            return None;
        }
        let ids: Vec<_> = locals
            .iter()
            .map(|node| {
                self.source
                    .bindings
                    .declarations
                    .iter()
                    .find(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Local
                            && d.syntax_range == node.range()
                    })
                    .map(|d| d.id)
            })
            .collect::<Option<Vec<_>>>()?;
        let private = [source, ids[0], ids[1], ids[2], ids[3]];
        let values: Vec<_> = locals[..3]
            .iter()
            .map(|node| node.child_nodes().find(|n| is_expression(n.kind())))
            .collect::<Option<Vec<_>>>()?;
        for (value, name, argument) in [
            (values[0], "index_set", source),
            (values[1], "min", ids[0]),
            (values[2], "card", ids[0]),
        ] {
            let value = unwrap(value);
            let args: Vec<_> = value.child_nodes().collect();
            if value.kind() != NodeKind::CallExpression
                || args.len() != 1
                || !self.source.core(file, value, view, name)
                || self.source.reference(file, unwrap(args[0])) != Some(argument)
            {
                return None;
            }
        }
        if locals[3].child_nodes().any(|n| is_expression(n.kind())) {
            return None;
        }
        let array_type = locals[3].child_nodes().next()?;
        let domains: Vec<_> = array_type.child_nodes().collect();
        if array_type.kind() != NodeKind::ArrayType
            || domains.len() != 2
            || domains.iter().any(|d| d.kind() != NodeKind::DomainType)
        {
            return None;
        }
        let axes: Vec<_> = domains[0].child_nodes().collect();
        let elements: Vec<_> = domains[1].child_nodes().collect();
        if axes.len() != 1
            || self.source.reference(file, unwrap(axes[0])) != Some(ids[0])
            || elements.len() != 1
            || unwrap(elements[0]).kind() != NodeKind::RangeExpression
        {
            return None;
        }
        let range = unwrap(elements[0]);
        let endpoints: Vec<_> = range.child_nodes().collect();
        if endpoints.len() != 2
            || !self.source.core(file, range, view, "..")
            || unwrap(endpoints[0]).kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(endpoints[0])).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
            || !crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                file,
                endpoints[0],
            )
            .is_ok_and(|value| value == Some(1))
            || self.source.reference(file, unwrap(endpoints[1])) != Some(ids[2])
        {
            return None;
        }
        let result: Vec<_> = local_node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::LetBlock)
            .collect();
        let [body] = result.as_slice() else {
            return None;
        };
        let mut pending = vec![*body];
        let mut relations = Vec::new();
        while let Some(node) = pending.pop() {
            let node = unwrap(node);
            if node.kind() == NodeKind::BinaryExpression
                && self.source.core(file, node, view, "/\\")
            {
                let parts: Vec<_> = node.child_nodes().collect();
                if parts.len() != 2
                    || !crate::definitions::annotations_safe(self.source.context, file, node)
                {
                    return Some(Err("private index conjunction is unsupported".into()));
                }
                pending.extend(parts.into_iter().rev());
            } else {
                relations.push(node);
            }
        }
        if relations.len() != 5
            || relations[..2]
                .iter()
                .any(|n| n.kind() != NodeKind::CallExpression)
            || relations[2].kind() != NodeKind::GeneratorCallExpression
            || relations[3].kind() != NodeKind::BinaryExpression
            || relations[4].kind() != NodeKind::GeneratorCallExpression
            || !self.source.core(file, relations[2], view, "forall")
            || !self.source.core(file, relations[3], view, "=")
            || !self.source.core(file, relations[4], view, "forall")
        {
            return None;
        }
        for (node, argument) in relations[..2].iter().zip([source, ids[3]]) {
            let args: Vec<_> = node.child_nodes().collect();
            let (id, parameters) = self.source.resolved(file, node, view)?;
            let declaration = &self.source.bindings.declarations[id.0];
            if declaration.name != "all_different"
                || self.source.context.files[declaration.file].kind != SourceKind::StandardLibrary
                || !self.instances.iter().any(|instance| {
                    instance.id == id && instance.parameters == parameters && !instance.recursive
                })
                || args.len() != 1
                || self.source.reference(file, unwrap(args[0])) != Some(argument)
            {
                return None;
            }
        }
        Some((|| {
            let typed = |node: &SyntaxNode| {
                self.source
                    .expression_type(self.source.view(file, node, view), file, node)
                    .map(|expression| &expression.ty)
            };
            let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
            let parameter_integer =
                |ty: &TypeInst| integer(ty) && ty.instantiation == Instantiation::Parameter;
            let set = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element) if parameter_integer(element))
            };
            let array = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Decision
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && parameter_integer(&indices[0]) && integer(element)
                        && element.instantiation == Instantiation::Decision)
            };
            let boolean = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Bool;
            if !array(&view.declarations[source.0].ty)
                || typed(subject) != Some(&view.declarations[source.0].ty)
                || !set(&view.declarations[ids[0].0].ty)
                || !parameter_integer(&view.declarations[ids[1].0].ty)
                || !parameter_integer(&view.declarations[ids[2].0].ty)
                || !array(&view.declarations[ids[3].0].ty)
                || typed(clause.node).is_none_or(|t| !boolean(t))
                || typed(local_node).is_none_or(|t| !boolean(t))
                || typed(guard)
                    .is_none_or(|t| !boolean(t) || t.instantiation != Instantiation::Parameter)
                || !crate::definitions::annotations_safe(self.source.context, file, written)
                || !crate::definitions::annotations_safe(self.source.context, file, clause.node)
            {
                return Err("private index owner type or annotation is unsupported".into());
            }
            let formal = find_node(
                self.source.context.files[file].parsed.tree(),
                &source_owner.syntax_range,
                source_owner.role,
            )
            .ok_or("private index source formal unavailable")?;
            self.type_dependencies(file, formal, view, &clause.generators)?;
            let mut annotations = vec![written];
            while let Some(node) = annotations.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, node) {
                    return Err("private index source annotation is unsupported".into());
                }
                annotations.extend(node.child_nodes());
            }
            for node in &relations[..2] {
                if typed(node).is_none_or(|t| !boolean(t) || t.instantiation != Instantiation::Decision)
                    || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && array(&parameters[0]) && set(&parameters[1])
                                && typed(node) == Some(return_type))) {
                    return Err("private index selected relation tuple is unsupported".into());
                }
            }
            for value in [
                length,
                guard,
                unwrap(values[0]),
                unwrap(values[1]),
                unwrap(values[2]),
                range,
            ] {
                let args: Vec<_> = value.child_nodes().collect();
                if !crate::definitions::annotations_safe(self.source.context, file, value)
                    || self.source.operation_fact(self.source.view(file, value, view), file, value).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == args.len() && typed(value) == Some(return_type)
                                && parameters.iter().zip(&args).all(|(formal, actual)| formal.known()
                                    && !optional(formal) && typed(actual).is_some_and(|t| crate::types::coerces(t, formal))))) {
                    return Err("private index operation signature is unsupported".into());
                }
            }
            if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                file,
                first[0],
                view,
                &clause.generators,
                &mut Vec::new(),
            ) {
                return Err(reason);
            }
            let local = Clause {
                file,
                item: clause.item,
                node: local_node,
                generators: clause.generators.clone(),
                kind: ClauseKind::Local,
            };
            self.inspect_uncertain_index_let(&local, view, Some(&private), &relations, None)
                .ok_or("private index local scope is unsupported")??;
            let incoming = invocation.as_ref().and_then(|state| state.reachable);
            let reachable = match invocation.as_ref() {
                Some(state) => match self.invocation_guard(file, first[0], view, state)? {
                    Some(true) => Some(false),
                    Some(false) => incoming,
                    None => {
                        if incoming == Some(false) {
                            Some(false)
                        } else {
                            None
                        }
                    }
                },
                None => None,
            };
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = reachable;
            }
            let before = unavailable.len();
            // Retain normal selected actual/default/body/cycle traversal. Only
            // its inspection errors survive; none of its certificates escape.
            for node in &relations[..2] {
                let call = Clause {
                    file,
                    item: clause.item,
                    node,
                    generators: clause.generators.clone(),
                    kind: ClauseKind::Call,
                };
                self.interpret_forwarded(
                    (&call, view, known),
                    &mut Vec::new(),
                    unavailable,
                    &mut Vec::new(),
                    active,
                    invocation.as_deref_mut(),
                );
            }
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = incoming;
            }
            if unavailable.len() != before {
                return Err(unavailable[before].reason.clone());
            }
            Ok(())
        })())
    }
    pub(in crate::callable_definitions) fn inspect_uncertain_index_let<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        private: Option<&[DeclarationId; 5]>,
        relations: &[&'a SyntaxNode],
        invocation: Option<&Invocation<'a, 'b>>,
    ) -> Option<Result<(), String>> {
        // This branch inspects a selected callable body only. It cannot certify
        // model-root locals through the stricter whole-item acceptance gates.
        if !self.source.bindings.declarations.iter().any(|d| {
            d.file == clause.file
                && d.item == clause.item
                && matches!(
                    d.role,
                    DeclarationRole::Predicate | DeclarationRole::Function | DeclarationRole::Test
                )
                && d.syntax_range.start <= clause.node.range().start
                && clause.node.range().end <= d.syntax_range.end
        }) {
            return None;
        }
        let locals: Vec<_> = clause
            .node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|n| n.child_nodes())
            .collect();
        let requested = private.is_some()
            || locals.iter().any(|local| {
                // An uncertain extremum can be written directly in a private array
                // range, without a separate initialized integer local.
                let mut values: Vec<_> = local
                    .child_nodes()
                    .filter(|n| is_expression(n.kind()) || n.kind() == NodeKind::ArrayType)
                    .collect();
                while let Some(value) = values.pop() {
                    if parameter_index_extremum(
                        self.source.context,
                        self.source.bindings,
                        self.source.view(clause.file, value, view),
                        clause.file,
                        value,
                    )
                    .is_some()
                        && matches!(
                            self.direct_safety(clause.file, value, view, &clause.generators),
                            DefinitionSafety::Unknown(_)
                        )
                    {
                        return true;
                    }
                    values.extend(value.child_nodes());
                }
                false
            });
        if !requested && private.is_none() {
            return None;
        }
        Some((|| {
            if !crate::definitions::annotations_safe(self.source.context, clause.file, clause.node)
            {
                return Err("uncertain-bound let annotation is unsupported".into());
            }
            let local_ids: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == clause.file
                        && d.role == DeclarationRole::Local
                        && locals.iter().any(|local| local.range() == d.syntax_range)
                })
                .map(|d| d.id)
                .collect();
            let private_integer_arrays = private.is_none() && local_ids.iter().any(|id| {
                let ty = &view.declarations[id.0].ty;
                ty.instantiation == Instantiation::Decision
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Int)
            });
            let mut checked_locals = Vec::new();
            let no_forward_reference = |value: &SyntaxNode| {
                let range = self.source.context.files[clause.file]
                    .location(value.range())
                    .range;
                !self.source.bindings.references.iter().any(|r|
                    r.file == clause.file && range.start <= r.location.range.start && r.location.range.end <= range.end
                        && matches!(r.resolution, BindingResolution::Resolved(id)
                            if local_ids.contains(&id)
                                && self.source.bindings.declarations[id.0].syntax_range.end
                                    + self.source.context.files[clause.file].byte_offset > r.location.range.start))
            };
            for local in &locals {
                if !crate::definitions::annotations_safe(self.source.context, clause.file, local) {
                    return Err("uncertain-bound local annotation is unsupported".into());
                }
                if local.kind() == NodeKind::Constraint {
                    for body in local.child_nodes().filter(|n| is_expression(n.kind())) {
                        self.inspect_uncertain_index_expression(
                            clause.file,
                            body,
                            view,
                            &clause.generators,
                            &checked_locals,
                            private,
                        )?;
                    }
                    continue;
                }
                let d = self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .find(|d| {
                        d.file == clause.file
                            && d.role == DeclarationRole::Local
                            && d.syntax_range == local.range()
                    })
                    .filter(|_| local.kind() == NodeKind::Declaration)
                    .ok_or("uncertain-bound local declaration is unsupported")?;
                let ty = &view.declarations[d.id.0].ty;
                if !ty.known() || optional(ty) {
                    return Err("uncertain-bound local type is unsupported".into());
                }
                let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
                let boolean_array = matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Bool);
                let integer_array = ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(|index| index.known() && !optional(index)
                            && index.instantiation == Instantiation::Parameter && index.kind == TypeKind::Int)
                            && element.known() && !optional(element) && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Int);
                let private_integer_array = private.is_none()
                    && ty.instantiation == Instantiation::Decision
                    && initializer.is_none()
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                            && element.known() && !optional(element) && element.instantiation == Instantiation::Decision
                            && element.kind == TypeKind::Int);
                let private_integer = ty.instantiation == Instantiation::Decision
                    && ty.kind == TypeKind::Int
                    && initializer.is_none();
                let private_set = private.is_some_and(|ids| d.id == ids[1]);
                let private_array = private.is_some_and(|ids| d.id == ids[4]);
                if !(private_set
                    || private_array
                    || (ty.instantiation == Instantiation::Parameter
                        && ty.kind == TypeKind::Int
                        && initializer.is_some())
                    || (integer_array && initializer.is_some())
                    || private_integer
                    || (ty.instantiation == Instantiation::Decision
                        && initializer.is_none()
                        && (ty.kind == TypeKind::Bool || boolean_array || private_integer_array)))
                {
                    return Err("uncertain-bound local declaration shape is unsupported".into());
                }
                let written = local
                    .child_nodes()
                    .next()
                    .ok_or("written local type unavailable")?;
                if private_integer
                    && (written.kind() != NodeKind::DomainType
                        || written.child_nodes().count() != 1
                        || written
                            .child_nodes()
                            .next()
                            .is_none_or(|value| unwrap(value).kind() != NodeKind::RangeExpression))
                {
                    return Err(
                        "uncertain-bound private integer requires a written parameter range".into(),
                    );
                }
                if (boolean_array || private_integer_array)
                    && written.child_nodes().next().is_none_or(|index| {
                        index.kind() != NodeKind::DomainType
                            || index.child_nodes().next().is_none_or(|value| {
                                unwrap(value).kind() != NodeKind::RangeExpression
                            })
                    })
                {
                    return Err(if private_integer_array {
                        "uncertain-bound private integer array requires a written integer range"
                    } else {
                        "uncertain-bound private Boolean array requires a written integer range"
                    }
                    .into());
                }
                let mut types = vec![written];
                while let Some(node) = types.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, clause.file, node)
                    {
                        return Err("uncertain-bound written type annotation is unsupported".into());
                    }
                    if node.kind() == NodeKind::DomainType {
                        for value in node.child_nodes() {
                            if private_integer_arrays
                                && let Some(reason) = self.source.closed_integer_source_error(
                                    clause.file,
                                    value,
                                    false,
                                    false,
                                )
                            {
                                return Err(reason);
                            }
                            if !no_forward_reference(value) {
                                return Err("uncertain-bound domain has a forward or cyclic local dependency".into());
                            }
                            self.inspect_uncertain_index_expression(
                                clause.file,
                                value,
                                view,
                                &clause.generators,
                                &checked_locals,
                                private,
                            )?;
                        }
                    } else {
                        types.extend(node.child_nodes());
                    }
                }
                if let Some(value) = initializer {
                    if private_integer_arrays
                        && let Some(reason) = self.source.closed_integer_source_error(
                            clause.file,
                            value,
                            false,
                            false,
                        )
                    {
                        return Err(reason);
                    }
                    if !no_forward_reference(value) {
                        return Err(
                            "uncertain-bound initializer has a forward or cyclic local dependency"
                                .into(),
                        );
                    }
                    if integer_array {
                        self.inspect_cartesian_parameter_array(
                            clause.file,
                            written,
                            value,
                            view,
                            &clause.generators,
                            &checked_locals,
                        )?;
                    } else if let Some(ids) = private {
                        self.inspect_uncertain_index_expression(
                            clause.file,
                            value,
                            view,
                            &clause.generators,
                            &checked_locals,
                            Some(ids),
                        )?;
                    } else if let DefinitionSafety::Unsupported(reason) =
                        self.direct_safety(clause.file, value, view, &clause.generators)
                    {
                        return Err(reason);
                    }
                }
                checked_locals.push(d.id);
            }
            if private_integer_arrays {
                self.inspect_uncertain_array_extrema(clause.file, &locals, view, invocation)?;
            }
            let bodies: Vec<_> = clause
                .node
                .child_nodes()
                .filter(|n| n.kind() != NodeKind::LetBlock)
                .collect();
            if bodies.len() != 1 {
                return Err("uncertain-bound let result is unsupported".into());
            }
            if let Some(ids) = private {
                for relation in relations
                    .iter()
                    .filter(|n| n.kind() != NodeKind::CallExpression)
                {
                    self.inspect_uncertain_index_expression(
                        clause.file,
                        relation,
                        view,
                        &clause.generators,
                        &checked_locals,
                        Some(ids),
                    )?;
                }
            } else {
                self.inspect_uncertain_index_expression(
                    clause.file,
                    bodies[0],
                    view,
                    &clause.generators,
                    &checked_locals,
                    None,
                )?;
            }
            Ok(())
        })())
    }
    pub(in crate::callable_definitions) fn inspect_uncertain_array_extrema<'b>(
        &'b self,
        file: FileId,
        locals: &[&'a SyntaxNode],
        view: &'b CallableFacts,
        invocation: Option<&Invocation<'a, 'b>>,
    ) -> Result<(), String> {
        let mut values: Vec<_> = locals
            .iter()
            .flat_map(|local| local.child_nodes())
            .collect();
        while let Some(value) = values.pop() {
            if parameter_index_extremum(
                self.source.context,
                self.source.bindings,
                self.source.view(file, value, view),
                file,
                value,
            )
            .is_some()
            {
                let set = unwrap(value)
                    .child_nodes()
                    .next()
                    .ok_or("uncertain-bound index extremum set unavailable")?;
                let subject = unwrap(set)
                    .child_nodes()
                    .next()
                    .ok_or("uncertain-bound index extremum subject unavailable")?;
                for selected in [unwrap(value), unwrap(set)] {
                    let selected_view = self.source.view(file, selected, view);
                    let call = self
                        .source
                        .operation_fact(selected_view, file, selected)
                        .ok_or("uncertain-bound index extremum selection unavailable")?;
                    let CallOutcome::Resolved {
                        declaration,
                        parameters,
                        return_type,
                    } = &call.outcome
                    else {
                        return Err("uncertain-bound index extremum selection unsupported".into());
                    };
                    if !self.source.prefix_primitive(
                        file,
                        selected,
                        selected_view,
                        &self.source.bindings.declarations[declaration.0].name,
                        parameters,
                        return_type,
                    ) {
                        return Err(
                            "uncertain-bound index extremum written declaration is unsupported"
                                .into(),
                        );
                    }
                }
                if let Some(state) = invocation
                    && self.invocation_array_empty(
                        file,
                        subject,
                        view,
                        state,
                        &mut Vec::new(),
                        true,
                    )? == Some(true)
                {
                    return Err(
                        "uncertain-bound index extremum has a known empty actual array".into(),
                    );
                }
            }
            values.extend(value.child_nodes());
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn inspect_cartesian_parameter_array(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        value: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        checked_locals: &[DeclarationId],
    ) -> Result<(), String> {
        let node = unwrap(value);
        let typed = |n: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, n, view), file, n)
                .map(|e| &e.ty)
        };
        let integer = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if integer(element))
        };
        let array = |t: &TypeInst, rank| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == rank && indices.iter().all(integer) && integer(element))
        };
        let args: Vec<_> = node.child_nodes().collect();
        if written.kind() != NodeKind::ArrayType
            || written.child_nodes().count() != 3
            || node.kind() != NodeKind::CallExpression
            || !self.source.core(file, node, view, "array2d")
            || args.len() != 3
            || !crate::definitions::annotations_safe(self.source.context, file, value)
            || typed(node).is_none_or(|t| !array(t, 2))
            || !self
                .source
                .operation_fact(self.source.view(file, node, view), file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 3 && set(&parameters[0]) && set(&parameters[1])
                        && array(&parameters[2], 1) && array(return_type, 2))
                })
        {
            return Err(
                "uncertain-bound parameter array requires the core rank-two integer array2d".into(),
            );
        }
        let range_ids = |value: &SyntaxNode| {
            let node = unwrap(value);
            let ends: Vec<_> = node.child_nodes().collect();
            if node.kind() != NodeKind::RangeExpression
                || ends.len() != 2
                || !self.source.core(file, node, view, "..")
                || typed(node).is_none_or(|t| !set(t))
                || !crate::definitions::annotations_safe(self.source.context, file, value)
            {
                return None;
            }
            let mut ids = Vec::new();
            for end in ends {
                let end = unwrap(end);
                if end.kind() != NodeKind::Expression
                    || typed(end).is_none_or(|t| !integer(t))
                    || !crate::definitions::annotations_safe(self.source.context, file, end)
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, end).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                {
                    return None;
                }
                let id = self.source.reference(file, end)?;
                if !checked_locals.contains(&id) {
                    return None;
                }
                ids.push(id);
            }
            Some(ids)
        };
        let axes: Vec<_> = written.child_nodes().take(2).collect();
        let collection = unwrap(args[2]);
        let parts: Vec<_> = collection.child_nodes().collect();
        if collection.kind() != NodeKind::ArrayComprehension
            || typed(collection).is_none_or(|t| !array(t, 1))
            || parts.len() != 2
            || parts[1].kind() != NodeKind::GeneratorList
        {
            return Err(
                "uncertain-bound parameter array requires a Cartesian comprehension".into(),
            );
        }
        let headers: Vec<_> = parts[1].child_nodes().collect();
        if headers.len() != 2 {
            return Err("uncertain-bound parameter array requires two Cartesian headers".into());
        }
        for ((axis, argument), header) in axes.iter().zip(&args[..2]).zip(headers) {
            let written: Vec<_> = axis.child_nodes().collect();
            let source: Vec<_> = header.child_nodes().collect();
            let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, header);
            if axis.kind() != NodeKind::DomainType
                || written.len() != 1
                || source.len() != 1
                || crate::domains::generator_slots(&self.source.context.files[file].parsed, header)
                    != 1
                || !matches!(
                    tokens.first().map(|t| t.kind),
                    Some(TokenKind::Identifier | TokenKind::QuotedIdentifier)
                )
                || tokens.get(1).is_none_or(|t| t.kind != TokenKind::In)
                || range_ids(written[0]).is_none()
                || range_ids(written[0]) != range_ids(argument)
                || range_ids(argument) != range_ids(source[0])
            {
                return Err(
                    "uncertain-bound parameter array axes and Cartesian headers do not match"
                        .into(),
                );
            }
        }
        // Inspect every header and body even when an axis might be empty. Unknown
        // cardinality or membership supplies no strict dependency or output proof.
        match self.collection_construction_safety(file, collection, view, generators) {
            DefinitionSafety::Unsupported(reason) => Err(reason),
            _ => Ok(()),
        }
    }
    pub(in crate::callable_definitions) fn inspect_uncertain_index_expression(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        checked_locals: &[DeclarationId],
        private: Option<&[DeclarationId; 5]>,
    ) -> Result<(), String> {
        let node = unwrap(node);
        if !crate::definitions::annotations_safe(self.source.context, file, node) {
            return Err("uncertain-bound expression annotation is unsupported".into());
        }
        let typed = |n: &SyntaxNode| {
            let range = self.source.context.files[file].location(n.range()).range;
            self.source
                .view(file, n, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        let ty = typed(node).ok_or("uncertain-bound expression type unavailable")?;
        if !ty.known() || optional(ty) {
            return Err("uncertain-bound expression type or optionality is unsupported".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        let private_integer_array = private.is_none() && checked_locals.iter().any(|id| {
            let ty = &view.declarations[id.0].ty;
            ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Int)
        });
        if private_integer_array
            && let Some(reason) = self
                .source
                .closed_integer_source_error(file, node, false, false)
        {
            return Err(reason);
        }
        if private.is_some() || private_integer_array {
            let integer = |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
            let parameter_integer =
                |t: &TypeInst| integer(t) && t.instantiation == Instantiation::Parameter;
            if node.kind() == NodeKind::Expression {
                if let Some(id) = self.source.reference(file, node)
                    && checked_locals.contains(&id)
                    && (private.is_some_and(|ids| [ids[1], ids[2], ids[3]].contains(&id))
                        || private_integer_array
                            && ty.instantiation == Instantiation::Parameter
                            && integer(ty))
                {
                    return if ty == &view.declarations[id.0].ty {
                        Ok(())
                    } else {
                        Err("private index alias type is unsupported".into())
                    };
                }
                if ty.kind == TypeKind::Int {
                    return match self.initialized_source_safety(
                        file,
                        node,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    };
                }
            }
            if node.kind() == NodeKind::CallExpression && children.len() == 1 {
                if self.source.core(file, node, view, "index_set")
                    && private.is_some_and(|ids| {
                        self.source.reference(file, unwrap(children[0])) == Some(ids[0])
                    })
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element) if parameter_integer(element))
                {
                    return match self.initialized_source_safety(
                        file,
                        children[0],
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    };
                }
                if parameter_integer(ty)
                    && private.is_some_and(|ids| {
                        self.source.reference(file, unwrap(children[0])) == Some(ids[1])
                            && checked_locals.contains(&ids[1])
                    })
                {
                    if self.source.core(file, node, view, "min") {
                        return match self
                            .parameter_set_extremum_safety(file, node, view, generators)
                        {
                            Some(DefinitionSafety::Unsupported(reason)) => Err(reason),
                            Some(_) => Ok(()),
                            None => Err("private index extremum signature is unsupported".into()),
                        };
                    }
                    if self.source.core(file, node, view, "card")
                        && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call|
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 1 && typed(children[0]) == Some(&parameters[0])
                                    && return_type == ty)) {
                        return Ok(());
                    }
                }
            }
            if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 && integer(ty)
            {
                let subject = unwrap(children[0]);
                let array = self.source.reference(file, subject);
                if !array.is_some_and(|id| private.is_some_and(|ids| id == ids[0] || id == ids[4] && checked_locals.contains(&id))
                    || private_integer_array && (checked_locals.contains(&id)
                        || self.source.bindings.declarations[id.0].file == file
                            && self.source.bindings.declarations[id.0].role == DeclarationRole::Parameter
                            && checked_locals.iter().any(|local| self.source.bindings.declarations[local.0].item == self.source.bindings.declarations[id.0].item)))
                    || typed(subject).is_none_or(|t| !t.known() || optional(t)
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && parameter_integer(&indices[0]) && integer(element)
                                && element.instantiation == Instantiation::Decision))
                    || typed(children[1]).is_none_or(|t| !integer(t)) {
                    return Err("private integer selection type or scope is unsupported".into());
                }
                if private_integer_array && array.is_some_and(|id| !checked_locals.contains(&id)) {
                    let selector = unwrap(children[1]);
                    let binder = self.source.reference(file, selector);
                    if selector.kind() != NodeKind::Expression
                        || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, selector).as_slice(),
                            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                        || typed(selector).is_none_or(|ty| !parameter_integer(ty))
                        || binder.is_none_or(|id| {
                            let declaration = &self.source.bindings.declarations[id.0];
                            declaration.file != file
                                || declaration.role != DeclarationRole::Generator
                                || typed(selector) != Some(&view.declarations[id.0].ty)
                                || !generators
                                    .iter()
                                    .any(|header| header.range() == declaration.syntax_range)
                        })
                    {
                        return Err("uncertain-bound formal integer selection requires an owning parameter integer binder".into());
                    }
                }
                self.inspect_uncertain_index_expression(
                    file,
                    children[1],
                    view,
                    generators,
                    checked_locals,
                    private,
                )?;
                if let Some(reason) =
                    self.source
                        .closed_integer_source_error(file, children[1], false, false)
                {
                    return Err(reason);
                }
                if let Some(array) = array
                    && let Domain::Array { indices, .. } = crate::domains::bare_index_domain(
                        &self.source.domains.declarations[array.0].domain,
                    )
                    && crate::domains::invariant_expression_integer(
                        self.source.context,
                        self.source.bindings,
                        file,
                        children[1],
                    )
                    .is_ok_and(|value| {
                        value.is_some_and(|value| {
                            crate::domains::index_domain_member(&indices[0], value) == Some(false)
                                || private_integer_array && matches!(crate::domains::bare_index_domain(&indices[0]), Domain::Range { lower, upper }
                                    if crate::domains::invariant_integer(lower).is_ok_and(|bound| bound.is_some_and(|bound| value < bound))
                                        || crate::domains::invariant_integer(upper).is_ok_and(|bound| bound.is_some_and(|bound| value > bound)))
                        })
                    })
                {
                    return Err("private integer selection is outside its declared axis".into());
                }
                // The declaration/actual source and every selector were inspected.
                // This is neither a membership nor an output dependency proof.
                return Ok(());
            }
            if private.is_some() && node.kind() == NodeKind::ConditionalExpression && integer(ty) {
                let mut complete = false;
                for (position, branch) in children.iter().enumerate() {
                    if !crate::definitions::annotations_safe(self.source.context, file, branch) {
                        return Err("private integer conditional annotation is unsupported".into());
                    }
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                        if typed(parts[0])
                            .is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
                        {
                            return Err("private integer conditional guard is unsupported".into());
                        }
                        self.inspect_uncertain_index_expression(
                            file,
                            parts[0],
                            view,
                            generators,
                            checked_locals,
                            private,
                        )?;
                        parts[1]
                    } else if branch.kind() == NodeKind::ElseBranch
                        && parts.len() == 1
                        && position + 1 == children.len()
                    {
                        complete = true;
                        parts[0]
                    } else {
                        return Err("private integer conditional branch is unsupported".into());
                    };
                    if typed(body).is_none_or(|t| !integer(t) || !crate::types::coerces(t, ty)) {
                        return Err("private integer conditional body is unsupported".into());
                    }
                    self.inspect_uncertain_index_expression(
                        file,
                        body,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                }
                return if complete {
                    Ok(())
                } else {
                    Err("private integer conditional requires an else".into())
                };
            }
            if node.kind() == NodeKind::BinaryExpression && integer(ty) && children.len() == 2 {
                let name = operator(self.source.context, file, node)
                    .and_then(crate::bindings::symbolic_operator);
                if matches!(name, Some("+" | "-")) && self.source.core(file, node, view, name.unwrap())
                    && children.iter().all(|n| typed(n).is_some_and(integer))
                    && self.source.operation_fact(self.source.view(file, node, view), file, node).is_some_and(|call|
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && return_type == ty && parameters.iter().zip(&children)
                                .all(|(formal, actual)| integer(formal) && typed(actual).is_some_and(|t| crate::types::coerces(t, formal))))) {
                    if private_integer_array && self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if self.source.prefix_primitive(file, node, self.source.view(file, node, view), name.unwrap(), parameters, return_type))) {
                        return Err("uncertain-bound integer operator written declaration is unsupported".into());
                    }
                    for child in &children {
                        self.inspect_uncertain_index_expression(file, child, view, generators, checked_locals, private)?;
                    }
                    if let Some(reason) = self.source.closed_integer_source_error(file, node, false, false) { return Err(reason); }
                    return Ok(());
                }
            }
            if integer(ty) {
                if private_integer_array && node.kind() != NodeKind::Expression {
                    return Err("uncertain-bound integer source form is unsupported".into());
                }
                return match self.initialized_source_safety(
                    file,
                    node,
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    DefinitionSafety::Unsupported(reason) => Err(reason),
                    _ => Ok(()),
                };
            }
        }
        if node.kind() == NodeKind::RangeExpression
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
            && children.len() == 2
            && self.source.core(file, node, view, "..")
            && children.iter().all(|child| {
                typed(child).is_some_and(|t| {
                    t.known()
                        && !optional(t)
                        && t.instantiation == Instantiation::Parameter
                        && t.kind == TypeKind::Int
                })
            })
        {
            if private_integer_array && self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if self.source.prefix_primitive(file, node, self.source.view(file, node, view), "..", parameters, return_type))) {
                return Err("uncertain-bound range written declaration is unsupported".into());
            }
            if private.is_some() || private_integer_array {
                for child in &children {
                    self.inspect_uncertain_index_expression(
                        file,
                        child,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                }
                return Ok(());
            }
            return match self.direct_children_safety(file, &children, view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if node.kind() == NodeKind::CallExpression
            && self.source.core(file, node, view, "index_set")
            && children.len() == 1
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
        {
            let subject = unwrap(children[0]);
            let declaration = self
                .source
                .reference(file, subject)
                .map(|id| &self.source.bindings.declarations[id.0]);
            if subject.kind() != NodeKind::Expression
                || declaration.is_none_or(|d| {
                    d.file != file
                        || !matches!(d.role, DeclarationRole::Parameter | DeclarationRole::Local)
                })
                || typed(subject).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter
                            && element.kind == TypeKind::Bool)
                })
            {
                return Err("uncertain-bound index-set subject is unsupported".into());
            }
            self.dependencies(file, subject, view, generators)?;
            return Ok(());
        }
        if node.kind() == NodeKind::ArrayAccessExpression
            && children.len() == 3
            && ty.kind == TypeKind::Int
        {
            let subject = unwrap(children[0]);
            let array = self
                .source
                .reference(file, subject)
                .filter(|id| checked_locals.contains(id))
                .ok_or("uncertain-bound parameter array has not been inspected")?;
            if subject.kind() != NodeKind::Expression || typed(subject).is_none_or(|t|
                !t.known() || optional(t) || t.instantiation != Instantiation::Parameter
                    || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(|index| index.known() && !optional(index)
                            && index.instantiation == Instantiation::Parameter && index.kind == TypeKind::Int)
                            && element.known() && !optional(element) && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Int))
                || self.source.bindings.declarations[array.0].role != DeclarationRole::Local
                || children.iter().any(|n| unwrap(n).kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(n)).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
            { return Err("uncertain-bound parameter array selection is unsupported".into()); }
            let binder = self
                .source
                .reference(file, unwrap(children[1]))
                .ok_or("uncertain-bound selector identity unavailable")?;
            let choice = self
                .source
                .reference(file, unwrap(children[2]))
                .filter(|id| checked_locals.contains(id))
                .ok_or("uncertain-bound private selector has not been inspected")?;
            if self.source.bindings.declarations[binder.0].role != DeclarationRole::Generator
                || !generators
                    .iter()
                    .any(|g| g.range() == self.source.bindings.declarations[binder.0].syntax_range)
                || self.source.bindings.declarations[choice.0].role != DeclarationRole::Local
                || typed(children[1]).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || t.instantiation != Instantiation::Parameter
                        || t.kind != TypeKind::Int
                })
                || typed(children[2]).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || t.instantiation != Instantiation::Decision
                        || t.kind != TypeKind::Int
                })
            {
                return Err("uncertain-bound parameter array selectors are unsupported".into());
            }
            // The initializer and private domain were checked in declaration order.
            // Inspect both selectors without reopening that partial initializer.
            return match self.direct_children_safety(file, &children[1..], view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if ty.kind != TypeKind::Bool {
            return Err("uncertain-bound body requires a present Boolean".into());
        }
        if !private_integer_array
            && self
                .boolean_relation_dependencies(file, node, view, generators)
                .is_ok()
        {
            return Ok(());
        }
        if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 {
            let subject = unwrap(children[0]);
            if subject.kind() != NodeKind::Expression || self.source.reference(file, subject).is_none()
                || typed(subject).is_none_or(|t| !t.known() || optional(t)
                    || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Bool))
                || typed(children[1]).is_none_or(|t| !t.known() || optional(t)
                    || t.instantiation != Instantiation::Parameter || t.kind != TypeKind::Int)
            { return Err("uncertain-bound Boolean selection type is unsupported".into()); }
            self.dependencies(file, subject, view, generators)?;
            return match self.direct_safety(file, children[1], view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(file, node, view, "forall")
        {
            if private_integer_array && self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if self.source.prefix_primitive(file, node, self.source.view(file, node, view), "forall", parameters, return_type))) {
                return Err("uncertain-bound forall written declaration is unsupported".into());
            }
            let list = children
                .iter()
                .find(|n| n.kind() == NodeKind::GeneratorList)
                .ok_or("uncertain-bound forall generators unavailable")?;
            let mut all = generators.to_vec();
            for header in list.child_nodes() {
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
                if binders.len() != 1 || header.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                    || !header.children().iter().any(|c| matches!(c, SyntaxElement::Token(i) if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In))
                    || !crate::definitions::annotations_safe(self.source.context, file, header)
                    || !view.declarations[binders[0].id.0].ty.known()
                    || view.declarations[binders[0].id.0].ty.kind != TypeKind::Int
                    || view.declarations[binders[0].id.0].ty.instantiation != Instantiation::Parameter
                    || optional(&view.declarations[binders[0].id.0].ty)
                { return Err("uncertain-bound forall binder, annotation or filter is unsupported".into()); }
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("uncertain-bound forall source unavailable")?;
                self.inspect_uncertain_index_expression(
                    file,
                    source,
                    view,
                    &all,
                    checked_locals,
                    private,
                )?;
                all.push(header);
            }
            let body = children
                .iter()
                .find(|n| n.kind() != NodeKind::GeneratorList)
                .ok_or("uncertain-bound forall body unavailable")?;
            return self.inspect_uncertain_index_expression(
                file,
                body,
                view,
                &all,
                checked_locals,
                private,
            );
        }
        let name =
            operator(self.source.context, file, node).and_then(crate::bindings::symbolic_operator);
        let supported = match node.kind() {
            NodeKind::BinaryExpression => {
                children.len() == 2
                    && matches!(
                        name,
                        Some(
                            "=" | "!="
                                | "<"
                                | "<="
                                | ">"
                                | ">="
                                | "/\\"
                                | "\\/"
                                | "<->"
                                | "->"
                                | "<-"
                        )
                    )
            }
            NodeKind::UnaryExpression => children.len() == 1 && name == Some("not"),
            _ => false,
        };
        if !supported || !self.source.core(file, node, view, name.unwrap()) {
            return Err("uncertain-bound Boolean operation identity is unsupported".into());
        }
        if private_integer_array && self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call|
            !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                if self.source.prefix_primitive(file, node, self.source.view(file, node, view), name.unwrap(), parameters, return_type))) {
            return Err("uncertain-bound Boolean operator written declaration is unsupported".into());
        }
        if node.kind() == NodeKind::BinaryExpression
            && matches!(name, Some("=" | "!=" | "<" | "<=" | ">" | ">="))
            && children.iter().all(|child| {
                typed(child).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            })
        {
            // A supported partial numeric operand can collapse to false at this
            // core Boolean relation; it supplies no value or output guarantee.
            for child in children {
                let value = unwrap(child);
                if private.is_some()
                    || private_integer_array
                    || (value.kind() == NodeKind::ArrayAccessExpression
                        && value.child_nodes().count() == 3
                        && value
                            .child_nodes()
                            .next()
                            .and_then(|subject| self.source.reference(file, unwrap(subject)))
                            .is_some_and(|id| checked_locals.contains(&id)))
                {
                    self.inspect_uncertain_index_expression(
                        file,
                        value,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                } else if let DefinitionSafety::Unsupported(reason) =
                    self.direct_safety(file, child, view, generators)
                {
                    return Err(reason);
                }
            }
            return Ok(());
        }
        for child in children {
            self.inspect_uncertain_index_expression(
                file,
                child,
                view,
                generators,
                checked_locals,
                private,
            )?;
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn default_collection_arguments(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(Vec<DeclarationId>, &'a SyntaxNode, Vec<&'a SyntaxNode>), String> {
        let node = unwrap(node);
        let ty = |node: &SyntaxNode| {
            self.source
                .view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range
                            == self.source.context.files[file].location(node.range()).range
                })
                .map(|e| &e.ty)
        };
        let present_array = |t: &TypeInst| {
            t.known()
                && !t.optional
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                    && element.known() && !optional(element) && element.kind == TypeKind::Int)
        };
        if !self.source.default_collection(file, node) || ty(node).is_none_or(|t| !present_array(t))
        {
            return Err("default collection requires a present rank-one integer result".into());
        }
        let list = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
            .ok_or("default generator is unavailable")?;
        let headers: Vec<_> = list.child_nodes().collect();
        if headers.len() != 1
            || headers[0]
                .child_nodes()
                .any(|n| n.kind() == NodeKind::WhereFilter)
            || !headers[0].children().iter().any(|c| {
                matches!(c, SyntaxElement::Token(i)
                if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
            })
        {
            return Err("default collection requires one unfiltered in generator".into());
        }
        let source = headers[0]
            .child_nodes()
            .next()
            .map(unwrap)
            .ok_or("default source is unavailable")?;
        let source_id = self
            .source
            .reference(file, source)
            .filter(|_| source.kind() == NodeKind::Expression)
            .ok_or("default collection source must be a bare array identity")?;
        let source_type = ty(source).ok_or("default source type is unavailable")?;
        if !source_type.known()
            || source_type.optional
            || !matches!(&source_type.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                    && element.known() && element.kind == TypeKind::Int)
        {
            return Err(
                "default collection source must retain known integer storage and axis types".into(),
            );
        }
        let declaration = &self.source.bindings.declarations[source_id.0];
        if !matches!(
            declaration.role,
            DeclarationRole::Value | DeclarationRole::Parameter
        ) {
            return Err("default collection source scope is unsupported".into());
        }
        let written = find_node(
            self.source.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("default array declaration is unavailable")?;
        if !crate::definitions::annotations_safe(self.source.context, declaration.file, written) {
            return Err("default array source annotations are unsupported".into());
        }
        let mut ids = self.type_dependencies(
            declaration.file,
            written,
            view,
            if declaration.file == file {
                generators
            } else {
                &[]
            },
        )?;
        extend(&mut ids, vec![source_id]);
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
            .map(unwrap)
            .ok_or("default body is unavailable")?;
        let parts: Vec<_> = body.child_nodes().map(unwrap).collect();
        let binders: Vec<_> = self
            .source
            .bindings
            .declarations
            .iter()
            .filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == headers[0].range()
            })
            .collect();
        if parts.len() != 2 || binders.len() != 1 || parts[0].kind() != NodeKind::Expression
            || self.source.reference(file, parts[0]) != Some(binders[0].id)
            || !self.source.core(file, body, view, "default")
            || ty(parts[0]).is_none_or(|t| !t.known() || t.kind != TypeKind::Int)
            || ty(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
            || ty(parts[1]).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int
                || t.instantiation != Instantiation::Parameter)
            || self.source.operation_fact(self.source.view(file, body, view), file, body).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(|t| t.known() && t.kind == TypeKind::Int)
                        && !optional(&parameters[1]) && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Int)
            })
        {
            return Err("default collection body, binder identity or selected signature is unsupported".into());
        }
        let mut all = generators.to_vec();
        all.extend(headers);
        Ok((ids, parts[1], all))
    }
    // Inspect collection construction without turning unproved index
    // membership, cardinality or filtered iteration into output dependencies.
    pub(in crate::callable_definitions) fn collection_construction_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let checked = (|| -> Result<(), String> {
            let facts = self.source.view(file, node, view);
            let typed = |value: &SyntaxNode| {
                let range = self.source.context.files[file]
                    .location(value.range())
                    .range;
                facts
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .map(|e| &e.ty)
            };
            let present_parameter = |t: &TypeInst| {
                t.known() && !optional(t) && t.instantiation == Instantiation::Parameter
            };
            let present = |t: &TypeInst| t.known() && !optional(t);
            let integer_set = |t: &TypeInst| {
                present_parameter(t)
                    && matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
            };
            let ty = typed(node).ok_or("parameter collection type unavailable")?;
            let enum_values = node.kind() == NodeKind::ArrayComprehension
                && present(ty)
                && ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && present_parameter(&indices[0])
                        && indices[0].kind == TypeKind::Int && present(element)
                        && element.instantiation == Instantiation::Decision
                        && matches!(element.kind, TypeKind::Enum(_)));
            let decision_sets = node.kind() == NodeKind::ArrayComprehension
                && ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { element, .. }
                    if decision_integer_set(element));
            let decision_union = node.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, node, view, "array_union")
                && decision_integer_set(ty);
            let element = match (&ty.kind, node.kind()) {
                (TypeKind::Set(element), NodeKind::SetComprehension) if integer_set(ty) => {
                    element.as_ref()
                }
                (TypeKind::Array { indices, element }, NodeKind::ArrayComprehension)
                    if present(ty)
                        && (ty.instantiation == Instantiation::Parameter
                            || matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                            || decision_sets
                            || enum_values)
                        && indices.len() == 1
                        && present_parameter(&indices[0])
                        && indices[0].kind == TypeKind::Int
                        && (matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                            || integer_set(element)
                            || decision_sets
                            || enum_values) =>
                {
                    element.as_ref()
                }
                (TypeKind::Bool, NodeKind::GeneratorCallExpression)
                    if present(ty)
                        && (self.source.core(file, node, view, "forall")
                            || self.source.core(file, node, view, "exists")) =>
                {
                    ty
                }
                (TypeKind::Int, NodeKind::GeneratorCallExpression)
                    if present(ty) && self.source.core(file, node, view, "sum") =>
                {
                    ty
                }
                (TypeKind::Set(_), NodeKind::GeneratorCallExpression) if decision_union => ty,
                _ => {
                    return Err(
                        "parameter collection shape, type or identity is unsupported".into(),
                    );
                }
            };
            let children: Vec<_> = node.child_nodes().collect();
            let [first, second] = children.as_slice() else {
                return Err("parameter collection body or headers are unsupported".into());
            };
            let (body, list) = if node.kind() == NodeKind::GeneratorCallExpression {
                (second, first)
            } else {
                (first, second)
            };
            if list.kind() != NodeKind::GeneratorList
                || list.child_nodes().count() == 0
                || typed(body).is_none_or(|t| {
                    !present(t)
                        || if decision_sets || decision_union || enum_values {
                            t != element
                        } else if node.kind() == NodeKind::GeneratorCallExpression
                            && element.kind == TypeKind::Int
                        {
                            !matches!(t.kind, TypeKind::Int | TypeKind::Bool)
                                || !crate::types::coerces(t, element)
                        } else {
                            t.kind != element.kind
                        }
                })
            {
                return Err("parameter collection body type is unsupported".into());
            }
            if (decision_sets || decision_union || enum_values)
                && unwrap(body).kind() != NodeKind::ArrayAccessExpression
            {
                return Err(if enum_values {
                    "enum value collection requires a selected array cell"
                } else {
                    "decision set collection requires a selected array cell"
                }
                .into());
            }
            if decision_union && list.child_nodes().count() != 1 {
                return Err("decision set union requires one unfiltered integer header".into());
            }
            // Decision-set generators introduce absent cells; this exact core
            // overload may inspect the sum without proving which cells occur.
            let optional_sum = node.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, node, view, "sum")
                && present(ty) && ty.kind == TypeKind::Int
                && ty.instantiation == Instantiation::Decision
                && typed(body).is_some_and(|t| present(t) && t.kind == TypeKind::Int)
                && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if return_type == ty && parameters.len() == 1
                            && parameters[0].known() && !parameters[0].optional
                            && parameters[0].instantiation == Instantiation::Decision
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && present_parameter(&indices[0])
                                    && indices[0].kind == TypeKind::Int && element.known()
                                    && element.optional && element.instantiation == Instantiation::Decision
                                    && element.kind == TypeKind::Int))
                });
            if optional_sum && list.child_nodes().count() != 1 {
                return Err("optional sum requires one decision enum set header".into());
            }
            if node.kind() == NodeKind::GeneratorCallExpression
                && self.source.operation_fact(facts, file, node).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && return_type == ty
                            && (present(&parameters[0]) || optional_sum)
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                                if indices.len() == 1 && present_parameter(&indices[0])
                                    && indices[0].kind == TypeKind::Int
                                    && (present(formal) || optional_sum)
                                    && formal.kind == element.kind
                                    && (!decision_union || parameters[0].instantiation == Instantiation::Decision
                                        && formal.as_ref() == element)
                                    && typed(body).is_some_and(|actual| crate::types::coerces(actual, formal))))
                })
            {
                return Err("collection selected signature is unsupported".into());
            }
            let optional_range_sum = optional_sum
                && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                        if call.generator_argument.as_ref() == parameters.first())
                })
                && list.child_nodes().all(|header| {
                    let values: Vec<_> = header.child_nodes().collect();
                    let [source] = values.as_slice() else { return false; };
                    let range = unwrap(source);
                    let bounds: Vec<_> = range.child_nodes().collect();
                    range.kind() == NodeKind::RangeExpression
                        && bounds.len() == 2
                        && self.source.core(file, range, view, "..")
                        && typed(range).is_some_and(decision_integer_set)
                        && self.source.operation_fact(facts, file, range).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2
                                    && parameters.iter().all(|t| present(t)
                                        && t.instantiation == Instantiation::Decision && t.kind == TypeKind::Int)
                                    && typed(range) == Some(return_type)
                                    && parameters.iter().zip(&bounds).all(|(formal, actual)|
                                        typed(actual).is_some_and(|t| present(t) && t.kind == TypeKind::Int
                                            && crate::types::coerces(t, formal))))
                        })
                });
            let enum_sum = !optional_sum
                && node.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, node, view, "sum")
                && list.child_nodes().any(|header| {
                    header.child_nodes().next().is_some_and(|source| {
                        typed(source).is_some_and(|t| {
                            present_parameter(t)
                                && matches!(&t.kind, TypeKind::Set(element)
                                    if present_parameter(element)
                                        && matches!(element.kind, TypeKind::Enum(_)))
                        })
                    })
                });
            let enum_collection = node.kind() == NodeKind::ArrayComprehension
                && element.kind == TypeKind::Bool
                && list.child_nodes().any(|header| {
                    header.child_nodes().next().is_some_and(|source| {
                        typed(source).is_some_and(|t| {
                            present_parameter(t)
                                && matches!(&t.kind, TypeKind::Array { element, .. }
                            if matches!(&element.kind, TypeKind::Set(member)
                                if matches!(member.kind, TypeKind::Enum(_))))
                        })
                    })
                });
            let enum_array_collection = node.kind() == NodeKind::ArrayComprehension
                && element.kind == TypeKind::Bool
                && list.child_nodes().count() == 1
                && list.child_nodes().any(|header| {
                    header.child_nodes().next().is_some_and(|source| {
                        typed(source).is_some_and(|t| {
                            present(t)
                                && t.instantiation == Instantiation::Decision
                                && matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && present_parameter(&indices[0])
                                && indices[0].kind == TypeKind::Int && present(element)
                                && element.instantiation == Instantiation::Decision
                                && matches!(element.kind, TypeKind::Enum(_)))
                        })
                    })
                });
            let mut nodes = vec![node];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, current) {
                    return Err("parameter collection annotation is unsupported".into());
                }
                if (enum_collection || enum_array_collection)
                    && current.kind() == NodeKind::BinaryExpression
                    && let Some(name) = operator(self.source.context, file, current)
                        .and_then(crate::bindings::symbolic_operator)
                    && matches!(name, "=" | "!=" | "<" | "<=" | ">" | ">=")
                {
                    let Some(call) = self.source.operation_fact(facts, file, current) else {
                        return Err("enum collection comparison selection unavailable".into());
                    };
                    let CallOutcome::Resolved {
                        parameters,
                        return_type,
                        ..
                    } = &call.outcome
                    else {
                        return Err("enum collection comparison selection unsupported".into());
                    };
                    let operands: Vec<_> = current.child_nodes().collect();
                    if parameters.len() != 2
                        || enum_array_collection
                            && (operands.len() != 2
                                || typed(operands[0]).is_none_or(|left| {
                                    !matches!(left.kind, TypeKind::Enum(_))
                                        || typed(operands[1])
                                            .is_none_or(|right| right.kind != left.kind)
                                }))
                        || !present(return_type)
                        || return_type.kind != TypeKind::Bool
                        || parameters.iter().zip(&operands).any(|(formal, actual)| {
                            typed(actual).is_none_or(|t| !present(t) || t.kind != formal.kind)
                        })
                        || !self.source.prefix_primitive(
                            file,
                            current,
                            facts,
                            name,
                            parameters,
                            return_type,
                        )
                    {
                        return Err(
                            "enum collection comparison written primitive unsupported".into()
                        );
                    }
                }
                nodes.extend(current.child_nodes());
            }
            let conditional_relation = node.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, node, view, "forall")
                && unwrap(body).kind() == NodeKind::ConditionalExpression;
            let mut all = generators.to_vec();
            let mut unsupported = None;
            if decision_sets
                || decision_union
                || enum_sum
                || optional_range_sum
                || enum_collection
                || enum_array_collection
                || enum_values
            {
                for (position, header) in generators.iter().enumerate() {
                    for value in header.child_nodes() {
                        let (value, scope) = if value.kind() == NodeKind::WhereFilter {
                            let conditions: Vec<_> = value.child_nodes().collect();
                            let [condition] = conditions.as_slice() else {
                                return Err("collection lexical filter is unsupported".into());
                            };
                            (*condition, &generators[..=position])
                        } else {
                            (value, &generators[..position])
                        };
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(file, value, view, scope, &mut Vec::new())
                        {
                            unsupported = Some(reason);
                        }
                        if let Some(reason) = self
                            .source
                            .closed_integer_source_error(file, value, true, true)
                        {
                            unsupported = Some(reason);
                        }
                    }
                }
            }
            for header in list.child_nodes() {
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
                let enum_header = enum_collection
                    && self
                        .source
                        .enum_collection_header(file, header, facts, &all);
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("parameter collection source unavailable")?;
                let enum_array_header = enum_array_collection
                    && binders.len() == 1
                    && header.child_nodes().count() == 1
                    && unwrap(source).kind() == NodeKind::Expression
                    && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, unwrap(source)).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    && self
                        .source
                        .reference(file, unwrap(source))
                        .is_some_and(|id| {
                            let declaration = &self.source.bindings.declarations[id.0];
                            declaration.file == file
                                && declaration.top_level
                                && declaration.role == DeclarationRole::Value
                                && typed(source).is_some_and(|t| {
                                    t == &facts.declarations[id.0].ty
                                        && matches!(&t.kind, TypeKind::Array { element, .. }
                                    if &facts.declarations[binders[0].id.0].ty == element.as_ref())
                                })
                        });
                let enum_value_header = enum_values && typed(source).is_some_and(|t|
                    present_parameter(t) && matches!(&t.kind, TypeKind::Set(element)
                        if present_parameter(element) && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                            && binders.iter().all(|binder| &facts.declarations[binder.id.0].ty == element.as_ref())));
                if !header.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                }) || binders.is_empty()
                    || optional_sum && binders.len() != 1
                    || decision_union && (binders.len() != 1 || header.child_nodes().count() != 1)
                    || binders.len()
                        != crate::domains::generator_slots(
                            &self.source.context.files[file].parsed,
                            header,
                        )
                    || !enum_header
                        && !enum_array_header
                        && !enum_value_header
                        && binders.iter().any(|d| {
                            let t = &facts.declarations[d.id.0].ty;
                            !present_parameter(t)
                                || if optional_range_sum {
                                    t.kind != TypeKind::Int
                                } else if optional_sum {
                                    !matches!(t.kind, TypeKind::Enum(_))
                                } else {
                                    t.kind != TypeKind::Int
                                        && !(enum_sum && matches!(t.kind, TypeKind::Enum(_)))
                                }
                        })
                {
                    return Err(if optional_sum {
                        "optional sum requires named parameter enum binders".into()
                    } else {
                        "parameter collection requires named integer in binders".into()
                    });
                }
                if optional_sum && !optional_range_sum {
                    let source_id = self
                        .source
                        .reference(file, unwrap(source))
                        .filter(|id| {
                            unwrap(source).kind() == NodeKind::Expression
                                && self.source.bindings.declarations[id.0].top_level
                                && self.source.bindings.declarations[id.0].role
                                    == DeclarationRole::Value
                        })
                        .ok_or("optional sum requires a bare decision enum set source")?;
                    if typed(source).is_none_or(|t| !present(t)
                        || t != &self.source.calls.declarations[source_id.0].ty
                        || t.instantiation != Instantiation::Decision
                        || !matches!(&t.kind, TypeKind::Set(element)
                            if present_parameter(element) && matches!(element.kind, TypeKind::Enum(_))
                                && binders.iter().all(|binder| facts.declarations[binder.id.0].ty.kind == element.kind)))
                    {
                        return Err("optional sum source and binder enum identities differ".into());
                    }
                } else if !optional_range_sum
                    && !enum_header
                    && !enum_array_header
                    && !enum_value_header
                    && typed(source).is_none_or(|t| {
                        !(integer_set(t)
                            && binders.iter().all(|binder| {
                                facts.declarations[binder.id.0].ty.kind == TypeKind::Int
                            }))
                            && !(enum_sum
                                && present_parameter(t)
                                && matches!(&t.kind, TypeKind::Set(element)
                                if present_parameter(element)
                                    && matches!(element.kind, TypeKind::Enum(_))
                                    && binders.iter().all(|binder|
                                        &facts.declarations[binder.id.0].ty == element.as_ref())))
                    })
                {
                    return Err(if enum_sum {
                        "sum requires matching present parameter set sources and binders"
                    } else {
                        "parameter collection requires a present parameter integer set source"
                    }
                    .into());
                }
                let source_safety = if optional_range_sum {
                    // Inspect both endpoints without proving range membership or extent.
                    let bounds: Vec<_> = unwrap(source).child_nodes().collect();
                    self.initialized_children_safety(file, &bounds, view, &all)
                } else {
                    self.source
                        .selected_generator_source_safety(file, header, view, &all)
                        .unwrap_or_else(|| {
                            self.initialized_source_safety(
                                file,
                                source,
                                view,
                                &all,
                                &mut Vec::new(),
                            )
                        })
                };
                if let DefinitionSafety::Unsupported(reason) = source_safety {
                    unsupported = Some(reason);
                } else if (conditional_relation
                    || optional_sum
                    || decision_sets
                    || decision_union
                    || enum_sum
                    || enum_collection
                    || enum_array_collection
                    || enum_values)
                    && let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, source, true, true)
                {
                    unsupported = Some(reason);
                }
                all.push(header);
                for filter in header
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let conditions: Vec<_> = filter.child_nodes().collect();
                    let [condition] = conditions.as_slice() else {
                        return Err("parameter collection filter is unsupported".into());
                    };
                    if typed(condition)
                        .is_none_or(|t| !present_parameter(t) || t.kind != TypeKind::Bool)
                    {
                        return Err("parameter collection filter type is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, condition, view, &all, &mut Vec::new())
                    {
                        unsupported = Some(reason);
                    } else if (conditional_relation
                        || optional_sum
                        || decision_sets
                        || decision_union
                        || enum_sum
                        || enum_collection
                        || enum_array_collection
                        || enum_values)
                        && let Some(reason) = self
                            .source
                            .closed_integer_source_error(file, condition, true, true)
                    {
                        unsupported = Some(reason);
                    }
                }
            }
            let body_safety = if decision_union {
                let cell = unwrap(body);
                let cells: Vec<_> = cell.child_nodes().collect();
                let [subject, selector] = cells.as_slice() else {
                    return Err("decision set union requires a rank-one selected cell".into());
                };
                let subject = unwrap(subject);
                let id = self.source.reference(file, subject)
                    .filter(|_| subject.kind() == NodeKind::Expression
                        && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                    .ok_or("decision set union source identity is unavailable")?;
                let declaration = &self.source.bindings.declarations[id.0];
                let source =
                    typed(subject).ok_or("decision set union source type is unavailable")?;
                if declaration.role != DeclarationRole::Parameter
                    || declaration.file != file
                    || !present(source)
                    || source != &facts.declarations[id.0].ty
                    || source.instantiation != Instantiation::Decision
                    || !matches!(&source.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && present_parameter(&indices[0])
                            && indices[0].kind == TypeKind::Int && element.as_ref() == ty)
                    || typed(selector)
                        .is_none_or(|t| !present_parameter(t) || t.kind != TypeKind::Int)
                {
                    return Err(
                        "decision set union formal source or selector type is unsupported".into(),
                    );
                }
                let owner = self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .find(|owner| {
                        owner.file == file
                            && owner.item == declaration.item
                            && matches!(
                                owner.role,
                                DeclarationRole::Function
                                    | DeclarationRole::Predicate
                                    | DeclarationRole::Test
                            )
                            && owner.syntax_range.start <= node.range().start
                            && node.range().end <= owner.syntax_range.end
                    })
                    .ok_or("decision set union owning callable is unavailable")?;
                let written = find_node(
                    self.source.context.files[file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("decision set union owning declaration is unavailable")?;
                if !written
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::ParameterList)
                    .is_some_and(|list| {
                        list.child_nodes()
                            .any(|p| p.range() == declaration.syntax_range)
                    })
                    || written
                        .child_nodes()
                        .filter(|n| is_expression(n.kind()))
                        .last()
                        .is_none_or(|body| {
                            !(body.range().start <= node.range().start
                                && node.range().end <= body.range().end)
                        })
                {
                    return Err("decision set union source is not its owning body formal".into());
                }
                let formal = find_node(
                    self.source.context.files[file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
                .ok_or("decision set union written formal is unavailable")?;
                if !crate::definitions::annotations_safe(self.source.context, file, formal)
                    || formal.child_nodes().any(|n| is_expression(n.kind()))
                {
                    return Err(
                        "decision set union formal annotation or default is unsupported".into(),
                    );
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, formal, true, true)
                {
                    return Err(reason);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, selector, true, true)
                {
                    return Err(reason);
                }
                let header = list.child_nodes().next().unwrap();
                let axis = header.child_nodes().next().unwrap();
                if self.source.reference(file, unwrap(selector)).is_none_or(|binder| {
                    let binder = &self.source.bindings.declarations[binder.0];
                    binder.file != file || binder.role != DeclarationRole::Generator
                        || binder.syntax_range != header.range()
                }) || !self.source.member_index(file, id, selector, &all, view)
                    || !self.source.core(file, axis, view, "index_set")
                    || axis.child_nodes().count() != 1
                    || self.source.operation_fact(facts, file, axis).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.as_slice() == [source.clone()]
                                && typed(axis) == Some(return_type) && present_parameter(return_type)
                                && matches!(&return_type.kind, TypeKind::Set(element)
                                    if present_parameter(element) && element.kind == TypeKind::Int)))
                {
                    return Err("decision set union selector lacks its same-formal index_set header".into());
                }
                let domain = crate::domains::bare_index_domain(
                    &self.source.domains.declarations[id.0].domain,
                );
                let Domain::Array { indices, element } = domain else {
                    return Err("decision set union formal array domain is unsupported".into());
                };
                if indices.len() != 1 {
                    return Err("decision set union formal array rank is unsupported".into());
                }
                indices[0].numeric_minimum().map_err(str::to_owned)?;
                let Domain::Set(member) = crate::domains::bare_index_domain(element) else {
                    return Err("decision set union formal member domain is unsupported".into());
                };
                member.numeric_minimum().map_err(str::to_owned)?;
                self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
            } else if decision_sets {
                // Exact traversal dependencies do not inspect the written set domain.
                self.decision_set_source_safety(file, body, view, &all, &mut Vec::new())
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported(
                            "decision set collection body is unsupported".into(),
                        )
                    })
            } else if conditional_relation {
                self.boolean_relation_safety(file, body, view, &all)
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported(
                            "Boolean conditional inspection is unavailable".into(),
                        )
                    })
            } else if node.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, node, view, "exists")
            {
                self.boolean_relation_safety(file, body, view, &all)
                    .unwrap_or_else(|| {
                        self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
                    })
            } else {
                self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
            };
            if let DefinitionSafety::Unsupported(reason) = body_safety {
                unsupported = Some(reason);
            } else if (optional_sum
                || decision_sets
                || decision_union
                || enum_sum
                || enum_collection
                || enum_array_collection
                || enum_values)
                && let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, body, true, true)
            {
                unsupported = Some(reason);
            }
            match unsupported {
                Some(reason) => Err(reason),
                None => Ok(()),
            }
        })();
        match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "parameter collection construction or membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        }
    }
    pub(in crate::callable_definitions) fn integer_comprehension_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let checked = (|| -> Result<(), String> {
            let facts = self.source.view(file, node, view);
            let range = self.source.context.files[file].location(node.range()).range;
            if node.kind() != NodeKind::ArrayComprehension
                || !facts.expressions.iter().any(|e| e.file == file && e.location.range == range
                    && e.ty.known() && !optional(&e.ty)
                    && matches!(&e.ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].kind == TypeKind::Int && indices[0].instantiation == Instantiation::Parameter
                            && element.known() && !optional(element) && element.kind == TypeKind::Int))
            { return Err("integer comprehension shape or optionality is unsupported".into()); }
            let children: Vec<_> = node.child_nodes().collect();
            let [body, list] = children.as_slice() else {
                return Err("integer comprehension body or header is unsupported".into());
            };
            let headers: Vec<_> = list.child_nodes().collect();
            if list.kind() != NodeKind::GeneratorList
                || headers.len() != 1
                || headers[0].child_nodes().count() != 1
                || !headers[0].children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
            {
                return Err("integer comprehension requires one unfiltered in binder".into());
            }
            let binders: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == headers[0].range()
                })
                .collect();
            if binders.len() != 1
                || !matches!(
                    &facts.declarations[binders[0].id.0].ty,
                    TypeInst {
                        kind: TypeKind::Int,
                        instantiation: Instantiation::Parameter,
                        ..
                    }
                )
                || !facts.declarations[binders[0].id.0].ty.known()
                || optional(&facts.declarations[binders[0].id.0].ty)
            {
                return Err("integer comprehension binder type is unsupported".into());
            }
            let source = unwrap(headers[0].child_nodes().next().unwrap());
            if self
                .source
                .expression_type(facts, file, source)
                .is_none_or(|e| {
                    !e.ty.known()
                        || optional(&e.ty)
                        || e.ty.instantiation != Instantiation::Parameter
                        || !matches!(&e.ty.kind, TypeKind::Set(element)
                        if element.known() && !optional(element) && element.kind == TypeKind::Int
                            && element.instantiation == Instantiation::Parameter)
                })
            {
                return Err("integer comprehension source is unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, source, view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            let mut nodes = vec![node];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, current) {
                    return Err("integer comprehension annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut all = generators.to_vec();
            all.push(headers[0]);
            match self.initialized_source_safety(file, body, view, &all, &mut Vec::new()) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            }
        })();
        match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "integer comprehension shape or membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        }
    }
    pub(in crate::callable_definitions) fn uncertain_comprehension_type(
        &self,
        file: FileId,
        local: &'a SyntaxNode,
        initializer: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let written = local
            .child_nodes()
            .next()
            .ok_or("local array type unavailable")?;
        let parts: Vec<_> = written.child_nodes().collect();
        if written.kind() != NodeKind::ArrayType || parts.len() != 2 {
            return Err("local comprehension requires one written integer axis".into());
        }
        let inferred_axis = parts[0].kind() == NodeKind::ScalarType
            && parts[0].child_nodes().next().is_none()
            && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, parts[0]).as_slice(),
                [token] if token.kind == TokenKind::Int);
        let axis = parts[0]
            .child_nodes()
            .next()
            .map(unwrap)
            .or_else(|| inferred_axis.then_some(parts[0]))
            .ok_or("local comprehension axis unavailable")?;
        let endpoints: Vec<_> = axis.child_nodes().collect();
        let source = initializer
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
            .and_then(|n| n.child_nodes().next())
            .and_then(|n| n.child_nodes().next())
            .map(unwrap)
            .ok_or("local comprehension source unavailable")?;
        let facts = self.source.view(file, initializer, view);
        if inferred_axis {
            // The initializer supplies this axis. Its extent remains unproved;
            // common written-type and construction checks still follow.
            let array = TypeInst {
                instantiation: Instantiation::Decision,
                optional: false,
                kind: TypeKind::Array {
                    indices: vec![TypeInst::par(TypeKind::Int)],
                    element: Box::new(TypeInst {
                        instantiation: Instantiation::Decision,
                        optional: false,
                        kind: TypeKind::Bool,
                    }),
                },
            };
            if initializer.kind() != NodeKind::ArrayComprehension
                || self
                    .source
                    .expression_type(facts, file, initializer)
                    .is_none_or(|e| e.ty != array)
                || self
                    .source
                    .bindings
                    .declarations
                    .iter()
                    .find(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Local
                            && d.syntax_range == local.range()
                    })
                    .is_none_or(|d| facts.declarations[d.id.0].ty != array)
            {
                return Err("local inferred-axis comprehension type is unsupported".into());
            }
        } else if let Some(source_id) = crate::domains::parameter_integer_set_source(
            self.source.context,
            self.source.bindings,
            facts,
            file,
            source,
        ) {
            if axis.kind() != NodeKind::RangeExpression
                || !self.source.core(file, axis, view, "..")
                || endpoints.len() != 2
                || crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    endpoints[0],
                ) != Ok(Some(1))
                || crate::domains::parameter_set_cardinality(
                    self.source.context,
                    self.source.bindings,
                    facts,
                    file,
                    unwrap(endpoints[1]),
                ) != Some(source_id)
            {
                return Err("local comprehension axis lacks the same-source cardinality".into());
            }
            self.dependencies(file, endpoints[0], view, generators)?;
        } else {
            // A single unfiltered 1..N source can use its own written axis.
            // Inspect its sources without evaluating N or publishing an extent.
            let bare = |node: &SyntaxNode| {
                node.kind() == NodeKind::Expression
                    && node.child_nodes().next().is_none()
                    && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            };
            let source_id = self
                .source
                .reference(file, source)
                .filter(|id| {
                    bare(source) && bare(axis) && self.source.reference(file, axis) == Some(*id)
                })
                .ok_or("local comprehension requires a bare parameter integer set")?;
            let owner = &self.source.bindings.declarations[source_id.0];
            let integer = TypeInst::par(TypeKind::Int);
            let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
            let headers: Vec<_> = initializer
                .child_nodes()
                .find(|node| node.kind() == NodeKind::GeneratorList)
                .into_iter()
                .flat_map(SyntaxNode::child_nodes)
                .collect();
            if owner.file != file
                || !owner.top_level
                || owner.role != DeclarationRole::Value
                || owner.instantiation != Instantiation::Parameter
                || facts.declarations[source_id.0].ty != set
                || self
                    .source
                    .expression_type(facts, file, source)
                    .is_none_or(|e| e.ty != set)
                || self
                    .source
                    .expression_type(facts, file, axis)
                    .is_none_or(|e| e.ty != set)
                || headers.len() != 1
                || headers[0].child_nodes().count() != 1
                || crate::domains::generator_slots(
                    &self.source.context.files[file].parsed,
                    headers[0],
                ) != 1
            {
                return Err(
                    "local comprehension same-axis source or traversal is unsupported".into(),
                );
            }
            let declaration = find_node(
                self.source.context.files[file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("local comprehension source declaration unavailable")?;
            let source_type = declaration
                .child_nodes()
                .next()
                .ok_or("local comprehension written source type unavailable")?;
            let members: Vec<_> = source_type.child_nodes().collect();
            let values: Vec<_> = declaration
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            if source_type.kind() != NodeKind::SetType
                || members.len() != 1
                || members[0].kind() != NodeKind::ScalarType
                || members[0].child_nodes().next().is_some()
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, members[0]).as_slice(),
                    [token] if token.kind == TokenKind::Int)
                || values.len() != 1
                || !crate::definitions::annotations_safe(self.source.context, file, declaration)
                || !crate::definitions::annotations_safe(self.source.context, file, source_type)
            {
                return Err("local comprehension written integer-set source is unsupported".into());
            }
            let range = unwrap(values[0]);
            let bounds: Vec<_> = range.child_nodes().collect();
            let range_view = self.source.view(file, range, view);
            if range.kind() != NodeKind::RangeExpression
                || bounds.len() != 2
                || bounds.iter().any(|bound| {
                    self.source
                        .expression_type(range_view, file, bound)
                        .is_none_or(|e| e.ty != integer)
                })
                || !self.source.prefix_primitive(
                    file,
                    range,
                    range_view,
                    "..",
                    &[integer.clone(), integer],
                    &set,
                )
            {
                return Err(
                    "local comprehension requires a written parameter integer range".into(),
                );
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, source, true, true)
            {
                return Err(reason);
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, source, view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            if crate::domains::invariant_expression_integer(
                self.source.context,
                self.source.bindings,
                file,
                bounds[0],
            ) != Ok(Some(1))
            {
                return Err("local comprehension same-axis range does not start at one".into());
            }
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return Err("local comprehension type annotation is unsupported".into());
            }
            if node.kind() == NodeKind::DomainType && node.range() != parts[0].range() {
                for expression in node.child_nodes() {
                    self.dependencies(file, expression, view, generators)?;
                }
            } else {
                nodes.extend(node.child_nodes());
            }
        }
        let range = self.source.context.files[file]
            .location(local.range())
            .range;
        if self.source.bindings.references.iter().any(|r| {
            r.file == file
                && range.start <= r.location.range.start
                && r.location.range.end <= range.end
                && matches!(r.resolution, BindingResolution::Resolved(id)
                if self.source.bindings.declarations[id.0].role == DeclarationRole::Local
                    && self.source.bindings.declarations[id.0].syntax_range.start >= local.range().start)
        }) {
            return Err("local comprehension has a cyclic or forward local dependency".into());
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn aggregate_source_safety(
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
        let sum = self.source.core(file, node, view, "sum");
        let card = self.source.core(file, node, view, "card");
        let length = self.source.core(file, node, view, "length");
        if !(sum || card || length) {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(facts, file, value)
                .map(|e| &e.ty)
        };
        let decision_enum_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Decision
                && matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element)
                    && element.instantiation == Instantiation::Parameter
                    && matches!(element.kind, TypeKind::Enum(_)))
        };
        let collection = |t: &TypeInst, actual: bool| {
            t.known()
                && !optional(t)
                && (!card
                    || t.instantiation == Instantiation::Parameter
                    || decision_integer_set(t)
                    || decision_enum_set(t))
                && if card {
                    matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element)
                        && element.instantiation == Instantiation::Parameter
                        && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)))
                } else if !sum {
                    // Inspect the original collection; matching views prove no size.
                    matches!(&t.kind, TypeKind::Array { indices, element }
                        if (indices.len() == 1 || actual && t.instantiation == Instantiation::Parameter
                                && indices.len() == 3 && **element == TypeInst::par(TypeKind::Int)
                                && indices.iter().all(|axis| *axis == TypeInst::par(TypeKind::Int)))
                            && indices.iter().all(|axis| axis.known() && !optional(axis)
                                && axis.instantiation == Instantiation::Parameter
                                && matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_)))
                            && element.known() && !optional(element))
                } else {
                    matches!(&t.kind, TypeKind::Array { indices, element }
                    if (indices.len() == 1 || actual && sum && indices.len() == 2)
                        && indices.iter().all(|index| index.known() && !optional(index)
                            && index.kind == TypeKind::Int && index.instantiation == Instantiation::Parameter)
                        && element.known() && !optional(element)
                        && (element.kind == TypeKind::Int
                            || actual && indices.len() == 1 && element.kind == TypeKind::Bool))
                }
        };
        let integer_result = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Int
                && (sum || card || t.instantiation == Instantiation::Parameter)
        };
        let children: Vec<_> = node.child_nodes().collect();
        // set2array is only a matching view. Inspect the written set and its
        // evaluation prerequisites without granting array extent or sum facts.
        let set_element = children
            .first()
            .and_then(|argument| typed(argument))
            .and_then(|t| {
                if t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && let TypeKind::Set(element) = &t.kind
                    && element.instantiation == Instantiation::Parameter
                    && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                {
                    Some(element.as_ref())
                } else {
                    None
                }
            });
        let set_argument = !card && set_element.is_some_and(|element| {
            typed(node).is_some_and(|t| integer_result(t) && t.instantiation == Instantiation::Parameter)
                && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1
                            && integer_result(return_type)
                            && return_type.instantiation == Instantiation::Parameter
                            && parameters[0].known() && !optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Parameter
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                    && indices[0].kind == TypeKind::Int
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && formal.known() && !optional(formal)
                                    && formal.instantiation == Instantiation::Parameter
                                    && formal.kind == element.kind))
                })
        });
        if children.len() != 1
            || children[0].kind() == NodeKind::NamedArgument
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || typed(node).is_none_or(|t| !integer_result(t))
            || typed(children[0]).is_none_or(|t| !collection(t, true) && !set_argument)
            || self.source.operation_fact(facts, file, node).is_none_or(|c| {
                !matches!(&c.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && collection(&parameters[0], false)
                        && integer_result(return_type) && Some(return_type) == typed(node)
                        && (!card || parameters[0] == *typed(children[0]).unwrap()
                            && return_type.instantiation == parameters[0].instantiation)
                        && (!sum || set_argument || typed(children[0]).is_some_and(|actual|
                            crate::types::coerces(actual, &parameters[0])
                                // Preserve the existing core sum rank2 Int flattening view.
                                || matches!((&actual.kind, &parameters[0].kind),
                                    (TypeKind::Array { indices, element },
                                     TypeKind::Array { indices: formal_indices, element: formal })
                                        if indices.len() == 2 && formal_indices.len() == 1
                                            && element.kind == TypeKind::Int && formal.kind == TypeKind::Int
                                            && crate::types::coerces(element, formal)))))
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "aggregate selected signature, collection or annotation is unsupported".into(),
            ));
        }
        let rank_three_length = length
            && typed(children[0]).is_some_and(
                |t| matches!(&t.kind, TypeKind::Array { indices, .. } if indices.len() == 3),
            );
        if rank_three_length
            && length_argument(self.source.context, self.source.bindings, facts, file, node)
                .is_none()
        {
            return Some(DefinitionSafety::Unsupported(
                "multidimensional length selected written primitive is unsupported".into(),
            ));
        }
        if card && typed(children[0]).is_some_and(decision_enum_set) {
            let Some(CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            }) = self
                .source
                .operation_fact(facts, file, node)
                .map(|call| &call.outcome)
            else {
                return Some(DefinitionSafety::Unsupported(
                    "decision enum card selected tuple is unavailable".into(),
                ));
            };
            if let Err(reason) =
                self.decision_enum_card_body_safety(*declaration, parameters, return_type)
            {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
        let safety =
            self.initialized_source_safety(file, children[0], view, generators, &mut Vec::new());
        if rank_three_length && !matches!(safety, DefinitionSafety::Unsupported(_)) {
            return Some(DefinitionSafety::Unknown(
                "multidimensional array length is unproved".into(),
            ));
        }
        if card
            && typed(children[0]).is_some_and(|t| decision_integer_set(t) || decision_enum_set(t))
            && !matches!(safety, DefinitionSafety::Unsupported(_))
        {
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(file, children[0], true, true)
            {
                return Some(DefinitionSafety::Unsupported(reason));
            }
            return Some(DefinitionSafety::Unknown(
                "decision set cardinality is unproved".into(),
            ));
        }
        let boolean_sum = sum && typed(children[0]).is_some_and(|actual|
            matches!(&actual.kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Bool));
        Some(
            if boolean_sum && !matches!(safety, DefinitionSafety::Unsupported(_)) {
                DefinitionSafety::Unknown("coerced Boolean sum value is unproved".into())
            } else if set_argument && safety == DefinitionSafety::Supported {
                DefinitionSafety::Unknown(
                    "parameter set aggregate value or cardinality is unproved".into(),
                )
            } else {
                safety
            },
        )
    }
    pub(in crate::callable_definitions) fn decision_prefix_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::RangeExpression {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [lower, upper] = children.as_slice() else {
            return None;
        };
        let one = |node: &SyntaxNode| {
            let node = unwrap(node);
            matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                [token] if node.kind() == NodeKind::Expression
                    && token.kind == TokenKind::IntegerLiteral
                    && &self.source.context.files[file].parsed.source()[token.range.clone()] == "1")
        };
        if !one(lower) {
            return None;
        }
        let upper = unwrap(upper);
        let mut selected = upper;
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let result = TypeInst::par(TypeKind::Set(Box::new(integer.clone())))
            .with_inst(Instantiation::Decision);
        if upper.kind() == NodeKind::BinaryExpression {
            let parts: Vec<_> = upper.child_nodes().collect();
            let [source, offset] = parts.as_slice() else {
                return None;
            };
            if !one(offset) {
                return None;
            }
            selected = unwrap(source);
        }
        if selected.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let parts: Vec<_> = selected.child_nodes().collect();
        let [subject, selector] = parts.as_slice() else {
            return None;
        };
        let bare = |source: &SyntaxNode| {
            source.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, source).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        };
        let (subject, selector) = (unwrap(subject), unwrap(selector));
        if !bare(subject)
            || !bare(selector)
            || self
                .source
                .expression_type(self.source.calls, file, selected)
                .is_none_or(|expression| expression.ty != decision)
        {
            return None;
        }
        Some(
            (|| -> Result<DefinitionSafety, String> {
                let error =
                    || "decision prefix source identity, type or scope is unsupported".to_owned();
                let array = self.source.reference(file, subject).ok_or_else(error)?;
                let binder = self.source.reference(file, selector).ok_or_else(error)?;
                let owner = &self.source.bindings.declarations[array.0];
                let binding = &self.source.bindings.declarations[binder.0];
                let declared = self.source.calls.declarations.get(array.0).ok_or_else(error)?;
                let header = generators
                    .iter()
                    .find(|g| g.range() == binding.syntax_range)
                    .ok_or_else(error)?;
                let typed = |source| {
                    self.source.expression_type(self.source.calls, file, source)
                        .map(|e| &e.ty)
                };
                if owner.role != DeclarationRole::Value
                    || !owner.top_level
                    || owner.instantiation != Instantiation::Decision
                    || declared.declaration != array
                    || !declared.ty.known()
                    || optional(&declared.ty)
                    || declared.ty.instantiation != Instantiation::Decision
                    || !matches!(&declared.ty.kind, TypeKind::Array { indices, element }
                    if indices.as_slice() == [integer.clone()] && element.as_ref() == &decision)
                    || typed(subject) != Some(&declared.ty)
                    || typed(selector) != Some(&integer)
                    || typed(selected) != Some(&decision)
                    || typed(lower) != Some(&integer)
                    || typed(upper) != Some(&decision)
                    || typed(node) != Some(&result)
                    || binding.file != file
                    || binding.role != DeclarationRole::Generator
                    || self.source
                        .calls
                        .declarations
                        .get(binder.0)
                        .is_none_or(|d| d.declaration != binder || d.ty != integer)
                    || !header.children().iter().any(|child| {
                        matches!(child,
                    SyntaxElement::Token(index)
                        if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In)
                    })
                    || header.child_nodes().next().and_then(typed)
                        != Some(&result.clone().with_inst(Instantiation::Parameter))
                    || generators.iter().any(|g| {
                        g.kind() != NodeKind::Generator || g.range().end > node.range().start
                    })
                    || generators
                        .windows(2)
                        .any(|pair| pair[0].range().start >= pair[1].range().start)
                    || !crate::definitions::annotations_safe(self.source.context, file, written)
                {
                    return Err(error());
                }
                if upper.kind() == NodeKind::BinaryExpression
                    && !self.source.prefix_primitive(
                        file,
                        upper,
                        self.source.calls,
                        "-",
                        &[decision.clone(), decision.clone()],
                        &decision,
                    )
                {
                    return Err(
                        "decision prefix predecessor signature or source is unsupported".into(),
                    );
                }
                let call = self.source
                    .operation_fact(self.source.calls, file, node)
                    .ok_or_else(error)?;
                let CallOutcome::Resolved {
                    declaration,
                    parameters,
                    return_type,
                } = &call.outcome
                else {
                    return Err(error());
                };
                if !crate::definitions::core_callable(
                    self.source.context,
                    self.source.bindings,
                    *declaration,
                    "..",
                ) || parameters.as_slice() != [decision.clone(), decision.clone()]
                    || return_type != &result
                    || call.generator_argument.is_some()
                    || !crate::types::coerces(&integer, &parameters[0])
                {
                    return Err(error());
                }
                self.source.decision_range_body_safety(*declaration)?;
                for (position, generator) in generators.iter().enumerate() {
                    for value in generator.child_nodes() {
                        let (value, lexical) = if value.kind() == NodeKind::WhereFilter {
                            (
                                value
                                    .child_nodes()
                                    .next()
                                    .ok_or("generator filter is unavailable")?,
                                &generators[..=position],
                            )
                        } else {
                            (value, &generators[..position])
                        };
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(
                                file,
                                value,
                                self.source.calls,
                                lexical,
                                &mut Vec::new(),
                            )
                        {
                            return Err(reason);
                        }
                        if let Some(reason) =
                            self.source.closed_integer_source_error(file, value, true, true)
                        {
                            return Err(reason);
                        }
                    }
                }
                for endpoint in &children {
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        endpoint,
                        self.source.calls,
                        generators,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) =
                        self.source.closed_integer_source_error(file, endpoint, true, true)
                    {
                        return Err(reason);
                    }
                }
                Ok(DefinitionSafety::Unknown(
                    "decision prefix members and extent are unproved".into(),
                ))
            })()
            .unwrap_or_else(DefinitionSafety::Unsupported),
        )
    }
    pub(in crate::callable_definitions) fn decision_set_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let result = typed(node).filter(|t| decision_integer_set(t))?;
        if node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(file, node, view, "array_union")
        {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Some(DefinitionSafety::Unsupported(
                    "decision set source annotation is unsupported".into(),
                ));
            }
            return Some(
                match self.decision_set_union_value_safety(file, node, view, generators, active) {
                    Ok(safety) => safety,
                    Err(boundary) => DefinitionSafety::Unsupported(boundary.reason),
                },
            );
        }
        let children: Vec<_> = node.child_nodes().collect();
        let checked = (|| -> Result<(), String> {
            if !crate::definitions::annotations_safe(self.source.context, file, written) {
                return Err("decision set source annotation is unsupported".into());
            }
            if node.kind() == NodeKind::BinaryExpression
                && self.source.core(file, node, view, "intersect")
            {
                if children.len() != 2
                    || children.iter().any(|child| typed(child) != Some(result))
                    || self.source.operation_fact(self.source.view(file, node, view), file, node).is_none_or(|call| {
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(|t| t == result)
                                && return_type == result)
                    })
                {
                    return Err("decision set intersection signature is unsupported".into());
                }
                for child in &children {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, child, view, generators, active)
                    {
                        return Err(reason);
                    }
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, child, true, true)
                    {
                        return Err(reason);
                    }
                }
                return Ok(());
            }
            if node.kind() != NodeKind::ArrayAccessExpression || children.len() != 3 {
                return Err("decision set source form is unsupported".into());
            }
            let subject = unwrap(children[0]);
            if subject.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("decision set source identity is unsupported".into());
            }
            let id = self
                .source
                .reference(file, subject)
                .ok_or("decision set source identity unavailable")?;
            let declaration = &self.source.bindings.declarations[id.0];
            let source = typed(subject).ok_or("decision set source type unavailable")?;
            if declaration.role != DeclarationRole::Value
                || !declaration.top_level
                || !source.known()
                || optional(source)
                || source != &self.source.view(file, subject, view).declarations[id.0].ty
            {
                return Err("decision set source scope, type or optionality is unsupported".into());
            }
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            if source.instantiation != Instantiation::Decision
                || !matches!(&source.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && element.as_ref() == result)
                || children[1..]
                    .iter()
                    .any(|selector| typed(selector).is_none_or(|t| !integer(t)))
            {
                return Err("decision set selection shape or selectors are unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            let declared = find_node(
                self.source.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("decision set source declaration unavailable")?;
            // The closed checker follows parameter aliases, but not decision arrays.
            // Start at the actual written array declaration to retain errors.
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(declaration.file, declared, true, true)
            {
                return Err(reason);
            }
            let domain =
                crate::domains::bare_index_domain(&self.source.domains.declarations[id.0].domain);
            let Domain::Array { indices, element } = domain else {
                return Err("decision set array source domain is unsupported".into());
            };
            if indices.len() != 2 {
                return Err("decision set array source rank is unsupported".into());
            }
            for (axis, selector) in indices.iter().zip(&children[1..]) {
                axis.numeric_minimum().map_err(str::to_owned)?;
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, selector, true, true)
                {
                    return Err(reason);
                }
                if crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    selector,
                )
                .is_ok_and(|value| {
                    value.is_some_and(|value| {
                        crate::domains::index_domain_member(axis, value) == Some(false)
                            || matches!(crate::domains::bare_index_domain(axis), Domain::Range { lower, .. }
                                if crate::domains::invariant_integer(lower).is_ok_and(|bound|
                                    bound.is_some_and(|bound| value < bound)))
                    })
                }) {
                    return Err("decision set selection is outside its declared index set".into());
                }
            }
            let member = crate::domains::bare_index_domain(element);
            let Domain::Set(member) = member else {
                return Err("decision set written element domain is unsupported".into());
            };
            member.numeric_minimum().map_err(str::to_owned)?;
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("decision set values and membership are unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect a written full-axis slice without establishing its scalar
    // selectors' membership, total evaluation or output extent.
    pub(in crate::callable_definitions) fn full_axis_slice_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let full_axis = |selector: &SyntaxNode| {
            selector.kind() == NodeKind::RangeExpression
                && selector.child_nodes().next().is_none()
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, selector).as_slice(),
                    [token] if token.kind == TokenKind::RangeInclusive)
        };
        if !children.iter().skip(1).any(|selector| full_axis(selector)) {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let typed = |value: &SyntaxNode| {
                self.source
                    .expression_type(self.source.view(file, value, view), file, value)
                    .map(|e| &e.ty)
            };
            let subject = unwrap(children.first().ok_or("slice subject unavailable")?);
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || subject.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("slice subject identity or annotation is unsupported".into());
            }
            let id = self
                .source
                .reference(file, subject)
                .ok_or("slice declaration identity unavailable")?;
            let declaration = &self.source.bindings.declarations[id.0];
            // Initial required cases use original model arrays. Local/default
            // array evaluation needs its own owning-scope inspection.
            if declaration.role != DeclarationRole::Value || !declaration.top_level {
                return Err("slice source declaration scope is unsupported".into());
            }
            let source = typed(subject).ok_or("slice source type unavailable")?;
            let result = typed(node).ok_or("slice result type unavailable")?;
            if !source.known()
                || optional(source)
                || !result.known()
                || optional(result)
                || source != &self.source.view(file, subject, view).declarations[id.0].ty
                || source.instantiation != result.instantiation
            {
                return Err("slice source or result qualifiers are unsupported".into());
            }
            let (
                TypeKind::Array { indices, element },
                TypeKind::Array {
                    indices: retained,
                    element: result_element,
                },
            ) = (&source.kind, &result.kind)
            else {
                return Err("slice source or result is not an array".into());
            };
            if indices.len() + 1 != children.len()
                || element != result_element
                || !matches!(
                    element.kind,
                    TypeKind::Int | TypeKind::Bool | TypeKind::Enum(_)
                )
                || indices.iter().any(|index| {
                    !index.known()
                        || optional(index)
                        || index.instantiation != Instantiation::Parameter
                        || !matches!(index.kind, TypeKind::Int | TypeKind::Enum(_))
                })
            {
                return Err("slice rank, element or axis identity is unsupported".into());
            }
            let mut count = 0;
            let mut scalars = Vec::new();
            for (selector, axis) in children[1..].iter().zip(indices) {
                let actual = typed(selector).ok_or("slice selector type unavailable")?;
                if full_axis(selector) {
                    if !actual.known()
                        || optional(actual)
                        || actual.instantiation != Instantiation::Parameter
                        || !matches!(&actual.kind, TypeKind::Set(element) if element.as_ref() == axis)
                        || retained.get(count) != Some(axis)
                    {
                        return Err(
                            "full slice marker or retained axis identity is unsupported".into()
                        );
                    }
                    count += 1;
                } else {
                    if !actual.known()
                        || optional(actual)
                        || actual.instantiation != Instantiation::Parameter
                        || actual.kind != axis.kind
                    {
                        return Err("slice scalar selector type or identity is unsupported".into());
                    }
                    scalars.push(*selector);
                }
            }
            if count != retained.len() {
                return Err("slice retained rank is unsupported".into());
            }
            let mut unsupported = None;
            // Check every source and scalar selector even when an earlier one
            // has uncertain membership. Unsupported evaluation must dominate.
            for value in std::iter::once(children[0]).chain(scalars) {
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, value, view, generators, &mut Vec::new())
                {
                    unsupported = Some(reason);
                }
            }
            match unsupported {
                Some(reason) => Err(reason),
                None => Ok(()),
            }
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "full-axis slice evaluation or scalar membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect integer generator sources and filters in lexical order without
    // establishing membership, extent or output dependencies.
    pub(in crate::callable_definitions) fn integer_source_headers_safety(
        &self,
        file: FileId,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let integer = TypeInst::par(TypeKind::Int);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        for (position, header) in generators.iter().enumerate() {
            let mut nodes = vec![*header];
            while let Some(node) = nodes.pop() {
                if node.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, node)
                {
                    return Err("native integer source header annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            let parts: Vec<_> = header.child_nodes().collect();
            let Some(source) = parts.first() else {
                return Err("native integer generator source is unavailable".into());
            };
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
            if header.kind() != NodeKind::Generator || binders.is_empty()
                || binders.len() != crate::domains::generator_slots(&self.source.context.files[file].parsed, header)
                || !header.children().iter().any(|part| matches!(part,
                    SyntaxElement::Token(index) if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In))
                || binders.iter().any(|binder| self.source.view(file, header, view).declarations[binder.id.0].ty != integer)
                || self.source.expression_type(self.source.view(file, source, view), file, source).is_none_or(|e| e.ty != set)
            {
                return Err("native integer generator identity or type is unsupported".into());
            }
            for (index, part) in parts.iter().enumerate() {
                let (value, scope) = if index == 0 {
                    (*part, &generators[..position])
                } else {
                    let conditions: Vec<_> = part.child_nodes().collect();
                    let [condition] = conditions.as_slice() else {
                        return Err("native integer generator filter is unsupported".into());
                    };
                    if part.kind() != NodeKind::WhereFilter
                        || self
                            .source
                            .expression_type(
                                self.source.view(file, condition, view),
                                file,
                                condition,
                            )
                            .is_none_or(|e| e.ty != TypeInst::par(TypeKind::Bool))
                    {
                        return Err("native integer generator filter type is unsupported".into());
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
        Ok(())
    }
    pub(in crate::callable_definitions) fn native_integer_flat_backing_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let integer = TypeInst::par(TypeKind::Int);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let (operation, rank) = if self.source.core(file, node, facts, "array3d") {
            ("array3d", 3)
        } else if self.source.core(file, node, facts, "array4d") {
            ("array4d", 4)
        } else {
            return None;
        };
        let instantiation = self
            .source
            .expression_type(facts, file, node)?
            .ty
            .instantiation;
        if instantiation != Instantiation::Parameter
            && !(rank == 3 && instantiation == Instantiation::Decision)
        {
            return None;
        }
        let element = integer.clone().with_inst(instantiation);
        let flat = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(element.clone()),
        })
        .with_inst(instantiation);
        let matrix = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone(); rank],
            element: Box::new(element),
        })
        .with_inst(instantiation);
        if self
            .source
            .expression_type(facts, file, node)
            .is_none_or(|e| e.ty != matrix)
        {
            return None;
        }
        // The backing source is a complete top-level initializer. It therefore
        // has no erased lexical generator or local-formal scope.
        let call = self.source.operation_fact(facts, file, node)?;
        let owner = self.source.bindings.declarations.iter().find(|d| {
            d.file == file
                && d.item == call.item
                && d.top_level
                && d.role == DeclarationRole::Value
                && self.source.calls.declarations[d.id.0].ty == matrix
        })?;
        let declaration = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let initializers: Vec<_> = declaration
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .collect();
        if !matches!(initializers.as_slice(), [source] if unwrap(source).range() == node.range()) {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let mut nodes = vec![declaration];
            while let Some(source) = nodes.pop() {
                if source.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, source)
                {
                    return Err("native integer backing annotation or source is unsupported".into());
                }
                nodes.extend(source.child_nodes());
            }
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(file, declaration, true, true)
            {
                return Err(reason);
            }
            let arguments: Vec<_> = node.child_nodes().collect();
            if arguments.len() != rank + 1
                || arguments
                    .iter()
                    .any(|n| n.kind() == NodeKind::NamedArgument)
            {
                return Err("native integer backing arguments are unsupported".into());
            }
            let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
            let mut parameters = vec![set; rank];
            parameters.push(flat.clone());
            if arguments
                .iter()
                .zip(&parameters)
                .any(|(argument, parameter)| {
                    self.source
                        .expression_type(self.source.view(file, argument, view), file, argument)
                        .is_none_or(|e| &e.ty != parameter)
                })
            {
                return Err("native integer backing actual types are unsupported".into());
            }
            self.source.parameter_array_primitive(
                file,
                node,
                view,
                operation,
                &parameters,
                &matrix,
            )?;
            let source = unwrap(arguments[rank]);
            let data = (source.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, source).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                .then(|| self.source.reference(file, source)).flatten()
                .ok_or("native integer backing requires a bare initialized data source")?;
            let data_owner = &self.source.bindings.declarations[data.0];
            if !data_owner.top_level
                || data_owner.role != DeclarationRole::Value
                || self.source.calls.declarations[data.0].ty != flat
                || instantiation == Instantiation::Decision && data_owner.file != file
            {
                return Err("native integer backing data identity is unsupported".into());
            }
            let data_source = find_node(
                self.source.context.files[data_owner.file].parsed.tree(),
                &data_owner.syntax_range,
                data_owner.role,
            )
            .ok_or("native integer backing data declaration is unavailable")?;
            let data_initializers: Vec<_> = data_source
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            let [initializer] = data_initializers.as_slice() else {
                return Err("native integer backing data initializer is unavailable".into());
            };
            if instantiation == Instantiation::Decision
                && unwrap(initializer).kind() != NodeKind::ArrayLiteral
            {
                return Err(
                    "native decision integer backing requires a written array literal".into(),
                );
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &arguments, view, &[])
            {
                return Err(reason);
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "native integer reshape extent and values are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn native_integer_flat_cell_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let typed = |value: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        if node.kind() != NodeKind::ArrayAccessExpression || typed(node) != Some(&decision) {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let rank = match children.len() {
            4 => 3,
            5 => 4,
            _ => return None,
        };
        let matrix = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone(); rank],
            element: Box::new(integer.clone()),
        });
        let subject = unwrap(children[0]);
        let selectors = &children[1..];
        if subject.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || typed(subject) != Some(&matrix)
            || selectors
                .iter()
                .any(|selector| typed(selector).is_none_or(|t| t != &integer && t != &decision))
            || selectors
                .iter()
                .filter(|selector| typed(selector) == Some(&decision))
                .count()
                != 1
        {
            return None;
        }
        let array = self.source.reference(file, subject)?;
        let owner = &self.source.bindings.declarations[array.0];
        if !owner.top_level
            || owner.role != DeclarationRole::Value
            || self.source.calls.declarations[array.0].ty != matrix
        {
            return None;
        }
        // Rank-three arrays also have a legacy symbolic selector route.
        // This reader applies only to a complete native flat backing source.
        let rank_three_backing = if rank == 3 {
            let declared = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )?;
            let initializers: Vec<_> = declared
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            let [initializer] = initializers.as_slice() else {
                return None;
            };
            Some(self.native_integer_flat_backing_safety(
                owner.file,
                initializer,
                self.source.calls,
            )?)
        } else {
            None
        };
        let checked = (|| -> Result<(), String> {
            let mut nodes = vec![written];
            while let Some(source) = nodes.pop() {
                if source.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(file, source)
                {
                    return Err("native integer cell annotation or source is unsupported".into());
                }
                nodes.extend(source.child_nodes());
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, written, true, true)
            {
                return Err(reason);
            }
            self.integer_source_headers_safety(file, view, generators)?;
            let declared = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("native integer backing declaration is unavailable")?;
            let initializers: Vec<_> = declared
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            let [initializer] = initializers.as_slice() else {
                return Err("native integer cell requires one initialized backing source".into());
            };
            match rank_three_backing.or_else(|| {
                self.native_integer_flat_backing_safety(owner.file, initializer, self.source.calls)
            }) {
                Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                Some(_) => {}
                None => return Err("native integer cell backing constructor is unsupported".into()),
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            let (axes, element) = match crate::domains::bare_index_domain(
                &self.source.domains.declarations[array.0].domain,
            ) {
                Domain::Array { indices, element } if indices.len() == rank => (indices, element),
                Domain::Unsupported(reason) => return Err(reason.clone()),
                _ => return Err("native integer declared domain is unsupported".into()),
            };
            for domain in axes.iter().chain(std::iter::once(element.as_ref())) {
                domain.numeric_minimum().map_err(str::to_owned)?;
            }
            for (axis, selector) in axes.iter().zip(&children[1..]) {
                if crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    selector,
                )
                .is_ok_and(|value| {
                    value.is_some_and(|value| {
                        crate::domains::index_domain_member(axis, value) == Some(false)
                    })
                }) {
                    return Err("native integer selection is outside its declared index set".into());
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "native integer selection membership and values are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    pub(in crate::callable_definitions) fn inline_reshape_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let axes = if self.source.core(file, node, view, "array1d") {
            1
        } else if self.source.core(file, node, view, "array2d") {
            2
        } else if self.source.core(file, node, view, "array3d") {
            3
        } else {
            return None;
        };
        if children.len() != axes + 1 {
            return None;
        }
        let facts = self.source.view(file, node, view);
        let typed = |n: &SyntaxNode| self.source.expression_type(facts, file, n).map(|e| &e.ty);
        let parameter_int = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let parameter_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if parameter_int(element))
        };
        let array = |t: &TypeInst, rank: usize| {
            t.known()
                && !optional(t)
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == rank && indices.iter().all(parameter_int)
                    && element.known() && !optional(element)
                    && (matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                        && (matches!(axes, 2 | 3) || element.kind == TypeKind::Bool)
                        || axes == 1 && t.instantiation == Instantiation::Parameter
                            && parameter_set(element)))
        };
        let parameter_set_array = |n: &SyntaxNode| {
            typed(n).is_some_and(|t| {
                array(t, 1)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { element, .. } if parameter_set(element))
            })
        };
        let source = unwrap(children[axes]);
        if source.kind() != NodeKind::ArrayComprehension
            && !(axes == 1
                && parameter_set_array(children[axes])
                && parameter_set_array(node)
                && array_concatenation(
                    self.source.context,
                    self.source.bindings,
                    facts,
                    file,
                    source,
                )
                .is_some())
        {
            return None;
        }
        let valid = children.iter().all(|n| n.kind() != NodeKind::NamedArgument)
            && children[..axes]
                .iter()
                .all(|n| typed(n).is_some_and(parameter_set))
            && typed(children[axes]).is_some_and(|t| array(t, 1))
            && typed(node).is_some_and(|t| array(t, axes))
            && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == axes + 1 && parameters.iter().zip(&children)
                        .all(|(formal, actual)| typed(actual) == Some(formal))
                        && typed(node) == Some(return_type)
                        && (axes != 3 || !matches!(&return_type.kind,
                            TypeKind::Array { element, .. } if element.kind == TypeKind::Int)
                            || self.source.prefix_primitive(file, node, facts, "array3d", parameters, return_type)))
            });
        let mut nodes = vec![written];
        while let Some(current) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, current) {
                return Some(DefinitionSafety::Unsupported(
                    "inline reshape annotation is unsupported".into(),
                ));
            }
            nodes.extend(current.child_nodes());
        }
        if !valid {
            return Some(DefinitionSafety::Unsupported(
                "inline reshape selected signature or type is unsupported".into(),
            ));
        }
        // Set-array inspection checks its eager axis independently. The
        // source's existing initialized inspection preserves lazy branches.
        if axes == 1
            && typed(node).is_some_and(
                |t| matches!(&t.kind, TypeKind::Array { element, .. } if parameter_set(element)),
            )
            && let Some(reason) =
                self.source
                    .closed_integer_source_error(file, children[0], true, true)
        {
            return Some(DefinitionSafety::Unsupported(reason));
        }
        Some(
            match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("inline reshape cardinality is unproved".into()),
            },
        )
    }
    pub(in crate::callable_definitions) fn decision_array_bound_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !(self.source.core(file, node, view, "lb_array")
                || self.source.core(file, node, view, "ub_array"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let source = unwrap(argument);
        let facts = self.source.view(file, node, view);
        let actual = self
            .source
            .expression_type(facts, file, argument)
            .map(|e| &e.ty)?;
        if actual.instantiation != Instantiation::Decision {
            return None;
        }
        let parameter_int = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.kind == TypeKind::Int
                && ty.instantiation == Instantiation::Parameter
        };
        let result = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        let Some(array) = self.source.reference(file, source) else {
            return Some(DefinitionSafety::Unsupported(
                "array reflection requires a bare array identity".into(),
            ));
        };
        let declaration = &self.source.bindings.declarations[array.0];
        if argument.kind() == NodeKind::NamedArgument
            || source.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.source.context.files[file].parsed, source).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || declaration.role != DeclarationRole::Value || !declaration.top_level
            || !actual.known() || optional(actual)
            || !matches!(&actual.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && parameter_int(&indices[0])
                    && element.known() && !optional(element) && element.kind == TypeKind::Int
                    && element.instantiation == Instantiation::Decision)
            || result.is_none_or(|ty| !parameter_int(ty))
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || !crate::definitions::annotations_safe(self.source.context, file, node)
            || !self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0] == *actual && Some(return_type) == result)
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "array reflection selected source or signature is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        if let Some(written) = find_node(
            self.source.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) && let Some(value) = written
            .child_nodes()
            .find(|node| is_expression(node.kind()))
            .map(unwrap)
            && value.kind() == NodeKind::ArrayLiteral
            && value.child_nodes().next().is_none()
        {
            return Some(DefinitionSafety::Unsupported(
                "array reflection source is empty".into(),
            ));
        }
        let mut domain = &self.source.domains.declarations[array.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        if let Domain::Array { indices, .. } = domain {
            for axis in indices {
                match crate::domains::bare_index_domain(axis) {
                    Domain::Range { lower, upper } => {
                        match (
                            crate::domains::invariant_integer(lower),
                            crate::domains::invariant_integer(upper),
                        ) {
                            (Err(reason), _) | (_, Err(reason)) => {
                                return Some(DefinitionSafety::Unsupported(reason));
                            }
                            (Ok(Some(lower)), Ok(Some(upper))) if upper < lower => {
                                return Some(DefinitionSafety::Unsupported(
                                    "array reflection source is empty".into(),
                                ));
                            }
                            _ => {}
                        }
                    }
                    Domain::LiteralSet(values) if values.is_empty() => {
                        return Some(DefinitionSafety::Unsupported(
                            "array reflection source is empty".into(),
                        ));
                    }
                    Domain::Unsupported(reason) => {
                        return Some(DefinitionSafety::Unsupported(reason.clone()));
                    }
                    _ => {}
                }
            }
        }
        Some(DefinitionSafety::Unknown(
            "reflected array bound is unproved".into(),
        ))
    }
    pub(in crate::callable_definitions) fn integer_array_set_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !self.source.core(file, node, view, "array2set")
        {
            return None;
        }
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(integer.clone()),
        });
        let set = TypeInst::par(TypeKind::Set(Box::new(integer)));
        if self
            .source
            .operation_fact(self.source.view(file, node, view), file, node)
            .is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                if parameters.as_slice() == [array.clone()] && *return_type == set)
            })
        {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let children: Vec<_> = node.child_nodes().collect();
            let [argument] = children.as_slice() else {
                return Err("array2set argument correspondence unsupported".into());
            };
            if self
                .source
                .expression_type(self.source.view(file, node, view), file, node)
                .is_none_or(|e| e.ty != set)
                || self
                    .source
                    .expression_type(self.source.view(file, argument, view), file, argument)
                    .map(|e| &e.ty)
                    .or_else(|| {
                        self.source
                            .reference(file, unwrap(argument))
                            .map(|id| &self.source.view(file, argument, view).declarations[id.0].ty)
                    })
                    .is_none_or(|ty| *ty != array)
            {
                return Err(
                    "array2set requires selected present parameter integer array and set".into(),
                );
            }
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, node) {
                    return Err("array2set source annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, argument, view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "array2set membership and cardinality are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // These two rank-two metadata views may inspect a selected owning formal,
    // but its default, written type and annotations still evaluate independently.
    pub(in crate::callable_definitions) fn rank_two_formal_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let node = unwrap(node);
        let id = self
            .source
            .reference(file, node)
            .ok_or("rank-two formal source identity is unavailable")?;
        let declaration = &self.source.bindings.declarations[id.0];
        let range = self.source.context.files[file].location(node.range()).range;
        let callable = self
            .source
            .bindings
            .references
            .iter()
            .find(|reference| {
                reference.file == file
                    && reference.kind == ReferenceKind::Value
                    && range.start <= reference.location.range.start
                    && reference.location.range.end <= range.end
                    && reference.resolution == BindingResolution::Resolved(id)
            })
            .and_then(|reference| reference.callable)
            .ok_or("rank-two formal source owner is unavailable")?;
        let signature = self
            .source
            .calls
            .signatures
            .iter()
            .find(|signature| signature.declaration == callable)
            .ok_or("rank-two formal signature is unavailable")?;
        let position = (0..signature.parameters.len())
            .find(|position| {
                formal_parameter(
                    self.source.context,
                    self.source.bindings,
                    callable,
                    *position,
                ) == Some(id)
            })
            .ok_or("rank-two formal correspondence is unavailable")?;
        let facts = self.source.view(file, node, view);
        if declaration.file != file
            || declaration.role != DeclarationRole::Parameter
            || signature.parameters[position].has_default
            || self
                .source
                .expression_type(facts, file, node)
                .is_none_or(|expression| expression.ty != facts.declarations[id.0].ty)
        {
            return Err("rank-two formal type or default is unsupported".into());
        }
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("rank-two formal declaration is unavailable")?;
        if written
            .child_nodes()
            .any(|child| is_expression(child.kind()))
        {
            return Err("rank-two formal initializer is unsupported".into());
        }
        let mut pending = vec![written];
        while let Some(node) = pending.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return Err("rank-two formal annotation is unsupported".into());
            }
            if node.kind() == NodeKind::DomainType {
                for source in node.child_nodes() {
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        source,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, source, true, true)
                    {
                        return Err(reason);
                    }
                }
            }
            pending.extend(node.child_nodes());
        }
        self.type_dependencies(file, written, view, generators)?;
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, written, true, false)
        {
            return Err(reason);
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn parameter_integer_array_value_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        matching_set: bool,
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let name = if self.source.core(file, node, view, "sort") {
            "sort"
        } else if self.source.core(file, node, view, "reverse") {
            "reverse"
        } else {
            return None;
        };
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(integer),
        });
        let facts = self.source.view(file, node, view);
        let checked = (|| -> Result<(), String> {
            let fact = self
                .source
                .operation_fact(facts, file, node)
                .ok_or("parameter array value selection is unavailable")?;
            let CallOutcome::Resolved {
                declaration: id,
                parameters,
                return_type,
            } = &fact.outcome
            else {
                return Err("parameter array value selection is unsupported".into());
            };
            let arguments: Vec<_> = node
                .child_nodes()
                .filter(|child| child.kind() != NodeKind::Annotation)
                .collect();
            if node.kind() != NodeKind::CallExpression
                || fact.generator_argument.is_some()
                || parameters.as_slice() != std::slice::from_ref(&array)
                || return_type != &array
                || arguments.len() != 1
                || arguments[0].kind() == NodeKind::NamedArgument
                || self
                    .source
                    .expression_type(facts, file, node)
                    .is_none_or(|value| value.ty != array)
                || self
                    .source
                    .expression_type(
                        self.source.view(file, arguments[0], view),
                        file,
                        arguments[0],
                    )
                    .is_none_or(|value| {
                        value.ty != array
                            && !(matching_set
                                && name == "reverse"
                                && value.ty
                                    == TypeInst::par(TypeKind::Set(Box::new(TypeInst::par(
                                        TypeKind::Int,
                                    )))))
                    })
                || !crate::definitions::annotations_safe(self.source.context, file, written)
            {
                return Err(
                    "parameter array value tuple or source annotation is unsupported".into(),
                );
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &arguments, view, generators)
            {
                return Err(reason);
            }
            if name == "sort" {
                return self.source.parameter_array_primitive(
                    file,
                    node,
                    facts,
                    name,
                    parameters,
                    return_type,
                );
            }
            let owner = &self.source.bindings.declarations[id.0];
            if owner.role != DeclarationRole::Function {
                return Err("reverse array declaration is unsupported".into());
            }
            let declaration = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("reverse array written declaration is unavailable")?;
            let formal = formal_parameter(self.source.context, self.source.bindings, *id, 0)
                .ok_or("reverse array formal identity is unavailable")?;
            let formal_node = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &self.source.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("reverse array written formal is unavailable")?;
            let bodies: Vec<_> = declaration
                .child_nodes()
                .filter(|child| is_expression(child.kind()))
                .collect();
            let [body] = bodies.as_slice() else {
                return Err("reverse array selected body is unavailable".into());
            };
            if !crate::definitions::annotations_safe(self.source.context, owner.file, declaration)
                || formal_node
                    .child_nodes()
                    .any(|child| is_expression(child.kind()))
                || declaration
                    .child_nodes()
                    .find(|child| child.kind() == NodeKind::ParameterList)
                    .is_none_or(|list| list.child_nodes().count() != 1)
            {
                return Err("reverse array formal default or annotation is unsupported".into());
            }
            let body_view = instantiated_body(
                self.source.context,
                self.source.bindings,
                self.source.calls,
                *id,
                parameters,
            );
            if body_view.declarations[formal.0].ty != array {
                return Err("reverse array concrete formal type is unsupported".into());
            }
            for header in [
                declaration
                    .child_nodes()
                    .next()
                    .ok_or("reverse array return type is unavailable")?,
                formal_node,
            ] {
                let mut nodes = vec![header];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, owner.file, node)
                        || is_expression(node.kind())
                    {
                        return Err(
                            "reverse array written type or annotation is unsupported".into()
                        );
                    }
                    nodes.extend(node.child_nodes());
                }
            }
            self.type_dependencies(owner.file, declaration, &body_view, &[])?;
            self.type_dependencies(owner.file, formal_node, &body_view, &[])?;
            self.reverse_parameter_array_body(owner.file, body, formal, &body_view, &array)
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("parameter array value and extent are unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
}
