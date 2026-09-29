use crate::{Diagnostic, Token, TokenKind};
use std::ops::Range;

pub(super) fn scan(source: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut scanner = Scanner {
        source,
        position: 0,
        diagnostics: Vec::new(),
        interpolations: Vec::new(),
        after_field_dot: false,
    };
    let mut tokens = Vec::new();
    while scanner.position < source.len() {
        let start = scanner.position;
        let kind = scanner.next_kind();
        if !matches!(
            kind,
            TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
        ) {
            scanner.after_field_dot = kind == TokenKind::Dot;
        }
        debug_assert!(scanner.position > start);
        tokens.push(Token {
            kind,
            range: start..scanner.position,
        });
    }
    for interpolation in &scanner.interpolations {
        scanner.diagnostics.push(Diagnostic {
            range: interpolation.start..source.len(),
            message: "unterminated string interpolation".to_owned(),
        });
    }
    (tokens, scanner.diagnostics)
}

struct Scanner<'a> {
    source: &'a str,
    position: usize,
    diagnostics: Vec<Diagnostic>,
    interpolations: Vec<Interpolation>,
    after_field_dot: bool,
}

impl Scanner<'_> {
    fn next_kind(&mut self) -> TokenKind {
        let character = self.current().unwrap();
        if let Some(interpolation) = self.interpolations.last_mut() {
            match character {
                ')' if interpolation.parentheses == 0 => return self.string_chunk(false),
                ')' => interpolation.parentheses -= 1,
                '(' => interpolation.parentheses += 1,
                _ => {}
            }
        }
        if is_whitespace(character) {
            self.consume_while(is_whitespace);
            return TokenKind::Whitespace;
        }
        if character == '%' {
            self.line_comment();
            return TokenKind::LineComment;
        }
        if self.remaining().starts_with("/*") {
            return self.block_comment();
        }
        if character == '\'' {
            return self.quoted_identifier();
        }
        if character == '`' {
            return self.infix_identifier();
        }
        if character == '"' {
            return self.string_chunk(true);
        }
        if character == '$' {
            return self.type_inst_variable();
        }
        if character.is_ascii_alphabetic()
            || (character == '_' && self.following().is_some_and(|c| c.is_ascii_alphabetic()))
        {
            let start = self.position;
            self.consume_while(is_identifier_continue);
            return keyword(&self.source[start..self.position]).unwrap_or(TokenKind::Identifier);
        }
        if character.is_ascii_digit() {
            return self.number();
        }
        for &(spelling, kind) in SYMBOLS {
            if self.remaining().starts_with(spelling) {
                self.position += spelling.len();
                return kind;
            }
        }
        let start = self.position;
        self.advance();
        self.diagnose(
            start..self.position,
            format!("unrecognized character {character:?}"),
        );
        TokenKind::Error
    }

    fn current(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn following(&self) -> Option<char> {
        self.remaining().chars().nth(1)
    }

    fn remaining(&self) -> &str {
        &self.source[self.position..]
    }

    fn advance(&mut self) {
        if let Some(character) = self.current() {
            self.position += character.len_utf8();
        }
    }

    fn consume_while(&mut self, predicate: impl Fn(char) -> bool) {
        while self.current().is_some_and(&predicate) {
            self.advance();
        }
    }

    fn diagnose(&mut self, range: Range<usize>, message: impl Into<String>) {
        self.diagnostics.push(Diagnostic {
            range,
            message: message.into(),
        });
    }

    fn line_comment(&mut self) {
        self.consume_while(|c| !matches!(c, '\r' | '\n'));
    }

    fn block_comment(&mut self) -> TokenKind {
        let start = self.position;
        self.position += 2;
        if let Some(end) = self.remaining().find("*/") {
            self.position += end + 2;
            TokenKind::BlockComment
        } else {
            self.position = self.source.len();
            self.diagnose(start..self.position, "unterminated block comment");
            TokenKind::Error
        }
    }

    fn quoted_identifier(&mut self) -> TokenKind {
        let start = self.position;
        self.advance();
        let contents = self.position;
        self.consume_while(|c| !matches!(c, '\'' | '\r' | '\n' | '\0'));
        if self.current() != Some('\'') {
            self.diagnose(start..self.position, "unterminated quoted identifier");
            return TokenKind::Error;
        }
        let empty = self.position == contents;
        self.advance();
        if empty {
            self.diagnose(start..self.position, "quoted identifiers cannot be empty");
            TokenKind::Error
        } else {
            TokenKind::QuotedIdentifier
        }
    }

    fn infix_identifier(&mut self) -> TokenKind {
        let start = self.position;
        self.advance();
        let contents = self.position;
        let valid = if self.current() == Some('\'') {
            self.quoted_identifier() == TokenKind::QuotedIdentifier
        } else {
            self.consume_while(|c| !matches!(c, '`' | '\r' | '\n' | '\0'));
            let name = &self.source[contents..self.position];
            is_identifier(name) && keyword(name).is_none()
        };
        if self.current() == Some('`') {
            self.advance();
            if valid {
                return TokenKind::InfixIdentifier;
            }
            self.diagnose(
                start..self.position,
                "expected an identifier between backticks",
            );
        } else {
            self.diagnose(start..self.position, "unterminated backquoted identifier");
        }
        TokenKind::Error
    }

    fn type_inst_variable(&mut self) -> TokenKind {
        let start = self.position;
        self.advance();
        if !self
            .current()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '$')
        {
            self.diagnose(
                start..self.position,
                "expected a letter or '$' after the type-inst variable prefix",
            );
            return TokenKind::Error;
        }
        self.advance();
        self.consume_while(is_identifier_continue);
        TokenKind::TypeInstVariable
    }

    fn number(&mut self) -> TokenKind {
        if self.remaining().starts_with("0x") || self.remaining().starts_with("0X") {
            return self.hex_number();
        }
        if self.remaining().starts_with("0o") {
            let start = self.position;
            self.position += 2;
            let digits = self.position;
            self.consume_while(|c| c.is_ascii_digit());
            let numeral = &self.source[digits..self.position];
            if numeral.is_empty() || numeral.contains(['8', '9']) {
                self.diagnose(
                    start..self.position,
                    "expected octal digits (0 through 7) after '0o'",
                );
                return TokenKind::Error;
            }
            return TokenKind::IntegerLiteral;
        }
        self.consume_while(|c| c.is_ascii_digit());
        // After a field-access dot, consecutive selectors such as .1.2 are
        // integer tokens separated by dots, rather than a float literal.
        if self.after_field_dot {
            return TokenKind::IntegerLiteral;
        }
        let mut kind = TokenKind::IntegerLiteral;
        // A decimal point needs a following digit; otherwise it belongs to a
        // range or field access. Signs belong to separate operator tokens.
        if self.current() == Some('.') && self.following().is_some_and(|c| c.is_ascii_digit()) {
            self.advance();
            self.consume_while(|c| c.is_ascii_digit());
            kind = TokenKind::FloatLiteral;
        }
        if matches!(self.current(), Some('e' | 'E')) {
            if !self.exponent() {
                return TokenKind::Error;
            }
            kind = TokenKind::FloatLiteral;
        }
        kind
    }

    fn hex_number(&mut self) -> TokenKind {
        let start = self.position;
        let uppercase_prefix = self.remaining().starts_with("0X");
        self.position += 2;
        let digits = self.position;
        self.consume_while(|c| c.is_ascii_hexdigit());
        let integer_end = self.position;
        if self.after_field_dot && integer_end > digits && !uppercase_prefix {
            return TokenKind::IntegerLiteral;
        }
        let mut has_digits = self.position > digits;
        let has_dot = self.current() == Some('.') && !self.remaining().starts_with("..");
        if has_dot {
            self.advance();
            let fraction = self.position;
            self.consume_while(|c| c.is_ascii_hexdigit());
            has_digits |= self.position > fraction;
        }
        if !has_digits {
            self.diagnose(
                start..self.position,
                "expected hexadecimal digits after the prefix",
            );
            return TokenKind::Error;
        }
        if matches!(self.current(), Some('p' | 'P')) {
            return if self.exponent() {
                TokenKind::FloatLiteral
            } else {
                TokenKind::Error
            };
        }
        // Without a binary exponent, a following dot belongs to field access
        // after the integer. Uppercase X is permitted only for hex floats.
        if uppercase_prefix || integer_end == digits {
            self.diagnose(
                start..self.position,
                "hexadecimal floats require a 'p' exponent",
            );
            return TokenKind::Error;
        }
        self.position = integer_end;
        TokenKind::IntegerLiteral
    }

    fn exponent(&mut self) -> bool {
        let start = self.position;
        self.advance();
        if matches!(self.current(), Some('+' | '-')) {
            self.advance();
        }
        let digits = self.position;
        self.consume_while(|c| c.is_ascii_digit());
        if digits == self.position {
            self.diagnose(
                start..self.position,
                "expected decimal digits in the exponent",
            );
            false
        } else {
            true
        }
    }

    fn string_chunk(&mut self, first: bool) -> TokenKind {
        let start = self.position;
        let diagnostics_before = self.diagnostics.len();
        self.advance();
        while let Some(character) = self.current() {
            match character {
                '"' => {
                    self.advance();
                    if !first {
                        self.interpolations.pop();
                    }
                    return if !first {
                        TokenKind::StringTail
                    } else if self.diagnostics.len() > diagnostics_before {
                        TokenKind::Error
                    } else {
                        TokenKind::StringLiteral
                    };
                }
                '\r' | '\n' => break,
                '\\' if self.following() == Some('(') => {
                    self.position += 2;
                    if first {
                        self.interpolations.push(Interpolation {
                            start,
                            parentheses: 0,
                        });
                    }
                    return if first {
                        TokenKind::StringHead
                    } else {
                        TokenKind::StringMiddle
                    };
                }
                '\\' => self.string_escape(),
                _ => self.advance(),
            }
        }
        if !first {
            self.interpolations.pop();
        }
        self.diagnose(start..self.position, "unterminated string literal");
        TokenKind::Error
    }

    fn string_escape(&mut self) {
        let start = self.position;
        self.advance();
        match self.current() {
            Some('n' | 't' | '"' | '\\') => self.advance(),
            Some('0'..='7') => {
                for _ in 0..3 {
                    if !matches!(self.current(), Some('0'..='7')) {
                        break;
                    }
                    self.advance();
                }
            }
            Some('x') => {
                self.advance();
                let digits = self.position;
                for _ in 0..2 {
                    if !self.current().is_some_and(|c| c.is_ascii_hexdigit()) {
                        break;
                    }
                    self.advance();
                }
                if digits == self.position {
                    self.diagnose(
                        start..self.position,
                        "expected one or two hexadecimal escape digits",
                    );
                }
            }
            Some('\r' | '\n') | None => {
                self.diagnose(start..self.position, "incomplete string escape");
            }
            Some(_) => {
                self.advance();
                self.diagnose(start..self.position, "invalid string escape");
            }
        }
    }
}

