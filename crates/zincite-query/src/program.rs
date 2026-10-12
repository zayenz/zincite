//! Ordered read-only programs over native query results.
use crate::{
    Input, Limits, Query, QueryError, QueryResult, Selection, SemanticResult, SemanticStream, Work,
    inspection, language, pipeline, semantic,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ops::Range, rc::Rc};
use zincite_lint::{ModelContext, ModelFacts, SourceDiagnostic};
use zirql::{
    Bindings, Budget, EmissionPolicy, ExpressionKind, ExpressionStep, Located, NativeInput,
    NativeValue, ProgramHost,
};

type Expression = zirql::Expression<usize, (), Emitter>;

/// Program bounds retain the single-pipeline stream ceiling and add a request
/// allowance for produced/copied native entries. Model tokens consume work.
#[derive(Clone, Copy, Debug)]
pub struct ProgramLimits {
    pub work: usize,
    pub results: usize,
    pub nesting: usize,
    pub stream_items: usize,
}
impl Default for ProgramLimits {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            work: limits.work,
            results: 100_000,
            nesting: limits.nesting,
            stream_items: limits.collection,
        }
    }
}
impl ProgramLimits {
    fn query(self) -> Limits {
        Limits {
            work: self.work,
            collection: self.stream_items,
            nesting: self.nesting,
        }
    }
    fn core(self) -> zirql::ProgramLimits {
        zirql::ProgramLimits {
            work: self.work,
            results: self.results,
            nesting: self.nesting,
            stream_items: self.stream_items,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Emitter {
    Source,
    Json,
}
#[derive(Clone, Copy, Debug)]
enum Kind {
    Nodes,
    Strings,
    Literals,
    Count,
    Tally,
    Record,
    Entities,
    Types,
    Instantiations,
}

/// Native report leaves retain source selections and semantic identities. A
/// constructed record has no input-source range; labels preserve written order.
#[derive(Debug)]
pub enum ReportValue<'a> {
    Query(SemanticStream<'a>),
    Record(Vec<(String, ReportValue<'a>)>),
}
impl<'a> ReportValue<'a> {
    pub fn fields(&self) -> Option<&[(String, ReportValue<'a>)]> {
        match self {
            Self::Record(fields) => Some(fields),
            Self::Query(_) => None,
        }
    }
    pub fn stream(&self) -> Option<&SemanticStream<'a>> {
        match self {
            Self::Query(stream) => Some(stream),
            Self::Record(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct ProgramEmission<'a> {
    value: ReportValue<'a>,
    emitter: Emitter,
    range: Range<usize>,
}
impl<'a> ProgramEmission<'a> {
    pub fn value(&self) -> &ReportValue<'a> {
        &self.value
    }
    pub fn is_json(&self) -> bool {
        self.emitter == Emitter::Json
    }
}

/// Successful evaluation returns all emission requests together. No callback
/// runs during evaluation. Rendering spends the same request budget and callers
/// should commit the returned bytes only after rendering succeeds.
#[derive(Debug)]
pub struct ProgramResult<'a> {
    emissions: Vec<ProgramEmission<'a>>,
    semantic: Option<SemanticResult<'a>>,
    budget: Budget,
}
impl<'a> ProgramResult<'a> {
    pub fn emissions(&self) -> &[ProgramEmission<'a>] {
        &self.emissions
    }
    pub fn semantic(&self) -> Option<&SemanticResult<'a>> {
        self.semantic.as_ref()
    }
    pub fn limitations(&self) -> &[SourceDiagnostic] {
        self.semantic
            .as_ref()
            .map_or(&[], SemanticResult::limitations)
    }
    pub fn is_complete(&self) -> bool {
        self.limitations().is_empty()
    }

    /// Render all requested outputs into one buffer. Repeated rendering charges
    /// the allowance again; a failure returns no partial bytes.
    pub fn render(&mut self) -> Result<Vec<u8>, QueryError> {
        let mut bytes = Vec::new();
        for emission in &self.emissions {
            let mut output = match (&emission.emitter, &emission.value) {
                (Emitter::Source, ReportValue::Query(SemanticStream::Inspection(result))) => {
                    charge_render(
                        &emission.value,
                        self.semantic.as_ref(),
                        &mut self.budget,
                        &emission.range,
                    )?;
                    if let QueryResult::Tally(values) = result.as_ref() {
                        let mut output = Vec::new();
                        serde_json::to_writer(
                            BudgetWriter {
                                bytes: &mut output,
                                budget: &mut self.budget,
                                range: &emission.range,
                            },
                            values,
                        )
                        .map_err(|error| {
                            QueryError::query(emission.range.clone(), error.to_string())
                        })?;
                        self.budget
                            .charge(1, zirql::Location::Query(emission.range.clone()))
                            .map_err(pipeline::core_error)?;
                        output.push(b'\n');
                        output
                    } else {
                        result.render()
                    }
                }
                (Emitter::Source, ReportValue::Query(stream)) => {
                    charge_render(
                        &emission.value,
                        self.semantic.as_ref(),
                        &mut self.budget,
                        &emission.range,
                    )?;
                    self.semantic.as_ref().unwrap().render_stream(stream)
                }
                _ => {
                    admit_json(
                        &emission.value,
                        self.semantic.as_ref(),
                        &mut Work {
                            budget: &mut self.budget,
                        },
                        &emission.range,
                    )?;
                    // The envelope owns completion metadata and limitation rows.
                    self.budget
                        .charge_results(
                            8 + self.limitations().len() * 18,
                            zirql::Location::Query(emission.range.clone()),
                        )
                        .map_err(pipeline::core_error)?;
                    for limitation in self
                        .semantic
                        .as_ref()
                        .map_or(&[][..], SemanticResult::limitations)
                    {
                        self.budget
                            .charge(
                                limitation.location.path.as_os_str().len()
                                    + limitation.message.len()
                                    + 96,
                                zirql::Location::Query(emission.range.clone()),
                            )
                            .map_err(pipeline::core_error)?;
                    }
                    let value = report_json(&emission.value, self.semantic.as_ref())?;
                    let envelope = json!({"complete": self.is_complete(), "limitations": self.limitations().iter().map(|d| json!({"file": d.location.path.to_string_lossy(), "range": {"start": d.location.range.start, "end": d.location.range.end}, "line": d.location.line, "column": d.location.column, "message": d.message})).collect::<Vec<_>>(), "result": value});
                    let mut output = Vec::new();
                    serde_json::to_writer(
                        BudgetWriter {
                            bytes: &mut output,
                            budget: &mut self.budget,
                            range: &emission.range,
                        },
                        &envelope,
                    )
                    .map_err(|error| {
                        QueryError::query(emission.range.clone(), error.to_string())
                    })?;
                    self.budget
                        .charge(1, zirql::Location::Query(emission.range.clone()))
                        .map_err(pipeline::core_error)?;
                    output.push(b'\n');
                    output
                }
            };
            // JSON's writer charged encoded bytes before appending. Source
            // output was admitted by charge_render before its native renderer.
            bytes.append(&mut output);
        }
        Ok(bytes)
    }
}
struct BudgetWriter<'a> {
    bytes: &'a mut Vec<u8>,
    budget: &'a mut Budget,
    range: &'a Range<usize>,
}
impl std::io::Write for BudgetWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.budget
            .charge(bytes.len(), zirql::Location::Query(self.range.clone()))
            .map_err(std::io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A parsed, read-only program. Query-source strings lower once with their
/// original program byte coordinates; execution never parses generated text.
#[derive(Debug)]
pub struct Program {
    program: zirql::Program<usize, (), Emitter>,
    queries: Vec<Query>,
}
impl Program {
    /// Parse immutable source bindings, query statements and record reports.
    /// All names, terminal emitters and supported result transitions are checked
    /// before any statement executes.
    pub fn parse(source: &str, limits: ProgramLimits) -> Result<Self, QueryError> {
        if source.len() > 1_048_576 {
            return Err(QueryError::query(
                0..0,
                "program byte limit exceeded (1048576 bytes)",
            ));
        }
        let mut parser = Parser {
            cursor: language::Parser {
                source,
                position: 0,
                nesting_limit: limits.nesting.min(64),
            },
            limits,
            names: BTreeMap::new(),
            queries: Vec::new(),
        };
        let mut statements = Vec::new();
        parser.cursor.space();
        while parser.cursor.position < source.len() {
            let start = parser.cursor.position;
            let binding = if parser.cursor.take_word("source") {
                let (name, range) = parser.identifier("expected a source binding name")?;
                if matches!(name, "source" | "report" | "query" | "json" | "emit") {
                    return Err(QueryError::query(
                        range,
                        "source binding name is reserved by the program grammar",
                    ));
                }
                if parser.names.contains_key(name) {
                    return Err(QueryError::query(range, "binding is already defined"));
                }
                parser
                    .cursor
                    .punctuation(b'=', "expected '=' after source name")?;
                Some(Located::new(name.to_owned(), range))
            } else {
                None
            };
            let report = binding.is_none() && parser.cursor.take_word("report");
            let (expression, kind, emits) = parser.expression(0, Kind::Nodes)?;
            if binding.is_some() && emits {
                return Err(QueryError::query(
                    expression.range.clone(),
                    "a source binding cannot emit",
                ));
            }
            parser
                .cursor
                .punctuation(b';', "expected ';' after program statement")?;
            let statement = if let Some(name) = binding {
                parser.names.insert(name.value.clone(), kind);
                zirql::Statement::Bind { name, expression }
            } else {
                zirql::Statement::Evaluate {
                    expression,
                    implicit_output: Some(if report || matches!(kind, Kind::Record) {
                        Emitter::Json
                    } else {
                        Emitter::Source
                    }),
                }
            };
            statements.push(Located::new(statement, start..parser.cursor.position));
            parser.cursor.space();
        }
        if statements.is_empty() {
            return Err(QueryError::query(
                0..source.len(),
                "expected a program statement",
            ));
        }
        Ok(Self {
            program: zirql::Program::new(statements, 0..source.len()),
            queries: parser.queries,
        })
    }
    pub fn requires_model_facts(&self) -> bool {
        self.queries.iter().any(Query::requires_model_facts)
    }
    pub fn evaluate<'a>(
        &self,
        input: &'a Input,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.evaluate_with_model(input, None, limits)
    }
    pub fn evaluate_with_model<'a>(
        &self,
        input: &'a Input,
        model: Option<&'a ModelContext>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        let facts = if self.requires_model_facts() {
            model.map(|model| Rc::new(ModelFacts::new(model)))
        } else {
            None
        };
        self.run(input, facts, limits)
    }
    pub fn evaluate_with_facts<'a>(
        &self,
        input: &'a Input,
        facts: Rc<ModelFacts<'a>>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.run(input, Some(facts), limits)
    }
    fn run<'a>(
        &self,
        input: &'a Input,
        facts: Option<Rc<ModelFacts<'a>>>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        let mut budget = Budget::for_program(limits.core());
        let empty = Query {
            stages: Vec::new(),
            range: self.program.range(),
            nesting_limit: limits.nesting,
        };
        let demand = self
            .queries
            .iter()
            .find(|query| query.requires_model_facts())
            .unwrap_or(&empty);
        let (mut context, rows) =
            pipeline::prepare_context(demand, input, facts, limits.query(), &mut budget)?;
        let input = ReportValue::Query(context.restore_stream(rows));
        let pipelines = self
            .queries
            .iter()
            .map(pipeline::lower_pipeline)
            .collect::<Result<Vec<_>, _>>()?;
        let host = NativeHost { pipelines };
        let mut context = Context {
            query: &mut context,
            emissions: Vec::new(),
        };
        zirql::evaluate_program(
            &self.program,
            &host,
            input,
            &mut context,
            Bindings::default(),
            &mut budget,
        )
        .map_err(pipeline::evaluation_error)?;
        let emissions = context.emissions;
        let semantic = if let Some(mut report) = context.query.report.take() {
            report.finish(
                std::mem::take(&mut context.query.callable_references),
                &mut Work {
                    budget: &mut budget,
                },
                demand
                    .stages
                    .iter()
                    .find(|stage| semantic::requests_facts(stage))
                    .unwrap(),
                &self.program.range(),
                limits.query(),
            )?;
            Some(report)
        } else {
            None
        };
        for emission in &emissions {
            validate_value(
                &emission.value,
                emission.emitter == Emitter::Json
                    || matches!(emission.value, ReportValue::Record(_)),
                &mut budget,
                &emission.range,
            )?;
        }
        Ok(ProgramResult {
            emissions,
            semantic,
            budget,
        })
    }
}

