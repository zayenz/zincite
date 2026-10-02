//! Clarity-of-intent advice over independently produced definition facts.
use crate::{
    BindingFacts, CallableFacts, DefinitionCoverage, DefinitionEnforcement, DefinitionFacts,
    DefinitionSafety, FileFinding, Instantiation, ModelContext, Rule, Severity, SourceDiagnostic,
};

pub(super) struct ConstantVariableResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_constant_variables(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    facts: &DefinitionFacts,
) -> ConstantVariableResult {
    let mut result = ConstantVariableResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    let mut advised = Vec::new();
    for definition in &facts.definitions {
        let file = &context.files[definition.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled()
            || suppressions[definition.item].contains(&Rule::ConstantVariable)
            || calls.declarations[definition.target.0].ty.instantiation != Instantiation::Decision
            || definition.enforcement == DefinitionEnforcement::Conditional
            || definition.cyclic
        {
            continue;
        }
        if definition.instantiation == Instantiation::Decision
            || (!matches!(
                definition.enforcement,
                DefinitionEnforcement::Unsupported(_)
            ) && matches!(
                definition.coverage,
                DefinitionCoverage::ArrayElement | DefinitionCoverage::Unproved(_)
            ))
        {
            continue;
        }
        let reason = if let DefinitionEnforcement::Unsupported(reason) = &definition.enforcement {
            Some(reason.clone())
        } else if let DefinitionCoverage::Unsupported(reason) = &definition.coverage {
            Some(reason.clone())
        } else if let DefinitionSafety::Unsupported(reason) = &definition.safety {
            Some(reason.clone())
        } else if definition.instantiation == Instantiation::Unknown {
            Some("definition value instantiation is unavailable".into())
        } else {
            None
        };
        if let Some(reason) = reason {
            result.limitations.push(SourceDiagnostic {
                location: definition.location.clone(),
                message: format!("constant-variable: {reason}"),
            });
            continue;
        }
        if definition.safety != DefinitionSafety::Supported || advised.contains(&definition.target)
        {
            continue;
        }
        advised.push(definition.target);
        result.findings.push(FileFinding { fix: None,
            location: definition.location.clone(), rule: Rule::ConstantVariable, severity: Severity::Warning,
            message: format!("consider declaring '{}' as a parameter to express its intent; this definition gives the whole value a parameter expression", bindings.declarations[definition.target.0].name),
        });
    }
    result
}
