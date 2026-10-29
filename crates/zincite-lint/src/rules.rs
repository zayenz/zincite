//! Stable rule catalogue and explicit selection, without rule dispatch machinery.
use crate::Severity;

/// A rule's purpose, independent of warning severity and fix safety.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleFamily {
    /// Potential violations of model semantics.
    Correctness,
    /// Patterns that may hide mistakes or unintended dependencies.
    Suspicious,
    /// Formulation, intent and search-structure advice.
    Modelling,
    /// Formulations that may affect solver work; no runtime guarantee.
    Performance,
    /// Names, source readability and conventional formulations.
    Style,
}

impl RuleFamily {
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Self::Correctness,
            Self::Suspicious,
            Self::Modelling,
            Self::Performance,
            Self::Style,
        ]
        .into_iter()
        .find(|family| family.as_str() == name)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Correctness => "correctness",
            Self::Suspicious => "suspicious",
            Self::Modelling => "modelling",
            Self::Performance => "performance",
            Self::Style => "style",
        }
    }
}

/// Implemented rule-level capability, independent of an individual finding's
/// optional fix. Infrastructure alone does not supply a fix producer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixSupport {
    /// No fix producer is implemented for the rule.
    None,
    /// A fix producer exists, but only findings meeting its conditions get edits.
    Sometimes,
    /// A fix producer supplies edits for every finding from the rule.
    Always,
}

impl FixSupport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sometimes => "sometimes",
            Self::Always => "always",
        }
    }
}

/// Inspectable rule documentation shared by library callers and CLI discovery.
/// Availability means the rule is implemented; analysis of a particular input
/// can still be limited or inapplicable. Fix support records implemented edits,
/// subject to each finding's proved applicability conditions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub family: RuleFamily,
    /// Severity of the rule's findings. Not configurable; expensive-comprehension
    /// also reports informational symbolic advice.
    pub severity: Severity,
    pub purpose: &'static str,
    pub requires_model: bool,
    /// The facts the implementation needs; metadata does not produce those facts.
    pub requirements: &'static str,
    pub available: bool,
    pub fix_support: FixSupport,
    /// Current rule-specific options. An empty slice means none are supported.
    pub options: &'static [&'static str],
    pub limitations: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Naming,
    MissingConstraintLabel,
    ArrayIndexStart,
    CompactIf,
    ConstantVariable,
    EffectiveZeroOne,
    ElementPredicate,
    ReifiedGlobal,
    GlobalVariableInFunction,
    UnboundedVariable,
    SearchCoverage,
    DecisionVariableOperator,
    UnmarkedSymmetryBreaking,
    UnusedDeclaration,
    DecisionVariableGenerator,
    DecisionVariableCondition,
    SuspiciousShadowing,
    ExpensiveComprehension,
    IndexSetMismatch,
    HiddenOptionality,
    PartialExpression,
    VacuousConstraint,
    UnusedGeneratorBinding,
    GlobalConstraintOpportunity,
    MissingInputPrecondition,
    SuspiciousDomain,
    PreferArrayComprehension,
    PreferEnum,
    PairwiseSelfComparison,
    DeprecatedCall,
    PreferAssertion,
    AmbiguousPrecedence,
    FloatEquality,
}

impl Rule {
    pub const DEFAULT: [Self; 10] = [
        Self::PreferArrayComprehension,
        Self::IndexSetMismatch,
        Self::SuspiciousShadowing,
        Self::PartialExpression,
        Self::HiddenOptionality,
        Self::VacuousConstraint,
        Self::UnboundedVariable,
        Self::UnusedDeclaration,
        Self::PairwiseSelfComparison,
        Self::DeprecatedCall,
    ];
    pub const THESIS: [Self; 14] = [
        Self::ArrayIndexStart,
        Self::CompactIf,
        Self::ConstantVariable,
        Self::EffectiveZeroOne,
        Self::ElementPredicate,
        Self::ReifiedGlobal,
        Self::GlobalVariableInFunction,
        Self::UnboundedVariable,
        Self::SearchCoverage,
        Self::DecisionVariableOperator,
        Self::UnmarkedSymmetryBreaking,
        Self::UnusedDeclaration,
        Self::DecisionVariableGenerator,
        Self::DecisionVariableCondition,
    ];

    pub const ADDITIONAL: [Self; 2] = [Self::SuspiciousShadowing, Self::ExpensiveComprehension];

