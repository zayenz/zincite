//! Potential expansion and bounded placement facts, separate from warning policy.
use crate::callables::core_operation;
use crate::definitions::{annotations_safe, resolved_reference};
use crate::guarded::{evaluate_iteration_prefix, node_at};
use crate::{
    BindingFacts, CallableFacts, CandidateCount, DeclarationId, DomainFacts, FileFinding, FileId,
    GuardActivation, GuardAssumptionKind, GuardObligation, GuardObligationKind, GuardedExpression,
    GuardedFacts, GuardedOutcome, Instantiation, InstantiationFacts, IterationExpression,
    IterationFact, IterationFacts, ModelContext, NumericFacts, OptionalFacts, Rule, Severity,
    SourceDiagnostic, SourceLocation, TypeKind,
};
use std::collections::BTreeSet;
use std::num::NonZeroU64;
use zincite_syntax::{NodeKind, SyntaxNode};

#[derive(Clone, Debug)]
pub enum IterationWork {
    Filter,
    Aggregate(String),
}
/// A lexical opportunity and its separate prospective evaluation proof.
/// A Proven result is conditional on the retained prefix/enclosing assumptions;
/// it is not permission to rewrite source or remove arithmetic multiplicity.
#[derive(Clone, Debug)]
pub struct IterationPlacement {
    pub location: SourceLocation,
    pub work: IterationWork,
    pub dependencies: Vec<DeclarationId>,
    pub earliest_scope: usize,
    pub crossed_bindings: Vec<DeclarationId>,
    /// First source is the prospective prefix anchor; order is retained.
    pub crossed_sources: Vec<SourceLocation>,
    pub safety: GuardedOutcome,
    pub prospective: Option<GuardedExpression>,
    pub obligations: Vec<GuardObligation>,
}
#[derive(Clone, Debug)]
pub struct ComprehensionStructureFact {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    /// Pre-filter potential candidates, not selected/present values or runtime.
    pub candidates: CandidateCount,
    pub placements: Vec<IterationPlacement>,
}
/// Call with the same-model prerequisites, in numeric→optional→guarded→iteration
/// order. No lint runs. Counts retain written slot multiplicity and symbolic
/// factors; threshold policy does not change them. Whole par filters and nested
/// resolved core aggregates have a separate bounded prefix-safety check, using
/// the existing guarded interpreter for that candidate only. Unknown data does
/// not prove repetition or safe movement; effects, assertions and semantic
/// annotations are excluded. No solver, compiler enumeration or edit is used.
#[allow(clippy::too_many_arguments)]
pub fn resolve_comprehension_structure(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
    optional: &OptionalFacts,
    guarded: &GuardedFacts,
    iteration: &IterationFacts,
) -> Vec<ComprehensionStructureFact> {
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        optional,
        guarded,
    };
    let mut facts = Vec::new();
    let mut placed = BTreeSet::new();
    for i in &iteration.iterations {
        let Some(g) = guarded.expression(i.file, &i.location) else {
            continue;
        };
        let mut placements = Vec::new();
        for filter in &i.filters {
            let Some(e) = i
                .expressions
                .iter()
                .find(|e| e.location.range == filter.location.range)
            else {
                continue;
            };
            let Some(current) = i
                .generators
                .iter()
                .find(|g| contains(&g.location, &e.location))
                .map(|g| g.position + 1)
            else {
                continue;
            };
            let parameter_boolean = calls
                .expressions
                .iter()
                .find(|t| t.file == i.file && t.location.range == e.location.range)
                .is_some_and(|t| !t.ty.optional && t.ty.kind == TypeKind::Bool)
                && instantiations
                    .expressions
                    .iter()
                    .find(|t| t.file == i.file && t.location.range == e.location.range)
                    .is_some_and(|t| t.instantiation == Instantiation::Parameter);
            if parameter_boolean
                && let Some(p) = producer.placement(i, e, current, IterationWork::Filter)
            {
                if p.safety == GuardedOutcome::Proven {
                    placed.insert((i.file, p.location.range.start, p.location.range.end));
                }
                placements.push(p);
            }
        }
        for e in &i.expressions {
            if !contains(&i.body, &e.location)
                || placed.contains(&(i.file, e.location.range.start, e.location.range.end))
            {
                continue;
            }
            let Some(node) = node_at(context, i.file, &e.location) else {
                continue;
            };
            if !matches!(
                node.kind(),
                NodeKind::GeneratorCallExpression | NodeKind::CallExpression
            ) {
                continue;
            }
            let Some(name) = ["sum", "product", "min", "max", "forall", "exists"]
                .into_iter()
                .find(|name| {
                    core_operation(context, bindings, calls, i.file, node, name) == Ok(true)
                })
            else {
                continue;
            };
            if let Some(p) = producer.placement(
                i,
                e,
                i.generators.len(),
                IterationWork::Aggregate(name.into()),
            ) {
                if p.safety == GuardedOutcome::Proven {
                    placed.insert((i.file, p.location.range.start, p.location.range.end));
                }
                placements.push(p);
            }
        }
        facts.push(ComprehensionStructureFact {
            file: i.file,
            item: g.item,
            location: i.location.clone(),
            candidates: i.candidates.clone(),
            placements,
        });
    }
    facts
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    numeric: &'a NumericFacts,
    optional: &'a OptionalFacts,
    guarded: &'a GuardedFacts,
}
impl Producer<'_> {
    fn placement(
        &self,
        i: &IterationFact,
        e: &IterationExpression,
        current: usize,
        work: IterationWork,
    ) -> Option<IterationPlacement> {
        let earliest = e.earliest_scope?;
        if earliest >= current {
            return None;
        }
        let crossed = &i.generators[earliest..current];
        // No exposure to an empty/unknown domain, assignment generator or a
        // decision set universe. A known singleton alone supplies no repetition.
        if crossed.iter().any(|g| {
            !g.membership
                || g.source_instantiation != Instantiation::Parameter
                || !g
                    .index_set
                    .cardinality
                    .bounds()
                    .is_some_and(|(lower, _)| lower > 0)
        }) || !crossed.iter().any(|g| {
            g.index_set
                .cardinality
                .bounds()
                .is_some_and(|(_, upper)| upper > 1)
        }) {
            return None;
        }
        let original = e.guarded.as_ref()?;
        let prefix = self.guarded.expression(i.file, &crossed[0].source)?;
        let mut p = IterationPlacement {
            location: e.location.clone(),
            work,
            dependencies: e.dependencies.clone(),
            earliest_scope: earliest,
            crossed_bindings: crossed
                .iter()
                .flat_map(|g| g.bindings.iter().copied())
                .collect(),
            crossed_sources: crossed.iter().map(|g| g.source.clone()).collect(),
            safety: GuardedOutcome::Unknown,
            prospective: None,
            obligations: vec![],
        };
        // Keep a genuinely enclosing conditional context, but do not expose work
        // skipped by a crossed filter or a body-local lazy branch.
        if original.context.activation == GuardActivation::Inactive
            || original.context.activation != prefix.context.activation
        {
            return Some(p);
        }
        // Two different unknown branches both have Conditional activation.
        // Any additional condition must already select this evaluation, or
        // moving it could expose work skipped inside the original body.
        if original.context.assumptions.iter().any(|a| {
            let GuardAssumptionKind::Condition { expected } = a.kind else {
                return false;
            };
            let retained = prefix.context.assumptions.iter().any(|b| {
                b.file == a.file
                    && b.location.range == a.location.range
                    && matches!(b.kind, GuardAssumptionKind::Condition { expected: other } if other == expected)
            });
            !retained
                && self
                    .guarded
                    .expression(a.file, &a.location)
                    .and_then(|g| g.truth.as_ref())
                    != Some(&if expected {
                        GuardedOutcome::Proven
                    } else {
                        GuardedOutcome::Refuted
                    })
        }) {
            return Some(p);
        }
        let Some(node) = node_at(self.context, i.file, &i.location) else {
            return Some(p);
        };
        if !self.annotations(i.file, node)
            || !self.enclosing_annotations(i.file, self.context.files[i.file].parsed.tree(), node)
        {
            p.safety =
                GuardedOutcome::Unsupported("semantic annotation prevents placement proof".into());
            return Some(p);
        }
        for g in crossed {
            let safety = self.original_safety(i.file, &g.source);
            if safety != GuardedOutcome::Proven {
                p.safety = safety;
                return Some(p);
            }
        }
        for filter in &i.filters {
            if filter.location.range.start >= crossed[0].source.range.start
                && filter.location.range.start < e.location.range.start
            {
                let safety = self.original_safety(i.file, &filter.location);
                if safety != GuardedOutcome::Proven {
                    p.safety = safety;
                    return Some(p);
                }
            }
        }
        if matches!(p.work, IterationWork::Aggregate(_)) {
            let safety = self.original_safety(i.file, &i.body);
            if safety != GuardedOutcome::Proven {
                p.safety = safety;
                return Some(p);
            }
        }
        match evaluate_iteration_prefix(
            self.context,
            self.bindings,
            self.calls,
            self.instantiations,
            self.domains,
            self.numeric,
            self.optional,
            original,
            &prefix.context,
        ) {
            Ok(facts) => {
                p.safety = self.safety(&facts, i.file, &e.location);
                p.prospective = facts.expression(i.file, &e.location).cloned();
                p.obligations = facts.obligations;
            }
            Err(reason) => p.safety = GuardedOutcome::Unsupported(reason),
        }
        Some(p)
    }
    fn annotations(&self, file: FileId, node: &SyntaxNode) -> bool {
        annotations_safe(self.context, file, node)
            && node.child_nodes().all(|c| self.annotations(file, c))
    }
    fn enclosing_annotations(&self, file: FileId, node: &SyntaxNode, target: &SyntaxNode) -> bool {
        if node.range().start > target.range().start || node.range().end < target.range().end {
            return true;
        }
        annotations_safe(self.context, file, node)
            && node
                .child_nodes()
                .all(|child| self.enclosing_annotations(file, child, target))
    }
    fn original_safety(&self, file: FileId, location: &SourceLocation) -> GuardedOutcome {
        self.safety(self.guarded, file, location)
    }
    fn safety(
        &self,
        facts: &GuardedFacts,
        file: FileId,
        location: &SourceLocation,
    ) -> GuardedOutcome {
        let Some(root) = facts.expression(file, location) else {
            return GuardedOutcome::Unknown;
        };
        if root.context.activation == GuardActivation::Inactive {
            return GuardedOutcome::Unknown;
        }
        if let crate::DefinitionEnforcement::Unsupported(reason) = &root.context.enforcement {
            return GuardedOutcome::Unsupported(reason.clone());
        }
        if let Some(node) = node_at(self.context, file, location)
            && !self.annotations(file, node)
        {
            return GuardedOutcome::Unsupported(
                "semantic annotation prevents placement proof".into(),
            );
        }
        for o in facts
            .obligations
            .iter()
            .filter(|o| o.file == file && contains(location, &o.operation))
        {
            if matches!(o.kind, GuardObligationKind::Assertion) {
                return GuardedOutcome::Unsupported(
                    "assertion evaluation is excluded from placement advice".into(),
                );
            }
            if o.context.activation != GuardActivation::Inactive
                && o.outcome != GuardedOutcome::Proven
            {
                return o.outcome.clone();
            }
        }
        for g in facts
            .expressions
            .iter()
            .filter(|g| g.file == file && contains(location, &g.location))
        {
            if let Some(node) = node_at(self.context, file, &g.location) {
                if matches!(
                    node.kind(),
                    NodeKind::CallExpression | NodeKind::GeneratorCallExpression
                ) {
                    let name = crate::domains::tokens(&self.context.files[file].parsed, node)
                        .first()
                        .map(|t| {
                            self.context.files[file].parsed.source()[t.range.clone()]
                                .trim_matches('\'')
                        });
                    if !name.is_some_and(|name| {
                        core_operation(self.context, self.bindings, self.calls, file, node, name)
                            == Ok(true)
                    }) {
                        return GuardedOutcome::Unsupported(
                            "non-core callable evaluation is excluded from placement advice".into(),
                        );
                    }
                } else if node.kind() == NodeKind::Expression
                    && let Some(id) = resolved_reference(self.context, self.bindings, file, node)
                    && self.bindings.declarations[id.0].role == crate::DeclarationRole::Local
                {
                    let safety = self.alias_safety(id, &mut BTreeSet::new());
                    if safety != GuardedOutcome::Proven {
                        return safety;
                    }
                }
            }
            // Inactive nested operations must remain inactive; only their
            // originally reachable raw evaluations need to be total.
            if g.context.activation != GuardActivation::Inactive
                && g.raw_definedness != GuardedOutcome::Proven
            {
                return g.raw_definedness.clone();
            }
        }
        GuardedOutcome::Proven
    }
    // An enclosing immutable alias remains at its original declaration. Its
    // already evaluated initializer must nevertheless have a total value;
    // a typed identifier alone is not evidence for a partial alias chain.
    fn alias_safety(&self, id: DeclarationId, active: &mut BTreeSet<usize>) -> GuardedOutcome {
        if !active.insert(id.0) {
            return GuardedOutcome::Unknown;
        }
        let d = &self.bindings.declarations[id.0];
        let source = &self.context.files[d.file];
        let result = node_at(
            self.context,
            d.file,
            &source.location(d.syntax_range.clone()),
        )
        .and_then(|n| {
            n.child_nodes()
                .find(|n| crate::callables::is_expression(n.kind()))
        })
        .and_then(|n| self.guarded.expression(d.file, &source.location(n.range())))
        .map_or(GuardedOutcome::Unknown, |g| {
            if g.raw_definedness != GuardedOutcome::Proven {
                return g.raw_definedness.clone();
            }
            for r in &self.bindings.references {
                if r.file == d.file
                    && contains(&g.location, &r.location)
                    && let crate::BindingResolution::Resolved(other) = r.resolution
                    && self.bindings.declarations[other.0].role == crate::DeclarationRole::Local
                {
                    let safety = self.alias_safety(other, active);
                    if safety != GuardedOutcome::Proven {
                        return safety;
                    }
                }
            }
            GuardedOutcome::Proven
        });
        active.remove(&id.0);
        result
    }
}
fn contains(outer: &SourceLocation, inner: &SourceLocation) -> bool {
    outer.path == inner.path
        && outer.range.start <= inner.range.start
        && inner.range.end <= outer.range.end
}
pub(super) fn check_comprehensions(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &[ComprehensionStructureFact],
    threshold: NonZeroU64,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for f in facts {
        let file = &context.files[f.file];
        if !file.warnings_enabled()
            || file
                .suppressions
                .as_ref()
                .is_none_or(|s| s[f.item].contains(&Rule::ExpensiveComprehension))
        {
            continue;
        }
        let message = match &f.candidates {
            CandidateCount::Exact(n) if *n > threshold.get() => Some(format!(
                "pre-filter Cartesian candidate count {n} exceeds max-candidates {}; potential expansion differs from selected/present elements",
                threshold
            )),
            CandidateCount::UpperBound(n) if *n > threshold.get() => Some(format!(
                "pre-filter candidate upper bound {n} exceeds max-candidates {}; this may describe a decision-set universe, not selected/present elements",
                threshold
            )),
            CandidateCount::Symbolic {
                coefficient,
                factors,
                upper_bound,
            } if factors.len() > 1 || *coefficient > 1 => {
                let dimensions: Vec<_> = factors
                    .iter()
                    .map(|d| {
                        let s = &context.files[d.file];
                        let range = d.location.range.start - s.byte_offset
                            ..d.location.range.end - s.byte_offset;
                        let deps = names(bindings, &d.dependencies);
                        format!(
                            "{} (slot {}, dependencies: {})",
                            s.parsed.source()[range].trim(),
                            d.slot + 1,
                            deps
                        )
                    })
                    .collect();
                Some(format!(
                    "symbolic Cartesian {}structure: coefficient {coefficient}, ordered dimensions [{}]; no numeric count or threshold exceedance is established",
                    if *upper_bound { "upper-bound " } else { "" },
                    dimensions.join("; ")
                ))
            }
            CandidateCount::Unsupported(reason) => {
                limitations.push(SourceDiagnostic {
                    location: f.location.clone(),
                    message: format!("expensive-comprehension: {reason}"),
                });
                None
            }
            _ => None,
        };
        if let Some(message) = message {
            findings.push(finding(&f.location, message));
        }
        for p in &f.placements {
            match &p.safety {
                GuardedOutcome::Proven => {
                    let kind = match &p.work {
                        IterationWork::Filter => "whole parameter filter".to_owned(),
                        IterationWork::Aggregate(name) => format!("core {name} aggregate"),
                    };
                    let sources = p
                        .crossed_sources
                        .iter()
                        .map(|s| source_text(context, f.file, s))
                        .collect::<Vec<_>>()
                        .join(", ");
                    findings.push(finding(
                        &p.location,
                        format!(
                            "{kind} uses [{}] and is invariant across repeated generator sources [{sources}] with crossed bindings [{}]; raw evaluation is proved total after generator prefix {} under retained enclosing/prefix assumptions; consider that earlier scope",
                            names(bindings, &p.dependencies),
                            names(bindings, &p.crossed_bindings),
                            p.earliest_scope
                        ),
                    ));
                }
                GuardedOutcome::Unsupported(reason) => limitations.push(SourceDiagnostic {
                    location: p.location.clone(),
                    message: format!("expensive-comprehension: {reason}"),
                }),
                _ => {}
            }
        }
    }
    let mut seen = BTreeSet::new();
    limitations.retain(|l| {
        seen.insert((
            l.location.path.clone(),
            l.location.range.start,
            l.location.range.end,
            l.message.clone(),
        ))
    });
    (findings, limitations)
}
fn source_text<'a>(context: &'a ModelContext, file: FileId, location: &SourceLocation) -> &'a str {
    let s = &context.files[file];
    s.parsed.source()[location.range.start - s.byte_offset..location.range.end - s.byte_offset]
        .trim()
}
fn names(bindings: &BindingFacts, ids: &[DeclarationId]) -> String {
    let names = ids
        .iter()
        .filter(|id| {
            !matches!(
                bindings.declarations[id.0].role,
                crate::DeclarationRole::Function
                    | crate::DeclarationRole::Predicate
                    | crate::DeclarationRole::Test
                    | crate::DeclarationRole::Annotation
            )
        })
        .map(|id| bindings.declarations[id.0].name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    if names.is_empty() {
        "none".into()
    } else {
        names
    }
}
fn finding(location: &SourceLocation, message: String) -> FileFinding {
    FileFinding {
        location: location.clone(),
        rule: Rule::ExpensiveComprehension,
        severity: Severity::Warning,
        message: format!(
            "{message}; the compiler may already optimize this structure; this is not a measured enumeration, runtime or speedup claim and supplies no source edit"
        ),
    }
}
