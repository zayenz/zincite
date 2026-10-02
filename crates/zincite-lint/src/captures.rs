//! Advisory policy over independently produced lexical binding facts.
use crate::{
    BindingFacts, BindingResolution, DeclarationRole, FileFinding, Instantiation, ModelContext,
    ReferenceKind, Rule, Severity, SourceDiagnostic,
};

pub(super) struct CaptureResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_captures(context: &ModelContext, facts: &BindingFacts) -> CaptureResult {
    let mut result = CaptureResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for reference in &facts.references {
        let Some(owner) = reference.callable else {
            continue;
        };
        if !matches!(
            facts.declarations[owner.0].role,
            DeclarationRole::Function | DeclarationRole::Predicate | DeclarationRole::Test
        ) || reference.kind != ReferenceKind::Value
        {
            continue;
        }
        let file = &context.files[reference.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled()
            || suppressions[reference.item].contains(&Rule::GlobalVariableInFunction)
        {
            continue;
        }
        let reason = match &reference.resolution {
            BindingResolution::Resolved(id) => {
                let declaration = &facts.declarations[id.0];
                if !declaration.top_level || declaration.role != DeclarationRole::Value {
                    continue;
                }
                match declaration.instantiation {
                    Instantiation::Decision => {
                        result.findings.push(FileFinding { fix: None,
                            location: reference.location.clone(),
                            rule: Rule::GlobalVariableInFunction,
                            severity: Severity::Warning,
                            message: format!("consider passing global decision '{}' as an argument to '{}' to make its dependencies clear", declaration.name, facts.declarations[owner.0].name),
                        });
                        continue;
                    }
                    Instantiation::Parameter => continue,
                    Instantiation::Unknown => "declared instantiation is unknown",
                }
            }
            BindingResolution::Unresolved => "value reference is unresolved",
            BindingResolution::Ambiguous(_) => "value reference is ambiguous",
            BindingResolution::Overloads(_) => continue,
        };
        result.limitations.push(SourceDiagnostic {
            location: reference.location.clone(),
            message: format!(
                "global-variable-in-function: {reason} for '{}'",
                reference.name
            ),
        });
    }
    result
}
