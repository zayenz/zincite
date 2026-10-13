//! Ordered read-only programs over native query results and constructed values.
use crate::{
    Input, Limits, Query, QueryError, QueryResult, Selection, SemanticResult, SemanticStream, Work,
    comprehension, inspection, language, pipeline, semantic,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ops::Range, rc::Rc};
use zincite_lint::{ModelContext, ModelFacts, SourceDiagnostic};
use zirql::{
    Bindings, Budget, EmissionPolicy, ExpressionKind, ExpressionStep, Located, NativeInput,
    NativeValue, ProgramHost,
};

type Expression = zirql::Expression<Stage, (), Emitter>;

#[derive(Debug)]
enum Stage {
    Query(usize),
    Literal(Located<String>),
    Value {
        expression: comprehension::LoweredValue,
        names: Vec<Located<String>>,
    },
}

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
    Constructed,
}

/// Native report leaves retain source selections and semantic identities.
/// Constructed values have no input-source ranges; record labels preserve order.
#[derive(Debug)]
pub enum ReportValue<'a> {
    Query(SemanticStream<'a>),
    Record(Vec<(String, ReportValue<'a>)>),
    Constructed(zirql::ConstructedValue),
}
impl<'a> ReportValue<'a> {
    pub fn fields(&self) -> Option<&[(String, ReportValue<'a>)]> {
        match self {
            Self::Record(fields) => Some(fields),
            Self::Query(_) | Self::Constructed(_) => None,
        }
    }
    pub fn stream(&self) -> Option<&SemanticStream<'a>> {
        match self {
            Self::Query(stream) => Some(stream),
            Self::Record(_) | Self::Constructed(_) => None,
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
    facts: Option<Rc<ModelFacts<'a>>>,
    report_limitations: Vec<SourceDiagnostic>,
    budget: Budget,
}
impl<'a> ProgramResult<'a> {
    pub fn emissions(&self) -> &[ProgramEmission<'a>] {
        &self.emissions
    }
    pub fn semantic(&self) -> Option<&SemanticResult<'a>> {
        self.semantic.as_ref()
    }
    /// Retained owner for native nominal IDs, including pure `.dzn` reports.
    pub fn model_facts(&self) -> Option<&ModelFacts<'a>> {
        self.facts.as_deref()
    }
    pub fn limitations(&self) -> &[SourceDiagnostic] {
        self.semantic
            .as_ref()
            .map_or(&self.report_limitations, SemanticResult::limitations)
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
                        .map_or(&self.report_limitations[..], SemanticResult::limitations)
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
                    let value = report_json(
                        &emission.value,
                        self.semantic.as_ref(),
                        &mut self.budget,
                        &emission.range,
                    )?;
                    let envelope = json!({"complete": self.is_complete(), "limitations": self.limitations().iter().map(|d| json!({"file": d.location.path.to_string_lossy(), "range": {"start": d.location.range.start, "end": d.location.range.end}, "line": d.location.line, "column": d.location.column, "message": d.message})).collect::<Vec<_>>()});
                    let mut envelope = envelope;
                    envelope
                        .as_object_mut()
                        .unwrap()
                        .insert("result".to_owned(), value);
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
    program: zirql::Program<Stage, (), Emitter>,
    preflight_work: usize,
    preflight_results: usize,
    preflight_range: Option<Range<usize>>,
    queries: Vec<Query>,
    report_model_demand: bool,
}
impl Program {
    /// Parse immutable source/let bindings, query statements and reports.
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
            report_model_demand: false,
            preflight: Budget::for_program(limits.core()),
            preflight_range: None,
            lexed: zincite_syntax::lex(source),
        };
        let mut statements = Vec::new();
        parser.cursor.space();
        while parser.cursor.position < source.len() {
            let start = parser.cursor.position;
            let is_source = parser.cursor.take_word("source");
            let is_let = !is_source && parser.cursor.take_word("let");
            let binding = if is_source || is_let {
                let (name, range) = parser.identifier("expected a binding name")?;
                if matches!(
                    name,
                    "source" | "let" | "literal" | "report" | "query" | "json" | "emit"
                ) {
                    return Err(QueryError::query(
                        range,
                        "binding name is reserved by the program grammar",
                    ));
                }
                if parser.names.contains_key(name) {
                    return Err(QueryError::query(range, "binding is already defined"));
                }
                parser
                    .cursor
                    .punctuation(b'=', "expected '=' after binding name")?;
                Some(Located::new(name.to_owned(), range))
            } else {
                None
            };
            let report = binding.is_none() && parser.cursor.take_word("report");
            parser.cursor.space();
            let expression_start = parser.cursor.position;
            let expression_end = parser.expression_end()?;
            let (expression, kind, emits) = if is_source && parser.cursor.take_word("literal") {
                parser.begin_pure(expression_start..expression_end)?;
                parser.report_model_demand = true;
                parser
                    .cursor
                    .punctuation(b'(', "expected '(' after literal")?;
                let (name, range) = parser.cursor.string()?;
                parser
                    .preflight
                    .charge(name.len() + 1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                parser
                    .cursor
                    .punctuation(b')', "expected ')' after literal name")?;
                parser.cursor.space();
                if parser.cursor.position != expression_end {
                    return Err(parser.cursor.error("expected one literal source wrapper"));
                }
                (
                    value_stage(
                        Stage::Literal(Located::new(name, range)),
                        expression_start..expression_end,
                    ),
                    Kind::Constructed,
                    false,
                )
            } else if !is_let && parser.native_syntax(expression_start..expression_end) {
                parser.expression(0, Kind::Nodes)?
            } else {
                if is_source {
                    return Err(QueryError::query(
                        expression_start..expression_end,
                        "source requires query(...) or literal(...)",
                    ));
                }
                parser.pure_expression(expression_start..expression_end)?
            };
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
                    implicit_output: Some(
                        if report || matches!(kind, Kind::Record | Kind::Constructed) {
                            Emitter::Json
                        } else {
                            Emitter::Source
                        },
                    ),
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
            report_model_demand: parser.report_model_demand,
            preflight_work: limits
                .work
                .checked_sub(parser.preflight.remaining_work())
                .unwrap(),
            preflight_results: limits
                .results
                .checked_sub(parser.preflight.remaining_results().unwrap())
                .unwrap(),
            preflight_range: parser.preflight_range,
        })
    }
    pub fn requires_model_facts(&self) -> bool {
        self.report_model_demand || self.queries.iter().any(Query::requires_model_facts)
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
        let facts = model.map(|model| Rc::new(ModelFacts::new(model)));
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
        if let Some(range) = &self.preflight_range {
            budget
                .charge(self.preflight_work, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            budget
                .charge_results(
                    self.preflight_results,
                    zirql::Location::Query(range.clone()),
                )
                .map_err(pipeline::core_error)?;
        }
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
            pipeline::prepare_context(demand, input, facts.clone(), limits.query(), &mut budget)?;
        let input_value = ReportValue::Query(context.restore_stream(rows));
        let pipelines = self
            .queries
            .iter()
            .map(pipeline::lower_pipeline)
            .collect::<Result<Vec<_>, _>>()?;
        let host = NativeHost {
            pipelines,
            nesting: limits.nesting,
            limits: limits.query(),
        };
        let mut context = Context {
            query: &mut context,
            input,
            facts,
            facts_admitted: false,
            emissions: Vec::new(),
        };
        zirql::evaluate_program(
            &self.program,
            &host,
            input_value,
            &mut context,
            Bindings::default(),
            &mut budget,
        )
        .map_err(pipeline::evaluation_error)?;
        let report_limitations = if context.facts_admitted {
            let limitations = context
                .facts
                .as_ref()
                .unwrap()
                .model()
                .limitations
                .as_slice();
            budget
                .charge_results(
                    limitations.len(),
                    zirql::Location::Query(self.program.range()),
                )
                .map_err(pipeline::core_error)?;
            for limitation in limitations {
                budget
                    .charge(
                        limitation.message.len() + limitation.location.path.as_os_str().len() + 1,
                        zirql::Location::Query(self.program.range()),
                    )
                    .map_err(pipeline::core_error)?;
            }
            limitations.to_vec()
        } else {
            Vec::new()
        };
        let retained_facts = context.facts;
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
            facts: retained_facts,
            report_limitations: if semantic.is_some() {
                Vec::new()
            } else {
                report_limitations
            },
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
    report_model_demand: bool,
    preflight: Budget,
    preflight_range: Option<Range<usize>>,
    lexed: zincite_syntax::LexedSource,
}
impl<'a> Parser<'a> {
    fn expression_end(&self) -> Result<usize, QueryError> {
        use zincite_syntax::TokenKind as T;
        let mut depth = 0usize;
        let tokens = self.lexed.tokens();
        let first = tokens.partition_point(|t| t.range.start < self.cursor.position);
        for token in &tokens[first..] {
            match token.kind {
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart => depth += 1,
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd => {
                    depth = depth.saturating_sub(1)
                }
                T::Semicolon if depth == 0 => return Ok(token.range.start),
                _ => {}
            }
        }
        Err(self.cursor.error("expected ';' after program statement"))
    }
    fn begin_pure(&mut self, range: Range<usize>) -> Result<(), QueryError> {
        if self.preflight_range.is_none() {
            let units = self
                .cursor
                .source
                .len()
                .checked_add(self.lexed.tokens().len())
                .ok_or_else(|| {
                    QueryError::query(range.clone(), "program allocation size overflow")
                })?;
            self.preflight
                .charge(units, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            self.preflight_range = Some(range);
        }
        Ok(())
    }
    fn pure_expression(
        &mut self,
        range: Range<usize>,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        self.begin_pure(range.clone())?;
        self.admit_cst(&range)?;
        let expression = comprehension::lower(
            &self.cursor.source[range.clone()],
            range.clone(),
            &mut self.preflight,
            self.limits.nesting,
        )?;
        let mut types = BTreeMap::new();
        for (name, kind) in &self.names {
            self.preflight
                .charge(name.len() + 3, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            let ty = match kind {
                Kind::Count => zirql::ValueType::Integer,
                Kind::Strings => zirql::ValueType::Array(zirql::ArrayType::Elements(vec![
                    zirql::ValueType::String,
                ])),
                Kind::Literals => zirql::ValueType::Array(zirql::ArrayType::Elements(vec![
                    zirql::ValueType::Unestablished,
                ])),
                _ => zirql::ValueType::Unestablished,
            };
            types.insert(name.clone(), ty);
        }
        for member in &expression.members {
            self.preflight
                .charge(
                    member.slot.len() + 1,
                    zirql::Location::Query(member.range.clone()),
                )
                .map_err(pipeline::core_error)?;
            types.insert(member.slot.clone(), zirql::ValueType::Unestablished);
        }
        self.report_model_demand |= !expression.members.is_empty()
            || self
                .names
                .values()
                .any(|kind| matches!(kind, Kind::Literals));
        let mut names =
            zirql::free_value_names(&expression.expression, &types, &mut self.preflight)
                .map_err(pipeline::core_error)?;
        names.retain(|name| !name.value.starts_with('\0'));
        for name in &names {
            if !matches!(
                self.names[&name.value],
                Kind::Count | Kind::Strings | Kind::Literals | Kind::Constructed
            ) {
                return Err(QueryError::query(
                    name.range.clone(),
                    "native result requires an explicit names, count or values projection",
                ));
            }
        }
        self.cursor.position = range.end;
        Ok((
            value_stage(Stage::Value { expression, names }, range),
            Kind::Constructed,
            false,
        ))
    }
    fn admit_cst(&mut self, range: &Range<usize>) -> Result<(), QueryError> {
        use zincite_syntax::TokenKind as T;
        let tokens = self.lexed.tokens();
        let first = tokens.partition_point(|t| t.range.start < range.start);
        let last = tokens.partition_point(|t| t.range.end <= range.end);
        self.preflight
            .charge(last - first, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        // The host CST parser has no nesting bound. Bound its structural paths
        // before calling it, without interpreting operators or building a tree.
        // Sibling expressions reset the local count; enclosing paths retain it.
        let mut segments = [0usize; 65];
        let mut depth = 0usize;
        let mut path = 1usize;
        let limit = self.limits.nesting.min(64);
        for token in &tokens[first..last] {
            match token.kind {
                T::Whitespace | T::LineComment | T::BlockComment => continue,
                T::Let | T::StringHead | T::StringMiddle | T::StringTail => {
                    return Err(QueryError::query(
                        token.range.clone(),
                        "unsupported expression in positional reports",
                    ));
                }
                T::RightParen | T::RightBracket | T::RightBrace | T::MatrixEnd | T::EndIf => {
                    path -= segments[depth];
                    segments[depth] = 0;
                    if depth != 0 {
                        depth -= 1;
                        path -= 1;
                    }
                }
                T::Comma | T::Colon | T::Pipe | T::Then | T::Else | T::ElseIf => {
                    path -= segments[depth];
                    segments[depth] = 0;
                }
                T::IntegerLiteral
                | T::FloatLiteral
                | T::StringLiteral
                | T::Identifier
                | T::QuotedIdentifier
                | T::Anonymous
                | T::True
                | T::False => {}
                T::LeftParen | T::LeftBracket | T::LeftBrace | T::MatrixStart | T::If => {
                    path += 1;
                    depth += 1;
                }
                _ => {
                    segments[depth] += 1;
                    path += 1;
                }
            }
            if path > limit {
                return Err(QueryError::query(
                    token.range.clone(),
                    "value nesting limit exceeded",
                ));
            }
        }
        Ok(())
    }
    fn native_syntax(&self, range: Range<usize>) -> bool {
        let tokens = self.lexed.tokens();
        let first = tokens.partition_point(|t| t.range.start < range.start);
        let last = tokens.partition_point(|t| t.range.end <= range.end);
        let mut syntax = NativeSyntax {
            tokens: &tokens[first..last],
            source: self.cursor.source,
            names: &self.names,
            position: 0,
        };
        syntax.expression(0) && syntax.current().is_none()
    }
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
            ExpressionStep::Stage(Stage::Query(index)),
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
fn value_stage(stage: Stage, range: Range<usize>) -> Expression {
    Located::new(
        ExpressionKind::Sequence {
            input: Box::new(Located::new(ExpressionKind::Input, range.clone())),
            steps: vec![Located::new(ExpressionStep::Stage(stage), range.clone())],
        },
        range,
    )
}

// Recognize only the complete legacy grammar, before its authoritative parser
// performs name/type/purity validation. This never mutates the query table.
struct NativeSyntax<'a> {
    tokens: &'a [zincite_syntax::Token],
    source: &'a str,
    names: &'a BTreeMap<String, Kind>,
    position: usize,
}
impl NativeSyntax<'_> {
    fn current(&mut self) -> Option<zincite_syntax::TokenKind> {
        use zincite_syntax::TokenKind as T;
        while self
            .tokens
            .get(self.position)
            .is_some_and(|t| matches!(t.kind, T::Whitespace | T::LineComment | T::BlockComment))
        {
            self.position += 1;
        }
        self.tokens.get(self.position).map(|t| t.kind)
    }
    fn take(&mut self, kind: zincite_syntax::TokenKind) -> bool {
        if self.current() != Some(kind) {
            return false;
        }
        self.position += 1;
        true
    }
    fn text(&mut self) -> Option<&str> {
        self.current()?;
        self.tokens
            .get(self.position)
            .map(|t| crate::identifier_identity(&self.source[t.range.clone()]))
    }
    fn query(&mut self) -> bool {
        use zincite_syntax::TokenKind as T;
        if self.text() != Some("query") {
            return false;
        }
        self.position += 1;
        self.take(T::LeftParen) && self.take(T::StringLiteral) && self.take(T::RightParen)
    }
    fn expression(&mut self, depth: usize) -> bool {
        use zincite_syntax::TokenKind as T;
        if depth > 64 {
            return false;
        }
        if self.text() == Some("query") {
            if !self.query() {
                return false;
            }
        } else if self.take(T::LeftParen) {
            let saved = self.position;
            let label = matches!(self.current(), Some(T::Identifier | T::QuotedIdentifier));
            if label {
                self.position += 1;
            }
            let record = label && self.take(T::Colon);
            if record {
                loop {
                    if !self.expression(depth + 1) {
                        return false;
                    }
                    if !self.take(T::Comma) {
                        break;
                    }
                    if !matches!(self.current(), Some(T::Identifier | T::QuotedIdentifier)) {
                        return false;
                    }
                    self.position += 1;
                    if !self.take(T::Colon) {
                        return false;
                    }
                }
            } else {
                self.position = saved;
                if !self.expression(depth + 1) {
                    return false;
                }
            }
            if !self.take(T::RightParen) {
                return false;
            }
        } else {
            if !matches!(self.current(), Some(T::Identifier | T::QuotedIdentifier)) {
                return false;
            }
            let name =
                crate::identifier_identity(&self.source[self.tokens[self.position].range.clone()]);
            if matches!(self.names.get(name), Some(Kind::Constructed)) {
                return false;
            }
            self.position += 1;
        }
        while self.take(T::Pipe) {
            if matches!(self.text(), Some("json" | "emit")) {
                self.position += 1;
            } else if !self.query() {
                return false;
            }
        }
        true
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
    input: &'a Input,
    facts: Option<Rc<ModelFacts<'a>>>,
    facts_admitted: bool,
    emissions: Vec<ProgramEmission<'a>>,
}
impl<'a> Context<'_, 'a> {
    fn report_facts(
        &mut self,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Option<Rc<ModelFacts<'a>>>, QueryError> {
        if let Some(facts) = &self.facts
            && !self.facts_admitted
        {
            if let Some(report) = &mut self.query.report {
                report.admit_fact_owner(&mut Work { budget }, range)?;
            } else {
                semantic::admit_model_facts(facts.model(), &mut Work { budget }, range)?;
            }
            self.facts_admitted = true;
        }
        Ok(self.facts.clone())
    }
}
struct NativeHost<'q> {
    pipelines:
        Vec<zirql::Pipeline<(&'q language::Predicate, &'q language::Stage), &'q language::Stage>>,
    nesting: usize,
    limits: Limits,
}
impl<'a, 'q> ProgramHost<Context<'_, 'a>> for NativeHost<'q> {
    type Value = ReportValue<'a>;
    type Stage = Stage;
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
        stage: &Stage,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let index = match stage {
            Stage::Query(index) => *index,
            Stage::Literal(name) => {
                let facts = context
                    .report_facts(budget, &name.range)
                    .map_err(zirql::EvaluationError::Host)?;
                return comprehension::literal_source(
                    context.input,
                    &name.value,
                    facts.as_deref(),
                    budget,
                    &name.range,
                    self.limits,
                )
                .map(ReportValue::Constructed)
                .map_err(zirql::EvaluationError::Host);
            }
            Stage::Value { expression, names } => {
                let facts = if !expression.members.is_empty() || names.iter().any(|name| matches!(bindings.get(&name.value, name.range.clone()), Ok(ReportValue::Query(SemanticStream::Inspection(value))) if matches!(value.as_ref(), QueryResult::Literals(_)))) {
                    context.report_facts(budget, range).map_err(zirql::EvaluationError::Host)?
                } else { context.facts.clone() };
                let count = names
                    .len()
                    .checked_add(expression.members.len())
                    .ok_or_else(|| {
                        zirql::QueryError::query(range.clone(), "projection size overflow")
                    })?;
                budget.charge(count, zirql::Location::Query(range.clone()))?;
                budget.charge_results(count, zirql::Location::Query(range.clone()))?;
                let mut projected = Vec::with_capacity(count);
                for name in names {
                    budget.charge(
                        name.value.len() + 20,
                        zirql::Location::Query(name.range.clone()),
                    )?;
                    let value = bindings.get(&name.value, name.range.clone())?;
                    let value = match value {
                        ReportValue::Constructed(value) => zirql::copy_constructed_value(
                            value,
                            budget,
                            zirql::Location::Query(name.range.clone()),
                        )?,
                        ReportValue::Query(stream) => comprehension::project(
                            stream,
                            facts.as_deref(),
                            budget,
                            &name.range,
                            self.nesting,
                        )
                        .map_err(zirql::EvaluationError::Host)?,
                        ReportValue::Record(_) => {
                            return Err(zirql::EvaluationError::Host(QueryError::query(
                                name.range.clone(),
                                "native records require explicit scalar projections",
                            )));
                        }
                    };
                    projected.push((
                        Located::new(name.value.to_owned(), name.range.clone()),
                        value,
                    ));
                }
                for member in &expression.members {
                    budget.charge(
                        member.slot.len() + 1,
                        zirql::Location::Query(member.range.clone()),
                    )?;
                    let value = comprehension::resolve_member(
                        facts.as_deref(),
                        Some(&member.enum_name.value),
                        &member.member_name.value,
                        budget,
                        &member.range,
                    )
                    .map_err(zirql::EvaluationError::Host)?;
                    projected.push((
                        Located::new(member.slot.clone(), member.range.clone()),
                        zirql::ConstructedValue::Member(value),
                    ));
                }
                let bindings = Bindings::new(projected)?;
                let value = zirql::evaluate_value_with_host_validation(
                    &expression.expression,
                    &bindings,
                    budget,
                    comprehension::validate_report_value,
                )?;
                comprehension::validate_report_shape(&value, budget, range)
                    .map_err(zirql::EvaluationError::Host)?;
                return Ok(ReportValue::Constructed(value));
            }
        };
        if self.pipelines[index].stages().is_empty() {
            return Ok(value);
        }
        let ReportValue::Query(stream) = value else {
            return Err(zirql::EvaluationError::Host(QueryError::query(
                range.clone(),
                "query stages require a native query result",
            )));
        };
        let SemanticStream::Inspection(ref value) = stream else {
            return self.run_pipeline(index, stream, context, budget, range);
        };
        if matches!(
            value.as_ref(),
            QueryResult::Count(_) | QueryResult::Tally(_)
        ) {
            if !self.pipelines[index].stages().is_empty() {
                return Err(zirql::EvaluationError::Host(QueryError::query(
                    range.clone(),
                    "query stages require a stream",
                )));
            }
            return Ok(ReportValue::Query(stream));
        }
        self.run_pipeline(index, stream, context, budget, range)
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
    if let ReportValue::Constructed(value) = value {
        return zirql::copy_constructed_value(value, budget, zirql::Location::Query(range.clone()))
            .map(ReportValue::Constructed)
            .map_err(pipeline::core_error);
    }
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
        ReportValue::Constructed(_) => unreachable!("constructed copies use the core"),
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
        ReportValue::Constructed(_) => unreachable!("constructed admission uses the core"),
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
        ReportValue::Constructed(_) => unreachable!("constructed admission uses the core"),
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
        ReportValue::Constructed(_) => {}
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
        ReportValue::Constructed(value) => {
            comprehension::validate_report_shape(value, budget, range)?
        }
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
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Value, QueryError> {
    match value {
        ReportValue::Constructed(value) => comprehension::json_value(value, budget, range),
        ReportValue::Record(fields) => fields
            .iter()
            .map(|(label, value)| Ok((label.clone(), report_json(value, semantic, budget, range)?)))
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
        ReportValue::Constructed(_) => unreachable!("constructed reports use tagged JSON"),
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