struct Interpolation {
    start: usize,
    parentheses: usize,
}

fn is_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn is_identifier(name: &str) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    name.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && name.chars().all(is_identifier_continue)
}

// MiniZinc 2.10.1, Syntax Overview and Operators:
// https://docs.minizinc.dev/en/2.10.1/spec.html#syntax-overview
fn keyword(name: &str) -> Option<TokenKind> {
    use TokenKind::*;
    Some(match name {
        "ann" => Ann,
        "annotation" => Annotation,
        "any" => Any,
        "array" => Array,
        "bool" => Bool,
        "case" => Case,
        "constraint" => Constraint,
        "default" => Default,
        "diff" => Diff,
        "div" => Div,
        "else" => Else,
        "elseif" => ElseIf,
        "endif" => EndIf,
        "enum" => Enum,
        "false" => False,
        "float" => Float,
        "function" => Function,
        "if" => If,
        "in" => In,
        "include" => Include,
        "infinity" => Infinity,
        "int" => Int,
        "intersect" => Intersect,
        "let" => Let,
        "list" => List,
        "maximize" => Maximize,
        "minimize" => Minimize,
        "mod" => Mod,
        "not" => Not,
        "of" => Of,
        "opt" => Opt,
        "output" => Output,
        "par" => Par,
        "predicate" => Predicate,
        "record" => Record,
        "satisfy" => Satisfy,
        "set" => Set,
        "solve" => Solve,
        "string" => String,
        "subset" => Subset,
        "superset" => Superset,
        "symdiff" => SymDiff,
        "test" => Test,
        "then" => Then,
        "true" => True,
        "tuple" => Tuple,
        "type" => Type,
        "union" => Union,
        "var" => Var,
        "variant_record" => VariantRecord,
        "where" => Where,
        "xor" => Xor,
        _ => return None,
    })
}

