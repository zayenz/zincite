//! Literal views and explicit collection edits over original CST ranges.
use std::cmp::Ordering;
use std::collections::HashSet;
use std::ops::Range;

use serde_json::{Value, json};
use zincite_lint::{EditPart, TextEdit};
use zincite_syntax::{FileMode, NodeKind, SyntaxElement, SyntaxNode, TokenKind};

use crate::inspection::{NodeSelection, SelectedNode, check_collection, into_nodes, original_text};
use crate::language::{Stage, StageKind};
use crate::{Input, Limits, QueryError, QueryResult, Work, assignment_rhs, identifier_identity};

/// Written indexing form; a starting key does not establish bare-tail identities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArrayIndexing {
    Positional,
    Explicit,
    StartingKey,
}

/// One literal with original source spelling and byte coordinates.
#[derive(Debug)]
pub struct LiteralValue<'a> {
    pub file: &'a str,
    pub range: Range<usize>,
    pub spelling: &'a [u8],
    pub kind: LiteralKind<'a>,
}

/// Recursive inspection data, without evaluating computed expressions.
#[derive(Debug)]
pub enum LiteralKind<'a> {
    Integer,
    Float,
    Boolean(bool),
    String,
    Member(&'a str),
    Set(Vec<LiteralValue<'a>>),
    Tuple(Vec<LiteralValue<'a>>),
    Record(Vec<LiteralField<'a>>),
    Array {
        indexing: ArrayIndexing,
        entries: Vec<LiteralEntry<'a>>,
    },
    Matrix {
        column_keys: Vec<LiteralValue<'a>>,
        rows: Vec<LiteralRow<'a>>,
    },
}

/// A field label is distinct from an enum member or string value.
#[derive(Debug)]
pub struct LiteralLabel<'a> {
    pub name: &'a str,
    pub range: Range<usize>,
    pub spelling: &'a [u8],
}

#[derive(Debug)]
pub struct LiteralField<'a> {
    pub label: LiteralLabel<'a>,
    pub value: LiteralValue<'a>,
}

#[derive(Debug)]
pub struct LiteralEntry<'a> {
    pub key: Option<LiteralValue<'a>>,
    pub value: LiteralValue<'a>,
}

#[derive(Debug)]
pub struct LiteralRow<'a> {
    pub key: Option<LiteralValue<'a>>,
    pub cells: Vec<LiteralValue<'a>>,
}

#[derive(Debug)]
pub struct LiteralSelection<'a> {
    input: &'a Input,
    values: Vec<LiteralValue<'a>>,
}

impl<'a> LiteralSelection<'a> {
    pub fn input(&self) -> &'a Input {
        self.input
    }
    pub fn values(&self) -> &[LiteralValue<'a>] {
        &self.values
    }
    pub(crate) fn truncate(&mut self, count: usize) {
        self.values.truncate(count);
    }
}

fn rhs<'a>(node: &'a SyntaxNode, input: &Input) -> Result<&'a SyntaxNode, QueryError> {
    if node.kind() == NodeKind::Assignment {
        assignment_rhs(node).ok_or_else(|| {
            QueryError::input(
                input.original_range(node.range()),
                "assignment has no single right-hand side",
            )
        })
    } else if node.kind() == NodeKind::RecordLiteralField {
        node.child_nodes().next().ok_or_else(|| {
            QueryError::input(
                input.original_range(node.range()),
                "record field has no value",
            )
        })
    } else {
        Ok(node)
    }
}

fn collection_node<'a>(
    node: &'a SyntaxNode,
    input: &Input,
    work: &mut Work,
    range: &Range<usize>,
    limits: Limits,
) -> Result<&'a SyntaxNode, QueryError> {
    let mut node = rhs(node, input)?;
    let mut depth = 0;
    while node.kind() == NodeKind::ParenthesizedExpression {
        work.visit(range)?;
        if depth >= limits.nesting.min(64) {
            return Err(QueryError::query(
                range.clone(),
                "literal nesting limit exceeded",
            ));
        }
        depth += 1;
        node = node.child_nodes().next().unwrap();
    }
    Ok(node)
}

