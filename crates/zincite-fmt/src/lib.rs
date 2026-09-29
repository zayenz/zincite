//! Formatting for Zincite's supported scalar and collection model syntax.

use zincite_syntax::{Diagnostic, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

/// Format a complete parse result, or return its diagnostics without output.
///
/// Identifiers, literals, operators and comments retain their spelling. Editable
/// layout uses LF and a final newline. No type or semantic checks are performed.
pub fn format(parsed: &ParsedFile) -> Result<String, &[Diagnostic]> {
    if !parsed.diagnostics().is_empty() {
        return Err(parsed.diagnostics());
    }
    let mut formatter = Formatter {
        parsed,
        output: String::new(),
        pending_breaks: 0,
        line_comment: false,
        last_was_comment: false,
        between_items: false,
        indent: 0,
    };
    for child in parsed.tree().children() {
        match child {
            SyntaxElement::Node(item) => formatter.item(item),
            SyntaxElement::Token(index) => formatter.token(*index),
        }
    }
    if !formatter.output.is_empty() {
        formatter.newlines(1);
    }
    Ok(formatter.output)
}

struct Formatter<'a> {
    parsed: &'a ParsedFile,
    output: String,
    pending_breaks: usize,
    line_comment: bool,
    last_was_comment: bool,
    between_items: bool,
    indent: usize,
}

impl Formatter<'_> {
    fn item(&mut self, item: &SyntaxNode) {
        if self.between_items || self.pending_breaks > 0 || self.line_comment {
            self.newlines(self.pending_breaks.max(1));
        }
        self.pending_breaks = 0;
        self.between_items = false;
        self.item_contents(item);
        if !item.children().iter().any(|child| {
            matches!(child, SyntaxElement::Token(index)
                if self.parsed.tokens()[*index].kind == TokenKind::Semicolon)
        }) {
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
        for child in item.children() {
            match child {
                SyntaxElement::Node(expression) => {
                    if labelled && !label_pending {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.expression(expression, expression.kind() != NodeKind::ParameterList);
                    label_pending = false;
                }
                SyntaxElement::Token(index) => {
                    self.token(*index);
                    if self.parsed.tokens()[*index].kind == TokenKind::StringLiteral {
                        label_pending = false;
                    }
                }
            }
        }
    }

    fn expression(&mut self, node: &SyntaxNode, leading_space: bool) {
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
        let delimiters = match node.kind() {
            NodeKind::CallExpression | NodeKind::ParameterList => {
                Some((TokenKind::LeftParen, TokenKind::RightParen))
            }
            NodeKind::IndexTuple => Some((TokenKind::LeftParen, TokenKind::RightParen)),
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
        for child in node.children() {
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
                    self.expression(child, space);
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
                    } else {
                        !matches!(
                            kind,
                            TokenKind::RightParen
                                | TokenKind::Comma
                                | TokenKind::Colon
                                | TokenKind::Inverse
                        )
                    };
                    self.token_with_space(*index, space);
                    first = false;
                    previous = Some(kind);
                }
            }
        }
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
                    for part in branch.children() {
                        match part {
                            SyntaxElement::Node(expression) => {
                                if body {
                                    self.newlines(self.pending_breaks.max(1));
                                }
                                self.expression(expression, true);
                            }
                            SyntaxElement::Token(index) => {
                                let kind = self.parsed.tokens()[*index].kind;
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
                    self.expression(body, true);
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
                    self.item_contents(item);
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
        let head_expanded = !call && self.has_line_break(node);
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
                    if head_expanded {
                        self.newlines(1);
                        self.pending_breaks = 0;
                    }
                    self.expression(head, false);
                }
                SyntaxElement::Token(index) => {
                    let kind = self.parsed.tokens()[*index].kind;
                    if kind == closing {
                        if header_expanded || head_expanded {
                            self.indent -= 1;
                            self.newlines(1);
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
        preview.exceeds_width()
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
                        self.newlines(1);
                        self.pending_breaks = 0;
                    }
                    self.generator(generator, expanded, !expanded && (leading_space || !first));
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
        for child in generator.children() {
            match child {
                SyntaxElement::Node(filter) if filter.kind() == NodeKind::WhereFilter => {
                    if expanded {
                        self.indent += 1;
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.expression(filter, true);
                    if expanded {
                        self.indent -= 1;
                    }
                }
                SyntaxElement::Node(source) => self.expression(source, true),
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
        let mut preview = self.preview();
        preview.expression(body, true);
        let expanded = forall || self.has_line_break(body) || preview.exceeds_width();
        let mut previous_end = self.output.len();
        for child in body.children() {
            match child {
                SyntaxElement::Node(expression) => {
                    let comment_before = self.last_was_comment
                        && !self.line_comment
                        && self.pending_breaks == 0
                        && self.output[previous_end..].contains('\n');
                    if expanded && !comment_before {
                        self.newlines(1);
                        self.pending_breaks = 0;
                    }
                    self.expression(expression, false);
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

    // Width decisions are limited to generator headers and bodies for now.
    // Keep the current line so nested headers account for their actual column.
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
        }
    }

    fn exceeds_width(&self) -> bool {
        self.output.lines().any(|line| {
            line.chars().fold(0, |column, character| {
                if character == '\t' {
                    column + 4 - column % 4
                } else {
                    column + 1
                }
            }) > 120
        })
    }

    fn matrix(&mut self, node: &SyntaxNode, leading_space: bool) {
        let multiline = node.child_nodes().next().is_some();
        let mut after_pipe = false;
        for child in node.children() {
            match child {
                SyntaxElement::Node(row) => {
                    if !after_pipe {
                        self.newlines(self.pending_breaks.max(1));
                        self.pending_breaks = 0;
                    }
                    self.expression(row, after_pipe);
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
        });
        let mut first = true;
        let mut first_entry = true;
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
                        self.newlines(1);
                        self.pending_breaks = 0;
                    }
                    let space = if in_list {
                        !first_entry && !multiline
                    } else {
                        first && leading_space || !first
                    };
                    self.expression(entry, space);
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
                        self.newlines(1);
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
        if self.line_comment || (self.last_was_comment && self.pending_breaks > 0) {
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
            for _ in 0..self.indent {
                self.output.push_str("    ");
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

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}
