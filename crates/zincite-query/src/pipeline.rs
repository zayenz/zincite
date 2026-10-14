//! Lower fixed query syntax to shared execution, retaining native rows and edits.
use std::ops::Range;
use std::rc::Rc;

use zincite_lint::{ModelContext, ModelFacts};
use zincite_syntax::SyntaxNode;
use zirql::{Budget, EvaluationError, Host, Located, NativeInput, NativeValue, Truth};

use crate::language::{Predicate, PredicateKind, Stage, StageKind};
use crate::{
    Input, Limits, LiteralSelection, LiteralValue, NodeSelection, Query, QueryError, QueryResult,
    SelectedItem, SelectedNode, Selection, SemanticEntity, SemanticInstantiation, SemanticResult,
    SemanticStream, SemanticSubject, SemanticType, Work, inspection, semantic,
};

type Failure = EvaluationError<QueryError>;
type Leaf<'a> = (&'a Predicate, &'a Stage);

#[derive(Debug)]
pub(crate) enum Row<'a> {
    Item(SelectedItem<'a>),
    Node(SelectedNode<'a>),
    String(String),
    Literal(LiteralValue<'a>),
    Entity(SemanticEntity),
    Type(SemanticType<'a>),
    Instantiation(SemanticInstantiation<'a>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum StreamKind {
    Items,
    Nodes,
    Strings,
    Literals,
    Entities,
    Types,
    Instantiations,
}

impl StreamKind {
    fn is_semantic(self) -> bool {
        matches!(self, Self::Entities | Self::Types | Self::Instantiations)
    }
}

#[derive(Debug)]
pub(crate) struct Context<'a> {
    input: &'a Input,
    model: Option<&'a ModelContext>,
    terminal: Option<SemanticStream<'a>>,
    limits: Limits,
    pub(crate) kind: StreamKind,
    pub(crate) document: bool,
    pub(crate) report: Option<SemanticResult<'a>>,
    pub(crate) callable_references: Vec<usize>,
    filter_subject: Option<SemanticSubject<'a>>,
    semantic_filter: bool,
    syntax_filter: Option<(String, Option<&'a str>)>,
}

pub(crate) fn budget(limits: Limits) -> Budget {
    Budget::new(zirql::Limits {
        work: limits.work,
        items: limits.collection,
        nesting: limits.nesting,
    })
}

pub(crate) fn core_error(error: zirql::QueryError) -> QueryError {
    let message = match error.category {
        zirql::QueryErrorCategory::WorkLimit => "query work limit exceeded".to_owned(),
        zirql::QueryErrorCategory::ItemLimit => "query collection limit exceeded".to_owned(),
        _ => error.message,
    };
    match error.location {
        zirql::Location::Query(range) => QueryError::query(range, message),
        zirql::Location::Source(range) => QueryError::input(range, message),
    }
}

pub(crate) fn evaluation_error(error: Failure) -> QueryError {
    match error {
        EvaluationError::Core(error) => core_error(error),
        EvaluationError::Host(error) => error,
    }
}

// Lower query stages, prepare native input, execute with one budget, restore the
// native result, then perform the host's terminal and completeness checks.
pub(crate) fn evaluate<'a>(
    query: &Query,
    input: &'a Input,
    facts: Option<Rc<ModelFacts<'a>>>,
    model: Option<&'a ModelContext>,
    limits: Limits,
) -> Result<QueryResult<'a>, QueryError> {
    let pipeline = lower_pipeline(query)?;
    // Written predicate depth belongs to parsing; literal depth uses the
    // evaluation limits retained in the host context.
    let mut budget = budget(Limits {
        nesting: query.nesting_limit,
        ..limits
    });
    let (mut context, rows) = prepare_context(query, input, facts, limits, &mut budget)?;
    context.model = model;
    let input = NativeInput::from_admitted(
        &mut context,
        rows,
        &budget,
        zirql::Location::Query(query.range.clone()),
    )
    .map_err(core_error)?;
    let result =
        zirql::evaluate(&pipeline, &NativeHost, input, &mut budget).map_err(evaluation_error)?;
    let value = result.value;
    finish_result(query, context, value, &mut budget)
}

pub(crate) fn lower_pipeline(
    query: &Query,
) -> Result<zirql::Pipeline<Leaf<'_>, &Stage>, QueryError> {
    let stages = query
        .stages
        .iter()
        .filter_map(|stage| {
            let value = match &stage.kind {
                StageKind::Filter(predicate) => {
                    zirql::Stage::Filter(lower_predicate(predicate, stage))
                }
                StageKind::Head(count) => zirql::Stage::Head(*count),
                StageKind::Count => zirql::Stage::Count,
                StageKind::Unique => zirql::Stage::Unique,
                StageKind::Tally => zirql::Stage::Tally,
                StageKind::Emit | StageKind::Json => return None,
                StageKind::Items
                | StageKind::References
                | StageKind::Declarations
                | StageKind::Uses
                | StageKind::TransitiveReferences(_)
                | StageKind::ReferencedDeclarations
                | StageKind::Types
                | StageKind::Instantiations
                | StageKind::Expressions
                | StageKind::Children
                | StageKind::Subtree
                | StageKind::Range(_)
                | StageKind::Names
                | StageKind::CallNames
                | StageKind::AnnotationNames
                | StageKind::Text
                | StageKind::Values
                | StageKind::Fields
                | StageKind::Elements
                | StageKind::Keys
                | StageKind::FilterElements(_)
                | StageKind::SetValue(_)
                | StageKind::Remove
                | StageKind::ReduceEnum(_, _)
                | StageKind::EmitDocument => zirql::Stage::Host(stage),
            };
            Some(Located::new(value, stage.range.clone()))
        })
        .collect();
    zirql::Pipeline::with_group_not_nesting(stages, query.range.clone(), query.nesting_limit)
        .map_err(core_error)
}

pub(crate) fn prepare_context<'a>(
    query: &Query,
    input: &'a Input,
    facts: Option<Rc<ModelFacts<'a>>>,
    limits: Limits,
    budget: &mut Budget,
) -> Result<(Context<'a>, Vec<Row<'a>>), QueryError> {
    let mut work = Work { budget };
    let requesting = query
        .stages
        .iter()
        .find(|stage| semantic::requests_facts(stage));
    let mut report = if let Some(requesting) = requesting {
        let facts = facts.ok_or_else(|| {
            QueryError::query(
                requesting.range.clone(),
                "semantic inspection requires a retained model context (--model for stdin)",
            )
        })?;
        Some(semantic::prepare(query, input, facts, &mut work, limits)?)
    } else {
        None
    };
    let stream = if let Some(report) = &mut report {
        std::mem::replace(&mut report.stream, SemanticStream::Entities(Vec::new()))
    } else {
        SemanticStream::Inspection(Box::new(QueryResult::Selection(Selection {
            input,
            items: crate::collect_items(input, limits.collection, &mut work, &query.range, true)?,
            document: true,
        })))
    };
    let mut context = Context {
        input,
        model: None,
        terminal: None,
        limits,
        kind: StreamKind::Items,
        document: true,
        report,
        callable_references: Vec::new(),
        filter_subject: None,
        semantic_filter: false,
        syntax_filter: None,
    };
    let rows = context.take_rows(stream);
    Ok((context, rows))
}

