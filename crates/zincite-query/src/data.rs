//! Model-backed data candidates. Expressions and bindings execute in Zirql.
use crate::{Input, Limits, QueryError, Work, comprehension as c, pipeline, structured};
use std::{collections::BTreeMap, ops::Range};
use zincite_lint::{DeclarationRole, EditPart, ModelFacts, TextEdit, TypeKind};
use zincite_syntax::{FileMode, NodeKind as N, TokenKind as T};
use zirql::{
    Budget, ConstructedValue as V, Located, ValueExpression as E, ValueExpressionKind as K,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditKind {
    Create,
    Replace,
    ScaleEnum,
}

#[derive(Debug)]
pub(crate) struct Edit {
    pub kind: EditKind,
    pub name: Located<String>,
    pub source: String,
    pub range: Range<usize>,
    pub names: Vec<Located<String>>,
}

pub(crate) fn charge(
    budget: &mut Budget,
    n: usize,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    budget
        .charge(n, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}

/// Establish targets, evaluate once against original bindings, validate one
/// coherent final document, and return its reparsed bytes without writing files.
pub(crate) fn prepare_data_batch<'a>(
    input: &Input,
    facts: &ModelFacts<'a>,
    edits: &[Edit],
    copies: &[(
        &Located<String>,
        std::rc::Rc<crate::program::SourceCopies<'a>>,
    )],
    budget: &mut Budget,
    limits: Limits,
    mut evaluate: impl FnMut(
        &c::LoweredValue,
        &[Located<String>],
        &mut Budget,
        &Range<usize>,
    ) -> Result<V, QueryError>,
) -> Result<Input, QueryError> {
    let ranges = || {
        edits
            .iter()
            .map(|edit| &edit.range)
            .chain(copies.iter().map(|(target, _)| &target.range))
    };
    let range = ranges().map(|range| range.start).min().unwrap()
        ..ranges().map(|range| range.end).max().unwrap();
    if input.mode != FileMode::Data {
        return Err(QueryError::query(
            range,
            "data construction requires data-file mode (.dzn)",
        ));
    }
    charge(
        budget,
        edits
            .len()
            .checked_mul(5)
            .ok_or_else(|| QueryError::query(range.clone(), "edit allocation overflow"))?,
        &range,
    )?;
    budget
        .charge_results(edits.len(), zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    let items = crate::collect_items(
        input,
        limits.collection,
        &mut Work { budget },
        &range,
        false,
    )?;
    let mut scales = Vec::new();
    let mut constructed = Vec::new();
    for edit in edits {
        if edit.kind == EditKind::ScaleEnum {
            let lowered = c::read_data(&edit.source, edit.range.clone(), budget, limits.nesting)?;
            let factor = evaluate(&lowered, &edit.names, budget, &edit.range)?;
            scales.push(crate::program::source_copy::prepare_scale(
                input, facts, edit, &factor, budget,
            )?);
        } else {
            constructed.push(edit);
        }
    }
    let edits = constructed.as_slice();
    let mut copy_edits = crate::program::source_copy::prepare_appends(
        input, facts, copies, edits, &scales, budget, limits, &range,
    )?;
    let mut targets = Vec::with_capacity(edits.len());
    for edit in edits {
        let owner = c::unique_declaration(
            facts,
            &edit.name.value,
            DeclarationRole::Value,
            budget,
            &edit.name.range,
        )?;
        let metadata =
            c::declared_value(facts, &edit.name.value, budget, &edit.name.range)?.unwrap();
        validate_declared_type(metadata.ty, budget, &edit.name.range, limits.nesting)?;
        let node =
            c::declaration_node(facts, owner.id, budget, &edit.name.range)?.ok_or_else(|| {
                QueryError::query(
                    edit.name.range.clone(),
                    "target has no retained model declaration",
                )
            })?;
        if node
            .child_nodes()
            .filter(|child| child.kind() != N::Annotation)
            .count()
            != 1
        {
            return Err(QueryError::query(
                edit.name.range.clone(),
                "target model declaration already has a value",
            ));
        }
        let mut matching = items
            .iter()
            .filter(|item| item.name == Some(edit.name.value.as_str()));
        let assignment = matching.next();
        if matching.next().is_some()
            || (edit.kind == EditKind::Create && assignment.is_some())
            || (edit.kind == EditKind::Replace && assignment.is_none())
        {
            return Err(QueryError::query(
                edit.name.range.clone(),
                if edit.kind == EditKind::Create {
                    "create requires an absent assignment"
                } else {
                    "replace requires exactly one original assignment"
                },
            ));
        }
        if let Some(item) = assignment {
            let rhs = crate::assignment_rhs(item.node).unwrap();
            let rhs_range = input.original_range(rhs.range());
            for (index, token) in input.syntax().tokens().iter().enumerate() {
                charge(budget, 1, &edit.range)?;
                if token.range.start + input.syntax_offset() >= rhs_range.end {
                    break;
                }
                if token.range.end + input.syntax_offset() > rhs_range.start
                    && matches!(token.kind, T::LineComment | T::BlockComment)
                {
                    return Err(QueryError::input(
                        input.token_range(index),
                        "cannot replace a right-hand side with internal comments without choosing their new placement",
                    ));
                }
            }
        }
        targets.push((owner.id, metadata, assignment));
    }
    // Scalar edits supply final axis obligations only. Sources and lets remain
    // bound to the original input, irrespective of request order.
    let mut scalars = BTreeMap::new();
    let mut values: Vec<Option<V>> = (0..edits.len()).map(|_| None).collect();
    for (i, edit) in edits.iter().enumerate() {
        if targets[i].1.ty.kind == TypeKind::Int {
            let lowered = c::read_data(&edit.source, edit.range.clone(), budget, limits.nesting)?;
            let value = evaluate(&lowered, &edit.names, budget, &edit.range)?;
            if let V::Integer(n) = value.data() {
                scalars.insert(targets[i].0.0, *n);
            }
            values[i] = Some(value);
        }
    }
    for (i, edit) in edits.iter().enumerate() {
        if values[i].is_none() {
            let mut lowered =
                c::read_data(&edit.source, edit.range.clone(), budget, limits.nesting)?;
            supply_empty_axes(
                &mut lowered.expression,
                targets[i].1,
                input,
                &scalars,
                facts,
                budget,
            )?;
            values[i] = Some(evaluate(&lowered, &edit.names, budget, &edit.range)?);
        }
    }
    let mut text_edits = Vec::with_capacity(edits.len());
    let mut created = String::new();
    let ending = if input.source_bytes().windows(2).any(|w| w == b"\r\n") {
        "\r\n"
    } else if input.source_bytes().contains(&b'\r') {
        "\r"
    } else {
        "\n"
    };
    for (i, edit) in edits.iter().enumerate() {
        let value = values[i].as_ref().unwrap();
        validate_target(
            value,
            targets[i].1,
            input,
            &scalars,
            facts,
            budget,
            &edit.range,
        )?;
        let mut text = String::new();
        serialize(value, facts, budget, &edit.range, &mut text)?;
        if let Some(item) = targets[i].2 {
            text_edits.push(TextEdit {
                range: input.original_range(crate::assignment_rhs(item.node).unwrap().range()),
                replacement: vec![EditPart::Text(text)],
            });
        } else {
            append(
                &mut created,
                &identifier(&edit.name.value, budget, &edit.name.range)?,
                budget,
                &edit.range,
            )?;
            append(&mut created, " = ", budget, &edit.range)?;
            append(&mut created, &text, budget, &edit.range)?;
            append(&mut created, ";", budget, &edit.range)?;
            append(&mut created, ending, budget, &edit.range)?;
        }
    }
    if !created.is_empty() {
        let position = items
            .last()
            .map_or(input.syntax_offset(), |item| item.source_range.end);
        protect_insertion(input, position, budget, &range)?;
        if position > input.syntax_offset()
            && !matches!(input.source_bytes()[position - 1], b'\n' | b'\r')
        {
            charge(budget, created.len() + ending.len(), &range)?;
            created.insert_str(0, ending);
        }
        text_edits.push(TextEdit {
            range: position..position,
            replacement: vec![EditPart::Text(created)],
        });
    }
    text_edits.append(&mut copy_edits);
    for scale in &scales {
        if let Some(edit) = &scale.edit {
            text_edits.push(edit.clone());
        }
    }
    let length = text_edits
        .iter()
        .try_fold(input.source_bytes().len(), |len, edit| {
            let added = edit
                .replacement
                .iter()
                .try_fold(0usize, |sum, part| match part {
                    EditPart::Text(s) => sum.checked_add(s.len()),
                    EditPart::Original(r) => sum.checked_add(r.len()),
                })?;
            len.checked_sub(edit.range.len())?.checked_add(added)
        })
        .ok_or_else(|| QueryError::query(range.clone(), "candidate byte size overflow"))?;
    // Admission covers edited-byte assembly and the complete candidate CST parse.
    charge(
        budget,
        length
            .checked_mul(3)
            .ok_or_else(|| QueryError::query(range.clone(), "candidate byte size overflow"))?,
        &range,
    )?;
    if !copies.is_empty() {
        // Copy sources can lie outside the target assignment. A single owning
        // edit retains every untouched span and contains every original slice.
        text_edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
        let mut parts = Vec::new();
        let mut cursor = 0;
        for edit in text_edits {
            if edit.range.start < cursor {
                return Err(QueryError::query(range.clone(), "overlapping data edits"));
            }
            charge(budget, edit.replacement.len() + 1, &range)?;
            budget
                .charge_results(
                    edit.replacement.len() + 1,
                    zirql::Location::Query(range.clone()),
                )
                .map_err(pipeline::core_error)?;
            parts.push(EditPart::Original(cursor..edit.range.start));
            parts.extend(edit.replacement);
            cursor = edit.range.end;
        }
        charge(budget, 2, &range)?;
        parts.push(EditPart::Original(cursor..input.source_bytes().len()));
        text_edits = vec![TextEdit {
            range: 0..input.source_bytes().len(),
            replacement: parts,
        }];
    }
    let candidate = crate::finish_candidate(input, &text_edits, &[], &mut Work { budget }, &range)?;
    validate_affected_assignments(input, &candidate, facts, edits, &scalars, budget, limits)?;
    crate::program::source_copy::validate_candidate(
        &candidate, facts, copies, &scales, budget, limits, &range,
    )?;
    Ok(candidate)
}

fn validate_declared_type(
    ty: &zincite_lint::TypeInst,
    budget: &mut Budget,
    range: &Range<usize>,
    remaining: usize,
) -> Result<(), QueryError> {
    charge(budget, 1, range)?;
    if remaining == 0 || ty.optional || ty.instantiation != zincite_lint::Instantiation::Parameter {
        return Err(QueryError::query(
            range.clone(),
            "target requires a bounded fixed nonoptional supported type",
        ));
    }
    match &ty.kind {
        TypeKind::Bool | TypeKind::Int | TypeKind::Float | TypeKind::String | TypeKind::Enum(_) => {
            Ok(())
        }
        TypeKind::Set(element) => {
            if !matches!(
                element.kind,
                TypeKind::Bool | TypeKind::Int | TypeKind::Float | TypeKind::Enum(_)
            ) {
                return Err(QueryError::query(
                    range.clone(),
                    "target set element type is unsupported",
                ));
            }
            validate_declared_type(element, budget, range, remaining - 1)
        }
        TypeKind::Array { indices, element } => {
            for index in indices {
                if !matches!(index.kind, TypeKind::Int | TypeKind::Enum(_)) {
                    return Err(QueryError::query(
                        range.clone(),
                        "target index type is unsupported",
                    ));
                }
                validate_declared_type(index, budget, range, remaining - 1)?;
            }
            validate_declared_type(element, budget, range, remaining - 1)
        }
        TypeKind::Tuple(types) => {
            for ty in types {
                validate_declared_type(ty, budget, range, remaining - 1)?;
            }
            Ok(())
        }
        TypeKind::Record(types) => {
            for (_, ty) in types {
                validate_declared_type(ty, budget, range, remaining - 1)?;
            }
            Ok(())
        }
        _ => Err(QueryError::query(
            range.clone(),
            "target declaration type is unsupported",
        )),
    }
}

pub(crate) fn record_metadata<'a>(
    metadata: c::LiteralMetadata<'a>,
    label: &str,
    ty: &'a zincite_lint::TypeInst,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<c::LiteralMetadata<'a>, QueryError> {
    let mut written = None;
    if let Some(record) = metadata.node {
        let parsed = &facts.model().files[metadata.file].parsed;
        for field in record.child_nodes() {
            charge(budget, 1 + field.children().len(), range)?;
            for element in field.children() {
                if let zincite_syntax::SyntaxElement::Token(i) = element {
                    let token = &parsed.tokens()[*i];
                    if matches!(token.kind, T::Identifier | T::QuotedIdentifier)
                        && crate::identifier_identity(&parsed.source()[token.range.clone()])
                            == label
                    {
                        written = field.child_nodes().next();
                    }
                }
            }
        }
    }
    Ok(c::LiteralMetadata {
        ty,
        file: metadata.file,
        node: written,
    })
}

/// Only a value directly governed by a declared target receives missing empty
/// evidence. Intermediate expressions retain the shared evaluator's policy.
fn supply_empty_axes(
    expression: &mut E,
    metadata: c::LiteralMetadata<'_>,
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
) -> Result<(), QueryError> {
    let range = expression.range.clone();
    charge(budget, 1, &range)?;
    let metadata = c::resolve_metadata(metadata, facts, budget, &range, 64)?;
    match (&mut expression.value, &metadata.ty.kind) {
        (
            K::Conditional {
                branches,
                otherwise,
            },
            _,
        ) => {
            for (_, value) in branches {
                supply_empty_axes(value, metadata, input, scalars, facts, budget)?;
            }
            supply_empty_axes(otherwise, metadata, input, scalars, facts, budget)?;
        }
        (K::Record(fields), TypeKind::Record(types)) => {
            for (name, value) in fields {
                if let Some((_, ty)) = types.iter().find(|(label, _)| label == &name.value) {
                    supply_empty_axes(
                        value,
                        record_metadata(metadata, &name.value, ty, facts, budget, &range)?,
                        input,
                        scalars,
                        facts,
                        budget,
                    )?;
                }
            }
        }
        (K::Tuple(values), TypeKind::Tuple(types)) => {
            for (i, (value, ty)) in values.iter_mut().zip(types).enumerate() {
                supply_empty_axes(
                    value,
                    c::child_metadata(Some(metadata), ty, Some(i)),
                    input,
                    scalars,
                    facts,
                    budget,
                )?;
            }
        }
        (value, TypeKind::Array { element, indices }) => {
            let child = c::child_metadata(Some(metadata), element, None);
            match value {
                K::KeyedComprehension {
                    head,
                    declared_axis_sources,
                    empty_axis_sources,
                    ..
                } => {
                    supply_empty_axes(head, child, input, scalars, facts, budget)?;
                    if declared_axis_sources.is_none()
                        && let Some(axes) = c::declared_axes(
                            input,
                            scalars,
                            Some(metadata),
                            Some(facts),
                            budget,
                            &range,
                        )?
                    {
                        charge(budget, axes.len(), &range)?;
                        budget
                            .charge_results(axes.len(), zirql::Location::Query(range.clone()))
                            .map_err(pipeline::core_error)?;
                        let mut sources = Vec::with_capacity(axes.len());
                        for axis in axes {
                            charge(budget, axis.keys.len(), &range)?;
                            budget
                                .charge_results(
                                    axis.keys.len(),
                                    zirql::Location::Query(range.clone()),
                                )
                                .map_err(pipeline::core_error)?;
                            let element_type = Some(match axis.key_type {
                                zirql::ValueKeyType::Integer => zirql::SetElementType::Integer,
                                zirql::ValueKeyType::Member(id) => {
                                    zirql::SetElementType::Member(id)
                                }
                            });
                            let elements = axis
                                .keys
                                .into_iter()
                                .map(|key| match key {
                                    zirql::ValueKey::Integer(n) => V::Integer(n),
                                    zirql::ValueKey::Member(m) => V::Member(m),
                                })
                                .collect();
                            sources.push(Located::new(
                                K::Literal(V::Set {
                                    element_type,
                                    elements,
                                }),
                                range.clone(),
                            ));
                        }
                        *empty_axis_sources = Some(sources);
                    }
                }
                K::KeyedArray {
                    axes,
                    declared_axis_sources,
                    entries,
                } => {
                    for (_, value) in entries.iter_mut() {
                        supply_empty_axes(value, child, input, scalars, facts, budget)?;
                    }
                    if entries.is_empty() && axes.is_none() && declared_axis_sources.is_none() {
                        *axes = c::declared_axes(
                            input,
                            scalars,
                            Some(metadata),
                            Some(facts),
                            budget,
                            &range,
                        )?;
                    }
                }
                K::Comprehension { head, .. } => {
                    supply_empty_axes(head, child, input, scalars, facts, budget)?
                }
                K::Array(values) => {
                    for value in values.iter_mut() {
                        supply_empty_axes(value, child, input, scalars, facts, budget)?;
                    }
                    if values.is_empty() {
                        let axes = c::declared_axes(
                            input,
                            scalars,
                            Some(metadata),
                            Some(facts),
                            budget,
                            &range,
                        )?;
                        if axes.as_ref().is_some_and(|axes| {
                            (indices.len() > 1 || axes[0].key_type != zirql::ValueKeyType::Integer)
                                && axes.iter().any(|axis| axis.keys.is_empty())
                        }) {
                            *value = K::KeyedArray {
                                axes,
                                declared_axis_sources: None,
                                entries: Vec::new(),
                            };
                        }
                    }
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_target(
    value: &V,
    metadata: c::LiteralMetadata<'_>,
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    charge(budget, 1, range)?;
    let metadata = c::resolve_metadata(metadata, facts, budget, range, 64)?;
    let ty = metadata.ty;
    if ty.optional || ty.instantiation != zincite_lint::Instantiation::Parameter {
        return Err(QueryError::query(
            range.clone(),
            "target requires a fixed nonoptional supported type",
        ));
    }
    let fail = || {
        QueryError::query(
            range.clone(),
            "constructed value does not match its declared target type or axes",
        )
    };
    match (value.data(), &ty.kind) {
        (V::Integer(_), TypeKind::Int) | (V::Float(_), TypeKind::Float) => {
            validate_numeric_domain(value, metadata, input, scalars, facts, budget, range)
        }
        (V::Boolean(_), TypeKind::Bool) | (V::String(_), TypeKind::String) => Ok(()),
        (V::Member(member), TypeKind::Enum(id)) if member.type_id.0 == id.0 as u64 => Ok(()),
        (
            V::Set {
                elements,
                element_type,
            },
            TypeKind::Set(element),
        ) => {
            let expected = match element.kind {
                TypeKind::Bool => zirql::SetElementType::Boolean,
                TypeKind::Int => zirql::SetElementType::Integer,
                TypeKind::Float => zirql::SetElementType::Float,
                TypeKind::Enum(id) => {
                    zirql::SetElementType::Member(zirql::NominalTypeId(id.0 as u64))
                }
                _ => return Err(fail()),
            };
            if element_type.is_some_and(|actual| actual != expected) {
                return Err(fail());
            }
            for child in elements {
                validate_target(
                    child,
                    c::child_metadata(Some(metadata), element, None),
                    input,
                    scalars,
                    facts,
                    budget,
                    range,
                )?;
            }
            Ok(())
        }
        (V::Tuple(values), TypeKind::Tuple(types)) if values.len() == types.len() => {
            for (i, (child, ty)) in values.iter().zip(types).enumerate() {
                validate_target(
                    child,
                    c::child_metadata(Some(metadata), ty, Some(i)),
                    input,
                    scalars,
                    facts,
                    budget,
                    range,
                )?;
            }
            Ok(())
        }
        (V::Record(fields), TypeKind::Record(types)) if fields.len() == types.len() => {
            for (label, child) in fields {
                let (_, ty) = types
                    .iter()
                    .find(|(expected, _)| expected == label)
                    .ok_or_else(fail)?;
                validate_target(
                    child,
                    record_metadata(metadata, label, ty, facts, budget, range)?,
                    input,
                    scalars,
                    facts,
                    budget,
                    range,
                )?;
            }
            Ok(())
        }
        (V::Array(values), TypeKind::Array { indices, element }) if indices.len() == 1 => {
            if let Some(axes) =
                c::declared_axes(input, scalars, Some(metadata), Some(facts), budget, range)?
            {
                charge(budget, axes[0].keys.len(), range)?;
                if axes[0].key_type != zirql::ValueKeyType::Integer || axes[0].keys.len() != values.len() || axes[0].keys.iter().enumerate().any(|(i, key)| !matches!(key, zirql::ValueKey::Integer(n) if i64::try_from(i + 1).ok() == Some(*n))) { return Err(fail()); }
            }
            for child in values {
                validate_target(
                    child,
                    c::child_metadata(Some(metadata), element, None),
                    input,
                    scalars,
                    facts,
                    budget,
                    range,
                )?;
            }
            Ok(())
        }
        (V::KeyedArray { axes, entries }, TypeKind::Array { indices, element })
            if axes.len() == indices.len() =>
        {
            if let Some(expected) =
                c::declared_axes(input, scalars, Some(metadata), Some(facts), budget, range)?
            {
                charge(
                    budget,
                    expected.iter().map(|axis| axis.keys.len() + 1).sum(),
                    range,
                )?;
                if *axes != expected {
                    return Err(fail());
                }
            } else if axes[0].key_type != zirql::ValueKeyType::Integer {
                return Err(fail());
            }
            c::validate_data_shape(value, budget, range)?;
            for entry in entries {
                validate_target(
                    &entry.value,
                    c::child_metadata(Some(metadata), element, None),
                    input,
                    scalars,
                    facts,
                    budget,
                    range,
                )?;
            }
            Ok(())
        }
        _ => Err(fail()),
    }
}

/// Written scalar ranges/sets are value obligations too. Resolve only their
/// fixed integer scalar names; shared arithmetic and finite containers stay bounded.
pub(crate) fn validate_numeric_domain(
    value: &V,
    metadata: c::LiteralMetadata<'_>,
    input: &Input,
    scalars: &BTreeMap<usize, i64>,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    let Some(node) = metadata.node.filter(|node| node.kind() == N::DomainType) else {
        return Ok(());
    };
    let expression = node
        .child_nodes()
        .next()
        .ok_or_else(|| QueryError::query(range.clone(), "written scalar domain is unavailable"))?;
    let file = &facts.model().files[metadata.file];
    let location = file.location(expression.range());
    let mut projected = Vec::new();
    for reference in &facts.bindings().references {
        charge(budget, 1, range)?;
        if reference.file == metadata.file
            && location.range.start <= reference.location.range.start
            && reference.location.range.end <= location.range.end
            && reference.kind == zincite_lint::ReferenceKind::Value
        {
            let zincite_lint::BindingResolution::Resolved(id) = reference.resolution else {
                return Err(QueryError {
                    location: crate::ErrorLocation::Model(reference.location.clone()),
                    range: reference.location.range.clone(),
                    message: "scalar domain dependency identity is unavailable".into(),
                });
            };
            let name = &facts.bindings().declarations[id.0].name;
            if projected
                .iter()
                .any(|(bound, _): &(Located<String>, V)| bound.value == *name)
            {
                continue;
            }
            let n = c::integer_bound(
                &zincite_lint::NumericBound::Symbol(id),
                input,
                scalars,
                facts,
                budget,
                range,
            )?;
            charge(budget, name.len() + 1, range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            projected.push((Located::new(name.clone(), range.clone()), V::Integer(n)));
        }
    }
    let domain_error = |mut error: QueryError| {
        if matches!(error.location, crate::ErrorLocation::Query) {
            error.location = crate::ErrorLocation::Model(file.location(
                error.range.start.saturating_sub(file.byte_offset)
                    ..error.range.end.saturating_sub(file.byte_offset),
            ));
        }
        error
    };
    let domain = c::read_data(
        &file.parsed.source()[expression.range()],
        location.range.clone(),
        budget,
        64,
    )
    .map_err(domain_error)?;
    fn check_bound(expression: &E, budget: &mut Budget) -> Result<(), QueryError> {
        charge(budget, 1, &expression.range)?;
        match &expression.value {
            K::Literal(V::Integer(_)) | K::Name(_) => Ok(()),
            K::Unary {
                operator: zirql::ValueUnaryOperator::Positive | zirql::ValueUnaryOperator::Negative,
                operand,
            } => check_bound(operand, budget),
            K::Binary {
                operator:
                    zirql::ValueBinaryOperator::Add
                    | zirql::ValueBinaryOperator::Subtract
                    | zirql::ValueBinaryOperator::Multiply
                    | zirql::ValueBinaryOperator::IntegerDivide
                    | zirql::ValueBinaryOperator::Modulo,
                left,
                right,
            } => {
                check_bound(left, budget)?;
                check_bound(right, budget)
            }
            _ => Err(QueryError::query(
                expression.range.clone(),
                "scalar domain requires supported integer bounds",
            )),
        }
    }
    match &domain.expression.value {
        K::Range { lower, upper } => {
            check_bound(lower, budget).map_err(domain_error)?;
            check_bound(upper, budget).map_err(domain_error)?;
        }
        K::Set(values) => {
            for value in values {
                check_bound(value, budget).map_err(domain_error)?;
            }
        }
        _ => {
            return Err(QueryError::query(
                range.clone(),
                "numeric scalar domain must be a supported finite integer interval or set",
            ));
        }
    }
    let bindings = zirql::Bindings::new(projected).map_err(pipeline::core_error)?;
    let domain = zirql::evaluate_value(&domain.expression, &bindings, budget)
        .map_err(pipeline::core_error)
        .map_err(domain_error)?;
    let values = match domain {
        V::Array(values)
        | V::Set {
            elements: values, ..
        } => values,
        _ => unreachable!("validated interval/set constructor"),
    };
    charge(budget, values.len(), range)?;
    if !values.iter().any(|allowed| allowed == value.data()) {
        return Err(QueryError::query(
            range.clone(),
            "constructed scalar is outside its declared domain",
        ));
    }
    Ok(())
}

fn validate_affected_assignments(
    original: &Input,
    candidate: &Input,
    facts: &ModelFacts<'_>,
    edits: &[&Edit],
    scalars: &BTreeMap<usize, i64>,
    budget: &mut Budget,
    limits: Limits,
) -> Result<(), QueryError> {
    if scalars.is_empty() {
        return Ok(());
    }
    for node in original.syntax().tree().child_nodes() {
        charge(budget, 1 + node.children().len(), &edits[0].range)?;
        if node.kind() != N::Assignment {
            continue;
        }
        let token = structured::direct_token(original, node).unwrap();
        let name =
            crate::identifier_identity(std::str::from_utf8(original.token_bytes(token)).unwrap());
        if edits.iter().any(|edit| edit.name.value == name) {
            continue;
        }
        let mut type_nodes = Vec::new();
        for declaration in &facts.bindings().declarations {
            charge(budget, 1, &edits[0].range)?;
            if declaration.top_level
                && declaration.role == DeclarationRole::Value
                && declaration.name == name
                && let Some(type_node) =
                    c::declaration_node(facts, declaration.id, budget, &edits[0].range)?.and_then(
                        |node| node.child_nodes().find(|node| node.kind() != N::Annotation),
                    )
            {
                let metadata = c::LiteralMetadata {
                    ty: c::declaration_type(facts, declaration.id, budget, &edits[0].range)?,
                    file: declaration.file,
                    node: Some(type_node),
                };
                let metadata =
                    c::resolve_metadata(metadata, facts, budget, &edits[0].range, limits.nesting)?;
                if let Some(node) = metadata.node {
                    charge(budget, 1, &edits[0].range)?;
                    type_nodes.push((
                        metadata.file,
                        facts.model().files[metadata.file]
                            .location(node.range())
                            .range,
                    ));
                }
            }
        }
        let mut affected = false;
        for reference in &facts.bindings().references {
            charge(budget, 1, &edits[0].range)?;
            if type_nodes.iter().any(|(file, type_range)| {
                reference.file == *file
                    && type_range.start <= reference.location.range.start
                    && reference.location.range.end <= type_range.end
            }) && let zincite_lint::BindingResolution::Resolved(id) = reference.resolution
                && scalars.contains_key(&id.0)
            {
                affected = true;
            }
        }
        if affected {
            let metadata = c::declared_value(facts, name, budget, &edits[0].range)?.unwrap();
            let source_range =
                original.original_range(crate::assignment_rhs(node).unwrap().range());
            let value =
                c::literal_source(candidate, name, Some(facts), budget, &source_range, limits)
                    .map_err(|mut error| {
                        if matches!(error.location, crate::ErrorLocation::Input) {
                            error.range = source_range.clone();
                        }
                        error
                    })?;
            validate_target(
                &value,
                metadata,
                original,
                scalars,
                facts,
                budget,
                &source_range,
            )
            .map_err(|mut error| {
                error.location = crate::ErrorLocation::Input;
                error
            })?;
        }
    }
    Ok(())
}

fn protect_insertion(
    input: &Input,
    position: usize,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    for (i, token) in input.syntax().tokens().iter().enumerate() {
        charge(budget, 1, range)?;
        if token.range.end + input.syntax_offset() > position {
            break;
        }
        if matches!(
            crate::directive(input.token_bytes(i)),
            Some(crate::Directive::Next)
        ) && crate::standalone(
            input.source_bytes(),
            token.range.start + input.syntax_offset(),
            input.syntax_offset(),
        ) {
            let target = input
                .syntax()
                .tree()
                .child_nodes()
                .find(|node| node.range().start >= token.range.end);
            if target.is_none_or(|node| node.range().start + input.syntax_offset() >= position) {
                return Err(QueryError::input(
                    input.token_range(i),
                    "creation would attach a next-item directive to a different item",
                ));
            }
        }
    }
    Ok(())
}

fn append(
    output: &mut String,
    text: &str,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    let len = output
        .len()
        .checked_add(text.len())
        .ok_or_else(|| QueryError::query(range.clone(), "serialized byte size overflow"))?;
    if len > isize::MAX as usize {
        return Err(QueryError::query(
            range.clone(),
            "serialized byte size overflow",
        ));
    }
    charge(budget, text.len(), range)?;
    output.push_str(text);
    Ok(())
}
fn identifier(name: &str, budget: &mut Budget, range: &Range<usize>) -> Result<String, QueryError> {
    charge(budget, name.len() + 2, range)?;
    let plain = !name.is_empty()
        && name.as_bytes()[0].is_ascii_alphabetic()
        && name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
    let spelling = if plain {
        name.to_owned()
    } else if !name.is_empty() && !name.contains(['\'', '\n', '\r', '\0']) {
        format!("'{name}'")
    } else {
        return Err(QueryError::query(
            range.clone(),
            "invalid generated identifier spelling",
        ));
    };
    let lexed = zincite_syntax::lex(&spelling);
    if lexed.tokens().len() != 1
        || !matches!(lexed.tokens()[0].kind, T::Identifier | T::QuotedIdentifier)
    {
        if !name.contains(['\'', '\n', '\r', '\0']) {
            return Ok(format!("'{name}'"));
        }
        return Err(QueryError::query(
            range.clone(),
            "invalid generated identifier spelling",
        ));
    }
    Ok(spelling)
}
fn member_spelling<'a>(
    member: &zirql::NominalMember,
    facts: &'a ModelFacts<'_>,
    range: &Range<usize>,
) -> Result<&'a str, QueryError> {
    let declaration = facts
        .bindings()
        .declarations
        .get(
            usize::try_from(member.member_id.0)
                .map_err(|_| QueryError::query(range.clone(), "member identity unavailable"))?,
        )
        .ok_or_else(|| QueryError::query(range.clone(), "member identity unavailable"))?;
    let file = &facts.model().files[declaration.file];
    let spelling = &file.parsed.source()[declaration.location.range.start - file.byte_offset
        ..declaration.location.range.end - file.byte_offset];
    Ok(spelling)
}
fn serialize_key(
    key: &zirql::ValueKey,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
    output: &mut String,
) -> Result<(), QueryError> {
    match key {
        zirql::ValueKey::Integer(n) => {
            charge(budget, 20, range)?;
            append(output, &n.to_string(), budget, range)
        }
        zirql::ValueKey::Member(member) => append(
            output,
            member_spelling(member, facts, range)?,
            budget,
            range,
        ),
    }
}
fn serialize(
    value: &V,
    facts: &ModelFacts<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
    output: &mut String,
) -> Result<(), QueryError> {
    charge(budget, 1, range)?;
    match value.data() {
        V::Declared { .. } => unreachable!("transparent annotation"),
        V::Absent => Err(QueryError::query(
            range.clone(),
            "absence and optional values are unavailable in data edits",
        )),
        V::Boolean(v) => append(output, if *v { "true" } else { "false" }, budget, range),
        V::Integer(v) => {
            charge(budget, 20, range)?;
            append(output, &v.to_string(), budget, range)
        }
        V::Float(v) => {
            charge(budget, 328, range)?;
            let text = v.to_string();
            append(output, &text, budget, range)?;
            if !text.contains(['.', 'e', 'E']) {
                append(output, ".0", budget, range)?;
            }
            Ok(())
        }
        V::String(v) => {
            append(output, "\"", budget, range)?;
            for ch in v.chars() {
                match ch {
                    '"' => append(output, "\\\"", budget, range)?,
                    '\\' => append(output, "\\\\", budget, range)?,
                    '\n' => append(output, "\\n", budget, range)?,
                    '\r' => append(output, "\\r", budget, range)?,
                    '\t' => append(output, "\\t", budget, range)?,
                    ch if ch.is_control() => {
                        return Err(QueryError::query(
                            range.clone(),
                            "string contains a control character without supported MiniZinc spelling",
                        ));
                    }
                    ch => {
                        let mut buf = [0; 4];
                        append(output, ch.encode_utf8(&mut buf), budget, range)?;
                    }
                }
            }
            append(output, "\"", budget, range)
        }
        V::Member(m) => append(output, member_spelling(m, facts, range)?, budget, range),
        V::Array(values)
        | V::Tuple(values)
        | V::Set {
            elements: values, ..
        } => {
            let (open, close) = match value {
                V::Array(_) => ("[", "]"),
                V::Tuple(_) => ("(", ")"),
                _ => ("{", "}"),
            };
            append(output, open, budget, range)?;
            for (i, child) in values.iter().enumerate() {
                if i > 0 {
                    append(output, ", ", budget, range)?;
                }
                serialize(child, facts, budget, range, output)?;
            }
            if matches!(value, V::Tuple(_)) && values.len() == 1 {
                append(output, ",", budget, range)?;
            }
            append(output, close, budget, range)
        }
        V::Record(fields) => {
            append(output, "(", budget, range)?;
            for (i, (label, child)) in fields.iter().enumerate() {
                if i > 0 {
                    append(output, ", ", budget, range)?;
                }
                append(output, &identifier(label, budget, range)?, budget, range)?;
                append(output, ": ", budget, range)?;
                serialize(child, facts, budget, range, output)?;
            }
            append(output, ")", budget, range)
        }
        V::KeyedArray { axes, entries } => {
            let matrix = axes.len() == 2 && axes.iter().all(|axis| !axis.keys.is_empty() && axis.keys.iter().enumerate().all(|(i, key)| matches!(key, zirql::ValueKey::Integer(n) if i64::try_from(i + 1).ok() == Some(*n))));
            append(output, if matrix { "[| " } else { "[" }, budget, range)?;
            for (i, entry) in entries.iter().enumerate() {
                if i > 0 {
                    append(
                        output,
                        if matrix && i % axes[1].keys.len() == 0 {
                            " | "
                        } else {
                            ", "
                        },
                        budget,
                        range,
                    )?;
                }
                if !matrix {
                    if entry.key.len() > 1 {
                        append(output, "(", budget, range)?;
                    }
                    for (j, key) in entry.key.iter().enumerate() {
                        if j > 0 {
                            append(output, ",", budget, range)?;
                        }
                        serialize_key(key, facts, budget, range, output)?;
                    }
                    if entry.key.len() > 1 {
                        append(output, ")", budget, range)?;
                    }
                    append(output, ": ", budget, range)?;
                }
                serialize(&entry.value, facts, budget, range, output)?;
            }
            append(output, if matrix { " |]" } else { "]" }, budget, range)
        }
    }
}
