use super::*;

impl<'a> SourceInspector<'a> {
    // The regular4 route is inspected as a relation, never as a definition of
    // its sequence, transition cells or private automaton states.
    pub(in crate::callable_definitions) fn regular_four_tuple(
        &self,
        parameters: &[TypeInst],
    ) -> bool {
        let integer = TypeInst::par(TypeKind::Int);
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let [sequence, matrix, initial, final_states] = parameters else {
            return false;
        };
        let TypeKind::Array {
            indices: sequence_axes,
            element: symbol,
        } = &sequence.kind
        else {
            return false;
        };
        let TypeKind::Array {
            indices: matrix_axes,
            element: state,
        } = &matrix.kind
        else {
            return false;
        };
        sequence.known()
            && !optional(sequence)
            && sequence.instantiation == Instantiation::Decision
            && sequence_axes.len() == 1
            && sequence_axes[0].instantiation == Instantiation::Parameter
            && !optional(&sequence_axes[0])
            && matches!(sequence_axes[0].kind, TypeKind::Int | TypeKind::Enum(_))
            && symbol.instantiation == Instantiation::Decision
            && matches!(symbol.kind, TypeKind::Int | TypeKind::Enum(_))
            && matrix.known()
            && !matrix.optional
            && matrix.instantiation == Instantiation::Parameter
            && matrix_axes.as_slice()
                == [
                    integer.clone(),
                    symbol.as_ref().clone().with_inst(Instantiation::Parameter),
                ]
            && **state
                == TypeInst {
                    optional: true,
                    ..integer.clone()
                }
            && *initial == integer
            && *final_states == set
    }
    pub(in crate::callable_definitions) fn regular_written_tokens(
        &self,
        file: FileId,
        node: &SyntaxNode,
        expected: &str,
    ) -> bool {
        let parsed = &self.context.files[file].parsed;
        parsed
            .tokens()
            .iter()
            .filter(|token| {
                node.range().start <= token.range.start
                    && token.range.end <= node.range().end
                    && !matches!(
                        token.kind,
                        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                    )
            })
            .map(|token| &parsed.source()[token.range.clone()])
            .eq(expected.split_whitespace())
    }
    pub(in crate::callable_definitions) fn regular_occurs_primitive(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
        id: DeclarationId,
    ) -> Result<(), String> {
        if !crate::optional::core_optional_call(
            self.context,
            self.bindings,
            view,
            file,
            node,
            "occurs",
        ) {
            return Err("regular4 occurs identity is unsupported".into());
        }
        let owner = &self.bindings.declarations[id.0];
        let mut signatures = self
            .calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == id);
        let signature = signatures
            .next()
            .ok_or("regular4 occurs signature is unavailable")?;
        let [parameter] = signature.parameters.as_slice() else {
            return Err("regular4 occurs arity is unsupported".into());
        };
        let TypeKind::Variable {
            name: variable,
            enum_only: false,
            any: false,
        } = &parameter.ty.kind
        else {
            return Err("regular4 occurs written scalar type is unsupported".into());
        };
        if signatures.next().is_some()
            || owner.role != DeclarationRole::Test
            || parameter.has_default
            || parameter.ty.instantiation != Instantiation::Parameter
            || !parameter.ty.optional
            || signature.return_type != TypeInst::par(TypeKind::Bool)
        {
            return Err("regular4 occurs written signature is unsupported".into());
        }
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("regular4 occurs written declaration is unavailable")?;
        let name = parameter
            .name
            .as_ref()
            .ok_or("regular4 occurs written formal is unavailable")?;
        if !self.regular_written_tokens(
            owner.file,
            written,
            &format!("test occurs ( opt {variable} : {name} ) ;"),
        ) {
            return Err("regular4 occurs body, default, type or metadata is unsupported".into());
        }
        Ok(())
    }
    pub(in crate::callable_definitions) fn literal_regular_enum_members(
        &self,
        id: DeclarationId,
    ) -> Result<Vec<DeclarationId>, String> {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        let expected = TypeInst::par(TypeKind::Set(Box::new(TypeInst::par(TypeKind::Enum(id)))));
        if !declaration.top_level
            || declaration.role != DeclarationRole::Enum
            || self.calls.declarations[id.0].ty != expected
        {
            return Err("literal regexp plain enum identity is unsupported".into());
        }
        let written = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("literal regexp enum source unavailable")?;
        let definition = written
            .child_nodes()
            .find(|n| n.kind() == NodeKind::EnumDefinition)
            .ok_or("literal regexp requires a written plain enum")?;
        let parts: Vec<_> = definition.child_nodes().collect();
        let [cases] = parts.as_slice() else {
            return Err("literal regexp enum construction is unsupported".into());
        };
        if cases.kind() != NodeKind::EnumCases {
            return Err("literal regexp enum construction is unsupported".into());
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, declaration.file, node) {
                return Err("literal regexp enum metadata is unsupported".into());
            }
            nodes.extend(node.child_nodes());
        }
        let mut members = Vec::new();
        for case in cases.child_nodes() {
            if case.kind() != NodeKind::EnumCase
                || case.child_nodes().next().is_some()
                || !matches!(crate::domains::tokens(&source.parsed, case).as_slice(),
                    [token] if token.kind == TokenKind::Identifier)
            {
                return Err(
                    "literal regexp enum member construction or spelling is unsupported".into(),
                );
            }
            let member = self
                .bindings
                .declarations
                .iter()
                .find(|d| {
                    d.file == declaration.file
                        && d.item == declaration.item
                        && d.role == DeclarationRole::EnumMember
                        && d.syntax_range == case.range()
                })
                .ok_or("literal regexp physical enum member unavailable")?;
            if self.calls.declarations[member.id.0].ty != TypeInst::par(TypeKind::Enum(id)) {
                return Err("literal regexp enum member type is unsupported".into());
            }
            members.push(member.id);
        }
        // Native dot constructs an integer interval. This local finite-domain
        // prerequisite supplies no published enum ordinal or cardinality fact.
        if members.is_empty() || members.len() > (i32::MAX - 1) as usize {
            return Err(
                "literal regexp enum must be nonempty and fit native integer bounds".into(),
            );
        }
        Ok(members)
    }
    pub(in crate::callable_definitions) fn literal_regular_pattern(
        &self,
        file: FileId,
        written: &SyntaxNode,
    ) -> Result<Vec<String>, String> {
        let node = unwrap(written);
        let tokens = crate::domains::tokens(&self.context.files[file].parsed, node);
        let [token] = tokens.as_slice() else {
            return Err("literal regexp requires one string literal".into());
        };
        if node.kind() != NodeKind::Expression || token.kind != TokenKind::StringLiteral {
            return Err("literal regexp requires one string literal".into());
        }
        let text = &self.context.files[file].parsed.source()[token.range.clone()];
        let text = text
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .ok_or("literal regexp string spelling is unsupported")?;
        let mut decoded = String::new();
        let mut bytes = text.bytes();
        while let Some(byte) = bytes.next() {
            let byte = if byte == b'\\' {
                match bytes.next() {
                    Some(b'n') => b'\n',
                    Some(b't') => b'\t',
                    _ => return Err("literal regexp string escape is unsupported".into()),
                }
            } else {
                byte
            };
            if !byte.is_ascii() {
                return Err("literal regexp non-ASCII syntax is unsupported".into());
            }
            decoded.push(char::from(byte));
        }
        let tokens: Vec<_> = decoded
            .split([' ', '\t', '\n'])
            .filter(|s| !s.is_empty())
            .collect();
        if tokens.is_empty() {
            return Err("literal regexp sequence is empty".into());
        }
        for token in &tokens {
            let mut bytes = token.bytes();
            if !matches!(*token, "." | ".*")
                && !(bytes.next().is_some_and(|b| b.is_ascii_alphabetic())
                    && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            {
                return Err("literal regexp token is outside the checked sequence subset".into());
            }
        }
        Ok(tokens.into_iter().map(str::to_owned).collect())
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn inspect_regular_four(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
        invocation: Option<&Invocation<'a, '_>>,
    ) -> Option<Result<(), String>> {
        let owner = &self.source.bindings.declarations[id.0];
        if clause.node.kind() != NodeKind::CallExpression
            || owner.role != DeclarationRole::Predicate
            || owner.name != "regular"
            || self.source.context.files[owner.file].kind != SourceKind::StandardLibrary
            || !self.source.regular_four_tuple(parameters)
        {
            return None;
        }
        Some((|| {
            if !crate::definitions::annotations_safe(self.source.context, clause.file, clause.node)
            {
                return Err("regular4 actual annotation is unsupported".into());
            }
            // Every inherited header still evaluates even when no output is inferred.
            let inspect_headers = |file, facts: &CallableFacts, headers: &[&'a SyntaxNode]| {
                for (position, header) in headers.iter().enumerate() {
                    for source in header.child_nodes() {
                        let filter = source.kind() == NodeKind::WhereFilter;
                        let source = if filter {
                            source
                                .child_nodes()
                                .next()
                                .ok_or("regular4 inherited filter is unavailable")?
                        } else {
                            source
                        };
                        let visible = if filter { position + 1 } else { position };
                        self.optional_matrix_source_safety(
                            (file, source, source.range().start),
                            self.source.view(file, source, facts),
                            &headers[..visible],
                            &mut Vec::new(),
                        )?;
                    }
                }
                Ok::<(), String>(())
            };
            inspect_headers(clause.file, view, &clause.generators)?;
            for (position, parameter) in parameters.iter().enumerate() {
                let (file, node) = call_argument(
                    self.source.context,
                    self.source.bindings,
                    self.source.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                    id,
                    position,
                )
                .ok_or("regular4 actual is unavailable")?;
                let forwarded = invocation
                    .map(|frame| self.invocation_source(file, node, frame))
                    .transpose()?
                    .flatten();
                if let Some(actual) = forwarded {
                    inspect_headers(actual.file, actual.view, &actual.generators)?;
                }
                if forwarded.is_some_and(|actual| actual.collection) {
                    return Err("regular4 generated actual is unsupported".into());
                }
                let (file, node, facts, headers) = forwarded.map_or(
                    (
                        file,
                        node,
                        self.source.view(file, node, view),
                        &clause.generators[..],
                    ),
                    |actual| {
                        (
                            actual.file,
                            actual.node,
                            actual.view,
                            &actual.generators[..],
                        )
                    },
                );
                if self
                    .source
                    .expression_type(facts, file, node)
                    .is_none_or(|actual| actual.ty != *parameter)
                {
                    return Err("regular4 raw actual does not match its selected formal".into());
                }
                let mut written = vec![node];
                while let Some(source) = written.pop() {
                    if !self.source.source_annotations_safe(file, source) {
                        return Err("regular4 written actual annotation is unsupported".into());
                    }
                    written.extend(source.child_nodes());
                }
                if position == 1 {
                    let node = unwrap(node);
                    let source = self
                        .source
                        .reference(file, node)
                        .filter(|_| {
                            node.kind() == NodeKind::Expression
                                && node.child_nodes().next().is_none()
                        })
                        .ok_or("regular4 transition matrix requires its inspected owning Local")?;
                    let declaration = &self.source.bindings.declarations[source.0];
                    if declaration.file != file
                        || declaration.role != DeclarationRole::Local
                        || declaration.syntax_range.end > node.range().start
                    {
                        return Err("regular4 transition matrix owning Local is unsupported".into());
                    }
                    let written = find_node(
                        self.source.context.files[file].parsed.tree(),
                        &declaration.syntax_range,
                        DeclarationRole::Local,
                    )
                    .ok_or("regular4 transition matrix declaration is unavailable")?;
                    let mut pending = vec![self.source.context.files[file].parsed.tree()];
                    let mut owning_scope = false;
                    while let Some(scope) = pending.pop() {
                        if scope.range().start <= written.range().start
                            && node.range().end <= scope.range().end
                        {
                            if scope.kind() == NodeKind::LetExpression
                                && scope
                                    .child_nodes()
                                    .filter(|child| child.kind() == NodeKind::LetBlock)
                                    .any(|block| {
                                        block
                                            .child_nodes()
                                            .any(|local| std::ptr::eq(local, written))
                                    })
                            {
                                owning_scope = true;
                            }
                            pending.extend(scope.child_nodes());
                        }
                    }
                    if !owning_scope {
                        return Err("regular4 transition matrix has no owning lexical scope".into());
                    }
                    match self
                        .optional_parameter_matrix_safety(file, written, source, facts, headers)
                    {
                        Some(DefinitionSafety::Unknown(_)) => {}
                        Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                        _ => {
                            return Err(
                                "regular4 transition matrix constructor is unsupported".into()
                            );
                        }
                    }
                } else {
                    self.optional_matrix_source_safety(
                        (file, node, node.range().start),
                        facts,
                        headers,
                        &mut Vec::new(),
                    )?;
                }
            }
            self.regular_source_body(id, parameters)
        })())
    }
    pub(in crate::callable_definitions) fn regular_four_owning_body(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
    ) -> Option<DefinitionSafety> {
        let instance = self.instances.iter().find(|instance| {
            let owner = &self.source.bindings.declarations[instance.id.0];
            owner.file == clause.file
                && owner.name == "regular"
                && owner.role == DeclarationRole::Predicate
                && self.source.context.files[owner.file].kind == SourceKind::StandardLibrary
                && self.source.regular_four_tuple(&instance.parameters)
                && instance
                    .parameters
                    .iter()
                    .enumerate()
                    .all(|(position, parameter)| {
                        formal_parameter(
                            self.source.context,
                            self.source.bindings,
                            instance.id,
                            position,
                        )
                        .is_some_and(|formal| view.declarations[formal.0].ty == *parameter)
                    })
                && instance
                    .clauses
                    .iter()
                    .any(|body| std::ptr::eq(body.node, clause.node))
        })?;
        Some(
            match self.regular_source_body(instance.id, &instance.parameters) {
                Ok(()) => DefinitionSafety::Unknown(
                    "regular4 relation has no output or membership proof".into(),
                ),
                Err(reason) => DefinitionSafety::Unsupported(reason),
            },
        )
    }
    // These three concrete owning bodies share the header, scope and primitive
    // checks. Their complete token shapes retain both branches and every guard.
    pub(in crate::callable_definitions) fn regular_source_body(
        &self,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Result<(), String> {
        let owner = &self.source.bindings.declarations[id.0];
        let file = owner.file;
        if owner.role != DeclarationRole::Predicate
            || self.source.context.files[file].kind != SourceKind::StandardLibrary
        {
            return Err("regular source owning declaration is unsupported".into());
        }
        let integer = TypeInst::par(TypeKind::Int);
        let boolean = TypeInst::par(TypeKind::Bool);
        let decision_integer = integer.clone().with_inst(Instantiation::Decision);
        let decision_boolean = boolean.clone().with_inst(Instantiation::Decision);
        let optional_integer = TypeInst {
            optional: true,
            ..integer.clone()
        };
        let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
        let array = |element: TypeInst| TypeInst {
            instantiation: element.instantiation,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(element),
            },
        };
        let matrix = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone(), integer.clone()],
            element: Box::new(integer.clone()),
        });
        let regular = owner.name == "regular" && self.source.regular_four_tuple(parameters);
        let set_branch = owner.name == "fzn_regular_set";
        if !regular
            && !(matches!(owner.name.as_str(), "fzn_regular" | "fzn_regular_set")
                && parameters
                    == [
                        array(decision_integer.clone()),
                        integer.clone(),
                        if set_branch {
                            set.clone()
                        } else {
                            integer.clone()
                        },
                        matrix.clone(),
                        integer.clone(),
                        set.clone(),
                    ])
        {
            return Err("regular source selected tuple is unsupported".into());
        }
        let written = find_node(
            self.source.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("regular source written declaration is unavailable")?;
        // Known source and body failures precede an unsupported written shape.
        if let Some(reason) = self
            .source
            .closed_integer_source_error(file, written, false, true)
        {
            return Err(reason);
        }
        let mut signatures = self
            .source
            .calls
            .signatures
            .iter()
            .filter(|signature| signature.declaration == id);
        let signature = signatures
            .next()
            .ok_or("regular source signature is unavailable")?;
        if signatures.next().is_some()
            || signature.parameters.len() != parameters.len()
            || signature
                .parameters
                .iter()
                .any(|parameter| parameter.has_default)
            || signature.return_type != decision_boolean
        {
            return Err("regular source signature or defaults are unsupported".into());
        }
        let instance = self
            .instances
            .iter()
            .find(|instance| {
                instance.id == id && instance.parameters == parameters && !instance.recursive
            })
            .ok_or("regular source exact nonrecursive body view is unavailable")?;
        let view = &instance.view;
        let bodies: Vec<_> = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .collect();
        let [body] = bodies.as_slice() else {
            return Err("regular source requires its complete owning body".into());
        };
        if self
            .source
            .expression_type(view, file, body)
            .is_none_or(|expression| expression.ty != decision_boolean)
        {
            return Err("regular source owning body type is unsupported".into());
        }
        let formals = (0..parameters.len())
            .map(|position| {
                let formal =
                    formal_parameter(self.source.context, self.source.bindings, id, position)
                        .ok_or("regular source formal identity is unavailable")?;
                if view.declarations[formal.0].ty != parameters[position] {
                    return Err("regular source concrete formal type is unsupported".into());
                }
                Ok(formal)
            })
            .collect::<Result<Vec<_>, String>>()?;
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
        let local_types = if regular {
            vec![
                set.clone(),
                set.clone(),
                integer.clone(),
                matrix.clone(),
                integer.clone(),
                set.clone(),
            ]
        } else {
            vec![
                integer.clone(),
                integer.clone(),
                array(decision_integer.clone()),
            ]
        };
        let binder_types = if regular {
            vec![optional_integer.clone(), integer.clone()]
        } else {
            vec![integer.clone()]
        };
        if locals.len() != local_types.len()
            || binders.len() != binder_types.len()
            || locals
                .iter()
                .zip(&local_types)
                .any(|(local, ty)| view.declarations[local.id.0].ty != *ty)
            || binders
                .iter()
                .zip(&binder_types)
                .any(|(binder, ty)| view.declarations[binder.id.0].ty != *ty)
        {
            return Err("regular source local or binder types are unsupported".into());
        }
        let name = |id: DeclarationId| self.source.bindings.declarations[id.0].name.as_str();
        let names: Vec<_> = formals
            .iter()
            .copied()
            .chain(locals.iter().map(|local| local.id))
            .chain(binders.iter().map(|binder| binder.id))
            .map(name)
            .collect();
        if names
            .iter()
            .enumerate()
            .any(|(position, name)| names[..position].contains(name))
        {
            return Err("regular source owning names are shadowed".into());
        }
        let expected = if regular {
            let variable = |ty: &TypeInst| match &ty.kind {
                TypeKind::Variable {
                    name,
                    enum_only: true,
                    any: false,
                } => Ok(name.clone()),
                _ => Err("regular4 written generic identity is unsupported"),
            };
            let TypeKind::Array { indices, element } = &signature.parameters[0].ty.kind else {
                return Err("regular4 written sequence signature is unsupported".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("regular4 written sequence rank is unsupported".into());
            };
            let xvar = variable(axis)?;
            let vvar = variable(element)?;
            let svar = variable(&signature.parameters[2].ty)?;
            if xvar == vvar || xvar == svar || vvar == svar {
                return Err("regular4 written generic links are unsupported".into());
            }
            let (xs, d, q0, final_states) = (
                name(formals[0]),
                name(formals[1]),
                name(formals[2]),
                name(formals[3]),
            );
            let (state, val, offset, dd, initial, final_values) = (
                &locals[0].name,
                &locals[1].name,
                &locals[2].name,
                &locals[3].name,
                &locals[4].name,
                &locals[5].name,
            );
            let (cell, member) = (&binders[0].name, &binders[1].name);
            format!("predicate regular ( array [ {xvar} ] of var {vvar} : {xs} , array [ {svar} , {vvar} ] of opt {svar} : {d} , {svar} : {q0} , set of {svar} : {final_states} , ) =
                let {{ any : {state} = enum2int ( index_set_1of2 ( {d} ) ) ;
                    any : {val} = enum2int ( index_set_2of2 ( {d} ) ) ;
                    any : {offset} = enum2int ( min ( {state} ) ) - 1 ;
                    any : {dd} = array2d ( {state} , {val} , [ if occurs ( {cell} ) then enum2int ( deopt ( {cell} ) ) - {offset} else 0 endif | {cell} in {d} ] , ) ;
                    any : {initial} = enum2int ( {q0} ) - {offset} ;
                    any : {final_values} = {{ enum2int ( {member} ) - {offset} | {member} in {final_states} }} ;
                }} in if min ( {val} ) = 1 then fzn_regular ( index2int ( enum2int ( {xs} ) ) , card ( {state} ) , max ( {val} ) , {dd} , {initial} , {final_values} )
                    else fzn_regular_set ( index2int ( enum2int ( {xs} ) ) , card ( {state} ) , {val} , {dd} , {initial} , {final_values} ) endif ;")
        } else {
            let (xs, q, s, d, q0, final_states) = (
                name(formals[0]),
                name(formals[1]),
                name(formals[2]),
                name(formals[3]),
                name(formals[4]),
                name(formals[5]),
            );
            let (m, n, a, i) = (
                &locals[0].name,
                &locals[1].name,
                &locals[2].name,
                &binders[0].name,
            );
            let initial = if set_branch {
                String::new()
            } else {
                format!("if length ( {xs} ) = 0 then {q0} in {final_states} else")
            };
            let end = if set_branch { "" } else { "endif" };
            let alphabet = if set_branch {
                s.to_owned()
            } else {
                format!("1 .. {s}")
            };
            let alphabet_type = if set_branch { "set of int" } else { "int" };
            format!("predicate {callee} ( array [ int ] of var int : {xs} , int : {q} , {alphabet_type} : {s} , array [ int , int ] of int : {d} , int : {q0} , set of int : {final_states} , ) =
                {initial} let {{ int : {m} = min ( index_set ( {xs} ) ) ; int : {n} = max ( index_set ( {xs} ) ) + 1 ; array [ {m} .. {n} ] of var 1 .. {q} : {a} ; }} in
                {a} [ {m} ] = {q0} /\\ forall ( {i} in index_set ( {xs} ) , ) ( {xs} [ {i} ] in {alphabet} /\\ {a} [ {i} + 1 ] = {d} [ {a} [ {i} ] , {xs} [ {i} ] ] ) /\\ {a} [ {n} ] in {final_states} {end} ;", callee=owner.name)
        };
        if !self.source.regular_written_tokens(file, written, &expected) {
            return Err("regular source complete written body or header is unsupported".into());
        }
        let body_range = self.source.context.files[file].location(body.range()).range;
        let mut scopes = Vec::new();
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.source.context, file, node) {
                return Err("regular source written annotation is unsupported".into());
            }
            if is_expression(node.kind())
                && self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|expression| !expression.ty.known())
            {
                return Err("regular source written expression type is unsupported".into());
            }
            if matches!(
                node.kind(),
                NodeKind::ArrayComprehension
                    | NodeKind::SetComprehension
                    | NodeKind::GeneratorCallExpression
            ) {
                let list = node
                    .child_nodes()
                    .find(|child| child.kind() == NodeKind::GeneratorList)
                    .ok_or("regular source generator list is unavailable")?;
                for header in list.child_nodes() {
                    let binder = binders
                        .iter()
                        .find(|binder| binder.syntax_range == header.range())
                        .ok_or("regular source header has no owning binder")?;
                    scopes.push((
                        binder.id,
                        self.source.context.files[file].location(node.range()).range,
                    ));
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
                    .ok_or("regular source selected operation is unavailable")?;
                let CallOutcome::Resolved {
                    declaration,
                    parameters: selected,
                    return_type,
                } = &fact.outcome
                else {
                    return Err("regular source selected operation is unsupported".into());
                };
                let actuals = if node.kind() == NodeKind::GeneratorCallExpression {
                    vec![
                        fact.generator_argument
                            .clone()
                            .ok_or("regular source generator collection is unavailable")?,
                    ]
                } else {
                    node.child_nodes()
                        .map(|child| {
                            self.source
                                .expression_type(view, file, child)
                                .map(|expression| expression.ty.clone())
                                .ok_or("regular source operand type is unavailable")
                        })
                        .collect::<Result<Vec<_>, _>>()?
                };
                if self
                    .source
                    .expression_type(view, file, node)
                    .is_none_or(|expression| expression.ty != *return_type)
                    || selected.len() != actuals.len()
                    || actuals
                        .iter()
                        .zip(selected)
                        .any(|(actual, formal)| !crate::types::coerces(actual, formal))
                {
                    return Err("regular source selected operation tuple is unsupported".into());
                }
                if matches!(fact.name.as_str(), "fzn_regular" | "fzn_regular_set") {
                    if !regular || *return_type != decision_boolean {
                        return Err("regular source nested FZN result is unsupported".into());
                    }
                    self.regular_source_body(*declaration, selected)?;
                } else {
                    let unary = |ty: &TypeInst| selected.as_slice() == std::slice::from_ref(ty);
                    let pair = |ty: &TypeInst| selected.as_slice() == [ty.clone(), ty.clone()];
                    let tuple = match fact.name.as_str() {
                        "index_set_1of2" | "index_set_2of2" => {
                            regular
                                && unary(&parameters[1])
                                && matches!(&return_type.kind, TypeKind::Set(element)
                                if return_type.instantiation == Instantiation::Parameter && !return_type.optional
                                    && match &parameters[1].kind {
                                        TypeKind::Array { indices, .. } => **element == indices[usize::from(fact.name == "index_set_2of2")],
                                        _ => false,
                                    })
                        }
                        "index_set" => {
                            unary(&array(decision_integer.clone())) && *return_type == set
                        }
                        "length" => {
                            unary(&array(decision_integer.clone())) && *return_type == integer
                        }
                        "min" | "max" | "card" => unary(&set) && *return_type == integer,
                        "+" | "-" => pair(&integer) && *return_type == integer,
                        ".." => pair(&integer) && *return_type == set,
                        "=" => {
                            pair(&integer) && *return_type == boolean
                                || pair(&decision_integer) && *return_type == decision_boolean
                                || !regular
                                    && selected.as_slice()
                                        == [decision_integer.clone(), integer.clone()]
                                    && *return_type == decision_boolean
                        }
                        "in" => {
                            selected.as_slice() == [integer.clone(), set.clone()]
                                && *return_type == boolean
                                || selected.as_slice() == [decision_integer.clone(), set.clone()]
                                    && *return_type == decision_boolean
                        }
                        "/\\" => pair(&decision_boolean) && *return_type == decision_boolean,
                        "forall" => {
                            !regular
                                && unary(&array(decision_boolean.clone()))
                                && *return_type == decision_boolean
                        }
                        "occurs" => regular && unary(&optional_integer) && *return_type == boolean,
                        "deopt" => regular && unary(&optional_integer) && *return_type == integer,
                        "array2d" => {
                            regular
                                && selected.as_slice()
                                    == [set.clone(), set.clone(), array(integer.clone())]
                                && *return_type == matrix
                        }
                        "enum2int" => {
                            regular && selected.len() == 1 && {
                                let input = &selected[0];
                                let mut converted = input.clone();
                                match &mut converted.kind {
                                    TypeKind::Int | TypeKind::Enum(_) => {
                                        converted.kind = TypeKind::Int
                                    }
                                    TypeKind::Set(element)
                                        if !optional(element)
                                            && matches!(
                                                element.kind,
                                                TypeKind::Int | TypeKind::Enum(_)
                                            ) =>
                                    {
                                        element.kind = TypeKind::Int
                                    }
                                    TypeKind::Array { indices, element }
                                        if indices.len() == 1
                                            && !optional(element)
                                            && matches!(
                                                element.kind,
                                                TypeKind::Int | TypeKind::Enum(_)
                                            ) =>
                                    {
                                        element.kind = TypeKind::Int
                                    }
                                    _ => {
                                        return Err(
                                            "regular source enum conversion type is unsupported"
                                                .into(),
                                        );
                                    }
                                }
                                !optional(input) && converted == *return_type
                            }
                        }
                        "index2int" => {
                            regular
                                && self
                                    .source
                                    .array_conversion_argument(file, node, view)
                                    .is_some()
                        }
                        _ => false,
                    };
                    let identity = if fact.name == "occurs" {
                        crate::optional::core_optional_call(
                            self.source.context,
                            self.source.bindings,
                            view,
                            file,
                            node,
                            "occurs",
                        )
                    } else {
                        self.source.core(file, node, view, &fact.name)
                    };
                    if !tuple || !identity {
                        return Err(
                            "regular source operation identity or exact tuple is unsupported"
                                .into(),
                        );
                    }
                    if fact.name == "occurs" {
                        self.source
                            .regular_occurs_primitive(file, node, view, *declaration)?;
                    } else if node.kind() == NodeKind::GeneratorCallExpression {
                        self.source.selected_union_primitive_safety(*declaration)?;
                    } else {
                        self.source.parameter_array_primitive(
                            file,
                            node,
                            view,
                            &fact.name,
                            selected,
                            return_type,
                        )?;
                    }
                }
            }
            nodes.extend(node.child_nodes());
        }
        if scopes.len() != binders.len() {
            return Err("regular source complete generator scopes are unsupported".into());
        }
        for reference in self.source.bindings.references.iter().filter(|reference| {
            reference.file == file
                && reference.kind == ReferenceKind::Value
                && body_range.start <= reference.location.range.start
                && reference.location.range.end <= body_range.end
        }) {
            let BindingResolution::Resolved(id) = reference.resolution else {
                return Err("regular source reference is unresolved".into());
            };
            if formals.contains(&id) || locals.iter().any(|local| local.id == id) {
                continue;
            }
            if !scopes.iter().any(|(binder, range)| {
                *binder == id
                    && range.start <= reference.location.range.start
                    && reference.location.range.end <= range.end
            }) {
                return Err("regular source reference has no owning scope".into());
            }
        }
        Ok(())
    }
    // Inspect a finite literal regexp source, never its match result or outputs.
    pub(in crate::callable_definitions) fn inspect_literal_regular(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
        invocation: Option<&Invocation<'a, '_>>,
    ) -> Option<Result<(), String>> {
        let declaration = &self.source.bindings.declarations[id.0];
        let source = &self.source.context.files[declaration.file];
        if clause.node.kind() != NodeKind::CallExpression
            || declaration.name != "fzn_regular"
            || declaration.role != DeclarationRole::Predicate
            || source.kind != SourceKind::StandardLibrary
            || source.implicit
        {
            return None;
        }
        let written = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        if parameters.len() != 2 || written.child_nodes().any(|node| is_expression(node.kind())) {
            return None;
        }
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst {
            instantiation: Instantiation::Decision,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer],
                element: Box::new(TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision)),
            },
        };
        let string = TypeInst::par(TypeKind::String);
        let boolean = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let facts = self.source.view(clause.file, clause.node, view);
        if parameters != [array.clone(), string.clone()]
            || self.source.operation_fact(facts, clause.file, clause.node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { declaration, parameters: selected, return_type }
                    if *declaration == id && selected.as_slice() == parameters && return_type == &boolean)
            })
        {
            return None;
        }
        Some((|| {
            if !crate::definitions::annotations_safe(self.source.context, clause.file, clause.node)
            {
                return Err("literal regexp call annotation is unsupported".into());
            }
            if let Some(reason) =
                self.source
                    .closed_integer_source_error(declaration.file, written, false, true)
            {
                return Err(reason);
            }
            if !self
                .source
                .callable_annotations_safe(declaration.file, written)
            {
                return Err("literal regexp primitive metadata is unsupported".into());
            }
            // Skip only the declaration's validated metadata; formal/type
            // annotations and their sources remain in the complete deep scan.
            let mut nodes: Vec<_> = written
                .child_nodes()
                .filter(|node| node.kind() != NodeKind::Annotation)
                .collect();
            while let Some(node) = nodes.pop() {
                if is_expression(node.kind())
                    || !crate::definitions::annotations_safe(
                        self.source.context,
                        declaration.file,
                        node,
                    )
                {
                    return Err(
                        "literal regexp primitive body, default or type source is unsupported"
                            .into(),
                    );
                }
                nodes.extend(node.child_nodes());
            }
            let mut signatures = self
                .source
                .calls
                .signatures
                .iter()
                .filter(|s| s.declaration == id);
            let signature = signatures
                .next()
                .ok_or("literal regexp signature unavailable")?;
            if signatures.next().is_some()
                || signature.return_type != boolean
                || signature.parameters.len() != 2
                || signature.parameters.iter().any(|p| p.has_default)
            {
                return Err("literal regexp written signature is unsupported".into());
            }
            self.type_dependencies(declaration.file, written, view, &[])?;
            for (position, parameter) in parameters.iter().enumerate() {
                let formal =
                    formal_parameter(self.source.context, self.source.bindings, id, position)
                        .ok_or("literal regexp formal identity unavailable")?;
                if self.source.calls.declarations[formal.0].ty != *parameter {
                    return Err("literal regexp written formal type is unsupported".into());
                }
            }
            let argument = |position| {
                call_argument(
                    self.source.context,
                    self.source.bindings,
                    facts,
                    clause.file,
                    clause.node,
                    id,
                    position,
                )
                .ok_or_else(|| "literal regexp actual unavailable".to_owned())
            };
            let (array_file, converted) = argument(0)?;
            let (pattern_file, pattern) = argument(1)?;
            let conversion = self
                .source
                .array_conversion_argument(array_file, converted, view)
                .ok_or("literal regexp requires a checked enum conversion")?;
            let conversion_facts = self.source.view(array_file, converted, view);
            let CallOutcome::Resolved {
                parameters: conversion_parameters,
                return_type,
                ..
            } = &self
                .source
                .operation_fact(conversion_facts, array_file, converted)
                .ok_or("literal regexp conversion selection unavailable")?
                .outcome
            else {
                return Err("literal regexp conversion selection is unsupported".into());
            };
            self.source.parameter_array_primitive(
                array_file,
                converted,
                view,
                "enum2int",
                conversion_parameters,
                return_type,
            )?;
            if return_type != &array {
                return Err("literal regexp conversion result is unsupported".into());
            }
            let array_source = invocation
                .map(|frame| self.invocation_source(array_file, conversion, frame))
                .transpose()?
                .flatten();
            let pattern_source = invocation
                .map(|frame| self.invocation_source(pattern_file, pattern, frame))
                .transpose()?
                .flatten();
            if array_source.is_some_and(|source| source.collection)
                || pattern_source.is_some_and(|source| source.collection)
            {
                return Err("literal regexp generated actuals are unsupported".into());
            }
            let (array_file, actual, array_view, headers) = array_source.map_or(
                (
                    array_file,
                    conversion,
                    self.source.view(array_file, conversion, view),
                    &clause.generators[..],
                ),
                |source| {
                    (
                        source.file,
                        source.node,
                        source.view,
                        &source.generators[..],
                    )
                },
            );
            let (pattern_file, pattern, pattern_view, pattern_headers) = pattern_source.map_or(
                (
                    pattern_file,
                    pattern,
                    self.source.view(pattern_file, pattern, view),
                    &clause.generators[..],
                ),
                |source| {
                    (
                        source.file,
                        source.node,
                        source.view,
                        &source.generators[..],
                    )
                },
            );
            // Inspect both actuals and every lexical header/filter before rejecting
            // a shape; symbolic membership never hides a closed source error.
            for (file, node, facts, generators) in [
                (array_file, actual, array_view, headers),
                (pattern_file, pattern, pattern_view, pattern_headers),
            ] {
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(file, node, false, true)
                {
                    return Err(reason);
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, node, facts, generators, &mut Vec::new())
                {
                    return Err(reason);
                }
                for (position, generator) in generators.iter().enumerate() {
                    for child in generator.child_nodes() {
                        let value = if child.kind() == NodeKind::WhereFilter {
                            child
                                .child_nodes()
                                .next()
                                .ok_or("literal regexp filter unavailable")?
                        } else {
                            child
                        };
                        if child.kind() == NodeKind::WhereFilter
                            && self
                                .source
                                .expression_type(self.source.view(file, value, facts), file, value)
                                .is_none_or(|expression| {
                                    !expression.ty.known()
                                        || optional(&expression.ty)
                                        || expression.ty.kind != TypeKind::Bool
                                })
                        {
                            return Err(
                                "literal regexp filter type or optionality is unsupported".into()
                            );
                        }
                        if let Some(reason) = self
                            .source
                            .closed_integer_source_error(file, value, false, true)
                        {
                            return Err(reason);
                        }
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(
                                file,
                                value,
                                facts,
                                &generators[..=position],
                                &mut Vec::new(),
                            )
                        {
                            return Err(reason);
                        }
                    }
                }
            }
            if self
                .source
                .expression_type(pattern_view, pattern_file, pattern)
                .is_none_or(|value| value.ty != string)
            {
                return Err("literal regexp pattern type is unsupported".into());
            }
            let actual = unwrap(actual);
            let actual_type = &self
                .source
                .expression_type(array_view, array_file, actual)
                .ok_or("literal regexp array type unavailable")?
                .ty;
            let TypeKind::Array { indices, element } = &actual_type.kind else {
                return Err("literal regexp source is not an array".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("literal regexp source must retain one axis".into());
            };
            let (TypeKind::Enum(axis_id), TypeKind::Enum(enum_id)) = (&axis.kind, &element.kind)
            else {
                return Err("literal regexp source requires plain enum axes and elements".into());
            };
            if !actual_type.known()
                || optional(actual_type)
                || actual_type.instantiation != Instantiation::Decision
                || axis.instantiation != Instantiation::Parameter
                || element.instantiation != Instantiation::Decision
                || !matches!(conversion_parameters.as_slice(), [input]
                    if input.instantiation == actual_type.instantiation && !optional(input)
                        && matches!(&input.kind, TypeKind::Array { indices, element: converted_element }
                            if indices == &[TypeInst::par(TypeKind::Int)] && converted_element == element))
            {
                return Err("literal regexp nominal source transport is unsupported".into());
            }
            let subject = if actual.kind() == NodeKind::ArrayAccessExpression {
                match self.full_axis_slice_safety(array_file, actual, array_view, headers) {
                    Some(DefinitionSafety::Unknown(_)) => {}
                    Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                    _ => {
                        return Err(
                            "literal regexp source requires a checked full-axis slice".into()
                        );
                    }
                }
                unwrap(
                    actual
                        .child_nodes()
                        .next()
                        .ok_or("literal regexp slice subject unavailable")?,
                )
            } else {
                actual
            };
            let owner_id = (subject.kind() == NodeKind::Expression)
                .then(|| self.source.reference(array_file, subject))
                .flatten()
                .ok_or("literal regexp owning array identity unavailable")?;
            let owner = &self.source.bindings.declarations[owner_id.0];
            let owner_written = find_node(
                self.source.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("literal regexp owning array source unavailable")?;
            if !owner.top_level
                || owner.role != DeclarationRole::Value
                || owner_written.child_nodes().any(|n| is_expression(n.kind()))
            {
                return Err("literal regexp requires an uninitialized whole-enum array".into());
            }
            let Domain::Array {
                indices: source_axes,
                element: source_element,
            } = &self.source.domains.declarations[owner_id.0].domain
            else {
                return Err("literal regexp source domains unavailable".into());
            };
            let whole_enum = |domain: &Domain, expected| {
                matches!(domain,
                Domain::Named { declaration, domain } if *declaration == expected && **domain == Domain::Enum(expected))
            };
            let retained_axis = if actual.kind() == NodeKind::ArrayAccessExpression {
                actual
                    .child_nodes()
                    .skip(1)
                    .zip(source_axes)
                    .find_map(|(selector, axis)| {
                        (selector.kind() == NodeKind::RangeExpression
                            && selector.child_nodes().next().is_none())
                        .then_some(axis)
                    })
            } else {
                (source_axes.len() == 1).then(|| &source_axes[0])
            };
            if !whole_enum(source_element, *enum_id)
                || retained_axis.is_none_or(|axis| !whole_enum(axis, *axis_id))
            {
                return Err("literal regexp requires exact whole-enum source domains".into());
            }
            self.source.literal_regular_enum_members(*axis_id)?;
            let members = self.source.literal_regular_enum_members(*enum_id)?;
            for token in self.source.literal_regular_pattern(pattern_file, pattern)? {
                if matches!(token.as_str(), "." | ".*") {
                    continue;
                }
                let mut identities = self.source.bindings.declarations.iter().filter(|d| {
                    d.name == token
                        && matches!(
                            d.role,
                            DeclarationRole::EnumMember | DeclarationRole::EnumConstructor
                        )
                });
                let member = identities
                    .next()
                    .ok_or("literal regexp enum member is unknown")?;
                if identities.next().is_some() || !members.contains(&member.id) {
                    return Err(
                        "literal regexp enum member identity or nominal domain is unsupported"
                            .into(),
                    );
                }
            }
            // Sources are inspected, but scalar membership and match evaluation
            // remain unknown. No output, cardinality or coverage is established.
            Ok(())
        })())
    }
}