fn finish_result<'a>(
    query: &Query,
    mut context: Context<'a>,
    value: NativeValue<Row<'a>>,
    budget: &mut Budget,
) -> Result<QueryResult<'a>, QueryError> {
    let mut stream = match value {
        NativeValue::Stream(rows) => context
            .terminal
            .take()
            .unwrap_or_else(|| context.restore_stream(rows)),
        NativeValue::Count(count) => {
            SemanticStream::Inspection(Box::new(QueryResult::Count(count)))
        }
        NativeValue::Tally(counts) => {
            SemanticStream::Inspection(Box::new(QueryResult::Tally(counts)))
        }
    };
    let mut work = Work { budget };
    if let Some(terminal) = query.stages.last()
        && matches!(terminal.kind, StageKind::Emit | StageKind::Json)
    {
        work.visit(&terminal.range)?;
        stream = if let Some(report) = &mut context.report {
            report.evaluate_stage(
                context.input,
                stream,
                terminal,
                &mut work,
                context.limits,
                &mut context.callable_references,
            )?
        } else {
            let SemanticStream::Inspection(result) = stream else {
                unreachable!()
            };
            SemanticStream::Inspection(Box::new(crate::evaluate_stage(
                context.input,
                None,
                *result,
                terminal,
                &mut work,
                context.limits,
            )?))
        };
    }
    if let SemanticStream::Inspection(result) = &stream
        && let QueryResult::Selection(selection) = result.as_ref()
        && !selection.document
    {
        crate::validate_fragment_directives(context.input, &selection.items)?;
    }
    if let Some(mut report) = context.report {
        report.stream = stream;
        report.finish(
            context.callable_references,
            &mut work,
            query
                .stages
                .iter()
                .find(|stage| semantic::requests_facts(stage))
                .expect("semantic report has a requesting stage"),
            &query.range,
            context.limits,
        )?;
        return Ok(QueryResult::Semantic(report));
    }
    let SemanticStream::Inspection(result) = stream else {
        unreachable!()
    };
    match result.as_ref() {
        QueryResult::Literals(selection) => {
            for value in selection.values() {
                work.spend(value.range.len(), &query.range)?;
            }
        }
        QueryResult::Nodes(selection) => {
            for node in selection.nodes() {
                work.spend(node.range.len(), &query.range)?;
            }
        }
        _ => {}
    }
    Ok(*result)
}

