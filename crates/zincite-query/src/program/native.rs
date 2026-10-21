//! Native collections retain admitted payloads, source owners and current order.
use super::*;
use std::hash::{Hash, Hasher};
use zirql::{DeclaredType as D, Location, ProjectionHost, RuntimeTypeHost};

type Failure = zirql::EvaluationError<QueryError>;

pub(super) enum Shape<'a> {
    Stream {
        kind: pipeline::StreamKind,
        owner: Option<Rc<ModelFacts<'a>>>,
        entities: Option<bool>,
        item: bool,
    },
    Sequence(Rc<zirql::DeclaredType<Kind>>),
    Array,
    Captures(Rc<zirql::NativeCapture<Kind>>),
    Capture,
}
impl<'a> Shape<'a> {
    pub(super) fn restore(
        self,
        values: Vec<ReportValue<'a>>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, Failure> {
        let location = Location::Query(range.clone());
        budget.charge(values.len() + 1, location.clone())?;
        budget.charge_results(values.len() + 1, location)?;
        Ok(match self {
            Self::Array => {
                let mut array = Vec::with_capacity(values.len());
                for value in values {
                    let value = functions::owned(value, budget, range)
                        .map_err(zirql::EvaluationError::Host)?;
                    array.push(match value {
                        ReportValue::Constructed(value) => value,
                        ReportValue::SharedConstructed(value) => match Arc::try_unwrap(value) {
                            Ok(value) => value,
                            Err(value) => zirql::copy_constructed_value(
                                &value,
                                budget,
                                Location::Query(range.clone()),
                            )?,
                        },
                        _ => unreachable!("constructed array filter keeps constructed elements"),
                    });
                }
                ReportValue::Constructed(zirql::ConstructedValue::Array(array))
            }
            Self::Sequence(element) => ReportValue::Sequence(NativeSequence {
                values: Rc::new(values),
                element,
                range: range.clone(),
            }),
            Self::Captures(schema) => ReportValue::Captures(CaptureMatches {
                rows: values
                    .into_iter()
                    .map(|value| match value {
                        ReportValue::Capture(capture) => capture,
                        _ => unreachable!("admitted capture rows"),
                    })
                    .collect(),
                schema,
            }),
            Self::Capture => values.into_iter().next().ok_or_else(|| {
                zirql::QueryError::query(range.clone(), "a singleton capture cannot be empty")
            })?,
            Self::Stream {
                kind,
                owner,
                entities,
                item,
            } => {
                let mut rows = Vec::with_capacity(values.len());
                for value in values {
                    let ReportValue::Query(mut stream) = functions::owned(value, budget, range)
                        .map_err(zirql::EvaluationError::Host)?
                    else {
                        unreachable!("admitted stream row")
                    };
                    if matches!(kind, pipeline::StreamKind::Nodes) {
                        let SemanticStream::Inspection(result) = stream else {
                            unreachable!("admitted syntax nodes")
                        };
                        let nodes = inspection::into_nodes(*result, range, &mut Work { budget })
                            .map_err(zirql::EvaluationError::Host)?;
                        stream = SemanticStream::Inspection(Box::new(QueryResult::Nodes(nodes)));
                    }
                    rows.extend(context.query.take_rows(stream));
                }
                context.query.kind = kind;
                context.query.document = false;
                let mut value = ReportValue::Query(context.query.restore_stream(rows));
                if let Some(declarations) = entities {
                    value = ReportValue::EntitySequence {
                        declarations,
                        owner,
                        value: Box::new(value),
                    };
                }
                if item {
                    value = ReportValue::Item(Box::new(value));
                }
                value
            }
        })
    }
}

pub(super) fn set_kind(kind: &Kind, range: &Range<usize>) -> Result<Kind, zirql::QueryError> {
    match kind {
        Kind::Nodes | Kind::Node => Ok(Kind::Nodes),
        Kind::Declarations | Kind::Declaration => Ok(Kind::Declarations),
        Kind::Entities | Kind::Types | Kind::Instantiations | Kind::Strings => Ok(kind.clone()),
        Kind::Sequence(element)
            if matches!(element.as_ref(), D::Native(Kind::Node | Kind::Declaration)) =>
        {
            Ok(kind.clone())
        }
        _ => Err(zirql::QueryError::query(
            range.clone(),
            "native sets require identity-bearing selections",
        )),
    }
}

