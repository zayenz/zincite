//! Source inspection and explicit data edits over the retained MiniZinc CST.
//!
//! [`Input`] owns original bytes and the CST. [`Query`] parses a fixed pipeline
//! for navigating nodes, inspecting literals or editing data collections/assignments.
//! Results stay native until source or JSON rendering. Source rendering retains
//! original bytes, including opaque
//! comment bytes. Literal comparisons are bounded; computed expressions and
//! includes are never evaluated.

use std::collections::BTreeMap;
use std::ops::Range;

use zincite_lint::{EditPart, SourceSnapshot, TextEdit, prepare_text_edits};
use zincite_syntax::{
    ByteParsedFile, Diagnostic, FileMode, NodeKind, ParsedFile, SyntaxElement, SyntaxNode,
    TokenKind, byte_line_column, parse_bytes_with_mode,
};

pub mod cli;
mod inspection;
mod language;
mod structured;
pub use inspection::{JsonResult, NodeSelection, SelectedNode};
pub use structured::{
    ArrayIndexing, LiteralEntry, LiteralField, LiteralKind, LiteralLabel, LiteralRow,
    LiteralSelection, LiteralValue,
};

use language::{Predicate, PredicateKind, Stage, StageKind};

/// Bounds checked before collecting items, visiting stages or expanding edits.
/// `nesting` lowers the ceiling of 64 query parentheses/`not` operators and
/// recursive literal levels.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub nesting: usize,
    pub work: usize,
    pub collection: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            nesting: 64,
            work: 1_000_000,
            collection: 100_000,
        }
    }
}

/// Coordinate space of a query failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorLocation {
    Query,
    Input,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryError {
    pub location: ErrorLocation,
    pub range: Range<usize>,
    pub message: String,
}