fn lower_predicate<'a>(predicate: &'a Predicate, stage: &'a Stage) -> zirql::Predicate<Leaf<'a>> {
    let value = match &predicate.kind {
        PredicateKind::Not(child) => {
            zirql::PredicateKind::Not(Box::new(lower_predicate(child, stage)))
        }
        PredicateKind::Group(child) => {
            zirql::PredicateKind::Group(Box::new(lower_predicate(child, stage)))
        }
        PredicateKind::And(children) => zirql::PredicateKind::And(
            children
                .iter()
                .map(|child| lower_predicate(child, stage))
                .collect(),
        ),
        PredicateKind::Or(children) => zirql::PredicateKind::Or(
            children
                .iter()
                .map(|child| lower_predicate(child, stage))
                .collect(),
        ),
        PredicateKind::Kind(_)
        | PredicateKind::Name(_)
        | PredicateKind::Type(_)
        | PredicateKind::Instantiation(_)
        | PredicateKind::SourceKind(_) => zirql::PredicateKind::Leaf((predicate, stage)),
    };
    Located::new(value, predicate.range.clone())
}

fn filter_stage<'a>(mut predicate: &zirql::Predicate<Leaf<'a>>) -> &'a Stage {
    loop {
        match &predicate.value {
            zirql::PredicateKind::Leaf((_, stage)) => return stage,
            zirql::PredicateKind::Not(child) | zirql::PredicateKind::Group(child) => {
                predicate = child
            }
            zirql::PredicateKind::And(children) | zirql::PredicateKind::Or(children) => {
                predicate = &children[0]
            }
        }
    }
}