impl<'a> composition::CompositionHost<'_, '_> {
    pub(super) fn rows(
        &self,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(Shape<'a>, Vec<ReportValue<'a>>), Failure> {
        let location = Location::Query(range.clone());
        let kind = self.kind(&value, budget, range)?;
        let owner = worklists::owner(&value).cloned();
        if let Some(owner) = &owner {
            let facts = context
                .report_facts(budget, range)
                .map_err(zirql::EvaluationError::Host)?;
            if !facts
                .as_ref()
                .is_some_and(|facts| std::ptr::eq(owner.bindings(), facts.bindings()))
            {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "foreign native selection owner",
                )
                .into());
            }
        }
        // Reuse the runtime's complete owner/coordinate admission before splitting.
        if matches!(
            kind,
            Kind::Nodes
                | Kind::Node
                | Kind::Declarations
                | Kind::Declaration
                | Kind::Sequence(_)
                | Kind::Capture(_)
                | Kind::Captures(_)
        ) {
            let declaration = D::Native(kind.clone());
            let declaration = if let Kind::Sequence(element) = &kind {
                D::Sequence(Box::new(element.as_ref().clone()))
            } else {
                declaration
            };
            self.admit_native(&value, &declaration, context, budget, location.clone())?;
        }
        let item = functions::is_item(&value);
        let entities = match &kind {
            Kind::Declarations | Kind::Declaration => Some(true),
            Kind::Entities => Some(false),
            _ => None,
        };
        let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
        let value = match value {
            ReportValue::SharedConstructed(value) => {
                ReportValue::Constructed(match Arc::try_unwrap(value) {
                    Ok(value) => value,
                    Err(value) => zirql::copy_constructed_value(&value, budget, location.clone())?,
                })
            }
            value => value,
        };
        match value {
            ReportValue::Constructed(value)
                if matches!(value.data(), zirql::ConstructedValue::Array(_)) =>
            {
                let zirql::ConstructedValue::Array(values) = value.into_data() else {
                    unreachable!()
                };
                budget.charge_results(values.len(), location)?;
                Ok((
                    Shape::Array,
                    values.into_iter().map(ReportValue::Constructed).collect(),
                ))
            }
            ReportValue::Sequence(sequence) => {
                let element = Rc::clone(&sequence.element);
                budget.charge_results(sequence.values.len() + 1, location)?;
                let values = sequence
                    .values
                    .iter()
                    .map(|value| self.copy_value(value, context, budget, range))
                    .collect::<Result<_, _>>()?;
                Ok((Shape::Sequence(element), values))
            }
            ReportValue::Captures(captures) => Ok((
                Shape::Captures(captures.schema),
                captures
                    .rows
                    .into_iter()
                    .map(ReportValue::Capture)
                    .collect(),
            )),
            ReportValue::Capture(capture) => {
                Ok((Shape::Capture, vec![ReportValue::Capture(capture)]))
            }
            ReportValue::Query(stream) => {
                let count = composition::stream_len(&stream);
                budget.check_items(count, location.clone())?;
                budget.charge(count.saturating_mul(4).saturating_add(1), location.clone())?;
                budget.charge_results(count.saturating_mul(4).saturating_add(1), location)?;
                let rows = context.query.take_rows(stream);
                let stream_kind = context.query.kind;
                let shape = Shape::Stream {
                    kind: stream_kind,
                    owner: owner.clone(),
                    entities,
                    item,
                };
                let mut values = Vec::with_capacity(rows.len());
                for row in rows {
                    validate_row(&row, context, budget, range)?;
                    context.query.kind = stream_kind;
                    context.query.document = false;
                    let mut value = ReportValue::Query(context.query.restore_stream(vec![row]));
                    if let Some(declarations) = entities {
                        value = ReportValue::EntitySequence {
                            declarations,
                            owner: owner.clone(),
                            value: Box::new(value),
                        };
                    }
                    if self.0.functions
                        && matches!(
                            kind,
                            Kind::Nodes | Kind::Node | Kind::Declarations | Kind::Declaration
                        )
                    {
                        value = ReportValue::Item(Box::new(value));
                    }
                    values.push(value);
                }
                Ok((shape, values))
            }
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "native ordering requires a selection",
            )
            .into()),
        }
    }

    pub(super) fn combine_set(
        &self,
        operator: zirql::NativeSetOperator,
        left: ReportValue<'a>,
        right: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, Failure> {
        let partial = selection_error(&left)
            .or_else(|| selection_error(&right))
            .cloned();
        if context.query.complete_observations {
            require_complete(&left, budget, range)?;
            require_complete(&right, budget, range)?;
        }
        let left_kind = self.kind(&left, budget, range)?;
        let right_kind = self.kind(&right, budget, range)?;
        let left_kind = set_kind(&left_kind, range)?;
        let right_kind = set_kind(&right_kind, range)?;
        if left_kind != right_kind {
            return Err(zirql::QueryError::query(
                range.clone(),
                "set operands require the same native kind",
            )
            .into());
        }
        let (mut shape, left) = self.rows(left, context, budget, range)?;
        if let Shape::Stream { item, .. } = &mut shape {
            *item = false;
        }
        let (right_shape, right) = self.rows(right, context, budget, range)?;
        if let Shape::Stream { kind, .. } = &mut shape
            && matches!(
                right_shape,
                Shape::Stream {
                    kind: pipeline::StreamKind::Nodes,
                    ..
                }
            )
        {
            *kind = pipeline::StreamKind::Nodes;
        }
        let mut descriptors =
            |values: Vec<ReportValue<'a>>| -> Result<Vec<Rc<SetRow<'a>>>, Failure> {
                budget.charge_results(values.len() + 1, Location::Query(range.clone()))?;
                values
                    .into_iter()
                    .map(|value| {
                        let identity = row_identity(&value, budget, range)?;
                        Ok(Rc::new(SetRow { identity, value }))
                    })
                    .collect()
            };
        let left = descriptors(left)?;
        let right = descriptors(right)?;
        let candidates = left.iter().chain(&right).cloned().map(Ok);
        let selected = zirql::combine_native_set(
            operator,
            left.iter().cloned(),
            right.iter().cloned(),
            candidates,
            budget,
            Location::Query(range.clone()),
            |row, budget| {
                let bytes = match &row.identity {
                    pipeline::Identity::String(value) => value.len(),
                    _ => 1,
                };
                budget.charge(bytes, Location::Query(range.clone()))?;
                Ok(())
            },
        )?;
        drop(left);
        drop(right);
        let values = selected
            .into_iter()
            .map(|row| {
                Rc::try_unwrap(row)
                    .ok()
                    .expect("set memberships released")
                    .value
            })
            .collect();
        let value = shape.restore(values, context, budget, range)?;
        incomplete(value, partial, budget, range)
    }
}

