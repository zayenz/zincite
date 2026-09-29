//! Source-preserving tokenization for MiniZinc 2.10.1.
//!
//! Lexing retains trivia and erroneous input. It does not check model or data
//! grammar, resolve names, or evaluate literals. String interpolation is retained
//! as a single unsupported token until expression tokenization is implemented.

use std::ops::Range;

mod lexer;

/// Tokenize an owned UTF-8 source, retaining every byte in source order.
///
/// Tokens have nonempty, contiguous, half-open byte ranges at UTF-8 boundaries.
/// Errors produce diagnostics and retained tokens rather than stopping lexing.
pub fn lex(source: impl Into<String>) -> LexedSource {
    let source = source.into();
    let (tokens, diagnostics) = lexer::scan(&source);
    LexedSource {
        source,
        tokens,
        diagnostics,
    }
}

/// Source text and its tokens. Read-only access keeps their ranges valid.
#[derive(Debug)]
pub struct LexedSource {
    source: String,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

impl LexedSource {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

/// A source spelling is available by slicing the source with this byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub range: Range<usize>,
}

/// A lexical error or an explicitly unsupported lexical form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: Range<usize>,
    pub message: String,
}

/// Lexical distinctions for a parser. Unicode operators share their ASCII
/// equivalent's kind; spelling is always retained in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Whitespace,
    LineComment,
    BlockComment,
    Identifier,
    QuotedIdentifier,
    InfixIdentifier,
    TypeInstVariable,
    IntegerLiteral,
    FloatLiteral,
    StringLiteral,
    UnsupportedString,
    Error,

    Ann,
    Annotation,
    Any,
    Array,
    Bool,
    Case,
    Constraint,
    Default,
    Diff,
    Div,
    Else,
    ElseIf,
    EndIf,
    Enum,
    False,
    Float,
    Function,
    If,
    In,
    Include,
    Infinity,
    Int,
    Intersect,
    Let,
    List,
    Maximize,
    Minimize,
    Mod,
    Not,
    Of,
    Opt,
    Output,
    Par,
    Predicate,
    Record,
    Satisfy,
    Set,
    Solve,
    String,
    Subset,
    Superset,
    SymDiff,
    Test,
    Then,
    True,
    Tuple,
    Type,
    Union,
    Var,
    VariantRecord,
    Where,
    Xor,

    LeftParen,
    RightParen,
    LeftBracket,
    RightBracket,
    LeftBrace,
    RightBrace,
    MatrixStart,
    MatrixEnd,
    Comma,
    Colon,
    Semicolon,
    Dot,
    Pipe,
    Anonymous,
    Absent,
    AnnotationMarker,
    Equivalence,
    Implies,
    ReverseImplies,
    Or,
    And,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    Equal,
    DoubleEqual,
    NotEqual,
    WeakEqual,
    WeakNotEqual,
    RangeInclusive,
    RangeExclusiveStart,
    RangeExclusiveEnd,
    RangeExclusive,
    Plus,
    Minus,
    Star,
    Slash,
    Power,
    Concat,
    WeakPlus,
    WeakMinus,
    WeakStar,
    WeakSlash,
    WeakDiv,
    /// The Unicode inverse spelling `⁻¹`, equivalent to the sequence `^-1`.
    Inverse,
}
