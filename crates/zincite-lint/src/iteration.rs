//! Index membership, lexical use and bounded expansion, independent of lint policy.
use crate::callables::{array_concatenation, core_operation, find_node, is_expression};
use crate::definitions::{core_callable, resolved_call};
use crate::domains::{
    bare_index_domain as bare, core_arithmetic, expression_domain, generator_slots,
    index_domain_interval as interval, index_domain_member as member,
    intersect_index_domains as intersection, invariant_integer, resolved_index_domain,
    same_members, shift_index_domain as shifted, tokens,
};
use crate::{
    BindingFacts, BindingResolution, CallableFacts, Cardinality, CollectionCardinality,
    DeclarationId, DeclarationRole, Domain, DomainFacts, FileId, GuardObligation,
    GuardObligationKind, GuardedExpression, GuardedFacts, GuardedOutcome, Instantiation,
    InstantiationFacts, ModelContext, NumericBound, NumericFacts, NumericOutcome, OptionalFacts,
    SourceLocation, TypeKind,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

/// Actual membership, distinct from a decision set's possible universe.
/// Array dimensions stay in written order and enum IDs remain distinct types.
#[derive(Clone, Debug)]
pub struct IterationIndexSet {
    pub file: FileId,
    pub location: SourceLocation,
    pub domain: Domain,
    pub universe: Option<Domain>,
    pub cardinality: Cardinality,
    /// The resolved array and one-based dimension for a standard index helper.
    pub array_dimension: Option<(DeclarationId, usize)>,
}
impl IterationIndexSet {
    pub fn equal_members(&self, other: &Self) -> GuardedOutcome {
        match same_members(&self.domain, &other.domain) {
            Ok(Some(true)) => GuardedOutcome::Proven,
            Ok(Some(false)) => GuardedOutcome::Refuted,
            Ok(None) => GuardedOutcome::Unknown,
            Err(reason) => GuardedOutcome::Unsupported(reason),
        }
    }
    /// Every member of self belongs to other. Counts alone never prove this.
    pub fn subset_of(&self, other: &Self) -> GuardedOutcome {
        relation(&self.domain, &other.domain, false)
    }
    pub fn disjoint_from(&self, other: &Self) -> GuardedOutcome {
        relation(&self.domain, &other.domain, true)
    }
    /// Coverage of the target by self; an unknown overlap remains Unknown.
    pub fn coverage_of(&self, target: &Self) -> IterationCoverage {
        if self.cardinality == Cardinality::Exact(0) {
            return IterationCoverage::Empty;
        }
        if target.subset_of(self) == GuardedOutcome::Proven {
            return IterationCoverage::Full;
        }
        if self.disjoint_from(target) == GuardedOutcome::Proven {
            return IterationCoverage::Empty;
        }
        if target.subset_of(self) == GuardedOutcome::Refuted
            && intersection(&self.domain, &target.domain)
                .is_some_and(|d| closed_count(&d).bounds().is_some_and(|(l, _)| l > 0))
        {
            return IterationCoverage::ProperPartial;
        }
        if self.subset_of(target) == GuardedOutcome::Proven
            && self.equal_members(target) == GuardedOutcome::Refuted
        {
            return IterationCoverage::ProperPartial;
        }
        IterationCoverage::Unknown
    }
}
#[derive(Clone, Debug)]
pub struct IterationArray {
    pub declaration: DeclarationId,
    pub dimensions: Vec<IterationIndexSet>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterationCoverage {
    Empty,
    Full,
    ProperPartial,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterationMultiplicity {
    Collection,
    /// Repeating a Boolean value is idempotent, without authorizing a rewrite.
    IdempotentQuantifier,
    /// Repeated terms/factors affect sum/product even when a binder is unused.
    Arithmetic,
    Unknown,
}
/// A Cartesian factor, in generator/slot order. The source domain, slot and
/// dependencies retain the formula; a dependency list alone is not a formula.
#[derive(Clone, Debug)]
pub struct CandidateFactor {
    pub file: FileId,
    pub location: SourceLocation,
    pub slot: usize,
    pub domain: Domain,
    pub dependencies: Vec<DeclarationId>,
}
/// Exact counts or upper bounds use checked u64 products, without enumeration.
/// Symbolic factors repeat for each written slot and retain a known coefficient.
#[derive(Clone, Debug)]
pub enum CandidateCount {
    Exact(u64),
    UpperBound(u64),
    Symbolic {
        coefficient: u64,
        factors: Vec<CandidateFactor>,
        upper_bound: bool,
    },
    Unknown,
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct IterationGenerator {
    pub location: SourceLocation,
    pub source: SourceLocation,
    pub position: usize,
    pub membership: bool,
    pub source_instantiation: Instantiation,
    pub slots: usize,
    pub bindings: Vec<DeclarationId>,
    pub index_set: IterationIndexSet,
    pub dependencies: Vec<DeclarationId>,
    /// A proved bound for a source depending on earlier binders, not its count.
    pub uniform_upper_bound: Option<u64>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IterationUseRegion {
    Body,
    Source(usize),
    Filter(usize),
}
#[derive(Clone, Debug)]
pub struct IterationBindingUse {
    pub binding: DeclarationId,
    pub location: SourceLocation,
    pub region: IterationUseRegion,
    pub annotation: bool,
}
#[derive(Clone, Debug)]
pub struct IterationDependency {
    pub location: SourceLocation,
    pub resolution: BindingResolution,
}
#[derive(Clone, Debug)]
pub struct IterationExpression {
    pub file: FileId,
    pub location: SourceLocation,
    pub dependencies: Vec<DeclarationId>,
    pub unresolved: Vec<IterationDependency>,
    /// Used generator bindings from this iteration and its enclosing context.
    pub iteration_dependencies: Vec<DeclarationId>,
    /// Independence from these bindings does not imply totality or safe hoisting.
    pub invariant_with_respect_to: Vec<DeclarationId>,
    /// Zero is before this iteration's generators, with enclosing bindings
    /// already available; p+1 is after generator p. None means an
    /// unresolved or out-of-scope dependency prevents a lexical placement proof.
    pub earliest_scope: Option<usize>,
    pub guarded: Option<GuardedExpression>,
    pub obligations: Vec<GuardObligation>,
}
/// A filter's own interpreted evidence, separate from inherited emptiness.
/// Membership intersections apply only to the supported scalar binding subset.
#[derive(Clone, Debug)]
pub struct IterationFilter {
    pub location: SourceLocation,
    pub truth: GuardedOutcome,
    pub definedness: GuardedOutcome,
    pub incoming_empty: bool,
    pub prior_domain: Option<Domain>,
    pub selected_domain: Option<Domain>,
    /// Coverage supplied by this filter; Unknown does not prove rejection.
    pub coverage: IterationCoverage,
}
#[derive(Clone, Debug)]
pub struct IterationFact {
    pub file: FileId,
    pub location: SourceLocation,
    pub body: SourceLocation,
    pub generators: Vec<IterationGenerator>,
    pub filters: Vec<IterationFilter>,
    pub uses: Vec<IterationBindingUse>,
    pub expressions: Vec<IterationExpression>,
    /// Before filtering. Decision-set universes supply only an upper bound.
    pub candidates: CandidateCount,
    /// Selected iterations, distinct from optional capacity/present values.
    pub selected: CandidateCount,
    pub coverage: IterationCoverage,
    pub optional_collection: Option<CollectionCardinality>,
    pub multiplicity: IterationMultiplicity,
}
#[derive(Clone, Debug)]
pub struct IterationLimitation {
    pub file: FileId,
    pub location: SourceLocation,
    pub reason: String,
}
#[derive(Debug, Default)]
pub struct IterationFacts {
    pub arrays: Vec<IterationArray>,
    pub index_sets: Vec<IterationIndexSet>,
    pub iterations: Vec<IterationFact>,
    pub limitations: Vec<IterationLimitation>,
}
impl IterationFacts {
    pub fn iteration(&self, file: FileId, location: &SourceLocation) -> Option<&IterationFact> {
        self.iterations
            .iter()
            .find(|i| i.file == file && i.location.range == location.range)
    }
    /// Whether exact interpreted candidates fail to fit this index dimension.
    /// All enclosing traversals must have known positive candidate/selected
    /// counts and supported full/proper-partial coverage. This is membership
    /// evidence, not proof that a candidate occurs in a satisfiable model.
    /// The obligation and these facts must come from the same ModelContext.
    pub fn has_incompatible_index_candidates(&self, obligation: &GuardObligation) -> bool {
        let GuardObligationKind::Index {
            array,
            dimension,
            selection: Some(selection),
            ..
        } = &obligation.kind
        else {
            return false;
        };
        if !selection.exact
            || !self
                .iterations
                .iter()
                .filter(|i| {
                    i.file == obligation.file
                        && i.location.range.start <= obligation.operation.range.start
                        && obligation.operation.range.end <= i.location.range.end
                })
                .all(|i| {
                    matches!(i.candidates,CandidateCount::Exact(n) if n>0)
                        && matches!(i.selected,CandidateCount::Exact(n) if n>0)
                        && matches!(
                            i.coverage,
                            IterationCoverage::Full | IterationCoverage::ProperPartial
                        )
                })
        {
            return false;
        }
        let Some(target) = self
            .arrays
            .iter()
            .find(|a| a.declaration == *array)
            .and_then(|a| a.dimensions.get(dimension - 1))
        else {
            return false;
        };
        let candidate = IterationIndexSet {
            file: obligation.file,
            location: obligation.operand.clone(),
            domain: selection.domain.clone(),
            universe: None,
            cardinality: Cardinality::Unknown,
            array_dimension: None,
        };
        candidate.subset_of(target) == GuardedOutcome::Refuted
    }
    pub fn index_set(&self, file: FileId, location: &SourceLocation) -> Option<&IterationIndexSet> {
        self.index_sets
            .iter()
            .find(|i| i.file == file && i.location.range == location.range)
    }
}

/// Call with same-context bindings/callables/instantiations/domains/definitions/
/// numeric, then optional, then resolve_guarded_facts_with_options. No rules run.
/// Supports invariant integer ranges/sets, immutable aliases, enum identities,
/// ordered declared array dimensions, resolved standard index-set helpers and
/// checked constant offsets of generator bindings. Replaceable defaults never
/// supply invariant membership; sharing a named set identity still proves equality.
///
/// Direct lexical references include annotations, filters, later sources and
/// nested scopes. Local aliases and assignment sources retain dependencies;
/// arbitrary callable bodies are not expanded. Closed one-binding membership
/// and comparison filters can establish proper-partial coverage. Other filters
/// retain guarded evidence and uncertainty. Dependent range sources use only
/// proved uniform bounds. Independent symbolic products retain source factors,
/// multiplicity and coefficients; large products are not enumerated. These
/// facts authorize neither filter movement nor removal of repeated iterations.
/// Follows [MiniZinc 2.10.1 generator scope and comprehensions](https://docs.minizinc.dev/en/2.10.1/spec.html#simple-array-comprehensions).
#[allow(clippy::too_many_arguments)] // Explicit same-context prerequisites, like the existing producers.
pub fn resolve_iteration_facts(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
    optional: &OptionalFacts,
    guarded: &GuardedFacts,
) -> IterationFacts {
    let mut expression_kinds = BTreeMap::new();
    for expression in &calls.expressions {
        expression_kinds
            .entry((
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .or_insert(&expression.ty.kind);
    }
    let mut reference_indices = HashMap::with_capacity(bindings.references.len());
    for (index, reference) in bindings.references.iter().enumerate() {
        reference_indices
            .entry((reference.file, reference.location.range.start))
            .or_insert(index);
    }
    let mut p = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        optional,
        guarded,
        expression_kinds,
        reference_indices,
        facts: IterationFacts::default(),
    };
    for declaration in &bindings.declarations {
        if !context.files[declaration.file].warnings_enabled() {
            continue;
        }
        let mut domain = &domains.declarations[declaration.id.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        if let Domain::Array { indices, .. } = domain {
            let dimensions = indices
                .iter()
                .map(|d| {
                    p.set(
                        declaration.file,
                        declaration.location.clone(),
                        p.declaration_math(declaration.id)
                            .map(|_| d.clone())
                            .unwrap_or_else(Domain::Unsupported),
                        None,
                    )
                })
                .collect();
            p.facts.arrays.push(IterationArray {
                declaration: declaration.id,
                dimensions,
            });
        }
    }
    for (file, source) in context.files.iter().enumerate() {
        if source.warnings_enabled() && source.parsed.diagnostics().is_empty() {
            p.walk(file, source.parsed.tree(), &BTreeMap::new());
        }
    }
    for set in p
        .facts
        .index_sets
        .iter()
        .chain(p.facts.arrays.iter().flat_map(|a| &a.dimensions))
    {
        if let Cardinality::Unsupported(reason) = &set.cardinality
            && !p.facts.limitations.iter().any(|l| {
                l.file == set.file && l.location.range == set.location.range && l.reason == *reason
            })
        {
            p.facts.limitations.push(IterationLimitation {
                file: set.file,
                location: set.location.clone(),
                reason: reason.clone(),
            });
        }
    }
    p.facts
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
    expression_kinds: BTreeMap<(FileId, usize, usize), &'a TypeKind>,
    reference_indices: HashMap<(FileId, usize), usize>,
    facts: IterationFacts,
}
impl Producer<'_> {
    fn resolved_reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let start = tokens(&self.context.files[file].parsed, node)
            .first()?
            .range
            .start
            + self.context.files[file].byte_offset;
        self.reference_indices
            .get(&(file, start))
            .and_then(|&index| match self.bindings.references[index].resolution {
                BindingResolution::Resolved(id) => Some(id),
                _ => None,
            })
    }
    fn location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        self.context.files[file].location(node.range())
    }
    fn bound(&self, value: &NumericBound) -> NumericBound {
        match value {
            NumericBound::Defined { declaration, value }
                if self.bindings.declarations[declaration.0].role == DeclarationRole::Local =>
            {
                self.declaration_math(*declaration)
                    .map(|_| self.bound(value))
                    .unwrap_or_else(NumericBound::Unsupported)
            }
            NumericBound::Defined { declaration, .. } => NumericBound::Symbol(*declaration),
            NumericBound::Arithmetic { operator, operands } => NumericBound::Arithmetic {
                operator: *operator,
                operands: operands.iter().map(|b| self.bound(b)).collect(),
            },
            other => other.clone(),
        }
    }
    fn declaration_math(&self, id: DeclarationId) -> Result<(), String> {
        let d = &self.bindings.declarations[id.0];
        let Some(node) = find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            return Ok(());
        };
        if let Some(ty) = node
            .child_nodes()
            .find(|n| !matches!(n.kind(), NodeKind::Annotation | NodeKind::ParameterList))
        {
            core_arithmetic(self.context, self.bindings, self.calls, d.file, ty)?;
            if let Some(reason) = crate::callable_definitions::closed_integer_type_source_error(
                self.context,
                self.bindings,
                self.calls,
                self.instantiations,
                self.domains,
                (d.file, ty),
            ) {
                return Err(reason);
            }
        }
        if d.role == DeclarationRole::Local
            && let Some(rhs) = node.child_nodes().find(|n| is_expression(n.kind()))
        {
            core_arithmetic(self.context, self.bindings, self.calls, d.file, rhs)?;
        }
        Ok(())
    }
    fn normalize(&self, domain: Domain) -> Domain {
        match domain {
            Domain::Named {
                declaration,
                domain,
            } => {
                let d = &self.bindings.declarations[declaration.0];
                let ty = &self.calls.declarations[declaration.0].ty;
                let replaceable = matches!(ty.kind, TypeKind::Set(_))
                    && matches!(
                        d.role,
                        DeclarationRole::Value
                            | DeclarationRole::Local
                            | DeclarationRole::Parameter
                            | DeclarationRole::Generator
                    )
                    && !(d.role == DeclarationRole::Local
                        && d.instantiation == Instantiation::Parameter);
                Domain::Named {
                    declaration,
                    domain: Box::new(if replaceable {
                        Domain::Unknown
                    } else {
                        self.declaration_math(declaration)
                            .map(|_| self.normalize(*domain))
                            .unwrap_or_else(Domain::Unsupported)
                    }),
                }
            }
            Domain::Set(elements) => self.normalize(*elements),
            Domain::Range { lower, upper } => Domain::Range {
                lower: self.bound(&lower),
                upper: self.bound(&upper),
            },
            Domain::LiteralSet(values) => {
                Domain::LiteralSet(values.iter().map(|b| self.bound(b)).collect())
            }
            Domain::Array { indices, element } => Domain::Array {
                indices: indices.into_iter().map(|d| self.normalize(d)).collect(),
                element,
            },
            d => d,
        }
    }
    fn count(&self, domain: &Domain) -> Cardinality {
        match domain {
            Domain::Named { domain, .. } => self.count(domain),
            Domain::Enum(id) => {
                let d = &self.bindings.declarations[id.0];
                let Some(node) = find_node(
                    self.context.files[d.file].parsed.tree(),
                    &d.syntax_range,
                    d.role,
                ) else {
                    return Cardinality::Unknown;
                };
                let cases = node
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::EnumDefinition)
                    .and_then(|n| {
                        let mut parts = n.child_nodes();
                        let first = parts.next()?;
                        (first.kind() == NodeKind::EnumCases && parts.next().is_none())
                            .then_some(first)
                    });
                if let Some(cases) = cases {
                    if cases
                        .child_nodes()
                        .any(|n| n.child_nodes().next().is_some())
                    {
                        return Cardinality::Unknown;
                    }
                    Cardinality::Exact(cases.child_nodes().count() as u64)
                } else {
                    Cardinality::Unknown
                }
            }
            _ => closed_count(domain),
        }
    }
    fn set(
        &self,
        file: FileId,
        location: SourceLocation,
        domain: Domain,
        array_dimension: Option<(DeclarationId, usize)>,
    ) -> IterationIndexSet {
        let universe = if let Domain::Named { declaration, .. } = &domain {
            let d = &self.bindings.declarations[declaration.0];
            if d.instantiation == Instantiation::Decision
                && matches!(
                    self.calls.declarations[declaration.0].ty.kind,
                    TypeKind::Set(_)
                )
            {
                match &self.domains.declarations[declaration.0].domain {
                    Domain::Set(inner) => Some(self.normalize(*inner.clone())),
                    _ => None,
                }
            } else {
                None
            }
        } else {
            None
        };
        let domain = self.normalize(domain);
        let cardinality = self.count(&domain);
        IterationIndexSet {
            file,
            location,
            domain,
            universe,
            cardinality,
            array_dimension,
        }
    }
    fn source_set(
        &self,
        file: FileId,
        node: &SyntaxNode,
        known: &BTreeMap<usize, Domain>,
    ) -> IterationIndexSet {
        let node = unwrap(node);
        if array_concatenation(self.context, self.bindings, self.calls, file, node).is_some()
            && self
                .guarded
                .expression(file, &self.location(file, node))
                .is_some_and(|fact| {
                    matches!(
                        &fact.raw_definedness,
                        GuardedOutcome::Proven | GuardedOutcome::Unknown
                    ) && matches!(
                        &fact.definedness,
                        GuardedOutcome::Proven | GuardedOutcome::Unknown
                    )
                })
        {
            // Inspected value traversal supplies neither a set identity nor a count.
            return self.set(file, self.location(file, node), Domain::Unknown, None);
        }
        let helper = resolved_index_domain(
            self.context,
            self.bindings,
            self.calls,
            self.domains,
            file,
            node,
        );
        let mut domain = helper
            .as_ref()
            .map(|(_, _, d)| d.clone())
            .or_else(|| {
                crate::domains::inspected_parameter_extremum_range_domain(
                    self.context,
                    self.bindings,
                    (self.calls, self.calls),
                    self.instantiations,
                    self.domains,
                    (file, node, &[]),
                )
            })
            .or_else(|| {
                if node.kind() != NodeKind::RangeExpression {
                    return None;
                }
                let mut generators = Vec::new();
                let mut scope_error = None;
                for (id, domain) in known {
                    let declaration = &self.bindings.declarations[*id];
                    if declaration.file != file
                        || declaration.role != DeclarationRole::Generator
                        || declaration.syntax_range.end > node.range().start
                    {
                        continue;
                    }
                    if let Domain::Unsupported(reason) = domain {
                        scope_error = Some(reason.clone());
                    }
                    let Some(header) = find_node(
                        self.context.files[file].parsed.tree(),
                        &declaration.syntax_range,
                        declaration.role,
                    ) else {
                        scope_error = Some("generator source header is unavailable".into());
                        continue;
                    };
                    if !generators
                        .iter()
                        .any(|node: &&SyntaxNode| node.range() == header.range())
                    {
                        generators.push(header);
                    }
                }
                generators.sort_by_key(|node| node.range().start);
                let safety = crate::callable_definitions::decision_prefix_source_safety(
                    self.context,
                    self.bindings,
                    self.calls,
                    self.instantiations,
                    self.domains,
                    (file, node, &generators),
                )?;
                if let Some(reason) = scope_error {
                    return Some(Domain::Unsupported(reason));
                }
                let Some(evaluated) = self.guarded.expression(file, &self.location(file, node))
                else {
                    return Some(Domain::Unsupported(
                        "decision prefix evaluated source is unavailable".into(),
                    ));
                };
                for outcome in [&evaluated.raw_definedness, &evaluated.definedness] {
                    if let GuardedOutcome::Unsupported(reason) = outcome {
                        return Some(Domain::Unsupported(reason.clone()));
                    }
                }
                Some(match safety {
                    crate::DefinitionSafety::Unsupported(reason) => Domain::Unsupported(reason),
                    _ => Domain::Unknown,
                })
            })
            .unwrap_or_else(|| expression_domain(self.context, self.bindings, file, node));
        if node.kind() == NodeKind::Expression {
            if let Some(id) = self.resolved_reference(file, node)
                && let Some(d) = known.get(&id.0)
            {
                domain = d.clone();
            }
        } else if node.kind() == NodeKind::BinaryExpression {
            let children: Vec<_> = node.child_nodes().collect();
            let op = tokens(&self.context.files[file].parsed, node)
                .into_iter()
                .find(|t| matches!(t.kind, TokenKind::Plus | TokenKind::Minus));
            if let (Some(op), [left, right]) = (op, children.as_slice())
                && core_operation(
                    self.context,
                    self.bindings,
                    self.calls,
                    file,
                    node,
                    if op.kind == TokenKind::Minus {
                        "-"
                    } else {
                        "+"
                    },
                ) == Ok(true)
            {
                if unwrap(left).kind() == NodeKind::Expression
                    && let (Some(id), Some(NumericOutcome::Exact(delta))) = (
                        self.resolved_reference(file, unwrap(left)),
                        self.numeric_value(file, right),
                    )
                    && let Some(base) = known.get(&id.0)
                {
                    domain = shifted(
                        base,
                        if op.kind == TokenKind::Minus {
                            delta.checked_neg()
                        } else {
                            Some(*delta)
                        },
                    );
                }
                if op.kind == TokenKind::Plus
                    && unwrap(right).kind() == NodeKind::Expression
                    && let (Some(NumericOutcome::Exact(delta)), Some(id)) = (
                        self.numeric_value(file, left),
                        self.resolved_reference(file, unwrap(right)),
                    )
                    && let Some(base) = known.get(&id.0)
                {
                    domain = shifted(base, Some(*delta));
                }
            }
        }
        if let Some((array, _, _)) = &helper
            && let Err(reason) = self.declaration_math(*array)
        {
            domain = Domain::Unsupported(reason);
        }
        if let Err(reason) = core_arithmetic(self.context, self.bindings, self.calls, file, node) {
            domain = Domain::Unsupported(reason);
        }
        self.set(
            file,
            self.location(file, node),
            domain,
            helper.map(|(a, d, _)| (a, d)),
        )
    }
    fn numeric_value(&self, file: FileId, node: &SyntaxNode) -> Option<&NumericOutcome> {
        let location = self.location(file, node);
        self.numeric
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map(|e| &e.outcome)
    }
    fn deps(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> (Vec<DeclarationId>, Vec<IterationDependency>) {
        let location = self.location(file, node);
        let mut ids = BTreeSet::new();
        let mut unresolved = Vec::new();
        for reference in self
            .bindings
            .references
            .iter()
            .filter(|r| r.file == file && contains(&location, &r.location))
        {
            match reference.resolution {
                BindingResolution::Resolved(id) => {
                    ids.insert(id.0);
                    self.alias_deps(id, &mut ids, &mut Vec::new());
                }
                BindingResolution::Overloads(_)
                    if self.calls.calls.iter().any(|c| {
                        c.file == file
                            && c.location.range == reference.location.range
                            && matches!(c.outcome, crate::CallOutcome::Resolved { .. })
                    }) => {}
                _ => unresolved.push(IterationDependency {
                    location: reference.location.clone(),
                    resolution: reference.resolution.clone(),
                }),
            }
        }
        (ids.into_iter().map(DeclarationId).collect(), unresolved)
    }
    fn alias_deps(
        &self,
        id: DeclarationId,
        ids: &mut BTreeSet<usize>,
        active: &mut Vec<DeclarationId>,
    ) {
        if active.contains(&id) {
            return;
        }
        let d = &self.bindings.declarations[id.0];
        if !matches!(d.role, DeclarationRole::Local | DeclarationRole::Generator) {
            return;
        }
        let Some(node) = find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            return;
        };
        if d.role == DeclarationRole::Generator
            && tokens(&self.context.files[d.file].parsed, node)
                .iter()
                .any(|t| t.kind == TokenKind::In)
        {
            return;
        }
        let Some(rhs) = node.child_nodes().find(|n| is_expression(n.kind())) else {
            return;
        };
        let location = self.location(d.file, rhs);
        active.push(id);
        for r in self
            .bindings
            .references
            .iter()
            .filter(|r| r.file == d.file && contains(&location, &r.location))
        {
            if let BindingResolution::Resolved(other) = r.resolution {
                ids.insert(other.0);
                self.alias_deps(other, ids, active);
            }
        }
        active.pop();
    }
    fn expression(
        &self,
        file: FileId,
        node: &SyntaxNode,
        scopes: &BTreeMap<usize, usize>,
        entry: &SourceLocation,
    ) -> IterationExpression {
        let location = self.location(file, node);
        let (dependencies, unresolved) = self.deps(file, node);
        let iteration_dependencies: Vec<_> = dependencies
            .iter()
            .copied()
            .filter(|id| scopes.contains_key(&id.0))
            .collect();
        let invariant_with_respect_to = scopes
            .keys()
            .copied()
            .map(DeclarationId)
            .filter(|id| !dependencies.contains(id))
            .collect();
        let external = dependencies.iter().any(|id| {
            let d = &self.bindings.declarations[id.0];
            matches!(d.role, DeclarationRole::Generator | DeclarationRole::Local)
                && !scopes.contains_key(&id.0)
                // A resolved immutable enclosing local already exists at this
                // iteration entry; a body-local alias does not exist there.
                && !(d.role == DeclarationRole::Local
                    && d.instantiation == Instantiation::Parameter
                    && d.file == file && d.location.range.end <= entry.range.start)
                && !(d.file == file
                    && contains(
                        &location,
                        &self.context.files[file].location(d.syntax_range.clone()),
                    ))
        });
        let earliest_scope = if unresolved.is_empty() && !external {
            Some(
                iteration_dependencies
                    .iter()
                    .map(|id| scopes[&id.0])
                    .max()
                    .unwrap_or(0),
            )
        } else {
            None
        };
        let guarded = self.guarded.expression(file, &location).cloned();
        let obligations = self
            .guarded
            .obligations
            .iter()
            .filter(|o| o.file == file && contains(&location, &o.operation))
            .cloned()
            .collect();
        IterationExpression {
            file,
            location,
            dependencies,
            unresolved,
            iteration_dependencies,
            invariant_with_respect_to,
            earliest_scope,
            guarded,
            obligations,
        }
    }
    fn walk(&mut self, file: FileId, node: &SyntaxNode, known: &BTreeMap<usize, Domain>) {
        if node
            .child_nodes()
            .any(|n| n.kind() == NodeKind::GeneratorList)
        {
            let (fact, local) = self.iteration(file, node, known);
            self.facts.iterations.push(fact);
            for child in node.child_nodes() {
                self.walk(file, child, &local);
            }
            return;
        }
        if is_expression(node.kind()) {
            let location = self.location(file, node);
            let ty = self
                .expression_kinds
                .get(&(file, location.range.start, location.range.end));
            if matches!(ty, Some(TypeKind::Set(_)))
                || resolved_index_domain(
                    self.context,
                    self.bindings,
                    self.calls,
                    self.domains,
                    file,
                    unwrap(node),
                )
                .is_some()
                || self.offset_binding(file, node, known)
            {
                let set = self.source_set(file, node, known);
                self.facts.index_sets.push(set);
            }
        }
        for child in node.child_nodes() {
            self.walk(file, child, known);
        }
    }
    fn offset_binding(
        &self,
        file: FileId,
        node: &SyntaxNode,
        known: &BTreeMap<usize, Domain>,
    ) -> bool {
        node.kind() == NodeKind::BinaryExpression
            && node.child_nodes().any(|n| {
                unwrap(n).kind() == NodeKind::Expression
                    && self
                        .resolved_reference(file, unwrap(n))
                        .is_some_and(|id| known.contains_key(&id.0))
            })
    }
    fn iteration(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        inherited: &BTreeMap<usize, Domain>,
    ) -> (IterationFact, BTreeMap<usize, Domain>) {
        let location = self.location(file, node);
        let head = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
            .unwrap_or(node);
        let mut known = inherited.clone();
        let mut scopes: BTreeMap<_, _> = inherited.keys().map(|id| (*id, 0)).collect();
        let mut generators = Vec::new();
        let mut expressions = Vec::new();
        let mut candidates = CandidateCount::Exact(1);
        let mut coverage = IterationCoverage::Full;
        let mut filtered_domains = BTreeMap::new();
        let mut filters = Vec::new();
        for list in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::GeneratorList)
        {
            for generator in list.child_nodes() {
                let Some(source) = generator.child_nodes().next() else {
                    continue;
                };
                let position = generators.len();
                let membership = tokens(&self.context.files[file].parsed, generator)
                    .iter()
                    .any(|t| t.kind == TokenKind::In);
                let bindings: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == generator.range()
                    })
                    .map(|d| d.id)
                    .collect();
                let slots = generator_slots(&self.context.files[file].parsed, generator);
                let mut index_set = self.source_set(file, source, &known);
                if !membership {
                    index_set.domain = self.assignment_domain(file, source);
                    index_set.cardinality = Cardinality::Exact(1);
                } else {
                    self.facts.index_sets.push(index_set.clone());
                }
                let (dependencies, _) = self.deps(file, source);
                let dependent = dependencies.iter().any(|id| scopes.contains_key(&id.0));
                let bound = if membership && dependent {
                    self.uniform_count(file, source, &known)
                } else {
                    None
                };
                if membership {
                    let count = if matches!(index_set.cardinality, Cardinality::Unsupported(_)) {
                        index_set.cardinality.clone()
                    } else if dependent {
                        bound
                            .map(|n| Cardinality::Bounds { lower: 0, upper: n })
                            .unwrap_or(Cardinality::Unknown)
                    } else if let Some(universe) = &index_set.universe {
                        let c = self.count(universe);
                        c.bounds()
                            .map(|(_, upper)| Cardinality::Bounds { lower: 0, upper })
                            .unwrap_or(c)
                    } else {
                        self.source_count(file, source, &index_set)
                    };
                    if dependent
                        && bound.is_none()
                        && !matches!(count, Cardinality::Unsupported(_))
                        && !matches!(
                            candidates,
                            CandidateCount::Exact(0) | CandidateCount::UpperBound(0)
                        )
                    {
                        candidates = CandidateCount::Unknown;
                    }
                    for slot in 0..slots {
                        candidates = product(
                            candidates,
                            &count,
                            CandidateFactor {
                                file,
                                location: self.location(file, source),
                                slot,
                                domain: index_set
                                    .universe
                                    .as_ref()
                                    .unwrap_or(&index_set.domain)
                                    .clone(),
                                dependencies: dependencies.clone(),
                            },
                        );
                    }
                }
                if index_set.universe.is_some() {
                    candidates = upper_bound(candidates);
                }
                self.expression_walk(file, source, &scopes, &location, &mut expressions);
                for id in &bindings {
                    scopes.insert(id.0, position + 1);
                    known.insert(
                        id.0,
                        if membership {
                            index_set.domain.clone()
                        } else {
                            self.assignment_domain(file, source)
                        },
                    );
                }
                if (dependent || index_set.universe.is_some())
                    && coverage != IterationCoverage::Empty
                {
                    coverage = IterationCoverage::Unknown;
                }
                for filter in generator
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    if let Some(condition) = filter.child_nodes().next() {
                        let e = self.expression(file, condition, &scopes, &location);
                        let truth = e
                            .guarded
                            .as_ref()
                            .and_then(|g| g.truth.clone())
                            .unwrap_or(GuardedOutcome::Unknown);
                        let definedness = e
                            .guarded
                            .as_ref()
                            .map(|g| g.definedness.clone())
                            .unwrap_or(GuardedOutcome::Unknown);
                        let incoming_empty = coverage == IterationCoverage::Empty
                            || matches!(
                                candidates,
                                CandidateCount::Exact(0) | CandidateCount::UpperBound(0)
                            );
                        let prior_domain = (bindings.len() == 1).then(|| {
                            filtered_domains
                                .get(&position)
                                .unwrap_or(&index_set.domain)
                                .clone()
                        });
                        let mut selected_domain = None;
                        let mut filter_coverage = IterationCoverage::Unknown;
                        match (&truth, &definedness) {
                            (GuardedOutcome::Refuted, GuardedOutcome::Proven) => {
                                filter_coverage = IterationCoverage::Empty;
                                coverage = IterationCoverage::Empty;
                            }
                            (GuardedOutcome::Proven, GuardedOutcome::Proven) => {
                                filter_coverage = IterationCoverage::Full
                            }
                            _ => {
                                if !incoming_empty
                                    && definedness == GuardedOutcome::Proven
                                    && bindings.len() == 1
                                {
                                    if let Some(d) = self.filter_domain(
                                        file,
                                        condition,
                                        bindings[0],
                                        prior_domain.as_ref().unwrap(),
                                    ) {
                                        let count = self.count(&d);
                                        if count == Cardinality::Exact(0) {
                                            filter_coverage = IterationCoverage::Empty;
                                            coverage = IterationCoverage::Empty;
                                        } else if same_members(&d, &index_set.domain)
                                            == Ok(Some(false))
                                        {
                                            filter_coverage = IterationCoverage::ProperPartial;
                                            if coverage == IterationCoverage::Full {
                                                coverage = IterationCoverage::ProperPartial;
                                            }
                                        } else if same_members(&d, prior_domain.as_ref().unwrap())
                                            == Ok(Some(true))
                                        {
                                            filter_coverage = IterationCoverage::Full;
                                        }
                                        selected_domain = Some(d.clone());
                                        filtered_domains.insert(position, d);
                                    } else if coverage != IterationCoverage::Empty {
                                        coverage = IterationCoverage::Unknown;
                                    }
                                } else if coverage != IterationCoverage::Empty {
                                    coverage = IterationCoverage::Unknown;
                                }
                            }
                        }
                        filters.push(IterationFilter {
                            location: self.location(file, condition),
                            truth,
                            definedness,
                            incoming_empty,
                            prior_domain,
                            selected_domain,
                            coverage: filter_coverage,
                        });
                        self.expression_walk(file, condition, &scopes, &location, &mut expressions);
                    }
                }
                let source_instantiation = self
                    .instantiations
                    .expressions
                    .iter()
                    .find(|e| {
                        e.file == file && e.location.range == self.location(file, source).range
                    })
                    .map(|e| e.instantiation)
                    .unwrap_or(Instantiation::Unknown);
                generators.push(IterationGenerator {
                    source_instantiation,
                    location: self.location(file, generator),
                    source: self.location(file, source),
                    position,
                    membership,
                    slots,
                    bindings,
                    index_set,
                    dependencies,
                    uniform_upper_bound: bound,
                });
            }
        }
        self.expression_walk(file, head, &scopes, &location, &mut expressions);
        let uses = self.uses(file, node, &generators);
        let selected = if coverage == IterationCoverage::Empty {
            CandidateCount::Exact(0)
        } else if coverage == IterationCoverage::Full {
            candidates.clone()
        } else if coverage == IterationCoverage::ProperPartial {
            let mut result = CandidateCount::Exact(1);
            for g in &generators {
                if !g.membership {
                    continue;
                }
                let d = filtered_domains
                    .get(&g.position)
                    .unwrap_or(&g.index_set.domain);
                for slot in 0..g.slots {
                    result = product(
                        result,
                        &self.count(d),
                        CandidateFactor {
                            file,
                            location: g.source.clone(),
                            slot,
                            domain: d.clone(),
                            dependencies: g.dependencies.clone(),
                        },
                    );
                }
            }
            result
        } else {
            upper_bound(candidates.clone())
        };
        if matches!(
            candidates,
            CandidateCount::Exact(0) | CandidateCount::UpperBound(0)
        ) {
            coverage = IterationCoverage::Empty;
        }
        if let CandidateCount::Unsupported(reason) = &candidates {
            self.facts.limitations.push(IterationLimitation {
                file,
                location: location.clone(),
                reason: reason.clone(),
            });
        }
        let multiplicity = if node.kind() == NodeKind::GeneratorCallExpression {
            resolved_call(self.context, self.calls, file, node)
                .filter(|id| {
                    core_callable(
                        self.context,
                        self.bindings,
                        *id,
                        &self.bindings.declarations[id.0].name,
                    )
                })
                .map(|id| match self.bindings.declarations[id.0].name.as_str() {
                    "forall" | "exists" => IterationMultiplicity::IdempotentQuantifier,
                    "sum" | "product" => IterationMultiplicity::Arithmetic,
                    _ => IterationMultiplicity::Unknown,
                })
                .unwrap_or(IterationMultiplicity::Unknown)
        } else {
            IterationMultiplicity::Collection
        };
        let optional_collection = self.optional.collection(file, &location).cloned();
        (
            IterationFact {
                file,
                location,
                body: self.location(file, head),
                generators,
                filters,
                uses,
                expressions,
                candidates,
                selected,
                coverage,
                optional_collection,
                multiplicity,
            },
            known,
        )
    }
    fn assignment_domain(&self, file: FileId, node: &SyntaxNode) -> Domain {
        match self.numeric_value(file, node) {
            Some(NumericOutcome::Exact(n)) => Domain::LiteralSet(vec![NumericBound::Integer(*n)]),
            _ => Domain::Unknown,
        }
    }
    fn source_count(
        &self,
        file: FileId,
        node: &SyntaxNode,
        set: &IterationIndexSet,
    ) -> Cardinality {
        if let Domain::Array { indices, .. } = &set.domain {
            return indices.iter().fold(Cardinality::Exact(1), |a, d| {
                cardinal_product(a, self.count(d))
            });
        }
        let location = self.location(file, node);
        if self.calls.expressions.iter().any(|e| {
            e.file == file
                && e.location.range == location.range
                && matches!(e.ty.kind, TypeKind::Array { .. })
        }) && let Some(c) = self.optional.collection(file, &location)
            && matches!(c.capacity, Cardinality::Exact(_))
        {
            return c.capacity.clone();
        }
        set.cardinality.clone()
    }
    fn uniform_count(
        &self,
        file: FileId,
        node: &SyntaxNode,
        known: &BTreeMap<usize, Domain>,
    ) -> Option<u64> {
        let domain = self.normalize(expression_domain(self.context, self.bindings, file, node));
        let Domain::Range { lower, upper } = domain else {
            return None;
        };
        let (l, _) = bound_interval(&lower, known)?;
        let (_, u) = bound_interval(&upper, known)?;
        range_count(l, u).ok()
    }
    fn filter_domain(
        &self,
        file: FileId,
        node: &SyntaxNode,
        id: DeclarationId,
        source: &Domain,
    ) -> Option<Domain> {
        let node = unwrap(node);
        if !matches!(node.kind(), NodeKind::BinaryExpression) {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [left, right] = children.as_slice() else {
            return None;
        };
        if unwrap(left).kind() != NodeKind::Expression
            || self.resolved_reference(file, unwrap(left)) != Some(id)
        {
            return None;
        }
        let op = tokens(&self.context.files[file].parsed, node)
            .into_iter()
            .find(|t| {
                matches!(
                    t.kind,
                    TokenKind::In
                        | TokenKind::Less
                        | TokenKind::LessEqual
                        | TokenKind::Greater
                        | TokenKind::GreaterEqual
                        | TokenKind::Equal
                        | TokenKind::DoubleEqual
                )
            })?
            .kind;
        let name = crate::bindings::symbolic_operator(op)?;
        if core_operation(self.context, self.bindings, self.calls, file, node, name) != Ok(true) {
            return None;
        }
        let target = if op == TokenKind::In {
            if core_arithmetic(self.context, self.bindings, self.calls, file, right).is_err() {
                return None;
            }
            self.normalize(expression_domain(self.context, self.bindings, file, right))
        } else {
            let NumericOutcome::Exact(n) = self.numeric_value(file, right)? else {
                return None;
            };
            let (l, u) = match op {
                TokenKind::Less => (i64::MIN, n.checked_sub(1)?),
                TokenKind::LessEqual => (i64::MIN, *n),
                TokenKind::Greater => (n.checked_add(1)?, i64::MAX),
                TokenKind::GreaterEqual => (*n, i64::MAX),
                _ => (*n, *n),
            };
            Domain::Range {
                lower: NumericBound::Integer(l),
                upper: NumericBound::Integer(u),
            }
        };
        intersection(source, &target)
    }
    fn expression_walk(
        &self,
        file: FileId,
        node: &SyntaxNode,
        scopes: &BTreeMap<usize, usize>,
        entry: &SourceLocation,
        out: &mut Vec<IterationExpression>,
    ) {
        if is_expression(node.kind()) {
            out.push(self.expression(file, node, scopes, entry));
        }
        for child in node.child_nodes() {
            self.expression_walk(file, child, scopes, entry, out);
        }
    }
    fn uses(
        &self,
        file: FileId,
        node: &SyntaxNode,
        generators: &[IterationGenerator],
    ) -> Vec<IterationBindingUse> {
        let all: Vec<_> = generators
            .iter()
            .flat_map(|g| &g.bindings)
            .copied()
            .collect();
        let location = self.location(file, node);
        self.bindings
            .references
            .iter()
            .filter_map(|r| {
                if r.file != file || !contains(&location, &r.location) {
                    return None;
                }
                let BindingResolution::Resolved(id) = r.resolution else {
                    return None;
                };
                if !all.contains(&id) {
                    return None;
                }
                let region = generators
                    .iter()
                    .find_map(|g| {
                        if contains(&g.source, &r.location) {
                            Some(IterationUseRegion::Source(g.position))
                        } else if contains(&g.location, &r.location) {
                            Some(IterationUseRegion::Filter(g.position))
                        } else {
                            None
                        }
                    })
                    .unwrap_or(IterationUseRegion::Body);
                let annotation = inside_kind(
                    self.context.files[file].parsed.tree(),
                    &r.location.range,
                    self.context.files[file].byte_offset,
                    NodeKind::Annotation,
                );
                Some(IterationBindingUse {
                    binding: id,
                    location: r.location.clone(),
                    region,
                    annotation,
                })
            })
            .collect()
    }
}
fn contains(a: &SourceLocation, b: &SourceLocation) -> bool {
    a.range.start <= b.range.start && b.range.end <= a.range.end
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
    ) {
        if let Some(n) = node.child_nodes().next() {
            node = n;
        } else {
            break;
        }
    }
    node
}
fn inside_kind(
    node: &SyntaxNode,
    range: &std::ops::Range<usize>,
    offset: usize,
    kind: NodeKind,
) -> bool {
    if node.range().start + offset > range.start || node.range().end + offset < range.end {
        return false;
    }
    node.kind() == kind
        || node
            .child_nodes()
            .any(|n| inside_kind(n, range, offset, kind))
}

