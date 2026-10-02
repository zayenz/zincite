//! Guarded truth and operation obligations, independent of lint policy.
use crate::callables::{core_operation, is_expression};
use crate::definitions::{
    annotations_safe, core_callable, resolved_call, resolved_reference,
    transparent_boolean_argument,
};
use crate::domains::{
    bare_index_domain, expression_domain, index_domain_interval, intersect_index_domains,
    invariant_integer, resolved_index_domain, same_members, shift_index_domain, tokens,
};
use crate::{
    BindingFacts, CallableFacts, Cardinality, DeclarationId, DefinitionEnforcement, Domain,
    DomainFacts, FileId, Instantiation, InstantiationFacts, ModelContext, NumericFacts,
    NumericOutcome, OptionalFacts, Presence, SourceLocation, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};

/// A proved/refuted proposition, ordinary uncertainty, or missing interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardedOutcome {
    Proven,
    Refuted,
    Unknown,
    Unsupported(String),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardActivation {
    Active,
    Inactive,
    Conditional,
}
/// Branch definedness is gated by selection for var-if; only par-if is lazy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardEvaluation {
    Strict,
    ParameterBranch,
    DecisionBranch,
    AssertionReturn,
    GeneratorBody,
    /// Right operand evaluated when default needs its fallback.
    DefaultFallback,
}
#[derive(Clone, Debug)]
pub enum GuardAssumptionKind {
    Condition {
        expected: bool,
    },
    GeneratorMembership {
        declaration: DeclarationId,
        domain: Domain,
    },
}
#[derive(Clone, Debug)]
pub struct GuardAssumption {
    pub file: FileId,
    pub location: SourceLocation,
    pub kind: GuardAssumptionKind,
    /// Only an enforced root condition can supply a model-wide assumption.
    pub global: bool,
}
#[derive(Clone, Debug)]
pub struct GuardContext {
    /// Visible conditions and generator domains. A condition is unavailable
    /// while its own expression or operands are being evaluated; it cannot
    /// discharge its own precondition. Local conditions apply only on success.
    pub assumptions: Vec<GuardAssumption>,
    /// Reachability/selection; it does not erase a raw operation failure.
    pub activation: GuardActivation,
    pub evaluation: GuardEvaluation,
    pub enforcement: DefinitionEnforcement,
    pub nearest_boolean: Option<SourceLocation>,
}
#[derive(Clone, Debug)]
pub enum GuardObligationKind {
    Nonzero,
    /// deopt requires a present value, separately from operand evaluation.
    Presence,
    Index {
        array: DeclarationId,
        dimension: usize,
        domain: Domain,
        /// Scoped possible index membership, independent of per-value safety.
        selection: Option<GuardedIndexSpace>,
    },
    Nonempty {
        aggregate: String,
    },
    Assertion,
}
/// A scoped index space. Exact means supported membership/condition operations
/// retain its members; it does not prove a traversal is executed or nonempty.
#[derive(Clone, Debug)]
pub struct GuardedIndexSpace {
    pub domain: Domain,
    pub exact: bool,
}
#[derive(Clone, Debug)]
pub struct GuardObligation {
    pub file: FileId,
    pub operation: SourceLocation,
    pub operand: SourceLocation,
    pub kind: GuardObligationKind,
    /// Precondition outcome before scoped guards, independent of activation.
    pub invariant: GuardedOutcome,
    /// Precondition outcome under the listed assumptions; not whole totality.
    pub outcome: GuardedOutcome,
    pub context: GuardContext,
}
#[derive(Clone, Debug)]
pub struct GuardedExpression {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    /// Present for Boolean expressions, conditional on the explicit context.
    pub truth: Option<GuardedOutcome>,
    /// Evaluation before conversion at this expression's Boolean boundary.
    pub raw_definedness: GuardedOutcome,
    /// Whole value definedness. A relationally false Boolean can be defined
    /// even while its numeric operand has a refuted obligation.
    pub definedness: GuardedOutcome,
    pub numeric: Option<NumericOutcome>,
    /// Intrinsic presence when defined, supplied only by the option-aware entry.
    pub invariant_presence: Option<Presence>,
    /// Presence under this expression's explicit local/global assumptions.
    /// This does not discharge its operand evaluation or whole definedness.
    pub presence: Option<Presence>,
    pub context: GuardContext,
}
#[derive(Clone, Debug)]
pub struct GuardedLimitation {
    pub file: FileId,
    pub location: SourceLocation,
    pub reason: String,
}
#[derive(Debug, Default)]
pub struct GuardedFacts {
    pub expressions: Vec<GuardedExpression>,
    pub obligations: Vec<GuardObligation>,
    pub limitations: Vec<GuardedLimitation>,
}
impl GuardedFacts {
    pub fn expression(
        &self,
        file: FileId,
        location: &SourceLocation,
    ) -> Option<&GuardedExpression> {
        self.expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
    }
    pub fn obligations_at(
        &self,
        file: FileId,
        operation: &SourceLocation,
    ) -> impl Iterator<Item = &GuardObligation> {
        self.obligations
            .iter()
            .filter(move |o| o.file == file && o.operation.range == operation.range)
    }
}

/// Interpret actual branch/filter/enforced contexts without enabling a lint.
/// All prerequisites must belong to this retained ModelContext. Parameter data
/// and defaults are never substituted; source/declaration identity is retained.
///
/// Follows MiniZinc 2.10.1 [relational partiality](https://docs.minizinc.dev/en/2.10.1/modelling2.html#partiality-and-relational-semantics)
/// and [conditional/call semantics](https://docs.minizinc.dev/en/2.10.1/spec.html#if-then-else-expressions).
/// Boolean operands are strict; implication/disjunction/negation do not enforce
/// their operands. Undefined numeric values become false at their nearest
/// Boolean expression, while raw obligations remain visible. Parameter if is
/// lazy; var-if links definedness to the selected branch. Assert has a lazy
/// third argument and an aborting condition obligation, not relational false.
///
/// Supports core integer arithmetic/comparisons, membership, Boolean operators,
/// if/elseif/else, generator membership/filters, core forall and transparent
/// Boolean forwarding, scalar and written closed-range array-index dimensions,
/// div/mod, assertions and directly known nonoptional min/max/sum/product emptiness.
/// Index obligations also expose resolved enum types and scoped generator
/// membership shifted by a checked constant, narrowed by supported closed
/// membership/comparison guards. Exact membership differs from a numeric hull.
/// No option presence, general cardinality, arbitrary function bodies or solver
/// reasoning is added.
/// Opaque calls, unresolved/user operators, unsupported controls and unavailable
/// types retain located limitations. These facts never issue warnings or edits
/// and do not change existing definition or selected-lint safety behavior.
pub fn resolve_guarded_facts(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
) -> GuardedFacts {
    interpret(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        None,
    )
}

/// Apply actual guards to the same-context OptionalFacts prerequisite. Call
/// bindings/callables/instantiations/domains/definitions/numeric first, then
/// resolve_optional_facts, then this entry. The original entry remains useful
/// without option facts and retains its existing subset.
///
/// Supports resolved core occurs/absent/deopt, scalar default (absence and
/// undefinedness capture), integer optional +/* absence identities, and present
/// counts for min/max/sum/product/forall/exists. Other optional operators and
/// arbitrary callable bodies remain located limitations. A deopt obligation
/// retains its invariant outcome even in a locally proven or inactive branch.
/// Presence always means presence when defined; it is not a totality proof.
pub fn resolve_guarded_facts_with_options(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
    options: &OptionalFacts,
) -> GuardedFacts {
    interpret(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        Some(options),
    )
}

