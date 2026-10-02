//! Domain advice over declared-domain and complete enforced-definition facts.
use crate::{
    BindingFacts, DeclarationRole, DefinitionCoverage, DefinitionEnforcement, DefinitionFacts,
    DefinitionSafety, DomainFacts, FileFinding, Instantiation, ModelContext, Rule, Severity,
    SourceDiagnostic,
};

pub(super) struct UnboundedResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_unbounded_variables(
    context: &ModelContext,
    bindings: &BindingFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
) -> UnboundedResult {
    let mut result = UnboundedResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    let defined = definitions.bounded_or_defined_targets(bindings, domains);
    for declaration in &bindings.declarations {
        let file = &context.files[declaration.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled()
            || !matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Local
            )
            || declaration.instantiation != Instantiation::Decision
            || defined.contains(&declaration.id)
            || suppressions[declaration.item].contains(&Rule::UnboundedVariable)
        {
            continue;
        }
        let unbounded = domains.declarations[declaration.id.0]
            .domain
            .has_unbounded_numeric_elements();
        let mut limitation = match unbounded {
            Ok(false) => continue,
            Ok(true) => None,
            Err(reason) => Some(reason.to_owned()),
        };

        for definition in definitions
            .definitions
            .iter()
            .filter(|d| d.target == declaration.id)
        {
            if definition.enforcement == DefinitionEnforcement::Conditional {
                continue;
            }
            // Reverse equality candidates can form a cycle around a bounded
            // source. Keep missing safety explicit when dependencies are anchored;
            // a genuine unanchored/self cycle still cannot define this value.
            if definition.cyclic
                && !definition
                    .dependencies
                    .iter()
                    .all(|id| defined.contains(id))
            {
                continue;
            }
            if !matches!(
                definition.enforcement,
                DefinitionEnforcement::Unsupported(_)
            ) && matches!(
                definition.coverage,
                DefinitionCoverage::ArrayElement | DefinitionCoverage::Unproved(_)
            ) {
                continue;
            }
            let reason = match (
                &definition.enforcement,
                &definition.coverage,
                &definition.safety,
            ) {
                (DefinitionEnforcement::Unsupported(reason), _, _)
                | (_, DefinitionCoverage::Unsupported(reason), _)
                | (
                    _,
                    _,
                    DefinitionSafety::Unsupported(reason) | DefinitionSafety::Unknown(reason),
                ) => Some(reason),
                _ => None,
            };
            if let Some(reason) = reason {
                limitation = Some(reason.clone());
            }
        }
        if let Some(reason) = limitation {
            result.limitations.push(SourceDiagnostic {
                location: declaration.location.clone(),
                message: format!("unbounded-variable: {reason} for '{}'", declaration.name),
            });
        } else {
            result.findings.push(FileFinding { fix: None,
                location: declaration.location.clone(),
                rule: Rule::UnboundedVariable,
                severity: Severity::Warning,
                message: format!("consider an explicit domain for '{}'; its numeric value or array elements have no explicit bounds or proven complete definition", declaration.name),
            });
        }
    }
    result
}