pub(crate) fn unwrapped(mut node: &SyntaxNode) -> &SyntaxNode {
    while node.kind() == NodeKind::ParenthesizedExpression {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}

pub(crate) fn direct_token(input: &Input, node: &SyntaxNode) -> Option<usize> {
    syntax_token(input.syntax(), node)
}

pub(crate) fn syntax_token(
    parsed: &zincite_syntax::ParsedFile,
    node: &SyntaxNode,
) -> Option<usize> {
    node.children().iter().find_map(|child| match child {
        SyntaxElement::Token(index)
            if !matches!(
                parsed.tokens()[*index].kind,
                TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
            ) =>
        {
            Some(*index)
        }
        _ => None,
    })
}

pub(crate) fn has_colon(input: &Input, node: &SyntaxNode) -> bool {
    node.children().iter().any(|child| matches!(child, SyntaxElement::Token(index) if input.syntax().tokens()[*index].kind == TokenKind::Colon))
}

pub(crate) fn array_indexing(node: &SyntaxNode) -> ArrayIndexing {
    let mut explicit = false;
    let mut bare = false;
    for entry in node.child_nodes() {
        explicit |= entry.kind() == NodeKind::IndexedArrayEntry;
        bare |= entry.kind() != NodeKind::IndexedArrayEntry;
    }
    match (explicit, bare) {
        (false, _) => ArrayIndexing::Positional,
        (true, false) => ArrayIndexing::Explicit,
        (true, true) => ArrayIndexing::StartingKey,
    }
}

pub(crate) fn project<'a>(
    result: QueryResult<'a>,
    stage: &Stage,
    work: &mut Work,
    limits: Limits,
) -> Result<QueryResult<'a>, QueryError> {
    let selection = into_nodes(result, &stage.range)?;
    let input = selection.input;
    let mut nodes = Vec::new();
    for selected in selection.nodes {
        work.visit(&stage.range)?;
        let node = collection_node(selected.node, input, work, &stage.range, limits)?;
        work.spend(node.children().len(), &stage.range)?;
        let mut push = |node: &'a SyntaxNode| -> Result<(), QueryError> {
            check_collection(nodes.len(), limits.collection, &stage.range)?;
            nodes.push(SelectedNode {
                node,
                file: input.file(),
                range: input.original_range(node.range()),
            });
            Ok(())
        };
        match stage.kind {
            StageKind::Fields if node.kind() == NodeKind::RecordLiteral => {
                for field in node.child_nodes() {
                    push(field)?;
                }
            }
            StageKind::Elements => match node.kind() {
                NodeKind::SetLiteral | NodeKind::TupleLiteral => {
                    for child in node.child_nodes() {
                        push(child)?;
                    }
                }
                NodeKind::ArrayLiteral => {
                    for entry in node.child_nodes() {
                        push(if entry.kind() == NodeKind::IndexedArrayEntry {
                            entry.child_nodes().nth(1).unwrap()
                        } else {
                            entry
                        })?;
                    }
                }
                NodeKind::MatrixLiteral => {
                    for row in node
                        .child_nodes()
                        .filter(|row| row.kind() == NodeKind::MatrixRow)
                    {
                        work.spend(row.children().len(), &stage.range)?;
                        for cell in row.child_nodes().skip(usize::from(has_colon(input, row))) {
                            push(cell)?;
                        }
                    }
                }
                _ => {
                    return Err(QueryError::input(
                        input.original_range(node.range()),
                        "elements requires a literal set, tuple, array or matrix",
                    ));
                }
            },
            StageKind::Keys => match node.kind() {
                NodeKind::ArrayLiteral => {
                    for entry in node
                        .child_nodes()
                        .filter(|entry| entry.kind() == NodeKind::IndexedArrayEntry)
                    {
                        push(entry.child_nodes().next().unwrap())?;
                    }
                }
                NodeKind::MatrixLiteral => {
                    for row in node.child_nodes() {
                        work.spend(row.children().len(), &stage.range)?;
                        if row.kind() == NodeKind::MatrixColumnIndices {
                            for key in row.child_nodes() {
                                push(key)?;
                            }
                        } else if has_colon(input, row) {
                            push(row.child_nodes().next().unwrap())?;
                        }
                    }
                }
                _ => {
                    return Err(QueryError::input(
                        input.original_range(node.range()),
                        "keys requires a literal array or matrix",
                    ));
                }
            },
            _ => {
                return Err(QueryError::input(
                    input.original_range(node.range()),
                    "fields requires a literal record",
                ));
            }
        }
    }
    Ok(QueryResult::Nodes(NodeSelection { input, nodes }))
}

struct LiteralReader<'a, 'w> {
    input: &'a Input,
    work: &'w mut Work,
    range: &'w Range<usize>,
    limits: Limits,
    entries: usize,
}

impl<'a> LiteralReader<'a, '_> {
    fn read(&mut self, node: &'a SyntaxNode, depth: usize) -> Result<LiteralValue<'a>, QueryError> {
        if depth >= self.limits.nesting.min(64) {
            return Err(QueryError::query(
                self.range.clone(),
                "literal nesting limit exceeded",
            ));
        }
        check_collection(self.entries, self.limits.collection, self.range)?;
        self.entries += 1;
        self.work.visit(self.range)?;
        self.work.spend(node.children().len(), self.range)?;
        let range = self.input.original_range(node.range());
        let kind = match node.kind() {
            NodeKind::ParenthesizedExpression => {
                self.read(node.child_nodes().next().unwrap(), depth + 1)?
                    .kind
            }
            NodeKind::UnaryExpression => {
                let index = direct_token(self.input, node).unwrap();
                if !matches!(
                    self.input.syntax().tokens()[index].kind,
                    TokenKind::Plus | TokenKind::Minus
                ) {
                    return Err(QueryError::input(
                        range,
                        "only numeric signs may wrap literal values",
                    ));
                }
                let inner = self.read(node.child_nodes().next().unwrap(), depth + 1)?;
                match inner.kind {
                    LiteralKind::Integer => LiteralKind::Integer,
                    LiteralKind::Float => LiteralKind::Float,
                    _ => {
                        return Err(QueryError::input(
                            range,
                            "numeric sign requires a numeric literal",
                        ));
                    }
                }
            }
            NodeKind::Expression => {
                let index = direct_token(self.input, node).ok_or_else(|| {
                    QueryError::input(range.clone(), "unsupported computed value")
                })?;
                match self.input.syntax().tokens()[index].kind {
                    TokenKind::IntegerLiteral => LiteralKind::Integer,
                    TokenKind::FloatLiteral => LiteralKind::Float,
                    TokenKind::StringLiteral => LiteralKind::String,
                    TokenKind::True => LiteralKind::Boolean(true),
                    TokenKind::False => LiteralKind::Boolean(false),
                    TokenKind::Identifier | TokenKind::QuotedIdentifier => {
                        let text = std::str::from_utf8(self.input.token_bytes(index)).unwrap();
                        LiteralKind::Member(identifier_identity(text))
                    }
                    _ => {
                        return Err(QueryError::input(
                            range,
                            "unsupported computed value; expected a literal scalar or enum member",
                        ));
                    }
                }
            }
            NodeKind::SetLiteral | NodeKind::TupleLiteral | NodeKind::IndexTuple => {
                let mut values = Vec::new();
                for child in node.child_nodes() {
                    values.push(self.read(child, depth + 1)?);
                }
                if node.kind() == NodeKind::SetLiteral {
                    LiteralKind::Set(values)
                } else {
                    LiteralKind::Tuple(values)
                }
            }
            NodeKind::RecordLiteral => {
                let mut fields = Vec::new();
                for field in node.child_nodes() {
                    self.work.spend(field.children().len(), self.range)?;
                    let index = direct_token(self.input, field).unwrap();
                    let label_range = self.input.token_range(index);
                    let spelling = self.input.token_bytes(index);
                    fields.push(LiteralField {
                        label: LiteralLabel {
                            name: identifier_identity(std::str::from_utf8(spelling).unwrap()),
                            range: label_range,
                            spelling,
                        },
                        value: self.read(field.child_nodes().next().unwrap(), depth + 1)?,
                    });
                }
                LiteralKind::Record(fields)
            }
            NodeKind::ArrayLiteral => {
                let mut entries = Vec::new();
                for entry in node.child_nodes() {
                    let mut children = entry.child_nodes();
                    let key = if entry.kind() == NodeKind::IndexedArrayEntry {
                        Some(self.read(children.next().unwrap(), depth + 1)?)
                    } else {
                        None
                    };
                    let value = if key.is_some() {
                        children.next().unwrap()
                    } else {
                        entry
                    };
                    entries.push(LiteralEntry {
                        key,
                        value: self.read(value, depth + 1)?,
                    });
                }
                LiteralKind::Array {
                    indexing: array_indexing(node),
                    entries,
                }
            }
            NodeKind::MatrixLiteral => {
                let mut column_keys = Vec::new();
                let mut rows = Vec::new();
                for row in node.child_nodes() {
                    self.work.spend(row.children().len(), self.range)?;
                    if row.kind() == NodeKind::MatrixColumnIndices {
                        for key in row.child_nodes() {
                            column_keys.push(self.read(key, depth + 1)?);
                        }
                    } else {
                        check_collection(self.entries, self.limits.collection, self.range)?;
                        self.entries += 1;
                        let mut cells = row.child_nodes();
                        let key = if has_colon(self.input, row) {
                            Some(self.read(cells.next().unwrap(), depth + 1)?)
                        } else {
                            None
                        };
                        let mut values = Vec::new();
                        for cell in cells {
                            values.push(self.read(cell, depth + 1)?);
                        }
                        rows.push(LiteralRow { key, cells: values });
                    }
                }
                LiteralKind::Matrix { column_keys, rows }
            }
            _ => {
                return Err(QueryError::input(
                    range,
                    "unsupported computed value; literal inspection does not evaluate expressions",
                ));
            }
        };
        Ok(LiteralValue {
            file: self.input.file(),
            spelling: &self.input.bytes[range.clone()],
            range,
            kind,
        })
    }
}

