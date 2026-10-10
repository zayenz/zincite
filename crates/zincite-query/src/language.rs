use std::ops::Range;

use crate::inspection::known_kind;
use crate::structured::{Comparison, ComparisonKind, Scalar, parse_number};
use crate::{Limits, Query, QueryError};

const MAX_QUERY_BYTES: usize = 1_048_576;
const MAX_NESTING: usize = 64;

#[derive(Debug)]
pub(crate) struct Stage {
    pub kind: StageKind,
    pub range: Range<usize>,
}

#[derive(Debug)]
pub(crate) enum StageKind {
    Items,
    Filter(Predicate),
    Head(usize),
    Count,
    Emit,
    Expressions,
    Children,
    Subtree,
    Range(std::ops::Range<usize>),
    Names,
    CallNames,
    AnnotationNames,
    Text,
    Values,
    Fields,
    Elements,
    Keys,
    FilterElements(Comparison),
    Unique,
    Tally,
    Json,
    SetValue(String),
    Remove,
    ReduceEnum(String, Retention),
    EmitDocument,
}

#[derive(Debug)]
pub(crate) enum Retention {
    Keep(Vec<(String, Range<usize>)>),
    First(usize),
}

#[derive(Debug)]
pub(crate) struct Predicate {
    pub kind: PredicateKind,
    pub range: Range<usize>,
}

#[derive(Debug)]
pub(crate) enum PredicateKind {
    Kind(String),
    Name(String),
    Not(Box<Predicate>),
    Group(Box<Predicate>),
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
}

