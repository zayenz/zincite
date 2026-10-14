//! Native reports and atomic model-backed data batches using shared evaluation.
use crate::{
    Input, Limits, Query, QueryError, QueryResult, Selection, SemanticResult, SemanticStream, Work,
    comprehension, data, inspection, language, pipeline, semantic,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ops::Range, rc::Rc};
use zincite_lint::{ModelContext, ModelFacts, SourceDiagnostic};
use zirql::{
    Bindings, Budget, EmissionPolicy, ExpressionKind, ExpressionStep, Located, NativeInput,
    NativeValue, ProgramHost,
};

mod composition;

type Expression = zirql::Expression<Stage, (), Emitter>;

#[derive(Debug)]
enum Stage {
    Parameter(usize),
    Query(usize),
    Projection(composition::ProjectionStage),
    Match(composition::MatchStage),
    Data(Vec<data::Edit>),
    Literal(Located<String>),
    LiteralValue {
        expression: comprehension::LoweredValue,
        names: Vec<Located<String>>,
    },
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
#[derive(Clone, Debug, PartialEq)]
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
    Scalar(zirql::ParameterType),
    Value(Rc<Vec<zirql::ValueType>>),
    Capture(Rc<zirql::NativeCapture<Kind>>),
    Captures(Rc<zirql::NativeCapture<Kind>>),
}

