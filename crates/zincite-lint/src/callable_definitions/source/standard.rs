//! Selected standard bodies retain written shape, metadata, owning scopes and primitives.
use super::*;
mod cardinality;
mod optional;
mod regular;
mod sliding_sum;
mod union;

impl<'a> SourceInspector<'a> {
    // The array source family uses only these selected standard primitives.
    // Their full written trees must contain no implementation/default expression.
    pub(in crate::callable_definitions) fn parameter_array_primitive(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
        name: &str,
        parameters: &[TypeInst],
        result: &TypeInst,
    ) -> Result<(), String> {
        let facts = self.view(file, node, view);
        let fact = self
            .operation_fact(facts, file, node)
            .ok_or("parameter array primitive selection is unavailable")?;
        let CallOutcome::Resolved {
            declaration,
            parameters: selected,
            return_type,
        } = &fact.outcome
        else {
            return Err("parameter array primitive selection is unsupported".into());
        };
        if !self.core(file, node, facts, name)
            || fact.generator_argument.is_some()
            || selected.as_slice() != parameters
            || return_type != result
            || self
                .expression_type(facts, file, node)
                .is_none_or(|value| value.ty != *result)
        {
            return Err("parameter array primitive identity or signature is unsupported".into());
        }
        let owner = &self.bindings.declarations[declaration.0];
        if owner.role != DeclarationRole::Function {
            return Err("parameter array primitive declaration is unsupported".into());
        }
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("parameter array primitive source is unavailable")?;
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Annotation {
                let value = node
                    .child_nodes()
                    .next()
                    .map(unwrap)
                    .ok_or("parameter array primitive annotation value is unavailable")?;
                let string = value.kind() == NodeKind::Expression
                    && crate::domains::tokens(&self.context.files[owner.file].parsed, value)
                        .first()
                        .is_some_and(|token| token.kind == TokenKind::StringLiteral);
                if !string
                    && ![
                        "mzn_internal_representation",
                        "promise_total",
                        "promise_commutative",
                    ]
                    .iter()
                    .any(|name| self.atomic_standard_metadata_safe(owner.file, node, name))
                {
                    return Err("parameter array primitive annotation is unsupported".into());
                }
                continue;
            }
            if is_expression(node.kind()) {
                return Err(
                    "parameter array primitive body, default or type expression is unsupported"
                        .into(),
                );
            }
            nodes.extend(node.child_nodes());
        }
        Ok(())
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    // Inspect the selected argument without treating an ignored wrapper as enforcement.
    pub(in crate::callable_definitions) fn inspect_ignored_constraint(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
        known: &[CallableOutput],
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
    ) -> Option<Result<Vec<DeclarationId>, String>> {
        let declaration = &self.source.bindings.declarations[id.0];
        let source = &self.source.context.files[declaration.file];
        if !matches!(
            declaration.name.as_str(),
            "redundant_constraint" | "symmetry_breaking_constraint"
        ) || declaration.role != DeclarationRole::Predicate
            || source.kind != SourceKind::StandardLibrary
        {
            return None;
        }
        if !source.implicit {
            let (file, argument) = call_argument(
                self.source.context,
                self.source.bindings,
                self.source.view(clause.file, clause.node, view),
                clause.file,
                clause.node,
                id,
                0,
            )?;
            let argument = unwrap(argument);
            let facts = self.source.view(file, argument, view);
            if file != clause.file
                || !clause.generators.is_empty()
                || clause.node.kind() != NodeKind::CallExpression
                || clause.node.child_nodes().count() != 1
                || argument.kind() != NodeKind::LetExpression
                || self.source.context.files[file].kind != SourceKind::User
                || !self.source.context.files[file]
                    .parsed
                    .tree()
                    .child_nodes()
                    .nth(clause.item)
                    .is_some_and(|item| item.kind() == NodeKind::Constraint)
                || !argument
                    .child_nodes()
                    .filter(|node| node.kind() == NodeKind::LetBlock)
                    .flat_map(|block| block.child_nodes())
                    .any(|local| {
                        local.kind() == NodeKind::Declaration
                            && !local.child_nodes().any(|node| is_expression(node.kind()))
                            && self.source.bindings.declarations.iter().any(|owner| {
                                let ty = &facts.declarations[owner.id.0].ty;
                                owner.file == file
                                    && owner.role == DeclarationRole::Local
                                    && owner.syntax_range == local.range()
                                    && ty.known()
                                    && !optional(ty)
                                    && ty.instantiation == Instantiation::Decision
                                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                                        if indices.len() == 2 && element.kind == TypeKind::Bool
                                            && element.instantiation == Instantiation::Decision
                                            && indices.iter().all(|axis| axis.kind == TypeKind::Int
                                                && axis.instantiation == Instantiation::Parameter))
                            })
                    })
            {
                // Other actuals retain generic inspection and its precise locations.
                return None;
            }
        }
        Some((|| {
            let written = find_node(
                source.parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("redundant constraint declaration unavailable")?;
            let boolean = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Bool;
            let formal = formal_parameter(self.source.context, self.source.bindings, id, 0)
                .ok_or("redundant constraint formal unavailable")?;
            let formals: Vec<_> = written
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
                .ok_or("redundant constraint parameters unavailable")?
                .child_nodes()
                .collect();
            let declared = &self.source.calls.declarations[formal.0].ty;
            let bodies: Vec<_> = written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            // Standard redefinitions may have the exact identity body.
            let identity = if !source.implicit
                && let [body] = bodies.as_slice()
            {
                let body = unwrap(body);
                formals.len() == 1
                    && formals[0].child_nodes().next().is_some_and(|ty| {
                        matches!(crate::domains::tokens(&source.parsed, ty).as_slice(),
                            [variable, boolean] if variable.kind == TokenKind::Var && boolean.kind == TokenKind::Bool)
                    })
                    && parameters == std::slice::from_ref(declared)
                    && body.kind() == NodeKind::Expression
                    && matches!(crate::domains::tokens(&source.parsed, body).as_slice(),
                        [name] if matches!(name.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    && self.source.reference(declaration.file, body) == Some(formal)
                    && self.source.expression_type(self.source.calls, declaration.file, body)
                        .is_some_and(|expression| expression.ty == *declared)
            } else {
                false
            };
            if clause.node.kind() != NodeKind::CallExpression
                || clause.node.child_nodes().count() != 1
                || !(source.implicit && bodies.is_empty() || identity)
                || formals.len() != 1
                || formals[0].child_nodes().any(|n| is_expression(n.kind()))
                || !boolean(declared) || declared.instantiation != Instantiation::Decision
                || parameters.len() != 1 || !boolean(&parameters[0])
                || self.source.operation_fact(self.source.view(clause.file, clause.node, view), clause.file, clause.node)
                    .is_none_or(|call| !matches!(&call.outcome,
                        CallOutcome::Resolved { declaration: selected, parameters, return_type }
                            if *selected == id && parameters.len() == 1 && parameters[0] == *declared
                                && boolean(return_type) && return_type.instantiation == Instantiation::Decision))
            {
                return Err("redundant constraint signature, body or defaults are unsupported".into());
            }
            for (file, root) in [(declaration.file, written), (clause.file, clause.node)] {
                let mut nodes = vec![root];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.source.context, file, node) {
                        return Err(
                            "redundant constraint declaration or call annotation is unsupported"
                                .into(),
                        );
                    }
                    nodes.extend(node.child_nodes());
                }
            }
            let (file, argument) = call_argument(
                self.source.context,
                self.source.bindings,
                self.source.view(clause.file, clause.node, view),
                clause.file,
                clause.node,
                id,
                0,
            )
            .ok_or("redundant constraint argument unavailable")?;
            let facts = self.source.view(file, argument, view);
            let ty = self
                .source
                .expression_type(facts, file, argument)
                .map(|e| &e.ty)
                .ok_or("redundant constraint argument type unavailable")?;
            if !boolean(ty) || !crate::types::coerces(ty, declared) {
                return Err(
                    "redundant constraint argument type or optionality is unsupported".into(),
                );
            }
            let lexical = if file == clause.file
                && clause.node.range().start <= argument.range().start
                && argument.range().end <= clause.node.range().end
            {
                &clause.generators[..]
            } else {
                &[]
            };
            // Inspect this complete owning scope before visiting naked Local
            // references. An ignored wrapper grants no outputs or membership.
            let quantified = unwrap(argument);
            if lexical.is_empty()
                && quantified.kind() == NodeKind::LetExpression
                && self.source.context.files[file].kind == SourceKind::User
                && self.source.context.files[file]
                    .parsed
                    .tree()
                    .child_nodes()
                    .nth(clause.item)
                    .is_some_and(|item| item.kind() == NodeKind::Constraint)
            {
                let locals: Vec<_> = quantified
                    .child_nodes()
                    .filter(|node| node.kind() == NodeKind::LetBlock)
                    .flat_map(|block| block.child_nodes())
                    .filter(|local| {
                        local.kind() == NodeKind::Declaration
                            && !local.child_nodes().any(|node| is_expression(node.kind()))
                    })
                    .filter_map(|local| {
                        self.source
                            .bindings
                            .declarations
                            .iter()
                            .find(|declaration| {
                                declaration.file == file
                                    && declaration.role == DeclarationRole::Local
                                    && declaration.syntax_range == local.range()
                            })
                    })
                    .filter(|declaration| {
                        let ty = &facts.declarations[declaration.id.0].ty;
                        ty.known()
                            && !optional(ty)
                            && ty.instantiation == Instantiation::Decision
                            && matches!(&ty.kind, TypeKind::Array { indices, element }
                                if indices.len() == 2 && element.kind == TypeKind::Bool
                                    && element.instantiation == Instantiation::Decision
                                    && indices.iter().all(|axis| axis.kind == TypeKind::Int
                                        && axis.instantiation == Instantiation::Parameter))
                    })
                    .map(|declaration| declaration.id)
                    .collect();
                if !locals.is_empty() {
                    let mut nodes = vec![quantified];
                    while let Some(node) = nodes.pop() {
                        if node.kind() == NodeKind::Generator {
                            let mut scope = generator_source_scope(quantified, node, lexical)
                                .ok_or("ignored constraint generator scope is unavailable")?;
                            let preceding = scope.len();
                            scope.push(node);
                            if let Some(checked) = self.ignored_reverse_header_safety(
                                file,
                                node,
                                facts,
                                &scope[..preceding],
                            ) {
                                checked?;
                            } else {
                                self.relation_iterations(file, &scope, preceding, facts, false)?;
                            }
                        }
                        if matches!(
                            node.kind(),
                            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
                        ) && self.source.core(file, node, facts, "forall")
                        {
                            let (selected, _) = self
                                .source
                                .resolved(file, node, facts)
                                .ok_or("ignored constraint forall selection is unavailable")?;
                            let owner = &self.source.bindings.declarations[selected.0];
                            let written = find_node(
                                self.source.context.files[owner.file].parsed.tree(),
                                &owner.syntax_range,
                                owner.role,
                            )
                            .ok_or("ignored constraint forall declaration is unavailable")?;
                            if !self
                                .source
                                .prefix_primitive_source_safe(owner.file, written)
                            {
                                return Err("ignored constraint forall body, default, domain or annotation is unsupported".into());
                            }
                        }
                        nodes.extend(node.child_nodes());
                    }
                    let owning = Clause {
                        file,
                        item: clause.item,
                        node: quantified,
                        generators: lexical.to_vec(),
                        kind: ClauseKind::Local,
                    };
                    let mut unavailable = Vec::new();
                    let mut inspected = Vec::new();
                    self.interpret_forwarded(
                        (&owning, facts, known),
                        &mut Vec::new(),
                        &mut unavailable,
                        &mut inspected,
                        active,
                        None,
                    );
                    if let Some(unavailable) = unavailable.into_iter().next() {
                        return Err(unavailable.reason);
                    }
                    extend(&mut inspected, locals);
                    return Ok(inspected);
                }
            }
            if lexical.is_empty()
                && quantified.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, quantified, facts, "forall")
            {
                let parts: Vec<_> = quantified.child_nodes().collect();
                let nested_rows = parts.get(1).is_some_and(|body| {
                    let body = unwrap(body);
                    let Some(first) = body.child_nodes().find(|n| n.kind() != NodeKind::LetBlock)
                    else {
                        return false;
                    };
                    let first = unwrap(first);
                    let Some(second) = first.child_nodes().nth(1) else {
                        return false;
                    };
                    let second = unwrap(second);
                    first.kind() == NodeKind::GeneratorCallExpression
                        && second.kind() == NodeKind::GeneratorCallExpression
                        && second
                            .child_nodes()
                            .nth(1)
                            .is_some_and(|inner| unwrap(inner).kind() == NodeKind::LetExpression)
                });
                if let [headers, body] = parts.as_slice()
                    && headers.kind() == NodeKind::GeneratorList
                    && (headers.child_nodes().count() == 2
                        || nested_rows && headers.child_nodes().count() == 1)
                    && unwrap(body).kind() == NodeKind::LetExpression
                {
                    let typed = self
                        .source
                        .expression_type(facts, file, quantified)
                        .map(|e| &e.ty);
                    let parameter_int = |t: &TypeInst| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && t.instantiation == Instantiation::Parameter
                    };
                    let decision_bool =
                        |t: &TypeInst| boolean(t) && t.instantiation == Instantiation::Decision;
                    if self.source.operation_fact(facts, file, quantified).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && decision_bool(return_type)
                                && Some(return_type) == typed && parameters[0].known()
                                && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && parameter_int(&indices[0])
                                        && decision_bool(element)
                                        && self.source.expression_type(self.source.view(file, body, facts), file, body)
                                            .is_some_and(|e| boolean(&e.ty)
                                                && crate::types::coerces(&e.ty, element)))))
                    {
                        return Err("redundant constraint row forall signature is unsupported".into());
                    }
                    let generators: Vec<_> = headers.child_nodes().collect();
                    match self.parameter_row_let_safety(file, body, facts, &generators, &[]) {
                        Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                        Some(DefinitionSafety::Unknown(_)) => return Ok(Vec::new()),
                        _ => {}
                    }
                }
            }
            // Boolean call bodies use the existing instance interpreter. Evaluate
            // their non-Boolean actuals and initialized references independently.
            let mut nodes = vec![argument];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.source.context, file, node) {
                    return Err("redundant constraint argument annotation is unsupported".into());
                }
                if node.kind() == NodeKind::Expression
                    && self.source.reference(file, node).is_some_and(|id| {
                        self.source.bindings.declarations[id.0].role == DeclarationRole::Local
                    })
                {
                    return Err(
                        "redundant constraint local source inspection is unsupported".into(),
                    );
                }
                if is_expression(node.kind())
                    && (node.kind() == NodeKind::Expression
                        || self
                            .source
                            .expression_type(self.source.view(file, node, view), file, node)
                            .is_none_or(|e| !boolean(&e.ty)))
                {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, node, view, lexical, &mut Vec::new())
                    {
                        return Err(reason);
                    }
                    continue;
                }
                if node.kind() == NodeKind::CallExpression {
                    let (selected, concrete) = self
                        .source
                        .resolved(file, node, view)
                        .ok_or("redundant constraint nested selection is unsupported")?;
                    for (position, formal) in concrete.iter().enumerate() {
                        let (actual_file, actual) = call_argument(
                            self.source.context,
                            self.source.bindings,
                            self.source.view(file, node, view),
                            file,
                            node,
                            selected,
                            position,
                        )
                        .ok_or("redundant constraint nested actual or default unavailable")?;
                        if actual_file == file
                            && node.range().start <= actual.range().start
                            && actual.range().end <= node.range().end
                        {
                            continue;
                        }
                        // An empty set has no evaluated cells. Its selected concrete
                        // formal supplies its type, not an instance-value certificate.
                        let empty_set = unwrap(actual).kind() == NodeKind::SetLiteral
                            && unwrap(actual).child_nodes().count() == 0
                            && formal.known()
                            && !optional(formal)
                            && formal.instantiation == Instantiation::Parameter
                            && matches!(formal.kind, TypeKind::Set(_))
                            && crate::definitions::annotations_safe(
                                self.source.context,
                                actual_file,
                                actual,
                            );
                        if !empty_set
                            && let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    actual_file,
                                    actual,
                                    view,
                                    &[],
                                    &mut Vec::new(),
                                )
                        {
                            return Err(reason);
                        }
                    }
                }
                nodes.extend(node.child_nodes());
            }
            let mut clauses = Vec::new();
            self.clauses(file, clause.item, argument, view, lexical, &mut clauses);
            if clauses.is_empty() {
                return Err("redundant constraint argument clauses unavailable".into());
            }
            let mut unavailable = Vec::new();
            let mut inspected = Vec::new();
            for child in clauses {
                self.interpret_forwarded(
                    (&child, view, known),
                    &mut Vec::new(),
                    &mut unavailable,
                    &mut inspected,
                    active,
                    None,
                );
            }
            match unavailable.into_iter().next() {
                Some(unavailable) => Err(unavailable.reason),
                None => Ok(inspected),
            }
        })())
    }

    // Inspect the aggregate source without enforcing members or forwarding outputs.
    pub(in crate::callable_definitions) fn inspect_bodyless_boolean_aggregate(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Option<Result<(), String>> {
        let array = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                        && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter
                        && element.known() && !optional(element) && element.kind == TypeKind::Bool
                        && element.instantiation == ty.instantiation)
        };
        let name = if self.source.core(clause.file, clause.node, view, "exists") {
            "exists"
        } else if self.source.core(clause.file, clause.node, view, "forall") {
            "forall"
        } else {
            return None;
        };
        if clause.node.kind() != NodeKind::CallExpression
            || parameters.len() != 1
            || !array(&parameters[0])
        {
            return None;
        }
        let owner = &self.source.bindings.declarations[id.0];
        let written = find_node(
            self.source.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let signature = self
            .source
            .calls
            .signatures
            .iter()
            .find(|s| s.declaration == id)?;
        if owner.role != DeclarationRole::Function
            || written.child_nodes().any(|node| is_expression(node.kind()))
            || signature.parameters.len() != 1
            || signature.parameters[0].has_default
        {
            return None;
        }
        let facts = self.source.view(clause.file, clause.node, view);
        let CallOutcome::Resolved {
            parameters: selected,
            return_type,
            ..
        } = &self
            .source
            .operation_fact(facts, clause.file, clause.node)?
            .outcome
        else {
            return None;
        };
        if selected.as_slice() != parameters
            || !return_type.known()
            || optional(return_type)
            || return_type.kind != TypeKind::Bool
            || return_type.instantiation != parameters[0].instantiation
        {
            return None;
        }
        let (file, actual) = call_argument(
            self.source.context,
            self.source.bindings,
            facts,
            clause.file,
            clause.node,
            id,
            0,
        )?;
        let actual_type = &self
            .source
            .expression_type(self.source.view(file, actual, view), file, actual)?
            .ty;
        // Only forall has the multidimensional matching view. Inspect the raw
        // source and retain its nominal axes instead of relaxing type coercions.
        let multidimensional_forall = name == "forall"
            && actual_type.known()
            && !optional(actual_type)
            && (actual_type.instantiation == parameters[0].instantiation
                || actual_type.instantiation == Instantiation::Parameter
                    && parameters[0].instantiation == Instantiation::Decision)
            && matches!(&actual_type.kind, TypeKind::Array { indices, element }
                if indices.len() > 1
                    && indices.iter().all(|index| index.known() && !optional(index)
                        && index.instantiation == Instantiation::Parameter
                        && matches!(index.kind, TypeKind::Int | TypeKind::Enum(_)))
                    && element.known() && !optional(element) && element.kind == TypeKind::Bool
                    && element.instantiation == actual_type.instantiation);
        if actual_type != &parameters[0] && !multidimensional_forall {
            return None;
        }
        Some((|| {
            self.source.parameter_array_primitive(
                clause.file,
                clause.node,
                view,
                name,
                selected,
                return_type,
            )?;
            if !crate::definitions::annotations_safe(self.source.context, clause.file, clause.node)
            {
                return Err(if name == "exists" {
                    "existential call annotation is unsupported".into()
                } else {
                    "universal call annotation is unsupported".into()
                });
            }
            if let Some(reason) = self
                .source
                .closed_integer_source_error(file, actual, false, true)
            {
                return Err(reason);
            }
            let lexical = if file == clause.file
                && clause.node.range().start <= actual.range().start
                && actual.range().end <= clause.node.range().end
            {
                &clause.generators[..]
            } else {
                &[]
            };
            match self.initialized_source_safety(file, actual, view, lexical, &mut Vec::new()) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                // This reader also checks dependencies; symbolic uncertainty
                // establishes no value, extent, member or output guarantee.
                _ => Ok(()),
            }
        })())
    }
    pub(in crate::callable_definitions) fn inspect_bodyless_integer_maximum(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Option<Result<(), String>> {
        let declaration = &self.source.bindings.declarations[id.0];
        let source = &self.source.context.files[declaration.file];
        if declaration.name != "array_int_maximum"
            || declaration.role != DeclarationRole::Predicate
            || source.kind != SourceKind::StandardLibrary
            || !source.implicit
        {
            return None;
        }
        let written = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        if written.child_nodes().any(|node| is_expression(node.kind())) {
            return None;
        }
        let integer = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.kind == TypeKind::Int
                && matches!(
                    ty.instantiation,
                    Instantiation::Parameter | Instantiation::Decision
                )
        };
        let array = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].kind == TypeKind::Int && indices[0].instantiation == Instantiation::Parameter
                    && integer(element))
        };
        Some((|| {
            let formals: Vec<_> = written
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
                .ok_or("integer maximum primitive formals unavailable")?
                .child_nodes()
                .collect();
            let declared: Vec<_> = (0..2)
                .map(|position| {
                    formal_parameter(self.source.context, self.source.bindings, id, position)
                        .map(|formal| &self.source.calls.declarations[formal.0].ty)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or("integer maximum declared formals unavailable")?;
            if !integer(declared[0])
                || declared[0].instantiation != Instantiation::Decision
                || !array(declared[1])
                || declared[1].instantiation != Instantiation::Decision
                || !matches!(&declared[1].kind, TypeKind::Array { element, .. } if element.instantiation == Instantiation::Decision)
            {
                return Err("integer maximum declared signature is unsupported".into());
            }
            if parameters.len() != 2 || !integer(&parameters[0]) || !array(&parameters[1])
                || formals.len() != 2 || formals.iter().any(|formal| formal.child_nodes().any(|n| is_expression(n.kind())))
                || !self.source.callable_annotations_safe(declaration.file, written)
                || self.source.operation_fact(self.source.view(clause.file, clause.node, view), clause.file, clause.node)
                    .is_none_or(|fact| !matches!(&fact.outcome, CallOutcome::Resolved { declaration: selected, parameters, return_type }
                        if *selected == id && parameters.len() == 2 && integer(&parameters[0]) && array(&parameters[1])
                            && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Bool))
            { return Err("integer maximum primitive signature or defaults are unsupported".into()); }
            let mut written_nodes = vec![written];
            while let Some(node) = written_nodes.pop() {
                if !crate::definitions::annotations_safe(
                    self.source.context,
                    declaration.file,
                    node,
                ) {
                    return Err("integer maximum primitive formal annotation is unsupported".into());
                }
                written_nodes.extend(node.child_nodes());
            }
            let mut unsupported = None;
            for position in 0..2 {
                let checked = (|| {
                    let (file, argument) = call_argument(
                        self.source.context,
                        self.source.bindings,
                        self.source.view(clause.file, clause.node, view),
                        clause.file,
                        clause.node,
                        id,
                        position,
                    )
                    .ok_or("integer maximum primitive argument unavailable")?;
                    let range = self.source.context.files[file]
                        .location(argument.range())
                        .range;
                    if !self
                        .source
                        .view(file, argument, view)
                        .expressions
                        .iter()
                        .any(|e| {
                            e.file == file
                                && e.location.range == range
                                && if position == 0 {
                                    integer(&e.ty)
                                } else {
                                    array(&e.ty)
                                }
                        })
                    {
                        return Err(
                            "integer maximum primitive actual type or optionality is unsupported"
                                .into(),
                        );
                    }
                    let mut nodes = vec![argument];
                    while let Some(node) = nodes.pop() {
                        if !crate::definitions::annotations_safe(self.source.context, file, node) {
                            return Err(
                                "integer maximum primitive argument annotation is unsupported"
                                    .into(),
                            );
                        }
                        nodes.extend(node.child_nodes());
                    }
                    let lexical = if file == clause.file
                        && clause.node.range().start <= argument.range().start
                        && argument.range().end <= clause.node.range().end
                    {
                        &clause.generators[..]
                    } else {
                        &[]
                    };
                    if let Err(reason) = self.dependencies(file, argument, view, lexical)
                        && !matches!(
                            self.direct_safety(file, argument, view, lexical),
                            DefinitionSafety::Unknown(_)
                        )
                    {
                        return Err(reason);
                    }
                    Ok(())
                })();
                if let Err(reason) = checked {
                    unsupported = Some(reason);
                }
            }
            if let Some(reason) = unsupported {
                return Err(reason);
            }
            // The primitive is inspected, not inverted. Its source may be empty;
            // no result, nonempty fact or output guarantee follows from this check.
            Ok(())
        })())
    }

    pub(in crate::callable_definitions) fn reverse_parameter_array_body(
        &self,
        file: FileId,
        body: &'a SyntaxNode,
        formal: DeclarationId,
        view: &CallableFacts,
        array: &TypeInst,
    ) -> Result<(), String> {
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let ty = |node: &SyntaxNode| {
            self.source
                .expression_type(view, file, node)
                .map(|value| &value.ty)
        };
        let reference = |node: &SyntaxNode| {
            (node.kind() == NodeKind::Expression && node.child_nodes().next().is_none())
                .then(|| self.source.reference(file, node))
                .flatten()
        };
        let literal = |node: &SyntaxNode, expected| {
            node.kind() == NodeKind::Expression
                && node.child_nodes().next().is_none()
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                && crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    node,
                ) == Ok(Some(expected))
        };
        // Check all closed body fragments before an unsupported shape can hide
        // a selected local initializer failure behind the unknown condition.
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, body, false, true)
        {
            return Err(reason);
        }
        let mut nodes = vec![body];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return Err("reverse array body annotation is unsupported".into());
            }
            nodes.extend(node.child_nodes());
        }
        let branches: Vec<_> = body.child_nodes().collect();
        let [then, otherwise] = branches.as_slice() else {
            return Err("reverse array body requires its complete conditional".into());
        };
        let first: Vec<_> = then.child_nodes().collect();
        let last: Vec<_> = otherwise.child_nodes().collect();
        let ([guard, empty], [local]) = (first.as_slice(), last.as_slice()) else {
            return Err("reverse array conditional branches are unsupported".into());
        };
        if body.kind() != NodeKind::ConditionalExpression
            || ty(body) != Some(array)
            || then.kind() != NodeKind::ConditionalBranch
            || otherwise.kind() != NodeKind::ElseBranch
            || guard.kind() != NodeKind::BinaryExpression
            || ty(guard) != Some(&boolean)
            || empty.kind() != NodeKind::ArrayLiteral
            || empty.child_nodes().next().is_some()
            || ty(empty).is_none_or(|value| !crate::types::coerces(value, array))
            || local.kind() != NodeKind::LetExpression
            || ty(local) != Some(array)
        {
            return Err("reverse array conditional types or empty branch are unsupported".into());
        }
        let guard_parts: Vec<_> = guard.child_nodes().collect();
        let [length, zero] = guard_parts.as_slice() else {
            return Err("reverse array condition operands are unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            guard,
            view,
            "=",
            &[integer.clone(), integer.clone()],
            &boolean,
        )?;
        self.source.parameter_array_primitive(
            file,
            length,
            view,
            "length",
            std::slice::from_ref(array),
            &integer,
        )?;
        if !literal(zero, 0)
            || length.child_nodes().count() != 1
            || length.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array condition source identity is unsupported".into());
        }
        let let_parts: Vec<_> = local.child_nodes().collect();
        let [block, reshape] = let_parts.as_slice() else {
            return Err("reverse array local body is unsupported".into());
        };
        let locals: Vec<_> = block.child_nodes().collect();
        let [cells, upper] = locals.as_slice() else {
            return Err("reverse array requires its two initialized locals".into());
        };
        if block.kind() != NodeKind::LetBlock || reshape.kind() != NodeKind::CallExpression {
            return Err("reverse array local or reshape shape is unsupported".into());
        }
        let local_id = |written: &SyntaxNode| {
            self.source
                .bindings
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Local
                        && declaration.syntax_range == written.range()
                })
                .map(|declaration| declaration.id)
        };
        let cells_id = local_id(cells).ok_or("reverse array cell local identity is unavailable")?;
        let upper_id =
            local_id(upper).ok_or("reverse array upper local identity is unavailable")?;
        let mut initializers = Vec::new();
        for (written, id, expected) in [(*cells, cells_id, array), (*upper, upper_id, &integer)] {
            let values: Vec<_> = written
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .collect();
            if written.kind() != NodeKind::Declaration
                || values.len() != 1
                || view.declarations[id.0].ty != *expected
                || ty(values[0]) != Some(expected)
            {
                return Err("reverse array local type or initializer is unsupported".into());
            }
            self.type_dependencies(file, written, view, &[])?;
            initializers.push(values[0]);
        }
        let flatten = initializers[0];
        let plus = initializers[1];
        self.source.parameter_array_primitive(
            file,
            flatten,
            view,
            "array1d",
            std::slice::from_ref(array),
            array,
        )?;
        if flatten.child_nodes().count() != 1
            || flatten.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array flattened local source is unsupported".into());
        }
        let plus_parts: Vec<_> = plus.child_nodes().collect();
        let [size, one] = plus_parts.as_slice() else {
            return Err("reverse array upper initializer shape is unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            plus,
            view,
            "+",
            &[integer.clone(), integer.clone()],
            &integer,
        )?;
        self.source.parameter_array_primitive(
            file,
            size,
            view,
            "length",
            std::slice::from_ref(array),
            &integer,
        )?;
        if !literal(one, 1)
            || size.child_nodes().count() != 1
            || size.child_nodes().next().and_then(reference) != Some(formal)
        {
            return Err("reverse array upper source identity is unsupported".into());
        }
        let reshape_parts: Vec<_> = reshape.child_nodes().collect();
        let [axis, comprehension] = reshape_parts.as_slice() else {
            return Err("reverse array reshape arguments are unsupported".into());
        };
        self.source.parameter_array_primitive(
            file,
            reshape,
            view,
            "array1d",
            &[set.clone(), array.clone()],
            array,
        )?;
        self.source.parameter_array_primitive(
            file,
            axis,
            view,
            "index_set",
            std::slice::from_ref(array),
            &set,
        )?;
        if axis.child_nodes().count() != 1
            || axis.child_nodes().next().and_then(reference) != Some(formal)
            || comprehension.kind() != NodeKind::ArrayComprehension
            || ty(comprehension) != Some(array)
        {
            return Err("reverse array reshape source identity is unsupported".into());
        }
        let comp_parts: Vec<_> = comprehension.child_nodes().collect();
        let [selection, list] = comp_parts.as_slice() else {
            return Err("reverse array comprehension shape is unsupported".into());
        };
        let headers: Vec<_> = list.child_nodes().collect();
        let [header] = headers.as_slice() else {
            return Err("reverse array requires one comprehension header".into());
        };
        let sources: Vec<_> = header.child_nodes().collect();
        let [source] = sources.as_slice() else {
            return Err("reverse array requires its unfiltered source".into());
        };
        let binders: Vec<_> = self
            .source
            .bindings
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.file == file
                    && declaration.role == DeclarationRole::Generator
                    && declaration.syntax_range == header.range()
            })
            .collect();
        let [binder] = binders.as_slice() else {
            return Err("reverse array binder identity is unsupported".into());
        };
        if list.kind() != NodeKind::GeneratorList
            || header.kind() != NodeKind::Generator
            || crate::domains::generator_slots(&self.source.context.files[file].parsed, header) != 1
            || !header.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In)
            })
            || view.declarations[binder.id.0].ty != integer
        {
            return Err("reverse array binder scope or type is unsupported".into());
        }
        self.source.parameter_array_primitive(
            file,
            source,
            view,
            "index_set",
            std::slice::from_ref(array),
            &set,
        )?;
        if source.child_nodes().count() != 1
            || source.child_nodes().next().and_then(reference) != Some(cells_id)
        {
            return Err("reverse array generator source identity is unsupported".into());
        }
        let selection_parts: Vec<_> = selection.child_nodes().collect();
        let [subject, selector] = selection_parts.as_slice() else {
            return Err("reverse array cell selection shape is unsupported".into());
        };
        let selector_parts: Vec<_> = selector.child_nodes().collect();
        let [upper_value, index] = selector_parts.as_slice() else {
            return Err("reverse array selector shape is unsupported".into());
        };
        if selection.kind() != NodeKind::ArrayAccessExpression
            || ty(selection) != Some(&integer)
            || reference(subject) != Some(cells_id)
            || reference(upper_value) != Some(upper_id)
            || reference(index) != Some(binder.id)
        {
            return Err("reverse array selected local or binder identity is unsupported".into());
        }
        self.source.parameter_array_primitive(
            file,
            selector,
            view,
            "-",
            &[integer.clone(), integer.clone()],
            &integer,
        )?;
        let checked = [*guard, flatten, plus, *axis, *source];
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_children_safety(file, &checked, view, &[])
        {
            return Err(reason);
        }
        if let DefinitionSafety::Unsupported(reason) =
            self.integer_comprehension_safety(file, comprehension, view, &[])
        {
            return Err(reason);
        }
        Ok(())
    }
    // This owning ignored scope needs source inspection, never reverse membership.
    pub(in crate::callable_definitions) fn ignored_reverse_header_safety(
        &self,
        file: FileId,
        header: &'a SyntaxNode,
        view: &CallableFacts,
        preceding: &[&'a SyntaxNode],
    ) -> Option<Result<(), String>> {
        let source = header.child_nodes().next()?;
        if !self.source.core(file, unwrap(source), view, "reverse") {
            return None;
        }
        Some((|| {
            let integer = TypeInst::par(TypeKind::Int);
            let binders: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Generator
                        && declaration.syntax_range == header.range()
                })
                .collect();
            if header.kind() != NodeKind::Generator
                || !header.children().iter().any(|child| {
                    matches!(child,
                    SyntaxElement::Token(index)
                        if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In)
                })
                || binders.is_empty()
                || binders.len()
                    != crate::domains::generator_slots(&self.source.context.files[file].parsed, header)
                || binders
                    .iter()
                    .any(|binder| view.declarations[binder.id.0].ty != integer)
            {
                return Err("ignored reverse header binder or scope is unsupported".into());
            }
            match self.parameter_integer_array_value_safety(file, source, view, preceding, true) {
                Some(DefinitionSafety::Supported | DefinitionSafety::Unknown(_)) => {}
                Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                None => return Err("ignored reverse header source is unsupported".into()),
            }
            let mut scope = preceding.to_vec();
            scope.push(header);
            for filter in header
                .child_nodes()
                .filter(|node| node.kind() == NodeKind::WhereFilter)
            {
                let condition = filter
                    .child_nodes()
                    .next()
                    .ok_or("iteration filter unavailable")?;
                if self
                    .source
                    .expression_type(self.source.view(file, condition, view), file, condition)
                    .is_none_or(|value| value.ty != TypeInst::par(TypeKind::Bool))
                {
                    return Err("iteration filter is not a total parameter Boolean".into());
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, condition, view, &scope, &mut Vec::new())
                {
                    return Err(reason);
                }
                self.dependencies(file, condition, view, &scope)?;
            }
            Ok(())
        })())
    }
    pub(in crate::callable_definitions) fn integer_product_written_body(
        &self,
        product: DeclarationId,
        array: &TypeInst,
        result: &TypeInst,
    ) -> Result<(), String> {
        let error = || "integer product selected written body is unsupported".to_owned();
        // Both selected signatures are inspected, including written defaults,
        // types and metadata; instantiation supplies types, not body safety.
        let body = |id: DeclarationId| -> Result<_, String> {
            let owner = &self.source.bindings.declarations[id.0];
            let written = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or_else(error)?;
            if let Some(reason) = self
                .source
                .closed_integer_source_error(owner.file, written, false, true)
            {
                return Err(reason);
            }
            let values: Vec<_> = written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            let [body] = values.as_slice() else {
                return Err(error());
            };
            let lists: Vec<_> = written
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::ParameterList)
                .collect();
            if owner.role != DeclarationRole::Function
                || self.source.context.files[owner.file].kind != SourceKind::StandardLibrary
                || !self.source.context.files[owner.file].implicit
                || !matches!(lists.as_slice(), [list] if list.child_nodes().count() == 1)
                || !self.source.callable_annotations_safe(owner.file, written)
                || written
                    .child_nodes()
                    .filter(|n| !is_expression(n.kind()) && n.kind() != NodeKind::Annotation)
                    .any(|n| !self.source.prefix_primitive_source_safe(owner.file, n))
            {
                return Err(error());
            }
            let formal = formal_parameter(self.source.context, self.source.bindings, id, 0)
                .ok_or_else(error)?;
            let view = instantiated_body(
                self.source.context,
                self.source.bindings,
                self.source.calls,
                id,
                std::slice::from_ref(array),
            );
            if view.declarations[formal.0].ty != *array
                || self
                    .source
                    .expression_type(&view, owner.file, body)
                    .is_none_or(|value| value.ty != *result)
            {
                return Err(error());
            }
            self.type_dependencies(owner.file, written, &view, &[])?;
            let mut nodes = vec![*body];
            while let Some(node) = nodes.pop() {
                if node.kind() == NodeKind::Error
                    || !self.source.source_annotations_safe(owner.file, node)
                {
                    return Err(error());
                }
                if let Some(call) = self.source.operation_fact(&view, owner.file, node)
                    && let CallOutcome::Resolved { declaration, .. } = &call.outcome
                {
                    if *declaration == id || (*declaration == product && id != product) {
                        return Err("recursive integer product source is unsupported".into());
                    }
                    let leaf = &self.source.bindings.declarations[declaration.0];
                    let source = find_node(
                        self.source.context.files[leaf.file].parsed.tree(),
                        &leaf.syntax_range,
                        leaf.role,
                    )
                    .ok_or_else(error)?;
                    if let Some(reason) = self
                        .source
                        .closed_integer_source_error(leaf.file, source, false, true)
                    {
                        return Err(reason);
                    }
                }
                nodes.extend(node.child_nodes());
            }
            Ok((owner.file, *body, formal, view))
        };
        let (outer_file, forwarding, outer_formal, outer) = body(product)?;
        let forwarded: Vec<_> = forwarding.child_nodes().collect();
        let [flatten] = forwarded.as_slice() else {
            return Err(error());
        };
        let flatten = unwrap(flatten);
        let inputs: Vec<_> = flatten.child_nodes().collect();
        if forwarding.kind() != NodeKind::CallExpression
            || !self
                .source
                .core(outer_file, forwarding, &outer, "product_rec")
            || flatten.kind() != NodeKind::CallExpression
            || !matches!(inputs.as_slice(), [input] if unwrap(input).kind() == NodeKind::Expression
                && unwrap(input).child_nodes().next().is_none() && self.source.reference(outer_file, unwrap(input)) == Some(outer_formal))
            || !self.source.prefix_primitive(
                outer_file,
                flatten,
                &outer,
                "array1d",
                std::slice::from_ref(array),
                array,
            )
        {
            return Err(error());
        }
        let CallOutcome::Resolved {
            declaration: inner_id,
            parameters,
            return_type,
        } = &self
            .source
            .operation_fact(&outer, outer_file, forwarding)
            .ok_or_else(error)?
            .outcome
        else {
            return Err(error());
        };
        if parameters.as_slice() != std::slice::from_ref(array) || return_type != result {
            return Err(error());
        }
        let (file, conditional, formal, view) = body(*inner_id)?;
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool);
        let decision_boolean = boolean.clone().with_inst(Instantiation::Decision);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let booleans = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(decision_boolean.clone()),
        })
        .with_inst(Instantiation::Decision);
        let typed = |node: &SyntaxNode| {
            self.source
                .expression_type(&view, file, node)
                .map(|value| &value.ty)
        };
        let reference = |node: &SyntaxNode| {
            let node = unwrap(node);
            (node.kind() == NodeKind::Expression && node.child_nodes().next().is_none())
                .then(|| self.source.reference(file, node))
                .flatten()
        };
        let literal = |node: &SyntaxNode, expected| {
            let node = unwrap(node);
            typed(node) == Some(&integer)
                && matches!(crate::domains::tokens(&self.source.context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                && crate::domains::invariant_expression_integer(
                    self.source.context,
                    self.source.bindings,
                    file,
                    node,
                ) == Ok(Some(expected))
        };
        let primitive = |node: &SyntaxNode, name, parameters: &[TypeInst], output: &TypeInst| {
            if self
                .source
                .prefix_primitive(file, unwrap(node), &view, name, parameters, output)
            {
                Ok(())
            } else {
                Err(error())
            }
        };
        let unary = |node: &SyntaxNode, name, id, output: &TypeInst| -> Result<(), String> {
            let node = unwrap(node);
            if node.kind() != NodeKind::CallExpression
                || node.child_nodes().count() != 1
                || node.child_nodes().next().and_then(reference) != Some(id)
            {
                return Err(error());
            }
            primitive(node, name, std::slice::from_ref(array), output)
        };
        let branches: Vec<_> = conditional.child_nodes().collect();
        let [empty_branch, single_branch, otherwise] = branches.as_slice() else {
            return Err(error());
        };
        let empty: Vec<_> = empty_branch.child_nodes().collect();
        let single: Vec<_> = single_branch.child_nodes().collect();
        let last: Vec<_> = otherwise.child_nodes().collect();
        let ([zero_guard, one], [one_guard, singleton], [local]) =
            (empty.as_slice(), single.as_slice(), last.as_slice())
        else {
            return Err(error());
        };
        if conditional.kind() != NodeKind::ConditionalExpression
            || empty_branch.kind() != NodeKind::ConditionalBranch
            || single_branch.kind() != NodeKind::ConditionalBranch
            || otherwise.kind() != NodeKind::ElseBranch
            || !literal(one, 1)
            || !crate::types::coerces(&integer, result)
        {
            return Err(error());
        }
        for (guard, expected) in [(*zero_guard, 0), (*one_guard, 1)] {
            let guard = unwrap(guard);
            let parts: Vec<_> = guard.child_nodes().collect();
            let [length, count] = parts.as_slice() else {
                return Err(error());
            };
            if guard.kind() != NodeKind::BinaryExpression || !literal(count, expected) {
                return Err(error());
            }
            primitive(guard, "=", &[integer.clone(), integer.clone()], &boolean)?;
            unary(length, "length", formal, &integer)?;
        }
        let singleton = unwrap(singleton);
        let selected: Vec<_> = singleton.child_nodes().collect();
        let [subject, minimum] = selected.as_slice() else {
            return Err(error());
        };
        let minimum = unwrap(minimum);
        let bounds: Vec<_> = minimum.child_nodes().collect();
        let [axis] = bounds.as_slice() else {
            return Err(error());
        };
        if singleton.kind() != NodeKind::ArrayAccessExpression
            || typed(singleton) != Some(result)
            || reference(subject) != Some(formal)
            || !self.source.array_bound_index(file, formal, minimum, &view)
        {
            return Err(error());
        }
        primitive(minimum, "min", std::slice::from_ref(&set), &integer)?;
        unary(axis, "index_set", formal, &set)?;
        let local = unwrap(local);
        let let_parts: Vec<_> = local.child_nodes().collect();
        let [block, returned] = let_parts.as_slice() else {
            return Err(error());
        };
        let locals: Vec<_> = block.child_nodes().collect();
        let [cells, products, base, recurrence] = locals.as_slice() else {
            return Err(error());
        };
        if local.kind() != NodeKind::LetExpression
            || typed(local) != Some(result)
            || block.kind() != NodeKind::LetBlock
            || cells.kind() != NodeKind::Declaration
            || products.kind() != NodeKind::Declaration
            || base.kind() != NodeKind::Constraint
            || recurrence.kind() != NodeKind::Constraint
        {
            return Err(error());
        }
        let local_id = |node: &SyntaxNode| {
            self.source
                .bindings
                .declarations
                .iter()
                .find(|id| {
                    id.file == file
                        && id.role == DeclarationRole::Local
                        && id.syntax_range == node.range()
                })
                .map(|id| id.id)
        };
        let cells_id = local_id(cells).ok_or_else(error)?;
        let products_id = local_id(products).ok_or_else(error)?;
        let cell_values: Vec<_> = cells
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .collect();
        let [flatten] = cell_values.as_slice() else {
            return Err(error());
        };
        if cells_id == products_id
            || cells_id == formal
            || products_id == formal
            || view.declarations[cells_id.0].ty != *array
            || view.declarations[products_id.0].ty != *array
            || products.child_nodes().any(|n| is_expression(n.kind()))
            || cells
                .child_nodes()
                .filter(|n| !is_expression(n.kind()))
                .any(|n| !self.source.prefix_primitive_source_safe(file, n))
        {
            return Err(error());
        }
        self.type_dependencies(file, cells, &view, &[])?;
        unary(flatten, "array1d", formal, array)?;
        let types: Vec<_> = products.child_nodes().collect();
        let [array_type] = types.as_slice() else {
            return Err(error());
        };
        let components: Vec<_> = array_type.child_nodes().collect();
        let [axis_type, member_type] = components.as_slice() else {
            return Err(error());
        };
        let axes: Vec<_> = axis_type.child_nodes().collect();
        let [axis] = axes.as_slice() else {
            return Err(error());
        };
        if array_type.kind() != NodeKind::ArrayType
            || axis_type.kind() != NodeKind::DomainType
            || !self.source.prefix_primitive_source_safe(file, member_type)
        {
            return Err(error());
        }
        unary(axis, "index_set", cells_id, &set)?;
        self.type_dependencies(file, products, &view, &[])?;
        let base_parts: Vec<_> = base.child_nodes().collect();
        let recurrence_parts: Vec<_> = recurrence.child_nodes().collect();
        let ([base], [quantified]) = (base_parts.as_slice(), recurrence_parts.as_slice()) else {
            return Err(error());
        };
        let base = unwrap(base);
        let base_operands: Vec<_> = base.child_nodes().collect();
        let [first_product, first_cell] = base_operands.as_slice() else {
            return Err(error());
        };
        let access = |node: &'a SyntaxNode, id| -> Result<_, String> {
            let node = unwrap(node);
            let parts: Vec<_> = node.child_nodes().collect();
            let [subject, selector] = parts.as_slice() else {
                return Err(error());
            };
            if node.kind() != NodeKind::ArrayAccessExpression
                || typed(node) != Some(result)
                || reference(subject) != Some(id)
            {
                return Err(error());
            }
            Ok(*selector)
        };
        if !literal(access(first_product, products_id)?, 1)
            || !literal(access(first_cell, cells_id)?, 1)
            || base.kind() != NodeKind::BinaryExpression
        {
            return Err(error());
        }
        primitive(
            base,
            "=",
            &[result.clone(), result.clone()],
            &decision_boolean,
        )?;
        let quantified = unwrap(quantified);
        let quantified_parts: Vec<_> = quantified.child_nodes().collect();
        let [list, equality] = quantified_parts.as_slice() else {
            return Err(error());
        };
        let headers: Vec<_> = list.child_nodes().collect();
        let [header] = headers.as_slice() else {
            return Err(error());
        };
        let sources: Vec<_> = header.child_nodes().collect();
        let [source] = sources.as_slice() else {
            return Err(error());
        };
        let binders: Vec<_> = self
            .source
            .bindings
            .declarations
            .iter()
            .filter(|id| {
                id.file == file
                    && id.role == DeclarationRole::Generator
                    && id.syntax_range == header.range()
            })
            .collect();
        let [binder] = binders.as_slice() else {
            return Err(error());
        };
        if quantified.kind() != NodeKind::GeneratorCallExpression
            || list.kind() != NodeKind::GeneratorList
            || header.kind() != NodeKind::Generator
            || crate::domains::generator_slots(&self.source.context.files[file].parsed, header) != 1
            || !header.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                if self.source.context.files[file].parsed.tokens()[*index].kind == TokenKind::In)
            })
            || view.declarations[binder.id.0].ty != integer
        {
            return Err(error());
        }
        primitive(
            quantified,
            "forall",
            std::slice::from_ref(&booleans),
            &decision_boolean,
        )?;
        let source = unwrap(source);
        let ends: Vec<_> = source.child_nodes().collect();
        let [two, length] = ends.as_slice() else {
            return Err(error());
        };
        if source.kind() != NodeKind::RangeExpression || !literal(two, 2) {
            return Err(error());
        }
        primitive(source, "..", &[integer.clone(), integer.clone()], &set)?;
        unary(length, "length", products_id, &integer)?;
        let equality = unwrap(equality);
        let operands: Vec<_> = equality.child_nodes().collect();
        let [next, multiply] = operands.as_slice() else {
            return Err(error());
        };
        let multiply = unwrap(multiply);
        let factors: Vec<_> = multiply.child_nodes().collect();
        let [previous, cell] = factors.as_slice() else {
            return Err(error());
        };
        let predecessor = unwrap(access(previous, products_id)?);
        let predecessor_parts: Vec<_> = predecessor.child_nodes().collect();
        let [index, one] = predecessor_parts.as_slice() else {
            return Err(error());
        };
        if equality.kind() != NodeKind::BinaryExpression
            || multiply.kind() != NodeKind::BinaryExpression
            || reference(access(next, products_id)?) != Some(binder.id)
            || reference(access(cell, cells_id)?) != Some(binder.id)
            || predecessor.kind() != NodeKind::BinaryExpression
            || reference(index) != Some(binder.id)
            || !literal(one, 1)
        {
            return Err(error());
        }
        primitive(
            equality,
            "=",
            &[result.clone(), result.clone()],
            &decision_boolean,
        )?;
        primitive(multiply, "*", &[result.clone(), result.clone()], result)?;
        primitive(
            predecessor,
            "-",
            &[integer.clone(), integer.clone()],
            &integer,
        )?;
        unary(
            access(returned, products_id)?,
            "length",
            products_id,
            &integer,
        )?;
        // These exact normalization/guard/recurrence sources are checked only
        // for inspection. No y/xx definitions or traversal facts escape here.
        Ok(())
    }
}