pub(crate) fn parse(source: &str, limits: Limits) -> Result<Query, QueryError> {
    if source.len() > MAX_QUERY_BYTES {
        return Err(QueryError::query(
            0..0,
            "query byte limit exceeded (1048576 bytes)",
        ));
    }
    let mut parser = Parser {
        source,
        position: 0,
        nesting_limit: limits.nesting.min(MAX_NESTING),
    };
    let mut stages = Vec::new();
    let mut editing = false;
    parser.space();
    while parser.position < source.len() {
        let start = parser.position;
        let (name, name_range) = parser.word("expected a query stage")?;
        let kind = match name {
            "items" => StageKind::Items,
            "filter" => {
                parser.punctuation(b'(', "expected '(' after filter")?;
                let predicate = parser.predicate(0)?;
                parser.punctuation(b')', "expected ')' after filter predicate")?;
                StageKind::Filter(predicate)
            }
            "head" => {
                parser.punctuation(b'(', "expected '(' after head")?;
                parser.space();
                let number_start = parser.position;
                while parser.current().is_some_and(|byte| byte.is_ascii_digit()) {
                    parser.position += 1;
                }
                let number_range = number_start..parser.position;
                let count = source[number_range.clone()].parse::<usize>().map_err(|_| {
                    QueryError::query(
                        number_range,
                        "head requires a non-negative integer fitting usize",
                    )
                })?;
                parser.punctuation(b')', "expected ')' after head count")?;
                StageKind::Head(count)
            }
            "expressions" => StageKind::Expressions,
            "children" => StageKind::Children,
            "subtree" => StageKind::Subtree,
            "range" => {
                parser.punctuation(b'(', "expected '(' after range")?;
                let start = parser.integer()?;
                parser.punctuation(b',', "expected ',' between range bounds")?;
                let end = parser.integer()?;
                parser.punctuation(b')', "expected ')' after range bounds")?;
                if start > end {
                    return Err(QueryError::query(name_range, "range start exceeds end"));
                }
                StageKind::Range(start..end)
            }
            "names" => StageKind::Names,
            "call_names" => StageKind::CallNames,
            "annotation_names" => StageKind::AnnotationNames,
            "text" => StageKind::Text,
            "values" => StageKind::Values,
            "fields" => StageKind::Fields,
            "elements" => StageKind::Elements,
            "keys" => StageKind::Keys,
            "filter_elements" => {
                parser.punctuation(b'(', "expected '(' after filter_elements")?;
                let comparison = parser.comparison()?;
                parser.punctuation(b')', "expected ')' after element comparison")?;
                StageKind::FilterElements(comparison)
            }
            "unique" => StageKind::Unique,
            "tally" => StageKind::Tally,
            "json" => StageKind::Json,
            "count" => StageKind::Count,
            "emit" => StageKind::Emit,
            "set_value" => {
                parser.punctuation(b'(', "expected '(' after set_value")?;
                let (value, _) = parser.string()?;
                parser.punctuation(b')', "expected ')' after replacement expression")?;
                StageKind::SetValue(value)
            }
            "reduce_enum" => {
                parser.punctuation(b'(', "expected '(' after reduce_enum")?;
                let name = parser.string()?.0;
                if crate::identifier_identity(&name).is_empty() {
                    return Err(QueryError::query(
                        name_range,
                        "enum identity must not be empty",
                    ));
                }
                parser.punctuation(b',', "expected ',' before enum retention")?;
                let (selection, selection_range) = parser.word("expected keep or keep_first")?;
                parser.punctuation(b'(', "expected '(' after retention")?;
                let retention = match selection {
                    "keep_first" => Retention::First(parser.integer()?),
                    "keep" => {
                        let mut names = Vec::new();
                        parser.space();
                        if parser.current() != Some(b')') {
                            loop {
                                if names.len() >= limits.collection {
                                    return Err(QueryError::query(
                                        start..parser.position,
                                        "query collection limit exceeded",
                                    ));
                                }
                                names.push(parser.string()?);
                                parser.space();
                                if parser.current() != Some(b',') {
                                    break;
                                }
                                parser.punctuation(b',', "expected ',' between members")?;
                            }
                        }
                        Retention::Keep(names)
                    }
                    _ => {
                        return Err(QueryError::query(
                            selection_range,
                            "expected keep or keep_first",
                        ));
                    }
                };
                parser.punctuation(b')', "expected ')' after retention")?;
                parser.punctuation(b')', "expected ')' after reduce_enum")?;
                StageKind::ReduceEnum(name, retention)
            }
            "remove" => StageKind::Remove,
            "emit_document" => StageKind::EmitDocument,
            _ => {
                return Err(QueryError::query(
                    name_range,
                    format!("unknown query stage '{name}'"),
                ));
            }
        };
        if stages
            .last()
            .is_some_and(|stage: &Stage| matches!(stage.kind, StageKind::Count | StageKind::Tally))
            && !matches!(kind, StageKind::Json)
        {
            return Err(QueryError::query(
                start..parser.position,
                "only json may follow count or tally",
            ));
        }
        if editing && !matches!(kind, StageKind::EmitDocument) {
            return Err(QueryError::query(
                start..parser.position,
                "only emit_document may follow a transformation stage",
            ));
        }
        if matches!(kind, StageKind::EmitDocument) && !editing {
            return Err(QueryError::query(
                start..parser.position,
                "emit_document requires a transformation stage",
            ));
        }
        editing |= matches!(
            kind,
            StageKind::SetValue(_)
                | StageKind::Remove
                | StageKind::FilterElements(_)
                | StageKind::ReduceEnum(_, _)
        );
        let terminal = matches!(
            kind,
            StageKind::Json | StageKind::Emit | StageKind::EmitDocument
        );
        stages.push(Stage {
            kind,
            range: start..parser.position,
        });
        parser.space();
        if parser.position == source.len() {
            break;
        }
        parser.punctuation(b'|', "expected '|' between query stages")?;
        if terminal {
            return Err(parser.error("no stage may follow an emitter"));
        }
        if parser.position == source.len() {
            return Err(parser.error("expected a query stage after '|'"));
        }
    }
    Ok(Query {
        stages,
        range: 0..source.len(),
    })
}

