//! Concrete native composition adapter; Zirql reads, validates and executes clauses.
use super::*;
use zirql::{MatchHost, ProjectionHost, ValueReaderHost};

type Matching = zirql::NativeMatch<Expression, Guard>;
type Paths = zirql::NativePaths<Stage, Guard>;

#[derive(Debug)]
pub(super) struct PathStage(Option<Box<Paths>>);
impl PathStage {
    pub(super) fn get(&self) -> &Paths {
        self.0.as_ref().expect("retained paths")
    }
}
impl Drop for PathStage {
    fn drop(&mut self) {
        if let Some(paths) = self.0.take() {
            zirql::dispose_paths(*paths);
        }
    }
}

#[derive(Debug)]
pub(super) struct ProjectionStage(Option<zirql::Projection<Expression>>);
impl ProjectionStage {
    pub(super) fn get(&self) -> &zirql::Projection<Expression> {
        self.0.as_ref().expect("retained projection")
    }
}
impl Drop for ProjectionStage {
    fn drop(&mut self) {
        if let Some(projection) = self.0.take() {
            for (_, value) in projection.fields {
                zirql::dispose_native_value(value);
            }
        }
    }
}
#[derive(Debug)]
pub(super) struct MatchStage(Option<Matching>);
impl MatchStage {
    pub(super) fn get(&self) -> &Matching {
        self.0.as_ref().expect("retained match")
    }
}
impl Drop for MatchStage {
    fn drop(&mut self) {
        if let Some(matching) = self.0.take() {
            zirql::dispose_match(matching);
        }
    }
}
#[derive(Debug)]
pub(super) struct Guard {
    operand: Option<Expression>,
    stage: language::Stage,
}
impl Guard {
    fn predicate(&self) -> &language::Predicate {
        let language::StageKind::Filter(predicate) = &self.stage.kind else {
            unreachable!("guard leaf")
        };
        predicate
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(operand) = self.operand.take() {
            zirql::dispose_expression(operand);
        }
    }
}