pub(super) fn scalar_key<'a>(
    value: ReportValue<'a>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<zirql::NativeOrderKey, Failure> {
    let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
    let value = match value {
        ReportValue::Constructed(value) => value,
        ReportValue::SharedConstructed(value) => {
            zirql::copy_constructed_value(&value, budget, Location::Query(range.clone()))?
        }
        ReportValue::Query(SemanticStream::Inspection(result))
            if matches!(result.as_ref(), QueryResult::Count(_)) =>
        {
            let QueryResult::Count(count) = *result else {
                unreachable!()
            };
            return count_key(count, range);
        }
        ReportValue::Query(stream) => {
            let facts = context
                .report_facts(budget, range)
                .map_err(zirql::EvaluationError::Host)?;
            comprehension::project(&stream, facts.as_deref(), budget, range, nesting)
                .map_err(zirql::EvaluationError::Host)?
        }
        _ => {
            return Err(zirql::QueryError::query(
                range.clone(),
                "ordering selector needs one present scalar",
            )
            .into());
        }
    };
    let value = match value {
        zirql::ConstructedValue::Array(mut values) if values.len() == 1 => values.pop().unwrap(),
        value => value,
    };
    Ok(zirql::NativeOrderKey::Value(value))
}

struct SetRow<'a> {
    identity: pipeline::Identity,
    value: ReportValue<'a>,
}
impl PartialEq for SetRow<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}
impl Eq for SetRow<'_> {}
impl Hash for SetRow<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identity.hash(state);
    }
}

