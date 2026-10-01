//! Stable rule catalogue and explicit selection, without rule dispatch machinery.
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
    /// Names and source readability.
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

/// Implemented fix capability, not a promise about possible future rewrites.
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
/// can still be limited or inapplicable. All current rules are diagnostic-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleMetadata {
    pub id: &'static str,
    pub family: RuleFamily,
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
}

impl Rule {
    pub const DEFAULT: [Self; 2] = [Self::Naming, Self::MissingConstraintLabel];
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

    /// Return the single catalogue entry for this rule. Extend this match when
    /// registering a rule; CLI explanations and the existing accessors use it.
    pub fn metadata(self) -> RuleMetadata {
        let (id, family, purpose, requirements, limitations) = match self {
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
                RuleFamily::Modelling,
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
                RuleFamily::Modelling,
                "Recommend indexing equality for a resolved standard-library three-argument element predicate.",
                "Lexical bindings and resolved callable signatures, types and standard-library identity.",
                "User lookalikes do not match. Ambiguous, unresolved or unsupported calls report limitations; advice is not full type checking or an implemented source rewrite.",
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
                RuleFamily::Suspicious,
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
                "Advise on solve-visible decisions not searched or derivable through supported unconditional definitions.",
                "Complete model root, resolved typed searches/aliases, whole-array views, definitions and dependency closure.",
                "Nested seq_search, simple annotation aliases, array1d views and supported callable output guarantees are followed. Partial selections, unseeded cycles, opaque/optional bodies and unknown annotations withhold affected coverage claims. Computed outputs are not inverted; model-level local decisions report a scoped limitation. Advice does not certify search completeness.",
            ),
            Self::DecisionVariableOperator => (
                "decision-variable-operator",
                RuleFamily::Performance,
                "Advise on selected expensive or reified operators with decision-dependent operands.",
                "Lexical bindings, resolved callable types and operand instantiation.",
                "The operators are ^, div, mod, /, xor, disjunction, implication in either direction, equivalence and not, including equivalent spellings. Parameter-only operands stay quiet; unknown facts report limitations. Reformulation/tabling advice is conditional and has no solving-speed guarantee.",
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
        };
        RuleMetadata {
            id,
            family,
            purpose,
            requirements,
            limitations,
            requires_model: !Self::DEFAULT.contains(&self),
            // Every current rule is implemented, with no fix producer or options.
            available: true,
            fix_support: FixSupport::None,
            options: &[],
        }
    }

    pub fn id(self) -> &'static str {
        self.metadata().id
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::DEFAULT
            .into_iter()
            .chain(Self::THESIS)
            .find(|rule| rule.id() == id)
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

/// Selected rules. The default keeps Zincite's original two syntax rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintOptions {
    pub rules: Vec<Rule>,
}

impl Default for LintOptions {
    fn default() -> Self {
        Self {
            rules: Rule::DEFAULT.to_vec(),
        }
    }
}

impl LintOptions {
    pub fn requires_model(&self) -> bool {
        self.rules.iter().any(|rule| rule.requires_model())
    }

    /// Expand comma-separated IDs, `family:NAME`, `preset:default|thesis|all`
    /// and the legacy `default`/`thesis`/`all` selectors. First occurrence wins.
    /// Availability is checked separately before execution.
    pub fn from_selection(selection: &str) -> Result<Self, String> {
        Ok(Self {
            rules: expand_selectors(selection.split(','))?,
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
            Rule::DEFAULT
                .into_iter()
                .chain(Rule::THESIS)
                .filter(|rule| rule.metadata().family == family)
                .collect()
        } else {
            let name = selector.strip_prefix("preset:").unwrap_or(selector);
            let preset = match name {
                "default" => Some(Rule::DEFAULT.to_vec()),
                "thesis" => Some(Rule::THESIS.to_vec()),
                "all" => Some(Rule::DEFAULT.into_iter().chain(Rule::THESIS).collect()),
                _ => None,
            };
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
