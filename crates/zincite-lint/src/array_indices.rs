//! Numeric array-index advice over domain facts, without domain resolution policy.
use crate::{DomainFacts, FileFinding, ModelContext, Rule, Severity, SourceDiagnostic};
pub(super) struct IndexResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}
pub(super) fn check_array_indices(context: &ModelContext, facts: &DomainFacts) -> IndexResult {
    let mut result = IndexResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for index in &facts.array_indices {
        let file = &context.files[index.file];
        let Some(suppressed) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressed[index.item].contains(&Rule::ArrayIndexStart) {
            continue;
        }
        match index.domain.numeric_minimum() {
            Ok(Some(lower)) if lower != 1 => result.findings.push(FileFinding {
                location: index.location.clone(),
                rule: Rule::ArrayIndexStart,
                severity: Severity::Warning,
                message: format!("numeric array index set starts at {lower}; consider starting at 1 for simpler indexing"),
            }),
            Err(reason) => result.limitations.push(SourceDiagnostic {
                location: index.location.clone(),
                message: format!("array-index-start: {reason}"),
            }),
            _ => {},
        }
    }
    result
}
