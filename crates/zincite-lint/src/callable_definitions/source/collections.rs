use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn array_conversion_argument(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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

    pub(in crate::callable_definitions) fn is_scalar_integer_extremum(
        &self,
        file: FileId,
        written: &SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
        inspect_index_set_axes: bool,
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
        if unsupported_bare_value(values) && inline.kind() != NodeKind::ArrayComprehension {
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
                        && return_type == result
                        && self.prefix_primitive(file, node, facts, "array2d", parameters, return_type))
            })
        {
            return None;
        }
        if arguments[..2].iter().any(|argument| {
            if !unsupported_bare_value(argument) {
                return false;
            }
            let axis = unwrap(argument);
            let axis_facts = self.view(file, axis, view);
            if inspect_index_set_axes
                && inline.kind() != NodeKind::ArrayComprehension
                && let Some(id) = self.integer_index_set(file, axis, axis_facts)
            {
                let owner = &self.bindings.declarations[id.0];
                let actual = &axis_facts.declarations[id.0].ty;
                if owner.top_level
                    && owner.role == DeclarationRole::Value
                    && actual.instantiation == Instantiation::Parameter
                    && matches!(&actual.kind, TypeKind::Array { indices, element }
                        if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                            && **element == TypeInst::par(TypeKind::Int))
                    && self.operation_fact(axis_facts, file, axis).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.as_slice() == std::slice::from_ref(actual)
                                && Some(return_type) == typed(argument)
                                && self.prefix_primitive(file, axis, axis_facts, "index_set", parameters, return_type))
                    })
                {
                    // This source admission supplies no stronger adapter view.
                    return false;
                }
            }
            inline.kind() == NodeKind::ArrayComprehension
                || axis.kind() != NodeKind::RangeExpression
                || !self.operation_fact(axis_facts, file, axis).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [TypeInst::par(TypeKind::Int), TypeInst::par(TypeKind::Int)]
                            && Some(return_type) == typed(argument)
                            && self.prefix_primitive(file, axis, axis_facts, "..", parameters, return_type))
                })
        }) {
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
        view: &crate::callables::CallableView<'_>,
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

    pub(in crate::callable_definitions) fn default_collection_arguments(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
    pub(in crate::callable_definitions) fn aggregate_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = &self
            .source
            .expression_type(self.source.view(file, node, view), file, node)?
            .ty;
        if !decision_integer_set(ty) {
            return None;
        }
        Some(
            match opaque_source_error(
                self.source.context,
                self.source.bindings,
                view,
                self.source.instantiations,
                self.source.domains,
                file,
                written,
            ) {
                Some(reason) => DefinitionSafety::Unsupported(reason),
                None => crate::value_safety::opaque_expression_safety(
                    self.source.context,
                    self.source.bindings,
                    view,
                    file,
                    written,
                ),
            },
        )
    }
    // Inspect a written full-axis slice without establishing its scalar
    // selectors' membership, total evaluation or output extent.
    pub(in crate::callable_definitions) fn full_axis_slice_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
                .closed_integer_source_error_with_branches(file, written, false, true, true)
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
        view: &crate::callables::CallableView<'_>,
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
    pub(in crate::callable_definitions) fn integer_array_bound_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
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
        if !matches!(
            actual.instantiation,
            Instantiation::Parameter | Instantiation::Decision
        ) {
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
                if !indices.is_empty() && indices.iter().all(parameter_int)
                    && element.known() && !optional(element) && element.kind == TypeKind::Int
                    && element.instantiation == actual.instantiation)
            || result.is_none_or(|ty| !parameter_int(ty))
            || !crate::definitions::annotations_safe(self.source.context, file, written)
            || !crate::definitions::annotations_safe(self.source.context, file, node)
            || !self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0] == actual.clone().with_inst(Instantiation::Decision)
                        && Some(return_type) == result
                        && self.source.prefix_primitive(file, node, facts, &call.name, parameters, return_type))
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
        view: &crate::callables::CallableView<'_>,
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
            let body_view = requested_body(
                self.source.context,
                self.source.bindings,
                self.source.calls,
                BodyProperty::Definedness,
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
            self.type_dependencies(owner.file, declaration, &body_view.view(), &[])?;
            self.type_dependencies(owner.file, formal_node, &body_view.view(), &[])?;
            self.reverse_parameter_array_body(owner.file, body, formal, &body_view.view(), &array)
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("parameter array value and extent are unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
}
