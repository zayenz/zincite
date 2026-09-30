//! Readability advice for resolved standard element predicates.
use crate::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationRole, FileFinding, ModelContext, Rule,
    Severity, SourceDiagnostic, SourceKind,
};

pub(super) struct ElementResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}

pub(super) fn check_element(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &CallableFacts,
) -> ElementResult {
    let mut result = ElementResult {
        findings: Vec::new(),
        limitations: Vec::new(),
    };
    for call in &facts.calls {
        let file = &context.files[call.file];
        let Some(suppressed) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressed[call.item].contains(&Rule::ElementPredicate) {
            continue;
        }
        let reason = match &call.outcome {
            CallOutcome::Resolved {
                declaration,
                parameters,
                ..
            } => {
                let declaration = &bindings.declarations[declaration.0];
                let source = &context.files[declaration.file];
                if declaration.name == "element"
                    && declaration.role == DeclarationRole::Predicate
                    && parameters.len() == 3
                    && source.kind == SourceKind::StandardLibrary
                    && context.standard_element.as_ref() == Some(&source.canonical_path)
                {
                    result.findings.push(FileFinding {location:call.location.clone(),rule:Rule::ElementPredicate,severity:Severity::Warning,message:"consider indexing equality (value = array[index]) to make this element constraint easier to read".into()});
                }
                continue;
            }
            CallOutcome::NoMatch { .. } => continue,
            CallOutcome::Unresolved { reason } | CallOutcome::Unsupported { reason, .. } => {
                reason.as_str()
            }
            CallOutcome::Ambiguous { .. } => "callable selection is ambiguous",
        };
        result.limitations.push(SourceDiagnostic {
            location: call.location.clone(),
            message: format!("element-predicate: {reason} for '{}'", call.name),
        });
    }
    result
}