pub(crate) fn values<'a>(
    result: QueryResult<'a>,
    stage: &Stage,
    work: &mut Work,
    limits: Limits,
) -> Result<QueryResult<'a>, QueryError> {
    let selection = into_nodes(result, &stage.range)?;
    let mut reader = LiteralReader {
        input: selection.input,
        work,
        range: &stage.range,
        limits,
        entries: 0,
    };
    let mut values = Vec::new();
    for selected in selection.nodes {
        values.push(reader.read(rhs(selected.node, selection.input)?, 0)?);
    }
    Ok(QueryResult::Literals(LiteralSelection {
        input: selection.input,
        values,
    }))
}

fn children<'a>(value: &'a LiteralValue<'a>) -> Vec<&'a LiteralValue<'a>> {
    match &value.kind {
        LiteralKind::Set(values) | LiteralKind::Tuple(values) => values.iter().collect(),
        LiteralKind::Record(fields) => fields.iter().map(|field| &field.value).collect(),
        LiteralKind::Array { entries, .. } => entries
            .iter()
            .flat_map(|entry| entry.key.iter().chain(std::iter::once(&entry.value)))
            .collect(),
        LiteralKind::Matrix { column_keys, rows } => column_keys
            .iter()
            .chain(
                rows.iter()
                    .flat_map(|row| row.key.iter().chain(row.cells.iter())),
            )
            .collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn validate_json(
    selection: &LiteralSelection<'_>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<(), QueryError> {
    let mut pending: Vec<_> = selection.values.iter().collect();
    while let Some(value) = pending.pop() {
        work.spend(value.range.len().max(1) + value.file.len(), range)?;
        original_text(selection.input, &value.range)?;
        pending.extend(children(value));
    }
    Ok(())
}

fn value_json(value: &LiteralValue<'_>) -> Value {
    let mut result = json!({"file":value.file, "range":{"start":value.range.start,"end":value.range.end}, "text":std::str::from_utf8(value.spelling).unwrap()});
    let (kind, content) = match &value.kind {
        LiteralKind::Integer => ("integer", Value::Null),
        LiteralKind::Float => ("float", Value::Null),
        LiteralKind::Boolean(value) => ("boolean", json!({"value":value})),
        LiteralKind::String => ("string", Value::Null),
        LiteralKind::Member(name) => ("member", json!({"name":name})),
        LiteralKind::Set(values) => (
            "set",
            json!({"elements":values.iter().map(value_json).collect::<Vec<_>>()}),
        ),
        LiteralKind::Tuple(values) => (
            "tuple",
            json!({"elements":values.iter().map(value_json).collect::<Vec<_>>()}),
        ),
        LiteralKind::Record(fields) => (
            "record",
            json!({"fields":fields.iter().map(|field| json!({"label":{"name":field.label.name,"range":{"start":field.label.range.start,"end":field.label.range.end},"text":std::str::from_utf8(field.label.spelling).unwrap()},"value":value_json(&field.value)})).collect::<Vec<_>>()}),
        ),
        LiteralKind::Array { indexing, entries } => (
            "array",
            json!({"indexing": match indexing {ArrayIndexing::Positional=>"positional",ArrayIndexing::Explicit=>"explicit",ArrayIndexing::StartingKey=>"starting_key"}, "entries":entries.iter().map(|entry| json!({"key":entry.key.as_ref().map(value_json),"value":value_json(&entry.value)})).collect::<Vec<_>>()}),
        ),
        LiteralKind::Matrix { column_keys, rows } => (
            "matrix",
            json!({"column_keys":column_keys.iter().map(value_json).collect::<Vec<_>>(),"rows":rows.iter().map(|row| json!({"key":row.key.as_ref().map(value_json),"cells":row.cells.iter().map(value_json).collect::<Vec<_>>()})).collect::<Vec<_>>()}),
        ),
    };
    result["kind"] = json!(kind);
    if let Value::Object(content) = content {
        result.as_object_mut().unwrap().extend(content);
    }
    result
}

pub(crate) fn json_value(selection: &LiteralSelection<'_>) -> Result<Value, QueryError> {
    Ok(Value::Array(
        selection.values.iter().map(value_json).collect(),
    ))
}

#[derive(Debug)]
pub(crate) enum Scalar {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    String(String),
    Member(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComparisonKind {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug)]
pub(crate) struct Comparison {
    pub kind: ComparisonKind,
    pub value: Scalar,
}

pub(crate) fn parse_number(text: &str) -> Result<Scalar, &'static str> {
    let (negative, unsigned) = if let Some(text) = text.strip_prefix('-') {
        (true, text)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    if unsigned.starts_with("0x") || unsigned.starts_with("0X") || unsigned.starts_with("0o") {
        if unsigned.contains(['p', 'P', '.']) {
            return Err("hexadecimal-float comparisons are unsupported");
        }
        let radix = if unsigned.starts_with("0o") { 8 } else { 16 };
        let magnitude = u64::from_str_radix(&unsigned[2..], radix)
            .map_err(|_| "integer comparison operand exceeds the signed 64-bit range")?;
        return signed_integer(magnitude, negative);
    }
    if unsigned.contains(['.', 'e', 'E']) {
        // Reject non-numeric Rust spellings (NaN, inf) and partial decimal forms.
        if unsigned.is_empty()
            || !unsigned.bytes().all(|byte| {
                byte.is_ascii_digit() || matches!(byte, b'.' | b'e' | b'E' | b'+' | b'-')
            })
        {
            return Err("expected a numeric literal");
        }
        if !decimal_float(unsigned) {
            return Err("expected a decimal numeric literal");
        }
        let value: f64 = text
            .parse()
            .map_err(|_| "expected a decimal numeric literal")?;
        if !value.is_finite() {
            return Err("float comparison operand overflows the finite range");
        }
        let nonzero = unsigned
            .split(['e', 'E'])
            .next()
            .unwrap()
            .bytes()
            .any(|byte| matches!(byte, b'1'..=b'9'));
        if value == 0.0 && nonzero {
            return Err("float comparison operand underflows the finite range");
        }
        Ok(Scalar::Float(value))
    } else {
        if unsigned.is_empty() || !unsigned.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("expected a numeric literal");
        }
        signed_integer(
            unsigned
                .parse()
                .map_err(|_| "integer comparison operand exceeds the signed 64-bit range")?,
            negative,
        )
    }
}

fn decimal_float(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut position = 0;
    while bytes.get(position).is_some_and(u8::is_ascii_digit) {
        position += 1;
    }
    if position == 0 {
        return false;
    }
    if bytes.get(position) == Some(&b'.') {
        position += 1;
        let start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            position += 1;
        }
        if start == position {
            return false;
        }
    }
    if bytes
        .get(position)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        position += 1;
        if bytes
            .get(position)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            position += 1;
        }
        let start = position;
        while bytes.get(position).is_some_and(u8::is_ascii_digit) {
            position += 1;
        }
        if start == position {
            return false;
        }
    }
    position == bytes.len()
}

