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
        let labelled = item.kind() == NodeKind::Constraint
            && item.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(index)
                    if self.parsed.tokens()[*index].kind == TokenKind::AnnotationMarker)
            });
        let mut has_semicolon = false;
        for child in item.children() {
            match child {
                SyntaxElement::Node(expression) => {
                    if labelled {
                        self.newlines(self.pending_breaks.max(1));
                    }
                    self.expression(expression, true);
                }
                SyntaxElement::Token(index) => {
                    has_semicolon |= self.parsed.tokens()[*index].kind == TokenKind::Semicolon;
                    self.token(*index);
                }
            }
        }
        if !has_semicolon {
            self.code(TokenKind::Semicolon, ";");
        }
        self.between_items = true;
    }

    fn expression(&mut self, node: &SyntaxNode, leading_space: bool) {
        if node.kind() == NodeKind::MatrixLiteral {
            self.matrix(node, leading_space);
            return;
        }
        let delimiters = match node.kind() {
            NodeKind::CallExpression => Some((TokenKind::LeftParen, TokenKind::RightParen)),
            NodeKind::IndexTuple => Some((TokenKind::LeftParen, TokenKind::RightParen)),
            NodeKind::ArrayLiteral | NodeKind::ArrayAccessExpression | NodeKind::ArrayType => {
                Some((TokenKind::LeftBracket, TokenKind::RightBracket))
            }
            NodeKind::SetLiteral => Some((TokenKind::LeftBrace, TokenKind::RightBrace)),
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
                            NodeKind::ParenthesizedExpression | NodeKind::SetCardinality => false,
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
                    let space =
                        if in_list || kind == opening || node.kind() == NodeKind::CallExpression {
                            first && leading_space
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
