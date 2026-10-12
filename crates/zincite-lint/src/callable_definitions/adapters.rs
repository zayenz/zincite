//! Existing consumer entry points; each keeps its original lookup and body-view contract.
use super::*;

pub(crate) fn direct_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    generators: &[&'a SyntaxNode],
) -> DefinitionSafety {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    BodyInterpreter::without_bodies(&inspector).direct_safety(source.0, source.1, calls, generators)
}
/// Retain closed source errors in arguments and already selected dependencies.
/// This scan supplies no value, safety or body-admission certificate.
pub(crate) fn opaque_source_error<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    file: FileId,
    node: &'a SyntaxNode,
) -> Option<String> {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    opaque_dependency_error(&inspector, file, node, calls, &[], &mut Vec::new())
}
pub(super) fn opaque_dependency_error<'a>(
    inspector: &SourceInspector<'a>,
    file: FileId,
    source: &'a SyntaxNode,
    view: &crate::callables::CallableView<'_>,
    actuals: &[InvocationActual<'a, '_>],
    active: &mut Vec<DeclarationId>,
) -> Option<String> {
    if let Some(reason) =
        inspector.closed_integer_source_error_with_branches(file, source, false, true, true)
    {
        return Some(reason);
    }
    let interpreter = BodyInterpreter::without_bodies(inspector);
    let invocation = Invocation {
        actuals: actuals.to_vec(),
        reachable: None,
    };
    let mut strict = vec![(file, source)];
    let mut sources = Vec::new();
    let mut nodes = vec![(file, source)];
    while let Some((file, value)) = strict.pop() {
        if (crate::value_safety::ClosedSourceErrors {
            context: inspector.context,
            bindings: inspector.bindings,
            calls: inspector.view(file, value, view),
        })
        .false_integer_assertion(file, value)
        {
            return Some("false assertion condition aborts evaluation".into());
        }
        // Reflection bounds still require a valid native signature and a
        // nonempty source, even when the enclosing value stays unknown.
        if value.kind() == NodeKind::CallExpression
            && (inspector.core(file, value, view, "lb_array")
                || inspector.core(file, value, view, "ub_array"))
            && value
                .child_nodes()
                .next()
                .and_then(|argument| inspector.reference(file, unwrap(argument)))
                .is_some_and(|id| {
                    inspector.bindings.declarations[id.0].top_level
                        && inspector.bindings.declarations[id.0].role == DeclarationRole::Value
                })
            && let Some(DefinitionSafety::Unsupported(reason)) =
                interpreter.integer_array_bound_safety(file, value, view, &[])
        {
            return Some(reason);
        }
        if value.kind() == NodeKind::CallExpression
            && inspector.core(file, value, view, "floor")
            && let Some(DefinitionSafety::Unsupported(reason)) =
                interpreter.floor_sum_source_safety(file, value, view, &[])
        {
            return Some(reason);
        }
        if value.kind() == NodeKind::CallExpression
            && inspector.core(file, value, view, "to_enum")
            && value.child_nodes().nth(1).is_some_and(|ordinal| {
                crate::domains::invariant_expression_integer(
                    inspector.context,
                    inspector.bindings,
                    file,
                    ordinal,
                )
                .ok()
                .flatten()
                .is_some()
            })
            && let Some(DefinitionSafety::Unsupported(reason)) =
                interpreter.parameter_enum_conversion_safety(file, value, view, &[])
        {
            return Some(reason);
        }
        if value.kind() == NodeKind::ArrayAccessExpression
            && inspector
                .expression_type(view, file, value)
                .is_some_and(|e| e.ty.known() && e.ty.kind != TypeKind::Bool)
            && let Some(subject) = value.child_nodes().next()
            && let (array_file, subject) =
                match interpreter.invocation_source(file, subject, &invocation) {
                    Ok(Some(actual)) => (actual.file, actual.node),
                    Ok(None) => (file, subject),
                    Err(reason) => return Some(reason),
                }
            && let Some(array) = inspector.reference(array_file, subject)
            && let Domain::Array { indices, .. } =
                crate::domains::bare_index_domain(&inspector.domains.declarations[array.0].domain)
        {
            let integer = inspector
                .expression_type(view, file, value)
                .is_some_and(|e| e.ty.kind == TypeKind::Int);
            for (axis, selector) in indices.iter().zip(value.child_nodes().skip(1)) {
                match crate::domains::invariant_expression_integer(
                    inspector.context,
                    inspector.bindings,
                    file,
                    selector,
                ) {
                    Ok(Some(value))
                        if crate::domains::index_domain_member(axis, value) == Some(false)
                            || matches!(crate::domains::bare_index_domain(axis), Domain::Range { lower, upper }
                            if crate::domains::invariant_integer(lower).ok().flatten().is_some_and(|lower| value < lower)
                                || crate::domains::invariant_integer(upper).ok().flatten().is_some_and(|upper| value > upper)) =>
                    {
                        return Some(
                            if integer {
                                "integer selection is outside its declared index set"
                            } else {
                                "selection is outside its declared index set"
                            }
                            .into(),
                        );
                    }
                    _ => {}
                }
            }
        }
        // Retain the existing set-extremum arithmetic error reader for these
        // concrete operands. Its successful result is discarded.
        if value.kind() == NodeKind::BinaryExpression
            && matches!(
                operator(inspector.context, file, value)
                    .and_then(crate::bindings::symbolic_operator),
                Some("div" | "mod")
            )
        {
            let mut operands: Vec<_> = value.child_nodes().collect();
            let mut set_extremum = false;
            while let Some(operand) = operands.pop() {
                let operand = unwrap(operand);
                operands.extend(operand.child_nodes());
                if operand.kind() == NodeKind::CallExpression
                    && (inspector.core(file, operand, view, "min") || inspector.core(file, operand, view, "max"))
                    && operand.child_nodes().next().is_some_and(|argument| inspector.expression_type(view, file, argument)
                        .is_some_and(|e| matches!(&e.ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)))
                {
                    set_extremum = true;
                }
            }
            if set_extremum
                && let DefinitionSafety::Unsupported(reason) = interpreter.integer_operator_safety(
                    file,
                    value,
                    view,
                    &[],
                    "integer arithmetic source is unsupported".into(),
                )
            {
                return Some(reason);
            }
        }
        if parameter_index_extremum(inspector.context, inspector.bindings, view, file, value)
            .is_some()
            && let Some(subject) = unwrap(value)
                .child_nodes()
                .next()
                .and_then(|set| unwrap(set).child_nodes().next())
            && interpreter.invocation_array_empty(
                file,
                subject,
                view,
                &invocation,
                &mut Vec::new(),
                true,
            ) == Ok(Some(true))
        {
            return Some("strict index extremum has a known empty actual array".into());
        }
        if let Some(call) = operation_fact(inspector.context, view, file, value)
            && matches!(call.name.as_str(), "min" | "max")
            && core_operation(
                inspector.context,
                inspector.bindings,
                view,
                file,
                value,
                &call.name,
            ) == Ok(true)
            && matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                if matches!(parameters.as_slice(), [parameter] if parameter.known() && !optional(parameter)
                    && parameter.instantiation == Instantiation::Parameter
                    && matches!(&parameter.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && element.kind == TypeKind::Int))
                    && return_type.kind == TypeKind::Int && !optional(return_type)
                    && return_type.instantiation == Instantiation::Parameter)
            && let Some(subject) = value.child_nodes().next()
            && interpreter.invocation_array_empty(
                file,
                subject,
                view,
                &invocation,
                &mut Vec::new(),
                true,
            ) == Ok(Some(true))
        {
            return Some("integer extrema source is empty".into());
        }
        if let Some(call) = operation_fact(inspector.context, view, file, value)
            && matches!(call.name.as_str(), "min" | "max")
            && inspector.core(file, value, view, &call.name)
            && let CallOutcome::Resolved {
                parameters,
                return_type,
                ..
            } = &call.outcome
            && return_type.known()
            && !optional(return_type)
            && return_type.instantiation == Instantiation::Parameter
            && return_type.kind == TypeKind::Int
        {
            if value.kind() == NodeKind::CallExpression
                && matches!(parameters.as_slice(), [parameter] if parameter.known() && !optional(parameter)
                    && parameter.instantiation == Instantiation::Parameter && matches!(&parameter.kind, TypeKind::Set(element) if element.kind == TypeKind::Int))
                && value
                    .child_nodes()
                    .next()
                    .is_some_and(|argument| inspector.parameter_set_source_empty(file, argument))
            {
                return Some("parameter set extremum is undefined for an empty set".into());
            }
            if value.kind() == NodeKind::GeneratorCallExpression
                && call.name == "max"
                && value.child_nodes().next().is_some_and(|list| {
                    list.child_nodes().any(|header| {
                        header.child_nodes().next().is_some_and(|source| {
                            inspector.parameter_set_source_empty(file, source)
                        }) || header.child_nodes().skip(1).any(|filter| {
                            filter.child_nodes().next().is_some_and(|condition| {
                                inspector.assertion_literal(file, condition)
                                    == Some(TokenKind::False)
                            })
                        })
                    })
                })
            {
                return Some("native max is undefined for an empty iteration".into());
            }
        }
        if value.kind() == NodeKind::Expression {
            match interpreter.invocation_source(file, value, &invocation) {
                Ok(Some(actual)) if !sources.contains(&actual.formal) => {
                    sources.push(actual.formal);
                    strict.push((actual.file, actual.node));
                    nodes.push((actual.file, actual.node));
                }
                Err(reason) => return Some(reason),
                _ => {}
            }
        }
        if value.kind() == NodeKind::Expression
            && let Some(id) = inspector.reference(file, value)
            && !sources.contains(&id)
        {
            let declaration = &inspector.bindings.declarations[id.0];
            let ty = &view.declarations[id.0].ty;
            if matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Local
            ) && ty.known()
                && (matches!(ty.kind, TypeKind::Set(_) | TypeKind::Array { .. })
                    || matches!(ty.kind, TypeKind::Int | TypeKind::Float)
                        && ty.instantiation == Instantiation::Parameter)
                && let Some(written) = find_node(
                    inspector.context.files[declaration.file].parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
            {
                sources.push(id);
                if let Some(reason) = inspector.closed_integer_source_error_with_branches(
                    declaration.file,
                    written,
                    false,
                    true,
                    true,
                ) {
                    return Some(reason);
                }
                strict.push((declaration.file, written));
                nodes.push((declaration.file, written));
            }
        }
        // A strict guard still evaluates, but an unproved branch is no witness
        // that its extremum executes. This check never admits the enclosing body.
        if value.kind() == NodeKind::ConditionalExpression {
            strict.extend(
                value
                    .child_nodes()
                    .filter(|branch| branch.kind() == NodeKind::ConditionalBranch)
                    .filter_map(|branch| branch.child_nodes().next())
                    .map(|node| (file, node)),
            );
        } else {
            strict.extend(value.child_nodes().map(|node| (file, node)));
        }
    }
    while let Some((file, node)) = nodes.pop() {
        nodes.extend(node.child_nodes().map(|node| (file, node)));
        if !matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) {
            continue;
        }
        let Some(call) = operation_fact(inspector.context, view, file, node) else {
            continue;
        };
        let CallOutcome::Resolved {
            declaration,
            parameters,
            ..
        } = &call.outcome
        else {
            continue;
        };
        if active.contains(declaration) {
            continue;
        }
        let owner = &inspector.bindings.declarations[declaration.0];
        let written = find_node(
            inspector.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        // Required user typing and already inspected proof bodies retain their
        // errors. Unrequested library implementations supply no admission check.
        let observed_body = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .last()
            .is_some_and(|body| {
                let range = inspector.context.files[owner.file]
                    .location(body.range())
                    .range;
                view.expressions
                    .iter()
                    .chain(inspector.calls.expressions.iter())
                    .any(|expression| {
                        expression.file == owner.file
                            && range.start <= expression.location.range.start
                            && expression.location.range.end <= range.end
                    })
            });
        if !inspector.context.files[owner.file].warnings_enabled() && !observed_body {
            continue;
        }
        if !written.child_nodes().any(|node| is_expression(node.kind())) {
            if let Some(reason) = inspector
                .closed_integer_source_error_with_branches(owner.file, written, false, true, true)
            {
                return Some(reason);
            }
            continue;
        }
        let mut actuals = invocation.actuals.clone();
        for position in 0..parameters.len() {
            if let Some(formal) = formal_parameter(
                inspector.context,
                inspector.bindings,
                *declaration,
                position,
            ) && let Some((argument_file, argument)) = call_argument(
                inspector.context,
                inspector.bindings,
                view,
                file,
                node,
                *declaration,
                position,
            ) {
                actuals.push(InvocationActual {
                    formal,
                    file: argument_file,
                    node: argument,
                    view: *view,
                    generators: Vec::new(),
                    collection: false,
                });
            }
        }
        let selected_view = requested_body(
            inspector.context,
            inspector.bindings,
            inspector.calls,
            BodyProperty::Definedness,
            *declaration,
            parameters,
        );
        if inspector.context.files[owner.file].warnings_enabled() {
            let range = inspector.context.files[owner.file]
                .location(written.range())
                .range;
            if let Some(reference) = inspector.bindings.references.iter().find(|reference| {
                reference.file == owner.file
                    && range.start <= reference.location.range.start
                    && reference.location.range.end <= range.end
                    && matches!(
                        reference.resolution,
                        BindingResolution::Unresolved | BindingResolution::Ambiguous(_)
                    )
            }) {
                return Some(format!(
                    "opaque body reference {} is unresolved or ambiguous",
                    reference.name
                ));
            }
            if selected_view.declarations.iter().any(|typed| {
                let local = &inspector.bindings.declarations[typed.declaration.0];
                local.file == owner.file
                    && written.range().start <= local.syntax_range.start
                    && local.syntax_range.end <= written.range().end
                    && matches!(
                        local.role,
                        DeclarationRole::Local
                            | DeclarationRole::Parameter
                            | DeclarationRole::Generator
                    )
                    && !typed.ty.known()
            }) {
                return Some("opaque body local or parameter typing is unavailable".into());
            }
            if let Some(body) = written
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .last()
            {
                let range = inspector.context.files[owner.file]
                    .location(body.range())
                    .range;
                let typed = selected_view.expressions.iter().find(|expression| {
                    expression.file == owner.file && expression.location.range == range
                });
                let CallOutcome::Resolved { return_type, .. } = &call.outcome else {
                    unreachable!()
                };
                if typed.is_none_or(|expression| {
                    !expression.ty.known() || !crate::types::coerces(&expression.ty, return_type)
                }) {
                    return Some("opaque body result typing is unavailable or inconsistent".into());
                }
            }
        }
        active.push(*declaration);
        let error = opaque_dependency_error(
            inspector,
            owner.file,
            written,
            &selected_view.view(),
            &actuals,
            active,
        );
        active.pop();
        if error.is_some() {
            return error;
        }
    }
    None
}
pub(crate) fn initialized_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode], bool),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> DefinitionSafety {
    let (file, node, generators, check_eager_integer) = source;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let safety =
        interpreter.initialized_source_safety(file, node, calls, generators, &mut Vec::new());
    // Only newly inspected traversal syntax needs the eager arithmetic veto.
    // Preserve legacy raw safety; lazy sources supply no new admission.
    if check_eager_integer
        && !matches!(safety, DefinitionSafety::Unsupported(_))
        && crate::expression_safety_view(context, bindings, calls, file, node)
            != DefinitionSafety::Supported
        && let Some(reason) = interpreter
            .source
            .closed_integer_source_error(file, node, true, false)
    {
        return DefinitionSafety::Unsupported(reason);
    }
    safety
}
// Check selected native source operations without certifying their values.
// The caller separately inspects initialized sources and owns result eligibility.
pub(crate) fn native_operation_source_safe<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
) -> bool {
    let (file, node) = source;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let Some(call) = interpreter.source.operation_fact(calls, file, node) else {
        return false;
    };
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &call.outcome
    else {
        return false;
    };
    bindings.declarations[declaration.0].role == DeclarationRole::Function
        && parameters.iter().all(|ty| ty.known() && !optional(ty))
        && return_type.known()
        && !optional(return_type)
        && interpreter.source.prefix_primitive(
            file,
            node,
            calls,
            &call.name,
            parameters,
            return_type,
        )
}
// A checked parameter conditional supplies an unknown bound, never its value.
// The caller owns the local declaration and any formal-source correspondence.
pub(crate) fn initialized_conditional_bound_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let (file, node, generators) = source;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    if node.kind() != NodeKind::ConditionalExpression
        || interpreter
            .source
            .expression_type(calls, file, node)
            .is_none_or(|e| {
                !e.ty.known()
                    || optional(&e.ty)
                    || e.ty.instantiation != Instantiation::Parameter
                    || e.ty.kind != TypeKind::Int
            })
    {
        return None;
    }
    for value in generators
        .iter()
        .flat_map(|generator| generator.child_nodes())
    {
        if let Some(reason) = opaque_source_error(
            context,
            bindings,
            calls,
            instantiations,
            domains,
            file,
            value,
        ) {
            return Some(DefinitionSafety::Unsupported(reason));
        }
    }
    // Inspect both branches and every closed fragment without choosing a branch
    // or waiving errors in a literal-inactive body.
    if let Some(reason) = interpreter
        .source
        .closed_integer_source_error(file, node, false, true)
    {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(DefinitionSafety::Unknown(
        "initialized conditional bound is unproved".into(),
    ))
}
// Retain closed arithmetic failures written in a consumer's declared type.
// This supplies no domain bound and does not evaluate symbolic extrema.
pub(crate) fn closed_integer_type_source_error<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
) -> Option<String> {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    interpreter
        .source
        .closed_integer_source_error_with_branches(source.0, source.1, false, false, true)
}
// Inspect only a decision prefix owned by an actual preceding integer binder.
// Unknown supplies neither prefix members nor a numeric bound.
pub(crate) fn decision_prefix_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    interpreter.decision_prefix_source_safety(source.0, source.1, source.2)
}
// Inspect only a scalar set selected from a present rank-one parameter array.
// This supplies no membership or extent; all initialized sources remain checked.
pub(crate) fn selected_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (
        &'a crate::callables::CallableView<'a>,
        &'a crate::callables::CallableView<'a>,
    ),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> Option<DefinitionSafety> {
    let (file, written, generators) = source;
    let (calls, view) = callables;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let node = unwrap(written);
    if node.kind() != NodeKind::ArrayAccessExpression {
        return None;
    }
    let children: Vec<_> = node.child_nodes().collect();
    let [subject, selector] = children.as_slice() else {
        return None;
    };
    let subject = unwrap(subject);
    if subject.kind() != NodeKind::Expression
        || !matches!(crate::domains::tokens(&context.files[file].parsed, subject).as_slice(),
            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
    {
        return None;
    }
    let id = interpreter.source.reference(file, subject)?;
    let declaration = &bindings.declarations[id.0];
    let declared = calls.declarations.get(id.0)?;
    let facts = interpreter.source.view(file, node, view);
    let ty = |node: &SyntaxNode| {
        interpreter
            .source
            .expression_type(facts, file, node)
            .map(|e| &e.ty)
    };
    let integer = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    let integer_set = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if integer(element))
    };
    if !declaration.top_level
        || declaration.role != DeclarationRole::Value
        || declaration.instantiation != Instantiation::Parameter
        || declared.declaration != id
        || ty(subject) != Some(&declared.ty)
        || ty(selector).is_none_or(|t| !integer(t))
        || ty(node).is_none_or(|t| !integer_set(t))
        || !declared.ty.known()
        || optional(&declared.ty)
        || declared.ty.instantiation != Instantiation::Parameter
        || !matches!(&declared.ty.kind, TypeKind::Array { indices, element }
            if indices.len() == 1 && integer(&indices[0]) && integer_set(element)
                && ty(node) == Some(element.as_ref()))
    {
        return None;
    }
    for value in generators
        .iter()
        .flat_map(|generator| generator.child_nodes())
    {
        if let Some(reason) = opaque_source_error(
            context,
            bindings,
            view,
            instantiations,
            domains,
            file,
            value,
        ) {
            return Some(DefinitionSafety::Unsupported(reason));
        }
    }
    Some(
        match opaque_source_error(
            context,
            bindings,
            view,
            instantiations,
            domains,
            file,
            written,
        ) {
            Some(reason) => DefinitionSafety::Unsupported(reason),
            None => {
                DefinitionSafety::Unknown("selected parameter set membership is unproved".into())
            }
        },
    )
}

