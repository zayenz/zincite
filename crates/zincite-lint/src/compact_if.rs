//! Compact conditional candidates and diagnostic policy over existing facts.
use crate::domains::expression_integer;
use crate::{
    BindingFacts, CallableFacts, DefinitionSafety, FileFinding, FileId, Instantiation,
    InstantiationFacts, ModelContext, Rule, Severity, SourceDiagnostic, SourceLocation, TypeKind,
    expression_safety,
};
use zincite_syntax::{NodeKind, SyntaxNode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactIfOutcome {
    Eligible { zero_in_then: bool },
    NotApplicable,
    Unknown(String),
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct CompactIfFact {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub outcome: CompactIfOutcome,
}
#[derive(Debug, Default)]
pub struct CompactIfFacts {
    pub conditionals: Vec<CompactIfFact>,
}

/// Interpret two-branch conditionals in readable user/explicit sources without
/// enabling a rule or editing source.
/// All prerequisite facts must belong to this same retained ModelContext.
/// Eligibility requires a decision Boolean guard, nonoptional integer branches,
/// a proved zero branch and supported safety for both the guard and branch values.
pub fn resolve_compact_ifs(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
) -> CompactIfFacts {
    let mut facts = CompactIfFacts::default();
    let interpreter = Interpreter {
        context,
        bindings,
        calls,
        instantiations,
    };
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            interpreter.walk(file, item, node, &mut facts);
        }
    }
    facts
}
struct Interpreter<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
}
impl Interpreter<'_> {
    fn walk(&self, file: FileId, item: usize, node: &SyntaxNode, facts: &mut CompactIfFacts) {
        if node.kind() == NodeKind::ConditionalExpression {
            facts.conditionals.push(CompactIfFact {
                file,
                item,
                location: self.context.files[file].location(node.range()),
                outcome: self.interpret(file, node),
            });
        }
        for child in node.child_nodes() {
            self.walk(file, item, child, facts);
        }
    }
    fn interpret(&self, file: FileId, node: &SyntaxNode) -> CompactIfOutcome {
        use CompactIfOutcome::*;
        let branches: Vec<_> = node.child_nodes().collect();
        if branches.len() != 2
            || branches[0].kind() != NodeKind::ConditionalBranch
            || branches[1].kind() != NodeKind::ElseBranch
        {
            return NotApplicable;
        }
        let first: Vec<_> = branches[0].child_nodes().collect();
        let Some(other) = branches[1].child_nodes().last() else {
            return NotApplicable;
        };
        if first.len() != 2 {
            return NotApplicable;
        }
        let condition = first[0];
        let then = first[1];
        let condition_range = self.context.files[file].location(condition.range()).range;
        let Some(inst) = self
            .instantiations
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == condition_range)
        else {
            return Unsupported("condition instantiation is unavailable".into());
        };
        if inst.instantiation == Instantiation::Parameter {
            return NotApplicable;
        }
        if inst.instantiation == Instantiation::Unknown {
            return Unknown(
                inst.reason
                    .clone()
                    .unwrap_or_else(|| "condition instantiation is unknown".into()),
            );
        }
        for (expression, condition) in [(condition, true), (then, false), (other, false)] {
            let range = self.context.files[file].location(expression.range()).range;
            let Some(ty) = self
                .calls
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
            else {
                return Unsupported("condition/branch type is unavailable".into());
            };
            if ty.optional {
                return NotApplicable;
            }
            if matches!(ty.kind, TypeKind::Unknown(_) | TypeKind::Variable { .. }) {
                return Unsupported("condition/branch type is unsupported".into());
            }
            if if condition {
                ty.kind != TypeKind::Bool
            } else {
                ty.kind != TypeKind::Int
            } {
                return NotApplicable;
            }
        }
        let zero_in_then =
            expression_integer(self.context, self.bindings, file, then) == Ok(Some(0));
        if !zero_in_then
            && expression_integer(self.context, self.bindings, file, other) != Ok(Some(0))
        {
            return NotApplicable;
        }
        for expression in [condition, then, other] {
            match expression_safety(self.context, self.bindings, self.calls, file, expression) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) => return Unknown(reason),
                DefinitionSafety::Unsupported(reason) => return Unsupported(reason),
            }
        }
        Eligible { zero_in_then }
    }
}

pub(super) struct CompactResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}
pub(super) fn check_compact_ifs(context: &ModelContext, facts: &CompactIfFacts) -> CompactResult {
    let mut result = CompactResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for fact in &facts.conditionals {
        let file = &context.files[fact.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressions[fact.item].contains(&Rule::CompactIf) {
            continue;
        }
        match &fact.outcome {
            CompactIfOutcome::Eligible { zero_in_then } => {
                let formulation = if *zero_in_then {
                    "bool2int(not condition) * value"
                } else {
                    "bool2int(condition) * value"
                };
                result.findings.push(FileFinding { fix: None,
                    location: fact.location.clone(),
                    rule: Rule::CompactIf,
                    severity: Severity::Warning,
                    message: format!("consider the compact form {formulation} for this zero-branch conditional; compare readability for your model"),
                });
            }
            CompactIfOutcome::Unknown(reason) | CompactIfOutcome::Unsupported(reason) => {
                result.limitations.push(SourceDiagnostic {
                    location: fact.location.clone(),
                    message: format!("compact-if: {reason}"),
                })
            }
            CompactIfOutcome::NotApplicable => {}
        }
    }
    result
}