struct Parser<'a> {
    cursor: language::Parser<'a>,
    limits: ProgramLimits,
    names: BTreeMap<String, Kind>,
    queries: Vec<Query>,
}
impl<'a> Parser<'a> {
    fn identifier(&mut self, message: &str) -> Result<(&'a str, Range<usize>), QueryError> {
        self.cursor.space();
        if self.cursor.current() != Some(b'\'') {
            return self.cursor.word(message);
        }
        let start = self.cursor.position;
        self.cursor.position += 1;
        let contents = self.cursor.position;
        while self
            .cursor
            .current()
            .is_some_and(|byte| !matches!(byte, b'\'' | b'\r' | b'\n' | 0))
        {
            self.cursor.position += 1;
        }
        if self.cursor.current() != Some(b'\'') || self.cursor.position == contents {
            return Err(self.cursor.error("expected a nonempty quoted identifier"));
        }
        let end = self.cursor.position;
        self.cursor.position += 1;
        Ok((
            &self.cursor.source[contents..end],
            start..self.cursor.position,
        ))
    }

    fn expression(
        &mut self,
        depth: usize,
        input: Kind,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        if depth > self.cursor.nesting_limit {
            return Err(self.cursor.error("query nesting limit exceeded"));
        }
        self.cursor.space();
        let start = self.cursor.position;
        let (mut expression, mut kind, mut emits) = if self.cursor.take_word("query") {
            self.query(input)?
        } else if self.cursor.current() == Some(b'(') {
            self.cursor.position += 1;
            self.cursor.space();
            let saved = self.cursor.position;
            let record = self
                .identifier("expected a record label or expression")
                .is_ok()
                && {
                    self.cursor.space();
                    self.cursor.current() == Some(b':')
                };
            self.cursor.position = saved;
            if record {
                let mut fields = Vec::new();
                let mut labels = std::collections::BTreeSet::new();
                loop {
                    let (label, range) = self.identifier("expected a record label")?;
                    if !labels.insert(label.to_owned()) {
                        return Err(QueryError::query(range, "duplicate report label"));
                    }
                    let label = Located::new(label.to_owned(), range);
                    self.cursor
                        .punctuation(b':', "expected ':' after report label")?;
                    let (value, _, emits) = self.expression(depth + 1, input)?;
                    if emits {
                        return Err(QueryError::query(value.range, "a report field cannot emit"));
                    }
                    fields.push((label, value));
                    self.cursor.space();
                    if self.cursor.current() != Some(b',') {
                        break;
                    }
                    self.cursor.position += 1;
                }
                self.cursor
                    .punctuation(b')', "expected ')' after report fields")?;
                (
                    Located::new(ExpressionKind::Record(fields), start..self.cursor.position),
                    Kind::Record,
                    false,
                )
            } else {
                let (expression, kind, emits) = self.expression(depth + 1, input)?;
                self.cursor
                    .punctuation(b')', "expected ')' after grouped expression")?;
                (
                    Located::new(
                        ExpressionKind::Group(Box::new(expression)),
                        start..self.cursor.position,
                    ),
                    kind,
                    emits,
                )
            }
        } else {
            let (name, range) =
                self.identifier("expected query(...), a binding or a report record")?;
            let kind = *self.names.get(name).ok_or_else(|| {
                QueryError::query(range.clone(), format!("undefined binding: {name}"))
            })?;
            (
                Located::new(ExpressionKind::Binding(name.to_owned()), range),
                kind,
                false,
            )
        };
        let mut steps = Vec::new();
        self.cursor.space();
        while self.cursor.current() == Some(b'|') {
            if emits {
                return Err(self.cursor.error("no stage may follow an emitter"));
            }
            self.cursor.position += 1;
            self.cursor.space();
            let step_start = self.cursor.position;
            if self.cursor.take_word("query") {
                let (next, next_kind, next_emits) = self.query(kind)?;
                let ExpressionKind::Sequence {
                    steps: next_steps, ..
                } = next.value
                else {
                    unreachable!()
                };
                steps.extend(next_steps);
                kind = next_kind;
                emits = next_emits;
            } else {
                let emitter = if self.cursor.take_word("json") {
                    Emitter::Json
                } else if self.cursor.take_word("emit") {
                    Emitter::Source
                } else {
                    return Err(self
                        .cursor
                        .error("expected query(...), json or emit after '|'"));
                };
                steps.push(Located::new(
                    ExpressionStep::Emit {
                        emitter,
                        policy: EmissionPolicy::Terminal,
                    },
                    step_start..self.cursor.position,
                ));
                emits = true;
            }
            self.cursor.space();
        }
        if !steps.is_empty() {
            expression = Located::new(
                ExpressionKind::Sequence {
                    input: Box::new(expression),
                    steps,
                },
                start..self.cursor.position,
            );
        }
        Ok((expression, kind, emits))
    }
    fn query(&mut self, input: Kind) -> Result<(Expression, Kind, bool), QueryError> {
        let start = self.cursor.position.saturating_sub(5);
        self.cursor.punctuation(b'(', "expected '(' after query")?;
        let (source, written) = self.cursor.string()?;
        let mapping = string_mapping(self.cursor.source, &written);
        let map = |range: Range<usize>| {
            mapping[range.start.min(mapping.len() - 1)]..mapping[range.end.min(mapping.len() - 1)]
        };
        let mut query = Query::parse(&source, self.limits.query()).map_err(|mut error| {
            error.range = map(error.range);
            error
        })?;
        if query.is_editing()
            || query
                .stages
                .iter()
                .any(|stage| matches!(stage.kind, language::StageKind::EmitDocument))
        {
            return Err(QueryError::query(
                written,
                "programs accept read-only query stages",
            ));
        }
        for stage in &mut query.stages {
            stage.range = map(stage.range.clone());
            if let language::StageKind::Filter(predicate) = &mut stage.kind {
                map_predicate(predicate, &map);
            }
        }
        query.range = map(query.range);
        self.cursor
            .punctuation(b')', "expected ')' after query string")?;
        let kind = validate_stages(&query, input)?;
        let emitter = match query.stages.last().map(|stage| &stage.kind) {
            Some(language::StageKind::Json) => Some(Emitter::Json),
            Some(language::StageKind::Emit) => Some(Emitter::Source),
            _ => None,
        };
        let emitter_range = query.stages.last().map(|stage| stage.range.clone());
        if emitter.is_some() {
            query.stages.pop();
        }
        let index = self.queries.len();
        self.queries.push(query);
        let mut steps = vec![Located::new(
            ExpressionStep::Stage(index),
            start..self.cursor.position,
        )];
        if let Some(emitter) = emitter {
            steps.push(Located::new(
                ExpressionStep::Emit {
                    emitter,
                    policy: EmissionPolicy::Terminal,
                },
                emitter_range.unwrap(),
            ));
        }
        Ok((
            Located::new(
                ExpressionKind::Sequence {
                    input: Box::new(Located::new(
                        ExpressionKind::Input,
                        start..self.cursor.position,
                    )),
                    steps,
                },
                start..self.cursor.position,
            ),
            kind,
            emitter.is_some(),
        ))
    }
}
fn string_mapping(source: &str, range: &Range<usize>) -> Vec<usize> {
    let mut mapping = Vec::new();
    let mut at = range.start + 1;
    while at + 1 < range.end {
        mapping.push(at);
        if source.as_bytes()[at] == b'\\' {
            at += 2;
        } else {
            at += 1;
        }
    }
    mapping.push(range.end - 1);
    mapping
}
fn map_predicate(predicate: &mut language::Predicate, map: &impl Fn(Range<usize>) -> Range<usize>) {
    predicate.range = map(predicate.range.clone());
    match &mut predicate.kind {
        language::PredicateKind::Not(child) | language::PredicateKind::Group(child) => {
            map_predicate(child, map)
        }
        language::PredicateKind::And(children) | language::PredicateKind::Or(children) => {
            for child in children {
                map_predicate(child, map);
            }
        }
        _ => {}
    }
}
fn validate_stages(query: &Query, mut kind: Kind) -> Result<Kind, QueryError> {
    use language::StageKind as S;
    for stage in &query.stages {
        let accepts = match &stage.kind {
            S::Json | S::Emit => true,
            S::Items => {
                let valid = matches!(kind, Kind::Nodes);
                kind = Kind::Nodes;
                valid
            }
            S::Count => {
                let valid = !matches!(kind, Kind::Count | Kind::Tally | Kind::Record);
                kind = Kind::Count;
                valid
            }
            S::Tally => {
                let valid = matches!(kind, Kind::Strings);
                kind = Kind::Tally;
                valid
            }
            S::Unique => !matches!(
                kind,
                Kind::Literals | Kind::Count | Kind::Tally | Kind::Record
            ),
            S::Head(_) => !matches!(kind, Kind::Count | Kind::Tally | Kind::Record),
            S::Range(_) => matches!(kind, Kind::Nodes),
            S::Filter(_) => matches!(
                kind,
                Kind::Nodes | Kind::Entities | Kind::Types | Kind::Instantiations
            ),
            S::References | S::Declarations | S::Uses | S::TransitiveReferences(_) => {
                let valid = matches!(
                    kind,
                    Kind::Nodes | Kind::Entities | Kind::Types | Kind::Instantiations
                );
                kind = Kind::Entities;
                valid
            }
            S::Types => {
                let valid = matches!(
                    kind,
                    Kind::Nodes | Kind::Entities | Kind::Types | Kind::Instantiations
                );
                kind = Kind::Types;
                valid
            }
            S::Instantiations => {
                let valid = matches!(
                    kind,
                    Kind::Nodes | Kind::Entities | Kind::Types | Kind::Instantiations
                );
                kind = Kind::Instantiations;
                valid
            }
            S::CallNames | S::AnnotationNames => {
                let valid = matches!(kind, Kind::Nodes);
                kind = Kind::Strings;
                valid
            }
            S::Names | S::Text => {
                let valid = matches!(
                    kind,
                    Kind::Nodes | Kind::Entities | Kind::Types | Kind::Instantiations
                );
                kind = Kind::Strings;
                valid
            }
            S::Values => {
                let valid = matches!(kind, Kind::Nodes);
                kind = Kind::Literals;
                valid
            }
            S::Fields | S::Elements | S::Keys => matches!(kind, Kind::Nodes),
            S::Expressions | S::Children | S::Subtree => {
                let valid = matches!(kind, Kind::Nodes);
                kind = Kind::Nodes;
                valid
            }
            S::FilterElements(_)
            | S::SetValue(_)
            | S::Remove
            | S::ReduceEnum(_, _)
            | S::EmitDocument => false,
        };
        if !accepts {
            return Err(QueryError::query(
                stage.range.clone(),
                "stage is incompatible with the program result kind",
            ));
        }
    }
    Ok(kind)
}

