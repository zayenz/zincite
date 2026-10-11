//! Existing consumer entry points; each keeps its original lookup and body-view contract.
use super::*;

pub(crate) fn direct_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    BodyInterpreter::without_bodies(&inspector).direct_safety(source.0, source.1, calls, generators)
}
// Inspect only the selected standard regular/4 relation and its nested bodies.
// Discovery supplies concrete views, never outputs or model-wide certificates.
pub(crate) fn regular_four_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    if node.kind() != NodeKind::CallExpression {
        return None;
    }
    let fact = interpreter.source.operation_fact(calls, file, node)?;
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &fact.outcome
    else {
        return None;
    };
    let owner = &bindings.declarations[declaration.0];
    if owner.name != "regular"
        || owner.role != DeclarationRole::Predicate
        || context.files[owner.file].kind != SourceKind::StandardLibrary
        || !interpreter.source.regular_four_tuple(parameters)
        || *return_type != TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
        || interpreter
            .source
            .expression_type(calls, file, node)
            .is_none_or(|actual| actual.ty != *return_type)
    {
        return None;
    }
    let id = *declaration;
    let parameters = parameters.clone();
    let clause = Clause {
        file,
        item: fact.item,
        node,
        generators: generators.to_vec(),
        kind: ClauseKind::Call,
    };
    let instances = discovery::discover_instances(&inspector, std::slice::from_ref(&clause));
    let interpreter = BodyInterpreter::new(&inspector, &instances, &[]);
    Some(
        match interpreter.inspect_regular_four(&clause, calls, id, &parameters, None)? {
            Ok(()) => DefinitionSafety::Unknown(
                "regular4 relation values, partiality and membership are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        },
    )
}
// Inspect the selected bound-GCC relation and its complete count source family.
// Concrete body views retain identities but supply no count or output proof.
pub(crate) fn gcc_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let (file, node, generators) = source;
    if node.kind() != NodeKind::CallExpression {
        return None;
    }
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let fact = interpreter.source.operation_fact(calls, file, node)?;
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &fact.outcome
    else {
        return None;
    };
    let owner = &bindings.declarations[declaration.0];
    if owner.name != "global_cardinality"
        || owner.role != DeclarationRole::Predicate
        || context.files[owner.file].kind != SourceKind::StandardLibrary
        || context.files[owner.file].implicit
        || !interpreter.source.gcc_tuple(parameters)
        || *return_type != TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
        || interpreter
            .source
            .expression_type(calls, file, node)
            .is_none_or(|actual| actual.ty != *return_type)
    {
        return None;
    }
    Some(
        match (|| {
            if !crate::definitions::annotations_safe(context, file, node) {
                return Err("GCC actual annotation is unsupported".into());
            }
            let clause = Clause {
                file,
                item: fact.item,
                node,
                generators: generators.to_vec(),
                kind: ClauseKind::Call,
            };
            let instance = Instance {
                id: *declaration,
                parameters: parameters.clone(),
                ancestry: Vec::new(),
                view: instantiated_body(context, bindings, calls, *declaration, parameters),
                clauses: Vec::new(),
                recursive: false,
            };
            let mut invocation = Invocation {
                actuals: Vec::new(),
                reachable: Some(true),
            };
            interpreter.invocation_actuals(&clause, calls, &instance, &mut invocation)?;
            interpreter
                .source
                .gcc_source_body(*declaration, parameters, &mut Vec::new())
        })() {
            Ok(()) => DefinitionSafety::Unknown(
                "GCC counts, index agreement and partiality are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        },
    )
}
// Inspect the selected literal-regexp wrapper without proving the relation total.
pub(crate) fn literal_regular_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    if node.kind() != NodeKind::CallExpression {
        return None;
    }
    let fact = interpreter.source.operation_fact(calls, file, node)?;
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &fact.outcome
    else {
        return None;
    };
    let owner = &bindings.declarations[declaration.0];
    let boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
    if owner.name != "regular"
        || owner.role != DeclarationRole::Predicate
        || context.files[owner.file].kind != SourceKind::StandardLibrary
        || context.files[owner.file].implicit
        || !matches!(parameters.as_slice(), [array, pattern]
            if array.known() && !optional(array) && array.instantiation == Instantiation::Decision
                && matches!(&array.kind, TypeKind::Array { indices, element }
                    if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                        && element.instantiation == Instantiation::Decision
                        && matches!(element.kind, TypeKind::Enum(_)))
                && *pattern == TypeInst::par(TypeKind::String))
        || *return_type != boolean
        || interpreter
            .source
            .expression_type(calls, file, node)
            .is_none_or(|actual| actual.ty != boolean)
    {
        return None;
    }
    let id = *declaration;
    let parameters = parameters.clone();
    let clause = Clause {
        file,
        item: fact.item,
        node,
        generators: generators.to_vec(),
        kind: ClauseKind::Call,
    };
    let instances = discovery::discover_instances(&inspector, std::slice::from_ref(&clause));
    let interpreter = BodyInterpreter::new(&inspector, &instances, &[]);
    let checked: Result<(), String> = (|| {
        if !crate::definitions::annotations_safe(context, file, node) {
            return Err("literal regexp wrapper actual annotation is unsupported".into());
        }
        let written = find_node(
            context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("literal regexp wrapper source is unavailable")?;
        if let Some(reason) = interpreter
            .source
            .closed_integer_source_error(owner.file, written, false, true)
        {
            return Err(reason);
        }
        if !interpreter
            .source
            .callable_annotations_safe(owner.file, written)
        {
            return Err("literal regexp wrapper metadata is unsupported".into());
        }
        let mut signatures = calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == id);
        let signature = signatures
            .next()
            .ok_or("literal regexp wrapper signature is unavailable")?;
        if signatures.next().is_some()
            || signature.parameters.len() != 2
            || signature
                .parameters
                .iter()
                .any(|parameter| parameter.has_default)
            || signature.return_type != boolean
        {
            return Err("literal regexp wrapper signature or defaults are unsupported".into());
        }
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|child| is_expression(child.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err("literal regexp wrapper requires its complete owning body".into());
        };
        // Validated declaration hints are separate from formal/type metadata.
        let mut types: Vec<_> = written
            .child_nodes()
            .filter(|child| !is_expression(child.kind()) && child.kind() != NodeKind::Annotation)
            .collect();
        while let Some(ty) = types.pop() {
            if is_expression(ty.kind())
                || !crate::definitions::annotations_safe(context, owner.file, ty)
            {
                return Err("literal regexp wrapper written type or default is unsupported".into());
            }
            types.extend(ty.child_nodes());
        }
        let instance = interpreter
            .instances
            .iter()
            .find(|instance| {
                instance.id == id && instance.parameters == parameters && !instance.recursive
            })
            .ok_or("literal regexp wrapper nonrecursive body view is unavailable")?;
        let body = unwrap(body);
        let view = &instance.view;
        if body.kind() != NodeKind::CallExpression
            || interpreter
                .source
                .expression_type(view, owner.file, body)
                .is_none_or(|expression| expression.ty != boolean)
        {
            return Err("literal regexp wrapper body form or type is unsupported".into());
        }
        let native = interpreter
            .source
            .operation_fact(view, owner.file, body)
            .ok_or("literal regexp wrapper native selection is unavailable")?;
        let CallOutcome::Resolved {
            declaration: native_id,
            parameters: native_parameters,
            ..
        } = &native.outcome
        else {
            return Err("literal regexp wrapper native selection is unsupported".into());
        };
        let native_argument = |position| {
            call_argument(
                context, bindings, view, owner.file, body, *native_id, position,
            )
            .ok_or("literal regexp wrapper native actual is unavailable")
        };
        let (conversion_file, conversion) = native_argument(0)?;
        let input = interpreter
            .source
            .array_conversion_argument(conversion_file, conversion, view)
            .ok_or("literal regexp wrapper requires its checked enum conversion")?;
        let (pattern_file, pattern) = native_argument(1)?;
        for (position, source_file, actual) in
            [(0, conversion_file, input), (1, pattern_file, pattern)]
        {
            let formal = formal_parameter(context, bindings, id, position)
                .ok_or("literal regexp wrapper formal identity is unavailable")?;
            if source_file != owner.file
                || unwrap(actual).kind() != NodeKind::Expression
                || interpreter.source.reference(source_file, unwrap(actual)) != Some(formal)
            {
                return Err("literal regexp wrapper must forward its exact owning formals".into());
            }
        }
        let mut invocation = Invocation {
            actuals: Vec::new(),
            reachable: Some(true),
        };
        interpreter.invocation_actuals(&clause, calls, instance, &mut invocation)?;
        let native_clause = Clause {
            file: owner.file,
            item: owner.item,
            node: body,
            generators: Vec::new(),
            kind: ClauseKind::Call,
        };
        interpreter
            .inspect_literal_regular(
                &native_clause,
                view,
                *native_id,
                native_parameters,
                Some(&invocation),
            )
            .ok_or("literal regexp wrapper native declaration or tuple is unsupported")?
    })();
    Some(match checked {
        Ok(()) => DefinitionSafety::Unknown(
            "literal regexp relation values, partiality and membership are unproved".into(),
        ),
        Err(reason) => DefinitionSafety::Unsupported(reason),
    })
}
// Source inspection of the selected standard sliding recurrence, without
// assuming array extent, window membership, relation truth or output guarantees.
pub(crate) fn sliding_sum_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let (file, node, generators) = source;
    if node.kind() != NodeKind::CallExpression {
        return None;
    }
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let fact = interpreter.source.operation_fact(calls, file, node)?;
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &fact.outcome
    else {
        return None;
    };
    let owner = &bindings.declarations[declaration.0];
    if owner.name != "sliding_sum"
        || owner.role != DeclarationRole::Predicate
        || context.files[owner.file].kind != SourceKind::StandardLibrary
        || !interpreter.source.sliding_sum_tuple(parameters)
        || *return_type != TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
        || interpreter
            .source
            .expression_type(calls, file, node)
            .is_none_or(|e| e.ty != *return_type)
    {
        return None;
    }
    let id = *declaration;
    let parameters = parameters.clone();
    let clause = Clause {
        file,
        item: fact.item,
        node,
        generators: generators.to_vec(),
        kind: ClauseKind::Call,
    };
    let instances = discovery::discover_instances(&inspector, std::slice::from_ref(&clause));
    let interpreter = BodyInterpreter::new(&inspector, &instances, &[]);
    Some(
        match interpreter.inspect_sliding_sum(&clause, calls, id, &parameters) {
            Ok(()) => DefinitionSafety::Unknown(
                "sliding relation, extent and window membership are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        },
    )
}

// Inspect only an optional matrix's whole owning lexical initializer.
// This shares constructor checks with model lets and supplies no value proof.
pub(crate) fn optional_parameter_matrix_initializer_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let ty = &interpreter.source.expression_type(calls, file, node)?.ty;
    if node.kind() != NodeKind::CallExpression
        || !ty.known()
        || ty.optional
        || ty.instantiation != Instantiation::Parameter
        || !matches!(&ty.kind, TypeKind::Array { indices, element }
            if indices.len() == 2 && element.optional
                && element.instantiation == Instantiation::Parameter
                && element.kind == TypeKind::Int)
        || interpreter.source.operation_fact(calls, file, node)?.name != "array2d"
    {
        return None;
    }
    let mut pending = vec![context.files[file].parsed.tree()];
    let local = 'owner: loop {
        let parent = pending.pop()?;
        if parent.kind() == NodeKind::LetExpression {
            for block in parent
                .child_nodes()
                .filter(|part| part.kind() == NodeKind::LetBlock)
            {
                for local in block
                    .child_nodes()
                    .filter(|part| part.kind() == NodeKind::Declaration)
                {
                    if local
                        .child_nodes()
                        .find(|part| is_expression(part.kind()))
                        .is_some_and(|initializer| std::ptr::eq(unwrap(initializer), node))
                    {
                        break 'owner local;
                    }
                }
            }
        }
        pending.extend(parent.child_nodes().filter(|part| {
            part.range().start <= node.range().start && node.range().end <= part.range().end
        }));
    };
    let owner = bindings.declarations.iter().find(|owner| {
        owner.file == file
            && owner.role == DeclarationRole::Local
            && owner.syntax_range == local.range()
    })?;
    if !crate::definitions::annotations_safe(context, file, local) {
        return Some(DefinitionSafety::Unsupported(
            "optional parameter matrix owning Local annotation is unsupported".into(),
        ));
    }
    for (position, generator) in generators.iter().enumerate() {
        if generator.kind() != NodeKind::Generator
            || generator.range().end > local.range().start
            || position > 0 && generators[position - 1].range().end > generator.range().start
        {
            return Some(DefinitionSafety::Unsupported(
                "optional parameter matrix preceding header order is unsupported".into(),
            ));
        }
        for source in generator.child_nodes() {
            let filter = source.kind() == NodeKind::WhereFilter;
            let value = if filter {
                let Some(value) = source.child_nodes().next() else {
                    return Some(DefinitionSafety::Unsupported(
                        "optional parameter matrix preceding filter is unavailable".into(),
                    ));
                };
                value
            } else {
                source
            };
            let visible = position + usize::from(filter);
            if let Err(reason) = interpreter.optional_matrix_source_safety(
                (file, value, value.range().start),
                calls,
                &generators[..visible],
                &mut vec![owner.id],
            ) {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
    }
    interpreter.optional_parameter_matrix_safety(file, local, owner.id, calls, generators)
}
// Numeric inspection must check initialized references, not only dependencies.
pub(crate) fn initialized_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    let safety =
        interpreter.initialized_source_safety(file, node, calls, generators, &mut Vec::new());
    // Only newly inspected traversal syntax needs the eager arithmetic veto.
    // Preserve legacy raw safety; lazy sources supply no new admission.
    if check_eager_integer
        && !matches!(safety, DefinitionSafety::Unsupported(_))
        && crate::expression_safety(context, bindings, calls, file, node)
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
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
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
// The caller owns the local declaration and formal-source correspondence.
pub(crate) fn initialized_conditional_bound_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
) -> Option<DefinitionSafety> {
    let (file, node) = source;
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        selected_set_source: false,
        inactive_integer_body: None,
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
    let safety = interpreter.initialized_source_safety(file, node, calls, &[], &mut Vec::new());
    if let unsupported @ DefinitionSafety::Unsupported(_) = safety {
        return Some(unsupported);
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
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
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
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let interpreter = BodyInterpreter::without_bodies(&inspector);
    interpreter.decision_prefix_source_safety(source.0, source.1, source.2)
}
// Inspect only a scalar set selected from a present rank-one parameter array.
// This supplies no membership or extent; all initialized sources remain checked.
pub(crate) fn selected_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
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
        selected_set_source: true,
        inactive_integer_body: None,
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
    // Actual preceding headers and filters are independent evaluated sources.
    for generator in generators {
        for value in generator.child_nodes() {
            let value = if value.kind() == NodeKind::WhereFilter {
                let Some(value) = value.child_nodes().next() else {
                    return Some(DefinitionSafety::Unsupported(
                        "generator filter is unavailable".into(),
                    ));
                };
                value
            } else {
                value
            };
            if let unsupported @ DefinitionSafety::Unsupported(_) = interpreter
                .initialized_source_safety(file, value, view, generators, &mut Vec::new())
            {
                return Some(unsupported);
            }
            if let Some(reason) = interpreter
                .source
                .closed_integer_source_error(file, value, true, true)
            {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
    }
    let safety =
        interpreter.initialized_source_safety(file, written, view, generators, &mut Vec::new());
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = interpreter
            .source
            .closed_integer_source_error(file, written, true, true)
    {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(safety)
}
// Inspect a typed integer-set source only after all semantic facts exist.
// Original calls own initialized declarations; the current view owns this source.
pub(crate) fn initialized_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
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
        selected_set_source: false,
        inactive_integer_body: None,
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
    let safety =
        interpreter.initialized_source_safety(file, node, view, generators, &mut Vec::new());
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = interpreter
            .source
            .closed_integer_source_error(file, node, true, true)
    {
        return DefinitionSafety::Unsupported(reason);
    }
    safety
}
// Admit only the selected present parameter integer-set extremum, never its value.
pub(crate) fn initialized_parameter_set_extremum_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
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
        selected_set_source: false,
        inactive_integer_body: None,
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
    calls: &CallableFacts,
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
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    BodyInterpreter::without_bodies(&inspector).direct_safety(source.0, source.1, calls, source.2)
}
// Ephemeral Search prerequisite accounting, separate from callable facts.
pub(crate) fn indexed_model_value_fresh_locals<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    owner: DeclarationId,
    lookups: DirectSafetyLookups<'a>,
) -> Option<Result<Vec<DeclarationId>, String>> {
    let inspector = SourceInspector {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: Some(lookups),
        selected_set_source: false,
        inactive_integer_body: None,
    };
    BodyInterpreter::without_bodies(&inspector).model_value_fresh_locals(owner)
}
// Admit only this checked reshape to the raw-definition inspection fallback.
pub(crate) fn indexed_set_axis_integer_reshape<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
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
    calls: &'a CallableFacts,
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
        selected_set_source: false,
        inactive_integer_body: None,
    };
    inspector
        .set_axis_integer_reshape_arguments(source.0, source.1, calls)
        .map(|arguments| arguments[2])
}
// A supported symbolic bound, not a nonempty-array or value guarantee.
pub(crate) fn parameter_index_extremum(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
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