    /// Every registered rule in stable catalogue order, independent of preset membership.
    pub fn all() -> impl Iterator<Item = Self> {
        [
            Self::Naming,
            Self::MissingConstraintLabel,
            Self::PreferArrayComprehension,
        ]
        .into_iter()
        .chain(Self::THESIS)
        .chain(Self::ADDITIONAL)
        .chain([
            Self::IndexSetMismatch,
            Self::HiddenOptionality,
            Self::PartialExpression,
            Self::VacuousConstraint,
            Self::UnusedGeneratorBinding,
            Self::GlobalConstraintOpportunity,
            Self::MissingInputPrecondition,
            Self::SuspiciousDomain,
            Self::PreferEnum,
            Self::PairwiseSelfComparison,
            Self::DeprecatedCall,
            Self::PreferAssertion,
            Self::AmbiguousPrecedence,
            Self::FloatEquality,
        ])
    }

    /// Return the single catalogue entry for this rule. Extend this match when
    /// registering a rule; CLI explanations and the existing accessors use it.
    pub fn metadata(self) -> RuleMetadata {
        let (id, family, purpose, requirements, limitations) = match self {
            Self::FloatEquality => (
                "float-equality",
                RuleFamily::Suspicious,
                "Report = and != between parameter float expressions, which rarely compare equal exactly after computation.",
                "Expression types, expression instantiation and the selected comparison operation.",
                "Both operands must be proved non-optional parameter floats. Decision, integer, optional and unknown-typed comparisons and user-defined comparison operators stay quiet without a limitation. No fix is offered.",
            ),
            Self::AmbiguousPrecedence => (
                "ambiguous-precedence",
                RuleFamily::Suspicious,
                "Report unparenthesized Boolean operators whose MiniZinc grouping is easy to misread.",
                "Binary expression syntax and explicit parentheses.",
                "Reports /\\ mixed with \\/ or xor at one level, and chains of two or more -> or <-, which group from the left. Other operators, not, <-> chains and backquoted infix calls stay quiet. No fix is offered.",
            ),
            Self::PreferAssertion => (
                "prefer-assertion",
                RuleFamily::Style,
                "Suggest assert for a constraint item whose expression depends only on parameters, so failed input is explained.",
                "Constraint item syntax and expression instantiation.",
                "Top-level constraint items only; constraints in let expressions are not checked. An item that already is an assert call, depends on a decision or has unknown instantiation stays quiet; unknown instantiation reports a limitation. No fix is offered.",
            ),
            Self::DeprecatedCall => (
                "deprecated-call",
                RuleFamily::Suspicious,
                "Report calls to standard-library callables annotated mzn_deprecated, quoting the version and explanation.",
                "Resolved callable identity and the mzn_deprecated annotation on that standard declaration or its identical prototype.",
                "Standard-library callables only; a user callable with the same name stays quiet. Unresolved or ambiguous calls with a deprecated standard namesake report a limitation. No fix is offered.",
            ),
            Self::PairwiseSelfComparison => (
                "pairwise-self-comparison",
                RuleFamily::Correctness,
                "Report an enforced !=, < or > between two bindings over one source that never excludes their equal pair.",
                "Resolved core comparisons and forall, proved-total array accesses, enforced guarded context and independent unfiltered iteration bindings.",
                "One core forall with two unfiltered bindings over the same parameter source, comparing elements of one one-dimensional nonoptional int, enum, bool or float array. Filters, dependent sources, nested foralls, disjunctions, conditionals, different arrays, unknown facts and known-empty sources stay quiet. No fix is offered.",
            ),
            Self::PreferEnum => (
                "prefer-enum",
                RuleFamily::Style,
                "Consider an enum for a finite integer set whose observed members serve as identities.",
                "Complete model, lexical identities, source callable signatures and written domains.",
                "Opt-in advice only. Numeric member uses and representation-sensitive output withhold advice. Recursive/opaque calls, computed membership and member storage in arrays or structured fields report affected limitations. External data/output compatibility and equivalent conversion are not established; no fix is offered.",
            ),
            Self::Naming => (
                "naming",
                RuleFamily::Style,
                "Check snake_case value/binding names and UpperCamelCase enum/type/constructor and explicit parameter-set names.",
                "Syntax declaration roles and direct binding names.",
                "References, alias-dependent roles and data assignment targets are skipped. Quoted and anonymous names are exempt; a single leading underscore is allowed. Decision sets use snake_case; SHOUTY_CASE is rejected.",
            ),
            Self::MissingConstraintLabel => (
                "missing-constraint-label",
                RuleFamily::Style,
                "Recommend a direct string label explaining each constraint.",
                "Constraint syntax and direct header annotations.",
                "Nested strings and expression annotations are not labels. Keyword-free equality items are constraints too. Labels are modelling advice; the rule does not invent them.",
            ),
            Self::ArrayIndexStart => (
                "array-index-start",
                RuleFamily::Style,
                "Advise starting known nonempty numeric array index sets at 1.",
                "Resolved declarations, named domains and symbolic numeric index sets.",
                "Enum indices are not numeric offsets. Unknown symbolic bounds and unconstrained int indices stay quiet; unsupported domain facts report limitations.",
            ),
            Self::CompactIf => (
                "compact-if",
                RuleFamily::Performance,
                "Suggest a Boolean-to-integer product for a decision-Boolean conditional with a zero integer branch.",
                "Lexical bindings, resolved callable types, decision instantiation and conditional safety.",
                "Optional, potentially partial and unknown-typed forms receive no replacement advice. Unsupported safety reports a limitation; the suggestion does not guarantee faster solving.",
            ),
            Self::ConstantVariable => (
                "constant-variable",
                RuleFamily::Modelling,
                "Advise expressing a decision with a proved whole parameter-valued definition as a parameter.",
                "Bindings, callable types, instantiation, domains and unconditional whole-definition facts.",
                "Initializers, enforced equalities and complete unfiltered core forall definitions are supported. Conditional, cyclic or partial definitions stay quiet; unsupported option/value safety reports limitations.",
            ),
            Self::EffectiveZeroOne => (
                "effective-zero-one",
                RuleFamily::Performance,
                "Suggest inequalities for matching zero/one implications and sum(array) for proved whole zero/one traversals.",
                "Bindings, callable types, instantiation, domains and instance-invariant integer bounds/constants.",
                "All domain and traversal obligations must hold for every instance. Optional, partial or unknown forms receive no formulation advice; unsupported proof reports a limitation. No solving-speed guarantee is made.",
            ),
            Self::ElementPredicate => (
                "element-predicate",
                RuleFamily::Style,
                "Recommend indexing equality for a resolved standard-library three-argument element predicate.",
                "Lexical bindings, selected callable bodies/types, actual array domains and guarded operand totality.",
                "User lookalikes do not match. A Safe indexing-equality fix requires the supported standard body, matching nonoptional scalar types, actual index membership, total operands and core equality; unknown, optional, opaque, named or annotated unsupported forms retain advice only. Ambiguous, unresolved or unsupported calls report limitations.",
            ),
            Self::ReifiedGlobal => (
                "reified-global",
                RuleFamily::Performance,
                "Advise reviewing included standard Boolean globals used in decision-dependent Boolean value contexts.",
                "Resolved standard callable identity, decision instantiation and enforcement-context facts.",
                "Always-enforced conjunctions/forall, implicit builtins and parameter-only actuals stay quiet. Advice does not guarantee solver speed; unknown required facts report limitations.",
            ),
            Self::GlobalVariableInFunction => (
                "global-variable-in-function",
                RuleFamily::Style,
                "Advise passing captured top-level decisions to user callables as arguments.",
                "Lexical bindings, declaration identity and declared instantiation.",
                "Shadowed parameters/locals and parameter-valued globals are not captures. Callable names and field labels are not variable uses. Analysis does not perform full type checking.",
            ),
            Self::UnboundedVariable => (
                "unbounded-variable",
                RuleFamily::Performance,
                "Recommend explicit domains for numeric decisions without bounds or proved complete definitions.",
                "Bindings, callable types, instantiation, resolved domains and whole-definition facts.",
                "Scalar and array element declarations and aliases are considered. Conditional, cyclic or partial equalities do not prove complete definitions; unsupported facts report limitations.",
            ),
            Self::SearchCoverage => (
                "search-coverage",
                RuleFamily::Modelling,
                "Advise on solve-visible decisions that an explicit solve search neither names nor derives through supported unconditional definitions.",
                "Complete model root, resolved typed searches/aliases, whole-array views, definitions and dependency closure.",
                "Nested seq_search, simple annotation aliases, array1d views and supported callable output guarantees are followed. Partial selections, unseeded cycles, opaque/optional bodies and unknown annotations withhold affected coverage claims. Computed outputs are not inverted; scoped decisions without a retained definition remain Unknown. Advice does not certify search completeness.",
            ),
            Self::DecisionVariableOperator => (
                "decision-variable-operator",
                RuleFamily::Performance,
                "Advise on ^, div, mod and / with decision-dependent operands.",
                "Lexical bindings, resolved callable types and operand instantiation.",
                "The operators are ^, div, mod and /, including quoted call spellings. Unlike the thesis, Boolean xor, disjunction, implication, equivalence and not stay quiet because they are ordinary modelling. Parameter-only operands stay quiet; unknown facts report limitations. Reformulation/tabling advice is conditional and has no solving-speed guarantee.",
            ),
            Self::UnmarkedSymmetryBreaking => (
                "unmarked-symmetry-breaking",
                RuleFamily::Modelling,
                "Ask whether resolved standard lexicographic, precedence or monotonicity constraints break symmetry.",
                "Resolved standard callable identity, enforcement contexts and symmetry-wrapper facts.",
                "Core symmetry_breaking_constraint wrappers mark nested uses; user lookalikes do not. A matched constraint may be required by the model. Advice does not prove symmetry or recommend unconditional removal.",
            ),
            Self::UnusedDeclaration => (
                "unused-declaration",
                RuleFamily::Suspicious,
                "Report outermost declarations unreachable from a complete model's constraints, solve, output and annotations.",
                "Complete model root, lexical/callable bindings, dependency graph and explicit/default output facts.",
                "Missing-solve fragments are inapplicable. Unknown reachable dependencies or output semantics withhold affected unused claims. Underscore names are exempt; implicit standard sources receive no warnings. No deletion is performed.",
            ),
            Self::DecisionVariableGenerator => (
                "decision-variable-generator",
                RuleFamily::Performance,
                "Advise on decision-dependent comprehension/generator sources and assignment expressions.",
                "Lexical bindings, resolved callable types and source/assignment instantiation.",
                "Aliases, calls and nested generators are considered. A parameter-valued iteration binding alone is not a match. Unknown required facts report limitations; reformulation advice has no solving-speed guarantee.",
            ),
            Self::DecisionVariableCondition => (
                "decision-variable-condition",
                RuleFamily::Performance,
                "Advise on decision-dependent if/elseif conditions and generator where filters.",
                "Lexical bindings, resolved callable types and condition/filter instantiation.",
                "Decisions appearing only in a branch or body are not matches. Parameter-only conditions stay quiet; unknown facts report limitations. Reformulation advice has no solving-speed guarantee.",
            ),
            Self::IndexSetMismatch => (
                "index-set-mismatch",
                RuleFamily::Correctness,
                "Report proved incompatible array index spaces and neighbour offsets.",
                "Resolved array dimensions, iteration membership and scoped guarded index obligations.",
                "Closed scalar/range indices, enum identities and checked neighbour offsets are supported. Traversal bounds of one parameter plus or minus a constant report when an outside index occurs whenever the traversal is active and nonempty. Guarded subsets and non-one-based arrays are valid. Unknown restricting guards, parameter relations and overapproximations stay quiet; unsupported required accesses report limitations. A candidate mismatch is not a guaranteed runtime error. No source rewrite is offered.",
            ),
            Self::HiddenOptionality => (
                "hidden-optionality",
                RuleFamily::Suspicious,
                "Explain when length counts comprehension slots rather than selected or present optional values.",
                "Resolved standard length/comparison calls, optional collection capacity/presence and guarded contexts.",
                "Direct integer comparisons and supported array aliases are considered. Advice is conditional on count intent; optional types and unknown data alone are not a mistake. Parameter filters, ordinary aggregates, capacity display, zero/all-present and inactive cases stay quiet. Unsupported required facts report limitations. No replacement count or source fix is offered.",
            ),
            Self::PartialExpression => (
                "partial-expression",
                RuleFamily::Suspicious,
                "Explain failed definedness requirements, incompatible candidate indices and symbolic traversals that leave an index set in their guarded and Boolean context.",
                "Scoped nonzero, index, nonempty and presence obligations; exact iteration membership and resolved default capture.",
                "Supported div/mod, array indexing, empty min/max and absent deopt are considered. Unknown facts alone stay quiet. Advice distinguishes failure when evaluated from incompatible candidates, without claiming execution, runtime crashes or infeasibility. Guards, assertions, inactive/empty iterations and supported default capture protect operations; relational Boolean collapse alone does not. Joint indexing advice is deduplicated at the same unsuppressed access. Unsupported required facts report limitations. No fix or rewrite is offered.",
            ),
            Self::VacuousConstraint => (
                "vacuous-constraint",
                RuleFamily::Suspicious,
                "Report proved empty quantifiers, rejecting filters and constant constraint or condition truth.",
                "Guarded Boolean truth/definedness, optional present counts and iteration filter/coverage facts.",
                "Supported empty forall/exists, impossible where filters, true constraints and false conditions/constraints are considered. Facts under guards remain conditional on those assumptions. Unknown parameters and partial self-comparisons stay quiet; relational false differs from numeric totality and assertion abort. Optional capacity does not prove present elements. Upstream evaluation must be defined before empty quantifier identities are reported. Advice guesses neither intent nor feasibility and offers no removal, reordering or fix. Unsupported required facts report limitations.",
            ),
            Self::GlobalConstraintOpportunity => (
                "global-constraint-opportunity",
                RuleFamily::Modelling,
                "Recognise complete pairwise disequalities and whole-array occurrence bounds.",
                "Resolved core calls, matching nonoptional equality types, actual index membership, guarded raw totality and enforced context.",
                "Complete two-index pairwise disequality can suggest all_different. Matching whole-array indicator sums bounded by par lower/upper arrays can suggest modern four-argument open global_cardinality, leaving values outside cover unrestricted. The initial count pattern requires proved nonempty xs. Partial pairs, shifts, extra filters, mismatched indices/types, optional or partial equality, user overloads and unsupported semantic annotations withhold advice. Unknown facts stay quiet; unsupported required facts report limitations. No fix, source-equivalence transformation or faster-solving claim is offered.",
            ),
            Self::UnusedGeneratorBinding => (
                "unused-generator-binding",
                RuleFamily::Style,
                "Identify unused generator names while retaining every iteration and evaluation.",
                "Resolved uses across remaining domains, filters, bodies and annotations; iteration multiplicity and guarded obligations.",
                "Anonymous and underscore-prefixed names are exempt. A Safe fix replaces only a completely unused membership name in an array, set or indexed comprehension with _, preserving multiplicity and evaluation. Generator-call shorthand, assignment and unknown/partial evaluation remain advice-only. No generator removal or hoisting is offered.",
            ),
            Self::SuspiciousShadowing => (
                "suspicious-shadowing",
                RuleFamily::Suspicious,
                "Report binding names that conceal another binding in a surrounding scope.",
                "Lexical scopes and binding identities.",
                "Parameters, let declarations and generators are checked against visible enclosing value bindings. Callable parameters that share a top-level name, disjoint scopes, field labels, underscore-prefixed names and exact ignore-names exceptions stay quiet. Ambiguous outer bindings report a limitation; types and callable names are not value bindings. Findings identify both declarations; no renaming or fix is offered.",
            ),
            Self::PreferArrayComprehension => (
                "prefer-array-comprehension",
                RuleFamily::Style,
                "Prefer direct comprehensions that preserve the wrapper's index axes.",
                "Standard source/signature callable identity, fixed set domains and independent ordered generator bindings.",
                "Supports standard array1d/array2d/array3d wrappers with matching Cartesian membership generators. Plain 1D requires proven 1..n indices; other 1D and all multidimensional cases retain keys. Multidimensional axes must be proven nonempty. Filters, dependencies, unknown/shadowed calls and annotations stay unchanged. Removed wrapper comments withhold a fix; body and generator text are copied exactly.",
            ),
            Self::SuspiciousDomain => (
                "suspicious-domain",
                RuleFamily::Suspicious,
                "Compare complete integer definitions with their explicit destination domains.",
                "Independent checked numeric RHS and actual declared membership, with definition enforcement/safety.",
                "Scalar initializer, immutable let-local and enforced equality contracts. Exact/disjoint values prove a contract contradiction; overlapping noncontained interval hulls supply conservative possible-conflict advice without an actual failing value or feasibility witness. Parameter defaults, symbolic/unknown values and checked overflow do not prove conflicts; candidate limitations are located. Unbounded declarations, unrelated nested intermediates, arbitrary magnitude/backend limits and automatic fixes are outside this rule.",
            ),
            Self::MissingInputPrecondition => (
                "missing-input-precondition",
                RuleFamily::Modelling,
                "Explain an unvalidated formal-input condition required by a supported callable body operation.",
                "Resolved callable/formal identities, scoped operation requirements and actual index/present-count evidence.",
                "General user callable bodies only; defaults and model-global assumptions do not validate formals. Covers array membership/matching dimensions, nonempty min/max present values, div/mod nonzero operands, and scalar core ln/log10/log2 with integer inputs (including int2float). Direct float, optional logarithms, general log and unresolved operation meaning remain unsupported, not guessed contracts. Guards, successful local/lazy assertions, inactive evaluations and supported default capture protect the actual operation. Advice is no failing-input, feasibility or runtime claim and inserts no assertion or fix.",
            ),
            Self::ExpensiveComprehension => (
                "expensive-comprehension",
                RuleFamily::Performance,
                "Advise on comprehension candidate-count upper bounds and symbolic expansion structure.",
                "Checked iteration bounds, ordered dimensions, resolved dependencies and prospective raw evaluation safety.",
                "Known pre-filter candidate counts and upper bounds warn strictly above max-candidates. Symbolic products describe ordered dimensions and dependencies as informational advice: no numeric count or threshold exceedance is established, and this advice alone does not fail lint. Supported whole par filters and repeated core aggregates receive earlier-prefix warnings only with retained guards, proved raw totality and nonempty repeated crossed sources. Decision filters, unknown/empty crossed domains, partial or opaque/effectful evaluation and semantic annotations withhold movement advice. Arithmetic multiplicity and optional present counts remain distinct. The compiler may already optimize the structure; no enumeration, runtime, speedup or source-fix claim is offered.",
            ),
        };
        RuleMetadata {
            id,
            family,
            // Advice proposing an alternative formulation, not a likely mistake.
            severity: if matches!(
                self,
                Self::ConstantVariable
                    | Self::CompactIf
                    | Self::EffectiveZeroOne
                    | Self::ReifiedGlobal
                    | Self::SearchCoverage
                    | Self::DecisionVariableOperator
                    | Self::DecisionVariableGenerator
                    | Self::DecisionVariableCondition
                    | Self::UnmarkedSymmetryBreaking
                    | Self::GlobalConstraintOpportunity
                    | Self::MissingInputPrecondition
                    | Self::UnusedDeclaration
            ) {
                Severity::Informational
            } else {
                Severity::Warning
            },
            purpose,
            requirements,
            limitations,
            requires_model: !matches!(
                self,
                Self::Naming | Self::MissingConstraintLabel | Self::AmbiguousPrecedence
            ),
            available: true,
            fix_support: if matches!(
                self,
                Self::ElementPredicate
                    | Self::UnusedGeneratorBinding
                    | Self::PreferArrayComprehension
            ) {
                FixSupport::Sometimes
            } else {
                FixSupport::None
            },
            options: match self {
                Self::SuspiciousShadowing => &["ignore-names: exact binding names; default []"],
                Self::ExpensiveComprehension => &[
                    "max-candidates: positive integer; default 1000000 (TOML range 1..=9223372036854775807)",
                ],
                _ => &[],
            },
        }
    }

