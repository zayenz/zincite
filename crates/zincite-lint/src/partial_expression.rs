//! Advice over scoped operation requirements, not a feasibility or crash proof.
//! Relational Boolean definedness preserves raw hazards; only interpreted core
//! default capture can absorb a left-side undefined value. No rewrite is offered.
use crate::callables::{core_operation_outcome, operation_head_start};
use crate::domains::tokens;
use crate::{
    BindingFacts, CallFact, CallableFacts, FileFinding, FileId, GuardActivation,
    GuardAssumptionKind, GuardContext, GuardEvaluation, GuardObligation, GuardObligationKind,
    GuardedExpression, GuardedFacts, GuardedOutcome, IterationCoverage, IterationFacts,
    ModelContext, Presence, Rule, Severity, SourceDiagnostic, SourceLocation,
};
use std::collections::HashMap;
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

pub(super) fn check_partial_expressions(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    guarded: &GuardedFacts,
    iteration: &IterationFacts,
    prior_findings: &[FileFinding],
    input_keys: &[crate::InputObligationKey],
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut call_indices = HashMap::with_capacity(calls.calls.len());
    for (row, fact) in calls.calls.iter().enumerate() {
        call_indices
            .entry((fact.file, fact.location.range.start))
            .or_insert(row);
    }
    let operation_at = |file, node: &SyntaxNode| {
        operation_head_start(context, file, node)
            .and_then(|start| call_indices.get(&(file, start)))
            .map(|&row| &calls.calls[row])
    };
    let mut expression_indices = HashMap::with_capacity(guarded.expressions.len());
    for (row, expression) in guarded.expressions.iter().enumerate() {
        expression_indices
            .entry((
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .or_insert(row);
    }
    let mut obligation_indices: HashMap<_, Vec<_>> = HashMap::new();
    for (row, obligation) in guarded.obligations.iter().enumerate() {
        if !matches!(obligation.kind, GuardObligationKind::Assertion) {
            obligation_indices
                .entry((
                    obligation.file,
                    obligation.operation.range.start,
                    obligation.operation.range.end,
                ))
                .or_default()
                .push(row);
        }
    }
    let mut limitation_indices: HashMap<_, Vec<_>> = HashMap::new();
    let mut abort_limitations = Vec::new();
    for (row, limitation) in guarded.limitations.iter().enumerate() {
        limitation_indices
            .entry((
                limitation.file,
                limitation.location.range.start,
                limitation.location.range.end,
            ))
            .or_default()
            .push(row);
        if limitation.reason == "default capture of aborting assertion is unsupported" {
            abort_limitations.push(row);
        }
    }
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() {
            continue;
        }
        let Some(suppressions) = &source.suppressions else {
            continue;
        };
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if suppressions[item].contains(&Rule::PartialExpression) {
                continue;
            }
            let mut nodes = Vec::new();
            collect_nodes(node, &mut nodes);
            let default_nodes: Vec<_> = nodes
                .iter()
                .copied()
                .filter(|node| {
                    node.kind() == NodeKind::BinaryExpression
                        && tokens(&source.parsed, node)
                            .first()
                            .is_some_and(|token| token.kind == TokenKind::Default)
                })
                .collect();
            for operation in &nodes {
                let Some(identity) =
                    operation_identity(context, bindings, operation_at, file, operation)
                else {
                    continue;
                };
                if identity == Ok(false) {
                    continue;
                }
                let location = source.location(operation.range());
                let key = (file, location.range.start, location.range.end);
                let evaluation = expression_indices
                    .get(&key)
                    .map(|&row| &guarded.expressions[row]);
                if evaluation.is_some_and(|e| e.context.activation == GuardActivation::Inactive)
                    || empty_iteration(iteration, file, &location)
                {
                    continue;
                }
                if let Err(reason) = identity {
                    limitations.push(limitation(location, &reason));
                    continue;
                }
                for &row in obligation_indices.get(&key).into_iter().flatten() {
                    let obligation = &guarded.obligations[row];
                    if obligation.context.activation == GuardActivation::Inactive
                        || captured_by_default(
                            context,
                            bindings,
                            operation_at,
                            |file, location| {
                                expression_indices
                                    .get(&(file, location.range.start, location.range.end))
                                    .map(|&row| &guarded.expressions[row])
                            },
                            file,
                            &default_nodes,
                            obligation,
                        )
                    {
                        continue;
                    }
                    if let GuardedOutcome::Unsupported(reason) = &obligation.outcome {
                        limitations.push(limitation(
                            location.clone(),
                            &format!("{reason}{}", describe_context(context, &obligation.context)),
                        ));
                        continue;
                    }
                    if matches!(obligation.kind, GuardObligationKind::Index { .. })
                        && prior_findings.iter().any(|f| {
                            f.rule == Rule::IndexSetMismatch
                                && f.location.path == location.path
                                && f.location.range == location.range
                        })
                    {
                        continue;
                    }
                    if crate::input_preconditions::obligation_key(obligation)
                        .is_some_and(|key| input_keys.contains(&key))
                    {
                        continue;
                    }
                    let failed = obligation.outcome == GuardedOutcome::Refuted;
                    let candidate = matches!(obligation.kind, GuardObligationKind::Index { .. })
                        && obligation.outcome != GuardedOutcome::Proven
                        && iteration.has_incompatible_index_candidates(obligation);
                    if !failed && !candidate {
                        continue;
                    }
                    let requirement = match &obligation.kind {
                        GuardObligationKind::Nonzero => "the divisor must be nonzero".into(),
                        GuardObligationKind::Positive => {
                            "the logarithm input must be strictly positive".into()
                        }
                        GuardObligationKind::Presence => "deopt requires a present value".into(),
                        GuardObligationKind::Nonempty { aggregate } => {
                            format!("{aggregate} requires a nonempty collection of present values")
                        }
                        GuardObligationKind::Index {
                            array, dimension, ..
                        } => {
                            let d = &bindings.declarations[array.0];
                            format!(
                                "the index must belong to array '{}' dimension {dimension} (declared at {}:{}:{})",
                                d.name,
                                d.location.path.display(),
                                d.location.line,
                                d.location.column
                            )
                        }
                        GuardObligationKind::Assertion => unreachable!(),
                    };
                    let proof = if failed {
                        "the requirement is refuted when this operation is evaluated"
                    } else {
                        "some exact interpreted candidate indices do not fit this dimension; this does not establish that any candidate is executed or feasible"
                    };
                    let mut message = format!(
                        "{requirement}; {proof}{}",
                        describe_context(context, &obligation.context)
                    );
                    if guarded.obligations.iter().any(|a| {
                        a.file == file
                            && matches!(a.kind, GuardObligationKind::Assertion)
                            && a.outcome == GuardedOutcome::Refuted
                            && contains(&a.operand, &location)
                    }) {
                        message.push_str("; the enclosing assertion condition is refuted and can abort when evaluated");
                    }
                    if !findings.iter().any(|f: &FileFinding| {
                        f.location.path == location.path
                            && f.location.range == location.range
                            && f.message == message
                    }) {
                        findings.push(FileFinding {
                            fix: None,
                            location: location.clone(),
                            rule: Rule::PartialExpression,
                            severity: Severity::Warning,
                            message,
                        });
                    }
                }
                for &row in limitation_indices.get(&key).into_iter().flatten() {
                    let l = &guarded.limitations[row];
                    limitations.push(limitation(location.clone(), &l.reason));
                }
            }
            // An abort is not ordinary undefinedness. Retain the producer's
            // explicit default-capture boundary without inventing an assertion rule.
            for l in abort_limitations
                .iter()
                .map(|&row| &guarded.limitations[row])
                .filter(|l| l.file == file && contains(&source.location(node.range()), &l.location))
            {
                let key = (file, l.location.range.start, l.location.range.end);
                let evaluation = expression_indices
                    .get(&key)
                    .map(|&row| &guarded.expressions[row]);
                if !evaluation.is_some_and(|e| e.context.activation == GuardActivation::Inactive)
                    && !empty_iteration(iteration, file, &l.location)
                {
                    limitations.push(limitation(l.location.clone(), &l.reason));
                }
            }
        }
    }
    limitations.sort_by(|a, b| {
        a.location
            .path
            .cmp(&b.location.path)
            .then(a.location.range.start.cmp(&b.location.range.start))
            .then(a.message.cmp(&b.message))
    });
    limitations.dedup_by(|a, b| {
        a.location.path == b.location.path
            && a.location.range == b.location.range
            && a.message == b.message
    });
    (findings, limitations)
}
pub(super) fn collect_nodes<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxNode>) {
    out.push(node);
    for child in node.child_nodes() {
        collect_nodes(child, out);
    }
}
fn contains(a: &SourceLocation, b: &SourceLocation) -> bool {
    a.path == b.path && a.range.start <= b.range.start && b.range.end <= a.range.end
}
pub(super) fn empty_iteration(
    facts: &IterationFacts,
    file: FileId,
    location: &SourceLocation,
) -> bool {
    facts.iterations.iter().any(|i| {
        i.file == file && contains(&i.body, location) && i.coverage == IterationCoverage::Empty
    })
}
fn operation_identity<'a>(
    context: &ModelContext,
    bindings: &BindingFacts,
    operation_at: impl Fn(FileId, &SyntaxNode) -> Option<&'a CallFact>,
    file: FileId,
    node: &SyntaxNode,
) -> Option<Result<bool, String>> {
    if node.kind() == NodeKind::ArrayAccessExpression {
        return Some(Ok(true));
    }
    let parsed = &context.files[file].parsed;
    if node.kind() == NodeKind::BinaryExpression {
        let kind = tokens(parsed, node).first()?.kind;
        return match kind {
            TokenKind::Div => Some(core_operation_outcome(
                context,
                bindings,
                operation_at(file, node).map(|fact| &fact.outcome),
                node.kind(),
                "div",
            )),
            TokenKind::Mod => Some(core_operation_outcome(
                context,
                bindings,
                operation_at(file, node).map(|fact| &fact.outcome),
                node.kind(),
                "mod",
            )),
            _ => None,
        };
    }
    if matches!(
        node.kind(),
        NodeKind::CallExpression | NodeKind::GeneratorCallExpression
    ) {
        let token = *tokens(parsed, node).first()?;
        let name = parsed.source()[token.range.clone()].trim_matches('\'');
        if node.kind() == NodeKind::CallExpression
            && node.child_nodes().count() != 1
            && !(name == "log" && node.child_nodes().count() == 2)
        {
            return None;
        }
        if matches!(
            name,
            "min" | "max" | "deopt" | "ln" | "log10" | "log2" | "log"
        ) {
            return Some(core_operation_outcome(
                context,
                bindings,
                operation_at(file, node).map(|fact| &fact.outcome),
                node.kind(),
                name,
            ));
        }
    }
    None
}
pub(super) fn captured_by_default<'a>(
    context: &ModelContext,
    bindings: &BindingFacts,
    operation_at: impl Fn(FileId, &SyntaxNode) -> Option<&'a CallFact>,
    expression_at: impl Fn(FileId, &SourceLocation) -> Option<&'a GuardedExpression>,
    file: FileId,
    nodes: &[&SyntaxNode],
    obligation: &GuardObligation,
) -> bool {
    let operation = &obligation.operation;
    let scope = &obligation.context;
    nodes.iter().any(|node| {
        let source = &context.files[file];
        if node.kind() != NodeKind::BinaryExpression
            || !tokens(&source.parsed, node)
                .first()
                .is_some_and(|t| t.kind == TokenKind::Default)
            || core_operation_outcome(
                context,
                bindings,
                operation_at(file, node).map(|fact| &fact.outcome),
                node.kind(),
                "default",
            ) != Ok(true)
        {
            return false;
        }
        let Some(left) = node.child_nodes().next() else {
            return false;
        };
        let left = source.location(left.range());
        if !contains(&left, operation)
            || scope
                .nearest_boolean
                .as_ref()
                .is_some_and(|boolean| contains(&left, boolean))
        {
            return false;
        }
        let Some(expression) = expression_at(file, &source.location(node.range())) else {
            return false;
        };
        let Some(left_value) = expression_at(file, &left) else {
            return false;
        };
        expression.raw_definedness == GuardedOutcome::Proven
            && expression.definedness == GuardedOutcome::Proven
            && (left_value.definedness != GuardedOutcome::Proven
                || left_value.presence != Some(Presence::Present))
    })
}
fn describe_context(context: &ModelContext, scope: &GuardContext) -> String {
    let mut message = if scope.activation == GuardActivation::Conditional {
        "; evaluation is conditional on the enclosing selection".into()
    } else {
        String::new()
    };
    match scope.evaluation {
        GuardEvaluation::DefaultFallback => message.push_str("; this is the default fallback"),
        GuardEvaluation::AssertionReturn => {
            message.push_str("; this value is evaluated after the assertion succeeds")
        }
        _ => {}
    }
    if let Some(assumption) = scope
        .assumptions
        .iter()
        .rev()
        .find(|a| matches!(a.kind, GuardAssumptionKind::Condition { .. }))
    {
        let GuardAssumptionKind::Condition { expected } = assumption.kind else {
            unreachable!();
        };
        let source = &context.files[assumption.file];
        let range = assumption.location.range.start - source.byte_offset
            ..assumption.location.range.end - source.byte_offset;
        message.push_str(&format!(
            "; under condition '{}' = {expected} at {}:{}:{}",
            source.parsed.source()[range].trim(),
            assumption.location.path.display(),
            assumption.location.line,
            assumption.location.column
        ));
    }
    if let Some(boolean) = &scope.nearest_boolean {
        message.push_str(&format!(
            "; undefinedness can become false at the nearest Boolean expression {}:{}:{}",
            boolean.path.display(),
            boolean.line,
            boolean.column
        ));
    }
    message
}
fn limitation(location: SourceLocation, reason: &str) -> SourceDiagnostic {
    SourceDiagnostic {
        location,
        message: format!("partial-expression: {reason}"),
    }
}
