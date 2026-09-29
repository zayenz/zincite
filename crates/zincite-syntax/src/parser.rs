use crate::{Diagnostic, NodeKind, SyntaxElement, SyntaxNode, Token, TokenKind};

pub(crate) fn parse(tokens: &[Token], source_len: usize) -> (SyntaxNode, Vec<Diagnostic>) {
    let mut parser = Parser {
        tokens,
        source_len,
        position: 0,
        diagnostics: Vec::new(),
    };
    let mut children = Vec::new();
    while parser.position < tokens.len() {
        if is_trivia(tokens[parser.position].kind) {
            children.push(SyntaxElement::Token(parser.position));
            parser.position += 1;
            continue;
        }
        let start = parser.position;
        let mut item_children = Vec::new();
        let item_kind = parser.item(&mut item_children);
        let node = match item_kind {
            Ok(kind) => parser.node(kind, start, item_children),
            Err(message) => {
                parser.diagnose(message);
                parser.recover(start);
                parser.node(
                    NodeKind::Error,
                    start,
                    (start..parser.position).map(SyntaxElement::Token).collect(),
                )
            }
        };
        children.push(SyntaxElement::Node(node));
    }
    (
        SyntaxNode {
            kind: NodeKind::Root,
            range: 0..source_len,
            children,
        },
        parser.diagnostics,
    )
}

