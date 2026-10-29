//! Shadowing policy consumes enclosing-binding facts; it does not rebuild scopes.
use crate::{
    BindingFacts, BindingResolution, Declaration, DeclarationRole, FileFinding, ModelContext, Rule,
    SourceDiagnostic,
};

pub(super) fn check_shadowing(
    context: &ModelContext,
    facts: &BindingFacts,
    ignore_names: &[String],
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    for inner in &facts.declarations {
        if !matches!(
            inner.role,
            DeclarationRole::Parameter | DeclarationRole::Local | DeclarationRole::Generator
        ) || inner.name.starts_with('_')
            || ignore_names.contains(&inner.name)
        {
            continue;
        }
        let file = &context.files[inner.file];
        let Some(suppressions) = &file.suppressions else {
            continue;
        };
        if !file.warnings_enabled() || suppressions[inner.item].contains(&Rule::SuspiciousShadowing)
        {
            continue;
        }
        // A formal named after the global it receives is a deliberate choice.
        let reported = |outer: &Declaration| {
            value_binding(outer.role)
                && !(inner.role == DeclarationRole::Parameter && outer.top_level)
        };
        let outer = match &inner.shadowed {
            BindingResolution::Resolved(id) => &facts.declarations[id.0],
            BindingResolution::Ambiguous(ids) => {
                if ids.iter().any(|id| reported(&facts.declarations[id.0])) {
                    limitations.push(SourceDiagnostic {
                        location: inner.location.clone(),
                        message: format!(
                            "suspicious-shadowing: enclosing binding for '{}' is ambiguous",
                            inner.name
                        ),
                    });
                }
                continue;
            }
            BindingResolution::Unresolved | BindingResolution::Overloads(_) => continue,
        };
        if !reported(outer) {
            continue;
        }
        let location = &outer.location;
        findings.push(FileFinding { fix: None,
            location: inner.location.clone(),
            rule: Rule::SuspiciousShadowing,
            severity: Rule::SuspiciousShadowing.severity(),
            message: format!("binding '{}' hides an enclosing binding at {}:{}:{} bytes {}..{}; consider a distinct name to make the scope clear", inner.name, location.path.display(), location.line, location.column, location.range.start, location.range.end),
        });
    }
    (findings, limitations)
}

fn value_binding(role: DeclarationRole) -> bool {
    matches!(
        role,
        DeclarationRole::Value
            | DeclarationRole::Parameter
            | DeclarationRole::Local
            | DeclarationRole::Generator
            | DeclarationRole::Index
            | DeclarationRole::EnumMember
    )
}
