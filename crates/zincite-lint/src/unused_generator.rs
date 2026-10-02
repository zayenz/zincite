//! Binding-use projection and separate advisory policy over existing iteration facts.
use crate::{
    BindingFacts, BindingResolution, Cardinality, DeclarationId, FileFinding, GuardObligation,
    GuardedFacts, GuardedOutcome, IterationFacts, IterationMultiplicity, ModelContext, Rule,
    Severity, SourceDiagnostic, SourceLocation,
};

/// Usage and retained evaluation requirements for one generator declaration.
/// Identities and locations belong to the same model as the supplied facts.
#[derive(Clone, Debug)]
pub struct GeneratorBindingUsage {
    pub binding: DeclarationId,
    pub iteration: SourceLocation,
    pub used: bool,
    /// False when a same-name unresolved/ambiguous reference could use this binding.
    pub usage_complete: bool,
    pub multiplicity: IterationMultiplicity,
    /// Exact source cardinality above one; potential repetition, not execution.
    pub repetitions: Option<u64>,
    pub obligations: Vec<GuardObligation>,
    pub evaluation_definedness: Vec<(SourceLocation, GuardedOutcome)>,
    /// Conservative semantic candidate eligibility. The fix consumer separately
    /// checks comprehension syntax and preserves every generator, domain, filter,
    /// body, annotation and evaluation. Unknown/unsupported safety or assignment
    /// syntax withholds this flag; that does not prove anonymization is unsafe.
    pub anonymization_eligible: bool,
}

/// Project complete-chain lexical uses and existing guarded evidence without
/// evaluating expressions or running a rule. Same-spelling nested declarations
/// remain distinct; body, later domains/filters and annotations count as uses.
pub fn resolve_generator_binding_usage(
    bindings: &BindingFacts,
    guarded: &GuardedFacts,
    iteration: &IterationFacts,
) -> Vec<GeneratorBindingUsage> {
    let mut result = Vec::new();
    for fact in &iteration.iterations {
        let obligations: Vec<_> = guarded
            .obligations
            .iter()
            .filter(|o| o.file == fact.file && contains(&fact.location, &o.operation))
            .cloned()
            .collect();
        let evaluation_definedness: Vec<_> = fact
            .expressions
            .iter()
            .map(|e| {
                (
                    e.location.clone(),
                    e.guarded
                        .as_ref()
                        .map(|g| g.raw_definedness.clone())
                        .unwrap_or(GuardedOutcome::Unknown),
                )
            })
            .collect();
        for generator in &fact.generators {
            for &binding in &generator.bindings {
                let declaration = &bindings.declarations[binding.0];
                let used = fact.uses.iter().any(|u| u.binding == binding);
                let usage_complete = !bindings.references.iter().any(|r| {
                    r.file == fact.file
                        && contains(&fact.location, &r.location)
                        && r.name == declaration.name
                        && match &r.resolution {
                            BindingResolution::Unresolved => true,
                            BindingResolution::Ambiguous(ids)
                            | BindingResolution::Overloads(ids) => ids.contains(&binding),
                            BindingResolution::Resolved(_) => false,
                        }
                });
                let repetitions = match generator.index_set.cardinality {
                    Cardinality::Exact(count) if generator.membership && count > 1 => Some(count),
                    _ => None,
                };
                let anonymization_eligible = !used
                    && usage_complete
                    && generator.membership
                    && evaluation_definedness
                        .iter()
                        .all(|(_, definedness)| *definedness == GuardedOutcome::Proven)
                    && obligations
                        .iter()
                        .all(|o| o.outcome == GuardedOutcome::Proven);
                result.push(GeneratorBindingUsage {
                    binding,
                    iteration: fact.location.clone(),
                    used,
                    usage_complete,
                    multiplicity: fact.multiplicity,
                    repetitions,
                    obligations: obligations.clone(),
                    evaluation_definedness: evaluation_definedness.clone(),
                    anonymization_eligible,
                });
            }
        }
    }
    result
}

