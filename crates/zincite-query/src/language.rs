use std::ops::Range;

use crate::{ItemKind, Limits, Query, QueryError};

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
}

#[derive(Debug)]
pub(crate) struct Predicate {
    pub kind: PredicateKind,
    pub range: Range<usize>,
}

#[derive(Debug)]
pub(crate) enum PredicateKind {
    Kind(ItemKind),
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
            "count" => StageKind::Count,
            "emit" => StageKind::Emit,
            _ => {
                return Err(QueryError::query(
                    name_range,
                    format!("unknown query stage '{name}'"),
                ));
            }
        };
        let terminal = matches!(kind, StageKind::Count | StageKind::Emit);
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
            return Err(parser.error("no stage may follow count or emit"));
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
            PredicateKind::Kind(ItemKind::parse(&value).ok_or_else(|| {
                QueryError::query(value_range, format!("unknown item kind '{value}'"))
            })?)
        } else {
            PredicateKind::Name(value)
        };
        Ok(Predicate {
            kind,
            range: start..self.position,
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
        self.punctuation(b'"', "expected a double-quoted predicate argument")?;
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