impl<'a> Context<'a> {
    pub(crate) fn take_rows(&mut self, stream: SemanticStream<'a>) -> Vec<Row<'a>> {
        match stream {
            SemanticStream::Entities(rows) => {
                self.kind = StreamKind::Entities;
                rows.into_iter().map(Row::Entity).collect()
            }
            SemanticStream::Types(rows) => {
                self.kind = StreamKind::Types;
                rows.into_iter().map(Row::Type).collect()
            }
            SemanticStream::Instantiations(rows) => {
                self.kind = StreamKind::Instantiations;
                rows.into_iter().map(Row::Instantiation).collect()
            }
            SemanticStream::Inspection(result) => match *result {
                QueryResult::Selection(selection) => {
                    self.kind = StreamKind::Items;
                    self.document = selection.document;
                    selection.items.into_iter().map(Row::Item).collect()
                }
                QueryResult::Nodes(selection) => {
                    self.kind = StreamKind::Nodes;
                    selection.nodes.into_iter().map(Row::Node).collect()
                }
                QueryResult::Strings(rows) => {
                    self.kind = StreamKind::Strings;
                    rows.into_iter().map(Row::String).collect()
                }
                QueryResult::Literals(selection) => {
                    self.kind = StreamKind::Literals;
                    selection.values.into_iter().map(Row::Literal).collect()
                }
                result @ (QueryResult::Document(_) | QueryResult::Reduction(_)) => {
                    self.terminal = Some(SemanticStream::Inspection(Box::new(result)));
                    Vec::new()
                }
                QueryResult::Count(_)
                | QueryResult::Tally(_)
                | QueryResult::Json(_)
                | QueryResult::Semantic(_) => unreachable!("host stages return native streams"),
            },
        }
    }

    pub(crate) fn restore_stream(&self, rows: Vec<Row<'a>>) -> SemanticStream<'a> {
        let result = match self.kind {
            StreamKind::Items => QueryResult::Selection(Selection {
                input: self.input,
                document: self.document,
                items: rows
                    .into_iter()
                    .map(|row| {
                        let Row::Item(item) = row else { unreachable!() };
                        item
                    })
                    .collect(),
            }),
            StreamKind::Nodes => QueryResult::Nodes(NodeSelection {
                input: self.input,
                nodes: rows
                    .into_iter()
                    .map(|row| {
                        let Row::Node(node) = row else { unreachable!() };
                        node
                    })
                    .collect(),
            }),
            StreamKind::Strings => QueryResult::Strings(
                rows.into_iter()
                    .map(|row| {
                        let Row::String(value) = row else {
                            unreachable!()
                        };
                        value
                    })
                    .collect(),
            ),
            StreamKind::Literals => QueryResult::Literals(LiteralSelection {
                input: self.input,
                values: rows
                    .into_iter()
                    .map(|row| {
                        let Row::Literal(value) = row else {
                            unreachable!()
                        };
                        value
                    })
                    .collect(),
            }),
            StreamKind::Entities => {
                return SemanticStream::Entities(
                    rows.into_iter()
                        .map(|row| {
                            let Row::Entity(value) = row else {
                                unreachable!()
                            };
                            value
                        })
                        .collect(),
                );
            }
            StreamKind::Types => {
                return SemanticStream::Types(
                    rows.into_iter()
                        .map(|row| {
                            let Row::Type(value) = row else {
                                unreachable!()
                            };
                            value
                        })
                        .collect(),
                );
            }
            StreamKind::Instantiations => {
                return SemanticStream::Instantiations(
                    rows.into_iter()
                        .map(|row| {
                            let Row::Instantiation(value) = row else {
                                unreachable!()
                            };
                            value
                        })
                        .collect(),
                );
            }
        };
        SemanticStream::Inspection(Box::new(result))
    }

    pub(crate) fn subject(&self, row: &Row<'a>) -> SemanticSubject<'a> {
        let node = match row {
            Row::Entity(entity) => return SemanticSubject::Entity(*entity),
            Row::Type(row) => return row.subject.clone(),
            Row::Instantiation(row) => return row.subject.clone(),
            Row::Item(item) => item.node,
            Row::Node(node) => node.node,
            Row::String(_) | Row::Literal(_) => {
                unreachable!("filter applicability checked before rows")
            }
        };
        let model = self.report.as_ref().unwrap().model();
        let file = model.root_file.unwrap();
        SemanticSubject::Node {
            file,
            node,
            location: model.files[file].location(node.range()),
        }
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) enum Identity {
    Node(*const SyntaxNode),
    String(String),
    Declaration(usize),
    Reference(usize),
    Subject(usize, usize, usize),
}

fn subject_identity(subject: &SemanticSubject<'_>) -> Identity {
    match subject {
        SemanticSubject::Entity(SemanticEntity::Declaration(id)) => Identity::Declaration(id.0),
        SemanticSubject::Entity(SemanticEntity::Reference(id)) => Identity::Reference(*id),
        SemanticSubject::Node { file, location, .. } => {
            Identity::Subject(*file, location.range.start, location.range.end)
        }
    }
}

#[derive(Debug)]
pub(crate) struct NativeHost;

impl<'a, 'q> Host<Row<'a>, &mut Context<'a>, Leaf<'q>, &'q Stage> for NativeHost {
    type Error = QueryError;
    type Key = Identity;

    fn prepare_stage(
        &self,
        stage: &Located<zirql::Stage<Leaf<'q>, &'q Stage>>,
        context: &mut &mut Context<'a>,
        _budget: &mut Budget,
    ) -> Result<(), Failure> {
        let error = match &stage.value {
            zirql::Stage::Filter(predicate) => {
                let filter = filter_stage(predicate);
                let StageKind::Filter(predicate) = &filter.kind else {
                    unreachable!()
                };
                context.semantic_filter = context.report.is_some()
                    && (semantic::predicate_facts(predicate) || context.kind.is_semantic());
                if matches!(context.kind, StreamKind::Strings | StreamKind::Literals) {
                    Some(if context.semantic_filter {
                        "filter requires selected nodes or semantic rows"
                    } else {
                        "filter requires selected nodes"
                    })
                } else {
                    if matches!(context.kind, StreamKind::Items) {
                        context.document = false;
                    }
                    None
                }
            }
            zirql::Stage::Head(_) => {
                if matches!(context.kind, StreamKind::Items) {
                    context.document = false;
                }
                None
            }
            zirql::Stage::Count => None,
            zirql::Stage::Unique => matches!(context.kind, StreamKind::Literals)
                .then_some("unique requires a node or string stream"),
            zirql::Stage::Tally => (!matches!(context.kind, StreamKind::Strings))
                .then_some("tally requires projected strings"),
            zirql::Stage::Host(_) => None,
        };
        if let Some(message) = error {
            return Err(EvaluationError::Host(QueryError::query(
                stage.range.clone(),
                message,
            )));
        }
        Ok(())
    }

    fn prepare_filter_item(
        &self,
        row: &Row<'a>,
        context: &mut &mut Context<'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), Failure> {
        let mut work = Work { budget };
        // Semantic syntax filters historically charge predicate visits only;
        // semantic rows and ordinary syntax filters also charge one row visit.
        if !context.semantic_filter || context.kind.is_semantic() {
            work.visit(range).map_err(EvaluationError::Host)?;
        }
        if context.semantic_filter {
            context.filter_subject = Some(context.subject(row));
        } else {
            match row {
                Row::Item(item) => {
                    context.syntax_filter = Some((item.kind.as_str().to_owned(), item.name));
                }
                Row::Node(node) => {
                    work.spend(node.node.children().len(), range)
                        .map_err(EvaluationError::Host)?;
                    context.syntax_filter = Some((
                        inspection::kind_name(node.node.kind()),
                        inspection::direct_names(context.input.syntax(), node.node)
                            .first()
                            .copied(),
                    ));
                }
                Row::String(_)
                | Row::Literal(_)
                | Row::Entity(_)
                | Row::Type(_)
                | Row::Instantiation(_) => unreachable!("ordinary filters accept only syntax rows"),
            }
        }
        Ok(())
    }

    fn evaluate_leaf(
        &self,
        leaf: &Leaf<'q>,
        _row: &Row<'a>,
        context: &mut &mut Context<'a>,
        budget: &mut Budget,
        _range: &Range<usize>,
    ) -> Result<Truth, Failure> {
        let (predicate, stage) = leaf;
        if context.semantic_filter {
            let subject = context
                .filter_subject
                .as_ref()
                .expect("semantic filter row was prepared");
            let truth = context
                .report
                .as_mut()
                .unwrap()
                .predicate_leaf(
                    predicate,
                    subject,
                    &mut Work { budget },
                    stage,
                    context.limits,
                )
                .map_err(EvaluationError::Host)?;
            return Ok(match truth {
                semantic::Truth::True => Truth::True,
                semantic::Truth::False => Truth::False,
                semantic::Truth::Unknown => Truth::Unknown,
            });
        }
        let (kind, name) = context
            .syntax_filter
            .as_ref()
            .expect("syntax filter row was prepared");
        let matches = match &predicate.kind {
            PredicateKind::Kind(expected) => kind == expected,
            PredicateKind::Name(expected) => *name == Some(crate::identifier_identity(expected)),
            PredicateKind::Type(_)
            | PredicateKind::Instantiation(_)
            | PredicateKind::SourceKind(_) => {
                return Err(EvaluationError::Host(QueryError::query(
                    predicate.range.clone(),
                    "predicate requires semantic inspection",
                )));
            }
            PredicateKind::Group(_)
            | PredicateKind::Not(_)
            | PredicateKind::And(_)
            | PredicateKind::Or(_) => unreachable!("the core evaluates Boolean nodes"),
        };
        Ok(if matches { Truth::True } else { Truth::False })
    }

    fn evaluate_stage(
        &self,
        stage: &&'q Stage,
        rows: Vec<Row<'a>>,
        context: &mut &mut Context<'a>,
        budget: &mut Budget,
        _range: &Range<usize>,
    ) -> Result<Vec<Row<'a>>, Failure> {
        let stream = context
            .terminal
            .take()
            .unwrap_or_else(|| context.restore_stream(rows));
        let mut work = Work { budget };
        let stream = if let Some(report) = &mut context.report {
            report
                .evaluate_stage(
                    context.input,
                    stream,
                    stage,
                    &mut work,
                    context.limits,
                    &mut context.callable_references,
                )
                .map_err(EvaluationError::Host)?
        } else {
            let SemanticStream::Inspection(result) = stream else {
                unreachable!()
            };
            SemanticStream::Inspection(Box::new(
                crate::evaluate_stage(
                    context.input,
                    context.model,
                    *result,
                    stage,
                    &mut work,
                    context.limits,
                )
                .map_err(EvaluationError::Host)?,
            ))
        };
        Ok(context.take_rows(stream))
    }

    fn identity_key(
        &self,
        row: &Row<'a>,
        _context: &mut &mut Context<'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Identity, Failure> {
        Ok(match row {
            Row::Item(item) => Identity::Node(std::ptr::from_ref(item.node)),
            Row::Node(node) => Identity::Node(std::ptr::from_ref(node.node)),
            Row::String(value) => {
                Work { budget }
                    .spend(value.len().saturating_sub(1), range)
                    .map_err(EvaluationError::Host)?;
                Identity::String(value.clone())
            }
            Row::Entity(entity) => subject_identity(&SemanticSubject::Entity(*entity)),
            Row::Type(row) => subject_identity(&row.subject),
            Row::Instantiation(row) => subject_identity(&row.subject),
            Row::Literal(_) => unreachable!(),
        })
    }

    fn tally_key(
        &self,
        row: Row<'a>,
        _context: &mut &mut Context<'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<String, Failure> {
        let Row::String(value) = row else {
            unreachable!()
        };
        Work { budget }
            .spend(value.len().saturating_sub(1), range)
            .map_err(EvaluationError::Host)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_categories_translate_independently_of_wording_and_keep_locations() {
        let mut allowance = budget(Limits {
            work: 0,
            collection: 0,
            ..Limits::default()
        });
        let mut work = allowance
            .charge(1, zirql::Location::Query(7..11))
            .unwrap_err();
        work.message = "changed work refusal".into();
        assert_eq!(
            core_error(work),
            QueryError::query(7..11, "query work limit exceeded")
        );
        let mut items = allowance
            .check_items(1, zirql::Location::Source(20..23))
            .unwrap_err();
        items.message = "changed stream refusal".into();
        assert_eq!(
            core_error(items),
            QueryError::input(20..23, "query collection limit exceeded")
        );
        assert_eq!(
            core_error(zirql::QueryError::query(3..9, "work limit exceeded")),
            QueryError::query(3..9, "work limit exceeded")
        );
        assert_eq!(
            core_error(zirql::QueryError::source(4..8, "specific source failure")),
            QueryError::input(4..8, "specific source failure")
        );
        let native = QueryError::input(30..35, "native source error");
        assert_eq!(
            evaluation_error(EvaluationError::Host(native.clone())),
            native
        );
    }
}