// Longest spellings precede their prefixes. Preserve distinct '=' and '=='
// tokens even though their expression semantics coincide.
const SYMBOLS: &[(&str, TokenKind)] = &[
    ("<..<", TokenKind::RangeExclusive),
    ("~div", TokenKind::WeakDiv),
    ("<->", TokenKind::Equivalence),
    ("<..", TokenKind::RangeExclusiveStart),
    ("..<", TokenKind::RangeExclusiveEnd),
    ("~!=", TokenKind::WeakNotEqual),
    ("..", TokenKind::RangeInclusive),
    ("::", TokenKind::AnnotationMarker),
    ("->", TokenKind::Implies),
    ("<-", TokenKind::ReverseImplies),
    ("\\/", TokenKind::Or),
    ("/\\", TokenKind::And),
    ("<=", TokenKind::LessEqual),
    (">=", TokenKind::GreaterEqual),
    ("==", TokenKind::DoubleEqual),
    ("!=", TokenKind::NotEqual),
    ("~=", TokenKind::WeakEqual),
    ("++", TokenKind::Concat),
    ("~+", TokenKind::WeakPlus),
    ("~-", TokenKind::WeakMinus),
    ("~*", TokenKind::WeakStar),
    ("~/", TokenKind::WeakSlash),
    ("[|", TokenKind::MatrixStart),
    ("|]", TokenKind::MatrixEnd),
    ("<>", TokenKind::Absent),
    ("↔", TokenKind::Equivalence),
    ("→", TokenKind::Implies),
    ("←", TokenKind::ReverseImplies),
    ("¬", TokenKind::Not),
    ("∨", TokenKind::Or),
    ("∧", TokenKind::And),
    ("≠", TokenKind::NotEqual),
    ("≤", TokenKind::LessEqual),
    ("≥", TokenKind::GreaterEqual),
    ("∈", TokenKind::In),
    ("⊆", TokenKind::Subset),
    ("⊇", TokenKind::Superset),
    ("∪", TokenKind::Union),
    ("∩", TokenKind::Intersect),
    ("⁻¹", TokenKind::Inverse),
    ("(", TokenKind::LeftParen),
    (")", TokenKind::RightParen),
    ("[", TokenKind::LeftBracket),
    ("]", TokenKind::RightBracket),
    ("{", TokenKind::LeftBrace),
    ("}", TokenKind::RightBrace),
    (",", TokenKind::Comma),
    (":", TokenKind::Colon),
    (";", TokenKind::Semicolon),
    (".", TokenKind::Dot),
    ("|", TokenKind::Pipe),
    ("_", TokenKind::Anonymous),
    ("<", TokenKind::Less),
    (">", TokenKind::Greater),
    ("=", TokenKind::Equal),
    ("+", TokenKind::Plus),
    ("-", TokenKind::Minus),
    ("*", TokenKind::Star),
    ("/", TokenKind::Slash),
    ("^", TokenKind::Power),
];
