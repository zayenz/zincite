//! Source-preserving syntax for MiniZinc 2.10.1.
//!
//! Lexing retains trivia and erroneous input. It does not check model or data
//! grammar, resolve names, or evaluate literals. Interpolated strings retain
//! literal chunks and embedded expression tokens in the same source buffer.
//! Parsing supports scalar and ordinary collection declarations, domains,
//! indexed and matrix literals, dependent indices, cardinalities, array access,
//! operator expressions, calls, interpolated strings, annotations, constraints
//! with optional string labels, comprehensions, generator calls, conditional and let
//! expressions, callable/annotation declarations with generic types and parameter
//! defaults, enums and constructors, type aliases and type-inst concatenation,
//! includes, output items and all solve modes.
//! Tuple/record types and literals and chained field access share the CST.
//! Data mode accepts only top-level assignments. Reserved variant_record/case
//! syntax remains unsupported.

use std::ops::Range;
use std::path::Path;

pub mod inputs;
mod lexer;
mod parser;

/// Parse the supported model subset, retaining all tokens even after an error.
///
/// This checks syntax only: it does not resolve names, require a solve item, or
/// check types. Unsupported syntax produces diagnostics rather than opaque items.
pub fn parse(source: impl Into<String>) -> ParsedFile {
    parse_with_mode(source, FileMode::Model)
}

/// The syntax permitted at the top level of a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMode {
    Model,
    Data,
}

impl FileMode {
    /// `.dzn` selects data syntax; other paths use model syntax.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        if path.as_ref().extension().is_some_and(|ext| ext == "dzn") {
            Self::Data
        } else {
            Self::Model
        }
    }
}

/// Parse model syntax or assignment-only data syntax without resolving names.
/// Calls in data expressions are retained: determining whether an operation is
/// user-defined requires the model and is outside syntax checking.
pub fn parse_with_mode(source: impl Into<String>, mode: FileMode) -> ParsedFile {
    let mut lexed = lex(source);
    let (tree, diagnostics) = parser::parse(&lexed.tokens, &lexed.source, mode);
    lexed.diagnostics.extend(diagnostics);
    ParsedFile { lexed, tree }
}

/// Owned source, its lossless tree, and lexical/parser diagnostics.
#[derive(Debug)]
pub struct ParsedFile {
    lexed: LexedSource,
    tree: SyntaxNode,
}

impl ParsedFile {
    pub fn source(&self) -> &str {
        self.lexed.source()
    }

    pub fn tokens(&self) -> &[Token] {
        self.lexed.tokens()
    }

    pub fn tree(&self) -> &SyntaxNode {
        &self.tree
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        self.lexed.diagnostics()
    }
}

/// A node's children retain source order. Each token occurs in exactly one leaf.
#[derive(Debug)]
pub struct SyntaxNode {
    kind: NodeKind,
    range: Range<usize>,
    children: Vec<SyntaxElement>,
}

impl SyntaxNode {
    pub fn kind(&self) -> NodeKind {
        self.kind
    }

    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn children(&self) -> &[SyntaxElement] {
        &self.children
    }

    /// Borrow the direct child nodes in source order, excluding token leaves.
    pub fn child_nodes(&self) -> impl Iterator<Item = &SyntaxNode> {
        self.children.iter().filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node),
            SyntaxElement::Token(_) => None,
        })
    }
}

