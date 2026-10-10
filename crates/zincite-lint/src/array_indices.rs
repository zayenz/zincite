//! Numeric array-index advice over domain facts, without domain resolution policy.
use crate::{
    ArrayIndexSet, BindingFacts, CallableFacts, DeclarationRole, DefinitionCoverage,
    DefinitionEnforcement, DefinitionFacts, DefinitionSafety, Domain, DomainFacts, FileFinding,
    Instantiation, InstantiationFacts, ModelContext, NumericBound, Rule, Severity,
    SourceDiagnostic, TypeKind,
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
            Err(_) if inspected_count_bound(context, bindings, calls, instantiations, facts, definitions, &index.domain)
                || inspected_initialized_axis_source(context, bindings, calls, instantiations, facts, definitions, &index.domain)
                || inspected_local_selection_bound(context, bindings, calls, instantiations, facts, index) => {},
            Err(reason) => result.limitations.push(SourceDiagnostic {
                location: index.location.clone(),
                message: format!("array-index-start: {reason}"),
            }),
            _ => {},
        }
    }
    result
}

// A checked parameter source can supply a symbolic local axis without a
// numeric minimum, membership guarantee or definition of the owning array.
fn inspected_local_selection_bound(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    index: &ArrayIndexSet,
) -> bool {
    let Domain::Range {
        lower: NumericBound::Integer(1),
        upper,
    } = &index.domain
    else {
        return false;
    };
    let (id, value, conditional) = match upper {
        NumericBound::Defined { declaration, value } => (declaration, value, false),
        NumericBound::Arithmetic {
            operator: TokenKind::Plus,
            operands,
        } => {
            let [
                NumericBound::Defined { declaration, value },
                NumericBound::Integer(1),
            ] = operands.as_slice()
            else {
                return false;
            };
            (declaration, value, true)
        }
        _ => return false,
    };
    let declaration = &bindings.declarations[id.0];
    let integer = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    if !matches!(value.as_ref(), NumericBound::Unsupported(_))
        || declaration.role != DeclarationRole::Local
        || declaration.file != index.file
        || declaration.item != index.item
        || !integer(&calls.declarations[id.0].ty)
    {
        return false;
    }
    let file = index.file;
    let source = &context.files[file];
    let reference = |node: &SyntaxNode| {
        let tokens = crate::domains::tokens(&source.parsed, node);
        if node.kind() != NodeKind::Expression
            || node.child_nodes().next().is_some()
            || !matches!(tokens.as_slice(), [token]
                if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        {
            return None;
        }
        crate::definitions::resolved_reference(context, bindings, file, node)
    };
    let typed = |node: &SyntaxNode| {
        let range = source.location(node.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    let range = index.location.range.start - source.byte_offset
        ..index.location.range.end - source.byte_offset;
    let Some(mut node) = source.parsed.tree().child_nodes().nth(index.item) else {
        return false;
    };
    let mut path = Vec::new();
    while node.kind() != NodeKind::RangeExpression || node.range() != range {
        path.push(node);
        let Some(child) = node
            .child_nodes()
            .find(|n| n.range().start <= range.start && range.end <= n.range().end)
        else {
            return false;
        };
        node = child;
    }
    let endpoints: Vec<_> = node.child_nodes().collect();
    let [lower, upper] = endpoints.as_slice() else {
        return false;
    };
    let local_reference = if conditional {
        let operands: Vec<_> = upper.child_nodes().collect();
        let [local, increment] = operands.as_slice() else {
            return false;
        };
        if upper.kind() != NodeKind::BinaryExpression
            || typed(upper).is_none_or(|t| !integer(t))
            || !matches!(crate::domains::tokens(&source.parsed, increment).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral
                    && &source.parsed.source()[token.range.clone()] == "1")
            || crate::callables::core_operation(context, bindings, calls, file, upper, "+")
                != Ok(true)
            || crate::callables::operation_fact(context, calls, file, upper).is_none_or(|fact| {
                !matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(integer)
                        && integer(return_type))
            })
        {
            return false;
        }
        *local
    } else {
        *upper
    };
    if !matches!(crate::domains::tokens(&source.parsed, lower).as_slice(),
        [token] if token.kind == TokenKind::IntegerLiteral)
        || &source.parsed.source()[lower.range()] != "1"
        || reference(local_reference) != Some(*id)
        || crate::callables::core_operation(context, bindings, calls, file, node, "..") != Ok(true)
        || crate::callables::operation_fact(context, calls, file, node).is_none_or(|fact| {
            !matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                if parameters.len() == 2 && parameters.iter().all(integer)
                    && return_type.known() && !crate::value_safety::optional(return_type)
                    && return_type.instantiation == Instantiation::Parameter
                    && matches!(&return_type.kind, TypeKind::Set(element) if integer(element)))
        })
        || path
            .iter()
            .chain([&node])
            .any(|n| !crate::definitions::annotations_safe(context, file, n))
    {
        return false;
    }
    let Some(local) = path
        .iter()
        .find(|n| n.kind() == NodeKind::LetBlock)
        .and_then(|block| {
            block
                .child_nodes()
                .find(|n| n.range() == declaration.syntax_range)
        })
    else {
        return false;
    };
    if local.kind() != NodeKind::Declaration
        || local.range().end > range.start
        || !crate::definitions::annotations_safe(context, file, local)
        || local.child_nodes().next().is_none_or(|header| {
            header.kind() != NodeKind::ScalarType
                || header.child_nodes().next().is_some()
                || !crate::definitions::annotations_safe(context, file, header)
        })
    {
        return false;
    }
    let Some(value) = local
        .child_nodes()
        .find(|n| crate::callables::is_expression(n.kind()))
    else {
        return false;
    };
    if conditional {
        let branches: Vec<_> = value.child_nodes().collect();
        let [first, otherwise] = branches.as_slice() else {
            return false;
        };
        let first_values: Vec<_> = first.child_nodes().collect();
        let fallback: Vec<_> = otherwise.child_nodes().collect();
        let ([guard, _], [extremum]) = (first_values.as_slice(), fallback.as_slice()) else {
            return false;
        };
        if value.kind() != NodeKind::ConditionalExpression
            || first.kind() != NodeKind::ConditionalBranch
            || otherwise.kind() != NodeKind::ElseBranch
            || typed(value).is_none_or(|t| !integer(t))
            || path.iter().any(|n| {
                matches!(
                    n.kind(),
                    NodeKind::GeneratorCallExpression
                        | NodeKind::ArrayComprehension
                        | NodeKind::SetComprehension
                        | NodeKind::IndexedArrayComprehension
                )
            })
        {
            return false;
        }
        let Some(formal) = crate::callable_definitions::parameter_index_extremum(
            context, bindings, calls, file, extremum,
        ) else {
            return false;
        };
        let guard_values: Vec<_> = guard.child_nodes().collect();
        let [selection, _] = guard_values.as_slice() else {
            return false;
        };
        let selected: Vec<_> = selection.child_nodes().collect();
        let [subject, _] = selected.as_slice() else {
            return false;
        };
        if selection.kind() != NodeKind::ArrayAccessExpression
            || reference(subject) != Some(formal)
            || crate::callables::core_operation(context, bindings, calls, file, guard, "=")
                != Ok(true)
        {
            return false;
        }
        let formal_type = &calls.declarations[formal.0].ty;
        if !formal_type.known()
            || crate::value_safety::optional(formal_type)
            || formal_type.instantiation != Instantiation::Parameter
            || typed(subject) != Some(formal_type)
            || !matches!(&formal_type.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0]) && integer(element))
        {
            return false;
        }
        let Some(owner) = bindings.references.iter().find_map(|r| {
            (r.file == file
                && r.kind == crate::ReferenceKind::Value
                && local_reference.range().start + source.byte_offset <= r.location.range.start
                && r.location.range.end <= local_reference.range().end + source.byte_offset
                && r.resolution == crate::BindingResolution::Resolved(*id))
            .then_some(r.callable)
            .flatten()
        }) else {
            return false;
        };
        let Some(signature) = calls.signatures.iter().find(|s| s.declaration == owner) else {
            return false;
        };
        let Some(position) = (0..signature.parameters.len()).find(|&position| {
            crate::callables::formal_parameter(context, bindings, owner, position) == Some(formal)
        }) else {
            return false;
        };
        if signature.parameters[position].has_default
            || signature.parameters[position].ty != *formal_type
            || bindings.references.iter().any(|r| {
                r.file == file
                    && r.kind == crate::ReferenceKind::Value
                    && value.range().start + source.byte_offset <= r.location.range.start
                    && r.location.range.end <= value.range().end + source.byte_offset
                    && matches!(r.resolution, crate::BindingResolution::Resolved(target)
                        if bindings.declarations[target.0].role == DeclarationRole::Local
                            || bindings.declarations[target.0].role == DeclarationRole::Generator
                            || bindings.declarations[target.0].role == DeclarationRole::Parameter
                                && (target != formal || r.callable != Some(owner)))
            })
        {
            return false;
        }
        let Domain::Array { indices, element } =
            crate::domains::bare_index_domain(&domains.declarations[formal.0].domain)
        else {
            return false;
        };
        if indices.len() != 1
            || indices[0].numeric_minimum().is_err()
            || element.numeric_minimum().is_err()
        {
            return false;
        }
        let formal_declaration = &bindings.declarations[formal.0];
        let Some(written) = crate::callables::find_node(
            source.parsed.tree(),
            &formal_declaration.syntax_range,
            formal_declaration.role,
        ) else {
            return false;
        };
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(context, file, node)
                || node.kind() == NodeKind::ScalarType
                    && !crate::domains::tokens(&source.parsed, node)
                        .iter()
                        .any(|token| token.kind == TokenKind::Int)
            {
                return false;
            }
            nodes.extend(node.child_nodes());
        }
        return matches!(
            crate::callable_definitions::initialized_conditional_bound_safety(
                context,
                bindings,
                calls,
                instantiations,
                domains,
                (file, value),
            ),
            Some(DefinitionSafety::Unknown(_))
        );
    }
    let children: Vec<_> = value.child_nodes().collect();
    let [subject, row, column] = children.as_slice() else {
        return false;
    };
    if value.kind() != NodeKind::ArrayAccessExpression || typed(value).is_none_or(|t| !integer(t)) {
        return false;
    }
    let Some(array) = reference(subject) else {
        return false;
    };
    let array_declaration = &bindings.declarations[array.0];
    let array_type = &calls.declarations[array.0].ty;
    if array_declaration.role != DeclarationRole::Value
        || !array_declaration.top_level
        || !array_type.known()
        || crate::value_safety::optional(array_type)
        || array_type.instantiation != Instantiation::Parameter
        || typed(subject) != Some(array_type)
        || !matches!(&array_type.kind, TypeKind::Array { indices, element }
            if indices.len() == 2 && indices.iter().all(integer) && integer(element))
    {
        return false;
    }
    let Some(written) = crate::callables::find_node(
        context.files[array_declaration.file].parsed.tree(),
        &array_declaration.syntax_range,
        array_declaration.role,
    ) else {
        return false;
    };
    if written
        .child_nodes()
        .any(|n| crate::callables::is_expression(n.kind()))
    {
        return false;
    }
    let mut domain = &domains.declarations[array.0].domain;
    while let Domain::Named { domain: inner, .. } = domain {
        domain = inner;
    }
    let Domain::Array { indices, element } = domain else {
        return false;
    };
    if indices.len() != 2
        || element.numeric_minimum().is_err()
        || indices.iter().any(|axis| axis.numeric_minimum().is_err())
    {
        return false;
    }
    let mut generators = Vec::new();
    for parent in &path {
        if matches!(
            parent.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) {
            if parent.kind() != NodeKind::GeneratorCallExpression
                || crate::callables::core_operation(context, bindings, calls, file, parent, "forall") != Ok(true)
                || crate::callables::operation_fact(context, calls, file, parent).is_none_or(|fact| {
                    !matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && parameters[0].known()
                            && !crate::value_safety::optional(&parameters[0])
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && integer(&indices[0])
                                    && element.known() && !crate::value_safety::optional(element)
                                    && element.kind == TypeKind::Bool)
                            && return_type.known() && !crate::value_safety::optional(return_type)
                            && return_type.kind == TypeKind::Bool)
                })
            {
                return false;
            }
            let Some(list) = parent
                .child_nodes()
                .find(|n| n.kind() == NodeKind::GeneratorList)
            else {
                return false;
            };
            for generator in list.child_nodes() {
                let headers: Vec<_> = generator.child_nodes().collect();
                if headers.len() != 1
                    || generator.range().end > local.range().start
                    || !crate::domains::tokens(&source.parsed, generator)
                        .iter()
                        .any(|t| t.kind == TokenKind::In)
                    || typed(headers[0]).is_none_or(|t| {
                        !t.known()
                            || crate::value_safety::optional(t)
                            || t.instantiation != Instantiation::Parameter
                            || !matches!(&t.kind, TypeKind::Set(element) if integer(element))
                    })
                    || crate::domains::expression_domain(context, bindings, file, headers[0])
                        .numeric_minimum()
                        .is_err()
                    || !bindings.declarations.iter().any(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == generator.range()
                            && integer(&calls.declarations[d.id.0].ty)
                    })
                    || matches!(
                        crate::callable_definitions::initialized_expression_safety(
                            context,
                            bindings,
                            calls,
                            instantiations,
                            domains,
                            (file, headers[0], &[], false),
                            None,
                        ),
                        DefinitionSafety::Unsupported(_)
                    )
                {
                    return false;
                }
                generators.push(generator);
            }
        }
    }
    for (selector, axis) in [row, column].into_iter().zip(indices) {
        if selector.kind() != NodeKind::Expression
            || typed(selector).is_none_or(|t| !integer(t))
            || crate::domains::index_domain_interval(axis)
                .is_some_and(|(lower, upper)| lower > upper)
        {
            return false;
        }
        if let Some(id) = reference(selector) {
            let d = &bindings.declarations[id.0];
            if !(d.role == DeclarationRole::Value && d.top_level
                || d.role == DeclarationRole::Generator
                    && generators.iter().any(|g| g.range() == d.syntax_range))
            {
                return false;
            }
        }
        match crate::domains::invariant_expression_integer(context, bindings, file, selector) {
            Err(_) => return false,
            Ok(Some(n)) if crate::domains::index_domain_member(axis, n) == Some(false) => {
                return false;
            }
            _ => {}
        }
    }
    !matches!(
        crate::callable_definitions::initialized_expression_safety(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            (file, value, &[], false),
            None,
        ),
        DefinitionSafety::Unsupported(_)
    ) && !matches!(
        crate::callable_definitions::direct_expression_safety(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            (file, value),
            &generators,
        ),
        DefinitionSafety::Unsupported(_)
    )
}