pub(super) fn row_identity(
    value: &ReportValue<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<pipeline::Identity, Failure> {
    let ReportValue::Query(stream) = functions::unretained(value) else {
        return Err(zirql::QueryError::query(
            range.clone(),
            "native sets require identity-bearing rows",
        )
        .into());
    };
    let value = match stream {
        SemanticStream::Inspection(result) => match result.as_ref() {
            QueryResult::Selection(selection) => {
                pipeline::Identity::Node(std::ptr::from_ref(selection.items[0].node))
            }
            QueryResult::Nodes(selection) => {
                pipeline::Identity::Node(std::ptr::from_ref(selection.nodes[0].node))
            }
            QueryResult::Strings(values) => {
                budget.charge(values[0].len(), Location::Query(range.clone()))?;
                pipeline::Identity::String(values[0].clone())
            }
            _ => {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "native sets require identity-bearing rows",
                )
                .into());
            }
        },
        SemanticStream::Entities(values) => {
            pipeline::subject_identity(&semantic::SemanticSubject::Entity(values[0]))
        }
        SemanticStream::Types(values) => pipeline::subject_identity(&values[0].subject),
        SemanticStream::Instantiations(values) => pipeline::subject_identity(&values[0].subject),
    };
    Ok(value)
}

fn validate_row<'a>(
    row: &pipeline::Row<'a>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), Failure> {
    budget.charge(1, Location::Query(range.clone()))?;
    let subject = match row {
        pipeline::Row::Entity(entity) => Some(semantic::SemanticSubject::Entity(*entity)),
        pipeline::Row::Type(value) => Some(value.subject.clone()),
        pipeline::Row::Instantiation(value) => Some(value.subject.clone()),
        _ => None,
    };
    if let Some(subject) = subject {
        let facts = context
            .report_facts(budget, range)
            .map_err(zirql::EvaluationError::Host)?
            .ok_or_else(|| {
                zirql::QueryError::query(range.clone(), "semantic rows require model facts")
            })?;
        let valid = match subject {
            semantic::SemanticSubject::Entity(semantic::SemanticEntity::Declaration(id)) => facts
                .bindings()
                .declarations
                .get(id.0)
                .is_some_and(|d| d.id == id),
            semantic::SemanticSubject::Entity(semantic::SemanticEntity::Reference(id)) => {
                facts.bindings().references.get(id).is_some()
            }
            semantic::SemanticSubject::Node {
                file,
                node,
                location,
            } => facts
                .model()
                .files
                .get(file)
                .is_some_and(|source| source.location(node.range()) == location),
        };
        if !valid {
            return Err(zirql::QueryError::query(
                range.clone(),
                "foreign or unavailable semantic subject",
            )
            .into());
        }
    }
    Ok(())
}

pub(super) fn supported_key(kind: &Kind) -> bool {
    match kind {
        Kind::Count | Kind::Strings | Kind::Literals | Kind::Constructed => true,
        Kind::Scalar(zirql::ParameterType::Int | zirql::ParameterType::String) => true,
        Kind::Value(types) => types.iter().all(ordered_key_type),
        _ => false,
    }
}

fn ordered_key_type(ty: &zirql::ValueType) -> bool {
    use zirql::ValueType as T;
    match ty {
        T::Declared {
            actual,
            declaration,
        } if matches!(actual.as_ref(), T::Unestablished) => declared_scalar_key(declaration, false),
        T::Declared { actual, .. } => ordered_key_type(actual),
        T::Integer | T::Float | T::String | T::Member(_) | T::Unestablished => true,
        _ => false,
    }
}

fn declared_scalar_key(mut declaration: &D<()>, string_only: bool) -> bool {
    while let D::Optional(inner) = declaration {
        declaration = inner;
    }
    matches!(declaration, D::String)
        || !string_only && matches!(declaration, D::Integer | D::Float | D::Member(_))
}

fn string_type(ty: &zirql::ValueType) -> bool {
    use zirql::ValueType as T;
    match ty {
        T::String | T::Unestablished => true,
        T::Declared {
            actual,
            declaration,
        } if matches!(actual.as_ref(), T::Unestablished) => declared_scalar_key(declaration, true),
        T::Declared { actual, .. } => string_type(actual),
        _ => false,
    }
}