    pub fn id(self) -> &'static str {
        self.metadata().id
    }

    pub fn severity(self) -> Severity {
        self.metadata().severity
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().find(|rule| rule.id() == id)
    }

    /// Registration alone does not mean a rule has an implementation.
    pub fn is_available(self) -> bool {
        self.metadata().available
    }

    /// The original rules inspect one file; thesis rules require model facts.
    pub fn requires_model(self) -> bool {
        self.metadata().requires_model
    }
}

// Semantic concerns are presets, independent of each rule's primary family.
const SEMANTIC_PRESETS: &[(&str, &[Rule])] = &[
    (
        "conventions",
        &[
            Rule::Naming,
            Rule::MissingConstraintLabel,
            Rule::ArrayIndexStart,
            Rule::DeprecatedCall,
            Rule::PreferAssertion,
            Rule::AmbiguousPrecedence,
        ],
    ),
    (
        "scope",
        &[
            Rule::SuspiciousShadowing,
            Rule::UnusedDeclaration,
            Rule::UnusedGeneratorBinding,
            Rule::GlobalVariableInFunction,
        ],
    ),
    (
        "arrays",
        &[
            Rule::IndexSetMismatch,
            Rule::HiddenOptionality,
            Rule::PreferArrayComprehension,
        ],
    ),
    (
        "domains",
        &[
            Rule::UnboundedVariable,
            Rule::SuspiciousDomain,
            Rule::PartialExpression,
            Rule::MissingInputPrecondition,
            Rule::FloatEquality,
        ],
    ),
    (
        "formulation",
        &[
            Rule::ConstantVariable,
            Rule::CompactIf,
            Rule::EffectiveZeroOne,
            Rule::ElementPredicate,
            Rule::GlobalConstraintOpportunity,
            Rule::PreferEnum,
            Rule::VacuousConstraint,
            Rule::PairwiseSelfComparison,
        ],
    ),
    (
        "flattening",
        &[
            Rule::ExpensiveComprehension,
            Rule::DecisionVariableOperator,
            Rule::DecisionVariableGenerator,
            Rule::DecisionVariableCondition,
            Rule::ReifiedGlobal,
        ],
    ),
    (
        "search",
        &[Rule::SearchCoverage, Rule::UnmarkedSymmetryBreaking],
    ),
];

