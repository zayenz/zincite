//! Formatting for Zincite's supported MiniZinc model syntax.

use std::num::NonZeroUsize;

use zincite_syntax::{Diagnostic, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

mod directives;
mod includes;
mod layout;
pub use directives::protected_ranges;

/// How indentation columns are written. Tabs use spaces for a partial tab stop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IndentStyle {
    #[default]
    Space,
    Tab,
}

/// Line endings for editable layout; protected text keeps its original bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn text(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }
}

/// Layout options for ordinary MiniZinc syntax.
///
/// Width counts Unicode scalar values, with tabs advancing to the next tab stop.
/// `None` disables optional width breaks; explicit lists and block layouts still
/// expand. Nonzero sizes prevent ambiguous indentation and tab stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatOptions {
    pub indent_style: IndentStyle,
    pub indent_size: NonZeroUsize,
    pub tab_width: NonZeroUsize,
    pub max_line_length: Option<NonZeroUsize>,
    pub line_ending: LineEnding,
    pub insert_final_newline: bool,
    pub trim_trailing_whitespace: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_style: IndentStyle::Space,
            indent_size: NonZeroUsize::new(4).unwrap(),
            tab_width: NonZeroUsize::new(4).unwrap(),
            max_line_length: NonZeroUsize::new(120),
            line_ending: LineEnding::Lf,
            insert_final_newline: true,
            trim_trailing_whitespace: true,
        }
    }
}

/// Format a complete parse result, or return syntax/directive diagnostics without output.
///
/// Identifiers, literals, operators and comments retain their spelling. Editable
/// layout uses LF and a final newline; protected spans retain their original bytes.
/// No type or semantic checks are performed.
pub fn format(parsed: &ParsedFile) -> Result<String, Vec<Diagnostic>> {
    format_with_options(parsed, &FormatOptions::default())
}

/// Format with the supplied options, or return syntax/directive diagnostics.
pub fn format_with_options(
    parsed: &ParsedFile,
    options: &FormatOptions,
) -> Result<String, Vec<Diagnostic>> {
    let protected = protected_ranges(parsed)?;
    let mut formatter = Formatter {
        parsed,
        output: String::new(),
        pending_breaks: 0,
        line_comment: false,
        last_was_comment: false,
        between_items: false,
        indent: 0,
        options: *options,
        measuring: false,
        measuring_prefix: false,
        break_before_code: false,
        suffix_columns: 0,
    };
    let mut ranges = protected.iter().peekable();
    let mut protected_output = Vec::new();
    let mut groups = includes::include_groups(parsed, &protected)
        .into_iter()
        .peekable();
    let children = parsed.tree().children();
    let mut position = 0;
    while position < children.len() {
        if groups
            .peek()
            .is_some_and(|group| group.children.start == position)
        {
            let group = groups.next().unwrap();
            formatter.include_group(&group);
            position = group.children.end;
            continue;
        }
        let child = &children[position];
        position += 1;
        let start = match child {
            SyntaxElement::Node(item) => item.range().start,
            SyntaxElement::Token(index) => parsed.tokens()[*index].range.start,
        };
        while ranges.peek().is_some_and(|range| range.end <= start) {
            ranges.next();
        }
        if let Some(range) = ranges.peek().filter(|range| range.contains(&start)) {
            if range.start == start {
                let output_start = formatter.output.len();
                formatter
                    .output
                    .push_str(&parsed.source()[(*range).clone()]);
                protected_output.push(output_start..formatter.output.len());
                formatter.pending_breaks = 0;
                formatter.line_comment = false;
                formatter.last_was_comment = false;
                formatter.between_items = false;
            }
            continue;
        }
        match child {
            SyntaxElement::Node(item) => formatter.item(item),
            SyntaxElement::Token(index) => formatter.token(*index),
        }
    }
    if !formatter.output.is_empty()
        && options.insert_final_newline
        && !protected
            .last()
            .is_some_and(|range| range.end == parsed.source().len())
    {
        formatter.newlines(1);
    }
    layout::apply_layout(formatter.output, protected_output, options)
}

struct Formatter<'a> {
    parsed: &'a ParsedFile,
    output: String,
    pending_breaks: usize,
    line_comment: bool,
    last_was_comment: bool,
    between_items: bool,
    indent: usize,
    options: FormatOptions,
    measuring: bool,
    measuring_prefix: bool,
    break_before_code: bool,
    suffix_columns: usize,
}

struct MatrixCell {
    text: String,
    slot: String,
    pending_breaks: usize,
    line_comment: bool,
    last_was_comment: bool,
}

struct MatrixLayout {
    rows: Vec<Vec<MatrixCell>>,
    starts: Vec<usize>,
    breaks: Vec<bool>,
}