fn string_key_type(ty: &zirql::ValueType) -> bool {
    use zirql::{ArrayType, ValueType as T};
    match ty {
        T::Declared { actual, .. } if !matches!(actual.as_ref(), T::Unestablished) => {
            string_key_type(actual)
        }
        T::Array(ArrayType::Positions(values)) => values.len() == 1 && string_type(&values[0]),
        T::Array(ArrayType::Elements(values)) | T::ArrayKind(values) => {
            values.iter().all(string_type)
        }
        _ => string_type(ty),
    }
}

pub(super) fn map_sort_kind(input: &Kind, range: &Range<usize>) -> Result<Kind, zirql::QueryError> {
    match input {
        Kind::Dictionary(values) if values.iter().all(supported_key) => Ok(input.clone()),
        Kind::Tally | Kind::Record => Ok(input.clone()),
        _ => Err(zirql::QueryError::query(
            range.clone(),
            "dictionary value sorting requires supported scalar values",
        )),
    }
}

fn count_key(count: usize, range: &Range<usize>) -> Result<zirql::NativeOrderKey, Failure> {
    let count = i64::try_from(count).map_err(|_| {
        zirql::QueryError::query(range.clone(), "native count exceeds signed sort key range")
    })?;
    Ok(zirql::NativeOrderKey::Value(
        zirql::ConstructedValue::Integer(count),
    ))
}

