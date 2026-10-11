//! Interpret selected clauses with borrowed discovery and boundary snapshots.
use super::*;
mod clauses;
mod invocation;
mod outputs;

pub(super) struct BodyInterpreter<'a, 's> {
    pub(super) source: &'s SourceInspector<'a>,
    pub(super) instances: &'s [Instance<'a>],
    pub(super) boundaries: &'s [Boundary],
}
impl<'a, 's> BodyInterpreter<'a, 's> {
    pub(super) fn new(
        source: &'s SourceInspector<'a>,
        instances: &'s [Instance<'a>],
        boundaries: &'s [Boundary],
    ) -> Self {
        Self {
            source,
            instances,
            boundaries,
        }
    }
    pub(super) fn without_bodies(source: &'s SourceInspector<'a>) -> Self {
        Self::new(source, &[], &[])
    }
}
impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn unavailable(
        &self,
        clause: &Clause<'a>,
        reason: &str,
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        let location = self.source.context.files[clause.file].location(clause.node.range());
        let mut targets = Vec::new();
        for r in &self.source.bindings.references {
            if r.file == clause.file
                && r.kind == ReferenceKind::Value
                && location.range.start <= r.location.range.start
                && r.location.range.end <= location.range.end
                && let BindingResolution::Resolved(id) = r.resolution
                && !targets.contains(&id)
            {
                targets.push(id);
            }
        }
        out.push(UnavailableCallableDefinition {
            location,
            targets,
            reason: reason.into(),
        });
    }
    pub(in crate::callable_definitions) fn interpret(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
    ) {
        self.interpret_forwarded(
            (clause, view, known),
            out,
            unavailable,
            inspected,
            &mut Vec::new(),
            None,
        );
    }
    pub(in crate::callable_definitions) fn interpret_root(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
    ) {
        let eligible = matches!(clause.kind, ClauseKind::Call)
            && self
                .source
                .resolved(clause.file, clause.node, view)
                .is_some_and(|(id, parameters)| {
                    self.instances.iter().any(|instance| {
                        instance.id == id
                            && instance.parameters == parameters
                            && !instance.recursive
                    }) && !known
                        .iter()
                        .any(|output| output.callable == id && output.parameters == parameters)
                });
        if !eligible {
            self.interpret(clause, view, known, out, unavailable, inspected);
            return;
        }
        let mut invocation = Invocation {
            actuals: Vec::new(),
            reachable: Some(true),
        };
        self.interpret_forwarded(
            (clause, view, known),
            out,
            unavailable,
            inspected,
            &mut Vec::new(),
            Some(&mut invocation),
        );
    }
    pub(in crate::callable_definitions) fn interpret_forwarded<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, _, _) = inputs;
        let key = (clause.file, clause.node.range());
        if active.contains(&key) {
            self.unavailable(
                clause,
                "recursive Boolean forwarding is unsupported",
                unavailable,
            );
            return;
        }
        active.push(key);
        if let Some(safety) = self.regular_four_owning_body(clause, inputs.1) {
            if let DefinitionSafety::Unsupported(reason) = safety {
                self.unavailable(clause, &reason, unavailable);
            }
            active.pop();
            return;
        }
        if matches!(clause.kind, ClauseKind::Call)
            && let Some((id, parameters)) = self.source.resolved(clause.file, clause.node, inputs.1)
            && let Some(checked) =
                self.inspect_regular_four(clause, inputs.1, id, &parameters, invocation.as_deref())
        {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
            }
            // Source inspection supplies no outputs or inspected-local certificate.
            active.pop();
            return;
        }
        if let Some(safety) = self.optional_increasing_body_safety(inputs.0, inputs.1) {
            if let DefinitionSafety::Unsupported(reason) = safety {
                self.unavailable(clause, &reason, unavailable);
            }
            // The checked optional recurrence supplies no outputs or local certificate.
            active.pop();
            return;
        }
        if let Some(checked) = self.inspect_private_index_conditional(
            inputs,
            unavailable,
            active,
            invocation.as_deref_mut(),
        ) {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
                if unavailable
                    .last()
                    .is_some_and(|row| !row.targets.is_empty())
                {
                    unavailable.push(UnavailableCallableDefinition {
                        location: self.source.context.files[clause.file]
                            .location(clause.node.range()),
                        targets: Vec::new(),
                        reason,
                    });
                }
            }
            active.pop();
            return;
        }
        // This owning let inspector checks locals and selectors in declaration
        // order. A context-free prescan must not reopen its partial initializer.
        if let Some(state) = invocation.as_ref()
            && (!matches!(clause.kind, ClauseKind::Local)
                || self
                    .inspect_uncertain_index_let(clause, inputs.1, None, &[], invocation.as_deref())
                    .is_none())
        {
            let (_, view, _) = inputs;
            let mut nodes = vec![clause.node];
            let mut unsupported = None;
            let before = unavailable.len();
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, clause.file, node) {
                    unsupported = Some("invoked body annotation is unsupported".into());
                }
                if is_expression(node.kind())
                    && (node.kind() == NodeKind::Expression
                        || self
                            .source
                            .expression_type(
                                self.source.view(clause.file, node, view),
                                clause.file,
                                node,
                            )
                            .is_none_or(|e| e.ty.kind != TypeKind::Bool))
                {
                    let source = self.invocation_source(clause.file, node, state);
                    let checked = match source {
                        Ok(Some(actual)) => self.invocation_actual_safety(actual),
                        Ok(None) => self.initialized_source_safety(
                            clause.file,
                            node,
                            view,
                            &clause.generators,
                            &mut Vec::new(),
                        ),
                        Err(reason) => DefinitionSafety::Unsupported(reason),
                    };
                    if let DefinitionSafety::Unsupported(reason) = checked {
                        let selected = unwrap(node);
                        if self
                            .source
                            .expression_type(
                                self.source.view(clause.file, node, view),
                                clause.file,
                                node,
                            )
                            .is_some_and(|e| decision_integer_set(&e.ty))
                            && (selected.kind() == NodeKind::GeneratorCallExpression
                                && self.source.core(clause.file, selected, view, "array_union")
                                || selected.kind() == NodeKind::BinaryExpression
                                    && self.source.core(clause.file, selected, view, "intersect"))
                            && let Err(boundary) = self.decision_set_union_value_safety(
                                clause.file,
                                node,
                                view,
                                &clause.generators,
                                &mut Vec::new(),
                            )
                            && !unavailable.contains(&boundary)
                        {
                            unavailable.push(boundary);
                        }
                        unsupported = Some(reason);
                    }
                    continue;
                }
                nodes.extend(node.child_nodes());
            }
            if let Some(reason) = unsupported {
                self.unavailable(clause, &reason, unavailable);
                if unavailable
                    .last()
                    .is_some_and(|row| !row.targets.is_empty())
                {
                    // A source failure still prevents inspection when its mapped
                    // dependencies are already searched or parameter values.
                    unavailable.push(UnavailableCallableDefinition {
                        location: self.source.context.files[clause.file]
                            .location(clause.node.range()),
                        targets: Vec::new(),
                        reason,
                    });
                }
                active.pop();
                return;
            }
            if unavailable.len() != before {
                active.pop();
                return;
            }
        }
        let mut found = Vec::new();
        let mut local_choices = Vec::new();
        let before = unavailable.len();
        let direct_invocation = invocation.is_none();
        self.interpret_body(
            inputs,
            &mut found,
            unavailable,
            &mut local_choices,
            active,
            invocation,
        );
        if unavailable.len() == before {
            extend(inspected, local_choices);
        }
        // Filtered traversal does not unconditionally enforce a body output.
        // Membership may still prove its relation operands safe.
        let filtered = clause
            .generators
            .iter()
            .any(|g| g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter));
        for output in found {
            if !output.scoped_local
                && (filtered
                    || !clause.generators.is_empty()
                        && (matches!(clause.kind, ClauseKind::Call)
                            || output.coverage == DefinitionCoverage::Scalar)
                        && !clause
                            .generators
                            .iter()
                            .all(|g| self.source.nonempty_source(clause.file, g))
                        && !(direct_invocation
                            && matches!(clause.kind, ClauseKind::Call)
                            && output.coverage == DefinitionCoverage::WholeArray
                            && self.complete_constructed_call_output(
                                clause,
                                inputs.1,
                                inputs.2,
                                output.target,
                            )))
            {
                continue;
            }
            out.push(output);
        }
        active.pop();
    }
    pub(in crate::callable_definitions) fn interpret_body<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, _) = inputs;
        match clause.kind {
            ClauseKind::Unsupported => self.interpret_unsupported_clause(clause, view, unavailable),
            ClauseKind::Relation => self.interpret_relation_clause(inputs, out, unavailable),
            ClauseKind::Conditional => self.interpret_conditional_clause(
                inputs,
                out,
                unavailable,
                inspected,
                active,
                invocation,
            ),
            ClauseKind::Local => {
                self.interpret_local_clause(inputs, out, unavailable, inspected, active, invocation)
            }
            ClauseKind::Assertion => self.interpret_assertion_clause(
                inputs,
                out,
                unavailable,
                inspected,
                active,
                invocation,
            ),
            ClauseKind::Boolean => self.interpret_boolean_clause(clause, view, out),
            ClauseKind::Equality => self.interpret_equality_clause(clause, view, out, unavailable),
            ClauseKind::Call => {
                self.interpret_call_clause(inputs, out, unavailable, inspected, active, invocation)
            }
        }
    }
}