fn signed_integer(magnitude: u64, negative: bool) -> Result<Scalar, &'static str> {
    let value = if negative && magnitude == 1_u64 << 63 {
        i64::MIN
    } else {
        let value = i64::try_from(magnitude)
            .map_err(|_| "integer comparison operand exceeds the signed 64-bit range")?;
        if negative { -value } else { value }
    };
    Ok(Scalar::Integer(value))
}

fn numeric_text(
    input: &Input,
    node: &SyntaxNode,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<String, QueryError> {
    let source_range = input.original_range(node.range());
    work.spend(source_range.len(), range)?;
    let mut text = String::new();
    let mut pending = vec![node];
    let mut negative = false;
    while let Some(node) = pending.pop() {
        work.visit(range)?;
        match node.kind() {
            NodeKind::ParenthesizedExpression => pending.push(node.child_nodes().next().unwrap()),
            NodeKind::UnaryExpression => {
                let token = direct_token(input, node).unwrap();
                match input.syntax().tokens()[token].kind {
                    TokenKind::Minus => negative = !negative,
                    TokenKind::Plus => {}
                    _ => {
                        return Err(QueryError::input(
                            source_range,
                            "unsupported computed comparison value",
                        ));
                    }
                }
                pending.push(node.child_nodes().next().unwrap());
            }
            NodeKind::Expression => {
                let index = direct_token(input, node).unwrap();
                text.push_str(std::str::from_utf8(input.token_bytes(index)).unwrap());
            }
            _ => {
                return Err(QueryError::input(
                    source_range,
                    "unsupported computed comparison value",
                ));
            }
        }
    }
    if negative {
        text.insert(0, '-');
    }
    Ok(text)
}

fn scalar(
    input: &Input,
    node: &SyntaxNode,
    value: &LiteralValue<'_>,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Scalar, QueryError> {
    match &value.kind {
        LiteralKind::Integer | LiteralKind::Float => {
            parse_number(&numeric_text(input, node, work, range)?)
                .map_err(|message| QueryError::input(value.range.clone(), message))
        }
        LiteralKind::Boolean(value) => Ok(Scalar::Boolean(*value)),
        LiteralKind::Member(name) => {
            work.spend(name.len(), range)?;
            Ok(Scalar::Member((*name).to_owned()))
        }
        LiteralKind::String => {
            let node = unwrapped(node);
            let index = direct_token(input, node).unwrap();
            let text = std::str::from_utf8(input.token_bytes(index)).unwrap();
            work.spend(text.len(), range)?;
            Ok(Scalar::String(decode_string(text).map_err(|message| {
                QueryError::input(value.range.clone(), message)
            })?))
        }
        _ => Err(QueryError::input(
            value.range.clone(),
            "element comparison requires scalar values; collections and records are incompatible",
        )),
    }
}

fn decode_string(text: &str) -> Result<String, &'static str> {
    let mut result = String::new();
    let mut characters = text[1..text.len() - 1].chars();
    while let Some(character) = characters.next() {
        result.push(if character == '\\' {
            match characters.next() {
                Some('"') => '"',
                Some('\\') => '\\',
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                _ => {
                    return Err(
                        "string comparison supports only escaped quote, backslash, n, r and t",
                    );
                }
            }
        } else {
            character
        });
    }
    Ok(result)
}

fn exact_float(integer: i64) -> Option<f64> {
    let magnitude = integer.unsigned_abs();
    let significant_bits = 64 - magnitude.leading_zeros();
    // Binary64 has 53 significant bits. Lower bits must be zero when the
    // integer is wider; a float-to-i64 round trip would saturate at i64::MAX.
    if significant_bits <= 53 || magnitude.trailing_zeros() >= significant_bits - 53 {
        Some(integer as f64)
    } else {
        None
    }
}

