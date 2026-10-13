use std::ops::Range;

use crate::inspection::known_kind;
use crate::structured::{Comparison, ComparisonKind, Scalar, parse_number};
use crate::{Limits, Query, QueryError};

const MAX_QUERY_BYTES: usize = 1_048_576;
const MAX_NESTING: usize = 64;

#[derive(Clone, Debug)]
pub(crate) struct Stage {
    pub kind: StageKind,
    pub range: Range<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum StageKind {
    Items,
    References,
    Declarations,
    Uses,
    TransitiveReferences(usize),
    Types,
    Instantiations,
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

#[derive(Clone, Debug)]
pub(crate) enum Retention {
    Keep(Vec<(String, Range<usize>)>),
    First(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Predicate {
    pub kind: PredicateKind,
    pub range: Range<usize>,
}

#[derive(Clone, Debug)]
pub(crate) enum PredicateKind {
    Kind(String),
    Type(String),
    Instantiation(String),
    SourceKind(String),
    Name(String),
    Not(Box<Predicate>),
    Group(Box<Predicate>),
    And(Vec<Predicate>),
    Or(Vec<Predicate>),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ArgumentKind {
    Head,
    RangeStart,
    RangeEnd,
    TransitiveReferences,
    Kind,
    Name,
    Type,
    Instantiation,
    SourceKind,
}
impl ArgumentKind {
    pub(crate) fn ty(self) -> zirql::ParameterType {
        match self {
            Self::Head | Self::RangeStart | Self::RangeEnd | Self::TransitiveReferences => {
                zirql::ParameterType::Int
            }
            Self::Kind | Self::Name | Self::Type | Self::Instantiation | Self::SourceKind => {
                zirql::ParameterType::String
            }
        }
    }
    pub(crate) fn predicate(
        self,
        value: String,
        range: Range<usize>,
    ) -> Result<PredicateKind, QueryError> {
        Ok(match self {
            Self::Kind => {
                if !known_kind(&value) {
                    return Err(QueryError::query(
                        range,
                        format!("unknown node kind '{value}'"),
                    ));
                }
                PredicateKind::Kind(value)
            }
            Self::Type => {
                if !crate::semantic::known_type_kind(&value) {
                    return Err(QueryError::query(
                        range,
                        format!("unknown type kind '{value}'"),
                    ));
                }
                PredicateKind::Type(value)
            }
            Self::Instantiation => {
                if !matches!(value.as_str(), "par" | "var") {
                    return Err(QueryError::query(
                        range,
                        "instantiation requires par or var",
                    ));
                }
                PredicateKind::Instantiation(value)
            }
            Self::SourceKind => {
                if !matches!(value.as_str(), "user" | "standard_library") {
                    return Err(QueryError::query(
                        range,
                        "source_kind requires user or standard_library",
                    ));
                }
                PredicateKind::SourceKind(value)
            }
            Self::Name => PredicateKind::Name(value),
            Self::Head | Self::RangeStart | Self::RangeEnd | Self::TransitiveReferences => {
                unreachable!("integer stage operand is not a predicate")
            }
        })
    }
}
type ArgumentReader<'a> =
    dyn FnMut(usize, ArgumentKind, Range<usize>) -> Result<(), QueryError> + 'a;

pub(crate) fn parse(source: &str, limits: Limits) -> Result<Query, QueryError> {
    parse_arguments(source, limits, None)
}
pub(crate) fn parse_scoped<'a>(
    source: &'a str,
    limits: Limits,
    arguments: &'a mut ArgumentReader<'a>,
) -> Result<Query, QueryError> {
    parse_arguments(source, limits, Some(arguments))
}
fn parse_arguments<'a>(
    source: &'a str,
    limits: Limits,
    arguments: Option<&'a mut ArgumentReader<'a>>,
) -> Result<Query, QueryError> {
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
        arguments,
    };
    let mut stages = Vec::new();
    let mut editing = false;
    parser.space();
    while parser.position < source.len() {
        let start = parser.position;
        let (name, name_range) = parser.word("expected a query stage")?;
        let kind = match name {
            "items" => StageKind::Items,
            "references" => StageKind::References,
            "declarations" => StageKind::Declarations,
            "uses" => StageKind::Uses,
            "types" => StageKind::Types,
            "instantiations" => StageKind::Instantiations,
            "transitive_references" => {
                parser.punctuation(b'(', "expected '(' after transitive_references")?;
                let depth = parser.integer_argument(start, ArgumentKind::TransitiveReferences)?;
                parser.punctuation(b')', "expected ')' after traversal depth")?;
                StageKind::TransitiveReferences(depth)
            }
            "filter" => {
                parser.punctuation(b'(', "expected '(' after filter")?;
                let predicate = parser.predicate(0)?;
                parser.punctuation(b')', "expected ')' after filter predicate")?;
                StageKind::Filter(predicate)
            }
            "head" => {
                parser.punctuation(b'(', "expected '(' after head")?;
                let count = if parser.arguments.is_some() {
                    parser.integer_argument(start, ArgumentKind::Head)?
                } else {
                    parser.space();
                    let number_start = parser.position;
                    while parser.current().is_some_and(|byte| byte.is_ascii_digit()) {
                        parser.position += 1;
                    }
                    let range = number_start..parser.position;
                    source[range.clone()].parse::<usize>().map_err(|_| {
                        QueryError::query(
                            range,
                            "head requires a non-negative integer fitting usize",
                        )
                    })?
                };
                parser.punctuation(b')', "expected ')' after head count")?;
                StageKind::Head(count)
            }
            "expressions" => StageKind::Expressions,
            "children" => StageKind::Children,
            "subtree" => StageKind::Subtree,
            "range" => {
                parser.punctuation(b'(', "expected '(' after range")?;
                let from = parser.integer_argument(start, ArgumentKind::RangeStart)?;
                parser.punctuation(b',', "expected ',' between range bounds")?;
                let end = parser.integer_argument(start, ArgumentKind::RangeEnd)?;
                parser.punctuation(b')', "expected ')' after range bounds")?;
                if from > end {
                    return Err(QueryError::query(name_range, "range start exceeds end"));
                }
                StageKind::Range(from..end)
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
            "emit" if editing => StageKind::EmitDocument,
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
                "only emit may follow a transformation stage",
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
        nesting_limit: parser.nesting_limit,
    })
}

// The cursor keeps located tokens without a second token buffer. Boolean chains
// are flat vectors; only written groups and `not` consume the nesting budget.
pub(crate) struct Parser<'a> {
    pub(crate) source: &'a str,
    pub(crate) position: usize,
    pub(crate) nesting_limit: usize,
    pub(crate) arguments: Option<&'a mut ArgumentReader<'a>>,
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
        if !matches!(
            name,
            "kind" | "name" | "type" | "instantiation" | "source_kind"
        ) {
            return Err(QueryError::query(
                range,
                format!("unknown predicate '{name}'"),
            ));
        }
        self.punctuation(b'(', "expected '(' after predicate name")?;
        let argument = match name {
            "kind" => ArgumentKind::Kind,
            "type" => ArgumentKind::Type,
            "instantiation" => ArgumentKind::Instantiation,
            "source_kind" => ArgumentKind::SourceKind,
            _ => ArgumentKind::Name,
        };
        let (value, value_range) = self.string_argument(start, argument)?;
        self.punctuation(b')', "expected ')' after predicate argument")?;
        let kind = argument.predicate(value, value_range)?;
        Ok(Predicate {
            kind,
            range: start..self.position,
        })
    }

    fn scalar_range(&mut self) -> Result<Range<usize>, QueryError> {
        self.space();
        let start = self.position;
        let mut depth = 0usize;
        let mut quote = None;
        let mut escaped = false;
        while let Some(byte) = self.current() {
            if let Some(delimiter) = quote {
                if byte == delimiter && !escaped {
                    quote = None;
                }
                escaped = byte == b'\\' && !escaped;
            } else {
                match byte {
                    b'"' | b'\'' => {
                        quote = Some(byte);
                        escaped = false;
                    }
                    b'%' => {
                        while self.current().is_some_and(|byte| byte != b'\n') {
                            self.position += 1;
                        }
                        continue;
                    }
                    b'/' if self.source.as_bytes()[self.position..].starts_with(b"/*") => {
                        self.position += 2;
                        while self.current().is_some()
                            && !self.source.as_bytes()[self.position..].starts_with(b"*/")
                        {
                            self.position += 1;
                        }
                        if self.current().is_some() {
                            self.position += 2;
                        }
                        continue;
                    }
                    b'(' | b'[' | b'{' => {
                        depth += 1;
                        if depth > self.nesting_limit {
                            return Err(self.error("value nesting limit exceeded"));
                        }
                    }
                    b')' | b']' | b'}' if depth > 0 => depth -= 1,
                    b')' | b',' if depth == 0 => break,
                    _ => {}
                }
            }
            self.position += 1;
        }
        if start == self.position {
            return Err(self.error("expected a primitive argument expression"));
        }
        Ok(start..self.position)
    }
    fn integer_argument(&mut self, owner: usize, kind: ArgumentKind) -> Result<usize, QueryError> {
        if self.arguments.is_none() {
            return self.integer();
        }
        self.space();
        let start = self.position;
        if let Ok(value) = self.integer() {
            self.space();
            if matches!(self.current(), Some(b')' | b',')) {
                return Ok(value);
            }
        }
        self.position = start;
        let range = self.scalar_range()?;
        self.arguments.as_mut().expect("scoped argument reader")(owner, kind, range)?;
        Ok(if matches!(kind, ArgumentKind::RangeEnd) {
            usize::MAX
        } else {
            0
        })
    }
    fn string_argument(
        &mut self,
        owner: usize,
        kind: ArgumentKind,
    ) -> Result<(String, Range<usize>), QueryError> {
        if self.arguments.is_none() {
            return self.string();
        }
        self.space();
        let start = self.position;
        if self.current() == Some(b'"') {
            let value = self.string()?;
            self.space();
            if matches!(self.current(), Some(b')' | b',')) {
                return Ok(value);
            }
        }
        self.position = start;
        let range = self.scalar_range()?;
        self.arguments.as_mut().expect("scoped argument reader")(owner, kind, range.clone())?;
        let placeholder = match kind {
            ArgumentKind::Kind => "assignment",
            ArgumentKind::Type => "int",
            ArgumentKind::Instantiation => "par",
            ArgumentKind::SourceKind => "user",
            _ => "",
        };
        Ok((placeholder.to_owned(), range))
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

    pub(crate) fn string(&mut self) -> Result<(String, Range<usize>), QueryError> {
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

    pub(crate) fn word(&mut self, message: &str) -> Result<(&'a str, Range<usize>), QueryError> {
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

    pub(crate) fn take_word(&mut self, word: &str) -> bool {
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

    pub(crate) fn punctuation(&mut self, byte: u8, message: &str) -> Result<(), QueryError> {
        self.space();
        if self.current() != Some(byte) {
            return Err(self.error(message));
        }
        self.position += 1;
        Ok(())
    }

    pub(crate) fn space(&mut self) {
        while self
            .current()
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            self.position += 1;
        }
    }

    pub(crate) fn current(&self) -> Option<u8> {
        self.source.as_bytes().get(self.position).copied()
    }

    pub(crate) fn error(&self, message: &str) -> QueryError {
        let end = self.source[self.position..]
            .chars()
            .next()
            .map_or(self.position, |character| {
                self.position + character.len_utf8()
            });
        QueryError::query(self.position..end, message)
    }
}