// Inspect a typed integer-set source only after all semantic facts exist.
// Original calls own initialized declarations; the current view owns this source.
pub(crate) fn initialized_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (
        &'a crate::callables::CallableView<'a>,
        &'a crate::callables::CallableView<'a>,
    ),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> DefinitionSafety {
    let (file, node, generators) = source;
    let (calls, view) = callables;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let present_integer = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    if interpreter
        .source
        .expression_type(interpreter.source.view(file, node, view), file, node)
        .is_none_or(|expression| {
            let ty = &expression.ty;
            !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if present_integer(element))
        })
    {
        return DefinitionSafety::Unsupported(
            "source requires a present parameter integer set".into(),
        );
    }
    for value in generators
        .iter()
        .flat_map(|generator| generator.child_nodes())
    {
        if let Some(reason) = opaque_source_error(
            context,
            bindings,
            view,
            instantiations,
            domains,
            file,
            value,
        ) {
            return DefinitionSafety::Unsupported(reason);
        }
    }
    match opaque_source_error(context, bindings, view, instantiations, domains, file, node) {
        Some(reason) => DefinitionSafety::Unsupported(reason),
        None => DefinitionSafety::Unknown("parameter set source value is unproved".into()),
    }
}

// Retain the selected set extremum type, closed errors and known empty-source veto.
pub(crate) fn initialized_parameter_set_extremum_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (
        &'a crate::callables::CallableView<'a>,
        &'a crate::callables::CallableView<'a>,
    ),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> Option<DefinitionSafety> {
    let (file, node, generators) = source;
    let written = unwrap(node);
    if written.kind() != NodeKind::CallExpression || written.child_nodes().count() != 1 {
        return None;
    }
    let (calls, view) = callables;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let ty = &interpreter
        .source
        .expression_type(interpreter.source.view(file, written, view), file, written)?
        .ty;
    if !ty.known()
        || optional(ty)
        || ty.instantiation != Instantiation::Parameter
        || ty.kind != TypeKind::Int
    {
        return None;
    }
    let safety = interpreter.parameter_set_extremum_safety(file, node, view, generators)?;
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = interpreter
            .source
            .closed_integer_source_error(file, node, true, true)
    {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(safety)
}
// Recognize the written collection and exact selected standard length signature.
// A set-to-array matching view establishes no evaluation or cardinality fact.
pub(crate) fn length_argument<'a>(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &'a SyntaxNode,
) -> Option<&'a SyntaxNode> {
    let integer = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && t.kind == TypeKind::Int
    };
    let index = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(t.kind, TypeKind::Int | TypeKind::Enum(_))
    };
    let typed = |value: &SyntaxNode| {
        let range = context.files[file].location(value.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if node.kind() != NodeKind::CallExpression
        || core_operation(context, bindings, calls, file, node, "length") != Ok(true)
        || !crate::definitions::annotations_safe(context, file, node)
        || typed(node).is_none_or(|t| !integer(t))
    {
        return None;
    }
    let arguments: Vec<_> = node.child_nodes().collect();
    let [argument] = arguments.as_slice() else {
        return None;
    };
    if argument.kind() == NodeKind::NamedArgument {
        return None;
    }
    let actual = typed(argument)?;
    if !actual.known() || optional(actual) {
        return None;
    }
    let rank_three = actual.instantiation == Instantiation::Parameter
        && matches!(&actual.kind, TypeKind::Array { indices, element }
            if indices.len() == 3 && indices.iter().all(integer) && integer(element));
    let (element, set) = match &actual.kind {
        TypeKind::Set(element)
            if actual.instantiation == Instantiation::Parameter && index(element) =>
        {
            (element.as_ref(), true)
        }
        TypeKind::Array { indices, element }
            if rank_three
                || indices.len() == 1
                    && index(&indices[0])
                    && element.known()
                    && !optional(element)
                    && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)) =>
        {
            (element.as_ref(), false)
        }
        _ => return None,
    };
    if !operation_fact(context, calls, file, node).is_some_and(|call| {
        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
            if parameters.len() == 1 && integer(return_type)
                && parameters[0].known() && !optional(&parameters[0])
                && (!set || parameters[0].instantiation == Instantiation::Parameter)
                && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                    if indices.len() == 1 && index(&indices[0])
                        && (!set || indices[0].kind == TypeKind::Int)
                        && formal.known() && !optional(formal) && formal.kind == element.kind
                        && (!set || formal.instantiation == Instantiation::Parameter)))
    }) {
        return None;
    }
    if rank_three {
        let CallOutcome::Resolved {
            declaration,
            parameters,
            ..
        } = &operation_fact(context, calls, file, node)?.outcome
        else {
            return None;
        };
        let owner = &bindings.declarations[declaration.0];
        let source = &context.files[owner.file];
        let signature = calls
            .signatures
            .iter()
            .find(|s| s.declaration == *declaration)?;
        let written = find_node(source.parsed.tree(), &owner.syntax_range, owner.role)?;
        if owner.role != DeclarationRole::Function
            || source.kind != SourceKind::StandardLibrary
            || !source.implicit
            || !crate::callables::length_primitive(signature, written)
            || parameters[0]
                != TypeInst::par(TypeKind::Array {
                    indices: vec![TypeInst::par(TypeKind::Int)],
                    element: Box::new(TypeInst::par(TypeKind::Int)),
                })
        {
            return None;
        }
    }
    Some(argument)
}