fn compare(
    left: &Scalar,
    comparison: &Comparison,
    range: &Range<usize>,
) -> Result<bool, QueryError> {
    let ordering = match (left, &comparison.value) {
        (Scalar::Integer(a), Scalar::Integer(b)) => a.cmp(b),
        (Scalar::Float(a), Scalar::Float(b)) => a.partial_cmp(b).unwrap(),
        (Scalar::Integer(a), Scalar::Float(b)) => exact_float(*a)
            .ok_or_else(|| {
                QueryError::input(
                    range.clone(),
                    "mixed numeric comparison would lose integer precision",
                )
            })?
            .partial_cmp(b)
            .unwrap(),
        (Scalar::Float(a), Scalar::Integer(b)) => a
            .partial_cmp(&exact_float(*b).ok_or_else(|| {
                QueryError::input(
                    range.clone(),
                    "mixed numeric comparison would lose integer precision",
                )
            })?)
            .unwrap(),
        (Scalar::Boolean(a), Scalar::Boolean(b)) if comparison.kind == ComparisonKind::Eq => {
            a.cmp(b)
        }
        (Scalar::String(a), Scalar::String(b)) | (Scalar::Member(a), Scalar::Member(b))
            if comparison.kind == ComparisonKind::Eq =>
        {
            a.cmp(b)
        }
        _ => {
            return Err(QueryError::input(
                range.clone(),
                "element and comparison operand have incompatible types",
            ));
        }
    };
    Ok(match comparison.kind {
        ComparisonKind::Eq => ordering == Ordering::Equal,
        ComparisonKind::Lt => ordering == Ordering::Less,
        ComparisonKind::Le => ordering != Ordering::Greater,
        ComparisonKind::Gt => ordering == Ordering::Greater,
        ComparisonKind::Ge => ordering != Ordering::Less,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Key {
    Integer(i64),
    Member(String),
}

pub(crate) fn key_parts(
    value: &LiteralValue<'_>,
    node: &SyntaxNode,
    input: &Input,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Vec<Key>, QueryError> {
    match &value.kind {
        LiteralKind::Tuple(values) => {
            let mut parts = Vec::new();
            for (value, child) in values.iter().zip(unwrapped(node).child_nodes()) {
                parts.push(key_part(value, child, input, work, range)?);
            }
            Ok(parts)
        }
        _ => Ok(vec![key_part(value, node, input, work, range)?]),
    }
}

fn key_part(
    value: &LiteralValue<'_>,
    node: &SyntaxNode,
    input: &Input,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Key, QueryError> {
    match scalar(input, node, value, work, range)? {
        Scalar::Integer(value) => Ok(Key::Integer(value)),
        Scalar::Member(name) => Ok(Key::Member(name)),
        _ => Err(QueryError::input(
            value.range.clone(),
            "array keys require integer or enum member identities",
        )),
    }
}

pub(crate) fn validate_keys(
    keys: &[Vec<Key>],
    source_range: &Range<usize>,
    work: &mut Work,
    query_range: &Range<usize>,
) -> Result<(), QueryError> {
    let Some(first) = keys.first() else {
        return Ok(());
    };
    let arity = first.len();
    let mut axes: Vec<HashSet<&Key>> = (0..arity).map(|_| HashSet::new()).collect();
    let mut seen = HashSet::new();
    for key in keys {
        work.spend(key.len().max(1), query_range)?;
        if key.len() != arity || arity == 0 {
            return Err(QueryError::input(
                source_range.clone(),
                "array key arity is inconsistent",
            ));
        }
        if !seen.insert(key) {
            return Err(QueryError::input(
                source_range.clone(),
                "duplicate array key",
            ));
        }
        for (axis, part) in axes.iter_mut().zip(key) {
            axis.insert(part);
        }
    }
    let mut coverage = 1usize;
    for axis in axes {
        coverage = coverage.checked_mul(axis.len()).ok_or_else(|| {
            QueryError::input(
                source_range.clone(),
                "array axis coverage exceeds the supported range",
            )
        })?;
        let mut integers = Vec::new();
        for part in &axis {
            if let Key::Integer(value) = part {
                integers.push(*value);
            }
        }
        if !integers.is_empty() {
            if integers.len() != axis.len() {
                return Err(QueryError::input(
                    source_range.clone(),
                    "array axis mixes integer and enum keys",
                ));
            }
            let min = *integers.iter().min().unwrap();
            let max = *integers.iter().max().unwrap();
            if i128::from(max) - i128::from(min) + 1 != integers.len() as i128 {
                return Err(QueryError::input(
                    source_range.clone(),
                    "integer array keys would leave a nonrectangular axis",
                ));
            }
        }
    }
    if coverage != keys.len() {
        return Err(QueryError::input(
            source_range.clone(),
            "tuple array keys do not cover a rectangular set of axes",
        ));
    }
    Ok(())
}

fn compare_elements(
    input: &Input,
    nodes: &[&SyntaxNode],
    values: &[&LiteralValue<'_>],
    comparison: &Comparison,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Vec<bool>, QueryError> {
    let mut retained = Vec::new();
    for (node, value) in nodes.iter().zip(values) {
        work.visit(range)?;
        retained.push(compare(
            &scalar(input, node, value, work, range)?,
            comparison,
            &value.range,
        )?);
    }
    Ok(retained)
}

/// Resolve original targets, read bounded entries, compare every element, check
/// shape, build source copies, then validate the complete candidate atomically.
pub(crate) fn transform_elements(
    input: &Input,
    result: QueryResult<'_>,
    comparison: &Comparison,
    range: &Range<usize>,
    work: &mut Work,
    limits: Limits,
) -> Result<Vec<u8>, QueryError> {
    if input.mode != FileMode::Data {
        return Err(QueryError::query(
            range.clone(),
            "collection transformations require data-file mode (.dzn)",
        ));
    }
    let targets = into_nodes(result, range)?;
    let mut reader = LiteralReader {
        input,
        work,
        range,
        limits,
        entries: 0,
    };
    let mut edits = Vec::new();
    for selected in targets.nodes {
        reader.work.visit(range)?;
        let target = collection_node(selected.node, input, reader.work, range, limits)?;
        if !matches!(
            target.kind(),
            NodeKind::SetLiteral | NodeKind::ArrayLiteral | NodeKind::MatrixLiteral
        ) {
            return Err(QueryError::input(
                input.original_range(target.range()),
                "filter_elements requires a literal set, array or matrix; tuples and records are inspection values",
            ));
        }
        let literal = reader.read(target, 0)?;
        let edit = match &literal.kind {
            LiteralKind::Set(values) => {
                let nodes: Vec<_> = target.child_nodes().collect();
                let values: Vec<_> = values.iter().collect();
                let retained =
                    compare_elements(input, &nodes, &values, comparison, reader.work, range)?;
                collection_edit(
                    input,
                    target,
                    &nodes,
                    &retained,
                    TokenKind::Comma,
                    reader.work,
                    range,
                )?
            }
            LiteralKind::Array { indexing, entries } => {
                let nodes: Vec<_> = target.child_nodes().collect();
                let values: Vec<_> = entries.iter().map(|entry| &entry.value).collect();
                let value_nodes: Vec<_> = nodes
                    .iter()
                    .map(|node| {
                        if node.kind() == NodeKind::IndexedArrayEntry {
                            node.child_nodes().nth(1).unwrap()
                        } else {
                            node
                        }
                    })
                    .collect();
                let retained =
                    compare_elements(input, &value_nodes, &values, comparison, reader.work, range)?;
                if *indexing == ArrayIndexing::StartingKey {
                    let key = entries[0].key.as_ref().unwrap();
                    let key_node = nodes[0].child_nodes().next().unwrap();
                    let parts = key_parts(key, key_node, input, reader.work, range)?;
                    validate_keys(&[parts], &literal.range, reader.work, range)?;
                    if retained.iter().any(|retain| !retain) {
                        return Err(QueryError::input(
                            literal.range,
                            "cannot filter a starting-key array with a bare tail without model facts establishing surviving key identities",
                        ));
                    }
                }
                if *indexing == ArrayIndexing::Explicit {
                    let mut keys = Vec::new();
                    for (entry, node) in entries.iter().zip(&nodes) {
                        keys.push(key_parts(
                            entry.key.as_ref().unwrap(),
                            node.child_nodes().next().unwrap(),
                            input,
                            reader.work,
                            range,
                        )?);
                    }
                    validate_keys(&keys, &literal.range, reader.work, range)?;
                    let kept: Vec<_> = keys
                        .iter()
                        .zip(&retained)
                        .filter(|(_, retain)| **retain)
                        .map(|(key, _)| key.clone())
                        .collect();
                    validate_keys(&kept, &literal.range, reader.work, range)?;
                    if kept.is_empty() && keys.first().is_some_and(|key| key.len() > 1) {
                        return Err(QueryError::input(
                            literal.range,
                            "empty tuple-key array cannot retain its written axis arity",
                        ));
                    }
                }
                collection_edit(
                    input,
                    target,
                    &nodes,
                    &retained,
                    TokenKind::Comma,
                    reader.work,
                    range,
                )?
            }
            LiteralKind::Matrix { column_keys, rows } => matrix_edit(
                input,
                target,
                column_keys,
                rows,
                comparison,
                reader.work,
                range,
            )?,
            _ => unreachable!(),
        };
        edits.push(edit);
    }
    let ancestors = enclosing_shapes(input, &edits, reader.work, range)?;
    let candidate = crate::finish_candidate(input, &edits, &[], reader.work, range)?;
    for (ancestor, kind) in ancestors {
        let start = shifted_boundary(ancestor.start, &edits);
        let end = shifted_boundary(ancestor.end, &edits);
        let mut pending = vec![candidate.syntax().tree()];
        let mut found = false;
        while let Some(node) = pending.pop() {
            reader.work.visit(range)?;
            let node_range = candidate.original_range(node.range());
            if node_range.start == start && node_range.end == end && node.kind() == kind {
                validate_enclosing_shape(&candidate, node, reader.work, range, limits)
                    .map_err(|error| QueryError::input(ancestor.clone(), error.message))?;
                found = true;
                break;
            }
            reader.work.spend(node.children().len(), range)?;
            pending.extend(node.child_nodes().filter(|child| {
                let child_range = candidate.original_range(child.range());
                child_range.start <= start && child_range.end >= end
            }));
        }
        if !found {
            return Err(QueryError::query(
                range.clone(),
                "data edit candidate changed an enclosing collection unexpectedly",
            ));
        }
    }
    Ok(candidate.bytes)
}

fn enclosing_shapes(
    input: &Input,
    edits: &[TextEdit],
    work: &mut Work,
    range: &Range<usize>,
) -> Result<Vec<(Range<usize>, NodeKind)>, QueryError> {
    let mut ancestors = Vec::new();
    let mut seen = HashSet::new();
    for edit in edits {
        let mut pending = vec![input.syntax().tree()];
        while let Some(node) = pending.pop() {
            work.visit(range)?;
            let node_range = input.original_range(node.range());
            work.spend(node.children().len(), range)?;
            if node_range != edit.range
                && (node.kind() == NodeKind::MatrixLiteral
                    || (node.kind() == NodeKind::ArrayLiteral
                        && array_indexing(node) != ArrayIndexing::Positional))
                && seen.insert(node_range.start)
            {
                ancestors.push((node_range, node.kind()));
            }
            pending.extend(node.child_nodes().filter(|child| {
                let child_range = input.original_range(child.range());
                child_range.start <= edit.range.start && child_range.end >= edit.range.end
            }));
        }
    }
    Ok(ancestors)
}

fn shifted_boundary(position: usize, edits: &[TextEdit]) -> usize {
    let mut shifted = position;
    for edit in edits.iter().filter(|edit| edit.range.end <= position) {
        let length: usize = edit
            .replacement
            .iter()
            .map(|part| match part {
                EditPart::Original(range) => range.len(),
                EditPart::Text(text) => text.len(),
            })
            .sum();
        shifted = shifted - edit.range.len() + length;
    }
    shifted
}

fn validate_enclosing_shape(
    input: &Input,
    node: &SyntaxNode,
    work: &mut Work,
    range: &Range<usize>,
    limits: Limits,
) -> Result<(), QueryError> {
    // Filtering a nested value preserves its enclosing entry/cell count. Check
    // enclosing keys and axes without evaluating unrelated sibling values.
    let mut reader = LiteralReader {
        input,
        work,
        range,
        limits,
        entries: 0,
    };
    let source_range = input.original_range(node.range());
    reader.work.spend(node.children().len(), range)?;
    let mut axes = Vec::new();
    if node.kind() == NodeKind::ArrayLiteral {
        for entry in node
            .child_nodes()
            .filter(|entry| entry.kind() == NodeKind::IndexedArrayEntry)
        {
            let key_node = entry.child_nodes().next().unwrap();
            let key = reader.read(key_node, 0)?;
            axes.push(key_parts(&key, key_node, input, reader.work, range)?);
        }
        validate_keys(&axes, &source_range, reader.work, range)?;
    } else {
        let mut width = None;
        let mut column_width = None;
        let mut row_keyed = None;
        let mut row_keys = Vec::new();
        for row in node.child_nodes() {
            reader.work.spend(row.children().len(), range)?;
            if row.kind() == NodeKind::MatrixColumnIndices {
                column_width = Some(row.child_nodes().count());
                for key_node in row.child_nodes() {
                    let key = reader.read(key_node, 0)?;
                    axes.push(vec![key_part(&key, key_node, input, reader.work, range)?]);
                }
            } else {
                let keyed = has_colon(input, row);
                if row_keyed.is_some_and(|previous| previous != keyed) {
                    return Err(QueryError::input(
                        source_range,
                        "matrix rows mix explicit and positional keys",
                    ));
                }
                row_keyed = Some(keyed);
                let cells = row.child_nodes().count() - usize::from(keyed);
                if width.is_some_and(|previous| previous != cells) {
                    return Err(QueryError::input(
                        source_range,
                        "enclosing matrix must remain rectangular",
                    ));
                }
                width = Some(cells);
                if keyed {
                    let key_node = row.child_nodes().next().unwrap();
                    let key = reader.read(key_node, 0)?;
                    row_keys.push(vec![key_part(&key, key_node, input, reader.work, range)?]);
                }
            }
        }
        if column_width.is_some_and(|columns| width != Some(columns)) {
            return Err(QueryError::input(
                source_range,
                "enclosing matrix cells and column keys must remain rectangular",
            ));
        }
        validate_keys(&axes, &source_range, reader.work, range)?;
        validate_keys(&row_keys, &source_range, reader.work, range)?;
    }
    Ok(())
}

fn matrix_edit(
    input: &Input,
    target: &SyntaxNode,
    columns: &[LiteralValue<'_>],
    rows: &[LiteralRow<'_>],
    comparison: &Comparison,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<TextEdit, QueryError> {
    let source_range = input.original_range(target.range());
    let written: Vec<_> = target.child_nodes().collect();
    let row_nodes: Vec<_> = written
        .iter()
        .copied()
        .filter(|node| node.kind() == NodeKind::MatrixRow)
        .collect();
    let width = rows.first().map_or(0, |row| row.cells.len());
    if rows.iter().any(|row| row.cells.len() != width)
        || (!columns.is_empty() && columns.len() != width)
    {
        return Err(QueryError::input(
            source_range,
            "matrix cells and column keys must form a rectangular shape",
        ));
    }
    let row_keyed = rows.first().is_some_and(|row| row.key.is_some());
    if rows.iter().any(|row| row.key.is_some() != row_keyed) {
        return Err(QueryError::input(
            source_range,
            "matrix rows mix explicit and positional keys",
        ));
    }
    for (keys, nodes) in [
        (
            columns.iter().collect::<Vec<_>>(),
            written
                .first()
                .filter(|node| node.kind() == NodeKind::MatrixColumnIndices)
                .map_or(Vec::new(), |node| node.child_nodes().collect()),
        ),
        (
            rows.iter().filter_map(|row| row.key.as_ref()).collect(),
            row_nodes
                .iter()
                .filter(|node| has_colon(input, node))
                .map(|node| node.child_nodes().next().unwrap())
                .collect(),
        ),
    ] {
        let mut parts = Vec::new();
        for (key, node) in keys.iter().zip(nodes) {
            parts.push(vec![key_part(key, node, input, work, range)?]);
        }
        validate_keys(&parts, &source_range, work, range)?;
    }
    let mut common = None;
    let mut parts = Vec::new();
    for (row, node) in rows.iter().zip(row_nodes) {
        let nodes: Vec<_> = node
            .child_nodes()
            .skip(usize::from(row.key.is_some()))
            .collect();
        let values: Vec<_> = row.cells.iter().collect();
        let retained = compare_elements(input, &nodes, &values, comparison, work, range)?;
        if let Some(common) = &common {
            if common != &retained {
                return Err(QueryError::input(
                    source_range,
                    "matrix filtering requires the same surviving columns in every row",
                ));
            }
        } else {
            common = Some(retained.clone());
        }
        parts.push(collection_edit(
            input,
            node,
            &nodes,
            &retained,
            TokenKind::Comma,
            work,
            range,
        )?);
    }
    if let Some(retained) = common {
        if !retained.iter().any(|retain| *retain) {
            return Err(QueryError::input(
                source_range,
                "matrix filtering cannot represent an empty surviving column axis",
            ));
        }
        if !columns.is_empty() {
            let header = written[0];
            let nodes: Vec<_> = header.child_nodes().collect();
            let mut kept = Vec::new();
            for ((key, node), retain) in columns.iter().zip(&nodes).zip(&retained) {
                if *retain {
                    kept.push(vec![key_part(key, node, input, work, range)?]);
                }
            }
            validate_keys(&kept, &source_range, work, range)?;
            parts.push(collection_edit(
                input,
                header,
                &nodes,
                &retained,
                TokenKind::Colon,
                work,
                range,
            )?);
        }
    }
    parts.sort_by_key(|part| part.range.start);
    let mut replacement = Vec::new();
    let mut cursor = source_range.start;
    for part in parts {
        if cursor < part.range.start {
            replacement.push(EditPart::Original(cursor..part.range.start));
        }
        replacement.extend(part.replacement);
        cursor = part.range.end;
    }
    if cursor < source_range.end {
        replacement.push(EditPart::Original(cursor..source_range.end));
    }
    Ok(TextEdit {
        range: source_range,
        replacement,
    })
}

pub(crate) fn collection_edit(
    input: &Input,
    target: &SyntaxNode,
    entries: &[&SyntaxNode],
    retained: &[bool],
    separator: TokenKind,
    work: &mut Work,
    range: &Range<usize>,
) -> Result<TextEdit, QueryError> {
    let source_range = input.original_range(target.range());
    let trailing_separator = separator == TokenKind::Colon;
    if retained.iter().all(|retain| *retain) {
        return Ok(TextEdit {
            range: source_range.clone(),
            replacement: vec![EditPart::Original(source_range)],
        });
    }
    let entry_ranges: Vec<_> = entries
        .iter()
        .map(|entry| input.original_range(entry.range()))
        .collect();
    let mut owned = entry_ranges.clone();
    let separators: Vec<_> = target
        .children()
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Token(index) if input.syntax().tokens()[*index].kind == separator => {
                Some(input.token_range(*index))
            }
            _ => None,
        })
        .filter(|token| {
            entry_ranges
                .first()
                .is_some_and(|first| token.start >= first.start)
        })
        .collect();
    attach_entry_comments(input, &entry_ranges, &mut owned, &source_range, work, range)?;
    let mut deleted = separators;
    for (entry, retain) in owned.into_iter().zip(retained) {
        if !retain {
            deleted.push(entry);
        }
    }
    deleted.sort_by_key(|range| (range.start, range.end));
    let kept: Vec<_> = entry_ranges
        .iter()
        .zip(retained)
        .filter(|(_, retain)| **retain)
        .map(|(range, _)| range)
        .collect();
    let punctuation = match separator {
        TokenKind::Colon => ":",
        TokenKind::Pipe => "|",
        _ => ",",
    };
    let insertions: Vec<_> = kept
        .iter()
        .enumerate()
        .filter(|(index, _)| trailing_separator || index + 1 < kept.len())
        .map(|(_, entry)| entry.end)
        .collect();
    // Cut only removed entries, their attached comments and old separators.
    // Copy every other byte, inserting separators before retained line comments.
    let mut replacement = Vec::new();
    let mut cursor = source_range.start;
    let mut insertion = 0;
    for deleted in deleted
        .iter()
        .chain(std::iter::once(&(source_range.end..source_range.end)))
    {
        while insertions
            .get(insertion)
            .is_some_and(|position| *position <= deleted.start)
        {
            let position = insertions[insertion];
            if cursor < position {
                replacement.push(EditPart::Original(cursor..position));
            }
            replacement.push(EditPart::Text(punctuation.to_owned()));
            cursor = cursor.max(position);
            insertion += 1;
        }
        if cursor < deleted.start {
            replacement.push(EditPart::Original(cursor..deleted.start));
        }
        cursor = cursor.max(deleted.end);
    }
    Ok(TextEdit {
        range: source_range,
        replacement,
    })
}

fn attach_entry_comments(
    input: &Input,
    entries: &[Range<usize>],
    owned: &mut [Range<usize>],
    scope: &Range<usize>,
    work: &mut Work,
    query_range: &Range<usize>,
) -> Result<(), QueryError> {
    // Delimiters bound the first/last gaps. Matrix rows have no delimiters;
    // their prefix (an optional row key) stays outside the first attachment.
    let tokens = input.syntax().tokens();
    let first_token =
        tokens.partition_point(|token| token.range.end + input.syntax_offset <= scope.start);
    let last_token =
        tokens.partition_point(|token| token.range.start + input.syntax_offset < scope.end);
    let entry_start = entries.first().map_or(scope.start, |entry| entry.start);
    let prefix_end =
        tokens.partition_point(|token| token.range.end + input.syntax_offset <= entry_start);
    work.spend(prefix_end.saturating_sub(first_token), query_range)?;
    let prefix = &tokens[first_token..prefix_end];
    let start = tokens
        .get(first_token)
        .filter(|token| matches!(token.kind, TokenKind::LeftBracket | TokenKind::LeftBrace))
        .map_or_else(
            || {
                prefix
                    .iter()
                    .rev()
                    .find(|token| token.kind == TokenKind::Colon)
                    .map_or(entry_start, |token| token.range.end + input.syntax_offset)
            },
            |token| token.range.end + input.syntax_offset,
        );
    let end = tokens
        .get(last_token.saturating_sub(1))
        .filter(|token| matches!(token.kind, TokenKind::RightBracket | TokenKind::RightBrace))
        .map_or(scope.end, |token| token.range.start + input.syntax_offset);
    for next in 0..=entries.len() {
        let gap_start = if next == 0 {
            start
        } else {
            entries[next - 1].end
        };
        let gap_end = entries.get(next).map_or(end, |entry| entry.start);
        let first =
            tokens.partition_point(|token| token.range.end + input.syntax_offset <= gap_start);
        let last =
            tokens.partition_point(|token| token.range.start + input.syntax_offset < gap_end);
        let mut boundary = gap_start;
        if next > 0 {
            let mut trailing = None;
            for (index, token) in tokens.iter().enumerate().take(last).skip(first) {
                work.visit(query_range)?;
                let token_range = input.token_range(index);
                let bytes = input.token_bytes(index);
                if token.kind == TokenKind::Whitespace
                    && let Some(length) = crate::first_line_end(bytes)
                {
                    if trailing.is_some() {
                        owned[next - 1].end = token_range.start + length;
                    }
                    boundary = token_range.start + length;
                    break;
                }
                if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
                    trailing = Some(token_range.end);
                    owned[next - 1].end = token_range.end;
                }
            }
            boundary = boundary.max(owned[next - 1].end);
        }
        if next < entries.len() {
            let mut attachment = entries[next].start;
            let mut comment = false;
            for index in (first..last).rev() {
                work.visit(query_range)?;
                let token_range = input.token_range(index);
                if token_range.end <= boundary {
                    break;
                }
                let token = &tokens[index];
                if token.kind == TokenKind::Whitespace {
                    if crate::line_breaks(input.token_bytes(index)) >= 2 {
                        break;
                    }
                    attachment = crate::line_indent_start(&input.bytes, attachment, boundary);
                } else if matches!(token.kind, TokenKind::Comma | TokenKind::Colon) {
                    break;
                } else if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
                    && crate::standalone(&input.bytes, token_range.start, boundary)
                {
                    comment = true;
                    attachment =
                        crate::line_indent_start(&input.bytes, token_range.start, boundary);
                } else {
                    break;
                }
            }
            if comment {
                owned[next].start = attachment.max(boundary);
            }
        }
    }
    Ok(())
}

/// Decode only an index, leaving computed descendants in the value untouched.
pub(crate) fn read_key(
    input: &Input,
    node: &SyntaxNode,
    work: &mut Work,
    range: &Range<usize>,
    limits: Limits,
) -> Result<Vec<Key>, QueryError> {
    let mut reader = LiteralReader {
        input,
        work,
        range,
        limits,
        entries: 0,
    };
    let value = reader.read(node, 0)?;
    key_parts(&value, node, input, reader.work, range)
}