pub(crate) fn preset_rules(name: &str) -> Option<Vec<Rule>> {
    match name {
        "default" => Some(Rule::DEFAULT.to_vec()),
        "thesis" => Some(Rule::THESIS.to_vec()),
        "all" => Some(Rule::all().collect()),
        _ => SEMANTIC_PRESETS
            .iter()
            .find(|(preset, _)| *preset == name)
            .map(|(_, rules)| rules.to_vec()),
    }
}

pub(crate) fn preset_memberships(rule: Rule) -> Vec<&'static str> {
    [
        ("default", Rule::DEFAULT.as_slice()),
        ("thesis", Rule::THESIS.as_slice()),
    ]
    .into_iter()
    .chain(SEMANTIC_PRESETS.iter().copied())
    .filter_map(|(name, rules)| rules.contains(&rule).then_some(name))
    .chain(["all"])
    .collect()
}

/// Selected rules. The default selects bounded model-backed likely-mistake checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintOptions {
    pub rules: Vec<Rule>,
    pub parameters: crate::settings::RuleOptions,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            rules: Rule::DEFAULT.to_vec(),
            parameters: crate::settings::RuleOptions::default(),
        }
    }
}

impl LintOptions {
    pub fn requires_model(&self) -> bool {
        self.rules.iter().any(|rule| rule.requires_model())
    }

