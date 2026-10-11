use super::*;

impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn gcc_tuple(&self, parameters: &[TypeInst]) -> bool {
        let integer = TypeInst::par(TypeKind::Int);
        let [xs, cover, lower, upper] = parameters else {
            return false;
        };
        let TypeKind::Array { indices, element } = &xs.kind else {
            return false;
        };
        let array = |element: TypeInst| TypeInst {
            instantiation: element.instantiation,
            optional: false,
            kind: TypeKind::Array {
                indices: vec![integer.clone()],
                element: Box::new(element),
            },
        };
        xs.known()
            && !optional(xs)
            && xs.instantiation == Instantiation::Decision
            && indices.len() == 1
            && indices[0].instantiation == Instantiation::Parameter
            && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
            && element.instantiation == Instantiation::Decision
            && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
            && *cover == array(element.as_ref().clone().with_inst(Instantiation::Parameter))
            && *lower == array(integer.clone())
            && *upper == *lower
    }
    // The exact complete shapes keep the outer assertions, every native branch
    // and the real count let. Changed bodies remain unsupported, never primitives.
    pub(in crate::callable_definitions) fn gcc_source_body(
        &self,
        id: DeclarationId,
        parameters: &[TypeInst],
        active: &mut Vec<DeclarationId>,
    ) -> Result<(), String> {
        if active.contains(&id) {
            return Err("GCC selected body is recursive".into());
        }
        active.push(id);
        let checked: Result<(), String> = (|| {
            let owner = &self.bindings.declarations[id.0];
            let file = owner.file;
            let integer = TypeInst::par(TypeKind::Int);
            let boolean = TypeInst::par(TypeKind::Bool);
            let string = TypeInst::par(TypeKind::String);
            let decision_integer = integer.clone().with_inst(Instantiation::Decision);
            let decision_boolean = boolean.clone().with_inst(Instantiation::Decision);
            let set = TypeInst::par(TypeKind::Set(Box::new(integer.clone())));
            let array = |element: TypeInst| TypeInst {
                instantiation: element.instantiation,
                optional: false,
                kind: TypeKind::Array {
                    indices: vec![integer.clone()],
                    element: Box::new(element),
                },
            };
            let outer = owner.name == "global_cardinality";
            let native = owner.name == "fzn_global_cardinality_low_up";
            let count = owner.name == "count";
            let result = if count {
                decision_integer.clone()
            } else {
                decision_boolean.clone()
            };
            if self.context.files[file].kind != SourceKind::StandardLibrary
                || !(outer
                    && owner.role == DeclarationRole::Predicate
                    && !self.context.files[file].implicit
                    && self.gcc_tuple(parameters)
                    || native
                        && owner.role == DeclarationRole::Predicate
                        && !self.context.files[file].implicit
                        && parameters
                            == [
                                array(decision_integer.clone()),
                                array(integer.clone()),
                                array(integer.clone()),
                                array(integer.clone()),
                            ]
                    || count
                        && owner.role == DeclarationRole::Function
                        && self.context.files[file].implicit
                        && parameters == [array(decision_boolean.clone())])
            {
                return Err("GCC selected source identity or tuple is unsupported".into());
            }
            let written = find_node(
                self.context.files[file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("GCC written source is unavailable")?;
            if let Some(reason) = self.closed_integer_source_error(file, written, false, true) {
                return Err(reason);
            }
            let mut signatures = self
                .calls
                .signatures
                .iter()
                .filter(|signature| signature.declaration == id);
            let signature = signatures
                .next()
                .ok_or("GCC written signature is unavailable")?;
            if signatures.next().is_some()
                || signature.parameters.len() != parameters.len()
                || signature
                    .parameters
                    .iter()
                    .any(|parameter| parameter.has_default)
                || signature.return_type != result
            {
                return Err("GCC written signature or defaults are unsupported".into());
            }
            let view = instantiated_body(self.context, self.bindings, self.calls, id, parameters);
            let bodies: Vec<_> = written
                .child_nodes()
                .filter(|node| is_expression(node.kind()))
                .collect();
            let [body] = bodies.as_slice() else {
                return Err("GCC requires its complete selected body".into());
            };
            if self
                .expression_type(&view, file, body)
                .is_none_or(|value| value.ty != result)
            {
                return Err("GCC selected body type is unsupported".into());
            }
            let formals = (0..parameters.len())
                .map(|position| {
                    let formal = formal_parameter(self.context, self.bindings, id, position)
                        .ok_or("GCC formal identity is unavailable")?;
                    if view.declarations[formal.0].ty != parameters[position] {
                        return Err("GCC concrete formal type is unsupported".into());
                    }
                    Ok(formal)
                })
                .collect::<Result<Vec<_>, String>>()?;
            let members = |role| {
                let mut rows: Vec<_> = self
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
            let local_types = if count {
                vec![array(decision_boolean.clone())]
            } else {
                Vec::new()
            };
            let binder_types = if outer {
                vec![integer.clone(), integer.clone()]
            } else if native {
                vec![
                    integer.clone(),
                    decision_integer.clone(),
                    decision_integer.clone(),
                    decision_integer.clone(),
                ]
            } else {
                vec![decision_boolean.clone()]
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
                return Err("GCC local or binder types are unsupported".into());
            }
            let name = |id: DeclarationId| self.bindings.declarations[id.0].name.as_str();
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
                return Err("GCC owning names are shadowed".into());
            }
            let variable = |ty: &TypeInst, enum_only| match &ty.kind {
                TypeKind::Variable {
                    name,
                    enum_only: actual,
                    any: false,
                } if *actual == enum_only => Ok(name.clone()),
                _ => Err("GCC written generic identity is unsupported"),
            };
            let expected = if outer {
                let TypeKind::Array {
                    indices: xs_axis,
                    element,
                } = &signature.parameters[0].ty.kind
                else {
                    return Err("GCC written sequence type is unsupported".into());
                };
                let TypeKind::Array {
                    indices: cover_axis,
                    ..
                } = &signature.parameters[1].ty.kind
                else {
                    return Err("GCC written cover type is unsupported".into());
                };
                let ([xs_axis], [cover_axis]) = (xs_axis.as_slice(), cover_axis.as_slice()) else {
                    return Err("GCC written rank is unsupported".into());
                };
                let (xvar, yvar, symbol) = (
                    variable(xs_axis, false)?,
                    variable(cover_axis, false)?,
                    variable(element, true)?,
                );
                if xvar == yvar || xvar == symbol || yvar == symbol {
                    return Err("GCC written generic links are unsupported".into());
                }
                let (xs, cover, lower, upper) = (
                    name(formals[0]),
                    name(formals[1]),
                    name(formals[2]),
                    name(formals[3]),
                );
                let (l, u) = (&binders[0].name, &binders[1].name);
                format!(
                    r#"predicate global_cardinality(array[{xvar}]of var {symbol}:{xs},array[{yvar}]of {symbol}:{cover},array[{yvar}]of int:{lower},array[{yvar}]of int:{upper},)=
                    assert(index_sets_agree({cover},{lower}) /\ index_sets_agree({cover},{upper}),
                    "global_cardinality: " ++ "cover has index sets " ++ show_index_sets({cover}) ++ ", lower_bound has index sets " ++ show_index_sets({lower}) ++ ", and upper_bound has index sets " ++ show_index_sets({lower}) ++ ", but they must have identical index sets",
                    if length({xs}) == 0 then assert(forall({l} in array1d({lower}))({l} <= 0) /\ forall({u} in array1d({upper}))({u} >= 0) \/ length({cover}) == 0,
                    "global_cardinality_low_up: " ++ "lower_bound and upper_bound must allow a count of 0 when xs is empty, or also be empty", true,)
                    else fzn_global_cardinality_low_up(enum2int(array1d({xs})),enum2int(array1d({cover})),array1d({lower}),array1d({upper}),) endif,);"#
                )
            } else if native {
                let (xs, cover, lower, upper) = (
                    name(formals[0]),
                    name(formals[1]),
                    name(formals[2]),
                    name(formals[3]),
                );
                let (i, x, y, z) = (
                    &binders[0].name,
                    &binders[1].name,
                    &binders[2].name,
                    &binders[3].name,
                );
                format!("predicate fzn_global_cardinality_low_up(array[int]of var int:{xs},array[int]of int:{cover},array[int]of int:{lower},array[int]of int:{upper},)=
                    forall({i} in index_set({cover}))(if {upper}[{i}] >= length({xs}) then count({x} in {xs})({x} = {cover}[{i}]) >= {lower}[{i}]
                    elseif {lower}[{i}] <= 0 then count({y} in {xs})({y} = {cover}[{i}]) <= {upper}[{i}]
                    else count({z} in {xs})({z} = {cover}[{i}]) in {lower}[{i}]..{upper}[{i}] endif);")
            } else {
                let TypeKind::Array { indices, .. } = &signature.parameters[0].ty.kind else {
                    return Err("count written array type is unsupported".into());
                };
                let [axis] = indices.as_slice() else {
                    return Err("count written rank is unsupported".into());
                };
                let axis = variable(axis, false)?;
                let (xs, xx, y) = (name(formals[0]), &locals[0].name, &binders[0].name);
                format!("function var int:count(array[{axis}]of var bool:{xs}::promise_ctx_monotone)::promise_commutative =
                    let{{array[int]of var bool:{xx}::promise_ctx_monotone = array1d({xs});}}in sum([bool2int({y})|{y} in {xx}]);")
            };
            let expected = zincite_syntax::lex(expected);
            let significant = |kind| {
                !matches!(
                    kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                )
            };
            let parsed = &self.context.files[file].parsed;
            if !parsed
                .tokens()
                .iter()
                .filter(|token| {
                    written.range().start <= token.range.start
                        && token.range.end <= written.range().end
                        && significant(token.kind)
                })
                .map(|token| (token.kind, &parsed.source()[token.range.clone()]))
                .eq(expected
                    .tokens()
                    .iter()
                    .filter(|token| significant(token.kind))
                    .map(|token| (token.kind, &expected.source()[token.range.clone()])))
            {
                return Err("GCC complete written body or header is unsupported".into());
            }
            let mut scopes = Vec::new();
            let mut annotations = Vec::new();
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if node.kind() == NodeKind::Annotation {
                    if !count
                        || !["promise_ctx_monotone", "promise_commutative"]
                            .iter()
                            .any(|name| self.atomic_standard_metadata_safe(file, node, name))
                    {
                        return Err("GCC written metadata is unsupported".into());
                    }
                    annotations.push(node.range());
                    continue;
                }
                if node.kind() == NodeKind::Error {
                    return Err("GCC written source annotation or syntax is unsupported".into());
                }
                if is_expression(node.kind())
                    && self
                        .expression_type(&view, file, node)
                        .is_none_or(|value| !value.ty.known() || optional(&value.ty))
                {
                    return Err("GCC written expression type or optionality is unsupported".into());
                }
                if matches!(
                    node.kind(),
                    NodeKind::ArrayComprehension | NodeKind::GeneratorCallExpression
                ) {
                    let list = node
                        .child_nodes()
                        .find(|child| child.kind() == NodeKind::GeneratorList)
                        .ok_or("GCC generator list is unavailable")?;
                    for header in list.child_nodes() {
                        let binder = binders
                            .iter()
                            .find(|binder| binder.syntax_range == header.range())
                            .ok_or("GCC generator owning identity is unavailable")?;
                        scopes.push((binder.id, node.range()));
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
                        .operation_fact(&view, file, node)
                        .ok_or("GCC selected operation is unavailable")?;
                    let CallOutcome::Resolved {
                        declaration,
                        parameters: selected,
                        return_type,
                    } = &fact.outcome
                    else {
                        return Err("GCC selected operation is unsupported".into());
                    };
                    let actuals = if node.kind() == NodeKind::GeneratorCallExpression {
                        vec![
                            fact.generator_argument
                                .clone()
                                .ok_or("GCC generator collection type is unavailable")?,
                        ]
                    } else {
                        node.child_nodes()
                            .map(|child| {
                                self.expression_type(&view, file, child)
                                    .map(|value| value.ty.clone())
                                    .ok_or("GCC operand type is unavailable")
                            })
                            .collect::<Result<Vec<_>, _>>()?
                    };
                    if self
                        .expression_type(&view, file, node)
                        .is_none_or(|value| value.ty != *return_type)
                        || actuals.len() != selected.len()
                        || actuals.iter().zip(selected).any(|(actual, formal)| {
                            !actual.known()
                                || optional(actual)
                                || !crate::types::coerces(actual, formal)
                        })
                    {
                        return Err("GCC selected operation tuple is unsupported".into());
                    }
                    if fact.name == "fzn_global_cardinality_low_up" || fact.name == "count" {
                        if !(outer
                            && fact.name == "fzn_global_cardinality_low_up"
                            && *return_type == decision_boolean
                            || native
                                && fact.name == "count"
                                && *return_type == decision_integer
                                && node.kind() == NodeKind::GeneratorCallExpression)
                        {
                            return Err("GCC nested selected body is unsupported".into());
                        }
                        self.gcc_source_body(*declaration, selected, active)?;
                    } else {
                        let pair = |ty: &TypeInst| selected.as_slice() == [ty.clone(), ty.clone()];
                        let unary = |ty: &TypeInst| selected.as_slice() == std::slice::from_ref(ty);
                        let array_type = |ty: &TypeInst| {
                            ty.known()
                                && !optional(ty)
                                && matches!(&ty.kind, TypeKind::Array { indices, .. }
                                    if indices.len() == 1
                                        && indices[0].instantiation == Instantiation::Parameter
                                        && (indices[0].kind == TypeKind::Int
                                            || outer && matches!(indices[0].kind, TypeKind::Enum(_))))
                        };
                        let tuple = match fact.name.as_str() {
                            "index_sets_agree" => {
                                outer
                                    && selected.len() == 2
                                    && selected.iter().all(array_type)
                                    && *return_type == boolean
                            }
                            "show_index_sets" => {
                                outer
                                    && selected.len() == 1
                                    && array_type(&selected[0])
                                    && *return_type == string
                            }
                            "length" => {
                                selected.len() == 1
                                    && array_type(&selected[0])
                                    && *return_type == integer
                            }
                            "array1d" => {
                                selected.len() == 1 && array_type(&selected[0]) && {
                                    let mut flattened = selected[0].clone();
                                    if let TypeKind::Array { indices, .. } = &mut flattened.kind {
                                        *indices = vec![integer.clone()];
                                    }
                                    flattened == *return_type
                                }
                            }
                            "enum2int" => {
                                outer && selected.len() == 1 && array_type(&selected[0]) && {
                                    let mut converted = selected[0].clone();
                                    if let TypeKind::Array { element, .. } = &mut converted.kind {
                                        element.kind = TypeKind::Int;
                                    }
                                    converted == *return_type
                                }
                            }
                            "index_set" => {
                                native && unary(&array(integer.clone())) && *return_type == set
                            }
                            "forall" => {
                                (outer && unary(&array(boolean.clone())) && *return_type == boolean)
                                    || native
                                        && unary(&array(decision_boolean.clone()))
                                        && *return_type == decision_boolean
                            }
                            "assert" => {
                                outer
                                    && selected.as_slice()
                                        == [boolean.clone(), string.clone(), return_type.clone()]
                                    && matches!(*return_type, ref ty if *ty == boolean || *ty == decision_boolean)
                            }
                            "++" => outer && pair(&string) && *return_type == string,
                            "/\\" | "\\/" => outer && pair(&boolean) && *return_type == boolean,
                            "=" | "<=" | ">=" => {
                                pair(&integer) && *return_type == boolean
                                    || pair(&decision_integer) && *return_type == decision_boolean
                                    || native
                                        && selected.as_slice()
                                            == [decision_integer.clone(), integer.clone()]
                                        && *return_type == decision_boolean
                            }
                            ".." => native && pair(&integer) && *return_type == set,
                            "in" => {
                                native
                                    && selected.as_slice()
                                        == [decision_integer.clone(), set.clone()]
                                    && *return_type == decision_boolean
                            }
                            "sum" => {
                                count
                                    && unary(&array(decision_integer.clone()))
                                    && *return_type == decision_integer
                            }
                            "bool2int" => {
                                count
                                    && unary(&decision_boolean)
                                    && *return_type == decision_integer
                            }
                            _ => false,
                        };
                        if !tuple {
                            return Err("GCC primitive exact tuple is unsupported".into());
                        }
                        if fact.name == "index_sets_agree" {
                            let primitive = &self.bindings.declarations[declaration.0];
                            let written = find_node(
                                self.context.files[primitive.file].parsed.tree(),
                                &primitive.syntax_range,
                                primitive.role,
                            )
                            .ok_or("GCC index agreement declaration is unavailable")?;
                            if primitive.role != DeclarationRole::Test
                                || self.context.files[primitive.file].kind
                                    != SourceKind::StandardLibrary
                                || !self.context.files[primitive.file].implicit
                                || !crate::optional::core_optional_call(
                                    self.context,
                                    self.bindings,
                                    &view,
                                    file,
                                    node,
                                    "index_sets_agree",
                                )
                                || !self.prefix_primitive_source_safe(primitive.file, written)
                            {
                                return Err(
                                    "GCC index agreement native declaration is unsupported".into(),
                                );
                            }
                        } else if !self.prefix_primitive(
                            file,
                            node,
                            &view,
                            &fact.name,
                            selected,
                            return_type,
                        ) {
                            return Err("GCC primitive written declaration is unsupported".into());
                        }
                    }
                }
                nodes.extend(node.child_nodes());
            }
            if scopes.len() != binders.len() {
                return Err("GCC complete generator scopes are unsupported".into());
            }
            for (position, (binder, scope)) in scopes.iter().enumerate() {
                if scopes[..position].iter().any(|(other, range)| {
                    name(*binder) == name(*other)
                        && scope.start < range.end
                        && range.start < scope.end
                }) {
                    return Err("GCC nested generator names are shadowed".into());
                }
            }
            for reference in self.bindings.references.iter().filter(|reference| {
                reference.file == file
                    && reference.kind == ReferenceKind::Value
                    && body.range().start <= reference.location.range.start
                    && reference.location.range.end <= body.range().end
            }) {
                if annotations.iter().any(|range| {
                    range.start <= reference.location.range.start
                        && reference.location.range.end <= range.end
                }) {
                    continue;
                }
                let BindingResolution::Resolved(id) = reference.resolution else {
                    return Err("GCC source reference is unresolved".into());
                };
                if formals.contains(&id) || locals.iter().any(|local| local.id == id) {
                    continue;
                }
                if !scopes.iter().any(|(binder, range)| {
                    *binder == id
                        && range.start <= reference.location.range.start
                        && reference.location.range.end <= range.end
                }) {
                    return Err("GCC source reference has no owning scope".into());
                }
            }
            Ok(())
        })();
        active.pop();
        checked
    }
}