impl Parser<'_> {
    pub(super) fn composition_syntax(&self, range: Range<usize>) -> bool {
        use zincite_syntax::TokenKind as T;
        let tokens: Vec<_> = self
            .lexed
            .tokens()
            .iter()
            .filter(|t| {
                t.range.start >= range.start
                    && t.range.end <= range.end
                    && !matches!(t.kind, T::Whitespace | T::LineComment | T::BlockComment)
            })
            .collect();
        let text = |t: &&zincite_syntax::Token| {
            crate::identifier_identity(&self.cursor.source[t.range.clone()])
        };
        let first = tokens.first().map(&text).unwrap_or("");
        let contextual_match = tokens
            .windows(2)
            .any(|pair| text(&pair[0]) == "match" && pair[1].kind == T::LeftBrace);
        let projection = tokens.windows(3).any(|pair| {
            pair[0].kind == T::Pipe
                && matches!(text(&pair[1]), "project" | "paths")
                && pair[2].kind == T::LeftParen
        });
        contextual_match
            || projection
            || (domain_stage(first) && first != "query" && !self.names.contains_key(first))
            || matches!(
                self.names.get(first),
                Some(Kind::Capture(_) | Kind::Captures(_))
            )
    }

    pub(super) fn composition_expression(
        &mut self,
        range: Range<usize>,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        self.begin_pure(range.clone())?;
        let mut budget =
            std::mem::replace(&mut self.preflight, Budget::for_program(self.limits.core()));
        let mut reader = zirql::ArgumentReader::new(self.cursor.source, range.start)
            .map_err(pipeline::core_error)?;
        let parsed =
            zirql::read_native_expression(&mut reader, self, &mut budget, self.limits.nesting);
        self.preflight = budget;
        let mut expression = parsed.map_err(pipeline::core_error)?;
        if let Some(error) = self.composition_error.take() {
            zirql::dispose_expression(expression);
            return Err(error);
        }
        self.cursor.position = reader.position();
        self.cursor.space();
        if self.cursor.position != range.end {
            zirql::dispose_expression(expression);
            return Err(self.cursor.error("expected one native expression"));
        }
        let host = NativeHost {
            queries: &self.queries,
            pipelines: Vec::new(),
            editing: false,
            nesting: self.limits.nesting,
            limits: self.limits.query(),
        };
        let terminal = if let ExpressionKind::Sequence { steps, .. } = &mut expression.value {
            if steps
                .last()
                .is_some_and(|step| matches!(step.value, ExpressionStep::Emit { .. }))
            {
                steps.pop()
            } else {
                None
            }
        } else {
            None
        };
        let emits = terminal.is_some();
        let result = zirql::native_expression_kind(
            &expression,
            &CompositionHost(&host),
            &Kind::Nodes,
            &self.names,
            &mut self.preflight,
        );
        if let (ExpressionKind::Sequence { steps, .. }, Some(terminal)) =
            (&mut expression.value, terminal)
        {
            steps.push(terminal);
        }
        match result {
            Ok(kind) => Ok((expression, kind, emits)),
            Err(error) => {
                zirql::dispose_expression(expression);
                Err(pipeline::core_error(error))
            }
        }
    }
    fn register(&mut self, stage: language::Stage) -> usize {
        let index = self.queries.len();
        let range = stage.range.clone();
        self.queries.push(ProgramQuery {
            query: Query {
                stages: vec![stage],
                range: range.clone(),
                nesting_limit: self.limits.nesting,
            },
            arguments: Vec::new(),
            copy_units: range.len() + 1,
        });
        index
    }
}
fn domain_stage(name: &str) -> bool {
    matches!(
        name,
        "query"
            | "items"
            | "subtree"
            | "children"
            | "expressions"
            | "references"
            | "declarations"
            | "uses"
            | "transitive_references"
            | "referenced_declarations"
            | "paths"
            | "types"
            | "instantiations"
            | "names"
            | "call_names"
            | "annotation_names"
            | "text"
            | "values"
            | "fields"
            | "elements"
            | "keys"
            | "range"
            | "head"
            | "count"
            | "unique"
            | "tally"
            | "filter"
            | "project"
            | "json"
            | "emit"
            | "set_value"
            | "remove"
            | "reduce_enum"
            | "emit_document"
    )
}
impl ValueReaderHost for Parser<'_> {
    type Stage = Stage;
    type SetOperator = ();
    type Emitter = Emitter;
    type Predicate = Guard;
    fn is_binding(&self, name: &str) -> bool {
        self.names.contains_key(name)
    }
    fn is_stage(&self, name: &str) -> bool {
        domain_stage(name)
    }
    fn parse_stage(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<ExpressionStep<Stage, Emitter>, zirql::QueryError> {
        let mut probe = zirql::ArgumentReader::new(reader.source(), reader.position())?;
        if probe.keyword("paths") {
            let paths = zirql::read_paths(reader, self, budget, self.limits.nesting)?;
            self.cursor.position = reader.position();
            return Ok(ExpressionStep::Stage(Stage::Paths(PathStage(Some(
                Box::new(paths),
            )))));
        }
        self.cursor.position = reader.position();
        let start = self.cursor.position;
        if self.cursor.take_word("query") {
            // The existing wrapper lowers checked MiniZinc arguments and maps decoded ranges.
            std::mem::swap(&mut self.preflight, budget);
            let result = self.query_with_kind(None);
            std::mem::swap(&mut self.preflight, budget);
            let (expression, _, emits) = result.map_err(query_error)?;
            if emits {
                zirql::dispose_expression(expression);
                return Err(zirql::QueryError::query(
                    start..self.cursor.position,
                    "constructed expressions cannot emit",
                ));
            }
            let ExpressionKind::Sequence { steps, .. } = expression.value else {
                unreachable!()
            };
            let step = steps.into_iter().next().expect("query wrapper stage").value;
            reader.advance(self.cursor.position - reader.position())?;
            return Ok(step);
        }
        let mut arguments = Vec::new();
        let limits = self.limits;
        let source = self.cursor.source;
        std::mem::swap(&mut self.preflight, budget);
        let mut lower = |owner, kind: language::ArgumentKind, range: Range<usize>| {
            let (expression, names) =
                self.pure_value(range.clone(), comprehension::ExpressionPolicy::Report)?;
            self.value_names(&expression, &range, Some(kind.ty()))?;
            if self.first_report_call.is_none() {
                self.first_report_call = expression.first_report_call.clone();
            }
            self.preflight
                .charge_results(1, zirql::Location::Query(range))
                .map_err(pipeline::core_error)?;
            arguments.push(QueryArgument {
                owner,
                kind,
                expression,
                names,
            });
            Ok(())
        };
        let mut cursor = language::Parser {
            source,
            position: start,
            nesting_limit: limits.nesting.min(64),
            collection_limit: limits.stream_items,
            arguments: Some(&mut lower),
        };
        let parsed = cursor.stage();
        let position = cursor.position;
        std::mem::swap(&mut self.preflight, budget);
        let stage = parsed.map_err(query_error)?;
        self.cursor.position = position;
        reader.advance(position - reader.position())?;
        budget.charge(
            stage.range.len() + 1,
            zirql::Location::Query(stage.range.clone()),
        )?;
        budget.charge_results(1, zirql::Location::Query(stage.range.clone()))?;
        Ok(match stage.kind {
            language::StageKind::Json => ExpressionStep::Emit {
                emitter: Emitter::Json,
                policy: EmissionPolicy::Terminal,
            },
            language::StageKind::Emit => ExpressionStep::Emit {
                emitter: Emitter::Source,
                policy: EmissionPolicy::Terminal,
            },
            _ => {
                let index = self.register(stage);
                self.queries[index].arguments = arguments;
                ExpressionStep::Stage(Stage::Query(index))
            }
        })
    }

    fn parse_predicate_leaf(
        &mut self,
        reader: &mut zirql::ArgumentReader<'_>,
        budget: &mut Budget,
    ) -> Result<Guard, zirql::QueryError> {
        let name = reader.word()?;
        let argument = match name.value {
            "kind" => language::ArgumentKind::Kind,
            "name" => language::ArgumentKind::Name,
            "type" => language::ArgumentKind::Type,
            "instantiation" => language::ArgumentKind::Instantiation,
            "source_kind" => language::ArgumentKind::SourceKind,
            _ => {
                return Err(zirql::QueryError::query(
                    name.range,
                    "unknown native predicate",
                ));
            }
        };
        reader.expect('(')?;
        reader.skip_whitespace();
        let operand = if reader.remaining().starts_with('"') {
            Located::new(ExpressionKind::Input, name.range.clone())
        } else {
            let operand = zirql::read_native_expression(reader, self, budget, self.limits.nesting)?;
            if let Err(error) = reader.expect(',') {
                zirql::dispose_expression(operand);
                return Err(error);
            }
            operand
        };
        let mut guard = Guard {
            operand: Some(operand),
            stage: language::Stage {
                kind: language::StageKind::Filter(language::Predicate {
                    kind: language::PredicateKind::Name(String::new()),
                    range: name.range.clone(),
                }),
                range: name.range.clone(),
            },
        };
        self.cursor.position = reader.position();
        let (value, value_range) = self.cursor.string().map_err(query_error)?;
        reader.advance(self.cursor.position - reader.position())?;
        reader.expect(')')?;
        budget.charge(value.len() + 1, zirql::Location::Query(value_range.clone()))?;
        let predicate = language::Predicate {
            kind: argument
                .predicate(value, value_range)
                .map_err(query_error)?,
            range: name.range.start..reader.position(),
        };
        guard.stage = language::Stage {
            range: predicate.range.clone(),
            kind: language::StageKind::Filter(predicate),
        };
        // Register semantic demand before the program prepares its one context.
        if semantic::requests_facts(&guard.stage) {
            budget.charge(
                guard.stage.range.len() + 1,
                zirql::Location::Query(guard.stage.range.clone()),
            )?;
            budget.charge_results(1, zirql::Location::Query(guard.stage.range.clone()))?;
            self.register(guard.stage.clone());
        }
        Ok(guard)
    }
    fn filter(&mut self, predicate: zirql::Predicate<Guard>, range: Range<usize>) -> Stage {
        let mut pending = vec![&predicate];
        while let Some(predicate) = pending.pop() {
            match &predicate.value {
                zirql::PredicateKind::Leaf(guard) => {
                    if !matches!(
                        guard.operand.as_ref().expect("guard operand").value,
                        ExpressionKind::Input
                    ) {
                        self.composition_error = Some(QueryError::query(
                            predicate.range.clone(),
                            "filter uses its current subject; use an explicit operand in where",
                        ));
                    }
                }
                zirql::PredicateKind::Not(child) | zirql::PredicateKind::Group(child) => {
                    pending.push(child)
                }
                zirql::PredicateKind::And(children) | zirql::PredicateKind::Or(children) => {
                    pending.extend(children)
                }
            }
        }
        fn lower(predicate: zirql::Predicate<Guard>) -> language::Predicate {
            let kind = match predicate.value {
                zirql::PredicateKind::Leaf(mut guard) => {
                    let language::StageKind::Filter(predicate) =
                        std::mem::replace(&mut guard.stage.kind, language::StageKind::Types)
                    else {
                        unreachable!("guard leaf")
                    };
                    predicate.kind
                }
                zirql::PredicateKind::Not(child) => {
                    language::PredicateKind::Not(Box::new(lower(*child)))
                }
                zirql::PredicateKind::Group(child) => {
                    language::PredicateKind::Group(Box::new(lower(*child)))
                }
                zirql::PredicateKind::And(children) => {
                    language::PredicateKind::And(children.into_iter().map(lower).collect())
                }
                zirql::PredicateKind::Or(children) => {
                    language::PredicateKind::Or(children.into_iter().map(lower).collect())
                }
            };
            language::Predicate {
                kind,
                range: predicate.range,
            }
        }
        Stage::Query(self.register(language::Stage {
            kind: language::StageKind::Filter(lower(predicate)),
            range,
        }))
    }
    fn projection(&mut self, projection: zirql::Projection<Expression>, _: Range<usize>) -> Stage {
        Stage::Projection(ProjectionStage(Some(projection)))
    }
    fn matching(
        &mut self,
        matching: Matching,
        _: Range<usize>,
    ) -> Result<Stage, zirql::QueryError> {
        Ok(Stage::Match(MatchStage(Some(matching))))
    }
    fn set_operator(&self, _: &str) -> Option<()> {
        None
    }
    fn parse_path_edge(
        &mut self,
        edge: Located<&str>,
        budget: &mut Budget,
    ) -> Result<Stage, zirql::QueryError> {
        if edge.value != "referenced_declarations" {
            return Err(zirql::QueryError::query(
                edge.range,
                "unsupported path edge",
            ));
        }
        budget.charge_results(1, zirql::Location::Query(edge.range.clone()))?;
        let index = self.register(language::Stage {
            kind: language::StageKind::ReferencedDeclarations,
            range: edge.range,
        });
        Ok(Stage::Query(index))
    }
}
fn query_error(error: QueryError) -> zirql::QueryError {
    zirql::QueryError::query(error.range, error.message)
}