// The cursor keeps located tokens without a second token buffer. Boolean chains
// are flat vectors; only written groups and `not` consume the nesting budget.
struct Parser<'a> {
    source: &'a str,
    position: usize,
    nesting_limit: usize,
}

impl<'a> Parser<'a> {
    fn comparison(&mut self) -> Result<Comparison, QueryError> {
        let (name, range) = self.word("expected eq, lt, le, gt or ge comparison")?;
        let kind = match name {
            "eq" => ComparisonKind::Eq,
            "lt" => ComparisonKind::Lt,
            "le" => ComparisonKind::Le,
            "gt" => ComparisonKind::Gt,
            "ge" => ComparisonKind::Ge,
            _ => return Err(QueryError::query(range, "unknown element comparison")),
        };
        self.punctuation(b'(', "expected '(' after comparison")?;
        self.space();
        let start = self.position;
        let value = if self.current() == Some(b'"') {
            Scalar::String(self.string()?.0)
        } else if self.take_word("member") {
            self.punctuation(b'(', "expected '(' after member")?;
            let name = self.string()?.0;
            let name = crate::identifier_identity(&name).to_owned();
            if name.is_empty() {
                return Err(QueryError::query(
                    start..self.position,
                    "member identity must not be empty",
                ));
            }
            self.punctuation(b')', "expected ')' after member identity")?;
            Scalar::Member(name)
        } else if self.take_word("true") {
            Scalar::Boolean(true)
        } else if self.take_word("false") {
            Scalar::Boolean(false)
        } else {
            while self
                .current()
                .is_some_and(|byte| !byte.is_ascii_whitespace() && byte != b')')
            {
                self.position += 1;
            }
            parse_number(&self.source[start..self.position])
                .map_err(|message| QueryError::query(start..self.position, message))?
        };
        if kind != ComparisonKind::Eq && !matches!(value, Scalar::Integer(_) | Scalar::Float(_)) {
            return Err(QueryError::query(
                start..self.position,
                "numeric comparison requires a numeric operand",
            ));
        }
        self.punctuation(b')', "expected ')' after comparison operand")?;
        Ok(Comparison { kind, value })
    }

    fn predicate(&mut self, depth: usize) -> Result<Predicate, QueryError> {
        self.boolean_chain(depth, false)
    }

    fn boolean_chain(&mut self, depth: usize, conjunction: bool) -> Result<Predicate, QueryError> {
        let mut predicates = Vec::new();
        loop {
            predicates.push(if conjunction {
                self.primary(depth)?
            } else {
                self.boolean_chain(depth, true)?
            });
            if !self.take_word(if conjunction { "and" } else { "or" }) {
                break;
            }
        }
        if predicates.len() == 1 {
            return Ok(predicates.pop().unwrap());
        }
        let range = predicates[0].range.start..predicates.last().unwrap().range.end;
        let kind = if conjunction {
            PredicateKind::And(predicates)
        } else {
            PredicateKind::Or(predicates)
        };
        Ok(Predicate { kind, range })
    }

    fn primary(&mut self, depth: usize) -> Result<Predicate, QueryError> {
        self.space();
        let start = self.position;
        if self.take_word("not") {
            self.check_depth(depth, start)?;
            let predicate = self.primary(depth + 1)?;
            let range = start..predicate.range.end;
            return Ok(Predicate {
                kind: PredicateKind::Not(Box::new(predicate)),
                range,
            });
        }
        if self.current() == Some(b'(') {
            self.check_depth(depth, start)?;
            self.position += 1;
            let predicate = self.predicate(depth + 1)?;
            self.punctuation(b')', "expected ')' after parenthesized predicate")?;
            return Ok(Predicate {
                kind: PredicateKind::Group(Box::new(predicate)),
                range: start..self.position,
            });
        }
        let (name, range) = self.word("expected kind(...) or name(...) predicate")?;
        if !matches!(name, "kind" | "name") {
            return Err(QueryError::query(
                range,
                format!("unknown predicate '{name}'"),
            ));
        }
        self.punctuation(b'(', "expected '(' after predicate name")?;
        let (value, value_range) = self.string()?;
        self.punctuation(b')', "expected ')' after predicate argument")?;
        let kind = if name == "kind" {
            if !known_kind(&value) {
                return Err(QueryError::query(
                    value_range,
                    format!("unknown node kind '{value}'"),
                ));
            }
            PredicateKind::Kind(value)
        } else {
            PredicateKind::Name(value)
        };
        Ok(Predicate {
            kind,
            range: start..self.position,
        })
    }