impl QueryError {
    fn query(range: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            location: ErrorLocation::Query,
            range,
            message: message.into(),
        }
    }

    fn input(range: Range<usize>, message: impl Into<String>) -> Self {
        Self {
            location: ErrorLocation::Input,
            range,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct InputError {
    /// Syntax/byte diagnostics in original input coordinates.
    pub diagnostics: Vec<Diagnostic>,
}

/// Read-only original input and its byte-preserving parser view.
#[derive(Debug)]
pub struct Input {
    bytes: Vec<u8>,
    parsed: ByteParsedFile,
    syntax_offset: usize,
    mode: FileMode,
    file: String,
}

impl Input {
    pub fn parse(bytes: Vec<u8>, mode: FileMode) -> Result<Self, InputError> {
        Self::parse_named(bytes, mode, "<input>")
    }

    /// Parse original bytes with the file identity used by inspection output.
    pub fn parse_named(
        bytes: Vec<u8>,
        mode: FileMode,
        file: impl Into<String>,
    ) -> Result<Self, InputError> {
        let syntax_offset = if bytes.starts_with(b"\xef\xbb\xbf") {
            3
        } else {
            0
        };
        let parsed =
            parse_bytes_with_mode(bytes[syntax_offset..].to_vec(), mode).map_err(|diagnostic| {
                InputError {
                    diagnostics: vec![shift_diagnostic(diagnostic, syntax_offset)],
                }
            })?;
        if !parsed.analysis_file().diagnostics().is_empty() {
            return Err(InputError {
                diagnostics: parsed
                    .analysis_file()
                    .diagnostics()
                    .iter()
                    .cloned()
                    .map(|diagnostic| shift_diagnostic(diagnostic, syntax_offset))
                    .collect(),
            });
        }
        Ok(Self {
            bytes,
            parsed,
            syntax_offset,
            mode,
            file: file.into(),
        })
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub fn source_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The analysis CST uses offsets after any leading BOM. Use `token_range`
    /// and selected item ranges for coordinates in the original input.
    pub fn syntax(&self) -> &ParsedFile {
        self.parsed.analysis_file()
    }

    pub fn syntax_offset(&self) -> usize {
        self.syntax_offset
    }

    pub fn token_range(&self, index: usize) -> Range<usize> {
        self.original_range(self.syntax().tokens()[index].range.clone())
    }

    pub fn token_bytes(&self, index: usize) -> &[u8] {
        &self.bytes[self.token_range(index)]
    }

    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        byte_line_column(&self.bytes, offset)
    }

    fn original_range(&self, range: Range<usize>) -> Range<usize> {
        range.start + self.syntax_offset..range.end + self.syntax_offset
    }
}

fn shift_diagnostic(mut diagnostic: Diagnostic, offset: usize) -> Diagnostic {
    diagnostic.range = diagnostic.range.start + offset..diagnostic.range.end + offset;
    diagnostic
}

/// Accepted `kind` predicate values, independent of expression node kinds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Assignment,
    Declaration,
    Enum,
    TypeAlias,
    Function,
    Predicate,
    Test,
    Annotation,
    Constraint,
    Include,
    Output,
    Solve,
}

impl ItemKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assignment => "assignment",
            Self::Declaration => "declaration",
            Self::Enum => "enum",
            Self::TypeAlias => "type_alias",
            Self::Function => "function",
            Self::Predicate => "predicate",
            Self::Test => "test",
            Self::Annotation => "annotation",
            Self::Constraint => "constraint",
            Self::Include => "include",
            Self::Output => "output",
            Self::Solve => "solve",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "assignment" => Self::Assignment,
            "declaration" => Self::Declaration,
            "enum" => Self::Enum,
            "type_alias" => Self::TypeAlias,
            "function" => Self::Function,
            "predicate" => Self::Predicate,
            "test" => Self::Test,
            "annotation" => Self::Annotation,
            "constraint" => Self::Constraint,
            "include" => Self::Include,
            "output" => Self::Output,
            "solve" => Self::Solve,
            _ => return None,
        })
    }

    fn from_node(kind: NodeKind) -> Self {
        match kind {
            NodeKind::Assignment => Self::Assignment,
            NodeKind::Declaration => Self::Declaration,
            NodeKind::EnumDeclaration => Self::Enum,
            NodeKind::TypeAlias => Self::TypeAlias,
            NodeKind::FunctionDeclaration => Self::Function,
            NodeKind::PredicateDeclaration => Self::Predicate,
            NodeKind::TestDeclaration => Self::Test,
            NodeKind::AnnotationDeclaration => Self::Annotation,
            NodeKind::Constraint => Self::Constraint,
            NodeKind::Include => Self::Include,
            NodeKind::Output => Self::Output,
            NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize => Self::Solve,
            _ => unreachable!("validated input contains only supported root items"),
        }
    }

    fn is_named(self) -> bool {
        matches!(
            self,
            Self::Assignment
                | Self::Declaration
                | Self::Enum
                | Self::TypeAlias
                | Self::Function
                | Self::Predicate
                | Self::Test
                | Self::Annotation
        )
    }
}

/// One retained item. Ranges use original input coordinates; the borrowed CST
/// node itself uses the parser's coordinates described by `Input::syntax`.
#[derive(Clone, Debug)]
pub struct SelectedItem<'a> {
    pub node: &'a SyntaxNode,
    pub kind: ItemKind,
    /// Identifier identity without quote delimiters; expressions are not names.
    pub name: Option<&'a str>,
    pub range: Range<usize>,
    /// Item plus attached comments and their original surrounding whitespace.
    pub source_range: Range<usize>,
}

#[derive(Debug)]
pub struct Selection<'a> {
    input: &'a Input,
    items: Vec<SelectedItem<'a>>,
    document: bool,
}

impl<'a> Selection<'a> {
    pub fn input(&self) -> &'a Input {
        self.input
    }

    pub fn items(&self) -> &[SelectedItem<'a>] {
        &self.items
    }

    /// Only the untouched initial selection or `items` emits the whole document.
    pub fn is_document(&self) -> bool {
        self.document
    }
}

