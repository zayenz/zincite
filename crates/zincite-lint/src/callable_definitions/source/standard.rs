//! Selected standard bodies retain written shape, metadata, owning scopes and primitives.
use super::*;
mod union;

impl<'a> SourceInspector<'a> {
    // The array source family uses only these selected standard primitives.
    // Their full written trees must contain no implementation/default expression.
    pub(in crate::callable_definitions) fn parameter_array_primitive(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        name: &str,
        parameters: &[TypeInst],
        result: &TypeInst,
    ) -> Result<(), String> {
        let facts = self.view(file, node, view);
        let fact = self
            .operation_fact(facts, file, node)
            .ok_or("parameter array primitive selection is unavailable")?;
        let CallOutcome::Resolved {
            declaration,
            parameters: selected,
            return_type,
        } = &fact.outcome
        else {
            return Err("parameter array primitive selection is unsupported".into());
        };
        if !self.core(file, node, facts, name)
            || fact.generator_argument.is_some()
            || selected.as_slice() != parameters
            || return_type != result
            || self
                .expression_type(facts, file, node)
                .is_none_or(|value| value.ty != *result)
        {
            return Err("parameter array primitive identity or signature is unsupported".into());
        }
        let owner = &self.bindings.declarations[declaration.0];
        if owner.role != DeclarationRole::Function {
            return Err("parameter array primitive declaration is unsupported".into());
        }
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("parameter array primitive source is unavailable")?;
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                let value = node
                    .child_nodes()
                    .next()
                    .map(unwrap)
                    .ok_or("parameter array primitive annotation value is unavailable")?;
                let string = value.kind() == NodeKind::Expression
                    && crate::domains::tokens(&self.context.files[owner.file].parsed, value)
                        .first()
                        .is_some_and(|token| token.kind == TokenKind::StringLiteral);
                if !string
                    && ![
                        "mzn_internal_representation",
                        "promise_total",
                        "promise_commutative",
                    ]
                    .iter()
                    .any(|name| self.atomic_standard_metadata_safe(owner.file, node, name))
                {
                    return Err("parameter array primitive annotation is unsupported".into());
                }
                continue;
            }
            if is_expression(node.kind()) {
                return Err(
                    "parameter array primitive body, default or type expression is unsupported"
                        .into(),
                );
            }
            nodes.extend(node.child_nodes());
        }
        Ok(())
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    // Inspect the selected argument without treating an ignored wrapper as enforcement.

    // Inspect the aggregate source without enforcing members or forwarding outputs.

    pub(in crate::callable_definitions) fn reverse_parameter_array_body(
        &self,
        file: FileId,
        body: &'a SyntaxNode,
        formal: DeclarationId,
        view: &crate::callables::CallableView<'_>,
        array: &TypeInst,
    ) -> Result<(), String> {
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(view, file, node)
                .map(|value| &value.ty)
        };
        let reference = |node: &SyntaxNode| {
            (node.kind() == NodeKind::Expression && node.child_nodes().next().is_none())
                .then(|| self.source.reference(file, node))
                .flatten()
        };
        let literal = |node: &SyntaxNode, expected| {
            node.kind() == NodeKind::Expression
                && node.child_nodes().next().is_none()
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                && crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    node,
                ) == Ok(Some(expected))
        };
        // Check all closed body fragments before an unsupported shape can hide
        // a selected local initializer failure behind the unknown condition.
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, body, false, true)
        {
            return Err(reason);
        }
        let mut nodes = vec![body];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return Err("reverse array body annotation is unsupported".into());
            }
            nodes.extend(node.child_nodes());
        }
        let branches: Vec<_> = body.child_nodes().collect();
        let [then, otherwise] = branches.as_slice() else {
            return Err("reverse array body requires its complete conditional".into());
        };
        let first: Vec<_> = then.child_nodes().collect();
        let last: Vec<_> = otherwise.child_nodes().collect();
        let ([guard, empty], [local]) = (first.as_slice(), last.as_slice()) else {
            return Err("reverse array conditional branches are unsupported".into());
        };
        if body.kind() != NodeKind::ConditionalExpression
            || ty(body) != Some(array)
            || then.kind() != NodeKind::ConditionalBranch
            || otherwise.kind() != NodeKind::ElseBranch
            || guard.kind() != NodeKind::BinaryExpression
            || ty(guard) != Some(&boolean)
            || empty.kind() != NodeKind::ArrayLiteral
            || empty.child_nodes().next().is_some()
            || ty(empty).is_none_or(|value| !crate::types::coerces(value, array))
            || local.kind() != NodeKind::LetExpression
            || ty(local) != Some(array)
        {
            return Err("reverse array conditional types or empty branch are unsupported".into());
        }
        let guard_parts: Vec<_> = guard.child_nodes().collect();
        let [length, zero] = guard_parts.as_slice() else {
            return Err("reverse array condition operands are unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            guard,
            view,
            "=",
            &[integer.clone(), integer.clone()],
            &boolean,
        )?;
        self.source.parameter_array_primitive(
            file,
            length,
            view,
            "length",
            std::slice::from_ref(array),
            &integer,
        )?;
        if !literal(zero, 0)
            || length.child_nodes().count() != 1
            || length.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array condition source identity is unsupported".into());
        }
        let let_parts: Vec<_> = local.child_nodes().collect();
        let [block, reshape] = let_parts.as_slice() else {
            return Err("reverse array local body is unsupported".into());
        };
        let locals: Vec<_> = block.child_nodes().collect();
        let [cells, upper] = locals.as_slice() else {
            return Err("reverse array requires its two initialized locals".into());
        };
        if block.kind() != NodeKind::LetBlock || reshape.kind() != NodeKind::CallExpression {
            return Err("reverse array local or reshape shape is unsupported".into());
        }
        let local_id = |written: &SyntaxNode| {
            self.source
                .bindings
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Local
                        && declaration.syntax_range == written.range()
                })
                .map(|declaration| declaration.id)
        };
        let cells_id = local_id(cells).ok_or("reverse array cell local identity is unavailable")?;
        let upper_id =
            local_id(upper).ok_or("reverse array upper local identity is unavailable")?;
        let mut initializers = Vec::new();
        for (written, id, expected) in [(*cells, cells_id, array), (*upper, upper_id, &integer)] {
            let values: Vec<_> = written
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .collect();
            if written.kind() != NodeKind::Declaration
                || values.len() != 1
                || view.declarations[id.0].ty != *expected
                || ty(values[0]) != Some(expected)
            {
                return Err("reverse array local type or initializer is unsupported".into());
            }
            self.type_dependencies(file, written, view, &[])?;
            initializers.push(values[0]);
        }
        let flatten = initializers[0];
        let plus = initializers[1];
        self.source.parameter_array_primitive(
            file,
            flatten,
            view,
            "array1d",
            std::slice::from_ref(array),
            array,
        )?;
        if flatten.child_nodes().count() != 1
            || flatten.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array flattened local source is unsupported".into());
        }
        let plus_parts: Vec<_> = plus.child_nodes().collect();
        let [size, one] = plus_parts.as_slice() else {
            return Err("reverse array upper initializer shape is unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            plus,
            view,
            "+",
            &[integer.clone(), integer.clone()],
            &integer,
        )?;
        self.source.parameter_array_primitive(
            file,
            size,
            view,
            "length",
            std::slice::from_ref(array),
            &integer,
        )?;
        if !literal(one, 1)
            || size.child_nodes().count() != 1
            || size.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array upper source identity is unsupported".into());
        }
        let reshape_parts: Vec<_> = reshape.child_nodes().collect();
        let [axis, comprehension] = reshape_parts.as_slice() else {
            return Err("reverse array reshape arguments are unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            reshape,
            view,
            "array1d",
            &[set.clone(), array.clone()],
            array,
        )?;
        self.source.parameter_array_primitive(
            file,
            axis,
            view,
            "index_set",
            std::slice::from_ref(array),
            &set,
        )?;
        if axis.child_nodes().count() != 1
            || axis.child_nodes().next().and_then(reference) != Some(formal)
            || comprehension.kind() != NodeKind::ArrayComprehension
            || ty(comprehension) != Some(array)
        {
            return Err("reverse array reshape source identity is unsupported".into());
        }
        let comp_parts: Vec<_> = comprehension.child_nodes().collect();
        let [selection, list] = comp_parts.as_slice() else {
            return Err("reverse array comprehension shape is unsupported".into());
        };
        let headers: Vec<_> = list.child_nodes().collect();
        let [header] = headers.as_slice() else {
            return Err("reverse array requires one comprehension header".into());
        };
        let sources: Vec<_> = header.child_nodes().collect();
        let [source] = sources.as_slice() else {
            return Err("reverse array requires its unfiltered source".into());
        };
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
        let [binder] = binders.as_slice() else {
            return Err("reverse array binder identity is unsupported".into());
        };
        if list.kind() != NodeKind::GeneratorList
            || header.kind() != NodeKind::Generator
            || crate::domains::generator_slots(&self.source.context.files[file].parsed, header) != 1
            || !header.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In)
            })
            || view.declarations[binder.id.0].ty != integer
        {
            return Err("reverse array binder scope or type is unsupported".into());
        }
        self.source.parameter_array_primitive(
            file,
            source,
            view,
            "index_set",
            std::slice::from_ref(array),
            &set,
        )?;
        if source.child_nodes().count() != 1
            || source.child_nodes().next().and_then(reference) != Some(cells_id)
        {
            return Err("reverse array generator source identity is unsupported".into());
        }
        let selection_parts: Vec<_> = selection.child_nodes().collect();
        let [subject, selector] = selection_parts.as_slice() else {
            return Err("reverse array cell selection shape is unsupported".into());
        };
        let selector_parts: Vec<_> = selector.child_nodes().collect();
        let [upper_value, index] = selector_parts.as_slice() else {
            return Err("reverse array selector shape is unsupported".into());
        };
        if selection.kind() != NodeKind::ArrayAccessExpression
            || ty(selection) != Some(&integer)
            || reference(subject) != Some(cells_id)
            || reference(upper_value) != Some(upper_id)
            || reference(index) != Some(binder.id)
        {
            return Err("reverse array selected local or binder identity is unsupported".into());
        }
        self.source.parameter_array_primitive(
            file,
            selector,
            view,
            "-",
            &[integer.clone(), integer.clone()],
            &integer,
        )?;
        let checked = [*guard, flatten, plus, *axis, *source];
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_children_safety(file, &checked, view, &[])
        {
            return Err(reason);
        }
        if let DefinitionSafety::Unsupported(reason) =
            self.opaque_source_safety(file, comprehension, view, &[])
        {
            return Err(reason);
        }
        Ok(())
    }
    // This owning ignored scope needs source inspection, never reverse membership.
}