    fn integer(&mut self) -> Result<usize, QueryError> {
        self.space();
        let start = self.position;
        while self.current().is_some_and(|byte| byte.is_ascii_digit()) {
            self.position += 1;
        }
        self.source[start..self.position].parse().map_err(|_| {
            QueryError::query(
                start..self.position,
                "range bounds require non-negative integers fitting usize",
            )
        })
    }

    fn check_depth(&self, depth: usize, start: usize) -> Result<(), QueryError> {
        if depth >= self.nesting_limit {
            Err(QueryError::query(
                start..self.position.max(start + 1),
                "query nesting limit exceeded",
            ))
        } else {
            Ok(())
        }
    }

    fn string(&mut self) -> Result<(String, Range<usize>), QueryError> {
        self.space();
        let start = self.position;
        self.punctuation(b'"', "expected a double-quoted argument")?;
        let mut value = String::new();
        while let Some(byte) = self.current() {
            match byte {
                b'"' => {
                    self.position += 1;
                    return Ok((value, start..self.position));
                }
                b'\\' => {
                    let escape_start = self.position;
                    self.position += 1;
                    let character = match self.current() {
                        Some(b'"') => '"',
                        Some(b'\\') => '\\',
                        Some(b'n') => '\n',
                        Some(b'r') => '\r',
                        Some(b't') => '\t',
                        _ => {
                            return Err(QueryError::query(
                                escape_start..self.position,
                                "supported string escapes are \\\" , \\\\ , \\n , \\r and \\t",
                            ));
                        }
                    };
                    self.position += 1;
                    value.push(character);
                }
                _ => {
                    let character = self.source[self.position..].chars().next().unwrap();
                    if character.is_control() {
                        return Err(self
                            .error("literal control characters are not allowed in query strings"));
                    }
                    value.push(character);
                    self.position += character.len_utf8();
                }
            }
        }
        Err(QueryError::query(
            start..self.position,
            "unterminated query string",
        ))
    }

    fn word(&mut self, message: &str) -> Result<(&'a str, Range<usize>), QueryError> {
        self.space();
        let start = self.position;
        if !self
            .current()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        {
            return Err(self.error(message));
        }
        while self
            .current()
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            self.position += 1;
        }
        Ok((&self.source[start..self.position], start..self.position))
    }

    fn take_word(&mut self, word: &str) -> bool {
        self.space();
        if self.source[self.position..].starts_with(word)
            && !self
                .source
                .as_bytes()
                .get(self.position + word.len())
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            self.position += word.len();
            true
        } else {
            false
        }
    }

    fn punctuation(&mut self, byte: u8, message: &str) -> Result<(), QueryError> {
        self.space();
        if self.current() != Some(byte) {
            return Err(self.error(message));
        }
        self.position += 1;
        Ok(())
    }

    fn space(&mut self) {
        while self
            .current()
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.position += 1;
        }
    }

    fn current(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    fn error(&self, message: &str) -> QueryError {
        let end = self.source[self.position..]
            .chars()
            .next()
            .map_or(self.position, |character| {
                self.position + character.len_utf8()
            });
        QueryError::query(self.position..end, message)
    }
}