fn closed_count(d: &Domain) -> Cardinality {
    match bare(d) {
        Domain::Range { lower, upper } => {
            match (invariant_integer(lower), invariant_integer(upper)) {
                (Ok(Some(l)), Ok(Some(u))) => range_count(l, u)
                    .map(Cardinality::Exact)
                    .unwrap_or_else(Cardinality::Unsupported),
                (Err(e), _) | (_, Err(e)) => Cardinality::Unsupported(e),
                _ => Cardinality::Unknown,
            }
        }
        Domain::LiteralSet(values) => {
            let mut set = BTreeSet::new();
            for value in values {
                match invariant_integer(value) {
                    Ok(Some(n)) => {
                        set.insert(n);
                    }
                    Ok(None) => return Cardinality::Unknown,
                    Err(e) => return Cardinality::Unsupported(e),
                }
            }
            Cardinality::Exact(set.len() as u64)
        }
        Domain::Unsupported(e) => Cardinality::Unsupported(e.clone()),
        _ => Cardinality::Unknown,
    }
}
fn range_count(l: i64, u: i64) -> Result<u64, String> {
    if u < l {
        return Ok(0);
    }
    u64::try_from(i128::from(u) - i128::from(l) + 1).map_err(|_| "iteration count overflow".into())
}

fn relation(a: &Domain, b: &Domain, disjoint: bool) -> GuardedOutcome {
    if let Err(e) = same_members(a, b) {
        return GuardedOutcome::Unsupported(e);
    }
    if closed_count(a) == Cardinality::Exact(0)
        || disjoint && closed_count(b) == Cardinality::Exact(0)
    {
        return GuardedOutcome::Proven;
    }
    if same_members(a, b) == Ok(Some(true)) {
        return if !disjoint {
            GuardedOutcome::Proven
        } else if closed_count(a).bounds().is_some_and(|(l, _)| l > 0) {
            GuardedOutcome::Refuted
        } else {
            GuardedOutcome::Unknown
        };
    }
    if let (Domain::Enum(x), Domain::Enum(y)) = (bare(a), bare(b))
        && x != y
    {
        return if disjoint {
            GuardedOutcome::Proven
        } else {
            GuardedOutcome::Unknown
        };
    }
    let result = if let Domain::LiteralSet(values) = bare(a) {
        let values: Option<Vec<_>> = values
            .iter()
            .map(|v| invariant_integer(v).ok().flatten())
            .collect();
        values.and_then(|v| {
            let memberships: Option<Vec<_>> = v.into_iter().map(|n| member(b, n)).collect();
            memberships.map(|v| {
                if disjoint {
                    v.iter().all(|m| !m)
                } else {
                    v.iter().all(|m| *m)
                }
            })
        })
    } else if disjoint && matches!(bare(b), Domain::LiteralSet(_)) {
        return relation(b, a, true);
    } else if let (Some((l, u)), Some((x, y))) = (interval(a), interval(b)) {
        if disjoint {
            Some(u < x || y < l)
        } else if matches!(bare(b), Domain::Range { .. }) {
            Some(x <= l && u <= y)
        } else if let Domain::LiteralSet(values) = bare(b) {
            let values: Option<BTreeSet<_>> = values
                .iter()
                .map(|v| invariant_integer(v).ok().flatten())
                .collect();
            values.and_then(|values| {
                range_count(l, u)
                    .ok()
                    .map(|n| values.iter().filter(|v| l <= **v && **v <= u).count() as u64 == n)
            })
        } else {
            None
        }
    } else {
        None
    };
    match result {
        Some(true) => GuardedOutcome::Proven,
        Some(false) => GuardedOutcome::Refuted,
        None => GuardedOutcome::Unknown,
    }
}

