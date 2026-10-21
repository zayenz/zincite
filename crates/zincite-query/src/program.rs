//! Native reports and atomic model-backed data batches using shared evaluation.
use crate::{
    Input, Limits, Query, QueryError, QueryResult, Selection, SemanticResult, SemanticStream, Work,
    comprehension, data, inspection, language, pipeline, semantic,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ops::Range, rc::Rc, sync::Arc};
use zincite_lint::{ModelContext, ModelFacts, SourceDiagnostic};
use zirql::{
    Bindings, Budget, EmissionPolicy, ExpressionKind, ExpressionStep, Located, NativeInput,
    NativeValue, ProgramHost, ProjectionHost,
};

pub(crate) mod builder;
pub use builder::{ProgramFunction, ProgramNative, ProgramValue};
mod composition;
mod functions;
mod native;
pub(crate) mod source_copy;
mod worklists;
pub use source_copy::SourceCopies;
pub use worklists::{Computational, NativeSequence, NativeTuple};

type Expression = zirql::Expression<Stage, zirql::NativeSetOperator, Emitter>;

#[derive(Debug)]
enum Stage {
    Parameter(usize),
    FunctionValue(zirql::ValueExpression<Expression>),
    FunctionSource(zirql::ValueExpression<Expression>),
    PatternBinding(zirql::Pattern, zirql::ValueExpression<Expression>),
    Primitive(
        functions::Primitive,
        Vec<zirql::ValueExpression<Expression>>,
    ),
    Member(comprehension::MemberRequest),
    Query(usize, Rc<Vec<QueryArgument>>),
    Ordering(native::OrderingStage),
    WhereExists(native::BodyStage),
    FixedPoint(native::BodyStage),
    Mapping(native::MappingStage),
    MapSorting,
    RetainPayload,
    Projection(composition::ProjectionStage),
    Match(composition::MatchStage),
    Paths(composition::PathStage),
    Filter(composition::FilterStage),
    Copy(source_copy::CopyStage),
    Data(Vec<data::Edit>, Vec<(Located<String>, Located<String>)>),
}