struct Parser<'a> {
    tokens: &'a [Token],
    source_len: usize,
    position: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn item(&mut self, children: &mut Vec<SyntaxElement>) -> Result<NodeKind, &'static str> {
        use TokenKind::*;
        let kind = match self.peek() {
            Some(Var | Par | Bool | Int | Float | String) => {
                if matches!(self.peek(), Some(Var | Par)) {
                    self.bump(children);
                }
                if !matches!(self.peek(), Some(Bool | Int | Float | String)) {
                    return Err("unsupported type; expected bool, int, float or string");
                }
                self.bump(children);
                self.expect(Colon, children, "expected ':' after scalar type")?;
                self.name(children)?;
                if self.peek() == Some(Equal) {
                    self.bump(children);
                    self.expression(children)?;
                }
                NodeKind::Declaration
            }
            Some(Identifier | QuotedIdentifier) => {
                self.bump(children);
                self.expect(Equal, children, "expected '=' after assignment name")?;
                self.expression(children)?;
                NodeKind::Assignment
            }
            Some(Constraint) => {
                self.bump(children);
                if self.peek() == Some(AnnotationMarker) {
                    self.bump(children);
                    self.expect(
                        StringLiteral,
                        children,
                        "only direct string constraint labels are supported",
                    )?;
                }
                self.expression(children)?;
                NodeKind::Constraint
            }
            Some(Solve) => {
                self.bump(children);
                self.expect(Satisfy, children, "only 'solve satisfy' is supported")?;
                NodeKind::Solve
            }
            _ => {
                return Err(
                    "unsupported top-level item; expected a scalar declaration, assignment, constraint or solve satisfy",
                );
            }
        };
        match self.peek() {
            Some(Semicolon) => self.bump(children),
            // The pinned model grammar permits the last item without a separator.
            // https://docs.minizinc.dev/en/2.10.1/spec.html#high-level-model-structure
            None => {}
            token => return Err(unsupported_expression(token).unwrap_or("expected ';' after item")),
        }
        Ok(kind)
    }

    fn name(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        if !matches!(
            self.peek(),
            Some(TokenKind::Identifier | TokenKind::QuotedIdentifier)
        ) {
            return Err("expected a declaration name");
        }
        self.bump(children);
        Ok(())
    }

    fn expression(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        use TokenKind::*;
        if !matches!(
            self.peek(),
            Some(
                Identifier
                    | QuotedIdentifier
                    | IntegerLiteral
                    | FloatLiteral
                    | StringLiteral
                    | True
                    | False
                    | Infinity
                    | Anonymous
            )
        ) {
            return Err(unsupported_expression(self.peek())
                .unwrap_or("expected an atom (literal, identifier or '_')"));
        }
        self.trivia(children);
        let start = self.position;
        let mut atom = Vec::new();
        self.bump(&mut atom);
        children.push(SyntaxElement::Node(self.node(
            NodeKind::Expression,
            start,
            atom,
        )));
        Ok(())
    }

    fn expect(
        &mut self,
        kind: TokenKind,
        children: &mut Vec<SyntaxElement>,
        message: &'static str,
    ) -> Result<(), &'static str> {
        if self.peek() != Some(kind) {
            return Err(message);
        }
        self.bump(children);
        Ok(())
    }

    fn peek(&self) -> Option<TokenKind> {
        self.significant(self.position)
            .map(|index| self.tokens[index].kind)
    }

    fn significant(&self, position: usize) -> Option<usize> {
        (position..self.tokens.len()).find(|&index| !is_trivia(self.tokens[index].kind))
    }

    fn trivia(&mut self, children: &mut Vec<SyntaxElement>) {
        while self.position < self.tokens.len() && is_trivia(self.tokens[self.position].kind) {
            children.push(SyntaxElement::Token(self.position));
            self.position += 1;
        }
    }

    fn bump(&mut self, children: &mut Vec<SyntaxElement>) {
        self.trivia(children);
        children.push(SyntaxElement::Token(self.position));
        self.position += 1;
    }

    fn node(&self, kind: NodeKind, start: usize, children: Vec<SyntaxElement>) -> SyntaxNode {
        SyntaxNode {
            kind,
            range: self.tokens[start].range.start..self.tokens[self.position - 1].range.end,
            children,
        }
    }

    fn diagnose(&mut self, message: &str) {
        let range = self
            .significant(self.position)
            .map(|index| self.tokens[index].range.clone())
            .unwrap_or(self.source_len..self.source_len);
        self.diagnostics.push(Diagnostic {
            range,
            message: message.to_owned(),
        });
    }

    fn recover(&mut self, start: usize) {
        // Do not mistake the scalar tail of an unsupported type for a new item.
        let can_find_next_item = self.position > start
            && !matches!(
                self.tokens[self.position - 1].kind,
                TokenKind::Var | TokenKind::Par
            );
        let mut depth = 0usize;
        while self.position < self.tokens.len() {
            let kind = self.tokens[self.position].kind;
            if depth == 0 && can_find_next_item && self.starts_item(self.position) {
                break;
            }
            self.position += 1;
            if kind == TokenKind::Semicolon {
                break;
            }
            match kind {
                TokenKind::LeftParen | TokenKind::LeftBracket | TokenKind::LeftBrace => depth += 1,
                TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace => {
                    depth = depth.saturating_sub(1);
                }
                _ => {}
            }
        }
    }

    fn starts_item(&self, position: usize) -> bool {
        use TokenKind::*;
        let mut index = position;
        if matches!(self.tokens[index].kind, Constraint | Solve) {
            return true;
        }
        if matches!(self.tokens[index].kind, Var | Par) {
            let Some(next) = self.significant(index + 1) else {
                return false;
            };
            index = next;
        }
        if !matches!(self.tokens[index].kind, Bool | Int | Float | String) {
            return false;
        }
        self.significant(index + 1)
            .is_some_and(|next| self.tokens[next].kind == Colon)
    }
}

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}

fn unsupported_expression(kind: Option<TokenKind>) -> Option<&'static str> {
    use TokenKind::*;
    match kind? {
        LeftParen => Some("calls and parenthesized expressions are unsupported"),
        AnnotationMarker => Some("general annotations are unsupported"),
        Absent => Some("absent atoms are unsupported"),
        Plus | Minus | Star | Slash | Power | Concat | Equal | DoubleEqual | NotEqual
        | WeakEqual | WeakNotEqual | Less | Greater | LessEqual | GreaterEqual | Equivalence
        | Implies | ReverseImplies | Or | And | Xor | Not | In | Subset | Superset | Union
        | Diff | SymDiff | Intersect | Div | Mod | RangeInclusive | RangeExclusiveStart
        | RangeExclusiveEnd | RangeExclusive | WeakPlus | WeakMinus | WeakStar | WeakSlash
        | WeakDiv | Inverse => Some("expression operators are unsupported"),
        LeftBracket | LeftBrace | MatrixStart | Dot | Pipe | If | Let | Case | InfixIdentifier => {
            Some("this expression syntax is unsupported; only atoms are supported")
        }
        _ => None,
    }
}