impl Formatter<'_> {
    fn include_group(&mut self, group: &includes::IncludeGroup) {
        for entry in &group.entries {
            self.newlines(self.pending_breaks.max(1));
            self.pending_breaks = 0;
            self.between_items = false;
            for child in &self.parsed.tree().children()[entry.clone()] {
                match child {
                    SyntaxElement::Node(item) => self.item(item),
                    SyntaxElement::Token(index) => self.token(*index),
                }
            }
        }
    }

    fn item(&mut self, item: &SyntaxNode) {
        if self.between_items || self.pending_breaks > 0 || self.line_comment {
            self.newlines(self.pending_breaks.max(1));
        }
        self.pending_breaks = 0;
        self.between_items = false;
        let has_semicolon = item.children().iter().any(|child| {
            matches!(child, SyntaxElement::Token(index)
                if self.parsed.tokens()[*index].kind == TokenKind::Semicolon)
        });
        self.suffix_columns = usize::from(!has_semicolon);
        self.item_contents(item);
        self.suffix_columns = 0;
        if !has_semicolon {
            self.code(TokenKind::Semicolon, ";");
        }
        self.between_items = true;
    }

    fn item_contents(&mut self, item: &SyntaxNode) {
        let labelled = item.kind() == NodeKind::Constraint
            && item.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                    if self.parsed.tokens()[*index].kind == TokenKind::AnnotationMarker)
            });
        let mut label_pending = labelled;
        let mut continuation = false;
        let mut previous = None;
        for (position, child) in item.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(expression) => {
                    if labelled && !label_pending {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    let space = expression.kind() != NodeKind::ParameterList;
                    let suffix = if previous == Some(TokenKind::Function) {
                        // The name can continue after the return type's colon.
                        1
                    } else {
                        self.following_columns(item.children(), position)
                    };
                    if !labelled
                        && (expression.kind() == NodeKind::Annotation
                            || matches!(
                                previous,
                                Some(
                                    TokenKind::Equal
                                        | TokenKind::DoubleEqual
                                        | TokenKind::Minimize
                                        | TokenKind::Maximize
                                        | TokenKind::Output
                                        | TokenKind::Constraint
                                        | TokenKind::Include
                                        | TokenKind::Function
                                )
                            ))
                        && self.prefix_exceeds_width(expression, space, suffix)
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.newlines(self.pending_breaks.max(1));
                    }
                    if expression.kind() == NodeKind::ParameterList
                        && self
                            .options
                            .max_line_length
                            .is_some_and(|width| self.current_columns() > width.get())
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.expression_before(expression, space, suffix);
                    label_pending = false;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if matches!(kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)
                        && previous == Some(TokenKind::Colon)
                        && self.token_exceeds_width(
                            *index,
                            true,
                            self.following_columns(item.children(), position),
                        )
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.break_before_code = true;
                    }
                    self.token(*index);
                    if kind == TokenKind::StringLiteral {
                        label_pending = false;
                    }
                    if !is_trivia(kind) {
                        previous = Some(kind);
                    }
                }
            }
        }
        if continuation {
            self.indent -= 1;
        }
    }

    fn expression(&mut self, node: &SyntaxNode, leading_space: bool) {
        self.expression_before(node, leading_space, 0);
    }

    fn expression_before(&mut self, node: &SyntaxNode, leading_space: bool, suffix: usize) {
        let previous = self.suffix_columns;
        self.suffix_columns = suffix;
        self.expression_contents(node, leading_space);
        self.suffix_columns = previous;
    }

    fn expression_contents(&mut self, node: &SyntaxNode, leading_space: bool) {
        if is_binary(node.kind()) {
            let expanded = self.measuring_prefix
                || self.node_exceeds_width(node, leading_space, self.suffix_columns);
            self.binary_expression(node, leading_space, expanded, self.indent + 1);
            return;
        }
        if node.kind() == NodeKind::InterpolatedString {
            self.interpolated_string(node, leading_space);
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::ConditionalExpression | NodeKind::LetExpression
        ) {
            self.control_expression(node, leading_space);
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::GeneratorCallExpression
                | NodeKind::SetComprehension
                | NodeKind::ArrayComprehension
                | NodeKind::IndexedArrayComprehension
        ) {
            self.generator_expression(node, leading_space);
            return;
        }
        if node.kind() == NodeKind::MatrixLiteral {
            self.matrix(node, leading_space);
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::ParenthesizedExpression
                | NodeKind::SetCardinality
                | NodeKind::EnumConstructor
        ) {
            self.parenthesized(node, leading_space);
            return;
        }
        let delimiters = match node.kind() {
            NodeKind::CallExpression | NodeKind::ParameterList => {
                Some((TokenKind::LeftParen, TokenKind::RightParen))
            }
            NodeKind::IndexTuple
            | NodeKind::TupleLiteral
            | NodeKind::RecordLiteral
            | NodeKind::TupleType
            | NodeKind::RecordType => Some((TokenKind::LeftParen, TokenKind::RightParen)),
            NodeKind::ArrayLiteral | NodeKind::ArrayAccessExpression | NodeKind::ArrayType => {
                Some((TokenKind::LeftBracket, TokenKind::RightBracket))
            }
            NodeKind::SetLiteral | NodeKind::EnumCases => {
                Some((TokenKind::LeftBrace, TokenKind::RightBrace))
            }
            _ => None,
        };
        if let Some((opening, closing)) = delimiters {
            self.comma_list(node, leading_space, opening, closing);
            return;
        }
        let mut first = true;
        let mut previous = None;
        let mut continuation = false;
        for (position, child) in node.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(child) => {
                    let space = if first {
                        leading_space
                    } else {
                        match node.kind() {
                            NodeKind::ParenthesizedExpression
                            | NodeKind::SetCardinality
                            | NodeKind::EnumConstructor => false,
                            NodeKind::SetType if child.kind() == NodeKind::SetCardinality => false,
                            NodeKind::UnaryExpression => {
                                previous == Some(TokenKind::Not)
                                    || child.kind() == NodeKind::UnaryExpression
                            }
                            _ => true,
                        }
                    };
                    let suffix = self.following_columns(node.children(), position);
                    if !first
                        && matches!(
                            node.kind(),
                            NodeKind::AnnotatedExpression
                                | NodeKind::NamedArgument
                                | NodeKind::IndexedArrayEntry
                                | NodeKind::RecordLiteralField
                                | NodeKind::ArrayIndexBinding
                                | NodeKind::WhereFilter
                                | NodeKind::SetType
                                | NodeKind::ListType
                        )
                        && self.prefix_exceeds_width(child, space, suffix)
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.expression_before(child, space, suffix);
                    first = false;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if is_trivia(kind) {
                        self.token(*index);
                        continue;
                    }
                    let space = if first {
                        leading_space
                    } else if node.kind() == NodeKind::FieldAccessExpression {
                        false
                    } else {
                        !matches!(
                            kind,
                            TokenKind::RightParen
                                | TokenKind::Comma
                                | TokenKind::Colon
                                | TokenKind::Inverse
                        )
                    };
                    if previous == Some(TokenKind::Colon)
                        && matches!(node.kind(), NodeKind::Parameter | NodeKind::RecordField)
                        && self.token_exceeds_width(
                            *index,
                            space,
                            self.following_columns(node.children(), position),
                        )
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.break_before_code = true;
                    }
                    self.token_with_space(*index, space);
                    first = false;
                    previous = Some(kind);
                }
            }
        }
        if continuation {
            self.indent -= 1;
        }
    }

    // Traverse the written binary tree without regrouping operands. All binary
    // children share one continuation column, irrespective of associativity.
    fn binary_expression(
        &mut self,
        node: &SyntaxNode,
        leading_space: bool,
        expanded: bool,
        continuation_indent: usize,
    ) {
        let initial_indent = self.indent;
        let mut first = true;
        let mut operator_seen = false;
        let mut before_operator = Vec::new();
        for (position, child) in node.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(child) => {
                    if operator_seen
                        && (expanded
                            || self.line_comment
                            || self.pending_breaks > 1
                            || self.has_leading_break(child))
                    {
                        self.indent = continuation_indent;
                        // Leading trivia can now belong to the right operand.
                        // Emit it before the break so end-of-line comments stay
                        // on the operator line on every formatting pass.
                        self.break_before_code = true;
                    }
                    let suffix = self.following_columns(node.children(), position);
                    if is_binary(child.kind()) {
                        let previous = self.suffix_columns;
                        self.suffix_columns = suffix;
                        self.binary_expression(
                            child,
                            first && leading_space || !first,
                            expanded,
                            continuation_indent,
                        );
                        self.suffix_columns = previous;
                    } else {
                        self.expression_before(child, first && leading_space || !first, suffix);
                    }
                    first = false;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if is_trivia(kind) && !operator_seen {
                        before_operator.push(*index);
                        continue;
                    }
                    self.token_with_space(*index, first && leading_space || !first);
                    if !is_trivia(kind) {
                        operator_seen = true;
                        first = false;
                        for index in before_operator.drain(..) {
                            self.token(index);
                        }
                    }
                }
            }
        }
        for index in before_operator {
            self.token(index);
        }
        self.indent = initial_indent;
    }

    fn parenthesized(&mut self, node: &SyntaxNode, leading_space: bool) {
        let expanded = self.measuring_prefix
            || self.node_exceeds_width(node, leading_space, self.suffix_columns);
        let mut first = true;
        for (position, child) in node.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(child) => {
                    if expanded {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    let suffix = if expanded {
                        0
                    } else {
                        self.following_columns(node.children(), position)
                    };
                    self.expression_before(child, false, suffix);
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if kind == TokenKind::RightParen && expanded {
                        self.indent -= 1;
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.token_with_space(*index, first && leading_space);
                    if kind == TokenKind::LeftParen && expanded {
                        self.indent += 1;
                    }
                    if !is_trivia(kind) {
                        first = false;
                    }
                }
            }
        }
    }

    fn has_leading_break(&self, node: &SyntaxNode) -> bool {
        for child in node.children() {
            match child {
                SyntaxElement::Node(node) => return self.has_leading_break(node),
                SyntaxElement::Token(index) => match self.parsed.tokens()[*index].kind {
                    TokenKind::LineComment => return true,
                    TokenKind::Whitespace => {
                        let text =
                            &self.parsed.source()[self.parsed.tokens()[*index].range.clone()];
                        if text.replace("\r\n", "\n").matches(['\r', '\n']).count() > 1 {
                            return true;
                        }
                    }
                    TokenKind::BlockComment => {}
                    _ => return false,
                },
            }
        }
        false
    }

    fn interpolated_string(&mut self, node: &SyntaxNode, leading_space: bool) {
        for child in node.children() {
            match child {
                SyntaxElement::Node(expression) => self.expression(expression, false),
                SyntaxElement::Token(index) => {
                    let token = &self.parsed.tokens()[*index];
                    match token.kind {
                        TokenKind::StringHead => self.token_with_space(*index, leading_space),
                        TokenKind::StringMiddle | TokenKind::StringTail => {
                            if self.line_comment || self.pending_breaks > 0 {
                                self.newlines(self.pending_breaks.max(1));
                                self.indent_line();
                            }
                            // Chunks contain delimiters and literal bytes. Resolve
                            // expression trivia before them without adding spacing
                            // inside the string or leaking comment state past it.
                            self.output
                                .push_str(&self.parsed.source()[token.range.clone()]);
                            self.pending_breaks = 0;
                            self.line_comment = false;
                            self.last_was_comment = false;
                        }
                        _ => self.token(*index),
                    }
                }
            }
        }
    }

    fn control_expression(&mut self, node: &SyntaxNode, leading_space: bool) {
        let mut first = true;
        for child in node.children() {
            match child {
                SyntaxElement::Node(branch)
                    if matches!(
                        branch.kind(),
                        NodeKind::ConditionalBranch | NodeKind::ElseBranch
                    ) =>
                {
                    if !first {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    let mut body = false;
                    for (position, part) in branch.children().iter().enumerate() {
                        match part {
                            SyntaxElement::Node(expression) => {
                                if body {
                                    self.newlines(self.pending_breaks.max(1));
                                }
                                let suffix = if body {
                                    0
                                } else {
                                    self.following_columns(branch.children(), position)
                                };
                                let continuation =
                                    !body && self.prefix_exceeds_width(expression, true, suffix);
                                if continuation {
                                    self.indent += 1;
                                    self.newlines(self.pending_breaks.max(1));
                                }
                                self.expression_before(expression, true, suffix);
                                if continuation {
                                    self.indent -= 1;
                                }
                            }
                            SyntaxElement::Token(index) => {
                                let kind = self.parsed.tokens()[*index].kind;
                                if kind == TokenKind::Then
                                    && self.token_exceeds_width(*index, true, 0)
                                {
                                    self.break_before_code = true;
                                }
                                self.token_with_space(*index, !first || leading_space);
                                if !is_trivia(kind) {
                                    first = false;
                                }
                                if matches!(kind, TokenKind::Then | TokenKind::Else) {
                                    body = true;
                                    self.indent += 1;
                                }
                            }
                        }
                    }
                    self.indent -= 1;
                }
                SyntaxElement::Node(block) if block.kind() == NodeKind::LetBlock => {
                    self.local_block(block);
                }
                SyntaxElement::Node(body) => {
                    self.indent += 1;
                    self.newlines(self.pending_breaks.max(1));
                    self.expression_before(body, true, self.suffix_columns);
                    self.indent -= 1;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if kind == TokenKind::EndIf {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.token_with_space(*index, !first || leading_space);
                    if !is_trivia(kind) {
                        first = false;
                    }
                }
            }
        }
    }

    fn local_block(&mut self, block: &SyntaxNode) {
        for child in block.children() {
            match child {
                SyntaxElement::Node(item) => {
                    self.newlines(self.pending_breaks.max(1));
                    let suffix = self.suffix_columns;
                    self.suffix_columns = 1;
                    self.item_contents(item);
                    self.suffix_columns = suffix;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if kind == TokenKind::RightBrace {
                        self.indent -= 1;
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.token_with_space(
                        *index,
                        !matches!(kind, TokenKind::Comma | TokenKind::Semicolon),
                    );
                    if kind == TokenKind::LeftBrace {
                        self.indent += 1;
                    }
                }
            }
        }
    }

    fn generator_expression(&mut self, node: &SyntaxNode, leading_space: bool) {
        let call = node.kind() == NodeKind::GeneratorCallExpression;
        let closing = match node.kind() {
            NodeKind::SetComprehension => TokenKind::RightBrace,
            NodeKind::GeneratorCallExpression => TokenKind::RightParen,
            _ => TokenKind::RightBracket,
        };
        let forall = call
            && node.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                    if matches!(self.parsed.tokens()[*index].kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier)
                    && matches!(&self.parsed.source()[self.parsed.tokens()[*index].range.clone()],
                        "forall" | "'forall'"))
            });
        let mut head_expanded = !call && self.has_line_break(node);
        let mut first = true;
        let mut header_expanded = false;
        for child in node.children() {
            match child {
                SyntaxElement::Node(list) if list.kind() == NodeKind::GeneratorList => {
                    header_expanded =
                        head_expanded || self.generator_header_expands(list, closing, !call);
                    if header_expanded && !head_expanded {
                        self.indent += 1;
                    }
                    self.generator_list(list, header_expanded, !call);
                }
                SyntaxElement::Node(body) if call => self.generator_body(body, forall),
                SyntaxElement::Node(head) => {
                    if !head_expanded && self.prefix_exceeds_width(head, false, 2) {
                        head_expanded = true;
                        self.indent += 1;
                    }
                    if head_expanded {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.expression_before(head, false, 2);
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if kind == closing {
                        if header_expanded || head_expanded {
                            self.indent -= 1;
                            self.newlines(self.pending_breaks.max(1));
                            self.pending_breaks = 0;
                        }
                        self.token_with_space(*index, false);
                    } else if is_trivia(kind) {
                        self.token(*index);
                    } else {
                        self.token_with_space(*index, first && leading_space || !first);
                        if head_expanded && first {
                            self.indent += 1;
                        }
                        first = false;
                    }
                }
            }
        }
    }

    fn generator_header_expands(
        &self,
        list: &SyntaxNode,
        closing: TokenKind,
        leading_space: bool,
    ) -> bool {
        if self.has_line_break(list)
            || list.child_nodes().any(|generator| {
                self.has_line_break(generator)
                    || generator
                        .child_nodes()
                        .filter(|child| child.kind() == NodeKind::WhereFilter)
                        .any(|filter| self.has_line_break(filter))
            })
        {
            return true;
        }
        if self.measuring_prefix {
            return true;
        }
        if self.measuring || self.options.max_line_length.is_none() {
            return false;
        }
        let mut preview = self.preview();
        preview.generator_list(list, false, leading_space);
        preview.code_with_space(
            match closing {
                TokenKind::RightBrace => "}",
                TokenKind::RightBracket => "]",
                _ => ")",
            },
            false,
        );
        preview.exceeds_width(
            self.current_columns(),
            if leading_space {
                self.suffix_columns
            } else {
                2
            },
        )
    }

    fn generator_list(&mut self, list: &SyntaxNode, expanded: bool, leading_space: bool) {
        let mut first = true;
        let mut previous_end = self.output.len();
        for (position, child) in list.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(generator) => {
                    let comment_before = self.last_was_comment
                        && !self.line_comment
                        && self.pending_breaks == 0
                        && self.output[previous_end..].contains('\n');
                    if expanded && !comment_before {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    let suffix = self.suffix_columns;
                    self.suffix_columns = if expanded {
                        1
                    } else {
                        self.following_columns(list.children(), position) + 1
                    };
                    self.generator(generator, expanded, !expanded && (leading_space || !first));
                    self.suffix_columns = suffix;
                    if expanded
                        && !list.children()[position + 1..].iter().any(|child| {
                            matches!(child, SyntaxElement::Token(index)
                                if self.parsed.tokens()[*index].kind == TokenKind::Comma)
                        })
                    {
                        self.code_with_space(",", false);
                    }
                    previous_end = self.output.len();
                    first = false;
                }
                SyntaxElement::Token(index) => self.token_with_space(*index, false),
            }
        }
    }

    fn generator(&mut self, generator: &SyntaxNode, expanded: bool, leading_space: bool) {
        let mut first = true;
        for (position, child) in generator.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(filter) if filter.kind() == NodeKind::WhereFilter => {
                    if expanded {
                        self.indent += 1;
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.expression_before(filter, true, self.suffix_columns);
                    if expanded {
                        self.indent -= 1;
                    }
                }
                SyntaxElement::Node(source) => {
                    let suffix = self.following_columns(generator.children(), position);
                    if self.prefix_exceeds_width(source, true, suffix) {
                        self.indent += 1;
                        self.break_before_code = true;
                        self.expression_before(source, true, suffix);
                        self.indent -= 1;
                    } else {
                        self.expression_before(source, true, suffix);
                    }
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    self.token_with_space(
                        *index,
                        if first {
                            leading_space
                        } else {
                            kind != TokenKind::Comma
                        },
                    );
                    if !is_trivia(kind) {
                        first = false;
                    }
                }
            }
        }
    }

    fn generator_body(&mut self, body: &SyntaxNode, forall: bool) {
        let expanded = forall
            || self.has_line_break(body)
            || self.measuring_prefix
            || self.node_exceeds_width(body, true, self.suffix_columns);
        if self.prefix_exceeds_width(body, true, self.suffix_columns) {
            self.newlines(self.pending_breaks.max(1));
        }
        let mut previous_end = self.output.len();
        for child in body.children() {
            match child {
                SyntaxElement::Node(expression) => {
                    let comment_before = self.last_was_comment
                        && !self.line_comment
                        && self.pending_breaks == 0
                        && self.output[previous_end..].contains('\n');
                    if expanded && !comment_before {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.expression_before(
                        expression,
                        false,
                        if expanded { 0 } else { 1 + self.suffix_columns },
                    );
                }
                SyntaxElement::Token(index) => match self.parsed.tokens()[*index].kind {
                    TokenKind::LeftParen => {
                        self.token_with_space(*index, true);
                        previous_end = self.output.len();
                        if expanded {
                            self.indent += 1;
                        }
                    }
                    TokenKind::RightParen => {
                        if expanded {
                            self.indent -= 1;
                            self.newlines(self.pending_breaks.max(1));
                            self.pending_breaks = 0;
                        }
                        self.token_with_space(*index, false);
                    }
                    _ => self.token(*index),
                },
            }
        }
    }

    fn has_line_break(&self, node: &SyntaxNode) -> bool {
        node.children().iter().any(|child| {
            matches!(child, SyntaxElement::Token(index)
                if self.parsed.tokens()[*index].kind == TokenKind::Whitespace
                && self.parsed.source()[self.parsed.tokens()[*index].range.clone()]
                    .contains(['\r', '\n']))
        })
    }

    // Measuring retains mandatory/explicit layouts but disables optional width
    // decisions, so a preview never recursively asks for another preview.
    fn preview(&self) -> Self {
        Self {
            parsed: self.parsed,
            output: self
                .output
                .rsplit_once('\n')
                .map_or_else(|| self.output.clone(), |(_, line)| format!("\n{line}")),
            pending_breaks: self.pending_breaks,
            line_comment: self.line_comment,
            last_was_comment: self.last_was_comment,
            between_items: self.between_items,
            indent: self.indent,
            options: self.options,
            measuring: true,
            measuring_prefix: false,
            break_before_code: self.break_before_code,
            suffix_columns: self.suffix_columns,
        }
    }

    fn columns(&self, text: &str) -> usize {
        self.columns_from(text, 0)
    }

    fn columns_from(&self, text: &str, start: usize) -> usize {
        let tab_width = self.options.tab_width.get();
        text.chars().fold(start, |column, character| {
            if character == '\t' {
                column + tab_width - column % tab_width
            } else {
                column + 1
            }
        })
    }

    fn current_columns(&self) -> usize {
        self.columns(self.output.rsplit('\n').next().unwrap_or_default())
    }

    fn exceeds_width(&self, prefix_columns: usize, suffix_columns: usize) -> bool {
        let Some(width) = self.options.max_line_length else {
            return false;
        };
        let mut lines = self.output.trim_start_matches('\n').split('\n');
        let first = lines.next().unwrap_or_default();
        let last = lines.next_back();
        // Internal lines belong to nested layouts. They must not force every
        // enclosing expression to expand, especially for preserved comments.
        // Measure a fragment independently when its prefix already overruns;
        // earlier indivisible text must not prevent this fragment from wrapping.
        if let Some(last) = last {
            prefix_columns <= width.get() && self.columns(first) > width.get()
                || self.columns(last) + suffix_columns > width.get()
        } else {
            let columns = if prefix_columns > width.get() {
                self.columns(first).saturating_sub(prefix_columns)
                    + self.indent * self.options.indent_size.get()
            } else {
                self.columns(first)
            };
            columns + suffix_columns > width.get()
        }
    }

    fn node_exceeds_width(&self, node: &SyntaxNode, space: bool, suffix: usize) -> bool {
        if self.measuring || self.options.max_line_length.is_none() {
            return false;
        }
        let mut preview = self.preview();
        preview.expression_before(node, space, suffix);
        preview.exceeds_width(self.current_columns(), suffix)
    }

    fn token_exceeds_width(&self, index: usize, space: bool, suffix: usize) -> bool {
        if self.measuring || self.options.max_line_length.is_none() {
            return false;
        }
        let mut preview = self.preview();
        preview.token_with_space(index, space);
        preview.exceeds_width(self.current_columns(), suffix)
    }

    // Before moving a whole child, see whether its initial unbroken segment
    // fits. Its own lists/operands can wrap after that segment. This decision
    // must not change when those nested layouts become explicit on reparse.
    fn prefix_exceeds_width(&self, node: &SyntaxNode, space: bool, suffix: usize) -> bool {
        if self.measuring {
            return false;
        }
        let Some(width) = self.options.max_line_length else {
            return false;
        };
        let mut preview = self.preview();
        preview.measuring_prefix = true;
        preview.expression_before(node, space, suffix);
        let text = preview.output.trim_start_matches('\n');
        let (first, remaining) = text.split_once('\n').unwrap_or((text, ""));
        self.columns(first) + if remaining.is_empty() { suffix } else { 0 } > width.get()
    }

    fn following_columns(&self, children: &[SyntaxElement], position: usize) -> usize {
        let mut columns = 0;
        for child in &children[position + 1..] {
            let SyntaxElement::Token(index) = child else {
                return columns;
            };
            let token = &self.parsed.tokens()[*index];
            if !is_trivia(token.kind) {
                columns += self.columns(&self.parsed.source()[token.range.clone()]);
                if !matches!(
                    token.kind,
                    TokenKind::RightParen
                        | TokenKind::RightBracket
                        | TokenKind::RightBrace
                        | TokenKind::Comma
                        | TokenKind::Colon
                        | TokenKind::Semicolon
                        | TokenKind::Dot
                        | TokenKind::Inverse
                ) {
                    columns += 1;
                }
            }
        }
        columns + self.suffix_columns
    }

    fn matrix(&mut self, node: &SyntaxNode, leading_space: bool) {
        let multiline = node.child_nodes().next().is_some();
        let layout = self.matrix_layout(node);
        let mut row_number = 0;
        let mut after_pipe = false;
        for child in node.children() {
            match child {
                SyntaxElement::Node(row) => {
                    if !after_pipe {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.matrix_row(row, &layout.rows[row_number], &layout);
                    row_number += 1;
                    after_pipe = false;
                }
                SyntaxElement::Token(index) => match self.parsed.tokens()[*index].kind {
                    TokenKind::MatrixStart => {
                        self.token_with_space(*index, leading_space);
                        self.indent += 1;
                    }
                    TokenKind::Pipe => {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                        self.token_with_space(*index, false);
                        after_pipe = true;
                    }
                    TokenKind::MatrixEnd => {
                        self.indent -= 1;
                        if multiline || self.line_comment || self.pending_breaks > 0 {
                            self.newlines(self.pending_breaks.max(1));
                            self.pending_breaks = 0;
                        }
                        self.token_with_space(*index, false);
                    }
                    _ => self.token(*index),
                },
            }
        }
    }

    // Render cells at the common continuation column before choosing shared
    // breaks. A nested multiline cell therefore keeps its ordinary layout even
    // when neighboring columns move on the next formatting pass.
    fn matrix_layout(&self, node: &SyntaxNode) -> MatrixLayout {
        let indent = (self.indent + 1) * self.options.indent_size.get();
        let index_width = node
            .child_nodes()
            .filter(|row| self.matrix_row_has_index(row))
            .map(|row| {
                let mut rendered =
                    self.render_matrix_cell(row.child_nodes().next().unwrap(), indent + 2, 1);
                let index_width = rendered.current_columns().saturating_sub(indent + 2) + 2;
                for child in row.children().iter().skip(1) {
                    let SyntaxElement::Token(index) = child else {
                        break;
                    };
                    rendered.token_with_space(*index, false);
                }
                if rendered.output[indent + 3..].contains('\n')
                    || rendered.line_comment
                    || rendered.last_was_comment && rendered.pending_breaks > 0
                {
                    index_width
                } else {
                    rendered.current_columns().saturating_sub(indent + 2) + 1
                }
            })
            .max()
            .unwrap_or(0);
        let base = indent + 2 + index_width;
        let mut rows = Vec::new();
        let mut forced_breaks = Vec::new();
        for row in node.child_nodes() {
            let indexed = self.matrix_row_has_index(row);
            let mut cells = Vec::new();
            for (position, child) in row.children().iter().enumerate() {
                let SyntaxElement::Node(cell) = child else {
                    continue;
                };
                let following = &row.children()[position + 1..];
                let tokens: Vec<_> = following
                    .iter()
                    .take_while(|child| matches!(child, SyntaxElement::Token(_)))
                    .filter_map(|child| match child {
                        SyntaxElement::Token(index) => Some(*index),
                        _ => None,
                    })
                    .collect();
                let suffix = tokens
                    .iter()
                    .filter(|&&index| !is_trivia(self.parsed.tokens()[index].kind))
                    .count();
                let is_index = indexed && cells.is_empty();
                let start = if is_index { indent + 2 } else { base };
                let mut rendered = self.render_matrix_cell(cell, start, suffix);
                let prefix_length = start + 1;
                let text = rendered.output[prefix_length..].to_owned();
                let pending_breaks = rendered.pending_breaks;
                let line_comment = rendered.line_comment;
                let last_was_comment = rendered.last_was_comment;
                let has_next = following
                    .iter()
                    .any(|child| matches!(child, SyntaxElement::Node(_)));
                for index in tokens {
                    let kind = self.parsed.tokens()[index].kind;
                    // A trailing comment has the usual preserved-text overrun
                    // exception; it does not widen the other rows' last cell.
                    if has_next || !is_trivia(kind) {
                        rendered.token_with_space(index, false);
                    }
                }
                let slot = rendered.output[prefix_length..].to_owned();
                if !is_index {
                    let column = cells.len() - usize::from(indexed);
                    forced_breaks.resize(forced_breaks.len().max(column + 2), false);
                    if text.contains('\n') {
                        forced_breaks[column] = true;
                        forced_breaks[column + 1] = true;
                    }
                    if slot.contains('\n')
                        || rendered.line_comment
                        || rendered.pending_breaks > 1
                        || rendered.last_was_comment && rendered.pending_breaks > 0
                    {
                        forced_breaks[column + 1] = true;
                    }
                }
                cells.push(MatrixCell {
                    text,
                    slot,
                    pending_breaks,
                    line_comment,
                    last_was_comment,
                });
            }
            rows.push(cells);
        }
        let mut starts = Vec::new();
        let mut breaks = Vec::new();
        let mut start = base;
        for (column, &forced_break) in forced_breaks
            .iter()
            .enumerate()
            .take(forced_breaks.len().saturating_sub(1))
        {
            let width_at = |start| {
                node.child_nodes()
                    .zip(&rows)
                    .filter_map(|(row, cells)| {
                        cells.get(column + usize::from(self.matrix_row_has_index(row)))
                    })
                    .map(|cell| {
                        self.columns_from(cell.slot.split('\n').next().unwrap_or_default(), start)
                            - start
                    })
                    .max()
                    .unwrap_or(0)
            };
            let mut width = width_at(start);
            let should_break = column > 0
                && (forced_break
                    || !self.measuring
                        && self
                            .options
                            .max_line_length
                            .is_some_and(|limit| start + width > limit.get()));
            if should_break {
                start = base;
                width = width_at(start);
            }
            starts.push(start);
            breaks.push(should_break);
            start += width + 1;
        }
        MatrixLayout {
            rows,
            starts,
            breaks,
        }
    }

    fn matrix_row_has_index(&self, row: &SyntaxNode) -> bool {
        row.kind() == NodeKind::MatrixRow
            && row.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                    if self.parsed.tokens()[*index].kind == TokenKind::Colon)
            })
    }

    fn render_matrix_cell(&self, cell: &SyntaxNode, start: usize, suffix: usize) -> Self {
        let mut rendered = self.preview();
        rendered.output = format!("\n{}", " ".repeat(start));
        rendered.indent = self.indent + 1;
        rendered.pending_breaks = 0;
        rendered.line_comment = false;
        rendered.last_was_comment = false;
        rendered.break_before_code = false;
        rendered.measuring = self.measuring;
        rendered.expression_before(cell, false, suffix);
        rendered
    }

    fn matrix_row(&mut self, row: &SyntaxNode, cells: &[MatrixCell], layout: &MatrixLayout) {
        let indexed = self.matrix_row_has_index(row);
        let mut entry: usize = 0;
        for child in row.children() {
            match child {
                SyntaxElement::Node(_) => {
                    let is_index = indexed && entry == 0;
                    let column = entry.saturating_sub(usize::from(indexed));
                    if !is_index && layout.breaks[column] {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    let start = if is_index {
                        self.indent * self.options.indent_size.get() + 2
                    } else {
                        layout.starts[column]
                    };
                    if self.line_comment
                        || self.pending_breaks > 1
                        || self.last_was_comment
                            && (self.pending_breaks > 0
                                || entry == 0 && self.current_columns() >= start)
                    {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.pending_breaks = 0;
                    self.indent_line();
                    self.output
                        .push_str(&" ".repeat(start.saturating_sub(self.current_columns())));
                    let cell = &cells[entry];
                    self.code_with_space(&cell.text, false);
                    self.pending_breaks = cell.pending_breaks;
                    self.line_comment = cell.line_comment;
                    self.last_was_comment = cell.last_was_comment;
                    entry += 1;
                }
                SyntaxElement::Token(index) => self.token_with_space(*index, false),
            }
        }
    }

    fn comma_list(
        &mut self,
        node: &SyntaxNode,
        leading_space: bool,
        opening: TokenKind,
        closing: TokenKind,
    ) {
        let mut in_list = false;
        let multiline = node.children().iter().any(|child| {
            if let SyntaxElement::Token(index) = child {
                let token = &self.parsed.tokens()[*index];
                if token.kind == closing {
                    in_list = false;
                }
                if token.kind == opening {
                    in_list = true;
                }
                in_list
                    && token.kind == TokenKind::Whitespace
                    && self.parsed.source()[token.range.clone()].contains(['\r', '\n'])
            } else {
                false
            }
        }) || self.measuring_prefix && node.child_nodes().next().is_some()
            || self.node_exceeds_width(node, leading_space, self.suffix_columns);
        let mut first = true;
        let mut first_entry = true;
        let mut continuation = false;
        let mut previous_entry_end = self.output.len();
        in_list = false;
        for (position, child) in node.children().iter().enumerate() {
            match child {
                SyntaxElement::Node(entry) => {
                    let comment_before_entry = self.last_was_comment
                        && !self.line_comment
                        && self.pending_breaks == 0
                        && self.output[previous_entry_end..].contains('\n');
                    if in_list && multiline && !comment_before_entry {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    let space = if in_list {
                        !first_entry && !multiline
                    } else {
                        first && leading_space || !first
                    };
                    if !in_list
                        && !first
                        && self.prefix_exceeds_width(
                            entry,
                            space,
                            self.following_columns(node.children(), position),
                        )
                    {
                        if !continuation {
                            self.indent += 1;
                            continuation = true;
                        }
                        self.newlines(self.pending_breaks.max(1));
                    }
                    let suffix = if in_list && multiline {
                        1
                    } else {
                        self.following_columns(node.children(), position)
                    };
                    self.expression_before(entry, space, suffix);
                    if in_list {
                        first_entry = false;
                        // All supported comma lists permit a trailing comma.
                        // Insert it before trivia so a following comment stays attached.
                        if multiline
                            && !node.children()[position + 1..].iter().any(|child| {
                                matches!(child, SyntaxElement::Token(index)
                                if self.parsed.tokens()[*index].kind == TokenKind::Comma)
                            })
                        {
                            self.code_with_space(",", false);
                        }
                        previous_entry_end = self.output.len();
                    }
                    first = false;
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if is_trivia(kind) {
                        self.token(*index);
                        continue;
                    }
                    if kind == closing && multiline {
                        self.indent -= 1;
                        self.newlines(self.pending_breaks.max(1));
                    }
                    let space = if first {
                        leading_space
                    } else if in_list || kind == opening || node.kind() == NodeKind::CallExpression
                    {
                        false
                    } else {
                        kind != TokenKind::Colon
                    };
                    self.token_with_space(*index, space);
                    if kind == opening {
                        in_list = true;
                        previous_entry_end = self.output.len();
                        if multiline {
                            self.indent += 1;
                        }
                    } else if kind == closing {
                        in_list = false;
                    }
                    first = false;
                }
            }
        }
        if continuation {
            self.indent -= 1;
        }
    }

    fn token(&mut self, index: usize) {
        let kind = self.parsed.tokens()[index].kind;
        self.token_with_space(
            index,
            !matches!(kind, TokenKind::Colon | TokenKind::Semicolon),
        );
    }

    fn token_with_space(&mut self, index: usize, space_before: bool) {
        let token = &self.parsed.tokens()[index];
        let text = &self.parsed.source()[token.range.clone()];
        match token.kind {
            TokenKind::Whitespace => {
                let mut characters = text.chars().peekable();
                while let Some(character) = characters.next() {
                    if character == '\n' || (character == '\r' && characters.peek() != Some(&'\n'))
                    {
                        self.pending_breaks = (self.pending_breaks + 1).min(2);
                    }
                }
            }
            TokenKind::LineComment | TokenKind::BlockComment => {
                if self.pending_breaks > 0 || self.line_comment {
                    self.newlines(self.pending_breaks.max(1));
                }
                self.space();
                self.indent_line();
                self.output.push_str(text);
                self.pending_breaks = 0;
                self.line_comment = token.kind == TokenKind::LineComment;
                self.last_was_comment = true;
            }
            _ => self.code_with_space(text, space_before),
        }
    }

    fn code(&mut self, kind: TokenKind, text: &str) {
        self.code_with_space(
            text,
            !matches!(kind, TokenKind::Colon | TokenKind::Semicolon),
        );
    }

    fn code_with_space(&mut self, text: &str, space_before: bool) {
        if self.break_before_code {
            self.newlines(self.pending_breaks.max(1));
            self.break_before_code = false;
        }
        if self.pending_breaks > 1
            || self.line_comment
            || (self.last_was_comment && self.pending_breaks > 0)
        {
            self.newlines(self.pending_breaks.max(1));
        }
        if space_before || self.last_was_comment {
            self.space();
        }
        self.indent_line();
        self.output.push_str(text);
        self.pending_breaks = 0;
        self.line_comment = false;
        self.last_was_comment = false;
    }

    fn indent_line(&mut self) {
        if self.output.ends_with('\n') {
            let columns = self.indent * self.options.indent_size.get();
            let tab_width = self.options.tab_width.get();
            if self.options.indent_style == IndentStyle::Tab {
                for _ in 0..columns / tab_width {
                    self.output.push('\t');
                }
                for _ in 0..columns % tab_width {
                    self.output.push(' ');
                }
            } else {
                for _ in 0..columns {
                    self.output.push(' ');
                }
            }
        }
    }

    fn space(&mut self) {
        if !self.output.is_empty() && !self.output.ends_with(['\n', '\r', ' ', '\t']) {
            self.output.push(' ');
        }
    }

    fn newlines(&mut self, count: usize) {
        if self.output.is_empty() {
            return;
        }
        let existing = self.output.chars().rev().take_while(|&c| c == '\n').count();
        for _ in existing..count.min(2) {
            self.output.push('\n');
        }
        self.line_comment = false;
    }
}

fn is_binary(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::BinaryExpression
            | NodeKind::RangeExpression
            | NodeKind::TypeInstConcatenation
            | NodeKind::EnumDefinition
    )
}

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}
