//! Option presence and collection counts, independent of guarded definedness.
use crate::callables::{is_expression, operation_head_start};
use crate::definitions::resolved_call;
use crate::domains::{expression_domain, generator_slots, invariant_integer, tokens};
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    DefinitionCoverage, DefinitionEnforcement, DefinitionFacts, Domain, DomainFacts, FileId,
    Instantiation, InstantiationFacts, ModelContext, NumericFacts, NumericOutcome, SourceKind,
    SourceLocation, TypeInst, TypeKind,
};
use std::collections::{BTreeMap, HashMap};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

/// Presence when a value is defined. Absent is a defined option value; Present
/// does not prove evaluation succeeds. Conditional retains a decision gate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Presence {
    Present,
    Absent,
    Conditional,
    Unknown,
    Unsupported(String),
}
/// Counts are nonnegative and checked. Symbolic dependencies retain identity,
/// not an algebraic formula or a proof of equality between two counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cardinality {
    Exact(u64),
    Bounds { lower: u64, upper: u64 },
    Symbolic { dependencies: Vec<DeclarationId> },
    Unknown,
    Unsupported(String),
}
impl Cardinality {
    pub fn bounds(&self) -> Option<(u64, u64)> {
        match self {
            Self::Exact(n) => Some((*n, *n)),
            Self::Bounds { lower, upper } => Some((*lower, *upper)),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OptionalConditionKind {
    Membership { declarations: Vec<DeclarationId> },
    Filter,
    Branch { expected: bool },
}
#[derive(Clone, Debug)]
pub struct OptionalCondition {
    pub file: FileId,
    pub location: SourceLocation,
    pub kind: OptionalConditionKind,
}
#[derive(Clone, Debug)]
pub struct CollectionCardinality {
    /// Candidates before filters; decision-set generators use their finite
    /// declared universe, not the unknown actual set membership.
    pub candidates: Cardinality,
    /// Array index capacity after parameter filters. Decision filters and
    /// membership leave this capacity unchanged.
    pub capacity: Cardinality,
    /// Defined present values; neither positive capacity nor an optional
    /// parameter's written default establishes a positive present count.
    pub present: Cardinality,
    pub element_presence: Presence,
    pub conditions: Vec<OptionalCondition>,
}
#[derive(Clone, Debug)]
pub struct OptionalExpression {
    pub file: FileId,
    pub location: SourceLocation,
    pub presence: Presence,
    pub conditions: Vec<OptionalCondition>,
}
#[derive(Clone, Debug)]
pub struct OptionalDeclaration {
    pub declaration: DeclarationId,
    pub file: FileId,
    pub location: SourceLocation,
    pub presence: Presence,
    pub conditions: Vec<OptionalCondition>,
    pub collection: Option<CollectionCardinality>,
}
#[derive(Clone, Debug)]
pub struct OptionalCollection {
    pub file: FileId,
    pub location: SourceLocation,
    /// For generator calls this describes the implicit input collection, not
    /// the aggregate's scalar result.
    pub cardinality: CollectionCardinality,
}
#[derive(Clone, Debug)]
pub struct OptionalLimitation {
    pub file: FileId,
    pub location: SourceLocation,
    pub reason: String,
}
#[derive(Debug, Default)]
pub struct OptionalFacts {
    pub declarations: Vec<OptionalDeclaration>,
    pub expressions: Vec<OptionalExpression>,
    pub collections: Vec<OptionalCollection>,
    pub limitations: Vec<OptionalLimitation>,
}
impl OptionalFacts {
    pub fn declaration(&self, id: DeclarationId) -> Option<&OptionalDeclaration> {
        self.declarations.iter().find(|d| d.declaration == id)
    }
    pub fn expression(
        &self,
        file: FileId,
        location: &SourceLocation,
    ) -> Option<&OptionalExpression> {
        self.expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
    }
    pub fn collection(
        &self,
        file: FileId,
        location: &SourceLocation,
    ) -> Option<&CollectionCardinality> {
        self.collections
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map(|e| &e.cardinality)
    }
}

/// Call after bindings, callables, instantiations, domains, definitions and
/// numeric facts, all from the same retained ModelContext. GuardedFacts is not
/// a prerequisite. Use these facts with resolve_guarded_facts_with_options for
/// local guards and operation definedness; these intrinsic facts issue no lint.
///
/// Supports scalar presence, enforced initializers/aliases, closed integer
/// ranges, literal arrays/sets and independent comprehension generators. Counts
/// do not enumerate parameter filters or dependent generators. Replaceable
/// parameter defaults and omitted optional data remain unknown. Decision-set
/// membership and filters retain optional gates separately from index capacity.
/// Arbitrary optional callable bodies, weak arithmetic and unsupported count
/// arithmetic retain located limitations. Structured projections use only their
/// known type's presence; their contents and general iteration are not evaluated.
///
/// Follows [MiniZinc 2.10.1 option types](https://docs.minizinc.dev/en/stable/optiontypes.html)
/// and [default](https://docs.minizinc.dev/en/2.10.1/spec.html#coercion-operations).
pub fn resolve_optional_facts(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
    definitions: &DefinitionFacts,
) -> OptionalFacts {
    let mut expression_indices = HashMap::with_capacity(calls.expressions.len());
    for (row, expression) in calls.expressions.iter().enumerate() {
        expression_indices
            .entry((
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .or_insert(row);
    }
    let mut reference_indices = HashMap::with_capacity(bindings.references.len());
    for (row, reference) in bindings.references.iter().enumerate() {
        reference_indices
            .entry((reference.file, reference.location.range.start))
            .or_insert(row);
    }
    let mut call_indices = HashMap::with_capacity(calls.calls.len());
    for (row, call) in calls.calls.iter().enumerate() {
        call_indices
            .entry((call.file, call.location.range.start))
            .or_insert(row);
    }
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        definitions,
        expression_indices,
        reference_indices,
        call_indices,
        memo: vec![None; bindings.declarations.len()],
        active: Vec::new(),
        expressions: BTreeMap::new(),
        facts: OptionalFacts::default(),
    };
    for declaration in &bindings.declarations {
        if context.files[declaration.file].warnings_enabled()
            && matches!(
                declaration.role,
                DeclarationRole::Value
                    | DeclarationRole::Local
                    | DeclarationRole::Parameter
                    | DeclarationRole::Generator
            )
        {
            let value = producer.declaration(declaration.id);
            producer.facts.declarations.push(OptionalDeclaration {
                declaration: declaration.id,
                file: declaration.file,
                location: declaration.location.clone(),
                presence: value.presence,
                conditions: value.conditions,
                collection: value.collection,
            });
        }
    }
    for (file, source) in context.files.iter().enumerate() {
        if source.warnings_enabled() && source.parsed.diagnostics().is_empty() {
            producer.inspect(file, source.parsed.tree());
        }
    }
    producer.facts
}
#[derive(Clone)]
struct Value {
    presence: Presence,
    collection: Option<CollectionCardinality>,
    conditions: Vec<OptionalCondition>,
}
impl Value {
    fn scalar(presence: Presence) -> Self {
        Self {
            presence,
            collection: None,
            conditions: Vec::new(),
        }
    }
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    numeric: &'a NumericFacts,
    definitions: &'a DefinitionFacts,
    expression_indices: HashMap<(FileId, usize, usize), usize>,
    reference_indices: HashMap<(FileId, usize), usize>,
    call_indices: HashMap<(FileId, usize), usize>,
    memo: Vec<Option<Value>>,
    active: Vec<DeclarationId>,
    expressions: BTreeMap<(FileId, usize, usize), Value>,
    facts: OptionalFacts,
}
impl Producer<'_> {
    fn location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        self.context.files[file].location(node.range())
    }
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        let range = self.location(file, node).range;
        self.expression_indices
            .get(&(file, range.start, range.end))
            .map(|row| &self.calls.expressions[*row].ty)
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let start = tokens(&self.context.files[file].parsed, node)
            .first()?
            .range
            .start
            + self.context.files[file].byte_offset;
        self.reference_indices.get(&(file, start)).and_then(|row| {
            match self.bindings.references[*row].resolution {
                BindingResolution::Resolved(id) => Some(id),
                _ => None,
            }
        })
    }
    fn core(&self, file: FileId, node: &SyntaxNode, name: &str) -> bool {
        let Some(start) = operation_head_start(self.context, file, node) else {
            return false;
        };
        match self
            .call_indices
            .get(&(file, start))
            .map(|row| &self.calls.calls[*row].outcome)
        {
            Some(CallOutcome::Resolved { declaration, .. }) => {
                crate::definitions::core_callable(self.context, self.bindings, *declaration, name)
            }
            Some(CallOutcome::Intrinsic {
                name: intrinsic, ..
            }) => name == "+" && intrinsic == "unary+" && node.kind() == NodeKind::UnaryExpression,
            _ => false,
        }
    }
    fn references_any(&self, file: FileId, node: &SyntaxNode, ids: &[DeclarationId]) -> bool {
        node.kind() == NodeKind::Expression
            && self
                .reference(file, node)
                .is_some_and(|id| ids.contains(&id))
            || node
                .child_nodes()
                .any(|child| self.references_any(file, child, ids))
    }
    fn inst(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        let range = self.location(file, node).range;
        self.instantiations
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map_or(Instantiation::Unknown, |e| e.instantiation)
    }
    fn inspect(&mut self, file: FileId, node: &SyntaxNode) {
        if is_expression(node.kind()) {
            self.expression(file, node);
        }
        for child in node.child_nodes() {
            self.inspect(file, child);
        }
    }
    fn unsupported(&mut self, file: FileId, node: &SyntaxNode, reason: impl Into<String>) -> Value {
        let reason = reason.into();
        self.facts.limitations.push(OptionalLimitation {
            file,
            location: self.location(file, node),
            reason: reason.clone(),
        });
        Value::scalar(Presence::Unsupported(reason))
    }
    fn declaration(&mut self, id: DeclarationId) -> Value {
        if let Some(value) = &self.memo[id.0] {
            return value.clone();
        }
        if self.active.contains(&id) {
            return Value::scalar(Presence::Unknown);
        }
        let declaration = &self.bindings.declarations[id.0];
        let ty = &self.calls.declarations[id.0].ty;
        if !ty.known() {
            let reason = "unavailable declaration type prevents presence interpretation".to_owned();
            self.facts.limitations.push(OptionalLimitation {
                file: declaration.file,
                location: declaration.location.clone(),
                reason: reason.clone(),
            });
            let value = Value::scalar(Presence::Unsupported(reason));
            self.memo[id.0] = Some(value.clone());
            return value;
        }
        let mut value = Value::scalar(if !ty.optional && ty.known() {
            Presence::Present
        } else if declaration.instantiation == Instantiation::Parameter {
            Presence::Unknown
        } else {
            Presence::Conditional
        });
        if let TypeKind::Array { element, .. } = &ty.kind {
            let capacity = match &self.domains.declarations[id.0].domain {
                Domain::Array { indices, .. } => indices
                    .iter()
                    .fold(Cardinality::Exact(1), |n, d| multiply(n, domain_count(d))),
                _ => Cardinality::Unknown,
            };
            let element_presence = if element.optional {
                Presence::Unknown
            } else {
                Presence::Present
            };
            value.collection = Some(collection(
                capacity.clone(),
                capacity,
                element_presence,
                Vec::new(),
            ));
        }
        // Model parameter defaults may be replaced by instance data. A local
        // par initializer is immutable and can supply an enforced value.
        if declaration.instantiation != Instantiation::Parameter
            || declaration.role == DeclarationRole::Local
        {
            self.active.push(id);
            let mut values = Vec::new();
            for definition in &self.definitions.definitions {
                if definition.target == id
                    && definition.enforcement == DefinitionEnforcement::Enforced
                    && matches!(
                        definition.coverage,
                        DefinitionCoverage::Scalar | DefinitionCoverage::WholeArray
                    )
                    && let Some(node) = node_at(
                        self.context.files[definition.file].parsed.tree(),
                        definition.value.range.start
                            - self.context.files[definition.file].byte_offset,
                        definition.value.range.end
                            - self.context.files[definition.file].byte_offset,
                    )
                {
                    values.push(self.expression(definition.file, node));
                }
            }
            if let Some(first) = values.first() {
                if values.iter().all(|v| v.presence == first.presence) {
                    value.presence = first.presence.clone();
                } else {
                    value.presence = Presence::Unknown;
                }
                if values.len() == 1 {
                    value.collection = first.collection.clone().or(value.collection);
                    value.conditions = first.conditions.clone();
                }
            }
            self.active.pop();
        }
        self.memo[id.0] = Some(value.clone());
        value
    }
    fn expression(&mut self, file: FileId, node: &SyntaxNode) -> Value {
        let range = node.range();
        let key = (file, range.start, range.end);
        if let Some(value) = self.expressions.get(&key) {
            return value.clone();
        }
        let children: Vec<_> = node.child_nodes().collect();
        let mut value = match node.kind() {
            NodeKind::ParenthesizedExpression
            | NodeKind::AnnotatedExpression
            | NodeKind::WhereFilter => children
                .first()
                .map(|n| self.expression(file, n))
                .unwrap_or_else(|| Value::scalar(Presence::Unknown)),
            NodeKind::Expression => {
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|t| t.kind == TokenKind::Absent)
                {
                    Value::scalar(Presence::Absent)
                } else if let Some(id) = self.reference(file, node) {
                    self.declaration(id)
                } else {
                    self.typed(file, node)
                }
            }
            NodeKind::ArrayLiteral => {
                let values: Vec<_> = children.iter().map(|n| self.expression(file, n)).collect();
                let capacity = Cardinality::Exact(values.len() as u64);
                let present = values.iter().fold(Cardinality::Exact(0), |n, v| {
                    add(n, presence_count(&v.presence, Cardinality::Exact(1)))
                });
                let element_presence = common_presence(values.iter().map(|v| &v.presence));
                Value {
                    presence: Presence::Present,
                    conditions: Vec::new(),
                    collection: Some(CollectionCardinality {
                        candidates: capacity.clone(),
                        capacity,
                        present,
                        element_presence,
                        conditions: Vec::new(),
                    }),
                }
            }
            NodeKind::ArrayComprehension
            | NodeKind::IndexedArrayComprehension
            | NodeKind::SetComprehension
            | NodeKind::GeneratorCallExpression => self.comprehension(file, node),
            NodeKind::ConditionalExpression => {
                let mut values = Vec::new();
                let mut conditions = Vec::new();
                let mut stopped = false;
                for branch in &children {
                    let parts: Vec<_> = branch.child_nodes().collect();
                    if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                        match self.truth(file, parts[0]) {
                            Some(false) => continue,
                            Some(true) => {
                                values.push(self.expression(file, parts[1]));
                                stopped = true;
                                break;
                            }
                            None => {
                                conditions.push(OptionalCondition {
                                    file,
                                    location: self.location(file, parts[0]),
                                    kind: OptionalConditionKind::Branch { expected: true },
                                });
                                values.push(self.expression(file, parts[1]));
                            }
                        }
                    } else if branch.kind() == NodeKind::ElseBranch
                        && let Some(body) = parts.last()
                    {
                        values.push(self.expression(file, body));
                        stopped = true;
                    }
                }
                if !stopped {
                    values.push(Value::scalar(Presence::Unknown));
                }
                Value {
                    presence: common_presence(values.iter().map(|v| &v.presence)),
                    collection: None,
                    conditions,
                }
            }
            NodeKind::UnaryExpression | NodeKind::BinaryExpression => {
                let optional = children
                    .iter()
                    .any(|n| self.ty(file, n).is_some_and(|t| t.optional));
                let operator = tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|t| t.kind);
                let name = operator.and_then(crate::bindings::symbolic_operator);
                let core = name.is_some_and(|name| self.core(file, node, name));
                if optional
                    && (!core
                        || !matches!(
                            operator,
                            Some(TokenKind::Plus | TokenKind::Star | TokenKind::Default)
                        ))
                {
                    self.unsupported(
                        file,
                        node,
                        "optional operator is outside presence interpretation",
                    )
                } else {
                    self.typed(file, node)
                }
            }
            NodeKind::CallExpression => {
                if ["occurs", "absent", "deopt"].iter().any(|name| {
                    core_optional_call(self.context, self.bindings, self.calls, file, node, name)
                }) {
                    Value::scalar(Presence::Present)
                } else if self.ty(file, node).is_some_and(|t| t.optional) {
                    self.unsupported(
                        file,
                        node,
                        "opaque optional callable presence is unsupported",
                    )
                } else {
                    self.typed(file, node)
                }
            }
            _ => self.typed(file, node),
        };
        if value.conditions.is_empty()
            && let Some(count) = &value.collection
        {
            value.conditions = count.conditions.clone();
        }
        self.expressions.insert(key, value.clone());
        if is_expression(node.kind()) {
            self.facts.expressions.push(OptionalExpression {
                file,
                location: self.location(file, node),
                presence: value.presence.clone(),
                conditions: value.conditions.clone(),
            });
            if let Some(cardinality) = &value.collection {
                for count in [
                    &cardinality.candidates,
                    &cardinality.capacity,
                    &cardinality.present,
                ] {
                    if let Cardinality::Unsupported(reason) = count
                        && !self.facts.limitations.iter().any(|l| {
                            l.file == file
                                && l.location.range == self.location(file, node).range
                                && l.reason == *reason
                        })
                    {
                        self.facts.limitations.push(OptionalLimitation {
                            file,
                            location: self.location(file, node),
                            reason: reason.clone(),
                        });
                    }
                }
                self.facts.collections.push(OptionalCollection {
                    file,
                    location: self.location(file, node),
                    cardinality: cardinality.clone(),
                });
            }
        }
        value
    }
    fn typed(&mut self, file: FileId, node: &SyntaxNode) -> Value {
        match self.ty(file, node) {
            Some(t) if t.known() && !t.optional => Value::scalar(Presence::Present),
            Some(t) if t.known() => Value::scalar(Presence::Conditional),
            _ => self.unsupported(
                file,
                node,
                "unavailable value type prevents presence interpretation",
            ),
        }
    }
    fn truth(&mut self, file: FileId, node: &SyntaxNode) -> Option<bool> {
        let node = unwrap(node);
        let operator = tokens(&self.context.files[file].parsed, node)
            .first()
            .map(|t| t.kind);
        if node.kind() == NodeKind::Expression {
            return match operator {
                Some(TokenKind::True) => Some(true),
                Some(TokenKind::False) => Some(false),
                _ => None,
            };
        }
        if node.kind() == NodeKind::CallExpression {
            for name in ["occurs", "absent"] {
                if core_optional_call(self.context, self.bindings, self.calls, file, node, name)
                    && let Some(arg) = node.child_nodes().next()
                {
                    return match self.expression(file, arg).presence {
                        Presence::Present => Some(name == "occurs"),
                        Presence::Absent => Some(name == "absent"),
                        _ => None,
                    };
                }
            }
        }
        if node.kind() == NodeKind::BinaryExpression {
            let children: Vec<_> = node.child_nodes().collect();
            if children.len() == 2
                && operator
                    .and_then(crate::bindings::symbolic_operator)
                    .is_some_and(|name| self.core(file, node, name))
            {
                let exact = |n: &SyntaxNode| {
                    let range = self.location(file, n).range;
                    self.numeric
                        .expressions
                        .iter()
                        .find(|e| e.file == file && e.location.range == range)
                        .and_then(|e| match e.outcome {
                            NumericOutcome::Exact(n) => Some(n),
                            _ => None,
                        })
                };
                if let (Some(a), Some(b)) = (exact(children[0]), exact(children[1])) {
                    return match operator {
                        Some(TokenKind::Equal | TokenKind::DoubleEqual) => Some(a == b),
                        Some(TokenKind::NotEqual) => Some(a != b),
                        Some(TokenKind::Less) => Some(a < b),
                        Some(TokenKind::LessEqual) => Some(a <= b),
                        Some(TokenKind::Greater) => Some(a > b),
                        Some(TokenKind::GreaterEqual) => Some(a >= b),
                        _ => None,
                    };
                }
            }
        }
        None
    }
    fn comprehension(&mut self, file: FileId, node: &SyntaxNode) -> Value {
        let Some(head) = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
        else {
            return self.unsupported(file, node, "comprehension head is missing");
        };
        let head = if head.kind() == NodeKind::IndexedArrayEntry {
            head.child_nodes().last().unwrap_or(head)
        } else {
            head
        };
        let mut candidates = Cardinality::Exact(1);
        let mut capacity = Cardinality::Exact(1);
        let mut conditions = Vec::new();
        let mut gated = false;
        let mut earlier = Vec::new();
        for list in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::GeneratorList)
        {
            for generator in list.child_nodes() {
                let Some(source) = generator.child_nodes().next() else {
                    continue;
                };
                let membership = tokens(&self.context.files[file].parsed, generator)
                    .iter()
                    .any(|t| t.kind == TokenKind::In);
                let declarations: Vec<_> = self
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
                let mut count = if membership {
                    self.source_count(file, source)
                } else {
                    Cardinality::Exact(1)
                };
                if self.references_any(file, source, &earlier) {
                    count = Cardinality::Unknown;
                }
                // `i,j in S` is an independent Cartesian product, not one
                // candidate per written generator node.
                for _ in 0..generator_slots(&self.context.files[file].parsed, generator) {
                    candidates = multiply(candidates, count.clone());
                    capacity = multiply(capacity, count.clone());
                }
                if membership
                    && self.inst(file, source) == Instantiation::Decision
                    && self
                        .ty(file, source)
                        .is_some_and(|t| matches!(t.kind, TypeKind::Set(_)))
                {
                    gated = true;
                    conditions.push(OptionalCondition {
                        file,
                        location: self.location(file, source),
                        kind: OptionalConditionKind::Membership {
                            declarations: declarations.clone(),
                        },
                    });
                }
                earlier.extend(declarations);
                for filter in generator
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let Some(condition) = filter.child_nodes().next() else {
                        continue;
                    };
                    conditions.push(OptionalCondition {
                        file,
                        location: self.location(file, condition),
                        kind: OptionalConditionKind::Filter,
                    });
                    if self.inst(file, condition) == Instantiation::Decision {
                        gated = true;
                    } else {
                        capacity = match self.truth(file, condition) {
                            Some(true) => capacity,
                            Some(false) => Cardinality::Exact(0),
                            None => uncertain_count(capacity),
                        };
                    }
                }
            }
        }
        let head_value = self.expression(file, head);
        let element_presence = if gated && head_value.presence != Presence::Absent {
            Presence::Conditional
        } else {
            head_value.presence
        };
        let mut counts = collection(candidates, capacity, element_presence, conditions.clone());
        if node.kind() == NodeKind::SetComprehension {
            counts.capacity = uncertain_count(counts.capacity);
            counts.present = uncertain_count(counts.present);
        }
        Value {
            presence: if node.kind() == NodeKind::GeneratorCallExpression {
                self.typed(file, node).presence
            } else {
                Presence::Present
            },
            collection: Some(counts),
            conditions,
        }
    }
    fn source_count(&mut self, file: FileId, node: &SyntaxNode) -> Cardinality {
        let node = unwrap(node);
        if node.kind() == NodeKind::Expression
            && let Some(id) = self.reference(file, node)
        {
            let ty = &self.calls.declarations[id.0].ty;
            if matches!(ty.kind, TypeKind::Array { .. }) {
                return self
                    .declaration(id)
                    .collection
                    .map_or(Cardinality::Unknown, |c| c.capacity);
            }
            if self.bindings.declarations[id.0].instantiation == Instantiation::Parameter {
                return Cardinality::Symbolic {
                    dependencies: vec![id],
                };
            }
            if let Domain::Set(elements) = &self.domains.declarations[id.0].domain {
                return domain_count(elements);
            }
        }
        if node.kind() == NodeKind::ArrayLiteral {
            return Cardinality::Exact(node.child_nodes().count() as u64);
        }
        if matches!(
            node.kind(),
            NodeKind::ArrayComprehension | NodeKind::IndexedArrayComprehension
        ) {
            return self
                .expression(file, node)
                .collection
                .map_or(Cardinality::Unknown, |c| c.capacity);
        }
        if let Err(reason) =
            crate::domains::core_arithmetic(self.context, self.bindings, self.calls, file, node)
        {
            return Cardinality::Unsupported(reason);
        }
        domain_count(&expression_domain(self.context, self.bindings, file, node))
    }
}
fn collection(
    candidates: Cardinality,
    capacity: Cardinality,
    element_presence: Presence,
    conditions: Vec<OptionalCondition>,
) -> CollectionCardinality {
    let present = presence_count(&element_presence, capacity.clone());
    CollectionCardinality {
        candidates,
        capacity,
        present,
        element_presence,
        conditions,
    }
}
fn presence_count(presence: &Presence, capacity: Cardinality) -> Cardinality {
    match presence {
        Presence::Present => capacity,
        Presence::Absent => Cardinality::Exact(0),
        Presence::Conditional | Presence::Unknown => uncertain_count(capacity),
        Presence::Unsupported(reason) => Cardinality::Unsupported(reason.clone()),
    }
}
fn uncertain_count(count: Cardinality) -> Cardinality {
    match count.bounds() {
        Some((_, 0)) => Cardinality::Exact(0),
        Some((_, upper)) => Cardinality::Bounds { lower: 0, upper },
        None => count,
    }
}
fn common_presence<'a>(mut values: impl Iterator<Item = &'a Presence>) -> Presence {
    let Some(first) = values.next() else {
        return Presence::Present;
    };
    let mut result = first.clone();
    for next in values {
        if result != *next {
            result = match (&result, next) {
                (Presence::Unsupported(r), _) | (_, Presence::Unsupported(r)) => {
                    Presence::Unsupported(r.clone())
                }
                (Presence::Unknown, _) | (_, Presence::Unknown) => Presence::Unknown,
                _ => Presence::Conditional,
            };
        }
    }
    result
}
fn multiply(a: Cardinality, b: Cardinality) -> Cardinality {
    count_math(a, b, true)
}
fn add(a: Cardinality, b: Cardinality) -> Cardinality {
    count_math(a, b, false)
}
fn count_math(a: Cardinality, b: Cardinality, product: bool) -> Cardinality {
    if product && (a == Cardinality::Exact(0) || b == Cardinality::Exact(0)) {
        return Cardinality::Exact(0);
    }
    if let (Some((al, au)), Some((bl, bu))) = (a.bounds(), b.bounds()) {
        let op = |a: u64, b: u64| {
            if product {
                a.checked_mul(b)
            } else {
                a.checked_add(b)
            }
        };
        return match (op(al, bl), op(au, bu)) {
            (Some(lower), Some(upper)) if lower == upper => Cardinality::Exact(lower),
            (Some(lower), Some(upper)) => Cardinality::Bounds { lower, upper },
            _ => Cardinality::Unsupported("collection count overflow".into()),
        };
    }
    if let Cardinality::Unsupported(r) = &a {
        return Cardinality::Unsupported(r.clone());
    }
    if let Cardinality::Unsupported(r) = &b {
        return Cardinality::Unsupported(r.clone());
    }
    if let (Cardinality::Symbolic { dependencies: a }, Cardinality::Symbolic { dependencies: b }) =
        (&a, &b)
    {
        let mut dependencies = a.clone();
        for id in b {
            if !dependencies.contains(id) {
                dependencies.push(*id);
            }
        }
        return Cardinality::Symbolic { dependencies };
    }
    match (&a, &b) {
        (Cardinality::Symbolic { .. }, Cardinality::Exact(_)) => a,
        (Cardinality::Exact(_), Cardinality::Symbolic { .. }) => b,
        _ => Cardinality::Unknown,
    }
}
fn bound_ids(bound: &crate::NumericBound, ids: &mut Vec<DeclarationId>) {
    match bound {
        crate::NumericBound::Symbol(id)
        | crate::NumericBound::Defined {
            declaration: id, ..
        } => {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        crate::NumericBound::Arithmetic { operands, .. } => {
            for n in operands {
                bound_ids(n, ids);
            }
        }
        _ => {}
    }
}
fn domain_count(domain: &Domain) -> Cardinality {
    match domain {
        Domain::Range { lower, upper } => {
            match (invariant_integer(lower), invariant_integer(upper)) {
                (Ok(Some(lo)), Ok(Some(hi))) => {
                    let count = (i128::from(hi) - i128::from(lo) + 1).max(0);
                    u64::try_from(count)
                        .map(Cardinality::Exact)
                        .unwrap_or_else(|_| {
                            Cardinality::Unsupported("collection count overflow".into())
                        })
                }
                (Err(reason), _) | (_, Err(reason)) => Cardinality::Unsupported(reason),
                _ => {
                    let mut dependencies = Vec::new();
                    bound_ids(lower, &mut dependencies);
                    bound_ids(upper, &mut dependencies);
                    if dependencies.is_empty() {
                        Cardinality::Unknown
                    } else {
                        Cardinality::Symbolic { dependencies }
                    }
                }
            }
        }
        Domain::LiteralSet(elements) => {
            let mut values = Vec::new();
            for element in elements {
                match invariant_integer(element) {
                    Ok(Some(n)) => {
                        if !values.contains(&n) {
                            values.push(n);
                        }
                    }
                    Ok(None) => return Cardinality::Unknown,
                    Err(r) => return Cardinality::Unsupported(r),
                }
            }
            Cardinality::Exact(values.len() as u64)
        }
        Domain::Named { declaration, .. } | Domain::Enum(declaration) => Cardinality::Symbolic {
            dependencies: vec![*declaration],
        },
        Domain::Set(element) => domain_count(element),
        Domain::Unsupported(reason) => Cardinality::Unsupported(reason.clone()),
        _ => Cardinality::Unknown,
    }
}
pub(super) fn core_optional_call(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &SyntaxNode,
    name: &str,
) -> bool {
    resolved_call(context, calls, file, node).is_some_and(|id| {
        let declaration = &bindings.declarations[id.0];
        declaration.name == name
            && matches!(
                declaration.role,
                DeclarationRole::Function | DeclarationRole::Test
            )
            && context.files[declaration.file].kind == SourceKind::StandardLibrary
            && context.files[declaration.file].implicit
    })
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
    ) {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
fn node_at(node: &SyntaxNode, start: usize, end: usize) -> Option<&SyntaxNode> {
    if node.range() == (start..end) && is_expression(node.kind()) {
        return Some(node);
    }
    node.child_nodes()
        .find_map(|child| node_at(child, start, end))
}