pub(super) struct CompositionHost<'h, 'q>(pub(super) &'h NativeHost<'q>);
impl<'a> ProgramHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Value = ReportValue<'a>;
    type Stage = Stage;
    type SetOperator = ();
    type Emitter = Emitter;
    type Error = QueryError;
    fn copy_value(
        &self,
        value: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0.copy_value(value, context, budget, range)
    }
    fn evaluate_stage(
        &self,
        stage: &Stage,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0
            .evaluate_stage(stage, value, context, bindings, budget, range)
    }
    fn combine(
        &self,
        operator: &(),
        left: Self::Value,
        right: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        self.0
            .combine(operator, left, right, context, budget, range)
    }
    fn construct_record(
        &self,
        fields: Vec<(String, Self::Value)>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        Ok(ReportValue::Capture(zirql::NativeCapture { fields }))
    }
    fn capture_record<'v>(
        &self,
        value: &'v Self::Value,
    ) -> Option<&'v zirql::NativeCapture<Self::Value>> {
        if let ReportValue::Capture(capture) = value {
            Some(capture)
        } else {
            None
        }
    }
    fn emit(
        &self,
        emitter: &Emitter,
        value: &Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::EvaluationError<QueryError>> {
        self.0.emit(emitter, value, context, budget, range)
    }
}
impl<'a> ProjectionHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Kind = Kind;
    fn kind(
        &self,
        value: &ReportValue<'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        Ok(match value {
            ReportValue::Constructed(value) => {
                Kind::Value(Rc::new(vec![comprehension::value_facts(
                    value, budget, range,
                )?]))
            }
            ReportValue::Record(_) => Kind::Record,
            ReportValue::Capture(capture) => {
                let mut fields = Vec::new();
                for (name, value) in &capture.fields {
                    budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
                    fields.push((name.clone(), self.kind(value, budget, range)?));
                }
                Kind::Capture(Rc::new(zirql::NativeCapture { fields }))
            }
            ReportValue::Captures(captures) => {
                Kind::Captures(copy_schema(&captures.schema, budget, range)?)
            }
            ReportValue::Query(stream) => match stream {
                SemanticStream::Entities(rows) => {
                    budget.charge(rows.len(), zirql::Location::Query(range.clone()))?;
                    if rows
                        .iter()
                        .all(|entity| matches!(entity, semantic::SemanticEntity::Declaration(_)))
                    {
                        Kind::Declarations
                    } else {
                        Kind::Entities
                    }
                }
                SemanticStream::Types(_) => Kind::Types,
                SemanticStream::Instantiations(_) => Kind::Instantiations,
                SemanticStream::Inspection(result) => match result.as_ref() {
                    QueryResult::Selection(_) | QueryResult::Nodes(_) => Kind::Nodes,
                    QueryResult::Strings(_) => Kind::Strings,
                    QueryResult::Literals(_) => Kind::Literals,
                    QueryResult::Count(_) => Kind::Count,
                    QueryResult::Tally(_) => Kind::Tally,
                    _ => unreachable!("read-only native values"),
                },
            },
        })
    }
    fn scalar_types(
        &self,
        kind: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<Vec<zirql::ValueType>>, zirql::QueryError> {
        use zirql::ValueType as T;
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        Ok(match kind {
            Kind::Value(types) => Some(zirql::copy_value_types(
                types,
                budget,
                zirql::Location::Query(range.clone()),
            )?),
            Kind::Scalar(ty) => Some(vec![(*ty).into()]),
            Kind::Constructed => Some(vec![T::Unestablished]),
            Kind::Count => Some(vec![T::Integer]),
            Kind::Strings => Some(vec![T::Array(zirql::ArrayType::Elements(vec![T::String]))]),
            Kind::Literals => Some(vec![T::Array(zirql::ArrayType::Elements(vec![
                T::Unestablished,
            ]))]),
            _ => None,
        })
    }
    fn constructed_kind(&self, types: Vec<zirql::ValueType>) -> Kind {
        Kind::Value(Rc::new(types))
    }
    fn stage_kind(
        &self,
        stage: &Stage,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        match stage {
            Stage::Query(index) => {
                validate_stages(&self.0.queries[*index].query, input.clone()).map_err(query_error)
            }
            Stage::Projection(projection) => {
                for name in scope.keys() {
                    budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
                }
                zirql::projection_types(
                    projection.get(),
                    self,
                    input.clone(),
                    scope.clone(),
                    budget,
                )
                .map(|types| Kind::Value(Rc::new(types)))
            }
            Stage::Match(matching) => {
                zirql::match_types(matching.get(), self, input, scope, budget)
                    .map(|schema| Kind::Captures(Rc::new(schema)))
            }
            Stage::Paths(paths) => zirql::path_types(paths.get(), self, input, scope, budget)
                .map(|schema| Kind::Captures(Rc::new(schema))),
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "stage is unavailable in native composition",
            )),
        }
    }
    fn is_pure(&self, stage: &Stage) -> bool {
        match stage {
            Stage::Data(_) => false,
            Stage::Query(index) => {
                !self.0.queries[*index].query.is_editing()
                    && !self.0.queries[*index]
                        .query
                        .stages
                        .iter()
                        .any(|s| matches!(s.kind, language::StageKind::EmitDocument))
            }
            _ => true,
        }
    }
    fn nested_queries(
        &self,
        _: &Stage,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<zirql::NativeOperands<Context<'_, 'a>, Self>, zirql::QueryError> {
        Ok(Vec::new())
    }
    fn subjects(
        &self,
        input: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<Self::Value>, zirql::EvaluationError<QueryError>> {
        if let ReportValue::Captures(captures) = input {
            budget.charge(captures.rows.len(), zirql::Location::Query(range.clone()))?;
            budget.charge_results(captures.rows.len(), zirql::Location::Query(range.clone()))?;
            return Ok(captures
                .rows
                .into_iter()
                .map(ReportValue::Capture)
                .collect());
        }
        if matches!(input, ReportValue::Capture(_)) {
            budget.charge(1, zirql::Location::Query(range.clone()))?;
            budget.charge_results(1, zirql::Location::Query(range.clone()))?;
            return Ok(vec![input]);
        }
        let ReportValue::Query(stream) = input else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "native subjects require a selection",
            )
            .into());
        };
        let len = stream_len(&stream);
        budget.check_items(len, zirql::Location::Query(range.clone()))?;
        let units = len.checked_mul(4).ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "subject allocation size overflow")
        })?;
        budget.charge(units, zirql::Location::Query(range.clone()))?;
        budget.charge_results(units, zirql::Location::Query(range.clone()))?;
        let rows = context.query.take_rows(stream);
        context.query.document = false;
        Ok(rows
            .into_iter()
            .map(|row| ReportValue::Query(context.query.restore_stream(vec![row])))
            .collect())
    }
    fn subject_kind(
        &self,
        input: &Kind,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Kind, zirql::QueryError> {
        match input {
            Kind::Captures(schema) => Ok(Kind::Capture(copy_schema(schema, budget, range)?)),
            Kind::Capture(_)
            | Kind::Nodes
            | Kind::Entities
            | Kind::Declarations
            | Kind::Types
            | Kind::Instantiations
            | Kind::Strings
            | Kind::Literals => Ok(input.clone()),
            _ => Err(zirql::QueryError::query(
                range.clone(),
                "native subjects require a selection",
            )),
        }
    }
    fn capture_record_kind<'k>(&self, kind: &'k Kind) -> Option<&'k zirql::NativeCapture<Kind>> {
        if let Kind::Capture(schema) = kind {
            Some(schema)
        } else {
            None
        }
    }
    fn project_value(
        &self,
        value: Self::Value,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::ConstructedValue, zirql::EvaluationError<QueryError>> {
        let value = match value {
            ReportValue::Constructed(value) => value,
            ReportValue::Query(stream) => comprehension::project(
                &stream,
                context.facts.as_deref(),
                budget,
                range,
                self.0.nesting,
            )
            .map_err(zirql::EvaluationError::Host)?,
            _ => {
                return Err(zirql::QueryError::query(
                    range.clone(),
                    "native handles require an explicit scalar projection",
                )
                .into());
            }
        };
        comprehension::validate_report_shape(&value, budget, range)
            .map_err(zirql::EvaluationError::Host)?;
        Ok(value)
    }
    fn lift_constructed(
        &self,
        value: zirql::ConstructedValue,
        _: Vec<zirql::ValueType>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        Ok(ReportValue::Constructed(value))
    }
}
fn stream_len(stream: &SemanticStream<'_>) -> usize {
    match stream {
        SemanticStream::Entities(rows) => rows.len(),
        SemanticStream::Types(rows) => rows.len(),
        SemanticStream::Instantiations(rows) => rows.len(),
        SemanticStream::Inspection(result) => match result.as_ref() {
            QueryResult::Selection(rows) => rows.items.len(),
            QueryResult::Nodes(rows) => rows.nodes.len(),
            QueryResult::Strings(rows) => rows.len(),
            QueryResult::Literals(rows) => rows.values.len(),
            _ => 0,
        },
    }
}
pub(super) fn copy_schema(
    schema: &zirql::NativeCapture<Kind>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Rc<zirql::NativeCapture<Kind>>, zirql::QueryError> {
    budget.charge(schema.fields.len(), zirql::Location::Query(range.clone()))?;
    budget.charge_results(schema.fields.len(), zirql::Location::Query(range.clone()))?;
    let mut fields = Vec::with_capacity(schema.fields.len());
    for (name, kind) in &schema.fields {
        budget.charge(name.len() + 1, zirql::Location::Query(range.clone()))?;
        let kind = match kind {
            Kind::Captures(schema) => Kind::Captures(copy_schema(schema, budget, range)?),
            Kind::Capture(schema) => Kind::Capture(copy_schema(schema, budget, range)?),
            Kind::Value(types) => Kind::Value(Rc::new(zirql::copy_value_types(
                types,
                budget,
                zirql::Location::Query(range.clone()),
            )?)),
            kind => kind.clone(),
        };
        fields.push((name.clone(), kind));
    }
    Ok(Rc::new(zirql::NativeCapture { fields }))
}
impl<'a> MatchHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Guard = Guard;
    fn validate_guard_leaf(
        &self,
        guard: &Guard,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        let kind = zirql::native_expression_kind(
            guard.operand.as_ref().expect("guard operand"),
            self,
            input,
            scope,
            budget,
        )?;
        if !matches!(
            kind,
            Kind::Nodes | Kind::Entities | Kind::Declarations | Kind::Types | Kind::Instantiations
        ) {
            return Err(zirql::QueryError::query(
                range.clone(),
                "semantic guard requires a native subject",
            ));
        }
        Ok(())
    }
    fn evaluate_guard_leaf(
        &self,
        guard: &Guard,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        let operand = guard.operand.as_ref().expect("guard operand");
        let relative = self.copy_value(input, context, budget, range)?;
        let value = zirql::evaluate_expression(operand, self, relative, context, bindings, budget)?;
        let ReportValue::Query(stream) = value else {
            return Err(zirql::QueryError::query(
                range.clone(),
                "semantic guard requires a native subject",
            )
            .into());
        };
        let len = stream_len(&stream);
        if len != 1 {
            return Err(zirql::QueryError::query(
                operand.range.clone(),
                "required guard needs exactly one subject",
            )
            .into());
        }
        budget.charge(len, zirql::Location::Query(range.clone()))?;
        budget.charge_results(len, zirql::Location::Query(range.clone()))?;
        let row = context
            .query
            .take_rows(stream)
            .pop()
            .expect("one guard subject");
        if context.query.report.is_none() {
            use zirql::Host;
            let leaf = (guard.predicate(), &guard.stage);
            let stage = Located::new(
                zirql::Stage::Filter(Located::new(
                    zirql::PredicateKind::Leaf(leaf),
                    range.clone(),
                )),
                range.clone(),
            );
            let mut query = &mut *context.query;
            pipeline::NativeHost.prepare_stage(&stage, &mut query, budget)?;
            pipeline::NativeHost.prepare_filter_item(&row, &mut query, budget, range)?;
            return pipeline::NativeHost.evaluate_leaf(&leaf, &row, &mut query, budget, range);
        }
        let subject = context.query.subject(&row);
        let report = context.query.report.as_mut().ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "semantic guard requires model facts")
        })?;
        let (truth, diagnostic) = report
            .guard_leaf(
                guard.predicate(),
                &subject,
                &mut Work { budget },
                &guard.stage,
                self.0.limits,
            )
            .map_err(zirql::EvaluationError::Host)?;
        if let Some(diagnostic) = diagnostic {
            context.guard_unknown = Some(diagnostic);
        }
        Ok(match truth {
            semantic::Truth::True => zirql::Truth::True,
            semantic::Truth::False => zirql::Truth::False,
            semantic::Truth::Unknown => zirql::Truth::Unknown,
        })
    }
}
impl<'a> CompositionHost<'_, '_> {
    pub(super) fn paths(
        &self,
        paths: &Paths,
        input: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let kind = self.kind(&input, budget, &paths.range)?;
        let mut scope = BTreeMap::new();
        for (name, value) in bindings.iter() {
            budget.charge(name.len() + 1, zirql::Location::Query(paths.range.clone()))?;
            scope.insert(name.to_owned(), self.kind(value, budget, &paths.range)?);
        }
        let schema = zirql::path_types(paths, self, &kind, &scope, budget)?;
        let previous = context.guard_unknown.take();
        let result = zirql::evaluate_paths(paths, self, input, context, bindings, budget);
        let responsible = std::mem::replace(&mut context.guard_unknown, previous);
        let rows = result.map_err(|error| predicate_error(error, responsible))?;
        budget.charge(rows.len() + 1, zirql::Location::Query(paths.range.clone()))?;
        budget.charge_results(rows.len() + 1, zirql::Location::Query(paths.range.clone()))?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let ReportValue::Capture(capture) = row else {
                    unreachable!("native path record")
                };
                capture
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
    pub(super) fn matching(
        &self,
        matching: &Matching,
        input: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let kind = self.kind(&input, budget, &matching.range)?;
        let mut scope = BTreeMap::new();
        for (name, value) in bindings.iter() {
            budget.charge(
                name.len() + 1,
                zirql::Location::Query(matching.range.clone()),
            )?;
            scope.insert(name.to_owned(), self.kind(value, budget, &matching.range)?);
        }
        let schema = zirql::match_types(matching, self, &kind, &scope, budget)?;
        let previous = context.guard_unknown.take();
        let result = zirql::evaluate_match(matching, self, input, context, bindings, budget);
        let responsible = std::mem::replace(&mut context.guard_unknown, previous);
        let rows = result.map_err(|error| predicate_error(error, responsible))?;
        budget.charge(
            rows.len() + 1,
            zirql::Location::Query(matching.range.clone()),
        )?;
        budget.charge_results(
            rows.len() + 1,
            zirql::Location::Query(matching.range.clone()),
        )?;
        let rows = rows
            .into_iter()
            .map(|row| {
                let ReportValue::Capture(capture) = row else {
                    unreachable!("composition host constructs captures")
                };
                capture
            })
            .collect();
        Ok(ReportValue::Captures(CaptureMatches {
            rows,
            schema: Rc::new(schema),
        }))
    }
}