/// Native report leaves retain source selections and semantic identities.
/// Constructed values have no input-source ranges; record labels preserve order.
#[derive(Debug)]
pub enum ReportValue<'a> {
    Query(SemanticStream<'a>),
    Record(Vec<(String, ReportValue<'a>)>),
    Capture(zirql::NativeCapture<ReportValue<'a>>),
    Captures(CaptureMatches<'a>),
    Constructed(zirql::ConstructedValue),
}
impl<'a> ReportValue<'a> {
    pub fn fields(&self) -> Option<&[(String, ReportValue<'a>)]> {
        match self {
            Self::Record(fields) => Some(fields),
            Self::Capture(capture) => Some(&capture.fields),
            Self::Query(_) | Self::Constructed(_) | Self::Captures(_) => None,
        }
    }
    pub fn stream(&self) -> Option<&SemanticStream<'a>> {
        match self {
            Self::Query(stream) => Some(stream),
            Self::Record(_) | Self::Constructed(_) | Self::Capture(_) | Self::Captures(_) => None,
        }
    }
}

/// Native match rows retain their declared fields even when no candidate matches.
#[derive(Debug)]
pub struct CaptureMatches<'a> {
    rows: Vec<zirql::NativeCapture<ReportValue<'a>>>,
    schema: Rc<zirql::NativeCapture<Kind>>,
}
impl<'a> CaptureMatches<'a> {
    pub fn rows(&self) -> &[zirql::NativeCapture<ReportValue<'a>>] {
        &self.rows
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
    candidate: Option<Input>,
    candidate_range: Range<usize>,
    semantic: Option<SemanticResult<'a>>,
    facts: Option<Rc<ModelFacts<'a>>>,
    report_limitations: Vec<SourceDiagnostic>,
    budget: Budget,
}
impl<'a> ProgramResult<'a> {
    /// The complete reparsed document, present only for an editing program.
    pub fn candidate(&self) -> Option<&Input> {
        self.candidate.as_ref()
    }
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
        if let Some(candidate) = &self.candidate {
            self.budget
                .charge(
                    candidate.source_bytes().len(),
                    zirql::Location::Query(self.candidate_range.clone()),
                )
                .map_err(pipeline::core_error)?;
            return Ok(candidate.source_bytes().to_vec());
        }
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

/// A parsed report or data program. Query-source strings lower once. Edit RHS
/// expressions are lowered again with target evidence under the request budget;
/// their original program coordinates are retained. Generated text is not evaluated.
#[derive(Debug)]
struct QueryArgument {
    owner: usize,
    kind: language::ArgumentKind,
    expression: comprehension::LoweredValue,
    names: Vec<Located<String>>,
}
#[derive(Debug)]
struct ProgramQuery {
    query: Query,
    arguments: Vec<QueryArgument>,
    copy_units: usize,
}

#[derive(Debug)]
pub struct Program {
    parameters: Vec<zirql::ParameterDeclaration>,
    program: zirql::Program<Stage, (), Emitter>,
    preflight_work: usize,
    preflight_results: usize,
    preflight_range: Option<Range<usize>>,
    queries: Vec<ProgramQuery>,
    report_model_demand: bool,
    editing: bool,
}
impl Program {
    /// Parse immutable bindings, reports, or one terminal data edit batch.
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
                collection_limit: limits.stream_items,
                arguments: None,
            },
            limits,
            names: BTreeMap::new(),
            queries: Vec::new(),
            report_model_demand: false,
            preflight: Budget::for_program(limits.core()),
            preflight_range: None,
            first_report_call: None,
            composition_error: None,
            lexed: zincite_syntax::lex(source),
        };
        let mut statements = Vec::new();
        let mut parameters = Vec::new();
        let mut edits = Vec::new();
        let mut target_names = std::collections::BTreeSet::new();
        let mut output_seen = false;
        let mut let_seen = false;
        let mut late_source = None;
        let mut invalid_source = None;
        let mut invalid_binding = None;
        let mut document_seen = false;
        parser.cursor.space();
        while parser.cursor.position < source.len() {
            let start = parser.cursor.position;
            if document_seen {
                return Err(parser.cursor.error("no statement may follow emit"));
            }
            let mut reader = zirql::ArgumentReader::new(source, parser.cursor.position)
                .map_err(pipeline::core_error)?;
            if source[parser.cursor.position..].starts_with("param")
                && let Some(zirql::CommonStatement::Param(declaration)) =
                    zirql::read_statement_prefix(&mut reader, &mut parser.preflight)
                        .map_err(pipeline::core_error)?
            {
                parser.begin_pure(declaration.name.range.clone())?;
                parser.cursor.position = reader.position();
                parser
                    .cursor
                    .punctuation(b';', "expected ';' after parameter declaration")?;
                if parser.names.contains_key(&declaration.name.value)
                    || matches!(
                        declaration.name.value.as_str(),
                        "source"
                            | "let"
                            | "literal"
                            | "report"
                            | "query"
                            | "json"
                            | "emit"
                            | "create"
                            | "replace"
                            | "emit_document"
                    )
                {
                    return Err(QueryError::query(
                        declaration.name.range,
                        "parameter name is reserved or already defined",
                    ));
                }
                parser
                    .preflight
                    .charge(
                        declaration.name.value.len() + 1,
                        zirql::Location::Query(declaration.name.range.clone()),
                    )
                    .map_err(pipeline::core_error)?;
                parser
                    .preflight
                    .charge_results(1, zirql::Location::Query(declaration.name.range.clone()))
                    .map_err(pipeline::core_error)?;
                parser.names.insert(
                    declaration.name.value.clone(),
                    Kind::Scalar(declaration.ty.value),
                );
                parser
                    .preflight
                    .charge(
                        declaration.name.value.len() + 1,
                        zirql::Location::Query(declaration.name.range.clone()),
                    )
                    .and_then(|()| {
                        parser.preflight.charge_results(
                            2,
                            zirql::Location::Query(declaration.name.range.clone()),
                        )
                    })
                    .map_err(pipeline::core_error)?;
                statements.push(Located::new(
                    zirql::Statement::Bind {
                        name: declaration.name.clone(),
                        expression: value_stage(
                            Stage::Parameter(parameters.len()),
                            declaration.name.range.clone(),
                        ),
                    },
                    declaration.name.range.clone(),
                ));
                parameters.push(declaration);
                parser.cursor.space();
                continue;
            }
            let create = parser.cursor.take_word("create");
            let replace = !create && parser.cursor.take_word("replace");
            if create || replace {
                if let Some(range) = &parser.first_report_call {
                    return Err(QueryError::query(
                        range.clone(),
                        "report-only call is unavailable in data expressions",
                    ));
                }
                if output_seen {
                    return Err(parser
                        .cursor
                        .error("an edit batch cannot include report or query emissions"));
                }
                let (name, range) = parser.identifier("expected an assignment target")?;
                if !parser.minizinc_identifier(&range) {
                    return Err(QueryError::query(
                        range,
                        "expected a MiniZinc identifier or quoted identifier",
                    ));
                }
                parser.begin_pure(range.clone())?;
                parser
                    .preflight
                    .charge(name.len() + 1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                parser
                    .preflight
                    .charge_results(1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                if !target_names.insert(name.to_owned()) {
                    return Err(QueryError::query(range, "duplicate edit target"));
                }
                parser
                    .cursor
                    .punctuation(b'=', "expected '=' after edit target")?;
                parser.cursor.space();
                let expression_range = parser.cursor.position..parser.expression_end()?;
                let (_, names) = parser.pure_value(
                    expression_range.clone(),
                    comprehension::ExpressionPolicy::Data,
                )?;
                parser
                    .preflight
                    .charge(
                        expression_range.len() + name.len() + 1,
                        zirql::Location::Query(range.clone()),
                    )
                    .map_err(pipeline::core_error)?;
                edits.push(data::Edit {
                    create,
                    name: Located::new(name.to_owned(), range),
                    source: source[expression_range.clone()].to_owned(),
                    range: expression_range,
                    names,
                });
                parser.cursor.punctuation(b';', "expected ';' after edit")?;
                parser.cursor.space();
                continue;
            }
            if parser.cursor.take_word("emit") || parser.cursor.take_word("emit_document") {
                if edits.is_empty() || output_seen {
                    return Err(QueryError::query(
                        start..parser.cursor.position,
                        "emit requires an edit batch",
                    ));
                }
                parser.cursor.punctuation(b';', "expected ';' after emit")?;
                let range = start..parser.cursor.position;
                statements.push(Located::new(
                    zirql::Statement::Evaluate {
                        expression: value_stage(
                            Stage::Data(std::mem::take(&mut edits)),
                            range.clone(),
                        ),
                        implicit_output: None,
                    },
                    range,
                ));
                document_seen = true;
                parser.cursor.space();
                continue;
            }
            if !edits.is_empty() {
                return Err(parser.cursor.error("an edit batch must end with emit"));
            }
            let is_source = parser.cursor.take_word("source");
            let is_let = !is_source && parser.cursor.take_word("let");
            if is_source && let_seen {
                late_source = Some(start..parser.cursor.position);
            }
            let_seen |= is_let;
            let binding = if is_source || is_let {
                let (name, range) = parser.identifier("expected a binding name")?;
                if !parser.minizinc_identifier(&range) {
                    invalid_binding = Some(range.clone());
                }
                if matches!(
                    name,
                    "source"
                        | "let"
                        | "literal"
                        | "report"
                        | "query"
                        | "json"
                        | "emit"
                        | "create"
                        | "replace"
                        | "emit_document"
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
            if is_source
                && !source[expression_start..expression_end]
                    .trim_start()
                    .starts_with("query")
                && !source[expression_start..expression_end]
                    .trim_start()
                    .starts_with("literal")
            {
                invalid_source = Some(expression_start..expression_end);
            }
            let (expression, kind, emits) = if is_source && parser.cursor.take_word("literal") {
                parser.begin_pure(expression_start..expression_end)?;
                parser.report_model_demand = true;
                parser
                    .cursor
                    .punctuation(b'(', "expected '(' after literal")?;
                parser.cursor.space();
                let argument_start = parser.cursor.position;
                let literal_name = if parser.cursor.current() == Some(b'"') {
                    let (name, range) = parser.cursor.string()?;
                    parser
                        .preflight
                        .charge(name.len() + 1, zirql::Location::Query(range.clone()))
                        .map_err(pipeline::core_error)?;
                    parser.cursor.space();
                    if parser.cursor.current() == Some(b')')
                        && parser.cursor.position + 1 == expression_end
                    {
                        Some((name, range))
                    } else {
                        parser.cursor.position = argument_start;
                        None
                    }
                } else {
                    None
                };
                if let Some((name, range)) = literal_name {
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
                } else {
                    let wrapper = &source[argument_start..expression_end];
                    let closing = expression_end - (wrapper.len() - wrapper.trim_end().len()) - 1;
                    if source.as_bytes().get(closing) != Some(&b')') {
                        return Err(parser.cursor.error("expected one literal source wrapper"));
                    }
                    let argument_range = argument_start..closing;
                    let (expression, names) = parser.pure_value(
                        argument_range.clone(),
                        comprehension::ExpressionPolicy::Report,
                    )?;
                    parser.value_names(
                        &expression,
                        &argument_range,
                        Some(zirql::ParameterType::String),
                    )?;
                    if parser.first_report_call.is_none() {
                        parser.first_report_call = expression.first_report_call.clone();
                    }
                    parser.cursor.position = expression_end;
                    (
                        value_stage(
                            Stage::LiteralValue { expression, names },
                            expression_start..expression_end,
                        ),
                        Kind::Constructed,
                        false,
                    )
                }
            } else if parser.composition_syntax(expression_start..expression_end) {
                if parser.first_report_call.is_none() {
                    parser.first_report_call = Some(expression_start..expression_end);
                }
                parser.composition_expression(expression_start..expression_end)?
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
            output_seen |= binding.is_none();
            let statement = if let Some(name) = binding {
                parser.names.insert(name.value.clone(), kind);
                zirql::Statement::Bind { name, expression }
            } else {
                zirql::Statement::Evaluate {
                    expression,
                    implicit_output: Some(
                        if report
                            || matches!(
                                kind,
                                Kind::Record
                                    | Kind::Constructed
                                    | Kind::Value(_)
                                    | Kind::Capture(_)
                                    | Kind::Captures(_)
                            )
                        {
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
        if !edits.is_empty() {
            return Err(parser.cursor.error("an edit batch must end with emit"));
        }
        if document_seen {
            if let Some(range) = invalid_binding {
                return Err(QueryError::query(
                    range,
                    "expected a MiniZinc identifier or quoted identifier",
                ));
            }
            if let Some(range) = late_source {
                return Err(QueryError::query(
                    range,
                    "sources must precede let bindings in an edit request",
                ));
            }
            if let Some(range) = invalid_source {
                return Err(QueryError::query(
                    range,
                    "an edit source requires query(...) or literal(...)",
                ));
            }
        }
        if statements.is_empty() {
            return Err(QueryError::query(
                0..source.len(),
                "expected a program statement",
            ));
        }
        Ok(Self {
            parameters,
            program: zirql::Program::new(statements, 0..source.len()),
            queries: parser.queries,
            report_model_demand: parser.report_model_demand || document_seen,
            editing: document_seen,
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
    pub fn is_editing(&self) -> bool {
        self.editing
    }
    pub fn requires_model_facts(&self) -> bool {
        self.report_model_demand
            || self
                .queries
                .iter()
                .any(|query| query.query.requires_model_facts())
    }
    pub fn evaluate<'a>(
        &self,
        input: &'a Input,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.evaluate_with_parameters(input, zirql::ParameterInputs::Typed(&[]), limits)
    }
    pub fn evaluate_with_model<'a>(
        &self,
        input: &'a Input,
        model: Option<&'a ModelContext>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.evaluate_with_model_and_parameters(
            input,
            model,
            zirql::ParameterInputs::Typed(&[]),
            limits,
        )
    }
    pub fn evaluate_with_facts<'a>(
        &self,
        input: &'a Input,
        facts: Rc<ModelFacts<'a>>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.evaluate_with_facts_and_parameters(
            input,
            facts,
            zirql::ParameterInputs::Typed(&[]),
            limits,
        )
    }
    /// Required scalar declarations retained by this program.
    pub fn parameters(&self) -> &[zirql::ParameterDeclaration] {
        &self.parameters
    }

    /// Admit the complete caller set before native/model execution.
    pub fn evaluate_with_parameters<'a>(
        &self,
        input: &'a Input,
        parameters: zirql::ParameterInputs<'_>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.evaluate_with_model_and_parameters(input, None, parameters, limits)
    }
    pub fn evaluate_with_model_and_parameters<'a>(
        &self,
        input: &'a Input,
        model: Option<&'a ModelContext>,
        parameters: zirql::ParameterInputs<'_>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        let facts = model.map(|model| Rc::new(ModelFacts::new(model)));
        self.run(input, facts, parameters, limits)
    }
    pub fn evaluate_with_facts_and_parameters<'a>(
        &self,
        input: &'a Input,
        facts: Rc<ModelFacts<'a>>,
        parameters: zirql::ParameterInputs<'_>,
        limits: ProgramLimits,
    ) -> Result<ProgramResult<'a>, QueryError> {
        self.run(input, Some(facts), parameters, limits)
    }
    fn run<'a>(
        &self,
        input: &'a Input,
        facts: Option<Rc<ModelFacts<'a>>>,
        parameters: zirql::ParameterInputs<'_>,
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
        let initial = zirql::admit_parameters(
            &self.parameters,
            parameters,
            &[],
            self.program.range(),
            &mut budget,
        )
        .map_err(pipeline::core_error)?;
        budget
            .charge_results(initial.len(), zirql::Location::Query(self.program.range()))
            .map_err(pipeline::core_error)?;
        let parameter_values = initial.into_iter().map(|(_, value)| Some(value)).collect();
        let bindings = Bindings::default();
        let empty = Query {
            stages: Vec::new(),
            range: self.program.range(),
            nesting_limit: limits.nesting,
        };
        let demand = self
            .queries
            .iter()
            .find(|query| query.query.requires_model_facts())
            .map(|query| &query.query)
            .unwrap_or(&empty);
        let (mut context, rows) =
            pipeline::prepare_context(demand, input, facts.clone(), limits.query(), &mut budget)?;
        let input_value = ReportValue::Query(context.restore_stream(rows));
        let pipelines = self
            .queries
            .iter()
            .map(|query| {
                if query.arguments.is_empty() {
                    pipeline::lower_pipeline(&query.query).map(Some)
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let host = NativeHost {
            queries: &self.queries,
            pipelines,
            nesting: limits.nesting,
            limits: limits.query(),
        };
        let mut context = Context {
            query: &mut context,
            input,
            facts,
            facts_admitted: false,
            parameters: parameter_values,
            emissions: Vec::new(),
            candidate: None,
            guard_unknown: None,
        };
        zirql::evaluate_program(
            &self.program,
            &host,
            input_value,
            &mut context,
            bindings,
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
        let candidate = context.candidate;
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
            candidate,
            candidate_range: self.program.range(),
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

impl Drop for Program {
    fn drop(&mut self) {
        let program = std::mem::replace(&mut self.program, zirql::Program::new(Vec::new(), 0..0));
        for statement in program.into_statements() {
            let expression = match statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::Evaluate { expression, .. } => expression,
            };
            zirql::dispose_expression(expression);
        }
    }
}

struct Parser<'a> {
    cursor: language::Parser<'a>,
    limits: ProgramLimits,
    names: BTreeMap<String, Kind>,
    queries: Vec<ProgramQuery>,
    report_model_demand: bool,
    preflight: Budget,
    preflight_range: Option<Range<usize>>,
    lexed: zincite_syntax::LexedSource,
    first_report_call: Option<Range<usize>>,
    composition_error: Option<QueryError>,
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
    fn pure_value(
        &mut self,
        range: Range<usize>,
        policy: comprehension::ExpressionPolicy,
    ) -> Result<(comprehension::LoweredValue, Vec<Located<String>>), QueryError> {
        self.begin_pure(range.clone())?;
        self.admit_cst(&range)?;
        let expression = comprehension::lower(
            &self.cursor.source[range.clone()],
            range.clone(),
            &mut self.preflight,
            self.limits.nesting,
            policy,
        )?;
        let names = self.value_names(&expression, &range, None)?;
        self.cursor.position = range.end;
        Ok((expression, names))
    }
    fn scope_types(
        &mut self,
        range: &Range<usize>,
    ) -> Result<BTreeMap<String, zirql::ValueType>, QueryError> {
        let mut types = BTreeMap::new();
        for (name, kind) in &self.names {
            self.preflight
                .charge(name.len() + 3, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            let ty = match kind {
                Kind::Scalar(ty) => (*ty).into(),
                Kind::Value(types) if types.len() == 1 => zirql::copy_value_types(
                    types,
                    &mut self.preflight,
                    zirql::Location::Query(range.clone()),
                )
                .map_err(pipeline::core_error)?
                .pop()
                .unwrap(),
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
        Ok(types)
    }
    fn value_names(
        &mut self,
        expression: &comprehension::LoweredValue,
        range: &Range<usize>,
        expected: Option<zirql::ParameterType>,
    ) -> Result<Vec<Located<String>>, QueryError> {
        let mut types = self.scope_types(range)?;
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
                Kind::Count
                    | Kind::Strings
                    | Kind::Literals
                    | Kind::Constructed
                    | Kind::Scalar(_)
                    | Kind::Value(_)
            ) {
                return Err(QueryError::query(
                    name.range.clone(),
                    "native result requires an explicit names, count or values projection",
                ));
            }
        }
        if let Some(expected) = expected {
            let expected: zirql::ValueType = expected.into();
            if zirql::value_expression_types(&expression.expression, &types, &mut self.preflight)
                .map_err(pipeline::core_error)?
                .iter()
                .any(|ty| *ty != expected)
            {
                return Err(QueryError::query(
                    expression.expression.range.clone(),
                    "primitive argument has an incompatible scalar type",
                ));
            }
        }
        Ok(names)
    }
    fn pure_expression(
        &mut self,
        range: Range<usize>,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        let (expression, names) =
            self.pure_value(range.clone(), comprehension::ExpressionPolicy::Report)?;
        if self.first_report_call.is_none() {
            self.first_report_call = expression.first_report_call.clone();
        }
        let mut types = self.scope_types(&range)?;
        for member in &expression.members {
            self.preflight
                .charge(
                    member.slot.len() + 1,
                    zirql::Location::Query(member.range.clone()),
                )
                .map_err(pipeline::core_error)?;
            types.insert(member.slot.clone(), zirql::ValueType::Unestablished);
        }
        let result_types =
            zirql::value_expression_types(&expression.expression, &types, &mut self.preflight)
                .map_err(pipeline::core_error)?;
        let kind = match result_types.as_slice() {
            [zirql::ValueType::Integer] => Kind::Scalar(zirql::ParameterType::Int),
            [zirql::ValueType::String] => Kind::Scalar(zirql::ParameterType::String),
            [zirql::ValueType::Boolean] => Kind::Scalar(zirql::ParameterType::Bool),
            _ => Kind::Constructed,
        };
        Ok((
            value_stage(Stage::Value { expression, names }, range),
            kind,
            false,
        ))
    }
    fn admit_cst(&mut self, range: &Range<usize>) -> Result<(), QueryError> {
        admit_cst(&self.lexed, range, &mut self.preflight, self.limits.nesting)
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
    fn minizinc_identifier(&self, range: &Range<usize>) -> bool {
        let i = self
            .lexed
            .tokens()
            .partition_point(|token| token.range.start < range.start);
        self.lexed.tokens().get(i).is_some_and(|token| {
            token.range == *range
                && matches!(
                    token.kind,
                    zincite_syntax::TokenKind::Identifier
                        | zincite_syntax::TokenKind::QuotedIdentifier
                )
        })
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
            self.query(input.clone())?
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
                    let (value, _, emits) = self.expression(depth + 1, input.clone())?;
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
                let (expression, kind, emits) = self.expression(depth + 1, input.clone())?;
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
            let kind = self.names.get(name).cloned().ok_or_else(|| {
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
        self.query_with_kind(Some(input))
    }
    fn query_with_kind(
        &mut self,
        input: Option<Kind>,
    ) -> Result<(Expression, Kind, bool), QueryError> {
        let start = self.cursor.position.saturating_sub(5);
        self.cursor.punctuation(b'(', "expected '(' after query")?;
        let (source, written) = self.cursor.string()?;
        let mapping = string_mapping(self.cursor.source, &written);
        let map = |range: Range<usize>| {
            mapping[range.start.min(mapping.len() - 1)]..mapping[range.end.min(mapping.len() - 1)]
        };
        let mut arguments = Vec::new();
        let mut decoded_lexed = None;
        let limits = self.limits.query();
        let mut callback_error = false;
        let parsed = {
            let mut lower = |owner, kind: language::ArgumentKind, range: Range<usize>| {
                let result = (|| {
                    self.begin_pure(written.clone())?;
                    if decoded_lexed.is_none() {
                        self.preflight
                            .charge(source.len() + 1, zirql::Location::Query(written.clone()))
                            .map_err(pipeline::core_error)?;
                        decoded_lexed = Some(zincite_syntax::lex(&source));
                    }
                    admit_cst(
                        decoded_lexed.as_ref().unwrap(),
                        &range,
                        &mut self.preflight,
                        self.limits.nesting,
                    )
                    .map_err(|mut error| {
                        error.range = map(error.range);
                        error
                    })?;
                    let mut expression = comprehension::lower(
                        &source[range.clone()],
                        range.clone(),
                        &mut self.preflight,
                        self.limits.nesting,
                        comprehension::ExpressionPolicy::Report,
                    )
                    .map_err(|mut error| {
                        error.range = map(error.range);
                        error
                    })?;
                    let mut names = self
                        .value_names(&expression, &range, Some(kind.ty()))
                        .map_err(|mut error| {
                            error.range = map(error.range);
                            error
                        })?;
                    map_value(&mut expression.expression, &map);
                    for name in &mut names {
                        name.range = map(name.range.clone());
                    }
                    for member in &mut expression.members {
                        member.range = map(member.range.clone());
                        member.enum_name.range = map(member.enum_name.range.clone());
                        member.member_name.range = map(member.member_name.range.clone());
                    }
                    expression.first_report_call = expression.first_report_call.map(&map);
                    if self.first_report_call.is_none() {
                        self.first_report_call = expression.first_report_call.clone();
                    }
                    self.preflight
                        .charge_results(1, zirql::Location::Query(written.clone()))
                        .map_err(pipeline::core_error)?;
                    arguments.push(QueryArgument {
                        owner: map(owner..owner).start,
                        kind,
                        expression,
                        names,
                    });
                    Ok(())
                })();
                if result.is_err() {
                    callback_error = true;
                }
                result
            };
            language::parse_scoped(&source, limits, &mut lower)
        };
        let mut query = parsed.map_err(|mut error| {
            if !callback_error {
                error.range = map(error.range);
            }
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
        let kind = input.map_or(Ok(Kind::Nodes), |input| validate_stages(&query, input))?;
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
        self.queries.push(ProgramQuery {
            query,
            arguments,
            copy_units: source.len().saturating_add(1),
        });
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
            if matches!(
                self.names.get(name),
                Some(Kind::Constructed | Kind::Scalar(_))
            ) {
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
fn admit_cst(
    lexed: &zincite_syntax::LexedSource,
    range: &Range<usize>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(), QueryError> {
    use zincite_syntax::TokenKind as T;
    let tokens = lexed.tokens();
    let first = tokens.partition_point(|t| t.range.start < range.start);
    let last = tokens.partition_point(|t| t.range.end <= range.end);
    budget
        .charge(last - first, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    // The host CST parser has no nesting bound. Bound its structural paths
    // before calling it, without interpreting operators or building a tree.
    // Sibling expressions reset the local count; enclosing paths retain it.
    let mut segments = [0usize; 65];
    let mut depth = 0usize;
    let mut path = 1usize;
    let limit = nesting.min(64);
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
fn map_value(expression: &mut zirql::ValueExpression, map: &impl Fn(Range<usize>) -> Range<usize>) {
    use zirql::ValueExpressionKind as K;
    expression.range = map(expression.range.clone());
    match &mut expression.value {
        K::Literal(_) | K::Name(_) | K::NativeQuery(_) => {}
        K::Array(values) | K::Tuple(values) | K::Set(values) => {
            for value in values {
                map_value(value, map);
            }
        }
        K::Record(fields) => {
            for (name, value) in fields {
                name.range = map(name.range.clone());
                map_value(value, map);
            }
        }
        K::Unary { operand, .. }
        | K::One(operand)
        | K::IndexSet(operand)
        | K::Entries(operand)
        | K::Reduce { array: operand, .. } => map_value(operand, map),
        K::Field { value, field } | K::GroupBy { rows: value, field } => {
            field.range = map(field.range.clone());
            map_value(value, map);
        }
        K::TupleAccess { value, position } => {
            position.range = map(position.range.clone());
            map_value(value, map);
        }
        K::Binary { left, right, .. }
        | K::ZipExact { left, right }
        | K::Range {
            lower: left,
            upper: right,
        }
        | K::ArrayAccess {
            array: left,
            index: right,
        } => {
            map_value(left, map);
            map_value(right, map);
        }
        K::Require {
            condition,
            value,
            message,
        } => {
            message.range = map(message.range.clone());
            map_value(condition, map);
            map_value(value, map);
        }
        K::Conditional {
            branches,
            otherwise,
        } => {
            for (condition, value) in branches {
                map_value(condition, map);
                map_value(value, map);
            }
            map_value(otherwise, map);
        }
        K::KeyedArray { entries, .. } => {
            for (key, value) in entries {
                map_value(key, map);
                map_value(value, map);
            }
        }
        K::Comprehension { clauses, head, .. } => {
            map_generators(clauses, map);
            map_value(head, map);
        }
        K::KeyedComprehension {
            clauses,
            key,
            head,
            empty_axis_sources,
        } => {
            map_generators(clauses, map);
            map_value(key, map);
            map_value(head, map);
            for value in empty_axis_sources.iter_mut().flatten() {
                map_value(value, map);
            }
        }
    }
}
fn map_generators(
    clauses: &mut [Located<zirql::ValueGenerator>],
    map: &impl Fn(Range<usize>) -> Range<usize>,
) {
    for clause in clauses {
        clause.range = map(clause.range.clone());
        match &mut clause.value {
            zirql::ValueGenerator::In {
                binders,
                source,
                filter,
            } => {
                for name in binders {
                    name.range = map(name.range.clone());
                }
                map_value(source, map);
                if let Some(value) = filter {
                    map_value(value, map);
                }
            }
            zirql::ValueGenerator::Bind {
                binder,
                value,
                filter,
            } => {
                binder.range = map(binder.range.clone());
                map_value(value, map);
                if let Some(value) = filter {
                    map_value(value, map);
                }
            }
        }
    }
}
fn set_predicate_argument(
    predicate: &mut language::Predicate,
    owner: usize,
    kind: &mut Option<language::PredicateKind>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<bool, zirql::QueryError> {
    budget.charge(1, zirql::Location::Query(range.clone()))?;
    match &mut predicate.kind {
        language::PredicateKind::Kind(_)
        | language::PredicateKind::Name(_)
        | language::PredicateKind::Type(_)
        | language::PredicateKind::Instantiation(_)
        | language::PredicateKind::SourceKind(_) => {
            if predicate.range.start == owner {
                predicate.kind = kind.take().expect("argument is replaced once");
                return Ok(true);
            }
        }
        language::PredicateKind::Not(predicate) | language::PredicateKind::Group(predicate) => {
            return set_predicate_argument(predicate, owner, kind, budget, range);
        }
        language::PredicateKind::And(predicates) | language::PredicateKind::Or(predicates) => {
            for predicate in predicates {
                if set_predicate_argument(predicate, owner, kind, budget, range)? {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
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
                let valid = matches!(
                    kind,
                    Kind::Nodes
                        | Kind::Strings
                        | Kind::Literals
                        | Kind::Entities
                        | Kind::Types
                        | Kind::Instantiations
                );
                kind = Kind::Count;
                valid
            }
            S::Tally => {
                let valid = matches!(kind, Kind::Strings);
                kind = Kind::Tally;
                valid
            }
            S::Unique => matches!(
                kind,
                Kind::Nodes | Kind::Strings | Kind::Entities | Kind::Types | Kind::Instantiations
            ),
            S::Head(_) => matches!(
                kind,
                Kind::Nodes
                    | Kind::Strings
                    | Kind::Literals
                    | Kind::Entities
                    | Kind::Types
                    | Kind::Instantiations
            ),
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
    parameters: Vec<Option<zirql::ConstructedValue>>,
    emissions: Vec<ProgramEmission<'a>>,
    candidate: Option<Input>,
    guard_unknown: Option<SourceDiagnostic>,
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
type NativePipeline<'q> =
    zirql::Pipeline<(&'q language::Predicate, &'q language::Stage), &'q language::Stage>;
struct NativeHost<'q> {
    queries: &'q [ProgramQuery],
    pipelines: Vec<Option<NativePipeline<'q>>>,
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
            Stage::Parameter(index) => {
                return Ok(ReportValue::Constructed(
                    context.parameters[*index]
                        .take()
                        .expect("admitted parameter is bound once"),
                ));
            }
            Stage::Query(index) => *index,
            Stage::Projection(projection) => {
                let host = composition::CompositionHost(self);
                return zirql::evaluate_projection(
                    projection.get(),
                    &host,
                    value,
                    context,
                    bindings,
                    budget,
                )
                .map(ReportValue::Constructed);
            }
            Stage::Match(matching) => {
                return composition::CompositionHost(self).matching(
                    matching.get(),
                    value,
                    context,
                    bindings,
                    budget,
                );
            }
            Stage::LiteralValue { expression, names } => {
                let value = self.constructed(
                    expression,
                    names,
                    context,
                    bindings,
                    budget,
                    &expression.expression.range,
                )?;
                let zirql::ConstructedValue::String(name) = value else {
                    return Err(zirql::EvaluationError::Host(QueryError::query(
                        expression.expression.range.clone(),
                        "literal source argument needs a string",
                    )));
                };
                let facts = context
                    .report_facts(budget, &expression.expression.range)
                    .map_err(zirql::EvaluationError::Host)?;
                return comprehension::literal_source(
                    context.input,
                    &name,
                    facts.as_deref(),
                    budget,
                    &expression.expression.range,
                    self.limits,
                )
                .map(ReportValue::Constructed)
                .map_err(zirql::EvaluationError::Host);
            }
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
                return self
                    .constructed(expression, names, context, bindings, budget, range)
                    .map(ReportValue::Constructed);
            }
            Stage::Data(edits) => {
                let facts = context
                    .report_facts(budget, range)
                    .map_err(zirql::EvaluationError::Host)?;
                let facts = facts.ok_or_else(|| {
                    zirql::EvaluationError::Host(QueryError::query(
                        range.clone(),
                        "data construction requires read-only model context (--model)",
                    ))
                })?;
                let candidate = data::prepare_data_batch(
                    context.input,
                    &facts,
                    edits,
                    budget,
                    self.limits,
                    |expression, names, budget, range| {
                        self.constructed(expression, names, context, bindings, budget, range)
                            .map_err(pipeline::evaluation_error)
                    },
                )
                .map_err(zirql::EvaluationError::Host)?;
                context.candidate = Some(candidate);
                return Ok(value);
            }
        };
        self.execute_query(index, value, context, bindings, budget, range)
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
    fn capture_record<'v>(
        &self,
        value: &'v ReportValue<'a>,
    ) -> Option<&'v zirql::NativeCapture<ReportValue<'a>>> {
        if let ReportValue::Capture(capture) = value {
            Some(capture)
        } else {
            None
        }
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
    fn execute_query(
        &self,
        index: usize,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        let resolved;
        let lowered;
        let pipeline = if let Some(pipeline) = &self.pipelines[index] {
            pipeline
        } else {
            resolved =
                self.resolve_query(&self.queries[index], context, bindings, budget, range)?;
            lowered = pipeline::lower_pipeline(&resolved).map_err(zirql::EvaluationError::Host)?;
            &lowered
        };
        if pipeline.stages().is_empty() {
            return Ok(value);
        }
        let ReportValue::Query(stream) = value else {
            return Err(zirql::EvaluationError::Host(QueryError::query(
                range.clone(),
                "query stages require a native query result",
            )));
        };
        let SemanticStream::Inspection(ref value) = stream else {
            return self.run_pipeline(pipeline, stream, context, budget, range);
        };
        if matches!(
            value.as_ref(),
            QueryResult::Count(_) | QueryResult::Tally(_)
        ) {
            if !pipeline.stages().is_empty() {
                return Err(zirql::EvaluationError::Host(QueryError::query(
                    range.clone(),
                    "query stages require a stream",
                )));
            }
            return Ok(ReportValue::Query(stream));
        }
        self.run_pipeline(pipeline, stream, context, budget, range)
    }
    fn resolve_query(
        &self,
        template: &ProgramQuery,
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Query, zirql::EvaluationError<QueryError>> {
        let units = template.copy_units.checked_mul(2).ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "query allocation size overflow")
        })?;
        budget.charge(units, zirql::Location::Query(range.clone()))?;
        budget.charge_results(units, zirql::Location::Query(range.clone()))?;
        let mut query = template.query.clone();
        for argument in &template.arguments {
            let range = &argument.expression.expression.range;
            let value = self.constructed(
                &argument.expression,
                &argument.names,
                context,
                bindings,
                budget,
                range,
            )?;
            if argument.kind.ty() == zirql::ParameterType::Int {
                let zirql::ConstructedValue::Integer(value) = value else {
                    return Err(zirql::EvaluationError::Host(QueryError::query(
                        range.clone(),
                        "primitive argument needs an int",
                    )));
                };
                let value = usize::try_from(value).map_err(|_| {
                    zirql::EvaluationError::Host(QueryError::query(
                        range.clone(),
                        "primitive argument needs a non-negative int fitting usize",
                    ))
                })?;
                let stage = query
                    .stages
                    .iter_mut()
                    .find(|stage| stage.range.start == argument.owner)
                    .expect("stage argument came from this template");
                match (&mut stage.kind, argument.kind) {
                    (language::StageKind::Head(count), language::ArgumentKind::Head)
                    | (
                        language::StageKind::TransitiveReferences(count),
                        language::ArgumentKind::TransitiveReferences,
                    ) => *count = value,
                    (language::StageKind::Range(bounds), language::ArgumentKind::RangeStart) => {
                        bounds.start = value
                    }
                    (language::StageKind::Range(bounds), language::ArgumentKind::RangeEnd) => {
                        bounds.end = value
                    }
                    _ => unreachable!("integer argument target came from this native stage"),
                }
            } else {
                let zirql::ConstructedValue::String(value) = value else {
                    return Err(zirql::EvaluationError::Host(QueryError::query(
                        range.clone(),
                        "primitive argument needs a string",
                    )));
                };
                let mut kind = Some(
                    argument
                        .kind
                        .predicate(value, range.clone())
                        .map_err(zirql::EvaluationError::Host)?,
                );
                for stage in &mut query.stages {
                    if let language::StageKind::Filter(predicate) = &mut stage.kind
                        && set_predicate_argument(
                            predicate,
                            argument.owner,
                            &mut kind,
                            budget,
                            range,
                        )?
                    {
                        break;
                    }
                }
                assert!(
                    kind.is_none(),
                    "predicate argument came from this native template"
                );
            }
        }
        for stage in &query.stages {
            if let language::StageKind::Range(bounds) = &stage.kind
                && bounds.start > bounds.end
            {
                return Err(zirql::EvaluationError::Host(QueryError::query(
                    stage.range.clone(),
                    "range start exceeds end",
                )));
            }
        }
        Ok(query)
    }

    fn constructed(
        &self,
        expression: &comprehension::LoweredValue,
        names: &[Located<String>],
        context: &mut Context<'_, 'a>,
        bindings: &Bindings<ReportValue<'a>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<zirql::ConstructedValue, zirql::EvaluationError<QueryError>> {
        let facts = if !expression.members.is_empty() || names.iter().any(|name| matches!(bindings.get(&name.value, name.range.clone()), Ok(ReportValue::Query(SemanticStream::Inspection(value))) if matches!(value.as_ref(), QueryResult::Literals(_)))) {
                    context.report_facts(budget, range).map_err(zirql::EvaluationError::Host)?
                } else { context.facts.clone() };
        let count = names
            .len()
            .checked_add(expression.members.len())
            .ok_or_else(|| zirql::QueryError::query(range.clone(), "projection size overflow"))?;
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
                ReportValue::Record(_) | ReportValue::Capture(_) | ReportValue::Captures(_) => {
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
        Ok(value)
    }

    fn run_pipeline(
        &self,
        pipeline: &NativePipeline<'_>,
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
        let value = zirql::evaluate(pipeline, &pipeline::NativeHost, input, budget)?.value;
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
    match value {
        ReportValue::Record(fields) => {
            budget
                .charge(fields.len() + 1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            budget
                .charge_results(fields.len() + 1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            let mut copied = Vec::with_capacity(fields.len());
            for (label, value) in fields {
                budget
                    .charge(label.len() + 1, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                copied.push((label.clone(), copy_value(value, budget, range)?));
            }
            return Ok(ReportValue::Record(copied));
        }
        ReportValue::Capture(capture) => {
            return copy_capture(capture, budget, range).map(ReportValue::Capture);
        }
        ReportValue::Captures(captures) => {
            budget
                .charge(
                    captures.rows.len() + 1,
                    zirql::Location::Query(range.clone()),
                )
                .map_err(pipeline::core_error)?;
            budget
                .charge_results(
                    captures.rows.len() + 1,
                    zirql::Location::Query(range.clone()),
                )
                .map_err(pipeline::core_error)?;
            let schema = composition::copy_schema(&captures.schema, budget, range)
                .map_err(pipeline::core_error)?;
            let rows = captures
                .rows
                .iter()
                .map(|row| copy_capture(row, budget, range))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(ReportValue::Captures(CaptureMatches { rows, schema }));
        }
        _ => {}
    }
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
fn copy_capture<'a>(
    capture: &zirql::NativeCapture<ReportValue<'a>>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<zirql::NativeCapture<ReportValue<'a>>, QueryError> {
    budget
        .charge_results(
            capture.fields.len() + 1,
            zirql::Location::Query(range.clone()),
        )
        .map_err(pipeline::core_error)?;
    let mut fields = Vec::with_capacity(capture.fields.len());
    for (name, value) in &capture.fields {
        budget
            .charge(name.len() + 1, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        fields.push((name.clone(), copy_value(value, budget, range)?));
    }
    Ok(zirql::NativeCapture { fields })
}

fn copy_admitted<'a>(value: &ReportValue<'a>) -> ReportValue<'a> {
    ReportValue::Query(match value {
        ReportValue::Constructed(_) | ReportValue::Capture(_) | ReportValue::Captures(_) => {
            unreachable!("structured copies use admitted paths")
        }
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
        ReportValue::Capture(capture) => {
            1 + capture
                .fields
                .iter()
                .map(|(_, value)| 1 + value_units(value))
                .sum::<usize>()
        }
        ReportValue::Captures(captures) => {
            1 + captures
                .rows
                .iter()
                .map(|capture| {
                    1 + capture
                        .fields
                        .iter()
                        .map(|(_, value)| 1 + value_units(value))
                        .sum::<usize>()
                })
                .sum::<usize>()
        }
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
        ReportValue::Capture(capture) => capture
            .fields
            .iter()
            .map(|(label, value)| label.len() + value_bytes(value))
            .sum(),
        ReportValue::Captures(captures) => captures
            .rows
            .iter()
            .map(|capture| {
                capture
                    .fields
                    .iter()
                    .map(|(label, value)| label.len() + value_bytes(value))
                    .sum::<usize>()
            })
            .sum(),
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
        ReportValue::Capture(capture) => {
            work.produce(1 + capture.fields.len(), range)?;
            for (label, value) in &capture.fields {
                work.spend(label.len(), range)?;
                admit_json(value, semantic, work, range)?;
            }
        }
        ReportValue::Captures(captures) => {
            work.produce(1 + captures.rows.len(), range)?;
            for capture in &captures.rows {
                work.produce(1 + capture.fields.len(), range)?;
                for (label, value) in &capture.fields {
                    work.spend(label.len(), range)?;
                    admit_json(value, semantic, work, range)?;
                }
            }
        }
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
        ReportValue::Capture(capture) => {
            for (_, value) in &capture.fields {
                validate_value(value, true, budget, range)?;
            }
        }
        ReportValue::Captures(captures) => {
            for capture in &captures.rows {
                for (_, value) in &capture.fields {
                    validate_value(value, true, budget, range)?;
                }
            }
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
        ReportValue::Capture(capture) => capture
            .fields
            .iter()
            .map(|(label, value)| Ok((label.clone(), report_json(value, semantic, budget, range)?)))
            .collect::<Result<serde_json::Map<_, _>, _>>()
            .map(Value::Object),
        ReportValue::Captures(captures) => captures
            .rows
            .iter()
            .map(|capture| {
                capture
                    .fields
                    .iter()
                    .map(|(label, value)| {
                        Ok((label.clone(), report_json(value, semantic, budget, range)?))
                    })
                    .collect::<Result<serde_json::Map<_, _>, _>>()
                    .map(Value::Object)
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
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
        ReportValue::Capture(_) | ReportValue::Captures(_) => unreachable!("captures use JSON"),
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