#[derive(Debug)]
pub enum SyntaxElement {
    Node(SyntaxNode),
    /// Index into the parse result's retained token buffer.
    Token(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Root,
    /// Direct name token, annotations, and an optional EnumDefinition child.
    EnumDeclaration,
    /// Direct name token, annotations, and the target type child.
    TypeAlias,
    /// Ordered case lists and constructors, separated by written `++` tokens.
    EnumDefinition,
    /// Braces containing EnumCase children in written order.
    EnumCases,
    /// The direct identifier token declares an enum member.
    EnumCase,
    /// A direct declaration name (or anonymous `_`) and a parenthesized expression.
    EnumConstructor,
    /// Left type and right base type, separated by a written `++` token.
    TypeInstConcatenation,
    /// The first child node is the type; later nodes are annotations/initializer.
    Declaration,
    /// Return type child, direct name token, and optional parameter list,
    /// annotations and body expression.
    FunctionDeclaration,
    /// Direct name token, optional parameter list, annotations and body.
    PredicateDeclaration,
    /// Direct name token, optional parameter list, annotations and body.
    TestDeclaration,
    /// Direct name token, optional parameter list and body; no annotations.
    AnnotationDeclaration,
    /// Parentheses and ordered Parameter children; omitted lists have no node.
    ParameterList,
    /// The type child, optional direct name token, ordered Annotation children,
    /// and optional default expression child. Unnamed parameters contain only a
    /// type; annotations after a default belong to that expression.
    Parameter,
    /// Generic variable spelling, with any/var/par/opt qualifiers retained.
    TypeInstVariable,
    ScalarType,
    /// Ordered type children within written tuple parentheses.
    TupleType,
    /// Ordered RecordField children within written record parentheses.
    RecordType,
    /// Type child and direct declaration name token with its precise range.
    RecordField,
    /// Parentheses and ordered value children; the first comma is required.
    TupleLiteral,
    /// Ordered RecordLiteralField children; the first comma is required.
    RecordLiteral,
    /// Direct field name token and value child; names label values, not declarations.
    RecordLiteralField,
    /// Subject child followed by a dot and direct reference name or integer token.
    FieldAccessExpression,
    /// Contains one domain expression, with any qualifiers retained as tokens.
    DomainType,
    /// Contains an optional cardinality followed by the element type.
    /// Qualifiers and `set of` remain tokens.
    SetType,
    /// Parentheses containing the written cardinality expression.
    SetCardinality,
    /// Child nodes are index types or bindings followed by the element type.
    ArrayType,
    /// Direct tokens retain the binding name and `in`; the child is its index type.
    ArrayIndexBinding,
    /// Contains the element type.
    ListType,
    Assignment,
    Constraint,
    /// Contains the written string literal path; no include resolution is performed.
    Include,
    /// Optional direct Annotation child followed by the output expression.
    Output,
    /// A satisfy item, with direct Annotation children and no objective.
    Solve,
    /// Direct Annotation children followed by the minimization objective expression.
    SolveMinimize,
    /// Direct Annotation children followed by the maximization objective expression.
    SolveMaximize,
    /// A literal or identifier atom, including anonymous and absent values.
    Expression,
    /// Exact string chunks alternate with embedded expression child nodes.
    InterpolatedString,
    UnaryExpression,
    BinaryExpression,
    ParenthesizedExpression,
    CallExpression,
    /// Written call name/header tokens, an ordered generator list, then a
    /// parenthesized body expression.
    GeneratorCallExpression,
    /// Ordered if/elseif branches, an optional else branch, and the endif token.
    ConditionalExpression,
    /// The keyword, condition expression, then token, and body expression.
    ConditionalBranch,
    /// The else token and its body expression.
    ElseBranch,
    /// The let token, local block, in token, and greedy body expression.
    LetExpression,
    /// Ordered Declaration/Constraint children with written separators. Local
    /// declarations expose names and their ranges as direct token leaves, just
    /// like top-level declarations; the first child node is their type.
    LetBlock,
    /// Child nodes are the head expression and ordered generator list.
    SetComprehension,
    ArrayComprehension,
    /// Child nodes are an indexed entry (key/value) and ordered generator list.
    IndexedArrayComprehension,
    /// Child nodes are generators in written order; commas remain token leaves.
    GeneratorList,
    /// Direct tokens retain binding names and `in` or `=`. Child nodes are the
    /// source/value expression followed by an optional attached WhereFilter.
    Generator,
    /// The written `where` token followed by its expression child.
    WhereFilter,
    /// Child nodes are the entries; empty literals have no child nodes.
    SetLiteral,
    /// Child nodes are bare expressions or indexed entries, in written order.
    ArrayLiteral,
    /// Child nodes are the written key and value, separated by a colon token.
    IndexedArrayEntry,
    /// Parenthesized key components; only supported in indexed-entry keys.
    IndexTuple,
    /// Contains an optional column header and rows, with written pipe delimiters.
    MatrixLiteral,
    /// Child nodes are column indices, each followed by a colon token.
    MatrixColumnIndices,
    /// Child nodes are cells, preceded by an index child when a colon is present.
    /// Empty written rows have a zero-width range at their pipe delimiter.
    MatrixRow,
    /// Child nodes are the subject followed by one or more index expressions.
    ArrayAccessExpression,
    NamedArgument,
    AnnotatedExpression,
    /// An annotation marker and its atom; distinct from the annotated subject.
    Annotation,
    /// Children contain the written bounds and marker; either bound may be omitted.
    RangeExpression,
    Error,
}

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

/// A lexical/parser error or an explicitly unsupported syntax form.
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
    /// Opening quote, literal contents and the first written `\(` delimiter.
    StringHead,
    /// Written `)`, literal contents and the next written `\(` delimiter.
    StringMiddle,
    /// Written `)`, remaining literal contents and closing quote.
    StringTail,
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
