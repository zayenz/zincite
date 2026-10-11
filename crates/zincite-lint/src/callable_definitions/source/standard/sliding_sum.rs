use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn sliding_sum_tuple(
        &self,
        parameters: &[TypeInst],
    ) -> bool {
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst {
            instantiation: Instantiation::Decision,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(integer.clone().with_inst(Instantiation::Decision)),
            },
        };
        parameters == [integer.clone(), integer.clone(), integer, array]
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn inspect_sliding_sum<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Result<(), String> {
        if !crate::definitions::annotations_safe(self.source.context, clause.file, clause.node) {
            return Err("sliding actual annotation is unsupported".into());
        }
        let instance = self
            .instances
            .iter()
            .find(|instance| {
                instance.id == id && instance.parameters == parameters && !instance.recursive
            })
            .ok_or("sliding exact nonrecursive body view is unavailable")?;
        let mut invocation = Invocation {
            actuals: Vec::new(),
            reachable: None,
        };
        // This retains raw Bool-array sources and their lexical headers; the
        // existing selected coercion supplies no values or extent certificate.
        self.invocation_actuals(clause, view, instance, &mut invocation)?;
        // Local constructions retain their defining headers, including when a
        // later consumer or concatenation carries additional lexical binders.
        for actual in &invocation.actuals {
            let mut nodes = vec![actual.node];
            while let Some(node) = nodes.pop() {
                nodes.extend(node.child_nodes());
                if node.kind() != NodeKind::Expression {
                    continue;
                }
                let Some(source) = self.source.reference(actual.file, node) else {
                    continue;
                };
                let owner = &self.source.bindings.declarations[source.0];
                if owner.role != DeclarationRole::Local {
                    continue;
                }
                if owner.file != actual.file || owner.syntax_range.end > node.range().start {
                    return Err(
                        "sliding actual Local source identity or order is unsupported".into(),
                    );
                }
                let local = find_node(
                    self.source.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("sliding actual Local declaration is unavailable")?;
                let headers =
                    local_source_scope(self.source.context.files[owner.file].parsed.tree(), local)
                        .ok_or("sliding actual Local owning scope is unavailable")?;
                for (position, header) in headers.iter().enumerate() {
                    if !self.source.source_annotations_safe(actual.file, header) {
                        return Err("sliding actual Local header annotation is unsupported".into());
                    }
                    for part in header.child_nodes() {
                        let filter = part.kind() == NodeKind::WhereFilter;
                        let value = if filter {
                            part.child_nodes()
                                .next()
                                .ok_or("sliding actual Local filter is unavailable")?
                        } else {
                            part
                        };
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(
                                actual.file,
                                value,
                                actual.view,
                                &headers[..position + usize::from(filter)],
                                &mut Vec::new(),
                            )
                        {
                            return Err(reason);
                        }
                    }
                }
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    actual.file,
                    node,
                    actual.view,
                    &headers,
                    &mut Vec::new(),
                ) {
                    return Err(reason);
                }
                if std::ptr::eq(node, unwrap(actual.node))
                    && let Some(initializer) =
                        local.child_nodes().find(|part| is_expression(part.kind()))
                    && self.invocation_array_empty(
                        actual.file,
                        initializer,
                        actual.view,
                        &invocation,
                        &mut Vec::new(),
                        false,
                    )? == Some(true)
                {
                    return Err("sliding extremum has a known empty actual array".into());
                }
            }
        }
        self.sliding_source_body(id, parameters, &invocation)
    }
    pub(in crate::callable_definitions) fn sliding_source_body<'b>(
        &'b self,
        id: DeclarationId,
        parameters: &[TypeInst],
        invocation: &Invocation<'a, 'b>,
    ) -> Result<(), String> {
        let owner = &self.source.bindings.declarations[id.0];
        let file = owner.file;
        let wrapper = owner.name == "sliding_sum";
        if owner.role != DeclarationRole::Predicate
            || self.source.context.files[file].kind != SourceKind::StandardLibrary
            || !(wrapper || owner.name == "fzn_sliding_sum")
            || !self.source.sliding_sum_tuple(parameters)
        {
            return Err("sliding owning declaration or tuple is unsupported".into());
        }
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("sliding written declaration is unavailable")?;
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, written, false, true)
        {
            return Err(reason);
        }
        let integer = TypeInst::par(TypeKind::Int);
        let decision = integer.clone().with_inst(Instantiation::Decision);
        let boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let array = |element: TypeInst| TypeInst {
            instantiation: element.instantiation,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(element),
            },
        };
        let mut signatures = self
            .source
            .calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == id);
        let signature = signatures
            .next()
            .ok_or("sliding signature is unavailable")?;
        if signatures.next().is_some()
            || signature.parameters.len() != 4
            || signature
                .parameters
                .iter()
                .any(|parameter| parameter.has_default)
            || signature.return_type != boolean
        {
            return Err("sliding signature or defaults are unsupported".into());
        }
        let instance = self
            .instances
            .iter()
            .find(|instance| {
                instance.id == id && instance.parameters == parameters && !instance.recursive
            })
            .ok_or("sliding exact nonrecursive body view is unavailable")?;
        let view = &instance.view;
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err("sliding requires its complete owning body".into());
        };
        if self
            .source
            .expression_type(view, file, body)
            .is_none_or(|e| e.ty != boolean)
        {
            return Err("sliding owning body type is unsupported".into());
        }
        let formals = (0..4)
            .map(|position| {
                let formal =
                    formal_parameter(self.source.context, self.source.bindings, id, position)
                        .ok_or("sliding formal identity is unavailable")?;
                if view.declarations[formal.0].ty != parameters[position] {
                    return Err("sliding concrete formal type is unsupported");
                }
                Ok(formal)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let members = |role| {
            let mut rows: Vec<_> = self
                .source
                .bindings
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.file == file
                        && declaration.role == role
                        && body.range().start <= declaration.syntax_range.start
                        && declaration.syntax_range.end <= body.range().end
                })
                .collect();
            rows.sort_by_key(|declaration| declaration.syntax_range.start);
            rows
        };
        let locals = members(DeclarationRole::Local);
        let binders = members(DeclarationRole::Generator);
        let local_types = if wrapper {
            Vec::new()
        } else {
            vec![integer.clone(), integer.clone(), array(decision.clone())]
        };
        if locals.len() != local_types.len()
            || binders.len() != if wrapper { 0 } else { 2 }
            || locals
                .iter()
                .zip(&local_types)
                .any(|(local, ty)| view.declarations[local.id.0].ty != *ty)
            || binders
                .iter()
                .any(|binder| view.declarations[binder.id.0].ty != integer)
        {
            return Err("sliding local or generator type is unsupported".into());
        }
        let name = |id: DeclarationId| self.source.bindings.declarations[id.0].name.as_str();
        let names: Vec<_> = formals
            .iter()
            .copied()
            .chain(locals.iter().map(|local| local.id))
            .map(name)
            .collect();
        if names
            .iter()
            .enumerate()
            .any(|(position, name)| names[..position].contains(name))
            || binders
                .iter()
                .any(|binder| names.contains(&binder.name.as_str()))
        {
            return Err("sliding owning names are shadowed".into());
        }
        let (low, up, width, xs) = (
            name(formals[0]),
            name(formals[1]),
            name(formals[2]),
            name(formals[3]),
        );
        let expected = if wrapper {
            let TypeKind::Array { indices, .. } = &signature.parameters[3].ty.kind else {
                return Err("sliding written array signature is unsupported".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("sliding written array rank is unsupported".into());
            };
            let TypeKind::Variable {
                name: axis,
                enum_only: true,
                any: false,
            } = &axis.kind
            else {
                return Err("sliding written generic axis is unsupported".into());
            };
            format!(
                "predicate sliding_sum ( int : {low} , int : {up} , int : {width} , array [ {axis} ] of var int : {xs} ) = fzn_sliding_sum ( {low} , {up} , {width} , index2int ( {xs} ) ) ;"
            )
        } else {
            let (lx, ux, prefix) = (&locals[0].name, &locals[1].name, &locals[2].name);
            let (i, j) = (&binders[0].name, &binders[1].name);
            format!(
                "predicate fzn_sliding_sum ( int : {low} , int : {up} , int : {width} , array [ int ] of var int : {xs} ) = let {{ int : {lx} = min ( index_set ( {xs} ) ) ; int : {ux} = max ( index_set ( {xs} ) ) ; array [ {lx} - 1 .. {ux} ] of var int : {prefix} ; }} in {prefix} [ {lx} - 1 ] = 0 /\\ forall ( {i} in {lx} .. {ux} ) ( {prefix} [ {i} ] = {xs} [ {i} ] + {prefix} [ {i} - 1 ] ) /\\ forall ( {j} in {lx} - 1 .. {ux} - {width} ) ( {prefix} [ {j} ] <= {prefix} [ {j} + {width} ] - {low} /\\ {prefix} [ {j} + {width} ] <= {prefix} [ {j} ] + {up} ) ;"
            )
        };
        if !self.source.regular_written_tokens(file, written, &expected) {
            return Err("sliding complete written header or recurrence is unsupported".into());
        }
        let mut scopes = Vec::new();
        let mut nested = None;
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::Error
                || !crate::definitions::annotations_safe(self.source.context, file, node)
            {
                return Err("sliding written syntax or annotation is unsupported".into());
            }
            if is_expression(node.kind())
                && self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|e| !e.ty.known() || optional(&e.ty))
            {
                return Err("sliding written expression type or optionality is unsupported".into());
            }
            if node.kind() == NodeKind::GeneratorCallExpression {
                let list = node
                    .child_nodes()
                    .find(|child| child.kind() == NodeKind::GeneratorList)
                    .ok_or("sliding generator list is unavailable")?;
                let headers: Vec<_> = list.child_nodes().collect();
                let [header] = headers.as_slice() else {
                    return Err("sliding generator header count is unsupported".into());
                };
                let binder = binders
                    .iter()
                    .find(|binder| binder.syntax_range == header.range())
                    .ok_or("sliding generator owning identity is unavailable")?;
                scopes.push((
                    binder.id,
                    self.source.context.files[file].location(node.range()).range,
                ));
            }
            if node.kind() == NodeKind::ArrayAccessExpression {
                let parts: Vec<_> = node.child_nodes().collect();
                let [subject, index] = parts.as_slice() else {
                    return Err("sliding selection rank is unsupported".into());
                };
                let source = self.source.reference(file, unwrap(subject));
                if wrapper
                    || !matches!(source, Some(source) if source == formals[3] || source == locals[2].id)
                    || self
                        .source
                        .expression_type(view, file, subject)
                        .is_none_or(|e| e.ty != array(decision.clone()))
                    || self
                        .source
                        .expression_type(view, file, index)
                        .is_none_or(|e| e.ty != integer)
                    || self
                        .source
                        .expression_type(view, file, node)
                        .is_none_or(|e| e.ty != decision)
                {
                    return Err("sliding selected array source or tuple is unsupported".into());
                }
            }
            if matches!(
                node.kind(),
                NodeKind::CallExpression
                    | NodeKind::GeneratorCallExpression
                    | NodeKind::BinaryExpression
                    | NodeKind::RangeExpression
            ) {
                let fact = self
                    .source
                    .operation_fact(view, file, node)
                    .ok_or("sliding selected operation is unavailable")?;
                let CallOutcome::Resolved {
                    declaration,
                    parameters: selected,
                    return_type,
                } = &fact.outcome
                else {
                    return Err("sliding selected operation is unsupported".into());
                };
                let actuals = if node.kind() == NodeKind::GeneratorCallExpression {
                    vec![
                        fact.generator_argument
                            .clone()
                            .ok_or("sliding generator collection is unavailable")?,
                    ]
                } else {
                    node.child_nodes()
                        .map(|child| {
                            self.source
                                .expression_type(view, file, child)
                                .map(|e| e.ty.clone())
                                .ok_or("sliding operand type is unavailable")
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                if self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|e| e.ty != *return_type)
                    || actuals.len() != selected.len()
                    || actuals
                        .iter()
                        .zip(selected)
                        .any(|(actual, formal)| !crate::types::coerces(actual, formal))
                {
                    return Err("sliding raw operation operands or result are unsupported".into());
                }
                if fact.name == "fzn_sliding_sum" {
                    if !wrapper
                        || !self.source.sliding_sum_tuple(selected)
                        || *return_type != boolean
                        || nested.is_some()
                    {
                        return Err("sliding nested FZN tuple is unsupported".into());
                    }
                    nested = Some((*declaration, selected.clone()));
                } else {
                    let unary = |ty: &TypeInst| selected.as_slice() == std::slice::from_ref(ty);
                    let pair = |ty: &TypeInst| selected.as_slice() == [ty.clone(), ty.clone()];
                    let tuple = match fact.name.as_str() {
                        "index2int" => {
                            wrapper
                                && unary(&array(decision.clone()))
                                && *return_type == array(decision.clone())
                                && self
                                    .source
                                    .array_conversion_argument(file, node, view)
                                    .is_some()
                        }
                        "index_set" => {
                            !wrapper && unary(&array(decision.clone())) && *return_type == set
                        }
                        "min" | "max" => !wrapper && unary(&set) && *return_type == integer,
                        "+" | "-" => {
                            !wrapper
                                && (pair(&integer) && *return_type == integer
                                    || pair(&decision) && *return_type == decision)
                        }
                        ".." => !wrapper && pair(&integer) && *return_type == set,
                        "=" => {
                            !wrapper
                                && (pair(&decision)
                                    || selected.as_slice() == [decision.clone(), integer.clone()])
                                && *return_type == boolean
                        }
                        "<=" => !wrapper && pair(&decision) && *return_type == boolean,
                        "/\\" => !wrapper && pair(&boolean) && *return_type == boolean,
                        "forall" => {
                            !wrapper && unary(&array(boolean.clone())) && *return_type == boolean
                        }
                        _ => false,
                    };
                    if !tuple
                        || !self.source.prefix_primitive(
                            file,
                            node,
                            view,
                            &fact.name,
                            selected,
                            return_type,
                        )
                    {
                        return Err(
                            "sliding operation identity, tuple or written primitive is unsupported"
                                .into(),
                        );
                    }
                }
            }
            nodes.extend(node.child_nodes());
        }
        if scopes.len() != binders.len() {
            return Err("sliding complete generator scopes are unsupported".into());
        }
        let range = self.source.context.files[file].location(body.range()).range;
        for reference in self.source.bindings.references.iter().filter(|reference| {
            reference.file == file
                && reference.kind == ReferenceKind::Value
                && range.start <= reference.location.range.start
                && reference.location.range.end <= range.end
        }) {
            let BindingResolution::Resolved(source) = reference.resolution else {
                return Err("sliding reference is unresolved".into());
            };
            if !formals.contains(&source)
                && !locals.iter().any(|local| local.id == source)
                && !scopes.iter().any(|(binder, range)| {
                    *binder == source
                        && range.start <= reference.location.range.start
                        && reference.location.range.end <= range.end
                })
            {
                return Err("sliding reference has no owning scope".into());
            }
        }
        if wrapper {
            let (id, selected) = nested.ok_or("sliding nested FZN body is unavailable")?;
            let mut forwarded = Invocation {
                actuals: Vec::new(),
                reachable: None,
            };
            // The complete forwarding shape and exact bodyless index2int above
            // retain the original sources. No converted array is manufactured.
            for (position, formal) in formals.iter().enumerate() {
                let actual = invocation
                    .actuals
                    .iter()
                    .rev()
                    .find(|actual| actual.formal == *formal)
                    .ok_or("sliding original actual correspondence is unavailable")?;
                forwarded.actuals.push(InvocationActual {
                    formal: formal_parameter(
                        self.source.context,
                        self.source.bindings,
                        id,
                        position,
                    )
                    .ok_or("sliding nested formal identity is unavailable")?,
                    file: actual.file,
                    node: actual.node,
                    view: actual.view,
                    generators: actual.generators.clone(),
                    collection: actual.collection,
                });
            }
            self.sliding_source_body(id, &selected, &forwarded)?;
        } else {
            let local_nodes = locals
                .iter()
                .map(|local| {
                    find_node(
                        self.source.context.files[file].parsed.tree(),
                        &local.syntax_range,
                        local.role,
                    )
                    .ok_or("sliding local declaration is unavailable")
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.inspect_uncertain_array_extrema(file, &local_nodes, view, Some(invocation))?;
        }
        Ok(())
    }
}