#[derive(Debug)]
pub enum QueryResult<'a> {
    Selection(Selection<'a>),
    Count(usize),
    Nodes(NodeSelection<'a>),
    Strings(Vec<String>),
    Literals(LiteralSelection<'a>),
    Tally(BTreeMap<String, usize>),
    Json(JsonResult<'a>),
    /// Complete validated candidate from one intentional data transformation.
    Document(Vec<u8>),
}

impl QueryResult<'_> {
    /// Render the complete result. Counts use decimal text and one LF; source
    /// selections and documents reproduce bytes without adding separators or LF.
    pub fn render(&self) -> Vec<u8> {
        match self {
            Self::Nodes(selection) => selection
                .nodes()
                .iter()
                .flat_map(|node| &selection.input().bytes[node.range.clone()])
                .copied()
                .collect(),
            Self::Literals(selection) => selection
                .values()
                .iter()
                .flat_map(|value| value.spelling)
                .copied()
                .collect(),
            Self::Strings(values) => values
                .iter()
                .flat_map(|value| value.bytes().chain(std::iter::once(b'\n')))
                .collect(),
            Self::Tally(values) => {
                let mut bytes = serde_json::to_vec(values).expect("string-key counts serialize");
                bytes.push(b'\n');
                bytes
            }
            Self::Json(result) => result.render(),
            Self::Count(count) => format!("{count}\n").into_bytes(),
            Self::Document(bytes) => bytes.clone(),
            Self::Selection(selection) if selection.document => selection.input.bytes.clone(),
            Self::Selection(selection) => {
                let mut bytes = Vec::new();
                for item in &selection.items {
                    bytes.extend_from_slice(&selection.input.bytes[item.source_range.clone()]);
                }
                bytes
            }
        }
    }
}

/// One fixed-stage query. Empty text selects and emits the original document.
#[derive(Debug)]
pub struct Query {
    stages: Vec<Stage>,
    range: Range<usize>,
}

impl Query {
    pub fn parse(expression: &str, limits: Limits) -> Result<Self, QueryError> {
        language::parse(expression, limits)
    }

    pub fn is_editing(&self) -> bool {
        self.stages.iter().any(|stage| {
            matches!(
                stage.kind,
                StageKind::SetValue(_) | StageKind::Remove | StageKind::FilterElements(_)
            )
        })
    }

    pub fn evaluate<'a>(
        &self,
        input: &'a Input,
        limits: Limits,
    ) -> Result<QueryResult<'a>, QueryError> {
        let mut work = Work {
            remaining: limits.work,
        };
        let mut result = QueryResult::Selection(Selection {
            input,
            items: collect_items(input, limits.collection, &mut work, &self.range, true)?,
            document: true,
        });
        for stage in &self.stages {
            work.visit(&stage.range)?;
            result = match &stage.kind {
                StageKind::Items => {
                    inspection::require_nodes(&result, &stage.range)?;
                    QueryResult::Selection(Selection {
                        input,
                        items: collect_items(
                            input,
                            limits.collection,
                            &mut work,
                            &stage.range,
                            true,
                        )?,
                        document: true,
                    })
                }
                StageKind::Filter(predicate) => match result {
                    QueryResult::Selection(mut selection) => {
                        let mut retained = Vec::new();
                        for item in selection.items {
                            work.visit(&stage.range)?;
                            if predicate.matches(&item, &mut work)? {
                                retained.push(item);
                            }
                        }
                        selection.items = retained;
                        selection.document = false;
                        QueryResult::Selection(selection)
                    }
                    QueryResult::Nodes(selection) => {
                        inspection::filter(selection, predicate, &mut work, &stage.range)?
                    }
                    _ => {
                        return Err(inspection::type_error(
                            &stage.range,
                            "filter requires selected nodes",
                        ));
                    }
                },
                StageKind::Head(count) => match result {
                    QueryResult::Selection(mut selection) => {
                        selection.items.truncate(*count);
                        selection.document = false;
                        QueryResult::Selection(selection)
                    }
                    QueryResult::Nodes(mut selection) => {
                        selection.truncate(*count);
                        QueryResult::Nodes(selection)
                    }
                    QueryResult::Literals(mut selection) => {
                        selection.truncate(*count);
                        QueryResult::Literals(selection)
                    }
                    QueryResult::Strings(mut strings) => {
                        strings.truncate(*count);
                        QueryResult::Strings(strings)
                    }
                    _ => {
                        return Err(inspection::type_error(
                            &stage.range,
                            "head requires a node, literal or string stream",
                        ));
                    }
                },
                StageKind::Count => QueryResult::Count(match &result {
                    QueryResult::Selection(selection) => selection.items.len(),
                    QueryResult::Nodes(selection) => selection.nodes().len(),
                    QueryResult::Strings(strings) => strings.len(),
                    QueryResult::Literals(selection) => selection.values().len(),
                    _ => {
                        return Err(inspection::type_error(
                            &stage.range,
                            "count requires a node, literal or string stream",
                        ));
                    }
                }),
                StageKind::Values => structured::values(result, stage, &mut work, limits)?,
                StageKind::Fields | StageKind::Elements | StageKind::Keys => {
                    structured::project(result, stage, &mut work, limits)?
                }
                StageKind::FilterElements(comparison) => {
                    QueryResult::Document(structured::transform_elements(
                        input,
                        result,
                        comparison,
                        &stage.range,
                        &mut work,
                        limits,
                    )?)
                }
                StageKind::SetValue(_) | StageKind::Remove => {
                    let QueryResult::Selection(selection) = result else {
                        return Err(inspection::type_error(
                            &stage.range,
                            "transformations require an item selection",
                        ));
                    };
                    QueryResult::Document(transform_assignments(
                        input,
                        &selection.items,
                        stage,
                        &mut work,
                        limits.collection,
                    )?)
                }
                StageKind::Emit => {
                    if !matches!(
                        result,
                        QueryResult::Selection(_)
                            | QueryResult::Nodes(_)
                            | QueryResult::Strings(_)
                            | QueryResult::Tally(_)
                            | QueryResult::Literals(_)
                    ) {
                        return Err(inspection::type_error(
                            &stage.range,
                            "emit requires a node, literal or string stream or tally",
                        ));
                    }
                    result
                }
                StageKind::EmitDocument => result,
                StageKind::Expressions
                | StageKind::Children
                | StageKind::Subtree
                | StageKind::Range(_) => {
                    inspection::navigate(result, stage, &mut work, limits.collection)?
                }
                StageKind::Names
                | StageKind::CallNames
                | StageKind::AnnotationNames
                | StageKind::Text => {
                    inspection::project(result, stage, &mut work, limits.collection)?
                }
                StageKind::Unique => inspection::unique(result, &mut work, &stage.range)?,
                StageKind::Tally => inspection::tally(result, &mut work, &stage.range)?,
                StageKind::Json => inspection::json(result, &mut work, &stage.range)?,
            };
        }
        if let QueryResult::Selection(selection) = &result
            && !selection.document
        {
            validate_fragment_directives(input, &selection.items)?;
        }
        if let QueryResult::Literals(selection) = &result {
            for value in selection.values() {
                work.spend(value.range.len(), &self.range)?;
            }
        }
        if let QueryResult::Nodes(selection) = &result {
            for node in selection.nodes() {
                work.spend(node.range.len(), &self.range)?;
            }
        }
        Ok(result)
    }
}