pub(crate) struct DirectSafetyLookups<'a> {
    pub(crate) expressions: &'a HashMap<(FileId, usize, usize), usize>,
    pub(crate) calls: &'a HashMap<(FileId, usize), usize>,
    pub(crate) value_references: &'a [Vec<usize>],
}
pub(crate) fn indexed_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
    lookups: DirectSafetyLookups<'a>,
) -> DefinitionSafety {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: Some(lookups),
    };
    BodyInterpreter::without_bodies(&inspector).direct_safety(source.0, source.1, calls, source.2)
}
// Admit only this checked reshape to the raw-definition inspection fallback.
pub(crate) fn indexed_set_axis_integer_reshape<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    lookups: DirectSafetyLookups<'a>,
) -> bool {
    set_axis_integer_reshape_source(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        source,
        Some(lookups),
    )
    .is_some()
}
// A successful standard reshape retains every value of this exact source.
pub(crate) fn set_axis_integer_reshape_source<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::callables::CallableView<'a>,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> Option<&'a SyntaxNode> {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
    };
    inspector
        .set_axis_integer_reshape_arguments(source.0, source.1, calls, false)
        .map(|arguments| arguments[2])
}
// A supported symbolic bound, not a nonempty-array or value guarantee.
pub(crate) fn parameter_index_extremum(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::callables::CallableView<'_>,
    file: FileId,
    node: &SyntaxNode,
) -> Option<DeclarationId> {
    let node = unwrap(node);
    let integer = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && t.kind == TypeKind::Int
    };
    let set_type = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(&t.kind, TypeKind::Set(element) if integer(element))
    };
    let array_type = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && matches!(&t.kind, TypeKind::Array { indices, element }
            if matches!(indices.len(), 1 | 2) && indices.iter().all(integer)
                && element.known() && !optional(element))
    };
    let ty = |n: &SyntaxNode| {
        let range = context.files[file].location(n.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if node.kind() != NodeKind::CallExpression
        || !(core_operation(context, bindings, calls, file, node, "min") == Ok(true)
            || core_operation(context, bindings, calls, file, node, "max") == Ok(true))
        || !crate::definitions::annotations_safe(context, file, node)
        || ty(node).is_none_or(|t| !integer(t))
        || !operation_fact(context, calls, file, node).is_some_and(|c| {
            matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                if parameters.len() == 1 && set_type(&parameters[0]))
        })
    {
        return None;
    }
    let args: Vec<_> = node.child_nodes().collect();
    let [set] = args.as_slice() else {
        return None;
    };
    let set = unwrap(set);
    let rank = if core_operation(context, bindings, calls, file, set, "index_set") == Ok(true) {
        1
    } else if core_operation(context, bindings, calls, file, set, "index_set_1of2") == Ok(true) {
        2
    } else {
        return None;
    };
    if set.kind() != NodeKind::CallExpression
        || !crate::definitions::annotations_safe(context, file, set)
        || ty(set).is_none_or(|t| !set_type(t))
        || !operation_fact(context, calls, file, set).is_some_and(|c| {
            matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                if parameters.len() == 1 && array_type(&parameters[0])
                    && matches!(&parameters[0].kind, TypeKind::Array { indices, .. } if indices.len() == rank))
        })
    {
        return None;
    }
    let subjects: Vec<_> = set.child_nodes().collect();
    let [subject] = subjects.as_slice() else {
        return None;
    };
    let subject = unwrap(subject);
    if subject.kind() != NodeKind::Expression
        || ty(subject).is_none_or(|t| {
            !array_type(t)
                || !matches!(&t.kind, TypeKind::Array { indices, .. } if indices.len() == rank)
        })
        || (rank == 2
            && !matches!(crate::domains::tokens(&context.files[file].parsed, subject).as_slice(),
            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
    {
        return None;
    }
    let id = crate::definitions::resolved_reference(context, bindings, file, subject)?;
    let d = &bindings.declarations[id.0];
    if d.file != file || d.role != DeclarationRole::Parameter {
        return None;
    }
    let formal = find_node(context.files[file].parsed.tree(), &d.syntax_range, d.role)?;
    if !crate::definitions::annotations_safe(context, file, formal)
        || formal.child_nodes().any(|n| is_expression(n.kind()))
    {
        return None;
    }
    let array = formal.child_nodes().next()?;
    let types: Vec<_> = array.child_nodes().collect();
    if array.kind() != NodeKind::ArrayType
        || types.len() != rank + 1
        || types.iter().any(|n| n.kind() != NodeKind::ScalarType)
        || types.iter().any(|n| n.child_nodes().next().is_some())
        || types[..rank].iter().any(|n| {
            !crate::domains::tokens(&context.files[file].parsed, n)
                .iter()
                .any(|t| t.kind == TokenKind::Int)
        })
    {
        return None;
    }
    Some(id)
}
