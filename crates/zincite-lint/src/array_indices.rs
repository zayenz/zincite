//! Numeric array-index advice over domain facts, without domain resolution policy.
use crate::{
    BindingFacts, CallableFacts, DeclarationRole, DefinitionCoverage, DefinitionEnforcement,
    DefinitionFacts, DefinitionSafety, Domain, DomainFacts, FileFinding, Instantiation,
    InstantiationFacts, ModelContext, NumericBound, Rule, Severity, SourceDiagnostic, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxNode, TokenKind};
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
    instantiations: &InstantiationFacts,
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
            Err(_) if inspected_count_bound(context, bindings, calls, definitions, &index.domain)
                || inspected_length_quotient_bound(context, bindings, calls, instantiations, facts, definitions, &index.domain) => {},
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

// Inspect this written symbolic range without evaluating its minimum or
// exporting stronger domain/definition facts.
fn inspected_length_quotient_bound(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
    mut domain: &Domain,
) -> bool {
    let integer = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    let reference = |file: crate::FileId, node: &SyntaxNode| {
        let tokens = crate::domains::tokens(&context.files[file].parsed, node);
        if node.kind() != NodeKind::Expression
            || tokens.len() != 1
            || !matches!(
                tokens[0].kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier
            )
        {
            return None;
        }
        let range = context.files[file].location(node.range()).range;
        bindings
            .references
            .iter()
            .find(|r| r.file == file && r.location.range == range)
            .and_then(|r| match r.resolution {
                crate::BindingResolution::Resolved(id) => Some(id),
                _ => None,
            })
    };
    let mut active = Vec::new();
    while let Domain::Named {
        declaration: id,
        domain: target,
    } = domain
    {
        let declaration = &bindings.declarations[id.0];
        let ty = &calls.declarations[id.0].ty;
        if active.contains(id)
            || !declaration.top_level
            || declaration.role != DeclarationRole::Value
            || !ty.known()
            || crate::value_safety::optional(ty)
            || ty.instantiation != Instantiation::Parameter
            || !matches!(&ty.kind, TypeKind::Set(element) if integer(element))
        {
            return false;
        }
        active.push(*id);
        let file = declaration.file;
        let source = &context.files[file];
        let Some(written) = crate::callables::find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) else {
            return false;
        };
        let Some(value) = written
            .child_nodes()
            .find(|n| crate::callables::is_expression(n.kind()))
        else {
            return false;
        };
        if written.kind() != NodeKind::Declaration
            || !crate::definitions::annotations_safe(context, file, written)
            || written.child_nodes().next().is_none_or(|header| {
                header.kind() != NodeKind::SetType
                    || !crate::definitions::annotations_safe(context, file, header)
                    || header.child_nodes().next().is_none_or(|element| {
                        element.kind() != NodeKind::ScalarType
                            || element.child_nodes().next().is_some()
                            || !crate::definitions::annotations_safe(context, file, element)
                    })
            })
            || !definitions.definitions.iter().any(|definition| {
                definition.target == *id
                    && definition.file == file
                    && definition.item == declaration.item
                    && definition.location == source.location(written.range())
                    && definition.value == source.location(value.range())
                    && definition.instantiation == Instantiation::Parameter
                    && definition.coverage == DefinitionCoverage::Scalar
                    && definition.enforcement == DefinitionEnforcement::Enforced
                    && !definition.cyclic
            })
        {
            return false;
        }
        if let Domain::Named {
            declaration: next, ..
        } = target.as_ref()
        {
            if reference(file, value) != Some(*next) {
                return false;
            }
            domain = target;
            continue;
        }
        if !matches!(
            target.as_ref(),
            Domain::Range {
                lower: NumericBound::Integer(1),
                ..
            }
        ) || value.kind() != NodeKind::RangeExpression
            || crate::callables::core_operation(context, bindings, calls, file, value, "..")
                != Ok(true)
            || !crate::definitions::annotations_safe(context, file, value)
        {
            return false;
        }
        let endpoints: Vec<_> = value.child_nodes().collect();
        let [lower, upper] = endpoints.as_slice() else {
            return false;
        };
        let lower_tokens = crate::domains::tokens(&source.parsed, lower);
        if lower.kind() != NodeKind::Expression || lower_tokens.len() != 1
            || lower_tokens[0].kind != TokenKind::IntegerLiteral
            || &source.parsed.source()[lower_tokens[0].range.clone()] != "1"
            || crate::callables::operation_fact(context, calls, file, value).is_none_or(|fact|
                !matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(integer)
                        && return_type.known() && !crate::value_safety::optional(return_type)
                        && return_type.instantiation == Instantiation::Parameter
                        && matches!(&return_type.kind, TypeKind::Set(element) if integer(element))))
        { return false;
        }
        let mut quotient = *upper;
        while quotient.kind() == NodeKind::ParenthesizedExpression {
            let Some(inner) = quotient.child_nodes().next() else {
                return false;
            };
            quotient = inner;
        }
        let operands: Vec<_> = quotient.child_nodes().collect();
        if quotient.kind() != NodeKind::BinaryExpression || operands.len() != 2
            || crate::callables::core_operation(context, bindings, calls, file, quotient, "div") != Ok(true)
            || crate::callables::operation_fact(context, calls, file, quotient).is_none_or(|fact|
                !matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(integer) && integer(return_type)))
        { return false;
        }
        for (row, length) in operands.iter().enumerate() {
            let Some(argument) = crate::callable_definitions::length_argument(
                context, bindings, calls, file, length,
            ) else {
                return false;
            };
            let Some(id) = reference(file, argument) else {
                return false;
            };
            let ty = &calls.declarations[id.0].ty;
            if (row == 0
                && !matches!(&ty.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0]) && element.known()
                    && !crate::value_safety::optional(element) && element.kind == TypeKind::Int))
                || (row == 1
                    && (ty.instantiation != Instantiation::Parameter
                        || !matches!(&ty.kind, TypeKind::Set(element) if integer(element))))
            {
                return false;
            }
        }
        let safety = crate::callable_definitions::initialized_expression_safety(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            (file, upper),
            None,
        );
        return matches!(
            safety,
            DefinitionSafety::Supported | DefinitionSafety::Unknown(_)
        );
    }
    false
}