// Checked count evaluation can be supported while its numeric value remains
// unproved. Decline this exact range without claiming its minimum/nonemptiness.
fn inspected_count_bound(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
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
    {
        return false;
    }
    if !definitions.definitions.iter().any(|definition| {
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
    }) {
        return false;
    }
    if call.kind() == NodeKind::CallExpression
        && crate::definitions::resolved_call(context, calls, declaration.file, call).is_some_and(
            |selected| {
                ["length", "card"].iter().any(|name| {
                    crate::definitions::core_callable(context, bindings, selected, name)
                })
            },
        )
    {
        return true;
    }
    call.kind() == NodeKind::BinaryExpression
        && inspected_count_expression(context, bindings, calls, declaration.file, call)
            == Some(true)
        && !matches!(
            crate::callable_definitions::initialized_expression_safety(
                context,
                bindings,
                calls,
                instantiations,
                domains,
                (declaration.file, value, &[], false),
                None,
            ),
            DefinitionSafety::Unsupported(_)
        )
}

// Recognize parameter count addition, retaining no numeric value. Some(true)
// means a checked count occurs; None declines the shape or a closed error.
fn inspected_count_expression(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: crate::FileId,
    node: &SyntaxNode,
) -> Option<bool> {
    let integer = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    let typed = |node: &SyntaxNode| {
        let range = context.files[file].location(node.range()).range;
        calls
            .expressions
            .iter()
            .find(|expression| expression.file == file && expression.location.range == range)
            .map(|expression| &expression.ty)
    };
    if typed(node).is_none_or(|ty| !integer(ty))
        || !crate::definitions::annotations_safe(context, file, node)
    {
        return None;
    }
    let children: Vec<_> = node.child_nodes().collect();
    match node.kind() {
        NodeKind::ParenthesizedExpression if children.len() == 1 => {
            inspected_count_expression(context, bindings, calls, file, children[0])
        }
        NodeKind::Expression if children.is_empty() => {
            // Inspect each ordinary leaf independently. A symbolic sum cannot
            // hide overflow in the written initializer of another parameter.
            let _ = crate::domains::expression_integer(context, bindings, file, node).ok()?;
            Some(false)
        }
        NodeKind::BinaryExpression
            if children.len() == 2
                && crate::callables::core_operation(context, bindings, calls, file, node, "+")
                    == Ok(true)
                && crate::callables::operation_fact(context, calls, file, node).is_some_and(|call|
                    matches!(&call.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters.iter().all(integer)
                            && typed(node) == Some(return_type))) =>
        {
            let left = inspected_count_expression(context, bindings, calls, file, children[0])?;
            let right = inspected_count_expression(context, bindings, calls, file, children[1])?;
            if !left && !right {
                let _ = crate::domains::expression_integer(context, bindings, file, node).ok()?;
            }
            Some(left || right)
        }
        NodeKind::CallExpression
            if children.len() == 1 && children[0].kind() != NodeKind::NamedArgument
                && crate::callables::core_operation(context, bindings, calls, file, node, "card")
                    == Ok(true)
                && crate::callables::operation_fact(context, calls, file, node).is_some_and(|call|
                    matches!(&call.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && typed(node) == Some(return_type)
                            && typed(children[0]) == Some(&parameters[0])
                            && parameters[0].known() && !crate::value_safety::optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Parameter
                            && matches!(&parameters[0].kind, TypeKind::Set(element) if integer(element)))) =>
        {
            Some(true)
        }
        NodeKind::GeneratorCallExpression
            if children.len() == 2 && children[0].kind() == NodeKind::GeneratorList
                && children[1].kind() == NodeKind::ParenthesizedExpression
                && children[1].child_nodes().count() == 1
                && children[1].child_nodes().next().is_some_and(|body| body.kind() == NodeKind::CallExpression)
                && crate::callables::core_operation(context, bindings, calls, file, node, "sum")
                    == Ok(true)
                && crate::callables::operation_fact(context, calls, file, node).is_some_and(|call|
                    matches!(&call.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && typed(node) == Some(return_type)
                            && parameters[0].known() && !crate::value_safety::optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Parameter
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && integer(&indices[0]) && integer(element)))) =>
        {
            inspected_count_expression(context, bindings, calls, file, children[1])
        }
        _ => None,
    }
}