fn bound_interval(b: &NumericBound, known: &BTreeMap<usize, Domain>) -> Option<(i64, i64)> {
    if let Some(n) = invariant_integer(b).ok()? {
        return Some((n, n));
    }
    match b {
        NumericBound::Symbol(id) => interval(known.get(&id.0)?),
        NumericBound::Arithmetic { operator, operands } => {
            let [a, b] = operands.as_slice() else {
                return None;
            };
            let (l, u) = bound_interval(a, known)?;
            let (x, y) = bound_interval(b, known)?;
            match operator {
                TokenKind::Plus => Some((l.checked_add(x)?, u.checked_add(y)?)),
                TokenKind::Minus => Some((l.checked_sub(y)?, u.checked_sub(x)?)),
                _ => None,
            }
        }
        _ => None,
    }
}
fn product(a: CandidateCount, b: &Cardinality, factor: CandidateFactor) -> CandidateCount {
    if matches!(a, CandidateCount::Exact(0) | CandidateCount::UpperBound(0))
        || b.bounds().is_some_and(|(_, u)| u == 0)
    {
        return CandidateCount::Exact(0);
    }
    if let Cardinality::Unsupported(reason) = b {
        return CandidateCount::Unsupported(reason.clone());
    }
    let (coefficient, mut factors, mut upper_bound) = match a {
        CandidateCount::Exact(n) => (n, Vec::new(), false),
        CandidateCount::UpperBound(n) => (n, Vec::new(), true),
        CandidateCount::Symbolic {
            coefficient,
            factors,
            upper_bound,
        } => (coefficient, factors, upper_bound),
        other => return other,
    };
    match b {
        Cardinality::Exact(n) | Cardinality::Bounds { upper: n, .. } => {
            upper_bound |= matches!(b, Cardinality::Bounds { .. });
            let Some(n) = coefficient.checked_mul(*n) else {
                return CandidateCount::Unsupported("iteration product overflow".into());
            };
            if factors.is_empty() {
                if upper_bound {
                    CandidateCount::UpperBound(n)
                } else {
                    CandidateCount::Exact(n)
                }
            } else {
                CandidateCount::Symbolic {
                    coefficient: n,
                    factors,
                    upper_bound,
                }
            }
        }
        Cardinality::Unsupported(e) => CandidateCount::Unsupported(e.clone()),
        Cardinality::Symbolic { .. } | Cardinality::Unknown => {
            if !symbolic_domain(&factor.domain) {
                return CandidateCount::Unknown;
            }
            factors.push(factor);
            CandidateCount::Symbolic {
                coefficient,
                factors,
                upper_bound,
            }
        }
    }
}
fn upper_bound(c: CandidateCount) -> CandidateCount {
    match c {
        CandidateCount::Exact(n) => CandidateCount::UpperBound(n),
        CandidateCount::Symbolic {
            coefficient,
            factors,
            ..
        } => CandidateCount::Symbolic {
            coefficient,
            factors,
            upper_bound: true,
        },
        c => c,
    }
}
fn cardinal_product(a: Cardinality, b: Cardinality) -> Cardinality {
    match (a, b) {
        (Cardinality::Exact(0), _) | (_, Cardinality::Exact(0)) => Cardinality::Exact(0),
        (Cardinality::Exact(a), Cardinality::Exact(b)) => a
            .checked_mul(b)
            .map(Cardinality::Exact)
            .unwrap_or_else(|| Cardinality::Unsupported("iteration product overflow".into())),
        (Cardinality::Unsupported(e), _) | (_, Cardinality::Unsupported(e)) => {
            Cardinality::Unsupported(e)
        }
        _ => Cardinality::Unknown,
    }
}

fn symbolic_domain(d: &Domain) -> bool {
    fn symbolic(b: &NumericBound) -> bool {
        match b {
            NumericBound::Symbol(_) => true,
            NumericBound::Arithmetic { operands, .. } => operands.iter().any(symbolic),
            _ => false,
        }
    }
    match d {
        Domain::Named { .. } | Domain::Enum(_) => true,
        Domain::Range { lower, upper } => symbolic(lower) || symbolic(upper),
        Domain::LiteralSet(values) => values.iter().any(symbolic),
        _ => false,
    }
}