pub(super) fn string_keys(
    value: ReportValue<'_>,
    context: &mut Context<'_, '_>,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<Option<Vec<String>>, Failure> {
    let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
    match value {
        ReportValue::Query(SemanticStream::Inspection(result)) => match *result {
            QueryResult::Strings(values) => Ok(Some(values)),
            QueryResult::Literals(values) => {
                let stream = SemanticStream::Inspection(Box::new(QueryResult::Literals(values)));
                let facts = context
                    .report_facts(budget, range)
                    .map_err(zirql::EvaluationError::Host)?;
                let value =
                    comprehension::project(&stream, facts.as_deref(), budget, range, nesting)
                        .map_err(zirql::EvaluationError::Host)?;
                string_keys(
                    ReportValue::Constructed(value),
                    context,
                    budget,
                    range,
                    nesting,
                )
            }
            _ => Ok(None),
        },
        ReportValue::SharedConstructed(value) => string_keys(
            ReportValue::Constructed(zirql::copy_constructed_value(
                &value,
                budget,
                Location::Query(range.clone()),
            )?),
            context,
            budget,
            range,
            nesting,
        ),
        ReportValue::Constructed(value) => match value.into_data() {
            zirql::ConstructedValue::String(value) => Ok(Some(vec![value])),
            zirql::ConstructedValue::Array(values) => {
                let mut strings = Vec::new();
                for value in values {
                    let zirql::ConstructedValue::String(value) = value.into_data() else {
                        return Ok(None);
                    };
                    strings.push(value);
                }
                Ok(Some(strings))
            }
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}

pub(super) fn bound_map<'a>(
    value: ReportValue<'a>,
    pipeline: &NativePipeline<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, Failure> {
    fn select<T>(
        entries: &mut Vec<(String, T)>,
        pipeline: &NativePipeline<'_>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), Failure> {
        for stage in pipeline.stages() {
            let selection = match stage.value {
                zirql::Stage::Head(n) => zirql::MapSelection::Head(n),
                zirql::Stage::Tail(n) => zirql::MapSelection::Tail(n),
                zirql::Stage::Reverse => zirql::MapSelection::Reverse,
                _ => {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "query stage is unavailable on dictionaries",
                    )
                    .into());
                }
            };
            zirql::select_map(
                entries,
                selection,
                budget,
                Location::Query(stage.range.clone()),
            )?;
        }
        Ok(())
    }
    match value {
        ReportValue::Record(mut entries) => {
            select(&mut entries, pipeline, budget, range)?;
            Ok(ReportValue::Record(entries))
        }
        ReportValue::Query(SemanticStream::Inspection(result)) => {
            let QueryResult::Tally(mut entries) = *result else {
                unreachable!("admitted dictionary")
            };
            select(&mut entries, pipeline, budget, range)?;
            Ok(ReportValue::Query(SemanticStream::Inspection(Box::new(
                QueryResult::Tally(entries),
            ))))
        }
        _ => unreachable!("admitted dictionary"),
    }
}

pub(super) fn selection_error<'v>(value: &'v ReportValue<'_>) -> Option<&'v Arc<QueryError>> {
    match value {
        ReportValue::IncompleteSelection { error, .. } => Some(error),
        ReportValue::Retained(value) => selection_error(value),
        ReportValue::Item(value) | ReportValue::EntitySequence { value, .. } => {
            selection_error(value)
        }
        _ => None,
    }
}
fn error_copy_units(error: &QueryError) -> usize {
    let path = if let crate::ErrorLocation::Model(location) = &error.location {
        location.path.as_os_str().len()
    } else {
        0
    };
    error.message.len().saturating_add(path).saturating_add(1)
}

pub(super) fn require_complete(
    value: &ReportValue<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), Failure> {
    if let Some(error) = selection_error(value) {
        budget.charge(error_copy_units(error), Location::Query(range.clone()))?;
        budget.charge_results(1, Location::Query(range.clone()))?;
        return Err(zirql::EvaluationError::Host(error.as_ref().clone()));
    }
    Ok(())
}
pub(super) fn incomplete<'a>(
    value: ReportValue<'a>,
    error: Option<Arc<QueryError>>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, Failure> {
    match error {
        Some(error) if selection_error(&value).is_none() => {
            budget.charge(1, Location::Query(range.clone()))?;
            budget.charge_results(1, Location::Query(range.clone()))?;
            Ok(ReportValue::IncompleteSelection {
                value: Box::new(value),
                error,
            })
        }
        _ => Ok(value),
    }
}

impl<'a> composition::CompositionHost<'_, '_> {
    pub(super) fn admitted_copy(
        &self,
        value: &ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, Failure> {
        require_complete(value, budget, range)?;
        let kind = self.kind(value, budget, range)?;
        set_kind(&kind, range)?;
        let value = self.copy_value(value, context, budget, range)?;
        let (shape, rows) = self.rows(value, context, budget, range)?;
        shape.restore(rows, context, budget, range)
    }
    pub(super) fn same_native_selection(
        &self,
        left: &ReportValue<'a>,
        right: &ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<bool, Failure> {
        require_complete(left, budget, range)?;
        require_complete(right, budget, range)?;
        if self.kind(left, budget, range)? != self.kind(right, budget, range)? {
            return Err(zirql::QueryError::query(
                range.clone(),
                "fixpoint body changed the native kind",
            )
            .into());
        }
        let left_owner = worklists::owner(left);
        let right_owner = worklists::owner(right);
        if left_owner.map(Rc::as_ptr) != right_owner.map(Rc::as_ptr) {
            return Err(zirql::QueryError::query(
                range.clone(),
                "fixpoint body changed the native owner",
            )
            .into());
        }
        let left = self.copy_value(left, context, budget, range)?;
        let right = self.copy_value(right, context, budget, range)?;
        let (_, left) = self.rows(left, context, budget, range)?;
        let (_, right) = self.rows(right, context, budget, range)?;
        zirql::ordered_native_equal(
            &left,
            &right,
            budget,
            Location::Query(range.clone()),
            |left, right, budget| {
                if let (
                    ReportValue::Query(SemanticStream::Inspection(a)),
                    ReportValue::Query(SemanticStream::Inspection(b)),
                ) = (functions::unretained(left), functions::unretained(right))
                    && let (QueryResult::Strings(a), QueryResult::Strings(b)) =
                        (a.as_ref(), b.as_ref())
                {
                    budget.charge(a[0].len() + b[0].len(), Location::Query(range.clone()))?;
                    return Ok(a[0] == b[0]);
                }
                Ok(row_identity(left, budget, range)? == row_identity(right, budget, range)?)
            },
        )
    }
}

pub(super) fn finish_filter(
    truth: zirql::Truth,
    context: &mut Context<'_, '_>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<zirql::Truth, Failure> {
    let diagnostic = context.guard_unknown.take();
    if truth == zirql::Truth::Unknown {
        let error = if let Some(diagnostic) = diagnostic {
            budget.charge(
                diagnostic.message.len() + diagnostic.location.path.as_os_str().len() + 1,
                Location::Query(range.clone()),
            )?;
            QueryError {
                range: diagnostic.location.range.clone(),
                location: crate::ErrorLocation::Model(diagnostic.location),
                message: diagnostic.message,
            }
        } else {
            QueryError::query(range.clone(), "predicate is uncertain")
        };
        if context.query.complete_observations {
            return Err(zirql::EvaluationError::Host(error));
        }
        if context.query.incomplete_observation.is_none() {
            context.query.incomplete_observation = Some(Arc::new(error));
        }
    }
    Ok(truth)
}

pub(super) fn shape<'a>(value: &ReportValue<'a>) -> Shape<'a> {
    let item = functions::is_item(value);
    let owner = worklists::owner(value).cloned();
    match functions::unretained(value) {
        ReportValue::Sequence(sequence) => Shape::Sequence(Rc::clone(&sequence.element)),
        ReportValue::Captures(captures) => Shape::Captures(Rc::clone(&captures.schema)),
        ReportValue::Capture(_) => Shape::Capture,
        ReportValue::Constructed(_) | ReportValue::SharedConstructed(_) => Shape::Array,
        ReportValue::Query(stream) => {
            let (kind, entities) = match stream {
                SemanticStream::Entities(rows) => {
                    (
                        pipeline::StreamKind::Entities,
                        Some(rows.iter().all(|entity| {
                            matches!(entity, semantic::SemanticEntity::Declaration(_))
                        })),
                    )
                }
                SemanticStream::Types(_) => (pipeline::StreamKind::Types, None),
                SemanticStream::Instantiations(_) => (pipeline::StreamKind::Instantiations, None),
                SemanticStream::Inspection(result) => (
                    match result.as_ref() {
                        QueryResult::Selection(_) => pipeline::StreamKind::Items,
                        QueryResult::Nodes(_) => pipeline::StreamKind::Nodes,
                        QueryResult::Strings(_) => pipeline::StreamKind::Strings,
                        QueryResult::Literals(_) => pipeline::StreamKind::Literals,
                        _ => unreachable!("selection shape"),
                    },
                    None,
                ),
            };
            Shape::Stream {
                kind,
                owner,
                entities,
                item,
            }
        }
        _ => unreachable!("admitted native selection"),
    }
}
pub(super) fn string_key_kind(key: &Kind) -> bool {
    match key {
        Kind::Strings
        | Kind::Literals
        | Kind::Scalar(zirql::ParameterType::String)
        | Kind::Constructed => true,
        Kind::Value(types) => types.iter().all(string_key_type),
        _ => false,
    }
}
pub(super) fn map_entries<'a>(
    value: ReportValue<'a>,
    _: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Vec<(String, ReportValue<'a>)>, Failure> {
    match functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)? {
        ReportValue::Record(entries) => Ok(entries),
        ReportValue::Query(SemanticStream::Inspection(result)) => match *result {
            QueryResult::Tally(entries) => Ok(entries
                .into_iter()
                .map(|(key, count)| {
                    (
                        key,
                        ReportValue::Query(SemanticStream::Inspection(Box::new(
                            QueryResult::Count(count),
                        ))),
                    )
                })
                .collect()),
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "value sorting requires a dictionary",
            )
            .into()),
        },
        _ => Err(
            zirql::QueryError::query(range.clone(), "value sorting requires a dictionary").into(),
        ),
    }
}
pub(super) fn restore_map<'a>(
    input: ReportValue<'a>,
    entries: Vec<(String, ReportValue<'a>)>,
    _: &mut Context<'_, 'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, Failure> {
    if matches!(functions::unretained(&input),ReportValue::Query(SemanticStream::Inspection(result)) if matches!(result.as_ref(),QueryResult::Tally(_)))
    {
        let entries = entries
            .into_iter()
            .map(
                |(key, value)| match functions::owned(value, budget, range) {
                    Ok(ReportValue::Query(SemanticStream::Inspection(value))) => match *value {
                        QueryResult::Count(count) => Ok((key, count)),
                        _ => unreachable!("tally value"),
                    },
                    Err(error) => Err(zirql::EvaluationError::Host(error)),
                    _ => unreachable!("tally value"),
                },
            )
            .collect::<Result<_, _>>()?;
        Ok(ReportValue::Query(SemanticStream::Inspection(Box::new(
            QueryResult::Tally(entries),
        ))))
    } else {
        Ok(ReportValue::Record(entries))
    }
}
