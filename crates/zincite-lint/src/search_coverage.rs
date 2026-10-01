//! Missing-search advice over independently produced coverage facts.
use crate::{
    BindingFacts, DeclarationRole, FileFinding, Instantiation, ModelContext, ModelRootState, Rule,
    SearchCoverage, SearchFacts, Severity,
};
pub(super) fn check_search_coverage(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &SearchFacts,
) -> Vec<FileFinding> {
    if facts.root_state != ModelRootState::Complete {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for fact in &facts.declarations {
        let d = &bindings.declarations[fact.declaration.0];
        let source = &context.files[d.file];
        if !source.warnings_enabled()
            || !d.top_level
            || d.role != DeclarationRole::Value
            || d.instantiation != Instantiation::Decision
            || !matches!(
                fact.coverage,
                SearchCoverage::Uncovered | SearchCoverage::PartialArray
            )
        {
            continue;
        }
        if source
            .suppressions
            .as_ref()
            .is_none_or(|s| s[d.item].contains(&Rule::SearchCoverage))
        {
            continue;
        }
        findings.push(FileFinding {
            location: d.location.clone(),
            rule: Rule::SearchCoverage,
            severity: Severity::Warning,
            message: format!(
                concat!(
                    "consider including '{}' in the solve search; its whole value is not covered ",
                    "by a supported search or complete direct/callable definition"
                ),
                d.name
            ),
        });
    }
    findings
}
