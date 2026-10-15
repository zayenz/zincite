//! Native ordering and sets retain the admitted stream shape across selectors.
use super::*;
use std::hash::{Hash, Hasher};
use zirql::{DeclaredType as D, Location, ProjectionHost, RuntimeTypeHost};

type Failure = zirql::EvaluationError<QueryError>;

#[derive(Debug)]
pub(super) struct OrderingStage {
    pub(super) order: zirql::NativeOrder,
    pub(super) selector: Option<zirql::ValueExpression<Expression>>,
}
impl OrderingStage {
    pub(super) fn new(ordering: zirql::NativeOrdering<Expression>) -> Self {
        Self {
            order: ordering.order,
            selector: ordering.selector.map(|mut selector| {
                let mut source = &mut selector;
                loop {
                    match &mut source.value {
                        ExpressionKind::Group(value) => source = value,
                        ExpressionKind::Sequence { input, steps } => {
                            for step in steps {
                                if let ExpressionStep::Stage(Stage::FunctionSource(value)) =
                                    &mut step.value
                                {
                                    let value = std::mem::replace(
                                        value,
                                        Located::new(
                                            zirql::ValueExpressionKind::Literal(
                                                zirql::ConstructedValue::Absent,
                                            ),
                                            0..0,
                                        ),
                                    );
                                    step.value = ExpressionStep::Stage(Stage::FunctionValue(value));
                                }
                            }
                            source = input;
                        }
                        _ => break,
                    }
                }
                let range = selector.range.clone();
                Located::new(
                    zirql::ValueExpressionKind::NativeValue(Box::new(selector)),
                    range,
                )
            }),
        }
    }
}
impl Drop for OrderingStage {
    fn drop(&mut self) {
        if let Some(selector) = self.selector.take() {
            zirql::dispose_native_value(selector);
        }
    }
}

enum Shape<'a> {
    Stream {
        kind: pipeline::StreamKind,
        owner: Option<Rc<ModelFacts<'a>>>,
        entities: Option<bool>,
        item: bool,
    },
    Sequence(Rc<zirql::DeclaredType<Kind>>),
    Captures(Rc<zirql::NativeCapture<Kind>>),
    Capture,
}
impl<'a> Shape<'a> {
    fn restore(
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
    pub(super) fn order_kind(
        &self,
        ordering: &OrderingStage,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        let subject = self.subject_kind(input, budget, range)?;
        let key = if let Some(selector) = &ordering.selector {
            functions::value_kind(selector, self, &subject, scope, budget)?
        } else {
            subject
        };
        let supported = match &key {
            Kind::Count | Kind::Strings | Kind::Literals | Kind::Constructed => true,
            Kind::Scalar(zirql::ParameterType::Int | zirql::ParameterType::String) => true,
            Kind::Value(types) => types.iter().all(|ty| {
                matches!(
                    ty,
                    zirql::ValueType::Integer
                        | zirql::ValueType::Float
                        | zirql::ValueType::String
                        | zirql::ValueType::Member(_)
                        | zirql::ValueType::Unestablished
                )
            }),
            _ => false,
        };
        if !supported {
            return Err(zirql::QueryError::query(
                range.clone(),
                "native ordering needs an explicit supported scalar key",
            ));
        }
        Ok(input.clone())
    }

    fn rows(
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
        match value {
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

    pub(super) fn order(
        &self,
        ordering: &OrderingStage,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, Failure> {
        let kind = self.kind(&value, budget, range)?;
        self.subject_kind(&kind, budget, range)?;
        let (shape, rows) = self.rows(value, context, budget, range)?;
        let rows = zirql::order_native_by(
            rows,
            ordering.order,
            budget,
            Location::Query(range.clone()),
            |row, budget| {
                let input = self.copy_value(row, context, budget, range)?;
                let value = if let Some(selector) = &ordering.selector {
                    let zirql::ValueExpressionKind::NativeValue(selector) = &selector.value else {
                        unreachable!()
                    };
                    zirql::evaluate_expression(selector, self, input, context, bindings, budget)?
                } else {
                    input
                };
                scalar_key(value, context, budget, range, self.0.nesting)
            },
        )?;
        shape.restore(rows, context, budget, range)
    }

    pub(super) fn order_with_arguments(
        &self,
        ordering: &OrderingStage,
        value: ReportValue<'a>,
        arguments: &mut dyn zirql::NativeArguments<Context<'_, 'a>, Self>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, Failure> {
        let (shape, rows) = self.rows(value, arguments.parts().0, budget, range)?;
        let rows = zirql::order_native_by(
            rows,
            ordering.order,
            budget,
            Location::Query(range.clone()),
            |row, budget| {
                let input = self.copy_value(row, arguments.parts().0, budget, range)?;
                let value = if let Some(selector) = &ordering.selector {
                    ReportValue::Retained(
                        arguments
                            .evaluate_native(selector, input, budget)?
                            .into_shared(),
                    )
                } else {
                    input
                };
                scalar_key(value, arguments.parts().0, budget, range, self.0.nesting)
            },
        )?;
        shape.restore(rows, arguments.parts().0, budget, range)
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
        shape.restore(values, context, budget, range)
    }
}

fn scalar_key<'a>(
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

fn row_identity(
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
