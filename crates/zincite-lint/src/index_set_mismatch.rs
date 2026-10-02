//! Diagnostic policy over interpreted index spaces; no condition interpretation.
use crate::{
    BindingFacts, CandidateCount, Cardinality, Domain, FileFinding, FileId, GuardActivation,
    GuardObligationKind, GuardedFacts, GuardedOutcome, IterationCoverage, IterationFacts,
    IterationIndexSet, ModelContext, NumericBound, Rule, Severity, SourceDiagnostic,
    SourceLocation,
};
use zincite_syntax::{NodeKind, SyntaxNode};

pub(super) fn check_index_set_mismatches(
    context: &ModelContext,
    bindings: &BindingFacts,
    guarded: &GuardedFacts,
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
            if suppressions[item].contains(&Rule::IndexSetMismatch) {
                continue;
            }
            let mut accesses = Vec::new();
            collect_accesses(node, &mut accesses);
            for access in accesses {
                let location = source.location(access.range());
                if guarded
                    .expression(file, &location)
                    .is_some_and(|e| e.context.activation == GuardActivation::Inactive)
                    || iteration.iterations.iter().any(|i| {
                        i.file == file
                            && contains(&i.location, &location)
                            && i.coverage == IterationCoverage::Empty
                    })
                {
                    continue;
                }
                let obligations: Vec<_> = guarded
                    .obligations_at(file, &location)
                    .filter(|o| matches!(o.kind, GuardObligationKind::Index { .. }))
                    .collect();
                if obligations.is_empty() {
                    let reason = guarded
                        .limitations
                        .iter()
                        .find(|l| l.file == file && l.location.range == location.range)
                        .map(|l| l.reason.as_str())
                        .unwrap_or("required array/index identity or slice form is unsupported");
                    limitations.push(limitation(location, reason));
                    continue;
                }
                let mut reported = Vec::new();
                for obligation in obligations {
                    let GuardObligationKind::Index {
                        array,
                        dimension,
                        selection,
                        ..
                    } = &obligation.kind
                    else {
                        continue;
                    };
                    if reported.contains(dimension) || obligation.outcome == GuardedOutcome::Proven
                    {
                        continue;
                    }
                    if let GuardedOutcome::Unsupported(reason) = &obligation.outcome {
                        limitations.push(limitation(location.clone(), reason));
                        reported.push(*dimension);
                        continue;
                    }
                    let Some(target) = iteration
                        .arrays
                        .iter()
                        .find(|a| a.declaration == *array)
                        .and_then(|a| a.dimensions.get(dimension - 1))
                    else {
                        limitations.push(limitation(
                            location.clone(),
                            "normalized array dimension is unavailable",
                        ));
                        continue;
                    };
                    let candidate = selection.as_ref().map(|s| IterationIndexSet {
                        file,
                        location: obligation.operand.clone(),
                        domain: s.domain.clone(),
                        universe: None,
                        cardinality: Cardinality::Unknown,
                        array_dimension: None,
                    });
                    let enum_mismatch = candidate.as_ref().is_some_and(|c| {
                        matches!((bare(&c.domain), bare(&target.domain)),
                            (Domain::Enum(a), Domain::Enum(b)) if a != b)
                    });
                    let refuted = obligation.outcome == GuardedOutcome::Refuted;
                    let coarse = selection.as_ref().is_some_and(|s| s.exact)
                        && exact_traversals(iteration, file, &location)
                        && candidate
                            .as_ref()
                            .is_some_and(|c| c.subset_of(target) == GuardedOutcome::Refuted);
                    if enum_mismatch || refuted || coarse {
                        let declaration = &bindings.declarations[array.0];
                        let space = candidate
                            .as_ref()
                            .map(|c| describe(&c.domain, bindings))
                            .unwrap_or_else(|| text(context, file, &obligation.operand).to_owned());
                        let concern = if enum_mismatch {
                            "has an incompatible enum index type"
                        } else if refuted
                            || candidate
                                .as_ref()
                                .is_some_and(|c| c.disjoint_from(target) == GuardedOutcome::Proven)
                        {
                            "is wholly outside the index set"
                        } else {
                            "has some candidate indices outside the index set"
                        };
                        findings.push(FileFinding {
                            location: location.clone(),
                            rule: Rule::IndexSetMismatch,
                            severity: Severity::Warning,
                            message: format!(
                                "index space {space} {concern} for array '{}' dimension {} ({}, declared at {}:{}:{})",
                                declaration.name, dimension, describe(&target.domain, bindings),
                                declaration.location.path.display(), declaration.location.line,
                                declaration.location.column,
                            ),
                        });
                        reported.push(*dimension);
                    } else {
                        for l in guarded.limitations.iter().filter(|l| {
                            l.file == file && contains(&obligation.operand, &l.location)
                        }) {
                            limitations.push(limitation(l.location.clone(), &l.reason));
                        }
                        if let Some(c) = &candidate
                            && let GuardedOutcome::Unsupported(reason) = c.subset_of(target)
                        {
                            limitations.push(limitation(location.clone(), &reason));
                        }
                    }
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
fn collect_accesses<'a>(node: &'a SyntaxNode, out: &mut Vec<&'a SyntaxNode>) {
    if node.kind() == NodeKind::ArrayAccessExpression {
        out.push(node);
    }
    for child in node.child_nodes() {
        collect_accesses(child, out);
    }
}
fn contains(a: &SourceLocation, b: &SourceLocation) -> bool {
    a.range.start <= b.range.start && b.range.end <= a.range.end
}
fn exact_traversals(facts: &IterationFacts, file: FileId, location: &SourceLocation) -> bool {
    facts
        .iterations
        .iter()
        .filter(|i| i.file == file && contains(&i.location, location))
        .all(|i| {
            matches!(i.candidates,CandidateCount::Exact(n) if n>0)
                && matches!(i.selected,CandidateCount::Exact(n) if n>0)
                && matches!(
                    i.coverage,
                    IterationCoverage::Full | IterationCoverage::ProperPartial
                )
        })
}
fn bare(mut d: &Domain) -> &Domain {
    while let Domain::Named { domain, .. } = d {
        d = domain;
    }
    d
}
fn describe(domain: &Domain, bindings: &BindingFacts) -> String {
    match domain {
        Domain::Named { declaration, .. } | Domain::Enum(declaration) => {
            format!("'{}'", bindings.declarations[declaration.0].name)
        }
        Domain::Range { lower, upper } => {
            format!("{}..{}", bound(lower, bindings), bound(upper, bindings))
        }
        Domain::LiteralSet(v) => format!(
            "{{{}}}",
            v.iter()
                .map(|v| bound(v, bindings))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => "unknown membership".into(),
    }
}
fn bound(b: &NumericBound, bindings: &BindingFacts) -> String {
    match b {
        NumericBound::Integer(n) => n.to_string(),
        NumericBound::Symbol(d) | NumericBound::Defined { declaration: d, .. } => {
            bindings.declarations[d.0].name.clone()
        }
        _ => "symbolic bound".into(),
    }
}
fn text<'a>(context: &'a ModelContext, file: FileId, location: &SourceLocation) -> &'a str {
    let s = &context.files[file];
    &s.parsed.source()[location.range.start - s.byte_offset..location.range.end - s.byte_offset]
}
fn limitation(location: SourceLocation, reason: &str) -> SourceDiagnostic {
    SourceDiagnostic {
        location,
        message: format!("index-set-mismatch: {reason}"),
    }
}