    /// Expand comma-separated IDs, `family:NAME`, built-in `preset:NAME`
    /// and the legacy `default`/`thesis`/`all` selectors. First occurrence wins.
    /// Availability is checked separately before execution.
    pub fn from_selection(selection: &str) -> Result<Self, String> {
        Ok(Self {
            rules: expand_selectors(selection.split(','))?,
            ..Self::default()
        })
    }

    /// Reject selected rules that are registered but not implemented yet.
    pub fn check_available(&self) -> Result<(), String> {
        let unavailable: Vec<_> = self
            .rules
            .iter()
            .filter(|rule| !rule.is_available())
            .map(|rule| rule.id())
            .collect();
        if unavailable.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "unavailable rules (not implemented): {}",
                unavailable.join(", ")
            ))
        }
    }
}

// One expander for CLI lists and explicit settings; family membership comes
// from the catalogue and empty registered families are valid selections.
pub(crate) fn expand_selectors<'a>(
    selectors: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    for selector in selectors {
        let expanded = if let Some(name) = selector.strip_prefix("family:") {
            let family = RuleFamily::from_name(name)
                .ok_or_else(|| format!("unknown rule family '{name}'"))?;
            Rule::all()
                .filter(|rule| rule.metadata().family == family)
                .collect()
        } else {
            let name = selector.strip_prefix("preset:").unwrap_or(selector);
            let preset = preset_rules(name);
            let exact = Rule::from_id(selector);
            match (preset, exact) {
                (Some(_), Some(_)) => {
                    return Err(format!("ambiguous rule selector '{selector}'"));
                }
                (Some(preset), None) => preset,
                (None, Some(rule)) => vec![rule],
                (None, None) => return Err(format!("unknown rule selector '{selector}'")),
            }
        };
        for rule in expanded {
            if !rules.contains(&rule) {
                rules.push(rule);
            }
        }
    }
    Ok(rules)
}