// Reuse the interpreter for one prospective iteration placement. The caller
// supplies the next generator source's original context, before that generator
// and its filters. This does not mutate the original expressions or obligations.
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_iteration_prefix(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    numeric: &NumericFacts,
    options: &OptionalFacts,
    candidate: &GuardedExpression,
    prefix: &GuardContext,
) -> Result<GuardedFacts, String> {
    let node = node_at(context, candidate.file, &candidate.location)
        .ok_or("candidate source is unavailable")?;
    let mut assumptions = Vec::new();
    for assumption in &prefix.assumptions {
        if assumption.file == candidate.file
            && (contains_range(&candidate.location.range, &assumption.location.range)
                || contains_range(&assumption.location.range, &candidate.location.range))
        {
            continue;
        }
        let source = node_at(context, assumption.file, &assumption.location)
            .ok_or("prefix assumption source is unavailable")?;
        assumptions.push(match &assumption.kind {
            GuardAssumptionKind::Condition { expected } => Assumption::Condition {
                file: assumption.file,
                node: source,
                expected: *expected,
                global: assumption.global,
            },
            GuardAssumptionKind::GeneratorMembership {
                declaration,
                domain,
            } => Assumption::Membership {
                file: assumption.file,
                source,
                declaration: *declaration,
                domain: domain.clone(),
            },
        });
    }
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        options: Some(options),
        facts: GuardedFacts::default(),
    };
    producer.walk(
        candidate.file,
        candidate.item,
        node,
        Scope {
            assumptions,
            activation: prefix.activation,
            evaluation: GuardEvaluation::Strict,
            enforcement: DefinitionEnforcement::Conditional,
            boolean: None,
        },
    );
    Ok(producer.facts)
}
pub(crate) fn node_at<'a>(
    context: &'a ModelContext,
    file: FileId,
    location: &SourceLocation,
) -> Option<&'a SyntaxNode> {
    fn find<'a>(node: &'a SyntaxNode, range: &std::ops::Range<usize>) -> Option<&'a SyntaxNode> {
        if node.range() == *range {
            return Some(node);
        }
        node.child_nodes().find_map(|c| find(c, range))
    }
    let source = &context.files[file];
    let range = location.range.start.checked_sub(source.byte_offset)?
        ..location.range.end.checked_sub(source.byte_offset)?;
    find(source.parsed.tree(), &range)
}
fn contains_range(outer: &std::ops::Range<usize>, inner: &std::ops::Range<usize>) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}
fn interpret<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    numeric: &'a NumericFacts,
    options: Option<&'a OptionalFacts>,
) -> GuardedFacts {
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        numeric,
        options,
        facts: GuardedFacts::default(),
    };
    let mut global = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if node.kind() == NodeKind::Constraint {
                for child in node.child_nodes().filter(|n| is_expression(n.kind())) {
                    producer.enforced_conditions(file, item, child, &mut global);
                }
            }
        }
    }
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            let scope = Scope {
                assumptions: global.clone(),
                activation: GuardActivation::Active,
                evaluation: GuardEvaluation::Strict,
                enforcement: DefinitionEnforcement::Conditional,
                boolean: None,
            };
            producer.walk(file, item, node, scope);
        }
    }
    producer.facts
}
#[derive(Clone)]
enum Assumption<'a> {
    Condition {
        file: FileId,
        node: &'a SyntaxNode,
        expected: bool,
        global: bool,
    },
    Membership {
        file: FileId,
        source: &'a SyntaxNode,
        declaration: DeclarationId,
        domain: Domain,
    },
}
#[derive(Clone)]
struct Scope<'a> {
    assumptions: Vec<Assumption<'a>>,
    activation: GuardActivation,
    evaluation: GuardEvaluation,
    enforcement: DefinitionEnforcement,
    boolean: Option<SourceLocation>,
}
#[derive(Clone)]
struct Evaluation {
    definedness: GuardedOutcome,
    truth: Option<GuardedOutcome>,
    numeric: Option<NumericOutcome>,
    /// A failed assertion aborts rather than becoming relational false.
    assertion: bool,
    presence: Option<Presence>,
}
impl Evaluation {
    fn total() -> Self {
        Self {
            definedness: GuardedOutcome::Proven,
            truth: None,
            numeric: None,
            assertion: false,
            presence: None,
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
    options: Option<&'a OptionalFacts>,
    facts: GuardedFacts,
}
impl<'a> Producer<'a> {
    fn location(&self, file: FileId, node: &SyntaxNode) -> SourceLocation {
        self.context.files[file].location(node.range())
    }
    fn context(&self, scope: &Scope<'a>) -> GuardContext {
        GuardContext {
            activation: scope.activation,
            evaluation: scope.evaluation,
            enforcement: scope.enforcement.clone(),
            nearest_boolean: scope.boolean.clone(),
            assumptions: scope
                .assumptions
                .iter()
                .map(|a| match a {
                    Assumption::Condition {
                        file,
                        node,
                        expected,
                        global,
                    } => GuardAssumption {
                        file: *file,
                        location: self.location(*file, node),
                        kind: GuardAssumptionKind::Condition {
                            expected: *expected,
                        },
                        global: *global,
                    },
                    Assumption::Membership {
                        file,
                        source,
                        declaration,
                        domain,
                    } => GuardAssumption {
                        file: *file,
                        location: self.location(*file, source),
                        kind: GuardAssumptionKind::GeneratorMembership {
                            declaration: *declaration,
                            domain: domain.clone(),
                        },
                        global: false,
                    },
                })
                .collect(),
        }
    }
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&crate::TypeInst> {
        let range = self.location(file, node).range;
        self.calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    }
    fn instantiation(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        let range = self.location(file, node).range;
        self.instantiations
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map_or(Instantiation::Unknown, |e| e.instantiation)
    }
    fn numeric(&self, file: FileId, node: &SyntaxNode) -> Option<NumericOutcome> {
        let range = self.location(file, node).range;
        self.numeric
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| e.outcome.clone())
    }
    fn operator(&self, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
        tokens(&self.context.files[file].parsed, node)
            .first()
            .map(|t| t.kind)
    }
    fn core_operator(&self, file: FileId, node: &SyntaxNode) -> Result<(), String> {
        let name = self
            .operator(file, node)
            .and_then(crate::bindings::symbolic_operator)
            .ok_or("operator is outside guarded interpretation")?;
        if core_operation(self.context, self.bindings, self.calls, file, node, name)? {
            Ok(())
        } else {
            Err("user operator is outside guarded interpretation".into())
        }
    }
    fn core_call(&self, file: FileId, node: &SyntaxNode, name: &str) -> bool {
        resolved_call(self.context, self.calls, file, node)
            .is_some_and(|id| core_callable(self.context, self.bindings, id, name))
    }
    fn assume(
        &self,
        out: &mut Vec<Assumption<'a>>,
        file: FileId,
        node: &'a SyntaxNode,
        expected: bool,
        global: bool,
    ) {
        out.push(Assumption::Condition {
            file,
            node,
            expected,
            global,
        });
        let unwrapped = unwrap(node);
        if expected
            && unwrapped.kind() == NodeKind::BinaryExpression
            && self.operator(file, unwrapped) == Some(TokenKind::And)
            && self.core_operator(file, unwrapped).is_ok()
        {
            for child in unwrapped.child_nodes() {
                self.assume(out, file, child, true, global);
            }
        }
    }
    fn enforced_conditions(
        &self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        out: &mut Vec<Assumption<'a>>,
    ) {
        if !annotations_safe(self.context, file, node) {
            return;
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression => {
                if let Some(child) = children.first() {
                    self.enforced_conditions(file, item, child, out);
                }
            }
            NodeKind::BinaryExpression
                if self.operator(file, node) == Some(TokenKind::And)
                    && self.core_operator(file, node).is_ok() =>
            {
                for child in children {
                    self.enforced_conditions(file, item, child, out);
                }
            }
            NodeKind::CallExpression if self.core_call(file, node, "assert") => {
                if let Some(condition) = children.first() {
                    self.assume(out, file, condition, true, true);
                }
            }
            NodeKind::CallExpression
                if self.options.is_some()
                    && ["occurs", "absent"]
                        .iter()
                        .any(|name| self.optional_call(file, node, name)) =>
            {
                self.assume(out, file, node, true, true);
            }
            NodeKind::UnaryExpression
                if self.options.is_some()
                    && self.operator(file, node) == Some(TokenKind::Not)
                    && self.core_operator(file, node).is_ok() =>
            {
                if let Some(child) = children.first()
                    && ["occurs", "absent"]
                        .iter()
                        .any(|name| self.optional_call(file, unwrap(child), name))
                {
                    self.assume(out, file, child, false, true);
                }
            }
            NodeKind::CallExpression => {
                if let Some(id) = resolved_call(self.context, self.calls, file, node)
                    && let Some((f, i, argument)) = transparent_boolean_argument(
                        self.context,
                        self.bindings,
                        self.calls,
                        (file, item, node),
                        id,
                    )
                {
                    self.enforced_conditions(f, i, argument, out);
                }
            }
            NodeKind::BinaryExpression
                if matches!(
                    self.operator(file, node),
                    Some(
                        TokenKind::Equal
                            | TokenKind::DoubleEqual
                            | TokenKind::NotEqual
                            | TokenKind::Less
                            | TokenKind::LessEqual
                            | TokenKind::Greater
                            | TokenKind::GreaterEqual
                            | TokenKind::In
                    )
                ) && self.core_operator(file, node).is_ok() =>
            {
                out.push(Assumption::Condition {
                    file,
                    node,
                    expected: true,
                    global: true,
                });
            }
            _ => {}
        }
    }
    fn unsupported(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        reason: impl Into<String>,
    ) -> Evaluation {
        let reason = reason.into();
        self.facts.limitations.push(GuardedLimitation {
            file,
            location: self.location(file, node),
            reason: reason.clone(),
        });
        Evaluation {
            definedness: GuardedOutcome::Unsupported(reason),
            ..Evaluation::total()
        }
    }
    fn walk(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        mut scope: Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        let is_boolean = self
            .ty(file, node)
            .is_some_and(|ty| ty.kind == TypeKind::Bool && !ty.optional);
        if is_boolean {
            scope.boolean = Some(self.location(file, node));
        }
        let mut result = match node.kind() {
            NodeKind::Constraint => {
                scope.enforcement = if annotations_safe(self.context, file, node) {
                    DefinitionEnforcement::Enforced
                } else {
                    self.unsupported(
                        file,
                        node,
                        "constraint annotation enforcement is unsupported",
                    );
                    DefinitionEnforcement::Unsupported(
                        "constraint annotation enforcement is unsupported".into(),
                    )
                };
                let values: Vec<_> = children
                    .iter()
                    .filter(|n| is_expression(n.kind()))
                    .map(|n| self.walk(file, item, n, scope.clone()))
                    .collect();
                combine(&values)
            }
            NodeKind::AnnotatedExpression if !annotations_safe(self.context, file, node) => {
                scope.enforcement = DefinitionEnforcement::Unsupported(
                    "annotation enforcement is unsupported".into(),
                );
                for child in children.iter().filter(|n| is_expression(n.kind())) {
                    self.walk(file, item, child, scope.clone());
                }
                self.unsupported(file, node, "annotation enforcement is unsupported")
            }
            NodeKind::ParenthesizedExpression
            | NodeKind::AnnotatedExpression
            | NodeKind::NamedArgument => children
                .first()
                .map(|n| self.walk(file, item, n, scope.clone()))
                .unwrap_or_else(Evaluation::total),
            NodeKind::Expression => {
                let mut result = Evaluation::total();
                result.numeric = self.numeric(file, node);
                if is_boolean {
                    result.truth = Some(match self.operator(file, node) {
                        Some(TokenKind::True) => GuardedOutcome::Proven,
                        Some(TokenKind::False) => GuardedOutcome::Refuted,
                        _ => self.assumed_truth(file, node, &scope),
                    });
                }
                if self.ty(file, node).is_some_and(|ty| {
                    self.options.is_none() && crate::value_safety::optional(ty)
                        || matches!(ty.kind, TypeKind::Unknown(_))
                }) {
                    self.unsupported(
                        file,
                        node,
                        "optional or unavailable value type requires additional facts",
                    )
                } else {
                    result
                }
            }
            NodeKind::ConditionalExpression => self.conditional(file, item, node, &scope),
            NodeKind::ArrayAccessExpression => self.access(file, item, node, &scope),
            NodeKind::UnaryExpression | NodeKind::BinaryExpression => {
                self.operation(file, item, node, &scope)
            }
            NodeKind::CallExpression => self.call(file, item, node, &scope),
            NodeKind::GeneratorCallExpression
            | NodeKind::ArrayComprehension
            | NodeKind::SetComprehension
            | NodeKind::IndexedArrayComprehension => self.generated(file, item, node, &scope),
            NodeKind::LetExpression | NodeKind::FieldAccessExpression => {
                for child in children {
                    self.walk(file, item, child, scope.clone());
                }
                self.unsupported(
                    file,
                    node,
                    "control or structured projection is outside guarded interpretation",
                )
            }
            _ => {
                let mut child_scope = scope.clone();
                child_scope.enforcement = DefinitionEnforcement::Conditional;
                let values: Vec<_> = children
                    .iter()
                    .map(|n| self.walk(file, item, n, child_scope.clone()))
                    .collect();
                combine(&values)
            }
        };
        let invariant_presence = self.intrinsic_presence(file, node);
        if result.presence.is_none() && self.options.is_some() {
            result.presence = Some(self.scoped_presence(file, node, &scope));
        }
        let raw = result.definedness.clone();
        if is_boolean && !result.assertion {
            match &raw {
                GuardedOutcome::Refuted => {
                    result.truth = Some(GuardedOutcome::Refuted);
                    result.definedness = GuardedOutcome::Proven;
                }
                GuardedOutcome::Unknown => {
                    result.truth = Some(GuardedOutcome::Unknown);
                    result.definedness = GuardedOutcome::Proven;
                }
                GuardedOutcome::Unsupported(reason) => {
                    result.truth = Some(GuardedOutcome::Unsupported(reason.clone()))
                }
                GuardedOutcome::Proven => {}
            }
        }
        if is_boolean && result.truth.is_none() {
            result.truth = Some(GuardedOutcome::Unknown);
        }
        if is_expression(node.kind()) {
            self.facts.expressions.push(GuardedExpression {
                file,
                item,
                location: self.location(file, node),
                truth: result.truth.clone(),
                raw_definedness: raw,
                definedness: result.definedness.clone(),
                numeric: result.numeric.clone(),
                invariant_presence,
                presence: result.presence.clone(),
                context: self.context(&scope),
            });
        }
        result
    }
    fn conditional(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let mut earlier = scope.clone();
        earlier.enforcement = DefinitionEnforcement::Conditional;
        let mut conditions = Vec::new();
        let mut alternatives = Vec::new();
        let decision = node.child_nodes().any(|branch| {
            branch.kind() == NodeKind::ConditionalBranch
                && branch.child_nodes().next().is_some_and(|condition| {
                    self.instantiation(file, condition) == Instantiation::Decision
                })
        });
        let mut unknown_guard = false;
        let mut has_else = false;
        for branch in node.child_nodes() {
            let children: Vec<_> = branch.child_nodes().collect();
            let (body, mut own) =
                if branch.kind() == NodeKind::ConditionalBranch && children.len() == 2 {
                    let condition = children[0];
                    let evaluated = self.walk(file, item, condition, earlier.clone());
                    let truth = evaluated.truth.clone().unwrap_or(GuardedOutcome::Unknown);
                    let inst = self.instantiation(file, condition);
                    if inst == Instantiation::Unknown {
                        unknown_guard = true;
                        self.unsupported(
                            file,
                            condition,
                            "conditional guard instantiation is unavailable",
                        );
                    }
                    conditions.push((earlier.activation, evaluated));
                    let mut own = earlier.clone();
                    own.activation = activate(own.activation, &truth, true);
                    self.assume(&mut own.assumptions, file, condition, true, false);
                    earlier.activation = activate(earlier.activation, &truth, false);
                    self.assume(&mut earlier.assumptions, file, condition, false, false);
                    (children[1], own)
                } else if branch.kind() == NodeKind::ElseBranch && !children.is_empty() {
                    has_else = true;
                    (*children.last().unwrap(), earlier.clone())
                } else {
                    return self.unsupported(file, node, "conditional branch form is unsupported");
                };
            own.evaluation = if decision {
                GuardEvaluation::DecisionBranch
            } else {
                GuardEvaluation::ParameterBranch
            };
            alternatives.push((own.activation, self.walk(file, item, body, own)));
        }
        if !has_else
            && self
                .ty(file, node)
                .is_some_and(|t| t.kind == TypeKind::Bool)
        {
            alternatives.push((
                earlier.activation,
                Evaluation {
                    truth: Some(GuardedOutcome::Refuted),
                    ..Evaluation::total()
                },
            ));
        }
        let selected: Vec<_> = alternatives
            .iter()
            .filter(|(a, _)| *a != GuardActivation::Inactive)
            .map(|(_, v)| v.clone())
            .collect();
        let mut result = if selected.is_empty() {
            Evaluation::total()
        } else {
            let mut result = selected[0].clone();
            if selected.iter().any(|v| v.definedness != result.definedness) {
                result.definedness = GuardedOutcome::Unknown;
            }
            if selected.iter().any(|v| v.truth != result.truth) {
                result.truth = Some(GuardedOutcome::Unknown);
            }
            if selected.iter().any(|v| v.numeric != result.numeric) {
                result.numeric = None;
            }
            if selected.iter().any(|v| v.presence != result.presence) {
                result.presence = self.intrinsic_presence(file, node);
            }
            result.assertion = selected.iter().any(|v| v.assertion);
            result
        };
        if unknown_guard {
            result.definedness = GuardedOutcome::Unsupported(
                "conditional guard instantiation is unavailable".into(),
            );
        }
        let mut condition_result = Evaluation::total();
        for (activation, value) in &conditions {
            if *activation == GuardActivation::Inactive && !decision {
                continue;
            }
            let definedness = if *activation == GuardActivation::Conditional
                && value.definedness != GuardedOutcome::Proven
            {
                GuardedOutcome::Unknown
            } else {
                value.definedness.clone()
            };
            condition_result.definedness = conjoin(&condition_result.definedness, &definedness);
            condition_result.assertion |= value.assertion;
        }
        result.definedness = conjoin(&result.definedness, &condition_result.definedness);
        result.assertion |= condition_result.assertion;
        if decision {
            for (_, value) in &alternatives {
                if value.assertion {
                    result.definedness = conjoin(&result.definedness, &value.definedness);
                    result.assertion = true;
                }
            }
        }
        result
    }
    fn operation(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        if self.options.is_some() && self.operator(file, node) == Some(TokenKind::Default) {
            return self.default_value(file, item, node, scope);
        }
        let mut own = scope.clone();
        if self.operator(file, node) != Some(TokenKind::And) {
            own.enforcement = DefinitionEnforcement::Conditional;
        } else if let Err(reason) = self.core_operator(file, node) {
            own.enforcement = DefinitionEnforcement::Unsupported(reason);
        }
        let values: Vec<_> = children
            .iter()
            .map(|n| self.walk(file, item, n, own.clone()))
            .collect();
        if let Err(reason) = self.core_operator(file, node) {
            return self.unsupported(file, node, reason);
        }
        let mut result = combine(&values);
        result.numeric = self.numeric(file, node);
        let Some(operator) = self.operator(file, node) else {
            return self.unsupported(file, node, "operator syntax is unavailable");
        };
        if self.options.is_some()
            && children
                .iter()
                .any(|n| self.ty(file, n).is_some_and(|t| t.optional))
        {
            if !matches!(operator, TokenKind::Plus | TokenKind::Star)
                || children.len() != 2
                || !self.ty(file, node).is_some_and(|t| t.kind == TypeKind::Int)
            {
                return self.unsupported(
                    file,
                    node,
                    "optional operator is outside guarded interpretation",
                );
            }
            result.numeric = None;
            let identity = if operator == TokenKind::Plus { 0 } else { 1 };
            let exact: Vec<_> = children
                .iter()
                .zip(&values)
                .map(|(n, v)| {
                    if self.scoped_presence(file, n, scope) == Presence::Absent {
                        Some(identity)
                    } else {
                        match v.numeric {
                            Some(NumericOutcome::Exact(n)) => Some(n),
                            _ => None,
                        }
                    }
                })
                .collect();
            if let [Some(a), Some(b)] = exact.as_slice() {
                let value = if operator == TokenKind::Plus {
                    a.checked_add(*b)
                } else {
                    a.checked_mul(*b)
                };
                let Some(value) = value else {
                    return self.unsupported(file, node, "checked optional arithmetic overflow");
                };
                result.numeric = Some(NumericOutcome::Exact(value));
            }
            result.presence = Some(Presence::Present);
            return result;
        }
        if matches!(operator, TokenKind::Div | TokenKind::Mod) && children.len() == 2 {
            let invariant = self.nonzero(file, children[1]);
            let outcome = if invariant == GuardedOutcome::Unknown
                && self.guarded_nonzero(file, children[1], scope)
            {
                GuardedOutcome::Proven
            } else {
                invariant.clone()
            };
            self.obligation(
                file,
                node,
                children[1],
                GuardObligationKind::Nonzero,
                (invariant, outcome.clone()),
                scope,
            );
            result.definedness = conjoin(&result.definedness, &outcome);
            if outcome == GuardedOutcome::Proven
                && let Some(NumericOutcome::Unsupported(reason)) = &result.numeric
            {
                return self.unsupported(file, node, reason.clone());
            }
        } else if matches!(
            operator,
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star
        ) {
            if let Some(NumericOutcome::Unsupported(reason)) = &result.numeric {
                return self.unsupported(file, node, reason.clone());
            }
        } else if matches!(
            operator,
            TokenKind::And
                | TokenKind::Or
                | TokenKind::Implies
                | TokenKind::ReverseImplies
                | TokenKind::Equivalence
                | TokenKind::Xor
                | TokenKind::Not
        ) {
            result.truth = Some(logical(operator, &values));
            if matches!(operator, TokenKind::And | TokenKind::Or)
                && children.len() == 2
                && values
                    .iter()
                    .all(|v| v.definedness == GuardedOutcome::Proven)
                && self.boolean_complements(file, children[0], children[1])
            {
                result.truth = Some(if operator == TokenKind::And {
                    GuardedOutcome::Refuted
                } else {
                    GuardedOutcome::Proven
                });
            }
        } else if children.len() == 2
            && matches!(
                operator,
                TokenKind::Equal
                    | TokenKind::DoubleEqual
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
                    | TokenKind::In
            )
        {
            result.truth = Some(if operator == TokenKind::In {
                self.membership(file, children[0], &self.domain(file, children[1]), scope)
            } else {
                self.comparison(file, children[0], children[1], operator, &values)
            });
            if result.truth == Some(GuardedOutcome::Unknown) {
                result.truth = Some(self.assumed_truth(file, node, scope));
            }
        } else {
            return self.unsupported(
                file,
                node,
                "operator is outside guarded arithmetic/Boolean interpretation",
            );
        }
        result
    }
    // Total Boolean operands may have crossed a relational boundary. Their
    // raw numeric obligations remain in the facts; this identity permits no edits.
    fn boolean_complements(&self, file: FileId, left: &SyntaxNode, right: &SyntaxNode) -> bool {
        if ![left, right]
            .iter()
            .all(|n| self.ty(file, n).is_some_and(|t| t.kind == TypeKind::Bool))
        {
            return false;
        }
        [(left, right), (right, left)]
            .iter()
            .any(|(subject, negated)| {
                let negated = unwrap(negated);
                negated.kind() == NodeKind::UnaryExpression
                    && self.operator(file, negated) == Some(TokenKind::Not)
                    && self.core_operator(file, negated).is_ok()
                    && negated
                        .child_nodes()
                        .next()
                        .is_some_and(|n| self.same(file, subject, file, n))
            })
    }
    fn access(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        let values: Vec<_> = children
            .iter()
            .map(|n| self.walk(file, item, n, scope.clone()))
            .collect();
        let Some(array) = children
            .first()
            .and_then(|n| resolved_reference(self.context, self.bindings, file, unwrap(n)))
        else {
            return self.unsupported(file, node, "array declaration identity is unavailable");
        };
        let mut domain = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let Domain::Array { indices, .. } = domain else {
            return self.unsupported(file, node, "array index domains are unavailable");
        };
        if children.len() != indices.len() + 1 {
            return self.unsupported(
                file,
                node,
                "sliced or mismatched array dimensions are unsupported",
            );
        }
        if children[1..].iter().any(|index| {
            let index = unwrap(index);
            index.kind() == NodeKind::RangeExpression && index.child_nodes().count() != 2
        }) {
            return self.unsupported(
                file,
                node,
                "sliced dimensions with omitted bounds are unsupported",
            );
        }
        let domain_error = self.array_domain_error(array);
        let mut result = combine(&values);
        result.numeric = None;
        for (dimension, (index, domain)) in children[1..].iter().zip(indices).enumerate() {
            let invariant = domain_error.as_ref().map_or_else(
                || self.invariant_membership(file, index, domain),
                |reason| GuardedOutcome::Unsupported(reason.clone()),
            );
            let selection = self.index_space(file, index, scope);
            let outcome = if invariant == GuardedOutcome::Unknown
                && (self.guarded_membership(file, index, domain, scope)
                    || selection.as_ref().is_some_and(|s| {
                        s.exact
                            && !self.replaceable_set(domain)
                            && intersect_index_domains(&s.domain, domain)
                                .is_some_and(|d| same_members(&s.domain, &d) == Ok(Some(true)))
                    })) {
                GuardedOutcome::Proven
            } else {
                invariant.clone()
            };
            self.obligation(
                file,
                node,
                index,
                GuardObligationKind::Index {
                    array,
                    dimension: dimension + 1,
                    domain: domain.clone(),
                    selection,
                },
                (invariant, outcome.clone()),
                scope,
            );
            result.definedness = conjoin(&result.definedness, &outcome);
        }
        result
    }
    fn call(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        if children.iter().any(|n| n.kind() == NodeKind::NamedArgument) {
            for child in &children {
                self.walk(file, item, child, scope.clone());
            }
            return self.unsupported(
                file,
                node,
                "named callable arguments are outside guarded interpretation",
            );
        }
        if self.options.is_some()
            && ["occurs", "absent", "deopt"]
                .iter()
                .any(|name| self.optional_call(file, node, name))
        {
            return self.option_call(file, item, node, scope);
        }
        if self.core_call(file, node, "assert") && matches!(children.len(), 2 | 3) {
            let condition = self.walk(file, item, children[0], scope.clone());
            let invariant = self.invariant_truth(file, children[0]);
            let outcome = condition.truth.clone().unwrap_or(GuardedOutcome::Unknown);
            self.obligation(
                file,
                node,
                children[0],
                GuardObligationKind::Assertion,
                (invariant, outcome.clone()),
                scope,
            );
            let message = self.walk(file, item, children[1], scope.clone());
            let mut result = combine(&[condition, message]);
            result.assertion = true;
            result.definedness = conjoin(&result.definedness, &outcome);
            if children.len() == 3 {
                let mut own = scope.clone();
                own.evaluation = GuardEvaluation::AssertionReturn;
                own.enforcement = DefinitionEnforcement::Conditional;
                own.activation = activate(own.activation, &outcome, true);
                self.assume(&mut own.assumptions, file, children[0], true, false);
                let value = self.walk(file, item, children[2], own);
                if outcome != GuardedOutcome::Refuted {
                    result.definedness = conjoin(&result.definedness, &value.definedness);
                }
                result.truth = value.truth;
                result.numeric = value.numeric;
            } else {
                result.truth = Some(outcome);
            }
            return result;
        }
        if let Some(id) = resolved_call(self.context, self.calls, file, node)
            && let Some((f, i, argument)) = transparent_boolean_argument(
                self.context,
                self.bindings,
                self.calls,
                (file, item, node),
                id,
            )
        {
            let forwarded = self.walk(f, i, argument, scope.clone());
            let mut values = vec![forwarded.clone()];
            for child in children {
                if f != file || child.range() != argument.range() {
                    let mut own = scope.clone();
                    own.enforcement = DefinitionEnforcement::Conditional;
                    values.push(self.walk(file, item, child, own));
                }
            }
            let mut result = combine(&values);
            result.truth = forwarded.truth;
            return result;
        }
        let mut own = scope.clone();
        if !self.core_call(file, node, "forall") {
            own.enforcement = DefinitionEnforcement::Conditional;
        }
        let values: Vec<_> = children
            .iter()
            .map(|n| self.walk(file, item, n, own.clone()))
            .collect();
        let mut result = combine(&values);
        if self.core_call(file, node, "bool2int") {
            if children.len() != 1
                || !self
                    .ty(file, children[0])
                    .is_some_and(|t| !t.optional && t.kind == TypeKind::Bool)
                || !self
                    .ty(file, node)
                    .is_some_and(|t| !t.optional && t.kind == TypeKind::Int)
            {
                return self.unsupported(
                    file,
                    node,
                    "only scalar nonoptional bool2int is interpreted",
                );
            }
            // Boolean relational totality belongs to this argument. Its raw
            // numeric obligations remain separate in the argument's facts.
            if result.definedness == GuardedOutcome::Proven {
                result.numeric = Some(match values[0].truth {
                    Some(GuardedOutcome::Proven) => NumericOutcome::Exact(1),
                    Some(GuardedOutcome::Refuted) => NumericOutcome::Exact(0),
                    _ => NumericOutcome::Interval { lower: 0, upper: 1 },
                });
            }
            return result;
        }
        for name in [
            "min", "max", "sum", "product", "forall", "exists", "length", "card",
        ] {
            if !self.core_call(file, node, name) {
                continue;
            }
            if children.len() != 1 {
                return self.unsupported(
                    file,
                    node,
                    "aggregate arity is outside supported interpretation",
                );
            }
            let nonempty = self
                .collection_nonempty(file, children[0], name)
                .unwrap_or_else(|| self.nonempty(file, children[0]));
            if matches!(name, "min" | "max") {
                let scoped = if nonempty == GuardedOutcome::Unknown
                    && (self.options.is_none()
                        || !self
                            .ty(file, children[0])
                            .is_some_and(crate::value_safety::optional))
                    && self.guarded_nonempty(file, children[0], scope)
                {
                    GuardedOutcome::Proven
                } else {
                    nonempty.clone()
                };
                self.obligation(
                    file,
                    node,
                    children[0],
                    GuardObligationKind::Nonempty {
                        aggregate: name.into(),
                    },
                    (nonempty.clone(), scoped.clone()),
                    scope,
                );
                result.definedness = conjoin(&result.definedness, &scoped);
            } else if nonempty == GuardedOutcome::Refuted {
                match name {
                    "sum" | "length" | "card" => result.numeric = Some(NumericOutcome::Exact(0)),
                    "product" => result.numeric = Some(NumericOutcome::Exact(1)),
                    "forall" => result.truth = Some(GuardedOutcome::Proven),
                    "exists" => result.truth = Some(GuardedOutcome::Refuted),
                    _ => {}
                }
            }
            if let GuardedOutcome::Unsupported(reason) = nonempty {
                return self.unsupported(file, node, reason);
            }
            return result;
        }
        if self.index_domain(file, node).is_some() {
            return result;
        }
        self.unsupported(
            file,
            node,
            "opaque or unresolved callable is outside guarded interpretation",
        )
    }
    fn generated(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let mut own = scope.clone();
        own.evaluation = GuardEvaluation::GeneratorBody;
        let core_forall = self.core_call(file, node, "forall");
        if node.kind() == NodeKind::GeneratorCallExpression && !core_forall {
            own.enforcement = DefinitionEnforcement::Conditional;
        }
        let mut inputs = Vec::new();
        let mut empty = false;
        let mut filtered = false;
        let Some(list) = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
        else {
            return self.unsupported(file, node, "generator list is unavailable");
        };
        for generator in list.child_nodes() {
            let Some(source) = generator.child_nodes().next() else {
                continue;
            };
            let mut source_scope = own.clone();
            source_scope.evaluation = GuardEvaluation::Strict;
            source_scope.enforcement = DefinitionEnforcement::Conditional;
            let source_value = self.walk(file, item, source, source_scope);
            inputs.push(evaluated_when(source_value, own.activation));
            let domain = self.domain(file, source);
            let is_membership = tokens(&self.context.files[file].parsed, generator)
                .iter()
                .any(|t| t.kind == TokenKind::In);
            if is_membership {
                let nonempty = self.domain_nonempty(&domain);
                if let GuardedOutcome::Unsupported(reason) = &nonempty {
                    let unavailable = self.unsupported(file, source, reason.clone());
                    inputs.push(evaluated_when(unavailable, own.activation));
                }
                empty |= nonempty == GuardedOutcome::Refuted;
                if empty {
                    own.activation = GuardActivation::Inactive;
                } else if own.activation != GuardActivation::Inactive
                    && nonempty != GuardedOutcome::Proven
                {
                    own.activation = GuardActivation::Conditional;
                }
                for declaration in &self.bindings.declarations {
                    if declaration.file == file
                        && declaration.role == crate::DeclarationRole::Generator
                        && declaration.syntax_range == generator.range()
                    {
                        own.assumptions.push(Assumption::Membership {
                            file,
                            source,
                            declaration: declaration.id,
                            domain: domain.clone(),
                        });
                    }
                }
            } else {
                inputs.push(self.unsupported(
                    file,
                    generator,
                    "assignment generator value propagation is unsupported",
                ));
            }
            for filter in generator
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::WhereFilter)
            {
                if let Some(condition) = filter.child_nodes().next() {
                    filtered = true;
                    let mut filter_scope = own.clone();
                    filter_scope.enforcement = DefinitionEnforcement::Conditional;
                    let value = self.walk(file, item, condition, filter_scope);
                    let truth = value.truth.clone().unwrap_or(GuardedOutcome::Unknown);
                    inputs.push(evaluated_when(value, own.activation));
                    own.activation = activate(own.activation, &truth, true);
                    self.assume(&mut own.assumptions, file, condition, true, false);
                }
            }
        }
        let mut bodies = Vec::new();
        for body in node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::GeneratorList)
        {
            bodies.push(self.walk(file, item, body, own.clone()));
        }
        let mut result = combine(&inputs);
        if own.activation != GuardActivation::Inactive {
            let body = if self.options.is_some() {
                bodies
                    .iter()
                    .cloned()
                    .map(|b| evaluated_when(b, own.activation))
                    .collect::<Vec<_>>()
            } else {
                bodies.clone()
            };
            result.definedness = conjoin(&result.definedness, &combine(&body).definedness);
        }
        if node.kind() == NodeKind::GeneratorCallExpression {
            let present_nonempty = self.collection_nonempty(file, node, "sum");
            if empty
                || own.activation == GuardActivation::Inactive
                || present_nonempty == Some(GuardedOutcome::Refuted)
            {
                if core_forall {
                    result.truth = Some(GuardedOutcome::Proven);
                } else if self.core_call(file, node, "exists") {
                    result.truth = Some(GuardedOutcome::Refuted);
                } else if self.core_call(file, node, "sum") {
                    result.numeric = Some(NumericOutcome::Exact(0));
                } else if self.core_call(file, node, "product") {
                    result.numeric = Some(NumericOutcome::Exact(1));
                }
            }
            if self.core_call(file, node, "min") || self.core_call(file, node, "max") {
                let outcome = if let Some(present) = present_nonempty {
                    present
                } else if empty || own.activation == GuardActivation::Inactive {
                    GuardedOutcome::Refuted
                } else if filtered || own.activation == GuardActivation::Conditional {
                    GuardedOutcome::Unknown
                } else {
                    GuardedOutcome::Proven
                };
                self.obligation(
                    file,
                    node,
                    node,
                    GuardObligationKind::Nonempty {
                        aggregate: if self.core_call(file, node, "min") {
                            "min".into()
                        } else {
                            "max".into()
                        },
                    },
                    (outcome.clone(), outcome.clone()),
                    scope,
                );
                result.definedness = conjoin(&result.definedness, &outcome);
            } else if core_forall
                && bodies
                    .iter()
                    .all(|b| b.truth == Some(GuardedOutcome::Proven))
            {
                result.truth = Some(GuardedOutcome::Proven);
            }
            if !["forall", "exists", "min", "max", "sum", "product"]
                .iter()
                .any(|name| self.core_call(file, node, name))
            {
                return self.unsupported(
                    file,
                    node,
                    "opaque generator call is outside guarded interpretation",
                );
            }
        }
        result
    }
    fn optional_call(&self, file: FileId, node: &SyntaxNode, name: &str) -> bool {
        crate::optional::core_optional_call(
            self.context,
            self.bindings,
            self.calls,
            file,
            node,
            name,
        )
    }
    fn intrinsic_presence(&self, file: FileId, node: &SyntaxNode) -> Option<Presence> {
        self.options
            .and_then(|facts| facts.expression(file, &self.location(file, node)))
            .map(|e| e.presence.clone())
    }
    fn presence_condition(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        expected: bool,
    ) -> Option<(&'a SyntaxNode, bool)> {
        let node = unwrap(node);
        if node.kind() == NodeKind::UnaryExpression
            && self.operator(file, node) == Some(TokenKind::Not)
            && self.core_operator(file, node).is_ok()
        {
            return self.presence_condition(file, node.child_nodes().next()?, !expected);
        }
        for name in ["occurs", "absent"] {
            if self.optional_call(file, node, name) {
                return node
                    .child_nodes()
                    .next()
                    .map(|arg| (arg, expected == (name == "occurs")));
            }
        }
        None
    }
    fn scoped_presence(&self, file: FileId, node: &SyntaxNode, scope: &Scope<'a>) -> Presence {
        let intrinsic = self
            .intrinsic_presence(file, node)
            .unwrap_or(Presence::Unknown);
        if matches!(
            intrinsic,
            Presence::Present | Presence::Absent | Presence::Unsupported(_)
        ) {
            return intrinsic;
        }
        for assumption in &scope.assumptions {
            if let Assumption::Condition {
                file: f,
                node: condition,
                expected,
                ..
            } = assumption
                && !contains_subject(*f, condition, file, node)
                && let Some((arg, present)) = self.presence_condition(*f, condition, *expected)
                && self.same(file, node, *f, arg)
            {
                return if present {
                    Presence::Present
                } else {
                    Presence::Absent
                };
            }
        }
        intrinsic
    }
    fn option_call(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        if children.len() != 1 {
            return self.unsupported(file, node, "option callable arity is unsupported");
        }
        let arg = children[0];
        let mut own = scope.clone();
        own.enforcement = DefinitionEnforcement::Conditional;
        let mut result = self.walk(file, item, arg, own);
        let presence = self.scoped_presence(file, arg, scope);
        if self.optional_call(file, node, "deopt") {
            let invariant = presence_outcome(
                &self
                    .intrinsic_presence(file, arg)
                    .unwrap_or(Presence::Unknown),
            );
            let outcome = presence_outcome(&presence);
            self.obligation(
                file,
                node,
                arg,
                GuardObligationKind::Presence,
                (invariant, outcome.clone()),
                scope,
            );
            result.definedness = conjoin(&result.definedness, &outcome);
            result.truth = None;
        } else {
            let truth = presence_outcome(&presence);
            result.truth = Some(if self.optional_call(file, node, "absent") {
                negate(truth)
            } else {
                truth
            });
            result.numeric = None;
        }
        result.presence = Some(Presence::Present);
        result
    }
    fn default_value(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scope: &Scope<'a>,
    ) -> Evaluation {
        let children: Vec<_> = node.child_nodes().collect();
        if children.len() != 2 || self.core_operator(file, node).is_err() {
            for child in children {
                self.walk(file, item, child, scope.clone());
            }
            return self.unsupported(
                file,
                node,
                "default requires its resolved scalar core overload",
            );
        }
        if self
            .ty(file, node)
            .is_some_and(|t| matches!(t.kind, TypeKind::Array { .. } | TypeKind::Set(_)))
        {
            for child in children {
                self.walk(file, item, child, scope.clone());
            }
            return self.unsupported(
                file,
                node,
                "structured default is outside guarded interpretation",
            );
        }
        let mut own = scope.clone();
        own.enforcement = DefinitionEnforcement::Conditional;
        let left = self.walk(file, item, children[0], own.clone());
        let presence = left
            .presence
            .clone()
            .unwrap_or_else(|| self.scoped_presence(file, children[0], scope));
        let use_left = left.definedness == GuardedOutcome::Proven && presence == Presence::Present;
        let fallback = left.definedness == GuardedOutcome::Refuted || presence == Presence::Absent;
        own.evaluation = GuardEvaluation::DefaultFallback;
        own.activation = if use_left {
            GuardActivation::Inactive
        } else if fallback {
            scope.activation
        } else {
            activate(scope.activation, &GuardedOutcome::Unknown, true)
        };
        let right = self.walk(file, item, children[1], own);
        if left.assertion {
            return self.unsupported(
                file,
                node,
                "default capture of aborting assertion is unsupported",
            );
        }
        // Missing interpretation is not a proved undefined value. An opaque
        // condition/body may abort; default cannot certify that evaluation.
        if matches!(left.definedness, GuardedOutcome::Unsupported(_)) {
            return left;
        }
        if use_left {
            return left;
        }
        if fallback {
            return right;
        }
        let mut result = Evaluation::total();
        // If fallback is defined, either selection returns a defined value.
        result.definedness = right.definedness.clone();
        if right.definedness == GuardedOutcome::Refuted {
            result.definedness = GuardedOutcome::Unknown;
        }
        result.presence = Some(
            if presence == Presence::Present && right.presence == Some(Presence::Present) {
                Presence::Present
            } else {
                self.intrinsic_presence(file, node)
                    .unwrap_or(Presence::Unknown)
            },
        );
        if left.numeric == right.numeric {
            result.numeric = left.numeric;
        }
        if left.truth == right.truth {
            result.truth = left.truth;
        }
        result.assertion = right.assertion;
        result
    }
    fn collection_nonempty(
        &self,
        file: FileId,
        node: &SyntaxNode,
        aggregate: &str,
    ) -> Option<GuardedOutcome> {
        let options = self.options?;
        let Some(count) = options.collection(file, &self.location(file, node)) else {
            return if self
                .ty(file, node)
                .is_some_and(crate::value_safety::optional)
            {
                Some(GuardedOutcome::Unsupported(
                    "optional collection count is outside interpretation".into(),
                ))
            } else {
                None
            };
        };
        let count = if matches!(aggregate, "length" | "card") {
            &count.capacity
        } else {
            &count.present
        };
        Some(match count {
            Cardinality::Exact(0) | Cardinality::Bounds { upper: 0, .. } => GuardedOutcome::Refuted,
            Cardinality::Exact(_) => GuardedOutcome::Proven,
            Cardinality::Bounds { lower, .. } if *lower > 0 => GuardedOutcome::Proven,
            Cardinality::Unsupported(reason) => GuardedOutcome::Unsupported(reason.clone()),
            _ => GuardedOutcome::Unknown,
        })
    }
    fn obligation(
        &mut self,
        file: FileId,
        operation: &SyntaxNode,
        operand: &SyntaxNode,
        kind: GuardObligationKind,
        outcomes: (GuardedOutcome, GuardedOutcome),
        scope: &Scope<'a>,
    ) {
        let (invariant, outcome) = outcomes;
        if let GuardedOutcome::Unsupported(reason) = &outcome {
            self.facts.limitations.push(GuardedLimitation {
                file,
                location: self.location(file, operation),
                reason: reason.clone(),
            });
        }
        self.facts.obligations.push(GuardObligation {
            file,
            operation: self.location(file, operation),
            operand: self.location(file, operand),
            kind,
            invariant,
            outcome,
            context: self.context(scope),
        });
    }
    fn nonzero(&self, file: FileId, node: &SyntaxNode) -> GuardedOutcome {
        match self.numeric(file, node).and_then(|n| n.excludes(0)) {
            Some(true) => GuardedOutcome::Proven,
            Some(false) => GuardedOutcome::Refuted,
            None => GuardedOutcome::Unknown,
        }
    }
    fn guarded_nonzero(&self, file: FileId, node: &SyntaxNode, scope: &Scope<'a>) -> bool {
        scope.assumptions.iter().any(|a| {
            let Assumption::Condition {
                file: f,
                node: condition,
                expected,
                ..
            } = a
            else {
                return false;
            };
            if contains_subject(*f, condition, file, node) {
                return false;
            }
            let condition = unwrap(condition);
            let children: Vec<_> = condition.child_nodes().collect();
            if children.len() != 2 || self.core_operator(*f, condition).is_err() {
                return false;
            }
            let (other, operator) = if self.same(file, node, *f, children[0]) {
                (children[1], self.operator(*f, condition))
            } else if self.same(file, node, *f, children[1]) {
                (
                    children[0],
                    self.operator(*f, condition).map(reverse_comparison),
                )
            } else {
                return false;
            };
            let Some(NumericOutcome::Exact(value)) = self.numeric(*f, other) else {
                return false;
            };
            match (operator, *expected) {
                (Some(TokenKind::NotEqual), true)
                | (Some(TokenKind::Equal | TokenKind::DoubleEqual), false) => value == 0,
                (Some(TokenKind::Greater), true) | (Some(TokenKind::LessEqual), false) => {
                    value >= 0
                }
                (Some(TokenKind::GreaterEqual), true) | (Some(TokenKind::Less), false) => value > 0,
                (Some(TokenKind::Less), true) | (Some(TokenKind::GreaterEqual), false) => {
                    value <= 0
                }
                (Some(TokenKind::LessEqual), true) | (Some(TokenKind::Greater), false) => value < 0,
                _ => false,
            }
        })
    }
    fn same(&self, af: FileId, a: &SyntaxNode, bf: FileId, b: &SyntaxNode) -> bool {
        let a = unwrap(a);
        let b = unwrap(b);
        if a.kind() == NodeKind::Expression
            && b.kind() == NodeKind::Expression
            && let (Some(a), Some(b)) = (
                resolved_reference(self.context, self.bindings, af, a),
                resolved_reference(self.context, self.bindings, bf, b),
            )
        {
            return a == b;
        }
        if a.kind() != b.kind() {
            return false;
        }
        let ac: Vec<_> = a.child_nodes().collect();
        let bc: Vec<_> = b.child_nodes().collect();
        if ac.len() != bc.len() || !ac.iter().zip(&bc).all(|(a, b)| self.same(af, a, bf, b)) {
            return false;
        }
        let at = tokens(&self.context.files[af].parsed, a);
        let bt = tokens(&self.context.files[bf].parsed, b);
        at.len() == bt.len()
            && at.iter().zip(bt).all(|(a, b)| {
                a.kind == b.kind
                    && self.context.files[af].parsed.source()[a.range.clone()]
                        == self.context.files[bf].parsed.source()[b.range.clone()]
            })
    }
    // A conservative invariant condition query: no scoped assumptions and no
    // second walk or duplicate observable facts. Other condition forms remain unknown.
    fn invariant_truth(&self, file: FileId, node: &SyntaxNode) -> GuardedOutcome {
        let node = unwrap(node);
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() == NodeKind::Expression {
            return match self.operator(file, node) {
                Some(TokenKind::True) => GuardedOutcome::Proven,
                Some(TokenKind::False) => GuardedOutcome::Refuted,
                _ => GuardedOutcome::Unknown,
            };
        }
        if self.options.is_some() && node.kind() == NodeKind::CallExpression {
            for name in ["occurs", "absent"] {
                if self.optional_call(file, node, name)
                    && let Some(arg) = children.first()
                {
                    let outcome = presence_outcome(
                        &self
                            .intrinsic_presence(file, arg)
                            .unwrap_or(Presence::Unknown),
                    );
                    return if name == "absent" {
                        negate(outcome)
                    } else {
                        outcome
                    };
                }
            }
        }
        if !matches!(
            node.kind(),
            NodeKind::BinaryExpression | NodeKind::UnaryExpression
        ) {
            return GuardedOutcome::Unknown;
        }
        if let Err(reason) = self.core_operator(file, node) {
            return GuardedOutcome::Unsupported(reason);
        }
        let Some(operator) = self.operator(file, node) else {
            return GuardedOutcome::Unknown;
        };
        if matches!(
            operator,
            TokenKind::And
                | TokenKind::Or
                | TokenKind::Implies
                | TokenKind::ReverseImplies
                | TokenKind::Equivalence
                | TokenKind::Xor
                | TokenKind::Not
        ) {
            let values: Vec<_> = children
                .iter()
                .map(|child| Evaluation {
                    truth: Some(self.invariant_truth(file, child)),
                    ..Evaluation::total()
                })
                .collect();
            return logical(operator, &values);
        }
        if children.len() != 2 || unwrap(children[0]).kind() != NodeKind::Expression {
            return GuardedOutcome::Unknown;
        }
        if operator == TokenKind::In {
            if crate::domains::core_arithmetic(
                self.context,
                self.bindings,
                self.calls,
                file,
                children[1],
            )
            .is_err()
            {
                return GuardedOutcome::Unknown;
            }
            return self.invariant_membership(file, children[0], &self.domain(file, children[1]));
        }
        if unwrap(children[1]).kind() != NodeKind::Expression
            || !matches!(
                operator,
                TokenKind::Equal
                    | TokenKind::DoubleEqual
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
            )
        {
            return GuardedOutcome::Unknown;
        }
        let values: Vec<_> = children
            .iter()
            .map(|child| Evaluation {
                numeric: self.numeric(file, child),
                definedness: if self.ty(file, child).is_some_and(|ty| {
                    !crate::value_safety::optional(ty) && !matches!(ty.kind, TypeKind::Unknown(_))
                }) {
                    GuardedOutcome::Proven
                } else {
                    GuardedOutcome::Unknown
                },
                ..Evaluation::total()
            })
            .collect();
        self.comparison(file, children[0], children[1], operator, &values)
    }
    fn assumed_truth(&self, file: FileId, node: &SyntaxNode, scope: &Scope<'a>) -> GuardedOutcome {
        scope
            .assumptions
            .iter()
            .find_map(|a| {
                if let Assumption::Condition {
                    file: f,
                    node: condition,
                    expected,
                    ..
                } = a
                    && !contains_subject(*f, condition, file, node)
                    && self.same(file, node, *f, condition)
                {
                    Some(if *expected {
                        GuardedOutcome::Proven
                    } else {
                        GuardedOutcome::Refuted
                    })
                } else {
                    None
                }
            })
            .unwrap_or(GuardedOutcome::Unknown)
    }
    fn comparison(
        &self,
        file: FileId,
        left: &SyntaxNode,
        right: &SyntaxNode,
        operator: TokenKind,
        values: &[Evaluation],
    ) -> GuardedOutcome {
        if values
            .iter()
            .all(|v| v.definedness == GuardedOutcome::Proven)
            && self.same(file, left, file, right)
        {
            return if matches!(
                operator,
                TokenKind::Equal
                    | TokenKind::DoubleEqual
                    | TokenKind::LessEqual
                    | TokenKind::GreaterEqual
            ) {
                GuardedOutcome::Proven
            } else {
                GuardedOutcome::Refuted
            };
        }
        let (Some((a, b)), Some((c, d))) = (
            values
                .first()
                .and_then(|v| v.numeric.as_ref())
                .and_then(NumericOutcome::interval),
            values
                .get(1)
                .and_then(|v| v.numeric.as_ref())
                .and_then(NumericOutcome::interval),
        ) else {
            return GuardedOutcome::Unknown;
        };
        let proved = match operator {
            TokenKind::Equal | TokenKind::DoubleEqual => a == b && c == d && a == c,
            TokenKind::NotEqual => b < c || d < a,
            TokenKind::Less => b < c,
            TokenKind::LessEqual => b <= c,
            TokenKind::Greater => a > d,
            TokenKind::GreaterEqual => a >= d,
            _ => false,
        };
        let refuted = match operator {
            TokenKind::Equal | TokenKind::DoubleEqual => b < c || d < a,
            TokenKind::NotEqual => a == b && c == d && a == c,
            TokenKind::Less => a >= d,
            TokenKind::LessEqual => a > d,
            TokenKind::Greater => b <= c,
            TokenKind::GreaterEqual => b < c,
            _ => false,
        };
        if proved {
            GuardedOutcome::Proven
        } else if refuted {
            GuardedOutcome::Refuted
        } else {
            GuardedOutcome::Unknown
        }
    }
    fn index_domain(&self, file: FileId, node: &SyntaxNode) -> Option<Domain> {
        let (array, _, domain) = resolved_index_domain(
            self.context,
            self.bindings,
            self.calls,
            self.domains,
            file,
            node,
        )?;
        self.array_domain_error(array).is_none().then_some(domain)
    }
    fn domain(&self, file: FileId, node: &SyntaxNode) -> Domain {
        self.index_domain(file, unwrap(node))
            .unwrap_or_else(|| expression_domain(self.context, self.bindings, file, node))
    }
    fn replaceable_set(&self, domain: &Domain) -> bool {
        match domain {
            Domain::Named {
                declaration,
                domain,
            } => {
                let ty = &self.calls.declarations[declaration.0].ty;
                (ty.instantiation == Instantiation::Parameter
                    && matches!(ty.kind, TypeKind::Set(_)))
                    || self.replaceable_set(domain)
            }
            _ => false,
        }
    }
    fn same_domain(&self, left: &Domain, right: &Domain) -> bool {
        let identities = |mut domain: &Domain| {
            let mut ids = Vec::new();
            while let Domain::Named {
                declaration,
                domain: inner,
            } = domain
            {
                ids.push(*declaration);
                domain = inner;
            }
            ids
        };
        let left_ids = identities(left);
        let right_ids = identities(right);
        if left_ids.iter().any(|id| right_ids.contains(id)) {
            return true;
        }
        if self.replaceable_set(left) || self.replaceable_set(right) {
            return false;
        }
        same_members(left, right) == Ok(Some(true))
    }
    fn invariant_membership(
        &self,
        file: FileId,
        node: &SyntaxNode,
        domain: &Domain,
    ) -> GuardedOutcome {
        let node = unwrap(node);
        if let Some(id) = resolved_reference(self.context, self.bindings, file, node) {
            let own = &self.domains.declarations[id.0].domain;
            if let Some(reason) = self
                .declaration_domain_error(id)
                .or_else(|| self.domain_error(own))
            {
                return GuardedOutcome::Unsupported(reason);
            }
            if self.same_domain(own, domain) {
                return GuardedOutcome::Proven;
            }
            let mut actual = domain;
            while let Domain::Named { domain, .. } = actual {
                actual = domain;
            }
            if let Domain::Enum(id) = actual
                && self
                    .ty(file, node)
                    .is_some_and(|ty| ty.kind == TypeKind::Enum(*id))
            {
                return GuardedOutcome::Proven;
            }
        }
        if self.replaceable_set(domain) {
            return GuardedOutcome::Unknown;
        }
        let mut domain = domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let Some(value) = self.numeric(file, node) else {
            return GuardedOutcome::Unknown;
        };
        let Some((a, b)) = value.interval() else {
            return GuardedOutcome::Unknown;
        };
        match domain {
            Domain::Range { lower, upper } => {
                match (invariant_integer(lower), invariant_integer(upper)) {
                    (Ok(Some(l)), Ok(Some(u))) => {
                        if l > u || b < l || a > u {
                            GuardedOutcome::Refuted
                        } else if a >= l && b <= u {
                            GuardedOutcome::Proven
                        } else {
                            GuardedOutcome::Unknown
                        }
                    }
                    (Err(reason), _) | (_, Err(reason)) => GuardedOutcome::Unsupported(reason),
                    _ => GuardedOutcome::Unknown,
                }
            }
            Domain::LiteralSet(values) => {
                let values = values
                    .iter()
                    .map(invariant_integer)
                    .collect::<Result<Option<Vec<_>>, _>>();
                match values {
                    Ok(Some(values)) if a == b => {
                        if values.contains(&a) {
                            GuardedOutcome::Proven
                        } else {
                            GuardedOutcome::Refuted
                        }
                    }
                    Ok(Some(values)) if values.iter().all(|n| *n < a || *n > b) => {
                        GuardedOutcome::Refuted
                    }
                    Err(reason) => GuardedOutcome::Unsupported(reason),
                    _ => GuardedOutcome::Unknown,
                }
            }
            Domain::Unsupported(reason) => GuardedOutcome::Unsupported(reason.clone()),
            _ => GuardedOutcome::Unknown,
        }
    }
    fn guarded_membership(
        &self,
        file: FileId,
        node: &SyntaxNode,
        domain: &Domain,
        scope: &Scope<'a>,
    ) -> bool {
        scope.assumptions.iter().any(|a| match a {
            Assumption::Membership {
                declaration,
                domain: own,
                ..
            } => {
                resolved_reference(self.context, self.bindings, file, unwrap(node))
                    == Some(*declaration)
                    && self.same_domain(own, domain)
            }
            Assumption::Condition {
                file: f,
                node: condition,
                expected: true,
                ..
            } => {
                if contains_subject(*f, condition, file, node) {
                    return false;
                }
                let condition = unwrap(condition);
                let children: Vec<_> = condition.child_nodes().collect();
                children.len() == 2
                    && self.operator(*f, condition) == Some(TokenKind::In)
                    && self.core_operator(*f, condition).is_ok()
                    && self.same(file, node, *f, children[0])
                    && crate::domains::core_arithmetic(
                        self.context,
                        self.bindings,
                        self.calls,
                        *f,
                        children[1],
                    )
                    .is_ok()
                    && self.same_domain(&self.domain(*f, children[1]), domain)
            }
            _ => false,
        })
    }
    fn index_space(
        &self,
        file: FileId,
        node: &SyntaxNode,
        scope: &Scope<'a>,
    ) -> Option<GuardedIndexSpace> {
        let node = unwrap(node);
        if matches!(
            node.kind(),
            NodeKind::RangeExpression | NodeKind::SetLiteral
        ) {
            crate::domains::core_arithmetic(self.context, self.bindings, self.calls, file, node)
                .ok()?;
            let domain = self.domain(file, node);
            return Some(GuardedIndexSpace {
                exact: index_domain_interval(&domain).is_some()
                    || matches!(bare_index_domain(&domain), Domain::LiteralSet(v) if v.is_empty()),
                domain,
            });
        }
        if node.kind() == NodeKind::Expression
            && let Some(TypeKind::Enum(id)) = self.ty(file, node).map(|ty| &ty.kind)
        {
            return Some(GuardedIndexSpace {
                domain: Domain::Enum(*id),
                exact: false,
            });
        }
        if let Some(NumericOutcome::Exact(n)) = self.numeric(file, node) {
            return Some(GuardedIndexSpace {
                domain: Domain::LiteralSet(vec![crate::NumericBound::Integer(n)]),
                exact: true,
            });
        }
        let (binding, offset) = if node.kind() == NodeKind::Expression {
            (node, 0)
        } else if node.kind() == NodeKind::BinaryExpression {
            self.core_operator(file, node).ok()?;
            let children: Vec<_> = node.child_nodes().collect();
            let [left, right] = children.as_slice() else {
                return None;
            };
            match self.operator(file, node)? {
                TokenKind::Plus => {
                    if let Some(NumericOutcome::Exact(n)) = self.numeric(file, right) {
                        (unwrap(left), n)
                    } else if let Some(NumericOutcome::Exact(n)) = self.numeric(file, left) {
                        (unwrap(right), n)
                    } else {
                        return None;
                    }
                }
                TokenKind::Minus => {
                    let NumericOutcome::Exact(n) = self.numeric(file, right)? else {
                        return None;
                    };
                    (unwrap(left), n.checked_neg()?)
                }
                _ => return None,
            }
        } else {
            return None;
        };
        if binding.kind() != NodeKind::Expression {
            return None;
        }
        let id = resolved_reference(self.context, self.bindings, file, binding)?;
        let mut domain = scope.assumptions.iter().find_map(|a| match a {
            Assumption::Membership {
                declaration,
                domain,
                ..
            } if *declaration == id => Some(domain.clone()),
            _ => None,
        })?;
        let mut exact = matches!(bare_index_domain(&domain), Domain::Enum(_))
            || (!self.replaceable_set(&domain) && index_domain_interval(&domain).is_some());
        for assumption in &scope.assumptions {
            let Assumption::Condition {
                file: f,
                node: condition,
                expected,
                ..
            } = assumption
            else {
                continue;
            };
            if contains_subject(*f, condition, file, node) {
                continue;
            }
            let condition = unwrap(condition);
            if self.invariant_truth(*f, condition)
                == if *expected {
                    GuardedOutcome::Proven
                } else {
                    GuardedOutcome::Refuted
                }
            {
                continue;
            }
            let children: Vec<_> = condition.child_nodes().collect();
            let narrowed = if let [left, right] = children.as_slice() {
                if self.same(file, binding, *f, left) && self.core_operator(*f, condition).is_ok() {
                    if self.operator(*f, condition) == Some(TokenKind::In) && *expected {
                        crate::domains::core_arithmetic(
                            self.context,
                            self.bindings,
                            self.calls,
                            *f,
                            right,
                        )
                        .ok()
                        .and_then(|_| intersect_index_domains(&domain, &self.domain(*f, right)))
                    } else if let Some(NumericOutcome::Exact(n)) = self.numeric(*f, right) {
                        comparison_index_domain(self.operator(*f, condition)?, n, *expected)
                            .and_then(|target| intersect_index_domains(&domain, &target))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            };
            if let Some(narrowed) = narrowed {
                domain = narrowed;
            } else {
                exact = false;
            }
        }
        if offset != 0 {
            domain = shift_index_domain(&domain, Some(offset));
        }
        Some(GuardedIndexSpace { domain, exact })
    }
    fn membership(
        &self,
        file: FileId,
        node: &SyntaxNode,
        domain: &Domain,
        scope: &Scope<'a>,
    ) -> GuardedOutcome {
        let invariant = self.invariant_membership(file, node, domain);
        if invariant == GuardedOutcome::Unknown
            && self.guarded_membership(file, node, domain, scope)
        {
            GuardedOutcome::Proven
        } else {
            invariant
        }
    }
    fn domain_nonempty(&self, domain: &Domain) -> GuardedOutcome {
        if self.replaceable_set(domain) {
            return GuardedOutcome::Unknown;
        }
        match domain {
            Domain::Named { domain, .. } => self.domain_nonempty(domain),
            Domain::Range { lower, upper } => {
                match (invariant_integer(lower), invariant_integer(upper)) {
                    (Ok(Some(l)), Ok(Some(u))) => {
                        if l <= u {
                            GuardedOutcome::Proven
                        } else {
                            GuardedOutcome::Refuted
                        }
                    }
                    (Err(reason), _) | (_, Err(reason)) => GuardedOutcome::Unsupported(reason),
                    _ => GuardedOutcome::Unknown,
                }
            }
            Domain::LiteralSet(values) => {
                if values.is_empty() {
                    GuardedOutcome::Refuted
                } else {
                    GuardedOutcome::Proven
                }
            }
            Domain::Unsupported(reason) => GuardedOutcome::Unsupported(reason.clone()),
            _ => GuardedOutcome::Unknown,
        }
    }
    fn array_domain_error(&self, array: DeclarationId) -> Option<String> {
        let domain = &self.domains.declarations[array.0].domain;
        self.declaration_domain_error(array)
            .or_else(|| self.domain_error(domain))
    }
    fn domain_error(&self, domain: &Domain) -> Option<String> {
        match domain {
            Domain::Named {
                declaration,
                domain,
            } => self
                .declaration_domain_error(*declaration)
                .or_else(|| self.domain_error(domain)),
            Domain::Array { indices, element } => indices
                .iter()
                .find_map(|d| self.domain_error(d))
                .or_else(|| self.domain_error(element)),
            Domain::Set(domain) => self.domain_error(domain),
            _ => None,
        }
    }
    fn declaration_domain_error(&self, id: DeclarationId) -> Option<String> {
        let declaration = &self.bindings.declarations[id.0];
        let node = crate::callables::find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        let ty = node
            .child_nodes()
            .find(|n| !matches!(n.kind(), NodeKind::Annotation | NodeKind::ParameterList))?;
        crate::domains::core_arithmetic(
            self.context,
            self.bindings,
            self.calls,
            declaration.file,
            ty,
        )
        .err()
    }
    fn guarded_nonempty(&self, file: FileId, node: &SyntaxNode, scope: &Scope<'a>) -> bool {
        scope.assumptions.iter().any(|a| {
            let Assumption::Condition {
                file: f,
                node: condition,
                expected,
                ..
            } = a
            else {
                return false;
            };
            if contains_subject(*f, condition, file, node) {
                return false;
            }
            let condition = unwrap(condition);
            let children: Vec<_> = condition.child_nodes().collect();
            if children.len() != 2 || self.core_operator(*f, condition).is_err() {
                return false;
            }
            let left = unwrap(children[0]);
            let right = unwrap(children[1]);
            if self.same(file, node, *f, left)
                && matches!(right.kind(), NodeKind::ArrayLiteral | NodeKind::SetLiteral)
                && right.child_nodes().next().is_none()
            {
                return matches!(
                    (self.operator(*f, condition), *expected),
                    (Some(TokenKind::NotEqual), true)
                        | (Some(TokenKind::Equal | TokenKind::DoubleEqual), false)
                );
            }
            if ["length", "card"]
                .iter()
                .any(|name| self.core_call(*f, left, name))
                && left
                    .child_nodes()
                    .next()
                    .is_some_and(|argument| self.same(file, node, *f, argument))
                && self.numeric(*f, right) == Some(NumericOutcome::Exact(0))
            {
                return matches!(
                    (self.operator(*f, condition), *expected),
                    (Some(TokenKind::Greater | TokenKind::NotEqual), true)
                        | (
                            Some(TokenKind::LessEqual | TokenKind::Equal | TokenKind::DoubleEqual),
                            false
                        )
                );
            }
            false
        })
    }
    fn nonempty(&self, file: FileId, node: &SyntaxNode) -> GuardedOutcome {
        let node = unwrap(node);
        if let Err(reason) =
            crate::domains::core_arithmetic(self.context, self.bindings, self.calls, file, node)
        {
            return GuardedOutcome::Unsupported(reason);
        }
        if self
            .ty(file, node)
            .is_some_and(crate::value_safety::optional)
        {
            return GuardedOutcome::Unsupported(
                "optional aggregate cardinality requires presence facts".into(),
            );
        }
        if matches!(node.kind(), NodeKind::ArrayLiteral | NodeKind::SetLiteral) {
            return if node.child_nodes().next().is_some() {
                GuardedOutcome::Proven
            } else {
                GuardedOutcome::Refuted
            };
        }
        let domain = self.domain(file, node);
        if let Some(id) = resolved_reference(self.context, self.bindings, file, node)
            && let Some(reason) = self
                .declaration_domain_error(id)
                .or_else(|| self.domain_error(&domain))
        {
            return GuardedOutcome::Unsupported(reason);
        }
        let mut actual = &domain;
        while let Domain::Named { domain, .. } = actual {
            actual = domain;
        }
        if let Domain::Array { indices, .. } = actual {
            let outcomes: Vec<_> = indices.iter().map(|d| self.domain_nonempty(d)).collect();
            if outcomes.contains(&GuardedOutcome::Refuted) {
                GuardedOutcome::Refuted
            } else if outcomes.iter().all(|o| *o == GuardedOutcome::Proven) {
                GuardedOutcome::Proven
            } else {
                GuardedOutcome::Unknown
            }
        } else {
            self.domain_nonempty(&domain)
        }
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
fn activate(current: GuardActivation, truth: &GuardedOutcome, expected: bool) -> GuardActivation {
    if current == GuardActivation::Inactive {
        return current;
    }
    match (truth, expected) {
        (GuardedOutcome::Refuted, true) | (GuardedOutcome::Proven, false) => {
            GuardActivation::Inactive
        }
        (GuardedOutcome::Proven, true) | (GuardedOutcome::Refuted, false) => current,
        _ => GuardActivation::Conditional,
    }
}
fn conjoin(left: &GuardedOutcome, right: &GuardedOutcome) -> GuardedOutcome {
    match (left, right) {
        (GuardedOutcome::Refuted, _) | (_, GuardedOutcome::Refuted) => GuardedOutcome::Refuted,
        (GuardedOutcome::Unsupported(reason), _) | (_, GuardedOutcome::Unsupported(reason)) => {
            GuardedOutcome::Unsupported(reason.clone())
        }
        (GuardedOutcome::Unknown, _) | (_, GuardedOutcome::Unknown) => GuardedOutcome::Unknown,
        _ => GuardedOutcome::Proven,
    }
}
fn evaluated_when(mut value: Evaluation, activation: GuardActivation) -> Evaluation {
    if activation == GuardActivation::Inactive {
        return Evaluation::total();
    }
    if activation == GuardActivation::Conditional && value.definedness == GuardedOutcome::Refuted {
        value.definedness = GuardedOutcome::Unknown;
    }
    value
}
fn combine(values: &[Evaluation]) -> Evaluation {
    let mut result = Evaluation::total();
    for value in values {
        result.definedness = conjoin(&result.definedness, &value.definedness);
        result.assertion |= value.assertion;
    }
    result
}
fn logical(operator: TokenKind, values: &[Evaluation]) -> GuardedOutcome {
    let truth = |i| {
        values
            .get(i)
            .and_then(|v: &Evaluation| v.truth.clone())
            .unwrap_or(GuardedOutcome::Unknown)
    };
    let a = truth(0);
    let b = truth(1);
    match operator {
        TokenKind::Not => negate(a),
        TokenKind::And => conjoin(&a, &b),
        TokenKind::Or => negate(conjoin(&negate(a), &negate(b))),
        TokenKind::Implies => negate(conjoin(&a, &negate(b))),
        TokenKind::ReverseImplies => negate(conjoin(&b, &negate(a))),
        TokenKind::Equivalence | TokenKind::Xor
            if matches!(a, GuardedOutcome::Proven | GuardedOutcome::Refuted)
                && matches!(b, GuardedOutcome::Proven | GuardedOutcome::Refuted) =>
        {
            if (a == b) == (operator == TokenKind::Equivalence) {
                GuardedOutcome::Proven
            } else {
                GuardedOutcome::Refuted
            }
        }
        _ => GuardedOutcome::Unknown,
    }
}
fn negate(truth: GuardedOutcome) -> GuardedOutcome {
    match truth {
        GuardedOutcome::Proven => GuardedOutcome::Refuted,
        GuardedOutcome::Refuted => GuardedOutcome::Proven,
        other => other,
    }
}
fn reverse_comparison(operator: TokenKind) -> TokenKind {
    match operator {
        TokenKind::Greater => TokenKind::Less,
        TokenKind::GreaterEqual => TokenKind::LessEqual,
        TokenKind::Less => TokenKind::Greater,
        TokenKind::LessEqual => TokenKind::GreaterEqual,
        other => other,
    }
}

fn contains_subject(
    condition_file: FileId,
    condition: &SyntaxNode,
    file: FileId,
    node: &SyntaxNode,
) -> bool {
    condition_file == file
        && condition.range().start <= node.range().start
        && condition.range().end >= node.range().end
}

fn presence_outcome(presence: &Presence) -> GuardedOutcome {
    match presence {
        Presence::Present => GuardedOutcome::Proven,
        Presence::Absent => GuardedOutcome::Refuted,
        Presence::Unsupported(reason) => GuardedOutcome::Unsupported(reason.clone()),
        _ => GuardedOutcome::Unknown,
    }
}

fn comparison_index_domain(operator: TokenKind, n: i64, expected: bool) -> Option<Domain> {
    let op = if expected {
        operator
    } else {
        match operator {
            TokenKind::Less => TokenKind::GreaterEqual,
            TokenKind::LessEqual => TokenKind::Greater,
            TokenKind::Greater => TokenKind::LessEqual,
            TokenKind::GreaterEqual => TokenKind::Less,
            TokenKind::NotEqual => TokenKind::Equal,
            _ => return None,
        }
    };
    let (l, u) = match op {
        TokenKind::Less => (i64::MIN, n.checked_sub(1)?),
        TokenKind::LessEqual => (i64::MIN, n),
        TokenKind::Greater => (n.checked_add(1)?, i64::MAX),
        TokenKind::GreaterEqual => (n, i64::MAX),
        TokenKind::Equal | TokenKind::DoubleEqual => (n, n),
        _ => return None,
    };
    Some(Domain::Range {
        lower: crate::NumericBound::Integer(l),
        upper: crate::NumericBound::Integer(u),
    })
}