// Inspect these initialized symbolic axes without evaluating their minimum
// or exporting stronger domain/definition facts.
fn inspected_initialized_axis_source(
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
        if value.kind() == NodeKind::SetComprehension {
            return initialized_native_integer_set_source_safe(
                context,
                bindings,
                calls,
                instantiations,
                domains,
                file,
                value,
            );
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
        let closed_divisor =
            crate::domains::expression_integer(context, bindings, file, operands[1])
                .is_ok_and(|value| value.is_some_and(|value| value != 0));
        for (row, length) in operands.iter().enumerate() {
            if row == 1 && closed_divisor {
                let range = source.location(length.range()).range;
                if calls
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .is_none_or(|expression| !integer(&expression.ty))
                {
                    return false;
                }
                continue;
            }
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
        if closed_divisor {
            return initialized_native_integer_set_source_safe(
                context,
                bindings,
                calls,
                instantiations,
                domains,
                file,
                value,
            );
        }
        let safety = crate::callable_definitions::initialized_expression_safety(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            (file, upper, &[], false),
            None,
        );
        return matches!(
            safety,
            DefinitionSafety::Supported | DefinitionSafety::Unknown(_)
        );
    }
    false
}

// Existing source inspection owns types, lexical scopes, errors and cycles.
// Check selected native declarations throughout the same source graph as well.
fn initialized_native_integer_set_source_safe<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    file: crate::FileId,
    node: &'a SyntaxNode,
) -> bool {
    if matches!(
        crate::callable_definitions::initialized_integer_set_source_safety(
            context,
            bindings,
            (calls, calls),
            instantiations,
            domains,
            (file, node, &[]),
        ),
        DefinitionSafety::Unsupported(_)
    ) {
        return false;
    }
    let mut nodes = vec![(file, node)];
    let mut inspected = Vec::new();
    while let Some((file, node)) = nodes.pop() {
        if matches!(
            node.kind(),
            NodeKind::CallExpression
                | NodeKind::GeneratorCallExpression
                | NodeKind::UnaryExpression
                | NodeKind::BinaryExpression
                | NodeKind::RangeExpression
        ) && !crate::callable_definitions::native_operation_source_safe(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            (file, node),
        ) {
            return false;
        }
        if node.kind() == NodeKind::Expression
            && let Some(id) = crate::definitions::resolved_reference(context, bindings, file, node)
        {
            let declaration = &bindings.declarations[id.0];
            // Type references are not followed by the initialized value reader.
            if declaration.role == DeclarationRole::TypeAlias {
                return false;
            }
            if declaration.top_level
                && matches!(
                    declaration.role,
                    DeclarationRole::Value | DeclarationRole::Enum
                )
                && !inspected.contains(&id)
            {
                let Some(written) = crate::callables::find_node(
                    context.files[declaration.file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                ) else {
                    return false;
                };
                inspected.push(id);
                nodes.push((declaration.file, written));
            }
        }
        nodes.extend(node.child_nodes().map(|child| (file, child)));
    }
    true
}