impl Drop for Stage {
    fn drop(&mut self) {
        let empty = || {
            Located::new(
                zirql::ValueExpressionKind::Literal(zirql::ConstructedValue::Absent),
                0..0,
            )
        };
        match self {
            Self::FunctionValue(value) | Self::FunctionSource(value) => {
                zirql::dispose_native_value(std::mem::replace(value, empty()))
            }
            Self::PatternBinding(pattern, value) => {
                zirql::dispose_pattern(std::mem::replace(
                    pattern,
                    Located::new(zirql::PatternKind::Wildcard, 0..0),
                ));
                zirql::dispose_native_value(std::mem::replace(value, empty()));
            }
            Self::Primitive(_, values) => {
                for value in std::mem::take(values) {
                    zirql::dispose_native_value(value);
                }
            }
            Self::Copy(_)
            | Self::Parameter(_)
            | Self::Member(_)
            | Self::Query(..)
            | Self::Ordering(_)
            | Self::WhereExists(_)
            | Self::FixedPoint(_)
            | Self::Mapping(_)
            | Self::MapSorting
            | Self::RetainPayload
            | Self::Projection(_)
            | Self::Match(_)
            | Self::Filter(_)
            | Self::Paths(_)
            | Self::Data(..) => {}
        }
    }
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
    Node,
    Computational(Rc<zirql::DeclaredType<Kind>>),
    Tuple(Rc<Vec<Kind>>),
    Sequence(Rc<zirql::DeclaredType<Kind>>),
    Declaration,
    Strings,
    Literals,
    MemberCopies,
    SourceCopies,
    Count,
    Tally,
    Record,
    Dictionary(Rc<Vec<Kind>>),
    Payload(Rc<Kind>),
    Entities,
    Declarations,
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
    /// Source copies are consumed only by an editing program's append statements.
    SourceCopies(Rc<SourceCopies<'a>>),
    Retained(Arc<Self>),
    /// A best-effort native selection that skipped an uncertain observation.
    /// Ordinary output remains available; required nested observations refuse it.
    IncompleteSelection {
        value: Box<Self>,
        error: Arc<QueryError>,
    },
    Computational(Computational<'a>),
    Tuple(NativeTuple<'a>),
    Sequence(NativeSequence<'a>),
    Item(Box<Self>),
    /// Retain the native entity element contract even for an empty stream.
    EntitySequence {
        declarations: bool,
        owner: Option<Rc<ModelFacts<'a>>>,
        value: Box<Self>,
    },
    Query(SemanticStream<'a>),
    Record(Vec<(String, ReportValue<'a>)>),
    Capture(zirql::NativeCapture<ReportValue<'a>>),
    Captures(CaptureMatches<'a>),
    Constructed(zirql::ConstructedValue),
    SharedConstructed(Arc<zirql::ConstructedValue>),
}
impl<'a> ReportValue<'a> {
    pub fn fields(&self) -> Option<&[(String, ReportValue<'a>)]> {
        match self {
            Self::Retained(value) => value.fields(),
            Self::Item(value)
            | Self::EntitySequence { value, .. }
            | Self::IncompleteSelection { value, .. } => value.fields(),
            Self::Record(fields) => Some(fields),
            Self::Capture(capture) => Some(&capture.fields),
            Self::SourceCopies(_)
            | Self::Computational(_)
            | Self::Tuple(_)
            | Self::Sequence(_)
            | Self::Query(_)
            | Self::Constructed(_)
            | Self::SharedConstructed(_)
            | Self::Captures(_) => None,
        }
    }
    pub fn stream(&self) -> Option<&SemanticStream<'a>> {
        match self {
            Self::Retained(value) => value.stream(),
            Self::Item(value)
            | Self::EntitySequence { value, .. }
            | Self::IncompleteSelection { value, .. } => value.stream(),
            Self::Query(stream) => Some(stream),
            Self::SourceCopies(_)
            | Self::Computational(_)
            | Self::Tuple(_)
            | Self::Sequence(_)
            | Self::Record(_)
            | Self::Constructed(_)
            | Self::SharedConstructed(_)
            | Self::Capture(_)
            | Self::Captures(_) => None,
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
                            &crate::inspection::tally_json(values),
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
    expression: zirql::ValueExpression<Expression>,
    eager_members: Vec<comprehension::MemberRequest>,
}
#[derive(Debug)]
struct ProgramQuery {
    query: Query,
    arguments: Rc<Vec<QueryArgument>>,
    copy_units: usize,
}

#[derive(Debug)]
pub struct Program {
    parameters: Vec<zirql::ParameterDeclaration>,
    program: zirql::Program<Stage, zirql::NativeSetOperator, Emitter>,
    preflight_work: usize,
    preflight_results: usize,
    preflight_range: Option<Range<usize>>,
    queries: Vec<ProgramQuery>,
    functions: Vec<zirql::UnresolvedPatternFunctionDeclaration<Expression>>,
    function_limits: zirql::FunctionLimits,
    report_model_demand: bool,
    editing: bool,
    prepared_values: bool,
    prepared: bool,
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
        let mut parser = Parser::new(source, limits);
        parser.collect_functions()?;
        parser.prepared = true;
        let mut statements = Vec::new();
        let mut parameters = Vec::new();
        let mut edits = Vec::new();
        let mut appends = Vec::new();
        let mut target_names = std::collections::BTreeSet::new();
        let mut output_seen = false;
        let mut invalid_binding = None;
        let mut document_seen = false;
        parser.cursor.space();
        while parser.cursor.position < source.len() {
            let start = parser.cursor.position;
            if let Some(range) = parser
                .function_ranges
                .iter()
                .find(|range| range.start == start)
            {
                parser.cursor.position = range.end;
                parser.cursor.space();
                continue;
            }
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
                        "let" | "report" | "json" | "emit" | "create" | "replace" | "emit_document"
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
            let append_start = parser.cursor.position;
            let append = !create && !replace && parser.cursor.take_word("append") && {
                parser.cursor.space();
                parser
                    .cursor
                    .current()
                    .is_some_and(|b| b.is_ascii_alphabetic() || b == b'\'')
            };
            if !create && !replace && !append {
                parser.cursor.position = append_start;
            }
            let scale_enum =
                !create && !replace && !append && parser.cursor.take_word("scale_enum") && {
                    parser.cursor.space();
                    parser
                        .cursor
                        .current()
                        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'\'')
                };
            if !create && !replace && !append && !scale_enum {
                parser.cursor.position = append_start;
            }
            if create || replace || append || scale_enum {
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
                if append {
                    let (expression, kind, emits) =
                        parser.composition_expression(expression_range.clone(), false, true)?;
                    if emits || !matches!(kind, Kind::MemberCopies | Kind::SourceCopies) {
                        return Err(QueryError::query(
                            expression_range,
                            "append requires native source copies",
                        ));
                    }
                    let slot = Located::new(format!("\0append{}", appends.len()), range.clone());
                    statements.push(Located::new(
                        zirql::Statement::Bind {
                            name: slot.clone(),
                            expression,
                        },
                        expression_range,
                    ));
                    appends.push((Located::new(name.to_owned(), range), slot));
                    parser.cursor.punctuation(b';', "expected ';' after edit")?;
                    parser.cursor.space();
                    continue;
                }
                let (_, names) = parser.pure_value::<()>(expression_range.clone())?;
                parser
                    .preflight
                    .charge(
                        expression_range.len() + name.len() + 1,
                        zirql::Location::Query(range.clone()),
                    )
                    .map_err(pipeline::core_error)?;
                edits.push(data::Edit {
                    kind: if scale_enum {
                        data::EditKind::ScaleEnum
                    } else if create {
                        data::EditKind::Create
                    } else {
                        data::EditKind::Replace
                    },
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
                if (edits.is_empty() && appends.is_empty()) || output_seen {
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
                            Stage::Data(std::mem::take(&mut edits), std::mem::take(&mut appends)),
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
            if !edits.is_empty() || !appends.is_empty() {
                return Err(parser.cursor.error("an edit batch must end with emit"));
            }
            let quoted_let = source[start..].starts_with("let")
                && parser
                    .lexed
                    .tokens()
                    .iter()
                    .filter(|token| {
                        token.range.start >= start
                            && !matches!(
                                token.kind,
                                zincite_syntax::TokenKind::Whitespace
                                    | zincite_syntax::TokenKind::LineComment
                                    | zincite_syntax::TokenKind::BlockComment
                            )
                    })
                    .nth(1)
                    .is_some_and(|token| token.kind == zincite_syntax::TokenKind::QuotedIdentifier);
            if quoted_let {
                parser.cursor.take_word("let");
            }
            let mut reader =
                zirql::ArgumentReader::new(source, start).map_err(pipeline::core_error)?;
            let pattern = if quoted_let {
                None
            } else {
                zirql::read_pattern_let(&mut reader, &mut parser.preflight, limits.nesting)
                    .map_err(pipeline::core_error)?
            };
            let is_let = pattern.is_some() || quoted_let;
            if let Some(pattern) = &pattern {
                parser.cursor.position = reader.position();
                for name in zirql::pattern_names(pattern, &mut parser.preflight)
                    .map_err(pipeline::core_error)?
                {
                    if !parser.minizinc_identifier(&name.range) {
                        invalid_binding = Some(name.range.clone());
                    }
                    if matches!(
                        name.value.as_str(),
                        "let" | "report" | "json" | "emit" | "create" | "replace" | "emit_document"
                    ) {
                        return Err(QueryError::query(
                            name.range.clone(),
                            "binding name is reserved by the program grammar",
                        ));
                    }
                    if parser.names.contains_key(&name.value) {
                        return Err(QueryError::query(
                            name.range.clone(),
                            "binding is already defined",
                        ));
                    }
                }
                if !matches!(pattern.value, zirql::PatternKind::Name(_)) {
                    parser.prepared = true;
                }
            }
            let binding = if quoted_let {
                let (name, range) = parser.identifier("expected a binding name")?;
                if !parser.minizinc_identifier(&range) {
                    invalid_binding = Some(range.clone());
                }
                if matches!(
                    name,
                    "let" | "report" | "json" | "emit" | "create" | "replace" | "emit_document"
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
            } else if let Some(Located {
                value: zirql::PatternKind::Name(name),
                ..
            }) = &pattern
            {
                Some(name.clone())
            } else {
                None
            };
            let report = !is_let && binding.is_none() && parser.cursor.take_word("report");
            parser.cursor.space();
            let expression_start = parser.cursor.position;
            let expression_end = match parser.expression_end() {
                Ok(end) => end,
                Err(_) if !is_let && binding.is_none() => source.len(),
                Err(error) => return Err(error),
            };
            let query_start = parser.queries.len();
            let (expression, kind, emits) =
                parser.composition_expression(expression_start..expression_end, !is_let, is_let)?;
            let editing_expression = parser.queries[query_start..]
                .iter()
                .any(|query| query.query.is_editing());
            if is_let && editing_expression {
                return Err(QueryError::query(
                    expression.range.clone(),
                    "a binding cannot perform an edit",
                ));
            }
            if (binding.is_some() || is_let) && emits {
                return Err(QueryError::query(
                    expression.range.clone(),
                    "a binding cannot emit",
                ));
            }
            if !(!is_let && binding.is_none() && parser.cursor.position == source.len()) {
                parser
                    .cursor
                    .punctuation(b';', "expected ';' after program statement")?;
            }
            output_seen |= binding.is_none() && !is_let;
            let statement = if let Some(pattern) =
                pattern.filter(|p| !matches!(p.value, zirql::PatternKind::Name(_)))
            {
                let names = zirql::pattern_names(&pattern, &mut parser.preflight)
                    .map_err(pipeline::core_error)?
                    .into_iter()
                    .cloned()
                    .collect::<Vec<_>>();
                for name in &names {
                    parser.names.insert(name.value.clone(), Kind::Constructed);
                }
                let range = expression.range.clone();
                let value = functions::into_value(expression);
                zirql::Statement::BindFields {
                    names,
                    expression: value_stage(Stage::PatternBinding(pattern, value), range),
                }
            } else if let Some(name) = binding {
                parser.names.insert(name.value.clone(), kind);
                zirql::Statement::Bind { name, expression }
            } else {
                zirql::Statement::Evaluate {
                    expression,
                    implicit_output: Some(
                        if report
                            || !editing_expression
                                && matches!(
                                    kind,
                                    Kind::Record
                                        | Kind::Dictionary(_)
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
        if !edits.is_empty() || !appends.is_empty() {
            return Err(parser.cursor.error("an edit batch must end with emit"));
        }
        let scalar_edit = parser.queries.iter().any(|query| {
            query
                .query
                .stages
                .iter()
                .any(|stage| matches!(stage.kind, language::StageKind::FilterElements(_)))
        });
        let editing = document_seen || parser.queries.iter().any(|query| query.query.is_editing());
        if !document_seen && let Some(range) = &parser.source_copy_range {
            return Err(QueryError::query(
                range.clone(),
                "source-copy stages require an editing program ending with emit",
            ));
        }
        if scalar_edit
            && (document_seen
                || parser
                    .queries
                    .iter()
                    .filter(|query| query.query.is_editing())
                    .count()
                    != 1
                || statements
                    .iter()
                    .filter(|statement| {
                        matches!(statement.value, zirql::Statement::Evaluate { .. })
                    })
                    .count()
                    != 1
                || statements.iter().any(|statement| {
                    matches!(
                        statement.value,
                        zirql::Statement::Evaluate {
                            implicit_output: Some(Emitter::Json),
                            ..
                        }
                    )
                }))
        {
            return Err(QueryError::query(
                0..source.len(),
                "a scalar edit program requires one edit pipeline and no other outputs",
            ));
        }
        if document_seen || scalar_edit {
            if !parser.functions.is_empty() {
                return Err(QueryError::query(
                    parser.functions[0].name.range.clone(),
                    "query functions are unavailable in data programs",
                ));
            }
            let mut budget =
                std::mem::replace(&mut parser.preflight, Budget::for_program(limits.core()));
            for statement in &statements {
                let expression = match &statement.value {
                    zirql::Statement::Bind { expression, .. }
                    | zirql::Statement::BindFields { expression, .. }
                    | zirql::Statement::Evaluate { expression, .. } => expression,
                };
                parser.admit_data_expression(expression, &mut budget, true)?;
            }
            parser.preflight = budget;
            if let Some(range) = invalid_binding {
                return Err(QueryError::query(
                    range,
                    "expected a MiniZinc identifier or quoted identifier",
                ));
            }
        }
        if statements.is_empty() {
            return Err(QueryError::query(
                0..source.len(),
                "expected a program statement",
            ));
        }
        if parser.prepared {
            let preserve_native = parser.functions.is_empty()
                && !statements.iter().any(|statement| {
                    matches!(statement.value, zirql::Statement::BindFields { .. })
                });
            statements =
                functions::wrap_program_entries(statements, preserve_native, &parser.queries);
        }
        Ok(Self {
            parameters,
            program: zirql::Program::new(statements, 0..source.len()),
            queries: std::mem::take(&mut parser.queries),
            functions: std::mem::take(&mut parser.functions),
            function_limits: zirql::FunctionLimits { active_depth: 64 },
            report_model_demand: parser.report_model_demand || document_seen,
            editing,
            prepared_values: parser.prepared_values,
            prepared: parser.prepared,
            preflight_work: limits
                .work
                .checked_sub(parser.preflight.remaining_work())
                .unwrap(),
            preflight_results: limits
                .results
                .checked_sub(parser.preflight.remaining_results().unwrap())
                .unwrap(),
            preflight_range: parser.preflight_range.take(),
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
    /// Set the independent active function depth; compatible tail calls reuse it.
    pub fn with_function_limits(mut self, limits: zirql::FunctionLimits) -> Self {
        self.function_limits = limits;
        self
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
                if query.arguments.is_empty() && !query.query.stages.iter().any(|stage| matches!(&stage.kind, language::StageKind::FilterElements(comparison) if comparison.predicate.is_some())) {
                    pipeline::lower_pipeline(&query.query, &mut budget).map(Some)
                } else {
                    Ok(None)
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut context = Context {
            functions: None,
            query: &mut context,
            input,
            facts,
            facts_admitted: false,
            parameters: parameter_values,
            emissions: Vec::new(),
            candidate: None,
            guard_unknown: None,
        };
        let mut written_values: Vec<&zirql::ValueExpression<Expression>> = self
            .functions
            .iter()
            .map(|function| &function.value)
            .collect();
        let mut written_native = self
            .program
            .statements()
            .iter()
            .map(|statement| match &statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::BindFields { expression, .. }
                | zirql::Statement::Evaluate { expression, .. } => expression,
            })
            .collect();
        admit_written_members(
            &mut written_native,
            &mut written_values,
            &mut context,
            &mut budget,
            limits.nesting,
        )?;
        let signatures = functions::resolve_signatures(&self.functions, &mut context, &mut budget)?;
        let host = NativeHost {
            functions: self.prepared,
            signatures: &signatures,
            declarations: &self.functions,
            parameters: &self.parameters,
            queries: &self.queries,
            pipelines,
            nesting: limits.nesting,
            limits: limits.query(),
            editing: self.editing,
        };

        let function_host = composition::CompositionHost(&host);
        let session = if !self.prepared && !self.prepared_values {
            None
        } else {
            Some(functions::Session::new(
                &self.functions,
                &signatures,
                &function_host,
                &self.program,
                self.function_limits,
                Kind::Nodes,
                &mut budget,
            )?)
        };
        context.functions = session.as_ref();
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
        zirql::dispose_unresolved_pattern_functions(std::mem::take(&mut self.functions));
        let program = std::mem::replace(&mut self.program, zirql::Program::new(Vec::new(), 0..0));
        for statement in program.into_statements() {
            let expression = match statement.value {
                zirql::Statement::Bind { expression, .. }
                | zirql::Statement::BindFields { expression, .. }
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
    functions: Vec<zirql::UnresolvedPatternFunctionDeclaration<Expression>>,
    function_ranges: Vec<Range<usize>>,
    function_names: std::collections::BTreeSet<String>,
    constructed_functions: std::collections::BTreeSet<String>,
    function_scope: Option<BTreeMap<String, Kind>>,
    reader_nesting: usize,
    report_model_demand: bool,
    prepared: bool,
    preflight: Budget,
    preflight_range: Option<Range<usize>>,
    lexed: zincite_syntax::LexedSource,
    composition_error: Option<QueryError>,
    prepared_values: bool,
    source_copy_range: Option<Range<usize>>,
}
impl<'a> Parser<'a> {
    fn new(source: &'a str, limits: ProgramLimits) -> Self {
        Self {
            cursor: language::Parser {
                source,
                position: 0,
                nesting_limit: limits.nesting.min(64),
                collection_limit: limits.stream_items,
                arguments: None,
                shared_budget: None,
            },
            limits,
            names: BTreeMap::new(),
            queries: Vec::new(),
            functions: Vec::new(),
            function_ranges: Vec::new(),
            function_names: std::collections::BTreeSet::new(),
            constructed_functions: std::collections::BTreeSet::new(),
            function_scope: None,
            reader_nesting: limits.nesting,
            report_model_demand: false,
            prepared: false,
            preflight: Budget::for_program(limits.core()),
            preflight_range: None,
            composition_error: None,
            prepared_values: false,
            source_copy_range: None,
            lexed: zincite_syntax::lex(source),
        }
    }
}
impl Drop for Parser<'_> {
    fn drop(&mut self) {
        zirql::dispose_unresolved_pattern_functions(std::mem::take(&mut self.functions));
    }
}
struct ConstructedTypes;
impl<Q> zirql::ValueQueryHost<Q> for ConstructedTypes {
    type Error = zirql::QueryError;
    fn query_types(
        &mut self,
        _: &Q,
        _: &[(&str, Vec<zirql::ValueType>)],
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<zirql::ValueType>, zirql::QueryError> {
        Err(zirql::QueryError::query(
            range.clone(),
            "native leaves are unavailable in constructed values",
        ))
    }
    fn evaluate_query(
        &mut self,
        _: &Q,
        _: &[(&str, zirql::ConstructedValue)],
        _: &mut Budget,
    ) -> Result<zirql::ConstructedValue, Self::Error> {
        unreachable!("CST metadata bridge")
    }
    fn read_binding(
        &mut self,
        _: &str,
        _: &Range<usize>,
        _: &mut Budget,
    ) -> Result<zirql::ConstructedValue, Self::Error> {
        unreachable!("CST metadata bridge")
    }
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
    fn pure_value<Q>(
        &mut self,
        range: Range<usize>,
    ) -> Result<(comprehension::LoweredValue<Q>, Vec<Located<String>>), QueryError> {
        self.begin_pure(range.clone())?;
        let expression = comprehension::read_data(
            &self.cursor.source[range.clone()],
            range.clone(),
            &mut self.preflight,
            self.reader_nesting,
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
    fn value_names<Q>(
        &mut self,
        expression: &comprehension::LoweredValue<Q>,
        range: &Range<usize>,
        expected: Option<language::ArgumentKind>,
    ) -> Result<Vec<Located<String>>, QueryError> {
        self.report_model_demand |= !expression.members.is_empty()
            || self
                .names
                .values()
                .any(|kind| matches!(kind, Kind::Literals));
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
        let types = types
            .into_iter()
            .map(|(name, ty)| (name, vec![ty]))
            .collect();
        let mut names = zirql::free_query_value_names(
            &expression.expression,
            &types,
            &mut ConstructedTypes,
            &mut self.preflight,
        )
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
        if let Some(expected) = expected
            && zirql::query_value_types(
                &expression.expression,
                &types,
                &mut ConstructedTypes,
                &mut self.preflight,
            )
            .map_err(pipeline::core_error)?
            .iter()
            .any(|ty| !expected.admits(ty))
        {
            return Err(QueryError::query(
                expression.expression.range.clone(),
                "primitive argument has an incompatible scalar type",
            ));
        }
        Ok(names)
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

// Recognize native binding records before their parser
// performs name/type/purity validation. This never mutates the query table.
fn admit_argument_members(
    argument: &QueryArgument,
    context: &mut Context<'_, '_>,
    budget: &mut Budget,
) -> Result<(), zirql::EvaluationError<QueryError>> {
    for member in &argument.eager_members {
        let facts = context
            .report_facts(budget, &member.range)
            .map_err(zirql::EvaluationError::Host)?;
        comprehension::resolve_member(
            facts.as_deref(),
            Some(&member.enum_name.value),
            &member.member_name.value,
            budget,
            &member.range,
        )
        .map_err(zirql::EvaluationError::Host)?;
    }
    Ok(())
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
    }
    Ok(false)
}

fn validate_stages(query: &Query, mut kind: Kind) -> Result<Kind, QueryError> {
    kind = match kind {
        Kind::Node => Kind::Nodes,
        Kind::Declaration => Kind::Declarations,
        other => other,
    };
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
                        | Kind::Declarations
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
                Kind::Nodes
                    | Kind::Strings
                    | Kind::Entities
                    | Kind::Declarations
                    | Kind::Types
                    | Kind::Instantiations
            ),
            S::Head(_) | S::Tail(_) | S::Reverse
                if matches!(kind, Kind::Dictionary(_) | Kind::Record | Kind::Tally) =>
            {
                true
            }
            S::Head(_) | S::Tail(_) | S::Reverse | S::Check { .. } => matches!(
                kind,
                Kind::Nodes
                    | Kind::Strings
                    | Kind::Literals
                    | Kind::Entities
                    | Kind::Declarations
                    | Kind::Types
                    | Kind::Instantiations
            ),
            S::Range(_) => matches!(kind, Kind::Nodes),
            S::Filter(_) | S::SharedFilter(_) => matches!(
                kind,
                Kind::Nodes
                    | Kind::Entities
                    | Kind::Declarations
                    | Kind::Types
                    | Kind::Instantiations
            ),
            S::Declarations => {
                let valid = matches!(
                    kind,
                    Kind::Nodes
                        | Kind::Entities
                        | Kind::Declarations
                        | Kind::Types
                        | Kind::Instantiations
                );
                kind = Kind::Declarations;
                valid
            }
            S::References | S::Uses | S::TransitiveReferences(_) => {
                let valid = matches!(
                    kind,
                    Kind::Nodes
                        | Kind::Entities
                        | Kind::Declarations
                        | Kind::Types
                        | Kind::Instantiations
                );
                kind = Kind::Entities;
                valid
            }
            S::ReferencedDeclarations => matches!(kind, Kind::Declarations),
            S::Types => {
                let valid = matches!(
                    kind,
                    Kind::Nodes
                        | Kind::Entities
                        | Kind::Declarations
                        | Kind::Types
                        | Kind::Instantiations
                );
                kind = Kind::Types;
                valid
            }
            S::Instantiations => {
                let valid = matches!(
                    kind,
                    Kind::Nodes
                        | Kind::Entities
                        | Kind::Declarations
                        | Kind::Types
                        | Kind::Instantiations
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
                    Kind::Nodes
                        | Kind::Entities
                        | Kind::Declarations
                        | Kind::Types
                        | Kind::Instantiations
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
    functions: Option<&'c functions::Session<'c, 'a>>,
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
struct ReportReads<'r, 'c, 'a> {
    context: &'r mut Context<'c, 'a>,
    bindings: &'r Bindings<ReportValue<'a>>,
    members: &'r [comprehension::MemberRequest],
    projected: BTreeMap<String, std::sync::Arc<zirql::ConstructedValue>>,
    nesting: usize,
}
impl zirql::ValueQueryHost<()> for ReportReads<'_, '_, '_> {
    type Error = zirql::EvaluationError<QueryError>;
    fn query_types(
        &mut self,
        _: &(),
        _: &[(&str, Vec<zirql::ValueType>)],
        _: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Vec<zirql::ValueType>, zirql::QueryError> {
        Err(zirql::QueryError::query(
            range.clone(),
            "native leaves are unavailable in constructed values",
        ))
    }
    fn evaluate_query(
        &mut self,
        _: &(),
        _: &[(&str, zirql::ConstructedValue)],
        _: &mut Budget,
    ) -> Result<zirql::ConstructedValue, Self::Error> {
        unreachable!("constructed Data values have no native leaves")
    }
    fn read_shared_binding(
        &mut self,
        name: &str,
        range: &Range<usize>,
        budget: &mut Budget,
    ) -> Result<Option<std::sync::Arc<zirql::ConstructedValue>>, Self::Error> {
        budget.charge(1, zirql::Location::Query(range.clone()))?;
        Ok(self.projected.get(name).map(std::sync::Arc::clone))
    }
    fn read_binding(
        &mut self,
        name: &str,
        range: &Range<usize>,
        budget: &mut Budget,
    ) -> Result<zirql::ConstructedValue, Self::Error> {
        if let Some(value) = self.projected.get(name) {
            return Ok(zirql::copy_constructed_value(
                value,
                budget,
                zirql::Location::Query(range.clone()),
            )?);
        }
        for member in self.members {
            budget.charge(
                1 + name.len().min(member.slot.len()),
                zirql::Location::Query(range.clone()),
            )?;
            if member.slot == name {
                let facts = self
                    .context
                    .report_facts(budget, range)
                    .map_err(zirql::EvaluationError::Host)?;
                return comprehension::resolve_member(
                    facts.as_deref(),
                    Some(&member.enum_name.value),
                    &member.member_name.value,
                    budget,
                    &member.range,
                )
                .map(zirql::ConstructedValue::Member)
                .map_err(zirql::EvaluationError::Host);
            }
        }
        match functions::unretained(self.bindings.get(name, range.clone())?) {
            ReportValue::Constructed(value) => Ok(zirql::copy_constructed_value(
                value,
                budget,
                zirql::Location::Query(range.clone()),
            )?),
            ReportValue::SharedConstructed(value) => Ok(zirql::copy_constructed_value(
                value,
                budget,
                zirql::Location::Query(range.clone()),
            )?),
            ReportValue::Query(stream) => {
                let facts = if matches!(stream, SemanticStream::Inspection(value) if matches!(value.as_ref(), QueryResult::Literals(_)))
                {
                    self.context
                        .report_facts(budget, range)
                        .map_err(zirql::EvaluationError::Host)?
                } else {
                    self.context.facts.clone()
                };
                comprehension::project(stream, facts.as_deref(), budget, range, self.nesting)
                    .map_err(zirql::EvaluationError::Host)
            }
            _ => Err(zirql::EvaluationError::Host(QueryError::query(
                range.clone(),
                "native records require explicit scalar projections",
            ))),
        }
    }
}
struct NativeHost<'q> {
    functions: bool,
    signatures: &'q [zirql::ResolvedPatternFunctionSignature<'q, Kind>],
    declarations: &'q [zirql::UnresolvedPatternFunctionDeclaration<Expression>],
    parameters: &'q [zirql::ParameterDeclaration],
    queries: &'q [ProgramQuery],
    pipelines: Vec<Option<NativePipeline<'q>>>,
    editing: bool,
    nesting: usize,
    limits: Limits,
}
impl<'a, 'q> ProgramHost<Context<'_, 'a>> for NativeHost<'q> {
    type Value = ReportValue<'a>;
    type Stage = Stage;
    type SetOperator = zirql::NativeSetOperator;
    type Emitter = Emitter;
    type Error = QueryError;
    fn copy_value(
        &self,
        value: &ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        if context.query.complete_observations {
            native::require_complete(value, budget, range)?;
        }
        if let ReportValue::SharedConstructed(owner) = value {
            budget.charge(1, zirql::Location::Query(range.clone()))?;
            return Ok(ReportValue::SharedConstructed(Arc::clone(owner)));
        }
        if let ReportValue::Retained(owner) = value {
            budget.charge(1, zirql::Location::Query(range.clone()))?;
            return Ok(ReportValue::Retained(Arc::clone(owner)));
        }
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
        let partial = native::selection_error(&value).cloned();
        if context.query.complete_observations {
            native::require_complete(&value, budget, range)?;
        }
        let result = (|| {
            let index = match stage {
                Stage::FunctionValue(expression) | Stage::FunctionSource(expression) => {
                    let session = context.functions.ok_or_else(|| {
                        zirql::EvaluationError::Host(QueryError::query(
                            range.clone(),
                            "function session is unavailable",
                        ))
                    })?;
                    return session
                        .run(expression, bindings, &value, context, budget)
                        .map_err(zirql::EvaluationError::Host);
                }
                Stage::PatternBinding(_, expression) => {
                    let session = context.functions.expect("prepared pattern program");
                    return session
                        .bind(expression, bindings, &value, context, budget)
                        .map_err(zirql::EvaluationError::Host);
                }
                Stage::Primitive(_, _) => {
                    return Err(zirql::QueryError::query(
                        range.clone(),
                        "native function primitive requires its active argument runner",
                    )
                    .into());
                }
                Stage::Member(member) => {
                    let facts = context
                        .report_facts(budget, &member.range)
                        .map_err(zirql::EvaluationError::Host)?;
                    return comprehension::resolve_member(
                        facts.as_deref(),
                        Some(&member.enum_name.value),
                        &member.member_name.value,
                        budget,
                        &member.range,
                    )
                    .map(zirql::ConstructedValue::Member)
                    .map(ReportValue::Constructed)
                    .map_err(zirql::EvaluationError::Host);
                }
                Stage::Parameter(index) => {
                    return Ok(ReportValue::Constructed(
                        context.parameters[*index]
                            .take()
                            .expect("admitted parameter is bound once"),
                    ));
                }
                Stage::Query(index, _) => *index,
                Stage::RetainPayload => return Ok(value),
                Stage::Mapping(mapping) => {
                    return composition::CompositionHost(self)
                        .mapping(mapping, value, context, bindings, budget, range);
                }
                Stage::MapSorting => {
                    return composition::CompositionHost(self)
                        .sort_map(value, context, budget, range);
                }
                Stage::WhereExists(body) => {
                    return composition::CompositionHost(self)
                        .where_exists(body, value, context, bindings, budget, range);
                }
                Stage::FixedPoint(body) => {
                    return composition::CompositionHost(self)
                        .fixed_point(body, value, context, bindings, budget, range);
                }
                Stage::Ordering(ordering) => {
                    return composition::CompositionHost(self)
                        .order(ordering, value, context, bindings, budget, range);
                }
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
                Stage::Filter(predicate) => {
                    return composition::CompositionHost(self).guard_filter(
                        predicate.get(),
                        value,
                        context,
                        bindings,
                        budget,
                        range,
                    );
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
                Stage::Paths(paths) => {
                    return composition::CompositionHost(self).paths(
                        paths.get(),
                        value,
                        context,
                        bindings,
                        budget,
                    );
                }
                Stage::Copy(stage) => {
                    return self.source_copy(stage, value, context, bindings, budget, range);
                }
                Stage::Data(edits, appends) => {
                    let facts = context
                        .report_facts(budget, range)
                        .map_err(zirql::EvaluationError::Host)?;
                    let facts = facts.ok_or_else(|| {
                        zirql::EvaluationError::Host(QueryError::query(
                            range.clone(),
                            "data construction requires read-only model context (--model)",
                        ))
                    })?;
                    let mut copies = Vec::new();
                    for (name, slot) in appends {
                        budget.charge_results(1, zirql::Location::Query(name.range.clone()))?;
                        let ReportValue::SourceCopies(copies_value) =
                            functions::unretained(bindings.get(&slot.value, slot.range.clone())?)
                        else {
                            return Err(zirql::QueryError::query(
                                name.range.clone(),
                                "append requires native source copies",
                            )
                            .into());
                        };
                        copies.push((name, Rc::clone(copies_value)));
                    }
                    let candidate = data::prepare_data_batch(
                        context.input,
                        &facts,
                        edits,
                        &copies,
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
        })();
        result.and_then(|value| native::incomplete(value, partial, budget, range))
    }
    fn combine(
        &self,
        operator: &zirql::NativeSetOperator,
        left: ReportValue<'a>,
        right: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        composition::CompositionHost(self)
            .combine_set(*operator, left, right, context, budget, range)
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
        if let ReportValue::Capture(capture) = functions::unretained(value) {
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
        let declarations = functions::query_declarations(&self.queries[index].query, &value);
        let resolved;
        let lowered;
        let pipeline = if let Some(pipeline) = &self.pipelines[index] {
            pipeline
        } else {
            resolved =
                self.resolve_query(&self.queries[index], context, bindings, budget, range)?;
            lowered = pipeline::lower_pipeline(&resolved, budget)
                .map_err(zirql::EvaluationError::Host)?;
            &lowered
        };
        let retained = if self.queries[index].query.is_editing() {
            Some(self.copy_value(&value, context, budget, range)?)
        } else {
            None
        };
        let result = self.execute_pipeline(pipeline, value, context, budget, range)?;
        if let ReportValue::Query(SemanticStream::Inspection(result)) = &result
            && let QueryResult::Document(bytes) = result.as_ref()
        {
            budget.charge(
                bytes.len().saturating_mul(2),
                zirql::Location::Query(range.clone()),
            )?;
            let candidate = Input::parse_named(
                bytes.clone(),
                zincite_syntax::FileMode::Data,
                context.input.file(),
            )
            .map_err(|error| {
                zirql::EvaluationError::Host(QueryError::query(
                    range.clone(),
                    format!(
                        "invalid data edit candidate: {}",
                        error.diagnostics[0].message
                    ),
                ))
            })?;
            context.candidate = Some(candidate);
            return Ok(retained.expect("document results come from edit stages"));
        }
        self.retain_entity_contract(result, declarations, context, budget, range)
    }
    fn retain_entity_contract(
        &self,
        value: ReportValue<'a>,
        declarations: bool,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        if matches!(
            functions::unretained(&value),
            ReportValue::Query(SemanticStream::Entities(_))
        ) {
            budget.charge_results(1, zirql::Location::Query(range.clone()))?;
            Ok(ReportValue::EntitySequence {
                declarations,
                owner: context.facts.clone(),
                value: Box::new(value),
            })
        } else {
            Ok(value)
        }
    }
    fn execute_pipeline(
        &self,
        pipeline: &NativePipeline<'_>,
        value: ReportValue<'a>,
        context: &mut Context<'_, 'a>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<ReportValue<'a>, zirql::EvaluationError<QueryError>> {
        if pipeline.stages().is_empty() {
            return Ok(value);
        }
        let value = functions::owned(value, budget, range).map_err(zirql::EvaluationError::Host)?;
        let dictionary = matches!(&value, ReportValue::Record(_))
            || matches!(&value, ReportValue::Query(SemanticStream::Inspection(result)) if matches!(result.as_ref(), QueryResult::Tally(_)));
        if dictionary {
            return native::bound_map(value, pipeline, budget, range);
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
        let mut query = self.resolve_query_by(
            template,
            &mut |argument, budget| {
                admit_argument_members(argument, context, budget)?;
                zirql::evaluate_native_value(
                    &argument.expression,
                    &composition::CompositionHost(self),
                    ReportValue::Record(Vec::new()),
                    context,
                    bindings,
                    budget,
                )
            },
            budget,
            range,
        )?;
        for stage in &mut query.stages {
            if let language::StageKind::FilterElements(comparison) = &mut stage.kind
                && let Some(predicate) = &comparison.predicate
            {
                let mut types = BTreeMap::new();
                for (name, _) in bindings.iter() {
                    types.insert(name.to_owned(), zirql::ValueType::Unestablished);
                }
                types.insert("\0scalar".into(), zirql::ValueType::ScalarKind);
                let names = zirql::free_value_names(predicate, &types, budget)?;
                let mut selected = Vec::new();
                for name in names {
                    if name.value == "\0scalar" {
                        continue;
                    }
                    let value = bindings.get(&name.value, name.range.clone())?;
                    let value = composition::CompositionHost(self).project_value(
                        self.copy_value(value, context, budget, &name.range)?,
                        context,
                        budget,
                        &name.range,
                    )?;
                    selected.push((name, value));
                }
                comparison.bindings = Rc::new(Bindings::new(selected)?);
            }
        }
        Ok(query)
    }

    fn resolve_query_by(
        &self,
        template: &ProgramQuery,
        read: &mut impl FnMut(
            &QueryArgument,
            &mut Budget,
        )
            -> Result<zirql::ConstructedValue, zirql::EvaluationError<QueryError>>,
        budget: &mut Budget,
        range: &Range<usize>,
    ) -> Result<Query, zirql::EvaluationError<QueryError>> {
        let units = template.copy_units.checked_mul(2).ok_or_else(|| {
            zirql::QueryError::query(range.clone(), "query allocation size overflow")
        })?;
        budget.charge(units, zirql::Location::Query(range.clone()))?;
        budget.charge_results(units, zirql::Location::Query(range.clone()))?;
        let mut query = template.query.clone();
        for argument in template.arguments.iter() {
            let range = &argument.expression.range;
            let value = read(argument, budget)?;
            let value = value.into_data();
            let ty = match &value {
                zirql::ConstructedValue::Integer(_) => zirql::ValueType::Integer,
                zirql::ConstructedValue::String(_) => zirql::ValueType::String,
                _ => {
                    return Err(zirql::EvaluationError::Host(QueryError::query(
                        range.clone(),
                        "primitive argument has an incompatible scalar type",
                    )));
                }
            };
            if !argument.kind.admits(&ty) {
                return Err(zirql::EvaluationError::Host(QueryError::query(
                    range.clone(),
                    "primitive argument has an incompatible scalar type",
                )));
            }
            if let zirql::ConstructedValue::Integer(value) = value {
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
                    | (language::StageKind::Tail(count), language::ArgumentKind::Tail)
                    | (
                        language::StageKind::TransitiveReferences(count),
                        language::ArgumentKind::TransitiveReferences,
                    ) => *count = value,
                    (
                        language::StageKind::Check { expected, .. },
                        language::ArgumentKind::CheckCount | language::ArgumentKind::CheckOperand,
                    ) => *expected = Some(value),
                    (language::StageKind::Range(bounds), language::ArgumentKind::RangeStart) => {
                        bounds.start = value
                    }
                    (language::StageKind::Range(bounds), language::ArgumentKind::RangeEnd) => {
                        bounds.end = value
                    }
                    _ => unreachable!("integer argument target came from this native stage"),
                }
            } else if let zirql::ConstructedValue::String(value) = value {
                if matches!(
                    argument.kind,
                    language::ArgumentKind::CheckMessage | language::ArgumentKind::CheckOperand
                ) {
                    let stage = query
                        .stages
                        .iter_mut()
                        .find(|stage| stage.range.start == argument.owner)
                        .expect("stage argument came from this template");
                    let language::StageKind::Check { expected, message } = &mut stage.kind else {
                        unreachable!()
                    };
                    if matches!(argument.kind, language::ArgumentKind::CheckOperand) {
                        *expected = None;
                    }
                    *message = Some(value);
                    continue;
                }
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
        if !self.editing {
            let mut types = BTreeMap::new();
            let mut eager = std::collections::BTreeSet::new();
            comprehension::eager_names(&expression.expression, &mut eager, budget)
                .map_err(zirql::EvaluationError::Host)?;
            let mut reads = ReportReads {
                context,
                bindings,
                members: &expression.members,
                projected: BTreeMap::new(),
                nesting: self.nesting,
            };
            for name in names {
                budget.charge(
                    name.value.len() + 1,
                    zirql::Location::Query(name.range.clone()),
                )?;
                let value = bindings.get(&name.value, name.range.clone())?;
                let ty = if eager.contains(&name.value) {
                    let value = zirql::ValueQueryHost::read_binding(
                        &mut reads,
                        &name.value,
                        &name.range,
                        budget,
                    )?;
                    let ty = zirql::constructed_value_type(
                        &value,
                        budget,
                        zirql::Location::Query(name.range.clone()),
                    )?;
                    budget.charge_results(1, zirql::Location::Query(name.range.clone()))?;
                    reads
                        .projected
                        .insert(name.value.clone(), std::sync::Arc::new(value));
                    ty
                } else {
                    match functions::unretained(value) {
                        ReportValue::Retained(_)
                        | ReportValue::Item(_)
                        | ReportValue::EntitySequence { .. } => {
                            unreachable!("unwrapped report")
                        }
                        ReportValue::Constructed(value) => {
                            comprehension::value_facts(value, budget, &name.range)?
                        }
                        ReportValue::Query(SemanticStream::Inspection(value)) => {
                            match value.as_ref() {
                                QueryResult::Count(_) => zirql::ValueType::Integer,
                                QueryResult::Strings(_) => {
                                    zirql::ValueType::Array(zirql::ArrayType::Elements(vec![
                                        zirql::ValueType::String,
                                    ]))
                                }
                                QueryResult::Literals(_) => {
                                    zirql::ValueType::Array(zirql::ArrayType::Elements(Vec::new()))
                                }
                                _ => zirql::ValueType::Unestablished,
                            }
                        }
                        _ => zirql::ValueType::Unestablished,
                    }
                };
                budget.charge_results(2, zirql::Location::Query(name.range.clone()))?;
                types.insert(name.value.clone(), vec![ty]);
            }
            for member in &expression.members {
                budget.charge(
                    member.slot.len() + 1,
                    zirql::Location::Query(member.range.clone()),
                )?;
                let ty = if eager.contains(&member.slot) {
                    let value = zirql::ValueQueryHost::read_binding(
                        &mut reads,
                        &member.slot,
                        &member.range,
                        budget,
                    )?;
                    let ty = zirql::constructed_value_type(
                        &value,
                        budget,
                        zirql::Location::Query(member.range.clone()),
                    )?;
                    budget.charge_results(1, zirql::Location::Query(member.range.clone()))?;
                    reads
                        .projected
                        .insert(member.slot.clone(), std::sync::Arc::new(value));
                    ty
                } else {
                    zirql::ValueType::Unestablished
                };
                budget.charge_results(2, zirql::Location::Query(member.range.clone()))?;
                types.insert(member.slot.clone(), vec![ty]);
            }
            return zirql::evaluate_query_value(&expression.expression, &types, &mut reads, budget);
        }
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
            let value = match functions::unretained(value) {
                ReportValue::IncompleteSelection { .. }
                | ReportValue::Retained(_)
                | ReportValue::Item(_)
                | ReportValue::EntitySequence { .. } => unreachable!("unwrapped report"),
                ReportValue::Constructed(value) => zirql::copy_constructed_value(
                    value,
                    budget,
                    zirql::Location::Query(name.range.clone()),
                )?,
                ReportValue::SharedConstructed(value) => zirql::copy_constructed_value(
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
                ReportValue::SourceCopies(_)
                | ReportValue::Computational(_)
                | ReportValue::Tuple(_)
                | ReportValue::Sequence(_)
                | ReportValue::Record(_)
                | ReportValue::Capture(_)
                | ReportValue::Captures(_) => {
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
            comprehension::validate_data_value,
        )?;
        comprehension::validate_data_shape(&value, budget, range)
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
        let previous = context.query.incomplete_observation.take();
        let rows = context.query.take_rows(stream);
        let input = NativeInput::from_admitted(
            &mut *context.query,
            rows,
            budget,
            zirql::Location::Query(range.clone()),
        )?;
        let result = zirql::evaluate(pipeline, &pipeline::NativeHost, input, budget)
            .map(|result| result.value);
        let partial = std::mem::replace(&mut context.query.incomplete_observation, previous);
        let value = result?;
        let stream = match value {
            NativeValue::Stream(rows) => context
                .query
                .terminal
                .take()
                .unwrap_or_else(|| context.query.restore_stream(rows)),
            NativeValue::Count(value) => {
                SemanticStream::Inspection(Box::new(QueryResult::Count(value)))
            }
            NativeValue::Tally(value) => {
                SemanticStream::Inspection(Box::new(QueryResult::Tally(value)))
            }
        };
        let value = native::incomplete(ReportValue::Query(stream), partial, budget, range)?;
        if context.query.complete_observations {
            native::require_complete(&value, budget, range)?;
        }
        Ok(value)
    }
}

fn copy_value<'a>(
    value: &ReportValue<'a>,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<ReportValue<'a>, QueryError> {
    if let Some(value) = worklists::copy(value) {
        budget
            .charge(1, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        budget
            .charge_results(1, zirql::Location::Query(range.clone()))
            .map_err(pipeline::core_error)?;
        return Ok(value);
    }
    match value {
        ReportValue::SourceCopies(value) => {
            budget
                .charge(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            return Ok(ReportValue::SourceCopies(Rc::clone(value)));
        }
        ReportValue::Retained(value) => return copy_value(value, budget, range),
        ReportValue::IncompleteSelection { value, error } => {
            let value = copy_value(value, budget, range)?;
            return native::incomplete(value, Some(Arc::clone(error)), budget, range)
                .map_err(pipeline::evaluation_error);
        }
        ReportValue::EntitySequence {
            declarations,
            owner,
            value,
        } => {
            return Ok(ReportValue::EntitySequence {
                declarations: *declarations,
                owner: owner.clone(),
                value: Box::new(copy_value(value, budget, range)?),
            });
        }
        ReportValue::Item(value) => {
            return Ok(ReportValue::Item(Box::new(copy_value(
                value, budget, range,
            )?)));
        }
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
    if let ReportValue::SharedConstructed(value) = value {
        return zirql::copy_constructed_value(value, budget, zirql::Location::Query(range.clone()))
            .map(ReportValue::Constructed)
            .map_err(pipeline::core_error);
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
    if let Some(value) = worklists::copy(value) {
        return value;
    }
    ReportValue::Query(match value {
        ReportValue::SourceCopies(value) => return ReportValue::SourceCopies(Rc::clone(value)),
        ReportValue::Retained(value) => return ReportValue::Retained(Arc::clone(value)),
        ReportValue::Item(value) => return ReportValue::Item(Box::new(copy_admitted(value))),
        ReportValue::IncompleteSelection { value, error } => {
            return ReportValue::IncompleteSelection {
                value: Box::new(copy_admitted(value)),
                error: Arc::clone(error),
            };
        }
        ReportValue::EntitySequence {
            declarations,
            owner,
            value,
        } => {
            return ReportValue::EntitySequence {
                declarations: *declarations,
                owner: owner.clone(),
                value: Box::new(copy_admitted(value)),
            };
        }
        ReportValue::Computational(_)
        | ReportValue::Tuple(_)
        | ReportValue::Sequence(_)
        | ReportValue::Constructed(_)
        | ReportValue::SharedConstructed(_)
        | ReportValue::Capture(_)
        | ReportValue::Captures(_) => {
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
        ReportValue::SourceCopies(_)
        | ReportValue::Computational(_)
        | ReportValue::Tuple(_)
        | ReportValue::Sequence(_) => 1,
        ReportValue::Retained(value) => value_units(value),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => value_units(value),
        ReportValue::Constructed(_) | ReportValue::SharedConstructed(_) => {
            unreachable!("constructed admission uses the core")
        }
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
        ReportValue::Retained(value) => value_bytes(value),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => value_bytes(value),
        ReportValue::Constructed(_) | ReportValue::SharedConstructed(_) => {
            unreachable!("constructed admission uses the core")
        }
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
            QueryResult::Tally(values) => values.iter().map(|(key, _)| key.len()).sum(),
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
        ReportValue::SourceCopies(_) => {
            return Err(QueryError::query(
                range.clone(),
                "source copies can only be consumed by append",
            ));
        }
        ReportValue::Computational(value) => {
            return Err(QueryError::query(
                value.range(),
                "queue and identity_set values cannot be rendered",
            ));
        }
        ReportValue::Tuple(value) => {
            work.produce(value.values().len() + 1, range)?;
            for value in value.values() {
                admit_json(value, semantic, work, range)?;
            }
        }
        ReportValue::Sequence(value) => {
            work.produce(value.values().len() + 1, range)?;
            for value in value.values() {
                admit_json(value, semantic, work, range)?;
            }
        }
        ReportValue::Retained(value) => return admit_json(value, semantic, work, range),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => {
            return admit_json(value, semantic, work, range);
        }
        ReportValue::Constructed(_) | ReportValue::SharedConstructed(_) => {}
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
        ReportValue::SourceCopies(_) => {
            return Err(QueryError::query(
                range.clone(),
                "source copies can only be consumed by append",
            ));
        }
        // Computational values are valid retained library results. Rendering
        // rejects them recursively in admit_json, before any bytes escape.
        ReportValue::Tuple(value) => {
            for value in value.values() {
                validate_value(value, true, budget, range)?;
            }
        }
        ReportValue::Sequence(value) => {
            for value in value.values() {
                validate_value(value, true, budget, range)?;
            }
        }
        ReportValue::Retained(value) => return validate_value(value, json, budget, range),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => {
            return validate_value(value, json, budget, range);
        }
        ReportValue::Constructed(value) => {
            comprehension::validate_report_shape(value, budget, range)?
        }
        ReportValue::SharedConstructed(value) => {
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
                crate::validate_fragment_directives(
                    selection.input,
                    &selection.items,
                    &mut Work { budget },
                    range,
                )?;
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
        ReportValue::SourceCopies(_) => Err(QueryError::query(
            range.clone(),
            "source copies can only be consumed by append",
        )),
        ReportValue::Computational(value) => Err(QueryError::query(
            value.range(),
            "queue and identity_set values cannot be rendered",
        )),
        ReportValue::Tuple(value) => value
            .values()
            .iter()
            .map(|v| report_json(v, semantic, budget, range))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        ReportValue::Sequence(value) => value
            .values()
            .iter()
            .map(|v| report_json(v, semantic, budget, range))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        ReportValue::Retained(value) => report_json(value, semantic, budget, range),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => {
            report_json(value, semantic, budget, range)
        }
        ReportValue::Constructed(value) => comprehension::json_value(value, budget, range),
        ReportValue::SharedConstructed(value) => comprehension::json_value(value, budget, range),
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
        ReportValue::SourceCopies(_) => 0,
        ReportValue::Computational(_) | ReportValue::Tuple(_) | ReportValue::Sequence(_) => {
            unreachable!("native containers use JSON")
        }
        ReportValue::Retained(value) => render_bytes(value, semantic),
        ReportValue::Item(value)
        | ReportValue::EntitySequence { value, .. }
        | ReportValue::IncompleteSelection { value, .. } => render_bytes(value, semantic),
        ReportValue::Constructed(_) | ReportValue::SharedConstructed(_) => {
            unreachable!("constructed reports use tagged JSON")
        }
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

/// The fixed Query API retains the same shared predicate and native adapter.
#[derive(Debug)]
pub(crate) struct QueryPredicate {
    filter: composition::FilterStage,
    queries: Vec<ProgramQuery>,
    work: usize,
    nesting: usize,
    report_model_demand: bool,
}
impl QueryPredicate {
    pub(crate) fn requires_model_facts(&self) -> bool {
        self.report_model_demand
            || self
                .queries
                .iter()
                .any(|query| query.query.requires_model_facts())
    }
}
pub(crate) fn read_query_predicate(
    source: &str,
    start: usize,
    limits: Limits,
    budget: &mut Budget,
) -> Result<(Rc<QueryPredicate>, usize), QueryError> {
    let limits = ProgramLimits {
        nesting: limits.nesting,
        work: limits.work,
        stream_items: limits.collection,
        ..ProgramLimits::default()
    };
    let mut parser = Parser::new(source, limits);
    parser.prepared = true;
    let mut reader = zirql::ArgumentReader::new(source, start).map_err(pipeline::core_error)?;
    let before = budget.remaining_work();
    let predicate = zirql::read_native_predicate_argument(
        &mut reader,
        &mut parser,
        &[],
        budget,
        limits.nesting,
    )
    .map_err(pipeline::core_error)?;
    Ok((
        Rc::new(QueryPredicate {
            filter: composition::FilterStage::new(predicate),
            queries: std::mem::take(&mut parser.queries),
            work: before - budget.remaining_work(),
            nesting: limits.nesting,
            report_model_demand: parser.report_model_demand,
        }),
        reader.position(),
    ))
}
pub(crate) fn filter_query_rows<'a>(
    predicate: &QueryPredicate,
    rows: Vec<pipeline::Row<'a>>,
    query: &mut pipeline::Context<'a>,
    input: &'a Input,
    budget: &mut Budget,
    limits: Limits,
    range: &Range<usize>,
) -> Result<Vec<pipeline::Row<'a>>, QueryError> {
    budget
        .charge(
            predicate.work,
            zirql::Location::Query(predicate.filter.get().range.clone()),
        )
        .map_err(pipeline::core_error)?;
    let value = Located::new(
        zirql::ValueExpressionKind::NativeValue(Box::new(Located::new(
            ExpressionKind::Sequence {
                input: Box::new(value_stage(
                    Stage::Filter(predicate.filter.clone()),
                    range.clone(),
                )),
                steps: vec![Located::new(
                    ExpressionStep::Stage(Stage::RetainPayload),
                    range.clone(),
                )],
            },
            range.clone(),
        ))),
        range.clone(),
    );
    let program = zirql::Program::new(
        vec![Located::new(
            zirql::Statement::Evaluate {
                expression: value_stage(Stage::FunctionValue(value), range.clone()),
                implicit_output: None,
            },
            range.clone(),
        )],
        range.clone(),
    );
    let pipelines = predicate
        .queries
        .iter()
        .map(|query| {
            if query.arguments.is_empty() {
                pipeline::lower_pipeline(&query.query, budget).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let stream = query.restore_stream(rows);
    let declarations = matches!(&stream, SemanticStream::Entities(rows) if rows.iter().all(|row| matches!(row, semantic::SemanticEntity::Declaration(_))));
    let entities = matches!(&stream, SemanticStream::Entities(_));
    let facts = query.report.as_ref().map(|report| Rc::clone(&report.facts));
    let input_value = if entities {
        ReportValue::EntitySequence {
            declarations,
            owner: facts.clone(),
            value: Box::new(ReportValue::Query(stream)),
        }
    } else {
        ReportValue::Query(stream)
    };
    let mut context = Context {
        functions: None,
        query,
        input,
        facts,
        facts_admitted: false,
        parameters: Vec::new(),
        emissions: Vec::new(),
        candidate: None,
        guard_unknown: None,
    };
    let mut written_native = program
        .statements()
        .iter()
        .map(|statement| match &statement.value {
            zirql::Statement::Evaluate { expression, .. } => expression,
            _ => unreachable!("fixed Query filter has one evaluation"),
        })
        .collect();
    admit_written_members(
        &mut written_native,
        &mut Vec::new(),
        &mut context,
        budget,
        predicate.nesting.min(limits.nesting),
    )?;
    let host = NativeHost {
        functions: true,
        signatures: &[],
        declarations: &[],
        parameters: &[],
        queries: &predicate.queries,
        pipelines,
        nesting: predicate.nesting.min(limits.nesting),
        limits,
        editing: false,
    };
    let host = composition::CompositionHost(&host);
    let input_kind = zirql::ProjectionHost::kind(&host, &input_value, budget, range)
        .map_err(pipeline::core_error)?;
    let session = functions::Session::new(
        &[],
        &[],
        &host,
        &program,
        zirql::FunctionLimits { active_depth: 64 },
        input_kind,
        budget,
    )?;
    context.functions = Some(&session);
    let zirql::Statement::Evaluate { expression, .. } = &program.statements()[0].value else {
        unreachable!()
    };
    let ExpressionKind::Sequence { steps, .. } = &expression.value else {
        unreachable!()
    };
    let ExpressionStep::Stage(Stage::FunctionValue(value)) = &steps[0].value else {
        unreachable!()
    };
    let bindings = Bindings::new(Vec::new()).map_err(pipeline::core_error)?;
    let result = session.run(value, &bindings, &input_value, &mut context, budget)?;
    let ReportValue::Query(stream) = functions::owned(result, budget, range)? else {
        return Err(QueryError::query(
            range.clone(),
            "filter requires a native selection",
        ));
    };
    Ok(context.query.take_rows(stream))
}

fn admit_written_members<'v, 'a>(
    native: &mut Vec<&'v Expression>,
    values: &mut Vec<&'v zirql::ValueExpression<Expression>>,
    context: &mut Context<'_, 'a>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<(), QueryError> {
    while !native.is_empty() || !values.is_empty() {
        while let Some(value) = values.pop() {
            native.extend(functions::eager_native_values(value, budget, nesting)?);
        }
        let Some(expression) = native.pop() else {
            break;
        };
        budget
            .charge(1, zirql::Location::Query(expression.range.clone()))
            .map_err(pipeline::core_error)?;
        match &expression.value {
            ExpressionKind::Input | ExpressionKind::Binding(_) => {}
            ExpressionKind::Group(value) | ExpressionKind::Field { value, .. } => {
                native.push(value)
            }
            ExpressionKind::Record(fields) => native.extend(fields.iter().map(|(_, value)| value)),
            ExpressionKind::Set { left, right, .. } => {
                native.extend([left.as_ref(), right.as_ref()])
            }
            ExpressionKind::Sequence { input, steps } => {
                native.push(input);
                for step in steps {
                    let ExpressionStep::Stage(stage) = &step.value else {
                        continue;
                    };
                    match stage {
                        Stage::Member(member) => {
                            let facts = context.report_facts(budget, &member.range)?;
                            comprehension::resolve_member(
                                facts.as_deref(),
                                Some(&member.enum_name.value),
                                &member.member_name.value,
                                budget,
                                &member.range,
                            )?;
                        }
                        Stage::FunctionValue(value)
                        | Stage::FunctionSource(value)
                        | Stage::PatternBinding(_, value) => values.push(value),
                        Stage::Primitive(_, arguments) => values.extend(arguments),
                        Stage::Query(_, arguments) => {
                            values.extend(arguments.iter().map(|argument| &argument.expression))
                        }
                        Stage::WhereExists(body) | Stage::FixedPoint(body) => {
                            values.push(&body.expression)
                        }
                        Stage::Mapping(mapping) => values.extend([&mapping.key, &mapping.value]),
                        Stage::Ordering(ordering) => values.extend(ordering.selector.iter()),
                        Stage::Projection(projection) => {
                            values.extend(projection.get().fields.iter().map(|(_, value)| value))
                        }
                        Stage::Filter(predicate) => {
                            for guard in composition::guard_leaves(predicate.get()) {
                                values.extend(guard.values());
                            }
                        }
                        Stage::Paths(paths) => {
                            for guard in composition::guard_leaves(&paths.get().target) {
                                values.extend(guard.values());
                            }
                        }
                        Stage::Match(matching) => {
                            for clause in &matching.get().clauses {
                                match &clause.value {
                                    zirql::MatchClause::From { source, .. }
                                    | zirql::MatchClause::Bind { value: source, .. } => {
                                        native.push(source)
                                    }
                                    zirql::MatchClause::Where { predicate } => {
                                        for guard in composition::guard_leaves(predicate) {
                                            values.extend(guard.values());
                                        }
                                    }
                                }
                            }
                        }
                        Stage::Copy(_)
                        | Stage::Parameter(_)
                        | Stage::Data(..)
                        | Stage::MapSorting
                        | Stage::RetainPayload => {}
                    }
                }
            }
        }
    }
    Ok(())
}
