//! Advice over interpreted emptiness and Boolean truth, not modeller intent.
//! Identities retain evaluation requirements and never authorize source edits.
use crate::callables::{core_operation, is_expression};
use crate::definitions::resolved_reference;
use crate::domains::tokens;
use crate::{
    BindingFacts, CallableFacts, FileFinding, FileId, GuardActivation, GuardAssumptionKind,
    GuardContext, GuardObligationKind, GuardedFacts, GuardedOutcome, IterationCoverage,
    IterationFacts, ModelContext, OptionalFacts, Rule, Severity, SourceDiagnostic, SourceLocation,
};
use zincite_syntax::{NodeKind, SyntaxNode};

pub(super) fn check_vacuous_constraints(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    guarded: &GuardedFacts,
    optional: &OptionalFacts,
    iteration: &IterationFacts,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
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
            if suppressions[item].contains(&Rule::VacuousConstraint) {
                continue;
            }
            let item_location = source.location(node.range());
            for fact in iteration
                .iterations
                .iter()
                .filter(|i| i.file == file && contains(&item_location, &i.location))
            {
                for filter in &fact.filters {
                    let value = guarded.expression(file, &filter.location);
                    if filter.incoming_empty
                        || value.is_some_and(|g| g.context.activation == GuardActivation::Inactive)
                    {
                        continue;
                    }
                    if let GuardedOutcome::Unsupported(reason) = &filter.definedness {
                        limitations.push(limitation(filter.location.clone(), reason));
                        continue;
                    }
                    if filter.coverage != IterationCoverage::Empty
                        || filter.definedness != GuardedOutcome::Proven
                    {
                        continue;
                    }
                    let proof = if filter.truth == GuardedOutcome::Refuted {
                        "the filter's Boolean value is proved false"
                    } else {
                        "the supported intersection with its incoming index domain is empty"
                    };
                    let mut message = format!(
                        "{proof}; it rejects every incoming candidate{}; this does not prove that any candidate is feasible",
                        value
                            .map(|g| context_message(context, &g.context, &filter.location))
                            .unwrap_or_default()
                    );
                    if value.is_some_and(|g| g.raw_definedness == GuardedOutcome::Refuted) {
                        message.push_str(
                            "; an undefined operand becomes false at this Boolean boundary",
                        );
                    }
                    findings.push(finding(filter.location.clone(), message));
                }
            }
            let mut nodes = Vec::new();
            collect(node, &mut nodes);
            for n in &nodes {
                let Some(name) = quantifier_name(context, file, n) else {
                    continue;
                };
                let location = source.location(n.range());
                let Some(value) = guarded.expression(file, &location) else {
                    continue;
                };
                if value.context.activation == GuardActivation::Inactive {
                    continue;
                }
                match core_operation(context, bindings, calls, file, n, name) {
                    Ok(false) => continue,
                    Err(reason) => {
                        limitations.push(limitation(location, &reason));
                        continue;
                    }
                    Ok(true) => {}
                }
                if let GuardedOutcome::Unsupported(reason) = &value.raw_definedness {
                    limitations.push(limitation(location, reason));
                    continue;
                }
                if value.raw_definedness != GuardedOutcome::Proven
                    || value.definedness != GuardedOutcome::Proven
                {
                    if guarded.obligations.iter().any(|o| {
                        o.file == file
                            && matches!(o.kind, GuardObligationKind::Assertion)
                            && o.outcome == GuardedOutcome::Refuted
                            && contains(&location, &o.operation)
                    }) {
                        limitations.push(limitation(location,"assertion evaluation can abort; no total Boolean identity is established"));
                    }
                    continue;
                }
                let traversal = iteration.iteration(file, &location).or_else(|| {
                    n.child_nodes()
                        .next()
                        .and_then(|arg| iteration.iteration(file, &source.location(arg.range())))
                });
                let counts = if n.kind() == NodeKind::GeneratorCallExpression {
                    optional.collection(file, &location)
                } else {
                    n.child_nodes().next().and_then(|arg| {
                        optional
                            .collection(file, &source.location(arg.range()))
                            .or_else(|| {
                                resolved_reference(context, bindings, file, unwrap(arg))
                                    .and_then(|id| optional.declaration(id))
                                    .and_then(|d| d.collection.as_ref())
                            })
                    })
                };
                let empty = traversal.is_some_and(|i| i.coverage == IterationCoverage::Empty);
                let absent =
                    counts.is_some_and(|c| c.present.bounds().is_some_and(|(_, upper)| upper == 0));
                if !empty && !absent {
                    continue;
                }
                // A rejecting filter already explains the same empty quantifier.
                if findings.iter().any(|f| contains(&location, &f.location)) {
                    continue;
                }
                let proof = if empty {
                    "no selected iterations"
                } else {
                    "no present Boolean elements (array capacity is distinct)"
                };
                let identity = if name == "forall" { "true" } else { "false" };
                findings.push(finding(location.clone(),format!("{name} has {proof}; its empty Boolean identity is {identity}, with input evaluation proved defined{}",context_message(context,&value.context,&location))));
            }
            for n in &nodes {
                let candidate = match n.kind() {
                    NodeKind::Constraint => n
                        .child_nodes()
                        .find(|c| is_expression(c.kind()))
                        .map(|c| (c, true)),
                    NodeKind::ConditionalBranch => n.child_nodes().next().map(|c| (c, false)),
                    _ => None,
                };
                let Some((condition, constraint)) = candidate else {
                    continue;
                };
                let location = source.location(condition.range());
                let Some(value) = guarded.expression(file, &location) else {
                    continue;
                };
                if value.context.activation == GuardActivation::Inactive {
                    continue;
                }
                if let GuardedOutcome::Unsupported(reason) = &value.definedness {
                    limitations.push(limitation(location, reason));
                    continue;
                }
                if value.definedness != GuardedOutcome::Proven {
                    if guarded.obligations.iter().any(|o| {
                        o.file == file
                            && matches!(o.kind, GuardObligationKind::Assertion)
                            && o.outcome == GuardedOutcome::Refuted
                            && contains(&location, &o.operation)
                    }) {
                        limitations.push(limitation(location,"assertion evaluation can abort; no total Boolean identity is established"));
                    }
                    continue;
                }
                let truth = match &value.truth {
                    Some(GuardedOutcome::Proven) if constraint => "true",
                    Some(GuardedOutcome::Refuted) => "false",
                    Some(GuardedOutcome::Unsupported(reason)) => {
                        limitations.push(limitation(location, reason));
                        continue;
                    }
                    _ => continue,
                };
                if quantifier_name(context, file, unwrap(condition)).is_some()
                    && findings.iter().any(|f| contains(&location, &f.location))
                {
                    continue;
                }
                let description = if constraint {
                    "constraint"
                } else {
                    "condition"
                };
                let proof = if value.raw_definedness == GuardedOutcome::Refuted {
                    "an operand's definedness is refuted; its undefinedness becomes false at the nearest Boolean boundary"
                } else {
                    "the interpreted Boolean value is proved constant"
                };
                findings.push(finding(location.clone(),format!("{description} is {truth}: {proof}{}; this identity does not authorize removal or reordering",context_message(context,&value.context,&location))));
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
fn collect<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxNode>) {
    out.push(node);
    for n in node.child_nodes() {
        collect(n, out);
    }
}
fn contains(a: &SourceLocation, b: &SourceLocation) -> bool {
    a.path == b.path && a.range.start <= b.range.start && b.range.end <= a.range.end
}
fn quantifier_name(
    context: &ModelContext,
    file: FileId,
    node: &SyntaxNode,
) -> Option<&'static str> {
    if !matches!(
        node.kind(),
        NodeKind::CallExpression | NodeKind::GeneratorCallExpression
    ) {
        return None;
    }
    let parsed = &context.files[file].parsed;
    let t = *tokens(parsed, node).first()?;
    match parsed.source()[t.range.clone()].trim_matches('\'') {
        "forall" => Some("forall"),
        "exists" => Some("exists"),
        _ => None,
    }
}
fn context_message(
    context: &ModelContext,
    scope: &GuardContext,
    location: &SourceLocation,
) -> String {
    let mut message = if scope.activation == GuardActivation::Conditional {
        "; evaluation is conditional on the enclosing selection".into()
    } else {
        String::new()
    };
    for a in scope.assumptions.iter().filter(|a| {
        matches!(a.kind, GuardAssumptionKind::Condition { .. }) && !contains(&a.location, location)
    }) {
        let GuardAssumptionKind::Condition { expected } = a.kind else {
            unreachable!()
        };
        let source = &context.files[a.file];
        let range =
            a.location.range.start - source.byte_offset..a.location.range.end - source.byte_offset;
        message.push_str(&format!(
            "; context includes {}condition '{}' = {expected} at {}:{}:{}",
            if a.global { "enforced " } else { "local " },
            source.parsed.source()[range].trim(),
            a.location.path.display(),
            a.location.line,
            a.location.column
        ));
    }
    message
}
fn finding(location: SourceLocation, message: String) -> FileFinding {
    FileFinding {
        fix: None,
        location,
        rule: Rule::VacuousConstraint,
        severity: Severity::Warning,
        message,
    }
}
fn limitation(location: SourceLocation, reason: &str) -> SourceDiagnostic {
    SourceDiagnostic {
        location,
        message: format!("vacuous-constraint: {reason}"),
    }
}

fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression | NodeKind::NamedArgument
    ) {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
