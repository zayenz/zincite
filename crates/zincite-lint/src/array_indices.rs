//! Numeric array-index advice over domain facts, without domain resolution policy.
use crate::{
    BindingFacts, CallableFacts, DeclarationRole, DefinitionCoverage, DefinitionEnforcement,
    DefinitionFacts, DefinitionSafety, Domain, DomainFacts, FileFinding, Instantiation,
    ModelContext, NumericBound, Rule, Severity, SourceDiagnostic, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode};
pub(super) struct IndexResult {
    pub findings: Vec<FileFinding>,
    pub limitations: Vec<SourceDiagnostic>,
}
pub(super) fn check_array_indices(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    facts: &DomainFacts,
    definitions: &DefinitionFacts,
) -> IndexResult {
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
            Ok(Some(lower)) if lower != 1 => result.findings.push(FileFinding { fix: None,
                location: index.location.clone(),
                rule: Rule::ArrayIndexStart,
                severity: Severity::Warning,
                message: format!("numeric array index set starts at {lower}; consider starting at 1 for simpler indexing"),
            }),
            Err(_) if inspected_count_bound(context, bindings, calls, definitions, &index.domain) => {},
            Err(reason) => result.limitations.push(SourceDiagnostic {
                location: index.location.clone(),
                message: format!("array-index-start: {reason}"),
            }),
            _ => {},
        }
    }
    result
}

// Checked count evaluation can be supported while its numeric value remains
// unproved. Decline this exact range without claiming its minimum/nonemptiness.
fn inspected_count_bound(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    definitions: &DefinitionFacts,
    domain: &Domain,
) -> bool {
    let Domain::Range {
        lower: NumericBound::Integer(1),
        upper: NumericBound::Defined {
            declaration: id,
            value,
        },
    } = domain
    else {
        return false;
    };
    if !matches!(value.as_ref(), NumericBound::Unsupported(_)) {
        return false;
    }
    let declaration = &bindings.declarations[id.0];
    let ty = &calls.declarations[id.0].ty;
    if !declaration.top_level
        || declaration.role != DeclarationRole::Value
        || !ty.known()
        || crate::value_safety::optional(ty)
        || ty.kind != TypeKind::Int
        || ty.instantiation != Instantiation::Parameter
    {
        return false;
    }
    let file = &context.files[declaration.file];
    let Some(node) = crate::callables::find_node(
        file.parsed.tree(),
        &declaration.syntax_range,
        declaration.role,
    ) else {
        return false;
    };
    let Some(value) = node
        .child_nodes()
        .find(|node| crate::callables::is_expression(node.kind()))
    else {
        return false;
    };
    let mut call: &SyntaxNode = value;
    while call.kind() == NodeKind::ParenthesizedExpression {
        let Some(inner) = call.child_nodes().next() else {
            return false;
        };
        call = inner;
    }
    if node.kind() != NodeKind::Declaration
        || !crate::definitions::annotations_safe(context, declaration.file, node)
        || node.child_nodes().next().is_none_or(|header| {
            header.kind() != NodeKind::ScalarType || header.child_nodes().next().is_some()
        })
        || call.kind() != NodeKind::CallExpression
        || crate::definitions::resolved_call(context, calls, declaration.file, call).is_none_or(
            |selected| {
                !["length", "card"].iter().any(|name| {
                    crate::definitions::core_callable(context, bindings, selected, name)
                })
            },
        )
    {
        return false;
    }
    definitions.definitions.iter().any(|definition| {
        definition.target == *id
            && definition.file == declaration.file
            && definition.item == declaration.item
            && definition.location == file.location(node.range())
            && definition.value == file.location(value.range())
            && definition.instantiation == Instantiation::Parameter
            && definition.coverage == DefinitionCoverage::Scalar
            && definition.enforcement == DefinitionEnforcement::Enforced
            && !definition.cyclic
            && matches!(
                definition.safety,
                DefinitionSafety::Supported | DefinitionSafety::Unknown(_)
            )
    })
}
