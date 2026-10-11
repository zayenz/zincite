//! Strict dependency proofs; checked Unknown sources remain refusals.
use super::*;
mod traversal;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn iterations(
        &self,
        file: FileId,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> Result<(), String> {
        for generator in generators {
            if generator.child_nodes().any(|n|n.kind()==NodeKind::WhereFilter) || !generator.children().iter().any(|c|matches!(c,SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind==TokenKind::In)){return Err("filtered or assigned iteration does not establish a whole output".into());}
            let source = generator
                .child_nodes()
                .next()
                .ok_or("iteration source is unavailable")?;
            let range = self.context.files[file].location(source.range()).range;
            let ty = self
                .view(file, source, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
                .ok_or("iteration source type is unavailable")?;
            if ty.instantiation != Instantiation::Parameter
                || optional(ty)
                || !matches!(ty.kind, TypeKind::Set(_))
            {
                return Err("iteration is not a supported finite parameter set".into());
            }
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn member_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        if index.kind() != NodeKind::Expression {
            return false;
        }
        let Some(binder) = self.reference(file, index) else {
            return false;
        };
        self.bindings.declarations[binder.0].role == DeclarationRole::Generator
            && generators.iter().any(|g| {
                self.bindings.declarations[binder.0].syntax_range == g.range()
                    && g.child_nodes().next().is_some_and(|source| {
                        self.core(file, source, view, "index_set")
                            && source.child_nodes().next().is_some_and(|subject| {
                                unwrap(subject).kind() == NodeKind::Expression
                                    && self.reference(file, unwrap(subject)) == Some(array)
                            })
                    })
            })
    }
    pub(in crate::callable_definitions) fn integer_index_set(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
        let set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if integer(element) && element.instantiation == Instantiation::Parameter)
        };
        let CallOutcome::Resolved {
            parameters,
            return_type,
            ..
        } = &self.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let [parameter] = parameters.as_slice() else {
            return None;
        };
        let subject = unwrap(node.child_nodes().next()?);
        let id = self.reference(file, subject)?;
        let actual = &facts.declarations[id.0].ty;
        (node.kind() == NodeKind::CallExpression
            && node.child_nodes().count() == 1
            && self.core(file, node, view, "index_set")
            && subject.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            && actual.known() && !optional(actual)
            && matches!(&actual.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && integer(element))
            && parameter.known() && !optional(parameter)
            && matches!(&parameter.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && integer(element))
            && crate::types::coerces(actual, parameter)
            && set(return_type)
            && self.expression_type(facts, file, node).is_some_and(|e| e.ty == *return_type))
        .then_some(id)
    }
    pub(in crate::callable_definitions) fn exact_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        generators.len() == 1
            && self.iterations(file, generators, view).is_ok()
            && self.member_index(file, array, index, generators, view)
    }
    pub(in crate::callable_definitions) fn named_selector_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        axis: usize,
        selector: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let selector = unwrap(selector);
        let range = self.context.files[file].location(selector.range()).range;
        let scalar = view
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range);
        if scalar.is_none_or(|e| {
            !e.ty.known()
                || optional(&e.ty)
                || e.ty.kind != TypeKind::Int
                || !matches!(
                    e.ty.instantiation,
                    Instantiation::Parameter | Instantiation::Decision
                )
        }) {
            return false;
        }
        let array_type = &view.declarations[array.0].ty;
        let TypeKind::Array {
            indices: types,
            element,
        } = &array_type.kind
        else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || !matches!(types.len(), 1 | 2)
            || !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
            || !types.iter().all(|index| {
                index.kind == TypeKind::Int
                    && index.instantiation == Instantiation::Parameter
                    && !optional(index)
            })
        {
            return false;
        }
        let mut outer = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain, .. } = outer {
            outer = domain;
        }
        let Domain::Array {
            indices,
            element: domain_element,
        } = outer
        else {
            return false;
        };
        if indices.len() != types.len()
            || indices
                .iter()
                .chain(std::iter::once(domain_element.as_ref()))
                .any(|domain| domain.numeric_minimum().is_err())
        {
            return false;
        }
        let Some(Domain::Named {
            declaration: expected,
            ..
        }) = indices.get(axis)
        else {
            return false;
        };
        let set_type = &view.declarations[expected.0].ty;
        if !set_type.known()
            || optional(set_type)
            || set_type.instantiation != Instantiation::Parameter
            || !matches!(&set_type.kind, TypeKind::Set(element)
                if element.kind == TypeKind::Int
                    && element.instantiation == Instantiation::Parameter)
        {
            return false;
        }
        let own = if selector.kind() == NodeKind::Expression {
            let Some(id) = self.reference(file, selector) else {
                return false;
            };
            &self.domains.declarations[id.0].domain
        } else if selector.kind() == NodeKind::ArrayAccessExpression {
            let parts: Vec<_> = selector.child_nodes().collect();
            let Some(subject) = parts.first().map(|node| unwrap(node)) else {
                return false;
            };
            let Some(id) = self
                .reference(file, subject)
                .filter(|_| subject.kind() == NodeKind::Expression)
            else {
                return false;
            };
            let array_type = &view.declarations[id.0].ty;
            if !array_type.known()
                || optional(array_type)
                || !matches!(&array_type.kind, TypeKind::Array { indices, element }
                    if matches!(indices.len(), 1 | 2) && parts.len() == indices.len() + 1
                        && element.kind == TypeKind::Int
                        && indices.iter().all(|index| index.kind == TypeKind::Int
                            && index.instantiation == Instantiation::Parameter))
            {
                return false;
            }
            let mut inner = &self.domains.declarations[id.0].domain;
            while let Domain::Named { domain, .. } = inner {
                inner = domain;
            }
            let Domain::Array { element, .. } = inner else {
                return false;
            };
            element.as_ref()
        } else {
            return false;
        };
        matches!(own, Domain::Named { declaration: actual, .. } if actual == expected)
    }
    pub(in crate::callable_definitions) fn presence_dependencies(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(node);
        let range = self.context.files[file].location(node.range()).range;
        let ty = self
            .view(file, node, view)
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty);
        if node.kind() != NodeKind::Expression
            || ty.is_none_or(|t| !t.known() || t.instantiation != Instantiation::Parameter)
        {
            return Err("presence guard requires a supported parameter reference".into());
        }
        // Presence is total with missing parameter data. Retain its identity,
        // without evaluating its optional value or substituting a default.
        Ok(self.reference(file, node).into_iter().collect())
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn relation_iterations(
        &self,
        file: FileId,
        generators: &[&'a SyntaxNode],
        first_new: usize,
        view: &CallableFacts,
        decision_arrays: bool,
    ) -> Result<(), String> {
        // Nested collections retain outer binders without re-evaluating their headers.
        for (position, generator) in generators.iter().enumerate().skip(first_new) {
            if !generator.children().iter().any(|c| matches!(c, SyntaxElement::Token(i) if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)) { return Err("assigned iteration source is unsupported".into()); }
            let source = generator
                .child_nodes()
                .next()
                .ok_or("iteration source unavailable")?;
            let range = self.source.context.files[file]
                .location(source.range())
                .range;
            let ty = self
                .source
                .view(file, source, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty);
            if ty.is_none_or(|t| {
                !t.known()
                    || optional(t)
                    || !(t.instantiation == Instantiation::Parameter
                        && matches!(t.kind, TypeKind::Set(_) | TypeKind::Array { .. })
                        || decision_arrays
                            && t.instantiation == Instantiation::Decision
                            && matches!(t.kind, TypeKind::Array { .. }))
            }) {
                return Err("iteration is not a supported finite parameter set or array".into());
            }
            self.dependencies(file, source, view, &generators[..position])?;
            for filter in generator
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::WhereFilter)
            {
                let condition = filter
                    .child_nodes()
                    .next()
                    .ok_or("iteration filter unavailable")?;
                let range = self.source.context.files[file]
                    .location(condition.range())
                    .range;
                let ty = self
                    .source
                    .view(file, condition, view)
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .map(|e| &e.ty);
                if ty.is_none_or(|t| {
                    t.kind != TypeKind::Bool
                        || t.optional
                        || t.instantiation != Instantiation::Parameter
                }) {
                    return Err("iteration filter is not a total parameter Boolean".into());
                }
                self.dependencies(file, condition, view, &generators[..=position])?;
            }
        }
        Ok(())
    }
    // This first contract belongs to the callable's entire direct body. A
    // conditional, local or nested call cannot carry it into an output summary.
    pub(in crate::callable_definitions) fn asserted_index_pair(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<[DeclarationId; 2]> {
        let owner = self.source.bindings.declarations.iter().find(|d| {
            d.file == file
                && matches!(
                    d.role,
                    DeclarationRole::Predicate | DeclarationRole::Function | DeclarationRole::Test
                )
                && d.syntax_range.start <= assertion.range().start
                && assertion.range().end <= d.syntax_range.end
        })?;
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let body = written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()?;
        if unwrap(body).range() != assertion.range() {
            return None;
        }
        let args = self.source.assertion_arguments(file, assertion, view)?;
        if args.len() != 3 || args.iter().any(|(source, _)| *source != file) {
            return None;
        }
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        let CallOutcome::Resolved {
            parameters,
            return_type,
            ..
        } = &self
            .source
            .operation_fact(self.source.view(file, assertion, view), file, assertion)?
            .outcome
        else {
            return None;
        };
        if parameters.len() != 3
            || !present(&parameters[0], TypeKind::Bool, Instantiation::Parameter)
            || !present(&parameters[1], TypeKind::String, Instantiation::Parameter)
            || !present(&parameters[2], TypeKind::Bool, Instantiation::Decision)
            || return_type != &parameters[2]
        {
            return None;
        }
        let condition = unwrap(args[0].1);
        let operands: Vec<_> = condition.child_nodes().collect();
        let set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if present(element, TypeKind::Int, Instantiation::Parameter))
        };
        if condition.kind() != NodeKind::BinaryExpression
            || operands.len() != 2
            || !self.source.core(file, condition, view, "=")
            || !self
                .source
                .operation_fact(self.source.view(file, condition, view), file, condition)
                .is_some_and(|call| {
                    matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(set)
                        && present(return_type, TypeKind::Bool, Instantiation::Parameter))
                })
            || self
                .source
                .expression_type(self.source.view(file, condition, view), file, condition)
                .is_none_or(|e| !present(&e.ty, TypeKind::Bool, Instantiation::Parameter))
        {
            return None;
        }
        let arrays = [
            self.source.integer_index_set(file, operands[0], view)?,
            self.source.integer_index_set(file, operands[1], view)?,
        ];
        if arrays.iter().any(|id| {
            let d = &self.source.bindings.declarations[id.0];
            d.file != file || d.item != owner.item || d.role != DeclarationRole::Parameter
        }) {
            return None;
        }
        let mut nodes = vec![assertion];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return None;
            }
            nodes.extend(node.child_nodes());
        }
        for (position, (_, value)) in args.iter().take(2).enumerate() {
            let kind = if position == 0 {
                TypeKind::Bool
            } else {
                TypeKind::String
            };
            if self
                .source
                .expression_type(self.source.view(file, value, view), file, value)
                .is_none_or(|e| !present(&e.ty, kind, Instantiation::Parameter))
                || self.dependencies(file, value, view, &[]).is_err()
            {
                return None;
            }
        }
        Some(arrays)
    }
    pub(in crate::callable_definitions) fn asserted_index_membership(
        &self,
        file: FileId,
        access: &'a SyntaxNode,
        array: DeclarationId,
        index: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let Some(binder) = self
            .source
            .reference(file, index)
            .filter(|_| index.kind() == NodeKind::Expression)
        else {
            return false;
        };
        let declaration = &self.source.bindings.declarations[binder.0];
        if declaration.file != file || declaration.role != DeclarationRole::Generator {
            return false;
        }
        let Some(source) = generators
            .iter()
            .find(|g| g.range() == declaration.syntax_range)
            .and_then(|g| g.child_nodes().next())
            .and_then(|source| self.source.integer_index_set(file, source, view))
        else {
            return false;
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= access.range().start && access.range().end <= node.range().end
        };
        let mut nodes = vec![self.source.context.files[file].parsed.tree()];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::CallExpression
                && let Some((_, pair)) = self.direct_asserted_forall(file, node, view)
                && (pair == [array, source] || pair == [source, array])
                && self
                    .source
                    .assertion_arguments(file, node, view)
                    .is_some_and(|args| contains(args[2].1))
            {
                // Normal return from this exact guard proves this selector's
                // membership only. Whole-array coverage keeps its direct check.
                return true;
            }
            nodes.extend(node.child_nodes().filter(|child| contains(child)));
        }
        false
    }
    pub(in crate::callable_definitions) fn shifted_index_membership(
        &self,
        file: FileId,
        access: &'a SyntaxNode,
        array: DeclarationId,
        index: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let operands: Vec<_> = index.child_nodes().map(unwrap).collect();
        let parameter_int = |node: &SyntaxNode| {
            self.source
                .view(file, node, view)
                .expressions
                .iter()
                .any(|e| {
                    e.file == file
                        && e.location.range
                            == self.source.context.files[file].location(node.range()).range
                        && e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.instantiation == Instantiation::Parameter
                        && e.ty.kind == TypeKind::Int
                })
        };
        if index.kind() != NodeKind::BinaryExpression
            || operands.len() != 2
            || !self.source.core(file, index, view, "-")
            || !parameter_int(index)
            || !operands
                .iter()
                .all(|node| node.kind() == NodeKind::Expression && parameter_int(node))
        {
            return false;
        }
        let (Some(later), Some(earlier)) = (
            self.source.reference(file, operands[0]),
            self.source.reference(file, operands[1]),
        ) else {
            return false;
        };
        if earlier == later {
            return false;
        }
        let array_type = &view.declarations[array.0].ty;
        if !array_type.known()
            || optional(array_type)
            || !matches!(&array_type.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].kind == TypeKind::Int
                    && indices[0].instantiation == Instantiation::Parameter
                    && element.kind == TypeKind::Int)
        {
            return false;
        }
        let Domain::Array { indices, .. } = &self.source.domains.declarations[array.0].domain
        else {
            return false;
        };
        let upper_identity = |domain: &Domain| match domain {
            Domain::Range {
                lower,
                upper:
                    crate::NumericBound::Symbol(id)
                    | crate::NumericBound::Defined {
                        declaration: id, ..
                    },
            } if crate::domains::invariant_integer(lower).ok() == Some(Some(1)) => Some(*id),
            _ => None,
        };
        let Some(upper) = indices.first().and_then(upper_identity) else {
            return false;
        };
        let owner = &self.source.bindings.declarations[array.0];
        let Some(declaration) = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) else {
            return false;
        };
        if self
            .type_dependencies(owner.file, declaration, view, &[])
            .is_err()
        {
            return false;
        }
        for binder in [earlier, later] {
            let declaration = &self.source.bindings.declarations[binder.0];
            if declaration.file != file || declaration.role != DeclarationRole::Generator {
                return false;
            }
            let Some((position, generator)) = generators
                .iter()
                .enumerate()
                .find(|(_, g)| g.range() == declaration.syntax_range)
            else {
                return false;
            };
            let Some(source) = generator.child_nodes().next().map(unwrap) else {
                return false;
            };
            if source.kind() != NodeKind::RangeExpression
                || !self.source.core(file, source, view, "..")
                || generator
                    .child_nodes()
                    .any(|n| n.kind() == NodeKind::WhereFilter)
                || !generator.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
                || upper_identity(&expression_domain(
                    self.source.context,
                    self.source.bindings,
                    file,
                    source,
                )) != Some(upper)
                || self
                    .dependencies(file, source, view, &generators[..position])
                    .is_err()
            {
                return false;
            }
        }
        // Only the true body of this exact parameter comparison supplies the
        // bound: 1 <= j-i <= U-1 <= U. It says nothing about whole output coverage.
        let contains = |node: &SyntaxNode| {
            node.range().start <= access.range().start && access.range().end <= node.range().end
        };
        let mut containing = vec![self.source.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
            {
                let condition = unwrap(children[0]);
                let operands: Vec<_> = condition.child_nodes().map(unwrap).collect();
                if operands.len() == 2
                    && operands.iter().all(|n| n.kind() == NodeKind::Expression)
                    && self.source.reference(file, operands[0]) == Some(earlier)
                    && self.source.reference(file, operands[1]) == Some(later)
                    && self.source.core(file, condition, view, "<")
                    && self
                        .source
                        .view(file, condition, view)
                        .expressions
                        .iter()
                        .any(|e| {
                            e.file == file
                                && e.location.range
                                    == self.source.context.files[file]
                                        .location(condition.range())
                                        .range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.instantiation == Instantiation::Parameter
                                && e.ty.kind == TypeKind::Bool
                        })
                    && self.dependencies(file, condition, view, generators).is_ok()
                {
                    return true;
                }
            }
            containing.extend(children.into_iter().filter(|child| contains(child)));
        }
        false
    }
    pub(in crate::callable_definitions) fn written_index_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        indices: &[&'a SyntaxNode],
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let array_type = &view.declarations[array.0].ty;
        let TypeKind::Array { indices: types, .. } = &array_type.kind else {
            return false;
        };
        let mut domain = &self.source.domains.declarations[array.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let Domain::Array {
            indices: domains, ..
        } = domain
        else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || !matches!(indices.len(), 1 | 2)
            || indices.len() != domains.len()
            || indices.len() != types.len()
        {
            return false;
        }
        let difference_subset = |written_source: &'a SyntaxNode, axis: DeclarationId| {
            let source = unwrap(written_source);
            let source_type = self
                .source
                .expression_type(view, file, source)
                .map(|e| &e.ty);
            if source.kind() != NodeKind::BinaryExpression {
                return false;
            }
            let operands: Vec<_> = source.child_nodes().collect();
            let [left, right] = operands.as_slice() else {
                return false;
            };
            let parameter_set = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element)
                                if element.kind == TypeKind::Int
                                    && element.instantiation == Instantiation::Parameter)
            };
            let operand_type = |operand| {
                self.source
                    .expression_type(view, file, operand)
                    .map(|fact| &fact.ty)
            };
            if !self.source.core(file, source, view, "diff")
                || source_type.is_none_or(|ty| !parameter_set(ty))
                || operands.iter().any(|operand| {
                    let bare = unwrap(operand);
                    bare.kind() != NodeKind::Expression
                        || self.source.reference(file, bare).is_none_or(|id| {
                            let owner = &self.source.bindings.declarations[id.0];
                            !owner.top_level || owner.role != DeclarationRole::Value
                        })
                        || operand_type(operand).is_none_or(|ty| !parameter_set(ty))
                })
                || self.source.reference(file, unwrap(left)) != Some(axis)
                || self.source.operation_fact(view, file, source).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2
                                    && Some(&parameters[0]) == operand_type(left)
                                    && Some(&parameters[1]) == operand_type(right)
                                    && Some(return_type) == source_type)
                })
            {
                return false;
            }
            // A checked difference contains only members of its exact
            // left axis. Inspect both written sources before using this
            // one-selector proof; it establishes no array coverage.
            if matches!(
                self.initialized_source_safety(
                    file,
                    written_source,
                    view,
                    generators,
                    &mut Vec::new(),
                ),
                DefinitionSafety::Unsupported(_)
            ) {
                return false;
            }
            self.source
                .closed_integer_source_error(file, written_source, true, true)
                .is_none()
        };
        indices
            .iter()
            .zip(domains)
            .zip(types)
            .all(|((index, domain), expected)| {
                if !matches!(domain, Domain::Named { .. } | Domain::Range { .. }) {
                    return false;
                }
                let index = unwrap(index);
                let index_type = self.source.expression_type(view, file, index).map(|e| &e.ty);
                if index.kind() != NodeKind::Expression
                    || !expected.known()
                    || optional(expected)
                    || expected.instantiation != Instantiation::Parameter
                    || index_type.is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.instantiation != Instantiation::Parameter
                            || t.kind != expected.kind
                    })
                {
                    return false;
                }
                let Some(binder) = self.source.reference(file, index) else {
                    return false;
                };
                let declaration = &self.source.bindings.declarations[binder.0];
                if declaration.file != file || declaration.role != DeclarationRole::Generator {
                    return false;
                }
                let Some(written_source) = generators
                    .iter()
                    .find(|g| g.range() == declaration.syntax_range)
                    .and_then(|g| g.child_nodes().next())
                else {
                    return false;
                };
                let source = unwrap(written_source);
                let source_type = self.source.expression_type(view, file, source).map(|e| &e.ty);
                if source_type.is_none_or(|t| !t.known() || optional(t)
                    || t.instantiation != Instantiation::Parameter
                    || !matches!(&t.kind, TypeKind::Set(element) if element.kind == expected.kind
                        && element.instantiation == Instantiation::Parameter))
                || self.dependencies(file, source, view, generators).is_err()
            {
                return false;
            }
                if source.kind() == NodeKind::RangeExpression {
                    let owner = &self.source.bindings.declarations[array.0];
                    let Some(array_declaration) = find_node(
                        self.source.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    ) else {
                        return false;
                    };
                    return matches!(domain, Domain::Range { .. })
                        && expected.kind == TypeKind::Int
                        && self.source.core(file, source, view, "..")
                        && generators.iter().any(|g| {
                            g.range() == declaration.syntax_range
                                && !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                                && g.children().iter().any(|c| matches!(c, SyntaxElement::Token(i)
                                    if self.source.context.files[file].parsed.tokens()[*i].kind == TokenKind::In))
                        })
                        && self
                            .type_dependencies(owner.file, array_declaration, view, &[])
                            .is_ok()
                        && crate::domains::same_members(
                            domain,
                            &expression_domain(self.source.context, self.source.bindings, file, source),
                        )
                        .is_ok_and(|same| same == Some(true));
                }
                if source.kind() == NodeKind::BinaryExpression && expected.kind == TypeKind::Int {
                    return matches!(domain, Domain::Named { declaration: axis, .. }
                        if difference_subset(written_source, *axis));
                }
                if source.kind() != NodeKind::Expression {
                    return false;
                }
                let Some(source_id) = self.source.reference(file, source) else {
                    return false;
                };
                if matches!(domain, Domain::Range { .. }) {
                    let set = &self.source.bindings.declarations[source_id.0];
                    if !set.top_level
                        || set.role != DeclarationRole::Value
                        || view.declarations[source_id.0].ty.instantiation
                            != Instantiation::Parameter
                    {
                        return false;
                    }
                    let Some(declaration) = find_node(
                        self.source.context.files[set.file].parsed.tree(),
                        &set.syntax_range,
                        set.role,
                    ) else {
                        return false;
                    };
                    let Some(range) = declaration
                        .child_nodes()
                        .find(|n| is_expression(n.kind()))
                        .map(unwrap)
                        .filter(|n| n.kind() == NodeKind::RangeExpression)
                    else {
                        return false;
                    };
                    let owner = &self.source.bindings.declarations[array.0];
                    let Some(array_declaration) = find_node(
                        self.source.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    ) else {
                        return false;
                    };
                    return self.source.core(set.file, range, view, "..")
                        && crate::definitions::annotations_safe(
                            self.source.context,
                            set.file,
                            declaration,
                        )
                        && self.dependencies(set.file, range, view, &[]).is_ok()
                        && self
                            .type_dependencies(owner.file, array_declaration, view, &[])
                            .is_ok()
                        && crate::domains::same_members(
                            domain,
                            &expression_domain(self.source.context, self.source.bindings, set.file, range),
                        )
                        .is_ok_and(|same| same == Some(true));
                }
                let Domain::Named {
                    declaration: domain_id,
                    ..
                } = domain
                else {
                    return false;
                };
                if source_id == *domain_id {
                    return true;
                }
                let local = &self.source.bindings.declarations[source_id.0];
                if local.file != file
                    || local.role != DeclarationRole::Local
                    || local.syntax_range.end > source.range().start
                {
                    return false;
                }
                let Some(written_local) = find_node(
                    self.source.context.files[file].parsed.tree(),
                    &local.syntax_range,
                    local.role,
                ) else {
                    return false;
                };
                let Some(collection) = written_local
                    .child_nodes().find(|n| is_expression(n.kind()))
                    .map(unwrap)
                    .filter(|n| n.kind() == NodeKind::SetComprehension) else {
                    return false;
                };
                let Some(list) = collection
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    return false;
                };
                let parts: Vec<_> = list.child_nodes().collect();
                if parts.len() != 1 {
                    return false;
                }
                let Some(body) = collection
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                    .map(unwrap)
                    .filter(|n| n.kind() == NodeKind::Expression)
                else {
                    return false;
                };
                let binders: Vec<_> = self.source
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == parts[0].range()
                    })
                    .collect();
                if binders.len() != 1
                    || self.source.reference(file, body) != Some(binders[0].id)
                {
                    return false;
                }
                let Some(written_source) = parts[0].child_nodes().next() else {
                    return false;
                };
                let source = unwrap(written_source);
                if source.kind() == NodeKind::Expression {
                    return self.source.reference(file, source) == Some(*domain_id);
                }
                if expected.kind != TypeKind::Int
                    || !difference_subset(written_source, *domain_id)
                    || self.type_dependencies(file, written_local, view, generators).is_err()
                    || self.source.closed_integer_source_error(file, written_local, true, true).is_some()
                {
                    return false;
                }
                let mut nodes = vec![written_local];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, file, node) {
                        return false;
                    }
                    // This source proof cannot independently inspect earlier
                    // Local aliases; the demonstrated filter uses only its
                    // generator and initialized top-level declarations.
                    if node.kind() == NodeKind::Expression
                        && self.source.reference(file, node).is_some_and(|id|
                            self.source.bindings.declarations[id.0].role == DeclarationRole::Local)
                    {
                        return false;
                    }
                    nodes.extend(node.child_nodes());
                }
                // An unchanged comprehension binder can only remove members
                // from its inspected diff source. Check every header/filter;
                // the subset proof supplies no extent or traversal coverage.
                !matches!(
                    self.initialized_source_safety(
                        file, collection, view, generators, &mut Vec::new(),
                    ),
                    DefinitionSafety::Unsupported(_)
                )
            })
    }
    pub(in crate::callable_definitions) fn type_dependencies(
        &self,
        file: FileId,
        declaration: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let mut types = vec![
            declaration
                .child_nodes()
                .next()
                .ok_or("written local type unavailable")?,
        ];
        let mut ids = Vec::new();
        while let Some(node) = types.pop() {
            if node.kind() == NodeKind::DomainType {
                for expression in node.child_nodes() {
                    extend(
                        &mut ids,
                        self.dependencies(file, expression, view, generators)?,
                    );
                }
            } else {
                types.extend(node.child_nodes());
            }
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn decision_selector_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        indices: &[&'a SyntaxNode],
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> bool {
        let selector = unwrap(indices[2]);
        let children: Vec<_> = selector.child_nodes().collect();
        if selector.kind() != NodeKind::ArrayAccessExpression || children.len() != 3 {
            return false;
        }
        let subject = unwrap(children[0]);
        let Some(selected) = self
            .source
            .reference(file, subject)
            .filter(|_| subject.kind() == NodeKind::Expression)
        else {
            return false;
        };
        let mut outer = &self.source.domains.declarations[array.0].domain;
        while let Domain::Named { domain, .. } = outer {
            outer = domain;
        }
        let mut inner = &self.source.domains.declarations[selected.0].domain;
        while let Domain::Named { domain, .. } = inner {
            inner = domain;
        }
        let (
            Domain::Array { indices: outer, .. },
            Domain::Array {
                indices: inner,
                element,
            },
        ) = (outer, inner)
        else {
            return false;
        };
        outer.len() == 3 && inner.len() == 2
            && matches!((&outer[2], element.as_ref()), (Domain::Named { declaration: expected, .. }, Domain::Named { declaration: actual, .. }) if expected == actual)
            && outer[..2].iter().zip(inner).all(|(expected, actual)| {
                matches!((expected, actual), (Domain::Named { declaration: expected, .. }, Domain::Named { declaration: actual, .. }) if expected == actual)
            })
            && indices[..2].iter().zip(&children[1..]).all(|(outer, inner)| {
                let (outer, inner) = (unwrap(outer), unwrap(inner));
                outer.kind() == NodeKind::Expression && inner.kind() == NodeKind::Expression
                    && self.source.reference(file, outer).is_some_and(|id| self.source.reference(file, inner) == Some(id))
            })
            && self.dependencies(file, selector, view, generators).is_ok()
    }
    pub(in crate::callable_definitions) fn literal_bool_index_membership(
        &self,
        file: FileId,
        access: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let children: Vec<_> = access.child_nodes().collect();
        let [subject, index] = children.as_slice() else {
            return false;
        };
        let parsed = &self.source.context.files[file].parsed;
        if subject.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || index.kind() != NodeKind::Expression
            || index.child_nodes().next().is_some()
            || !matches!(crate::domains::tokens(parsed, index).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            return false;
        }
        let declared = &view.declarations[array.0].ty;
        let type_of = |node: &SyntaxNode| {
            self.source
                .expression_type(self.source.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if !declared.known()
            || optional(declared)
            || type_of(subject) != Some(declared)
            || type_of(access)
                .is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
            || type_of(index).is_none_or(|ty| {
                !ty.known()
                    || optional(ty)
                    || ty.instantiation != Instantiation::Parameter
                    || ty.kind != TypeKind::Int
            })
            || !matches!(&declared.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter
                    && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Bool)
        {
            return false;
        }
        let Domain::Array { indices, .. } = &self.source.domains.declarations[array.0].domain
        else {
            return false;
        };
        let [Domain::Range { .. }] = indices.as_slice() else {
            return false;
        };
        let Some((lower, upper)) = crate::domains::index_domain_interval(&indices[0]) else {
            return false;
        };
        if !crate::domains::invariant_expression_integer(
            self.source.context,
            self.source.bindings,
            file,
            index,
        )
        .is_ok_and(|value| value.is_some_and(|value| lower <= value && value <= upper))
        {
            return false;
        }
        let owner = &self.source.bindings.declarations[array.0];
        if owner.role != DeclarationRole::Value || !owner.top_level {
            return false;
        }
        let Some(written) = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) else {
            return false;
        };
        // Initial support is the original uninitialized array, not evaluation
        // of an arbitrary initializer or a callable/default-local array.
        if !crate::definitions::annotations_safe(self.source.context, owner.file, written)
            || written.child_nodes().any(|node| is_expression(node.kind()))
        {
            return false;
        }
        let Some(array_type) = written
            .child_nodes()
            .next()
            .filter(|node| node.kind() == NodeKind::ArrayType)
        else {
            return false;
        };
        let types: Vec<_> = array_type.child_nodes().collect();
        if types.len() != 2 || types[0].kind() != NodeKind::DomainType {
            return false;
        }
        let axes: Vec<_> = types[0].child_nodes().collect();
        if axes.len() != 1
            || axes[0].kind() != NodeKind::RangeExpression
            || !self.source.core(owner.file, axes[0], view, "..")
            || self
                .type_dependencies(owner.file, written, view, &[])
                .is_err()
        {
            return false;
        }
        true
    }
    pub(in crate::callable_definitions) fn dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let written = node;
        let node = unwrap(node);
        let facts = self.source.view(file, node, view);
        let ty = self
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty);
        if ty.is_none_or(|t| !t.known() || optional(t)) {
            return Err("output dependency type or optionality is unsupported".into());
        }
        if self.source.default_collection(file, node) {
            let (mut ids, fallback, all) =
                self.default_collection_arguments(file, node, view, generators)?;
            extend(&mut ids, self.dependencies(file, fallback, view, &all)?);
            return Ok(ids);
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => self.reference_dependencies(file, written, view, generators),
            NodeKind::ArrayAccessExpression => {
                self.selector_dependencies(file, written, view, generators, children)
            }
            NodeKind::BinaryExpression | NodeKind::RangeExpression => {
                self.operator_dependencies(file, written, view, generators, children)
            }
            NodeKind::UnaryExpression => {
                let name = crate::bindings::symbolic_operator(
                    operator(self.source.context, file, node)
                        .ok_or("unary operator unavailable")?,
                )
                .ok_or("unary operator unsupported")?;
                if !matches!(name, "not" | "+" | "-") || !self.source.core(file, node, view, name) {
                    return Err("unary operation identity is unsupported".into());
                }
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::CallExpression => {
                self.call_dependencies(file, written, view, generators, children)
            }
            NodeKind::GeneratorCallExpression
            | NodeKind::SetComprehension
            | NodeKind::ArrayComprehension => {
                self.iteration_body_dependencies(file, written, view, generators)
            }
            NodeKind::ArrayLiteral | NodeKind::SetLiteral => {
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::ConditionalExpression => {
                if ty.is_none_or(|t| {
                    t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                }) {
                    return Err(
                        "value conditional requires a present parameter integer result".into(),
                    );
                }
                let values = self.source.integer_conditional_values(file, node, view)?;
                self.child_dependencies(file, &values, view, generators)
            }
            NodeKind::LetExpression => {
                self.let_dependencies(file, written, view, generators, children)
            }
            _ => Err("output dependency control, option or value safety is unsupported".into()),
        }
    }
    pub(in crate::callable_definitions) fn boolean_relation_dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        if let Ok(ids) = self.dependencies(file, node, view, generators) {
            return Ok(ids);
        }
        let node = unwrap(node);
        let facts = self.source.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            let range = self.source.context.files[file]
                .location(value.range())
                .range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        if ty(node).is_none_or(|t| !t.known() || t.kind != TypeKind::Bool || optional(t)) {
            return Err("nondefining Boolean relation type is unsupported".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() == NodeKind::ArrayAccessExpression && matches!(children.len(), 2 | 3) {
            let subject = unwrap(children[0]);
            let array = ty(subject).ok_or("Boolean array type unavailable")?;
            if subject.kind() != NodeKind::Expression
                || self.source.reference(file, subject).is_none()
                || !array.known()
                || optional(array)
                || !matches!(&array.kind, TypeKind::Array { indices, element }
                    if indices.len() + 1 == children.len()
                        && element.known() && !optional(element) && element.kind == TypeKind::Bool
                        && indices.iter().zip(&children[1..]).all(|(axis, selector)|
                            axis.known() && !optional(axis) && axis.instantiation == Instantiation::Parameter
                                && matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_))
                                && ty(selector).is_some_and(|index| index.known() && !optional(index)
                                    && index.instantiation == Instantiation::Parameter && index.kind == axis.kind)))
            {
                return Err("nondefining Boolean array or index is unsupported".into());
            }
            if children.len() == 3 {
                let array = self.source.reference(file, subject).unwrap();
                if let Domain::Array { indices, .. } =
                    &self.source.domains.declarations[array.0].domain
                    && indices.iter().any(|axis| axis.numeric_minimum().is_err())
                {
                    return Err("nondefining Boolean source index domain is unsupported".into());
                }
                for selector in &children[1..] {
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(file, selector, false, false)
                    {
                        return Err(reason);
                    }
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &children, view, generators)
                {
                    return Err(reason);
                }
            }
            // Undefined Boolean selection becomes false in this relation. Index
            // evaluation stays strict; this proves no raw membership or output.
            return self.child_dependencies(file, &children, view, generators);
        }
        let name = operator(self.source.context, file, node)
            .and_then(crate::bindings::symbolic_operator)
            .ok_or("nondefining Boolean operation unavailable")?;
        let supported = match node.kind() {
            NodeKind::BinaryExpression => {
                children.len() == 2
                    && matches!(
                        name,
                        "=" | "!=" | "<" | "<=" | ">" | ">=" | "/\\" | "\\/" | "<->" | "->" | "<-"
                    )
            }
            NodeKind::UnaryExpression => children.len() == 1 && name == "not",
            _ => false,
        };
        if !supported || !self.source.core(file, node, view, name) {
            return Err("nondefining Boolean operation identity is unsupported".into());
        }
        let mut ids = Vec::new();
        for child in children {
            if ty(child).is_none_or(|t| !t.known() || t.kind != TypeKind::Bool || optional(t)) {
                return Err("nondefining Boolean operand is unsupported".into());
            }
            extend(
                &mut ids,
                self.boolean_relation_dependencies(file, child, view, generators)?,
            );
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn relational_operand_dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(node);
        let children: Vec<_> = node.child_nodes().collect();
        let facts = self.source.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            let range = self.source.context.files[file]
                .location(value.range())
                .range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::BinaryExpression
            && children.len() == 2
            && ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            && children.iter().all(|child| {
                ty(child).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            })
            && operator(self.source.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                .is_some_and(|name| {
                    matches!(name, "+" | "-") && self.source.core(file, node, view, name)
                })
        {
            let mut ids = Vec::new();
            for child in &children {
                let dependencies =
                    self.dependencies(file, child, view, generators)
                        .or_else(|_| {
                            self.relational_operand_dependencies(file, child, view, generators)
                        })?;
                extend(&mut ids, dependencies);
            }
            return Ok(ids);
        }
        if node.kind() == NodeKind::CallExpression
            && children.len() == 1
            && self.source.core(file, node, view, "bool2int")
            && ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            && ty(children[0]).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Bool)
            && self.source.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0]) && parameters[0].kind == TypeKind::Bool)
            })
        {
            return self.boolean_relation_dependencies(file, children[0], view, generators);
        }
        if node.kind() == NodeKind::RangeExpression
            && children.len() == 2
            && self.source.core(file, node, view, "..")
        {
            let facts = self.source.view(file, node, view);
            let supported = children.iter().all(|child| {
                let range = self.source.context.files[file]
                    .location(child.range())
                    .range;
                matches!(
                    unwrap(child).kind(),
                    NodeKind::Expression | NodeKind::ArrayAccessExpression
                ) && facts
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .is_some_and(|e| {
                        e.ty.kind == TypeKind::Int
                            && e.ty.instantiation == Instantiation::Parameter
                            && !optional(&e.ty)
                    })
            });
            if !supported {
                return Err("relational range endpoints are unsupported".into());
            }
            let mut ids = Vec::new();
            for child in &children {
                let dependencies = match self.dependencies(file, child, view, generators) {
                    Ok(ids) => ids,
                    Err(reason) => self
                        .relational_operand_dependencies(file, child, view, generators)
                        .map_err(|_| reason)?,
                };
                extend(&mut ids, dependencies);
            }
            return Ok(ids);
        }
        if node.kind() != NodeKind::ArrayAccessExpression
            || !matches!(children.len(), 2 | 3)
            || unwrap(children[0]).kind() != NodeKind::Expression
            || self.source.reference(file, unwrap(children[0])).is_none()
            || children[1..].iter().any(|index| {
                !matches!(
                    unwrap(index).kind(),
                    NodeKind::Expression
                        | NodeKind::ArrayAccessExpression
                        | NodeKind::BinaryExpression
                )
            })
        {
            return Err("relational selection shape is unsupported".into());
        }
        let array = ty(children[0]).ok_or("relational array type unavailable")?;
        let TypeKind::Array { indices, element } = &array.kind else {
            return Err("relational subject is not an array".into());
        };
        if !array.known()
            || optional(array)
            || indices.len() != children.len() - 1
            || indices
                .iter()
                .any(|index| index.instantiation != Instantiation::Parameter)
            || !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
            || children[1..].iter().zip(indices).any(|(node, formal)| {
                ty(node).is_none_or(|index| {
                    !index.known()
                        || optional(index)
                        || index.instantiation != Instantiation::Parameter
                        || index.kind != formal.kind
                        || !matches!(index.kind, TypeKind::Int | TypeKind::Enum(_))
                })
            })
        {
            return Err("relational array or scalar index type is unsupported".into());
        }
        // The nearest core Boolean relation converts undefined numeric selections
        // to false. Nested parameter indices retain their own dependencies; this
        // supplies neither raw membership nor a directional output guarantee.
        let mut ids = self.dependencies(file, children[0], view, generators)?;
        for index in &children[1..] {
            let dependencies = self
                .dependencies(file, index, view, generators)
                .or_else(|_| self.relational_operand_dependencies(file, index, view, generators))?;
            extend(&mut ids, dependencies);
        }
        Ok(ids)
    }
    pub(in crate::callable_definitions) fn child_dependencies(
        &self,
        file: FileId,
        children: &[&'a SyntaxNode],
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let mut ids = Vec::new();
        for child in children {
            extend(&mut ids, self.dependencies(file, child, view, generators)?);
        }
        Ok(ids)
    }
}