// Build one original-coordinate batch, validate its complete data candidate,
// then return bytes. No selection is evaluated against the changed document.
fn transform_assignments(
    input: &Input,
    items: &[SelectedItem<'_>],
    stage: &Stage,
    work: &mut Work,
    collection_limit: usize,
) -> Result<Vec<u8>, QueryError> {
    if input.mode != FileMode::Data {
        return Err(QueryError::query(
            stage.range.clone(),
            "assignment transformations require data-file mode (.dzn)",
        ));
    }
    let replacement = match &stage.kind {
        StageKind::SetValue(value) => {
            work.spend(value.len(), &stage.range)?;
            validate_replacement(value, &stage.range)?;
            Some(value)
        }
        StageKind::Remove => None,
        _ => unreachable!("only transformation stages construct a candidate"),
    };
    // Directive attachments can cross a blank-line boundary. Keep the ordinary
    // comment attachments too, so separated section comments survive removal.
    let ordinary_items = if replacement.is_none() {
        collect_items(input, collection_limit, work, &stage.range, false)?
    } else {
        Vec::new()
    };
    let mut edits = Vec::with_capacity(items.len());
    for item in items {
        work.visit(&stage.range)?;
        if item.kind != ItemKind::Assignment {
            return Err(QueryError::input(
                item.range.clone(),
                "selected item is not an assignment",
            ));
        }
        let edit = if let Some(value) = replacement {
            work.spend(value.len(), &stage.range)?;
            let rhs = assignment_rhs(item.node).ok_or_else(|| {
                QueryError::input(
                    item.range.clone(),
                    "assignment has no single right-hand side",
                )
            })?;
            let range = input.original_range(rhs.range());
            let first = input
                .syntax()
                .tokens()
                .partition_point(|token| token.range.end + input.syntax_offset <= range.start);
            for (index, token) in input.syntax().tokens().iter().enumerate().skip(first) {
                if token.range.start + input.syntax_offset >= range.end {
                    break;
                }
                work.visit(&stage.range)?;
                if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
                    return Err(QueryError::input(
                        input.token_range(index),
                        "cannot replace a right-hand side with internal comments without choosing their new placement",
                    ));
                }
            }
            TextEdit {
                range,
                replacement: vec![EditPart::Text(value.clone())],
            }
        } else {
            let ordinary = ordinary_items
                .binary_search_by_key(&item.range.start, |item| item.range.start)
                .unwrap();
            removal_edit(
                input,
                item,
                &ordinary_items[ordinary].source_range,
                work,
                &stage.range,
            )?
        };
        edits.push(edit);
    }
    let removed = if replacement.is_none() { items } else { &[] };
    finish_candidate(input, &edits, removed, work, &stage.range).map(|candidate| candidate.bytes)
}

