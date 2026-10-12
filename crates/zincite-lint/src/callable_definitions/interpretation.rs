//! Interpret selected clauses with borrowed discovery and boundary snapshots.
use super::*;
mod clauses;
mod invocation;
mod outputs;

pub(super) struct CheckedSourceGraph<'a, 's> {
    pub(super) file: FileId,
    pub(super) node: &'a SyntaxNode,
    pub(super) view: &'s crate::callables::CallableView<'s>,
    pub(super) contributions: &'s [(&'a SyntaxNode, DefinitionSafety)],
}

pub(super) struct BodyInterpreter<'a, 's> {
    pub(super) source: &'s SourceInspector<'a>,
    pub(super) instances: &'s [Instance<'a>],
    pub(super) boundaries: &'s [Boundary],
    // Borrow completed reference contributions only for this direct dispatch.
    pub(super) checked_source_graph: Option<CheckedSourceGraph<'a, 's>>,
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
            checked_source_graph: None,
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
        self.boundary(
            clause,
            reason,
            CallableDefinitionStatus::Unsupported,
            CallableDefinitionOperation::Source,
            out,
        );
    }
    pub(in crate::callable_definitions) fn unknown(
        &self,
        clause: &Clause<'a>,
        reason: &str,
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        self.boundary(
            clause,
            reason,
            CallableDefinitionStatus::Unknown,
            CallableDefinitionOperation::Source,
            out,
        );
    }
    pub(in crate::callable_definitions) fn output_unavailable(
        &self,
        clause: &Clause<'a>,
        reason: &str,
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        self.boundary(
            clause,
            reason,
            CallableDefinitionStatus::Unsupported,
            CallableDefinitionOperation::Output,
            out,
        );
    }
    fn boundary(
        &self,
        clause: &Clause<'a>,
        reason: &str,
        status: CallableDefinitionStatus,
        operation: CallableDefinitionOperation,
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        let location = self.source.context.files[clause.file].location(clause.node.range());
        let mut targets = Vec::new();
        let mut unresolved = false;
        for reference in &self.source.bindings.references {
            if reference.file == clause.file
                && reference.kind == ReferenceKind::Value
                && location.range.start <= reference.location.range.start
                && reference.location.range.end <= location.range.end
            {
                match reference.resolution {
                    BindingResolution::Resolved(id) => extend(&mut targets, vec![id]),
                    _ => unresolved = true,
                }
            }
        }
        if unresolved && status == CallableDefinitionStatus::Unknown {
            targets.clear();
        } else if status == CallableDefinitionStatus::Unknown && targets.is_empty() {
            // A closed relation with no affected references cannot supply a bound
            // or definition for an independent declaration.
            return;
        }
        out.push(UnavailableCallableDefinition {
            operation,
            status,
            location,
            targets,
            reason: reason.into(),
        });
    }
    pub(in crate::callable_definitions) fn interpret(
        &self,
        clause: &Clause<'a>,
        view: &crate::callables::CallableView<'_>,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
    ) {
        self.interpret_forwarded(
            (clause, view, known),
            out,
            unavailable,
            &mut Vec::new(),
            None,
        );
    }
    pub(in crate::callable_definitions) fn interpret_root(
        &self,
        clause: &Clause<'a>,
        view: &crate::callables::CallableView<'_>,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
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
            self.interpret(clause, view, known, out, unavailable);
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
            &mut Vec::new(),
            Some(&mut invocation),
        );
    }
    pub(in crate::callable_definitions) fn interpret_forwarded<'b>(
        &'b self,
        inputs: (
            &Clause<'a>,
            &crate::callables::CallableView<'b>,
            &[CallableOutput],
        ),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        invocation: Option<&mut Invocation<'a, 'b>>,
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

        for generator in &clause.generators {
            for value in generator.child_nodes() {
                if let Some(reason) = opaque_source_error(
                    self.source.context,
                    self.source.bindings,
                    inputs.1,
                    self.source.instantiations,
                    self.source.domains,
                    clause.file,
                    value,
                ) {
                    self.unavailable(clause, &reason, unavailable);
                    unavailable.push(UnavailableCallableDefinition {
                        operation: CallableDefinitionOperation::Source,
                        status: CallableDefinitionStatus::Unsupported,
                        location: self.source.context.files[clause.file].location(value.range()),
                        targets: Vec::new(),
                        reason,
                    });
                    active.pop();
                    return;
                }
            }
        }
        if let Some(state) = invocation.as_ref() {
            let (_, view, known) = inputs;
            let mut nodes = vec![clause.node];
            let mut unsupported = None;
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, clause.file, node) {
                    unsupported = Some("invoked body annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            if unsupported.is_none() {
                unsupported = super::adapters::opaque_dependency_error(
                    self.source,
                    clause.file,
                    clause.node,
                    view,
                    &state.actuals,
                    &mut known.iter().map(|output| output.callable).collect(),
                );
            }
            if let Some(reason) = unsupported {
                self.unavailable(clause, &reason, unavailable);
                if unavailable
                    .last()
                    .is_some_and(|row| !row.targets.is_empty())
                {
                    unavailable.push(UnavailableCallableDefinition {
                        operation: CallableDefinitionOperation::Source,
                        status: CallableDefinitionStatus::Unsupported,
                        location: self.source.context.files[clause.file]
                            .location(clause.node.range()),
                        targets: Vec::new(),
                        reason,
                    });
                }
                active.pop();
                return;
            }
        }
        let mut found = Vec::new();
        let direct_invocation = invocation.is_none();
        self.interpret_body(inputs, &mut found, unavailable, active, invocation);
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
        inputs: (
            &Clause<'a>,
            &crate::callables::CallableView<'b>,
            &[CallableOutput],
        ),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, _) = inputs;
        match clause.kind {
            ClauseKind::Unsupported => self.interpret_unsupported_clause(clause, view, unavailable),
            ClauseKind::Relation => self.interpret_relation_clause(inputs, out, unavailable),
            ClauseKind::Conditional => {
                self.interpret_conditional_clause(inputs, out, unavailable, active, invocation)
            }
            ClauseKind::Local => {
                self.interpret_local_clause(inputs, out, unavailable, active, invocation)
            }
            ClauseKind::Assertion => {
                self.interpret_assertion_clause(inputs, out, unavailable, active, invocation)
            }
            ClauseKind::Boolean => self.interpret_boolean_clause(clause, view, out),
            ClauseKind::Equality => self.interpret_equality_clause(clause, view, out, unavailable),
            ClauseKind::Call => {
                self.interpret_call_clause(inputs, out, unavailable, active, invocation)
            }
        }
    }
}
