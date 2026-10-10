//! Syntax-only queries over top-level MiniZinc items in written order.
//!
//! [`Input`] owns original bytes and the CST. [`Query`] parses a fixed pipeline
//! of `items`, `filter`, `head`, `count` and `emit`; evaluation returns native
//! selections or a count. Rendering uses original bytes, including opaque
//! comment bytes. Includes, names and data values are never evaluated.

use std::ops::Range;

use zincite_syntax::{
    ByteParsedFile, Diagnostic, FileMode, NodeKind, ParsedFile, SyntaxElement, SyntaxNode,
    TokenKind, byte_line_column, parse_bytes_with_mode,
};

pub mod cli;
mod language;

use language::{Predicate, PredicateKind, Stage, StageKind};

/// Bounds checked before adding items or visiting query stages and predicates.
/// `nesting` can lower the hard ceiling of 64 nested parentheses/`not` operators.
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
}

impl Input {
    pub fn parse(bytes: Vec<u8>, mode: FileMode) -> Result<Self, InputError> {
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
        })
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
}

impl QueryResult<'_> {
    /// Render the complete result. Counts use decimal text and one LF; source
    /// selections reproduce bytes without inserting separators or a final LF.
    pub fn render(&self) -> Vec<u8> {
        match self {
            Self::Count(count) => format!("{count}\n").into_bytes(),
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

    pub fn evaluate<'a>(
        &self,
        input: &'a Input,
        limits: Limits,
    ) -> Result<QueryResult<'a>, QueryError> {
        let mut work = Work {
            remaining: limits.work,
        };
        let mut items = collect_items(input, limits.collection, &mut work, &self.range)?;
        let mut document = true;
        for stage in &self.stages {
            work.visit(&stage.range)?;
            match &stage.kind {
                StageKind::Items => {
                    items = collect_items(input, limits.collection, &mut work, &stage.range)?;
                    document = true;
                }
                StageKind::Filter(predicate) => {
                    let mut retained = Vec::new();
                    for item in items {
                        work.visit(&stage.range)?;
                        if predicate.matches(&item, &mut work)? {
                            retained.push(item);
                        }
                    }
                    items = retained;
                    document = false;
                }
                StageKind::Head(count) => {
                    items.truncate(*count);
                    document = false;
                }
                StageKind::Count => return Ok(QueryResult::Count(items.len())),
                StageKind::Emit => {}
            }
        }
        if !document {
            validate_fragment_directives(input, &items)?;
        }
        Ok(QueryResult::Selection(Selection {
            input,
            items,
            document,
        }))
    }
}

struct Work {
    remaining: usize,
}

impl Work {
    fn visit(&mut self, range: &Range<usize>) -> Result<(), QueryError> {
        if self.remaining == 0 {
            return Err(QueryError::query(
                range.clone(),
                "query work limit exceeded",
            ));
        }
        self.remaining -= 1;
        Ok(())
    }
}

impl Predicate {
    fn matches(&self, item: &SelectedItem<'_>, work: &mut Work) -> Result<bool, QueryError> {
        work.visit(&self.range)?;
        match &self.kind {
            PredicateKind::Kind(kind) => Ok(item.kind == *kind),
            PredicateKind::Name(name) => Ok(item.name == Some(identifier_identity(name))),
            PredicateKind::Not(predicate) => Ok(!predicate.matches(item, work)?),
            PredicateKind::Group(predicate) => predicate.matches(item, work),
            PredicateKind::And(predicates) => {
                for predicate in predicates {
                    if !predicate.matches(item, work)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            PredicateKind::Or(predicates) => {
                for predicate in predicates {
                    if predicate.matches(item, work)? {
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
    attach_comments(input, &mut items);
    Ok(items)
}

fn attach_comments(input: &Input, items: &mut [SelectedItem<'_>]) {
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
