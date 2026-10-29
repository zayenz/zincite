//! Calls whose resolved standard callable carries mzn_deprecated.
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

use crate::callables::CallableView;
use crate::{
    BindingFacts, CallOutcome, DeclarationId, DeclarationRole, FileFinding, ModelContext, Rule,
    SourceDiagnostic, SourceKind,
};

struct Deprecation {
    declaration: DeclarationId,
    version: String,
    explanation: String,
}

pub(super) fn check_deprecated_calls(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableView<'_>,
) -> (Vec<FileFinding>, Vec<SourceDiagnostic>) {
    let deprecations = standard_deprecations(context, bindings);
    let mut findings = Vec::new();
    let mut limitations = Vec::new();
    if deprecations.is_empty() {
        return (findings, limitations);
    }
    let deprecation = |id| {
        deprecations
            .iter()
            .find(|d| same_callable(context, bindings, calls, d.declaration, id))
    };
    // Spelling only signals uncertainty; it never proves a deprecated identity.
    let namesake = |name: &str| {
        deprecations
            .iter()
            .any(|d| bindings.declarations[d.declaration.0].name == name)
    };
    for call in calls.calls.iter().filter(|call| !call.symbolic_operator) {
        let file = &context.files[call.file];
        if !file.warnings_enabled()
            || file
                .suppressions
                .as_ref()
                .is_none_or(|s| s[call.item].contains(&Rule::DeprecatedCall))
        {
            continue;
        }
        let uncertain = match &call.outcome {
            CallOutcome::Resolved { declaration, .. } => {
                if let Some(deprecation) = deprecation(*declaration) {
                    findings.push(FileFinding {
                        fix: None,
                        location: call.location.clone(),
                        rule: Rule::DeprecatedCall,
                        severity: Rule::DeprecatedCall.severity(),
                        message: format!(
                            "'{}' is deprecated since MiniZinc {}: {}",
                            call.name, deprecation.version, deprecation.explanation
                        ),
                    });
                }
                None
            }
            CallOutcome::Intrinsic { .. } => None,
            CallOutcome::Ambiguous { candidates } => candidates
                .iter()
                .any(|id| deprecation(*id).is_some())
                .then_some("callable selection is ambiguous"),
            CallOutcome::Unsupported { reason, candidates } => candidates
                .iter()
                .any(|id| deprecation(*id).is_some())
                .then_some(reason.as_str()),
            CallOutcome::NoMatch { reason } | CallOutcome::Unresolved { reason } => {
                namesake(&call.name).then_some(reason.as_str())
            }
        };
        if let Some(reason) = uncertain {
            limitations.push(SourceDiagnostic {
                location: call.location.clone(),
                message: format!(
                    "deprecated-call: cannot establish whether '{}' selects a deprecated standard callable: {reason}",
                    call.name
                ),
            });
        }
    }
    (findings, limitations)
}

/// A deprecation companion redeclares an implementation as a body-less
/// prototype, so the marker applies to every declaration with its signature.
fn same_callable(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableView<'_>,
    marked: DeclarationId,
    selected: DeclarationId,
) -> bool {
    if marked == selected {
        return true;
    }
    let (a, b) = (
        &bindings.declarations[marked.0],
        &bindings.declarations[selected.0],
    );
    let signature = |id| calls.signatures.iter().find(|s| s.declaration == id);
    a.name == b.name
        && a.role == b.role
        && a.top_level
        && b.top_level
        && context.files[b.file].kind == SourceKind::StandardLibrary
        && signature(marked)
            .zip(signature(selected))
            .is_some_and(|(a, b)| {
                a.return_type == b.return_type
                    && a.parameters.len() == b.parameters.len()
                    && a.parameters
                        .iter()
                        .zip(&b.parameters)
                        .all(|(a, b)| a.ty == b.ty)
            })
}

fn standard_deprecations(context: &ModelContext, bindings: &BindingFacts) -> Vec<Deprecation> {
    bindings
        .declarations
        .iter()
        .filter(|declaration| {
            declaration.top_level
                && matches!(
                    declaration.role,
                    DeclarationRole::Predicate | DeclarationRole::Function | DeclarationRole::Test
                )
                && context.files[declaration.file].kind == SourceKind::StandardLibrary
        })
        .filter_map(|declaration| {
            let source = &context.files[declaration.file];
            let node = source.parsed.tree().child_nodes().nth(declaration.item)?;
            let (version, explanation) = node
                .child_nodes()
                .filter(|child| child.kind() == NodeKind::Annotation)
                .find_map(|annotation| deprecation_arguments(source, annotation))?;
            Some(Deprecation {
                declaration: declaration.id,
                version,
                explanation,
            })
        })
        .collect()
}

/// The two string literals of a written `mzn_deprecated(version, explanation)`.
fn deprecation_arguments(
    source: &crate::ModelFile,
    annotation: &SyntaxNode,
) -> Option<(String, String)> {
    let parsed = &source.parsed;
    let call = annotation
        .child_nodes()
        .find(|child| child.kind() == NodeKind::CallExpression)?;
    let token_text = |node: &SyntaxNode, kind: TokenKind| {
        node.children().iter().find_map(|child| match child {
            SyntaxElement::Token(index) if parsed.tokens()[*index].kind == kind => {
                Some(&parsed.source()[parsed.tokens()[*index].range.clone()])
            }
            _ => None,
        })
    };
    if token_text(call, TokenKind::Identifier)? != "mzn_deprecated" {
        return None;
    }
    let strings: Vec<_> = call
        .child_nodes()
        .map(|argument| token_text(argument, TokenKind::StringLiteral))
        .collect::<Option<_>>()?;
    let [version, explanation] = strings.as_slice() else {
        return None;
    };
    let unquote = |literal: &str| literal[1..literal.len() - 1].to_owned();
    Some((unquote(version), unquote(explanation)))
}