pub(super) fn check_unused_generators(
    context: &ModelContext,
    bindings: &BindingFacts,
    usage: &[GeneratorBindingUsage],
    snapshots: &mut std::collections::BTreeMap<crate::FileId, crate::SourceSnapshot>,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for fact in usage {
        let binding = &bindings.declarations[fact.binding.0];
        if fact.used || binding.name.starts_with('_') {
            continue;
        }
        let file = &context.files[binding.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled()
            || suppressions[binding.item].contains(&Rule::UnusedGeneratorBinding)
        {
            continue;
        }
        if !fact.usage_complete {
            limitations.push(SourceDiagnostic {
                location: binding.location.clone(),
                message: format!(
                    "unused-generator-binding: use of '{}' cannot be resolved completely",
                    binding.name
                ),
            });
            continue;
        }
        let mut message = format!(
            "generator binding '{}' is unused in the remaining domains, filters, body and annotations",
            binding.name
        );
        let repetition = fact.repetitions.map(|n| format!("; its source has {n} values, so each remaining value can repeat {n} times if those candidates are selected")).unwrap_or_default();
        match fact.multiplicity {
            IterationMultiplicity::IdempotentQuantifier => {
                message.push_str(&repetition);
                message.push_str("; repeated Boolean values are idempotent in this core quantifier, but this does not authorize dropping evaluation");
            }
            IterationMultiplicity::Arithmetic => {
                message.push_str(&repetition);
                message.push_str("; sum/product retains repeated terms or factors, so an unused index does not make iterations removable");
            }
            IterationMultiplicity::Collection => message
                .push_str("; preserve collection membership, positions and multiplicity, including repeated sum terms or product factors when the collection is aggregated"),
            IterationMultiplicity::Unknown => message.push_str(
                "; aggregate semantics are not established, so no repetition identity is claimed",
            ),
        }
        message.push_str("; preserve every generator, source, filter, body, annotation and definedness requirement");
        if fact
            .obligations
            .iter()
            .any(|o| o.outcome == GuardedOutcome::Refuted)
        {
            message.push_str("; a retained operation precondition is refuted under its scoped assumptions, without proving that the operation is evaluated");
        } else if fact
            .evaluation_definedness
            .iter()
            .any(|(_, d)| *d == GuardedOutcome::Unknown)
        {
            message.push_str("; some retained definedness facts are unknown");
        }
        for (_, definedness) in &fact.evaluation_definedness {
            if let GuardedOutcome::Unsupported(reason) = definedness {
                let message = format!(
                    "unused-generator-binding: retained evaluation safety is unsupported: {reason}"
                );
                if !limitations
                    .iter()
                    .any(|d| d.location == binding.location && d.message == message)
                {
                    limitations.push(SourceDiagnostic {
                        location: binding.location.clone(),
                        message,
                    });
                }
            }
        }
        findings.push(FileFinding {
            fix: if fact.anonymization_eligible
                && comprehension_binder(context, binding, &fact.iteration)
            {
                Some(crate::semantic_fixes::unused_name(
                    crate::semantic_fixes::snapshot(context, binding.file, snapshots),
                    &binding.location,
                ))
            } else {
                None
            },
            location: binding.location.clone(),
            rule: Rule::UnusedGeneratorBinding,
            severity: Severity::Warning,
            message,
        });
    }
    (findings, limitations)
}

fn contains(outer: &SourceLocation, inner: &SourceLocation) -> bool {
    outer.range.start <= inner.range.start && inner.range.end <= outer.range.end
}

fn comprehension_binder(
    context: &ModelContext,
    binding: &crate::Declaration,
    iteration: &SourceLocation,
) -> bool {
    let Some(node) = crate::guarded::node_at(context, binding.file, iteration) else {
        return false;
    };
    matches!(
        node.kind(),
        zincite_syntax::NodeKind::ArrayComprehension
            | zincite_syntax::NodeKind::SetComprehension
            | zincite_syntax::NodeKind::IndexedArrayComprehension
    ) && node
        .child_nodes()
        .filter(|n| n.kind() == zincite_syntax::NodeKind::GeneratorList)
        .flat_map(|list| list.child_nodes())
        .any(|generator| generator.range() == binding.syntax_range)
}