struct Context<'c, 'a> {
    query: &'c mut pipeline::Context<'a>,
    emissions: Vec<ProgramEmission<'a>>,
}
struct NativeHost<'q> {
    pipelines:
        Vec<zirql::Pipeline<(&'q language::Predicate, &'q language::Stage), &'q language::Stage>>,
}
impl<'a, 'q> ProgramHost<Context<'_, 'a>> for NativeHost<'q> {
    type Value = ReportValue<'a>;
    type Stage = usize;
    type SetOperator = ();
    type Emitter = Emitter;
    type Error = QueryError;
    fn copy_value(
        &self,
        value: &ReportValue<'a>,
        _: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        copy_value(value, budget, range).map_err(zirql::EvaluationError::Host)
    }
    fn evaluate_stage(
        &self,
        stage: &usize,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        _: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        if self.pipelines[*stage].stages().is_empty() {
            return Ok(value);
        }
        let ReportValue::Query(stream) = value else {
            return Err(zirql::EvaluationError::Host(QueryError::query(
                range.clone(),
                "query stages require a native query result",
            )));
        };
        let SemanticStream::Inspection(ref value) = stream else {
            return self.run_pipeline(*stage, stream, context, budget, range);
        };
        if matches!(
            value.as_ref(),
            QueryResult::Count(_) | QueryResult::Tally(_)
        ) {
            if !self.pipelines[*stage].stages().is_empty() {
                return Err(zirql::EvaluationError::Host(QueryError::query(
                    range.clone(),
                    "query stages require a stream",
                )));
            }
            return Ok(ReportValue::Query(stream));
        }
        self.run_pipeline(*stage, stream, context, budget, range)
    }
    fn combine(
        &self,
        _: &(),
        _: ReportValue<'a>,
        _: ReportValue<'a>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        Err(zirql::EvaluationError::Host(QueryError::query(
            range.clone(),
            "set expressions are unsupported in Zincite programs",
        )))
    }
    fn construct_record(
        &self,
        fields: Vec<(String, ReportValue<'a>)>,
        _: &mut Context<'_, 'a>,
        _: &mut Budget,
        _: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        Ok(ReportValue::Record(fields))
    }
    fn emit(
        &self,
        emitter: &Emitter,
        value: &ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<(), zirql::EvaluationError<QueryError>> {
        let value = copy_value(value, budget, range).map_err(zirql::EvaluationError::Host)?;
        budget.charge_results(1, zirql::Location::Query(range.clone()))?;
        context.emissions.push(ProgramEmission {
            value,
            emitter: *emitter,
            range: range.clone(),
        });
        Ok(())
    }
}
impl<'a> NativeHost<'_> {
    fn run_pipeline(
        &self,
        index: usize,
        stream: SemanticStream<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let rows = context.query.take_rows(stream);
        let input = NativeInput::from_admitted(
            &mut *context.query,
            rows,
            budget,
            zirql::Location::Query(range.clone()),
        )?;
        let value =
            zirql::evaluate(&self.pipelines[index], &pipeline::NativeHost, input, budget)?.value;
        let stream = match value {
            NativeValue::Stream(rows) => context.query.restore_stream(rows),
            NativeValue::Count(value) => {
                SemanticStream::Inspection(Box::new(QueryResult::Count(value)))
            }
            NativeValue::Tally(value) => {
                SemanticStream::Inspection(Box::new(QueryResult::Tally(value)))
            }
        };
        Ok(ReportValue::Query(stream))
    }
}

fn copy_value<'a>(
    value: &ReportValue<'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, QueryError> {
    budget
        .charge_results(value_units(value), zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    budget
        .charge(value_bytes(value), zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(copy_admitted(value))
}
fn copy_admitted<'a>(value: &ReportValue<'a>) -> ReportValue<'a> {
    ReportValue::Query(match value {
        ReportValue::Record(fields) => {
            return ReportValue::Record(
                fields
                    .iter()
                    .map(|(label, value)| (label.clone(), copy_admitted(value)))
                    .collect(),
            );
        }
        ReportValue::Query(SemanticStream::Entities(rows)) => {
            SemanticStream::Entities(rows.clone())
        }
        ReportValue::Query(SemanticStream::Types(rows)) => SemanticStream::Types(rows.clone()),
        ReportValue::Query(SemanticStream::Instantiations(rows)) => {
            SemanticStream::Instantiations(rows.clone())
        }
        ReportValue::Query(SemanticStream::Inspection(value)) => {
            SemanticStream::Inspection(Box::new(match value.as_ref() {
                QueryResult::Selection(s) => QueryResult::Selection(Selection {
                    input: s.input,
                    items: s.items.clone(),
                    document: s.document,
                }),
                QueryResult::Nodes(s) => QueryResult::Nodes(crate::NodeSelection {
                    input: s.input,
                    nodes: s.nodes.clone(),
                }),
                QueryResult::Literals(s) => QueryResult::Literals(crate::LiteralSelection {
                    input: s.input,
                    values: s.values.clone(),
                }),
                QueryResult::Strings(s) => QueryResult::Strings(s.clone()),
                QueryResult::Count(s) => QueryResult::Count(*s),
                QueryResult::Tally(s) => QueryResult::Tally(s.clone()),
                _ => unreachable!("programs retain native read-only leaves"),
            }))
        }
    })
}
fn value_units(value: &ReportValue<'_>) -> usize {
    match value {
        ReportValue::Record(fields) => {
            1 + fields
                .iter()
                .map(|(_, value)| 1 + value_units(value))
                .sum::<usize>()
        }
        ReportValue::Query(SemanticStream::Entities(rows)) => rows.len(),
        ReportValue::Query(SemanticStream::Types(rows)) => rows
            .iter()
            .map(|row| {
                1 + row
                    .ty
                    .as_ref()
                    .map_or(0, |ty| semantic::type_allocation(ty).0)
            })
            .sum(),
        ReportValue::Query(SemanticStream::Instantiations(rows)) => rows.len(),
        ReportValue::Query(SemanticStream::Inspection(value)) => match value.as_ref() {
            QueryResult::Selection(s) => s.items.len(),
            QueryResult::Nodes(s) => s.nodes.len(),
            QueryResult::Strings(s) => s.len(),
            QueryResult::Count(_) => 1,
            QueryResult::Tally(s) => 1 + 2 * s.len(),
            QueryResult::Literals(s) => 1 + s.values.iter().map(literal_units).sum::<usize>(),
            _ => unreachable!(),
        },
    }
}
fn literal_units(value: &crate::LiteralValue<'_>) -> usize {
    use crate::LiteralKind as K;
    1 + match &value.kind {
        K::Set(values) | K::Tuple(values) => values.iter().map(literal_units).sum(),
        K::Record(fields) => fields
            .iter()
            .map(|field| 1 + literal_units(&field.value))
            .sum(),
        K::Array { entries, .. } => entries
            .iter()
            .map(|entry| entry.key.as_ref().map_or(0, literal_units) + literal_units(&entry.value))
            .sum(),
        K::Matrix { column_keys, rows } => {
            column_keys.iter().map(literal_units).sum::<usize>()
                + rows
                    .iter()
                    .map(|row| {
                        row.key.as_ref().map_or(0, literal_units)
                            + row.cells.iter().map(literal_units).sum::<usize>()
                    })
                    .sum::<usize>()
        }
        _ => 0,
    }
}
fn value_bytes(value: &ReportValue<'_>) -> usize {
    match value {
        ReportValue::Record(fields) => fields
            .iter()
            .map(|(label, value)| label.len() + value_bytes(value))
            .sum(),
        ReportValue::Query(SemanticStream::Inspection(value)) => match value.as_ref() {
            QueryResult::Strings(values) => values.iter().map(String::len).sum(),
            QueryResult::Tally(values) => values.keys().map(String::len).sum(),
            _ => 0,
        },
        ReportValue::Query(SemanticStream::Types(rows)) => rows
            .iter()
            .map(|row| {
                subject_bytes(&row.subject)
                    + row
                        .ty
                        .as_ref()
                        .map_or(0, |ty| semantic::type_allocation(ty).1)
            })
            .sum(),
        ReportValue::Query(SemanticStream::Instantiations(rows)) => rows
            .iter()
            .map(|row| subject_bytes(&row.subject) + row.reason.as_ref().map_or(0, String::len))
            .sum(),
        _ => 0,
    }
}
fn subject_bytes(subject: &crate::SemanticSubject<'_>) -> usize {
    match subject {
        crate::SemanticSubject::Node { location, .. } => location.path.as_os_str().len(),
        _ => 0,
    }
}
fn admit_json<'a>(
    value: &ReportValue<'a>,
    semantic: Option<&SemanticResult<'a>>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    match value {
        ReportValue::Record(fields) => {
            work.produce(1 + fields.len(), range)?;
            for (label, value) in fields {
                work.spend(label.len(), range)?;
                admit_json(value, semantic, work, range)?;
            }
        }
        ReportValue::Query(SemanticStream::Inspection(value)) => {
            inspection::admit_json(value, work, range)?
        }
        ReportValue::Query(stream) => semantic.unwrap().admit_program_json(stream, work, range)?,
    }
    Ok(())
}
fn validate_value(
    value: &ReportValue<'_>,
    json: bool,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    match value {
        ReportValue::Record(fields) => {
            for (_, value) in fields {
                validate_value(value, true, budget, range)?;
            }
        }
        ReportValue::Query(SemanticStream::Inspection(value)) => {
            if let QueryResult::Selection(selection) = value.as_ref()
                && !selection.document
            {
                crate::validate_fragment_directives(selection.input, &selection.items)?;
            }
            if json {
                inspection::validate_json(value, &mut Work { budget }, range)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn report_json<'a>(
    value: &ReportValue<'a>,
    semantic: Option<&SemanticResult<'a>>,
) -> Result<Value, QueryError> {
    match value {
        ReportValue::Record(fields) => fields
            .iter()
            .map(|(label, value)| Ok((label.clone(), report_json(value, semantic)?)))
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(Value::Object),
        ReportValue::Query(stream) => {
            if let Some(semantic) = semantic {
                semantic.json_stream(stream)
            } else {
                let SemanticStream::Inspection(value) = stream else {
                    unreachable!()
                };
                inspection::json_value(value)
            }
        }
    }
}
fn charge_render<'a>(
    value: &ReportValue<'a>,
    semantic: Option<&SemanticResult<'a>>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    budget
        .charge_results(value_units(value), zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    let bytes = render_bytes(value, semantic);
    budget
        .charge(bytes, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}

fn render_bytes<'a>(value: &ReportValue<'a>, semantic: Option<&SemanticResult<'a>>) -> usize {
    match value {
        ReportValue::Record(fields) => fields
            .iter()
            .map(|(label, value)| label.len() + render_bytes(value, semantic))
            .sum(),
        ReportValue::Query(SemanticStream::Inspection(value)) => match value.as_ref() {
            QueryResult::Selection(s) => {
                if s.document {
                    s.input.source_bytes().len()
                } else {
                    s.items.iter().map(|item| item.source_range.len()).sum()
                }
            }
            QueryResult::Nodes(s) => s.nodes.iter().map(|node| node.range.len()).sum(),
            QueryResult::Literals(s) => s.values.iter().map(|value| value.spelling.len()).sum(),
            QueryResult::Strings(s) => s.iter().map(|value| value.len() + 1).sum(),
            QueryResult::Count(_) => 32,
            QueryResult::Tally(_) => 0,
            _ => unreachable!(),
        },
        ReportValue::Query(SemanticStream::Entities(rows)) => rows
            .iter()
            .map(|entity| semantic.unwrap().source_bytes(*entity).len())
            .sum(),
        ReportValue::Query(stream) => semantic.unwrap().source_output_bytes(stream),
    }
}
