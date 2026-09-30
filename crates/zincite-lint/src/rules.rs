//! Stable rule catalogue and explicit selection, without rule dispatch machinery.
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

    pub fn id(self) -> &'static str {
        match self {
            Self::Naming => "naming",
            Self::MissingConstraintLabel => "missing-constraint-label",
            Self::ArrayIndexStart => "array-index-start",
            Self::CompactIf => "compact-if",
            Self::ConstantVariable => "constant-variable",
            Self::EffectiveZeroOne => "effective-zero-one",
            Self::ElementPredicate => "element-predicate",
            Self::ReifiedGlobal => "reified-global",
            Self::GlobalVariableInFunction => "global-variable-in-function",
            Self::UnboundedVariable => "unbounded-variable",
            Self::SearchCoverage => "search-coverage",
            Self::DecisionVariableOperator => "decision-variable-operator",
            Self::UnmarkedSymmetryBreaking => "unmarked-symmetry-breaking",
            Self::UnusedDeclaration => "unused-declaration",
            Self::DecisionVariableGenerator => "decision-variable-generator",
            Self::DecisionVariableCondition => "decision-variable-condition",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::DEFAULT
            .into_iter()
            .chain(Self::THESIS)
            .find(|rule| rule.id() == id)
    }

    /// Registration alone does not mean a rule has an implementation.
    pub fn is_available(self) -> bool {
        matches!(self, Self::Naming | Self::MissingConstraintLabel)
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
    /// Resolve a preset or comma-separated IDs. Availability is checked before
    /// execution, so callers can inspect the complete registered presets.
    pub fn from_selection(selection: &str) -> Result<Self, String> {
        let rules = match selection {
            "default" => Rule::DEFAULT.to_vec(),
            "thesis" => Rule::THESIS.to_vec(),
            "all" => Rule::DEFAULT.into_iter().chain(Rule::THESIS).collect(),
            _ => {
                let mut rules = Vec::new();
                for id in selection.split(',') {
                    let rule =
                        Rule::from_id(id).ok_or_else(|| format!("unknown rule ID '{id}'"))?;
                    if !rules.contains(&rule) {
                        rules.push(rule);
                    }
                }
                rules
            }
        };
        Ok(Self { rules })
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