fn finish_candidate(
    input: &Input,
    edits: &[TextEdit],
    removed: &[SelectedItem<'_>],
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Input, QueryError> {
    validate_document_directives(input, removed, work, range)?;
    let snapshot = SourceSnapshot::new("<query>.dzn", input.source_bytes());
    let candidate = prepare_text_edits(&snapshot, input.source_bytes(), edits)
        .map_err(|error| QueryError::query(range.clone(), error.to_string()))?;
    let candidate_input = Input::parse(candidate, FileMode::Data).map_err(|error| {
        QueryError::query(
            range.clone(),
            format!(
                "invalid data edit candidate: {}",
                error.diagnostics[0].message
            ),
        )
    })?;
    validate_document_directives(&candidate_input, &[], work, range).map_err(|error| {
        QueryError::query(
            range.clone(),
            format!("invalid data edit candidate: {}", error.message),
        )
    })?;
    Ok(candidate_input)
}

fn removal_edit(
    input: &Input,
    item: &SelectedItem<'_>,
    ordinary_range: &Range<usize>,
    work: &mut Work,
    query_range: &Range<usize>,
) -> Result<TextEdit, QueryError> {
    let range = item.source_range.clone();
    let mut deleted = Vec::new();
    let tokens = input.syntax().tokens();
    let first =
        tokens.partition_point(|token| token.range.end + input.syntax_offset <= range.start);
    for (index, token) in tokens.iter().enumerate().skip(first) {
        if token.range.start + input.syntax_offset >= range.end {
            break;
        }
        work.visit(query_range)?;
        if !matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
            || directive(input.token_bytes(index)).is_none()
        {
            continue;
        }
        let marker = input.token_range(index);
        let start = line_indent_start(&input.bytes, marker.start, range.start);
        let end = tokens
            .get(index + 1)
            .filter(|token| token.kind == TokenKind::Whitespace)
            .map_or(marker.end, |token| token.range.end + input.syntax_offset)
            .min(range.end);
        deleted.push(start..end);
    }
    deleted.push(ordinary_range.start.max(range.start)..ordinary_range.end.min(range.end));
    deleted.sort_by_key(|range| (range.start, range.end));
    let mut replacement = Vec::new();
    let mut cursor = range.start;
    for deleted in deleted {
        if cursor < deleted.start {
            replacement.push(EditPart::Original(cursor..deleted.start));
        }
        cursor = cursor.max(deleted.end);
    }
    if cursor < range.end {
        replacement.push(EditPart::Original(cursor..range.end));
    }
    Ok(TextEdit { range, replacement })
}

fn assignment_rhs(node: &SyntaxNode) -> Option<&SyntaxNode> {
    let mut children = node.child_nodes();
    let rhs = children.next()?;
    children.next().is_none().then_some(rhs)
}

fn validate_replacement(value: &str, range: &Range<usize>) -> Result<(), QueryError> {
    let source = format!("zincite_value = {value};");
    let input = Input::parse(source.into_bytes(), FileMode::Data).map_err(|error| {
        QueryError::query(
            range.clone(),
            format!(
                "invalid replacement expression: {}",
                error.diagnostics[0].message
            ),
        )
    })?;
    // A final item may omit its semicolon. Require our separator to survive
    // tokenization so a trailing line comment cannot consume original source.
    if !input
        .syntax()
        .tokens()
        .last()
        .is_some_and(|token| token.kind == TokenKind::Semicolon)
    {
        return Err(QueryError::query(
            range.clone(),
            "invalid replacement expression: trailing comment would hide the assignment separator",
        ));
    }
    let mut items = input.syntax().tree().child_nodes();
    if !items
        .next()
        .is_some_and(|item| assignment_rhs(item).is_some())
        || items.next().is_some()
    {
        return Err(QueryError::query(
            range.clone(),
            "replacement must contain exactly one expression",
        ));
    }
    Ok(())
}

fn validate_document_directives(
    input: &Input,
    removed: &[SelectedItem<'_>],
    work: &mut Work,
    query_range: &Range<usize>,
) -> Result<(), QueryError> {
    let mut open = None;
    let mut removal = 0;
    let mut root_items = input.syntax().tree().child_nodes().peekable();
    for (index, token) in input.syntax().tokens().iter().enumerate() {
        work.visit(query_range)?;
        if !matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
            continue;
        }
        let range = input.token_range(index);
        while removed
            .get(removal)
            .is_some_and(|item| item.source_range.end <= range.start)
        {
            removal += 1;
        }
        let deleted = removed.get(removal).is_some_and(|item| {
            item.source_range.start <= range.start && range.end <= item.source_range.end
        });
        while root_items
            .peek()
            .is_some_and(|node| node.range().end + input.syntax_offset <= range.start)
        {
            root_items.next();
        }
        let marker = directive(input.token_bytes(index));
        let between_items = root_items
            .peek()
            .is_none_or(|node| node.range().start + input.syntax_offset >= range.end);
        let standalone = standalone(&input.bytes, range.start, input.syntax_offset);
        if matches!(marker, Some(Directive::Next))
            && standalone
            && between_items
            && let Some(target) = root_items.peek()
        {
            let target = target.range().start + input.syntax_offset;
            let target_deleted = removed
                .binary_search_by_key(&target, |item| item.range.start)
                .is_ok();
            if deleted != target_deleted {
                return Err(QueryError::input(
                    range,
                    "transformation would separate a next-item directive from its target",
                ));
            }
        }
        if deleted {
            continue;
        }
        if matches!(marker, Some(Directive::Off | Directive::On)) {
            if !standalone || !between_items {
                return Err(QueryError::input(
                    range,
                    "paired formatter directives must be standalone comments between complete items",
                ));
            }
            match marker {
                Some(Directive::Off) if open.is_none() => open = Some(range),
                Some(Directive::On) if open.is_some() => open = None,
                _ => {
                    return Err(QueryError::input(
                        range,
                        "transformation would unbalance paired formatter directives",
                    ));
                }
            }
        }
    }
    if let Some(range) = open {
        return Err(QueryError::input(
            range,
            "transformation would unbalance paired formatter directives",
        ));
    }
    Ok(())
}

struct Work {
    remaining: usize,
}

impl Work {
    fn visit(&mut self, range: &Range<usize>) -> Result<(), QueryError> {
        self.spend(1, range)
    }

    fn spend(&mut self, amount: usize, range: &Range<usize>) -> Result<(), QueryError> {
        self.remaining = self
            .remaining
            .checked_sub(amount)
            .ok_or_else(|| QueryError::query(range.clone(), "query work limit exceeded"))?;
        Ok(())
    }
}

impl Predicate {
    fn matches(&self, item: &SelectedItem<'_>, work: &mut Work) -> Result<bool, QueryError> {
        self.matches_values(item.kind.as_str(), item.name, work)
    }

    fn matches_values(
        &self,
        kind: &str,
        name: Option<&str>,
        work: &mut Work,
    ) -> Result<bool, QueryError> {
        work.visit(&self.range)?;
        match &self.kind {
            PredicateKind::Kind(expected) => Ok(kind == expected),
            PredicateKind::Name(expected) => Ok(name == Some(identifier_identity(expected))),
            PredicateKind::Not(predicate) => Ok(!predicate.matches_values(kind, name, work)?),
            PredicateKind::Group(predicate) => predicate.matches_values(kind, name, work),
            PredicateKind::And(predicates) => {
                for predicate in predicates {
                    if !predicate.matches_values(kind, name, work)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            PredicateKind::Or(predicates) => {
                for predicate in predicates {
                    if predicate.matches_values(kind, name, work)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }
}

fn identifier_identity(written: &str) -> &str {
    written
        .strip_prefix('\'')
        .and_then(|name| name.strip_suffix('\''))
        .unwrap_or(written)
}

fn collect_items<'a>(
    input: &'a Input,
    collection_limit: usize,
    work: &mut Work,
    range: &Range<usize>,
    include_directives: bool,
) -> Result<Vec<SelectedItem<'a>>, QueryError> {
    let mut items = Vec::new();
    for node in input.syntax().tree().child_nodes() {
        work.visit(range)?;
        if items.len() >= collection_limit {
            return Err(QueryError::query(
                range.clone(),
                "query collection limit exceeded",
            ));
        }
        let kind = ItemKind::from_node(node.kind());
        let name = kind.is_named().then(|| {
            node.children().iter().find_map(|child| {
                let SyntaxElement::Token(index) = child else {
                    return None;
                };
                let token = &input.syntax().tokens()[*index];
                matches!(
                    token.kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                )
                .then(|| identifier_identity(&input.syntax().source()[token.range.clone()]))
            })
        });
        let range = input.original_range(node.range());
        items.push(SelectedItem {
            node,
            kind,
            name: name.flatten(),
            source_range: range.clone(),
            range,
        });
    }
    attach_comments(input, &mut items, include_directives);
    Ok(items)
}

fn attach_comments(input: &Input, items: &mut [SelectedItem<'_>], include_directives: bool) {
    let tokens = input.syntax().tokens();
    for next in 0..=items.len() {
        let gap_start = if next == 0 {
            input.syntax_offset
        } else {
            items[next - 1].range.end
        };
        let gap_end = items
            .get(next)
            .map_or(input.bytes.len(), |item| item.range.start);
        let first =
            tokens.partition_point(|token| token.range.end + input.syntax_offset <= gap_start);
        let last =
            tokens.partition_point(|token| token.range.start + input.syntax_offset < gap_end);
        let gap_tokens = &tokens[first..last];
        let mut boundary = gap_start;
        if next > 0 {
            // Same-line comments and the first line ending stay with the preceding item.
            let mut end = gap_start;
            for token in gap_tokens {
                let range = input.original_range(token.range.clone());
                let bytes = &input.bytes[range.clone()];
                if token.kind == TokenKind::Whitespace {
                    if let Some(length) = first_line_end(bytes) {
                        end = range.start + length;
                        break;
                    }
                    end = range.end;
                } else {
                    end = range.end;
                }
            }
            items[next - 1].source_range.end = end;
            boundary = end;
        }
        if next < items.len() {
            let mut start = gap_end;
            for token in gap_tokens.iter().rev() {
                let range = input.original_range(token.range.clone());
                if range.end <= boundary {
                    break;
                }
                let bytes = &input.bytes[range.clone()];
                if token.kind == TokenKind::Whitespace {
                    // Keep indentation, but a blank line separates standalone blocks.
                    start = line_indent_start(&input.bytes, start, boundary);
                    if line_breaks(bytes) >= 2 {
                        break;
                    }
                } else if standalone(&input.bytes, range.start, boundary) {
                    start = line_indent_start(&input.bytes, range.start, boundary);
                } else {
                    break;
                }
            }
            items[next].source_range.start = start.max(boundary);
        }
        if !include_directives {
            continue;
        }
        // Next-item markers stay with their target even across blank lines.
        // The closing formatter marker belongs to the last item in its region.
        for token in gap_tokens {
            let range = input.original_range(token.range.clone());
            if range.start < boundary || !standalone(&input.bytes, range.start, boundary) {
                continue;
            }
            match directive(&input.bytes[range.clone()]) {
                Some(Directive::Next | Directive::Off) if next < items.len() => {
                    items[next].source_range.start = items[next]
                        .source_range
                        .start
                        .min(line_indent_start(&input.bytes, range.start, boundary));
                }
                Some(Directive::On) if next > 0 => {
                    let end = range.end
                        + first_line_end(&input.bytes[range.end..gap_end])
                            .unwrap_or(gap_end - range.end);
                    items[next - 1].source_range.end = items[next - 1].source_range.end.max(end);
                    if next < items.len() {
                        items[next].source_range.start = items[next].source_range.start.max(end);
                    }
                }
                _ => {}
            }
        }
    }
}

fn first_line_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .iter()
        .position(|byte| matches!(byte, b'\r' | b'\n'))
        .map(|index| {
            index + 1 + usize::from(bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n'))
        })
}

fn line_breaks(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .enumerate()
        .filter(|(index, byte)| {
            **byte == b'\r' || (**byte == b'\n' && (*index == 0 || bytes[*index - 1] != b'\r'))
        })
        .count()
}

fn line_indent_start(bytes: &[u8], offset: usize, boundary: usize) -> usize {
    let mut start = offset;
    while start > boundary && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    start
}

fn standalone(bytes: &[u8], offset: usize, boundary: usize) -> bool {
    let start = line_indent_start(bytes, offset, boundary);
    start == boundary || matches!(bytes[start - 1], b'\r' | b'\n')
}

#[derive(Clone, Copy)]
enum Directive {
    Next,
    Off,
    On,
}

fn directive(bytes: &[u8]) -> Option<Directive> {
    let text = std::str::from_utf8(bytes).ok()?.trim();
    if let Some(command) = text.strip_prefix("% zincite-fmt:").map(str::trim) {
        match command {
            "skip" => Some(Directive::Next),
            "off" => Some(Directive::Off),
            "on" => Some(Directive::On),
            _ => None,
        }
    } else if text
        .strip_prefix("% zincite-lint:")
        .map(str::trim)
        .is_some_and(|command| command == "ignore" || command.starts_with("ignore "))
    {
        Some(Directive::Next)
    } else {
        None
    }
}

fn validate_fragment_directives(
    input: &Input,
    items: &[SelectedItem<'_>],
) -> Result<(), QueryError> {
    for pair in items.windows(2) {
        if pair[0].source_range.end > pair[1].source_range.start {
            return Err(QueryError::input(
                pair[1].source_range.clone(),
                "fragment selection would overlap directive attachments",
            ));
        }
    }
    let mut open = None;
    let mut selected = 0;
    let mut root_items = input.syntax().tree().child_nodes().peekable();
    for (index, token) in input.syntax().tokens().iter().enumerate() {
        if !matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
            continue;
        }
        let range = input.token_range(index);
        while root_items
            .peek()
            .is_some_and(|node| node.range().end + input.syntax_offset <= range.start)
        {
            root_items.next();
        }
        while items
            .get(selected)
            .is_some_and(|item| item.source_range.end <= range.start)
        {
            selected += 1;
        }
        let retained = items.get(selected).is_some_and(|item| {
            item.source_range.start <= range.start && range.end <= item.source_range.end
        });
        let marker = directive(input.token_bytes(index));
        if matches!(marker, Some(Directive::Next))
            && standalone(&input.bytes, range.start, input.syntax_offset)
            && root_items
                .peek()
                .is_some_and(|node| node.range().start + input.syntax_offset >= range.end)
        {
            let target = root_items.peek().unwrap().range().start + input.syntax_offset;
            let target_retained = items
                .binary_search_by_key(&target, |item| item.range.start)
                .is_ok();
            if retained != target_retained {
                return Err(QueryError::input(
                    range,
                    "fragment selection would separate a next-item directive from its target",
                ));
            }
        }
        if !retained {
            continue;
        }
        match marker {
            Some(Directive::Off) if open.is_none() => open = Some(range),
            Some(Directive::On) if open.is_some() => open = None,
            Some(Directive::Off | Directive::On) => {
                return Err(QueryError::input(
                    range,
                    "fragment selection would unbalance paired formatter directives",
                ));
            }
            _ => {}
        }
    }
    if let Some(range) = open {
        return Err(QueryError::input(
            range,
            "fragment selection would unbalance paired formatter directives",
        ));
    }
    Ok(())
}
