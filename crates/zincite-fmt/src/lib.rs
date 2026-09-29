//! Formatting for Zincite's supported scalar model syntax.

use zincite_syntax::{Diagnostic, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

/// Format a complete parse result, or return its diagnostics without output.
///
/// Identifiers, atoms and comments retain their original spelling. Editable
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
                    for atom in expression.children() {
                        if let SyntaxElement::Token(index) = atom {
                            self.token(*index);
                        }
                    }
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

    fn token(&mut self, index: usize) {
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
                self.output.push_str(text);
                self.pending_breaks = 0;
                self.line_comment = token.kind == TokenKind::LineComment;
                self.last_was_comment = true;
            }
            kind => self.code(kind, text),
        }
    }

    fn code(&mut self, kind: TokenKind, text: &str) {
        if self.line_comment || (self.last_was_comment && self.pending_breaks > 0) {
            self.newlines(self.pending_breaks.max(1));
        }
        if !matches!(kind, TokenKind::Colon | TokenKind::Semicolon) {
            self.space();
        }
        self.output.push_str(text);
        self.pending_breaks = 0;
        self.line_comment = false;
        self.last_was_comment = false;
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