fn path_declaration(
    value: &ReportValue<'_>,
    range: &Range<usize>,
) -> Result<semantic::SemanticEntity, zirql::QueryError> {
    if let ReportValue::Query(SemanticStream::Entities(rows)) = value
        && let [entity @ semantic::SemanticEntity::Declaration(_)] = rows.as_slice()
    {
        return Ok(*entity);
    }
    Err(zirql::QueryError::query(
        range.clone(),
        "paths require native declarations",
    ))
}

impl<'a> zirql::PathHost<Context<'_, 'a>> for CompositionHost<'_, '_> {
    type Target = Guard;
    fn validate_path_edge(
        &self,
        edge: &Stage,
        input: &Kind,
        _: &BTreeMap<String, Kind>,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        if let Stage::Query(index) = edge
            && matches!(input, Kind::Declarations)
            && matches!(
                self.0.queries[*index].query.stages.as_slice(),
                [language::Stage {
                    kind: language::StageKind::ReferencedDeclarations,
                    ..
                }]
            )
        {
            return Ok(());
        }
        Err(zirql::QueryError::query(
            range.clone(),
            "path edge requires declarations",
        ))
    }
    fn validate_target_leaf(
        &self,
        target: &Guard,
        input: &Kind,
        scope: &BTreeMap<String, Kind>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::QueryError> {
        self.validate_guard_leaf(target, input, scope, budget, range)
    }
    fn evaluate_target_leaf(
        &self,
        target: &Guard,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::Truth, zirql::EvaluationError<QueryError>> {
        path_declaration(input, range)?;
        self.evaluate_guard_leaf(target, input, context, bindings, budget, range)
    }
    fn path_neighbors(
        &self,
        edge: &Stage,
        input: &Self::Value,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<Self::Value>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<Self::Value>, zirql::EvaluationError<QueryError>> {
        path_declaration(input, range)?;
        let input = self.copy_value(input, context, budget, range)?;
        let neighbors = self
            .0
            .evaluate_stage(edge, input, context, bindings, budget, range)?;
        self.subjects(neighbors, context, budget, range)
    }
    fn same_path_identity(
        &self,
        left: &Self::Value,
        right: &Self::Value,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<bool, zirql::EvaluationError<QueryError>> {
        Ok(path_declaration(left, range)? == path_declaration(right, range)?)
    }
    fn path_steps(
        &self,
        steps: Vec<Self::Value>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Self::Value, zirql::EvaluationError<QueryError>> {
        budget.check_items(steps.len(), zirql::Location::Query(range.clone()))?;
        budget.charge(steps.len(), zirql::Location::Query(range.clone()))?;
        budget.charge_results(steps.len() + 1, zirql::Location::Query(range.clone()))?;
        let mut declarations = Vec::with_capacity(steps.len());
        for step in steps {
            declarations.push(path_declaration(&step, range)?);
        }
        Ok(ReportValue::Query(SemanticStream::Entities(declarations)))
    }
}

fn predicate_error(
    error: zirql::EvaluationError<QueryError>,
    responsible: Option<SourceDiagnostic>,
) -> zirql::EvaluationError<QueryError> {
    match error {
        zirql::EvaluationError::Core(error)
            if error.category == zirql::QueryErrorCategory::UnknownPredicate =>
        {
            match responsible {
                Some(diagnostic) => zirql::EvaluationError::Host(QueryError {
                    range: diagnostic.location.range.clone(),
                    location: crate::ErrorLocation::Model(diagnostic.location),
                    message: diagnostic.message,
                }),
                None => zirql::EvaluationError::Core(error),
            }
        }
        error => error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_category_uses_the_responsible_model_diagnostic_or_retains_core_error() {
        let responsible = SourceDiagnostic {
            location: zincite_lint::SourceLocation {
                path: "included/model.mzn".into(),
                range: 20..27,
                line: 3,
                column: 5,
            },
            message: "model predicate is uncertain".into(),
        };
        let error = zirql::QueryError::query(7..14, "changed Unknown refusal")
            .with_category(zirql::QueryErrorCategory::UnknownPredicate);
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Core(error.clone()),
                Some(responsible.clone())
            ),
            zirql::EvaluationError::Host(QueryError {
                range: 20..27,
                location: crate::ErrorLocation::Model(responsible.location.clone()),
                message: responsible.message.clone(),
            })
        );
        assert_eq!(
            predicate_error(zirql::EvaluationError::Core(error.clone()), None),
            zirql::EvaluationError::Core(error)
        );
        let other = zirql::QueryError::query(8..15, "path target is Unknown");
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Core(other.clone()),
                Some(responsible.clone())
            ),
            zirql::EvaluationError::Core(other)
        );
        let native = QueryError::input(30..35, "native source error");
        assert_eq!(
            predicate_error(
                zirql::EvaluationError::Host(native.clone()),
                Some(responsible)
            ),
            zirql::EvaluationError::Host(native)
        );
    }
}
