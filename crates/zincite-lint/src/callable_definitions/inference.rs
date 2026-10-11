//! Stabilize outputs before propagating complete unavailable body boundaries.
use super::*;

pub(super) fn infer_outputs(
    source: &SourceInspector<'_>,
    instances: &[Instance<'_>],
) -> Vec<CallableOutput> {
    let interpreter = BodyInterpreter::new(source, instances, &[]);
    let mut outputs = Vec::new();
    loop {
        let before = outputs.len();
        for instance in instances {
            let mut found = Vec::new();
            for clause in &instance.clauses {
                interpreter.interpret(
                    clause,
                    &instance.view,
                    &outputs,
                    &mut found,
                    &mut Vec::new(),
                    &mut Vec::new(),
                );
            }
            if let Some(reciprocal) = interpreter.reciprocal_array_outputs(instance) {
                found = reciprocal;
            }
            for fact in found {
                let output = CallableOutput {
                    callable: instance.id,
                    parameters: instance.parameters.clone(),
                    target: fact.target,
                    dependencies: fact.dependencies,
                    coverage: fact.coverage,
                    location: fact.location,
                    enforced_boolean: fact.enforced_boolean,
                };
                if !outputs.iter().any(|old| same_output(old, &output)) {
                    outputs.push(output);
                }
            }
        }
        if outputs.len() == before {
            break;
        }
    }
    outputs
}

pub(super) fn infer_boundaries(
    source: &SourceInspector<'_>,
    instances: &[Instance<'_>],
    outputs: &[CallableOutput],
) -> Vec<Boundary> {
    let mut boundaries = Vec::new();
    loop {
        let before = boundaries.len();
        let previous = boundaries.clone();
        let interpreter = BodyInterpreter::new(source, instances, &previous);
        for instance in instances {
            let mut unavailable = Vec::new();
            for clause in &instance.clauses {
                interpreter.interpret(
                    clause,
                    &instance.view,
                    outputs,
                    &mut Vec::new(),
                    &mut unavailable,
                    &mut Vec::new(),
                );
            }
            for unavailable in unavailable {
                let boundary = Boundary {
                    callable: instance.id,
                    parameters: instance.parameters.clone(),
                    unavailable,
                };
                if !boundaries.contains(&boundary) {
                    boundaries.push(boundary);
                }
            }
        }
        if boundaries.len() == before {
            break;
        }
    }
    boundaries
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn reciprocal_array_outputs(
        &self,
        instance: &Instance<'a>,
    ) -> Option<Vec<Output>> {
        if instance.parameters.len() != 2 || instance.clauses.len() != 4 {
            return None;
        }
        let declaration = &self.source.bindings.declarations[instance.id.0];
        let file = declaration.file;
        let view = &instance.view;
        let integer = |ty: &TypeInst, instantiation| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == instantiation
                && ty.kind == TypeKind::Int
        };
        let boolean_operation = |node: &SyntaxNode, name: &str| {
            if !self.source.core(file, node, view, name)
                || self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|expression| {
                        !expression.ty.known()
                            || optional(&expression.ty)
                            || expression.ty.kind != TypeKind::Bool
                    })
            {
                return false;
            }
            let Some(CallFact {
                outcome:
                    CallOutcome::Resolved {
                        parameters,
                        return_type,
                        ..
                    },
                ..
            }) = self.source.operation_fact(view, file, node)
            else {
                return false;
            };
            let [left, right] = parameters.as_slice() else {
                return false;
            };
            return_type.known()
                && !optional(return_type)
                && return_type.kind == TypeKind::Bool
                && integer(left, left.instantiation)
                && if name == "=" {
                    integer(right, right.instantiation)
                } else {
                    right.known()
                        && !optional(right)
                        && right.instantiation == Instantiation::Parameter
                        && matches!(&right.kind, TypeKind::Set(element) if integer(element, Instantiation::Parameter))
                }
        };
        let mut formals = Vec::new();
        for (position, ty) in instance.parameters.iter().enumerate() {
            if !ty.known() || optional(ty) || ty.instantiation != Instantiation::Decision {
                return None;
            }
            let TypeKind::Array { indices, element } = &ty.kind else {
                return None;
            };
            if indices.len() != 1
                || !integer(&indices[0], Instantiation::Parameter)
                || !integer(element, Instantiation::Decision)
            {
                return None;
            }
            let id = formal_parameter(
                self.source.context,
                self.source.bindings,
                instance.id,
                position,
            )?;
            let d = &self.source.bindings.declarations[id.0];
            let written = find_node(
                self.source.context.files[file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )?;
            if !crate::definitions::annotations_safe(self.source.context, file, written)
                || written.child_nodes().any(|n| is_expression(n.kind()))
            {
                return None;
            }
            let header = written.child_nodes().next()?;
            let components: Vec<_> = header.child_nodes().collect();
            if header.kind() != NodeKind::ArrayType
                || components.len() != 2
                || components.iter().any(|n| {
                    n.kind() != NodeKind::ScalarType
                        || n.child_nodes().next().is_some()
                        || !crate::domains::tokens(&self.source.context.files[file].parsed, n)
                            .iter()
                            .any(|t| t.kind == TokenKind::Int)
                })
            {
                return None;
            }
            formals.push(id);
        }
        let bare = |node: &SyntaxNode| {
            let node = unwrap(node);
            let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, node);
            (node.kind() == NodeKind::Expression
                && tokens.len() == 1
                && matches!(
                    tokens[0].kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                ))
            .then(|| self.source.reference(file, node))
            .flatten()
        };
        let access = |node: &'a SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::ArrayAccessExpression
                || !integer(
                    &self.source.expression_type(view, file, node)?.ty,
                    Instantiation::Decision,
                )
            {
                return None;
            }
            let children: Vec<_> = node.child_nodes().collect();
            let [subject, index] = children.as_slice() else {
                return None;
            };
            Some((bare(subject)?, *index))
        };
        let index_set = |node: &SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::CallExpression
                || !self.source.core(file, node, view, "index_set")
            {
                return None;
            }
            let ty = &self.source.expression_type(view, file, node)?.ty;
            if !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if integer(element, Instantiation::Parameter))
            {
                return None;
            }
            let mut children = node.child_nodes();
            let subject = children.next()?;
            if children.next().is_some() {
                return None;
            }
            bare(subject)
        };
        let mut directions = Vec::new();
        for equality in instance
            .clauses
            .iter()
            .filter(|c| matches!(c.kind, ClauseKind::Equality))
        {
            if equality.file != file
                || equality.generators.len() != 1
                || !boolean_operation(equality.node, "=")
                || !crate::definitions::annotations_safe(self.source.context, file, equality.node)
            {
                return None;
            }
            let operands: Vec<_> = equality.node.child_nodes().collect();
            let [nested, result] = operands.as_slice() else {
                return None;
            };
            let (target, inner) = access(nested)?;
            let (source, index) = access(inner)?;
            let binder = bare(result)?;
            if source == target
                || !formals.contains(&source)
                || !formals.contains(&target)
                || bare(index) != Some(binder)
                || !integer(&view.declarations[binder.0].ty, Instantiation::Parameter)
                || !self
                    .source
                    .exact_index(file, source, index, &equality.generators, view)
            {
                return None;
            }
            let generator = equality.generators[0];
            let tokens = crate::domains::tokens(&self.source.context.files[file].parsed, generator);
            if tokens.iter().position(|t| t.kind == TokenKind::In) != Some(1)
                || !matches!(
                    tokens[0].kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                )
                || generator.child_nodes().next().and_then(index_set) != Some(source)
                || !crate::definitions::annotations_safe(self.source.context, file, generator)
            {
                return None;
            }
            let membership = instance.clauses.iter().find(|c| {
                if !matches!(c.kind, ClauseKind::Relation)
                    || c.file != file
                    || c.generators.len() != 1
                    || c.generators[0].range() != generator.range()
                    || !boolean_operation(c.node, "in")
                    || !crate::definitions::annotations_safe(self.source.context, file, c.node)
                {
                    return false;
                }
                let operands: Vec<_> = c.node.child_nodes().collect();
                let [value, indices] = operands.as_slice() else {
                    return false;
                };
                access(value).is_some_and(|(array, position)| {
                    array == source && bare(position) == Some(binder)
                }) && index_set(indices) == Some(target)
            })?;
            directions.push((source, target, binder, membership.node.range()));
        }
        let [first, second] = directions.as_slice() else {
            return None;
        };
        if first.0 != second.1
            || first.1 != second.0
            || first.2 == second.2
            || first.3 == second.3
            || instance
                .clauses
                .iter()
                .filter(|c| matches!(c.kind, ClauseKind::Relation))
                .count()
                != 2
        {
            return None;
        }
        // The two memberships and inverse equations make both arrays bijections,
        // including empty axes. Each whole array still depends on its opposite.
        Some(
            instance
                .clauses
                .iter()
                .filter(|c| matches!(c.kind, ClauseKind::Equality))
                .zip(&directions)
                .map(|(clause, (source, target, _, _))| Output {
                    target: *target,
                    dependencies: vec![*source],
                    coverage: DefinitionCoverage::WholeArray,
                    location: self.source.context.files[file].location(clause.node.range()),
                    enforced_boolean: false,
                    scoped_local: false,
                })
                .collect(),
        )
    }
}
