use crate::{Diagnostic, NodeKind, SyntaxElement, SyntaxNode, Token, TokenKind};

pub(crate) fn parse(tokens: &[Token], source: &str) -> (SyntaxNode, Vec<Diagnostic>) {
    let mut parser = Parser {
        tokens,
        source,
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
            range: 0..source.len(),
            children,
        },
        parser.diagnostics,
    )
}

struct Parser<'a> {
    tokens: &'a [Token],
    source: &'a str,
    position: usize,
    diagnostics: Vec<Diagnostic>,
}

impl Parser<'_> {
    fn item(&mut self, children: &mut Vec<SyntaxElement>) -> Result<NodeKind, &'static str> {
        let kind = self.item_body(children)?;
        match self.peek() {
            Some(TokenKind::Semicolon) => self.bump(children),
            // The pinned model grammar permits the last item without a separator.
            // https://docs.minizinc.dev/en/2.10.1/spec.html#high-level-model-structure
            None => {}
            token => return Err(unsupported_expression(token).unwrap_or("expected ';' after item")),
        }
        Ok(kind)
    }

    fn item_body(&mut self, children: &mut Vec<SyntaxElement>) -> Result<NodeKind, &'static str> {
        use TokenKind::*;
        let kind = match self.peek() {
            Some(Identifier | QuotedIdentifier) if self.peek_after(1) == Some(Equal) => {
                self.bump(children);
                self.bump(children);
                self.expression(children)?;
                NodeKind::Assignment
            }
            Some(Any) if self.peek_after(1) == Some(Colon) => {
                let start = self.position;
                let mut type_children = Vec::new();
                self.bump(&mut type_children);
                children.push(SyntaxElement::Node(self.node(
                    NodeKind::ScalarType,
                    start,
                    type_children,
                )));
                self.declaration_tail(children)?;
                NodeKind::Declaration
            }
            Some(Function | Predicate | Test | Annotation) => return self.callable(children),
            token if token.is_some_and(starts_type) => {
                children.push(SyntaxElement::Node(self.type_inst()?));
                self.declaration_tail(children)?;
                NodeKind::Declaration
            }
            Some(Constraint) => {
                self.bump(children);
                if self.peek() == Some(AnnotationMarker) {
                    self.bump(children);
                    match self.peek() {
                        Some(StringLiteral) => self.bump(children),
                        Some(StringHead) => {
                            self.trivia(children);
                            let start = self.position;
                            let mut label = Vec::new();
                            self.interpolated_string(&mut label)?;
                            children.push(SyntaxElement::Node(self.node(
                                NodeKind::InterpolatedString,
                                start,
                                label,
                            )));
                        }
                        _ => return Err("only direct string constraint labels are supported"),
                    }
                }
                self.expression(children)?;
                NodeKind::Constraint
            }
            Some(Include) => {
                self.bump(children);
                if !matches!(self.peek(), Some(StringLiteral | StringHead)) {
                    return Err("expected a string literal after 'include'");
                }
                self.trivia(children);
                let path = self.atom_head(Grammar::General, false)?;
                if !matches!(
                    path.kind(),
                    NodeKind::Expression | NodeKind::InterpolatedString
                ) {
                    return Err("expected a string literal after 'include'");
                }
                children.push(SyntaxElement::Node(path));
                NodeKind::Include
            }
            Some(Output) => {
                self.bump(children);
                if self.peek() == Some(AnnotationMarker) {
                    self.trivia(children);
                    children.push(SyntaxElement::Node(self.output_annotation()?));
                }
                self.expression(children)?;
                NodeKind::Output
            }
            Some(Solve) => {
                self.bump(children);
                while self.peek() == Some(AnnotationMarker) {
                    self.trivia(children);
                    children.push(SyntaxElement::Node(self.annotation()?));
                }
                let mode = self.peek();
                if !matches!(mode, Some(Satisfy | Minimize | Maximize)) {
                    return Err(
                        "expected 'satisfy', 'minimize' or 'maximize' after solve annotations",
                    );
                }
                self.bump(children);
                match mode {
                    Some(Satisfy) => NodeKind::Solve,
                    Some(Minimize) => {
                        self.expression(children)?;
                        NodeKind::SolveMinimize
                    }
                    Some(Maximize) => {
                        self.expression(children)?;
                        NodeKind::SolveMaximize
                    }
                    _ => unreachable!(),
                }
            }
            _ => {
                return Err(
                    "unsupported top-level item; expected a declaration, assignment, constraint, include, output or solve",
                );
            }
        };
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

    fn declaration_tail(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        self.expect(TokenKind::Colon, children, "expected ':' after type")?;
        self.name(children)?;
        while self.peek() == Some(TokenKind::AnnotationMarker) {
            self.trivia(children);
            children.push(SyntaxElement::Node(self.annotation()?));
        }
        if self.peek() == Some(TokenKind::Equal) {
            self.bump(children);
            self.expression(children)?;
        }
        Ok(())
    }

    fn callable(&mut self, children: &mut Vec<SyntaxElement>) -> Result<NodeKind, &'static str> {
        use TokenKind::*;
        let keyword = self.peek().unwrap();
        self.bump(children);
        let kind = match keyword {
            Function => {
                self.trivia(children);
                children.push(SyntaxElement::Node(self.type_inst()?));
                self.expect(Colon, children, "expected ':' after function return type")?;
                NodeKind::FunctionDeclaration
            }
            Predicate => NodeKind::PredicateDeclaration,
            Test => NodeKind::TestDeclaration,
            Annotation => NodeKind::AnnotationDeclaration,
            _ => unreachable!(),
        };
        self.name(children)?;
        if self.peek() == Some(LeftParen) {
            self.trivia(children);
            children.push(SyntaxElement::Node(self.parameters()?));
        }
        if keyword != Annotation {
            while self.peek() == Some(AnnotationMarker) {
                self.trivia(children);
                children.push(SyntaxElement::Node(self.annotation()?));
            }
        }
        if self.peek() == Some(Equal) {
            self.bump(children);
            self.expression(children)?;
        }
        Ok(kind)
    }

    fn parameters(&mut self) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let start = self.position;
        let mut parameters = Vec::new();
        self.bump(&mut parameters);
        while self.peek() != Some(RightParen) {
            self.trivia(&mut parameters);
            let parameter_start = self.position;
            let mut parameter = vec![SyntaxElement::Node(self.type_inst_context(true)?)];
            self.expect(Colon, &mut parameter, "expected ':' after parameter type")?;
            self.name(&mut parameter)?;
            if self.peek() == Some(Equal) {
                self.bump(&mut parameter);
                self.expression(&mut parameter)?;
            }
            parameters.push(SyntaxElement::Node(self.node(
                NodeKind::Parameter,
                parameter_start,
                parameter,
            )));
            if self.peek() != Some(Comma) {
                break;
            }
            self.bump(&mut parameters);
        }
        self.expect(
            RightParen,
            &mut parameters,
            "expected ',' or ')' after parameter",
        )?;
        Ok(self.node(NodeKind::ParameterList, start, parameters))
    }

    fn type_inst(&mut self) -> Result<SyntaxNode, &'static str> {
        self.type_inst_context(false)
    }

    // Parameter array indices use ordinary ti-expr, but their direct bindings
    // and parameter element cardinalities are excluded by param-ti-expr.
    // https://docs.minizinc.dev/en/2.10.1/spec.html#items
    fn type_inst_context(&mut self, parameter: bool) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let start = self.position;
        let mut children = Vec::new();
        let kind = match self.peek() {
            Some(Array) => {
                self.bump(&mut children);
                self.expect(LeftBracket, &mut children, "expected '[' after 'array'")?;
                if self.peek() == Some(RightBracket) {
                    return Err("expected at least one array index type");
                }
                loop {
                    self.trivia(&mut children);
                    children.push(SyntaxElement::Node(self.array_index(parameter)?));
                    if self.peek() != Some(Comma) {
                        break;
                    }
                    self.bump(&mut children);
                    if self.peek() == Some(RightBracket) {
                        break;
                    }
                }
                self.expect(
                    RightBracket,
                    &mut children,
                    "expected ',' or ']' after array index type",
                )?;
                self.expect(Of, &mut children, "expected 'of' after array index types")?;
                children.push(SyntaxElement::Node(self.type_inst_context(parameter)?));
                NodeKind::ArrayType
            }
            Some(List) => {
                self.bump(&mut children);
                self.expect(Of, &mut children, "expected 'of' after 'list'")?;
                children.push(SyntaxElement::Node(self.type_inst_context(parameter)?));
                NodeKind::ListType
            }
            Some(Any) => {
                self.bump(&mut children);
                self.expect(
                    TypeInstVariable,
                    &mut children,
                    "expected type-inst variable after 'any'",
                )?;
                NodeKind::TypeInstVariable
            }
            _ => {
                if matches!(self.peek(), Some(Var | Par)) {
                    self.bump(&mut children);
                }
                if self.peek() == Some(Opt) {
                    self.bump(&mut children);
                }
                if self.peek() == Some(Set) {
                    self.bump(&mut children);
                    if self.peek() == Some(LeftParen) {
                        if parameter {
                            return Err("set cardinalities are not allowed in parameter types");
                        }
                        self.trivia(&mut children);
                        children.push(SyntaxElement::Node(self.set_cardinality()?));
                    }
                    self.expect(Of, &mut children, "expected 'of' after 'set'")?;
                    children.push(SyntaxElement::Node(
                        self.base_type(self.position, Vec::new())?,
                    ));
                    NodeKind::SetType
                } else {
                    return self.base_type(start, children);
                }
            }
        };
        Ok(self.node(kind, start, children))
    }

    fn array_index(&mut self, parameter: bool) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        if !matches!(self.peek(), Some(Identifier | QuotedIdentifier))
            || self.peek_after(1) != Some(In)
        {
            return self.type_inst();
        }
        if parameter {
            return Err("dependent array indices are not allowed in parameter types");
        }
        let start = self.position;
        let mut children = Vec::new();
        self.bump(&mut children);
        self.bump(&mut children);
        children.push(SyntaxElement::Node(self.type_inst()?));
        Ok(self.node(NodeKind::ArrayIndexBinding, start, children))
    }

    fn set_cardinality(&mut self) -> Result<SyntaxNode, &'static str> {
        let start = self.position;
        let mut children = Vec::new();
        self.bump(&mut children);
        self.expression(&mut children)?;
        self.expect(
            TokenKind::RightParen,
            &mut children,
            "expected ')' after set cardinality",
        )?;
        Ok(self.node(NodeKind::SetCardinality, start, children))
    }

    fn base_type(
        &mut self,
        start: usize,
        mut children: Vec<SyntaxElement>,
    ) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let kind = if matches!(self.peek(), Some(Bool | Int | Float | String | Ann)) {
            self.bump(&mut children);
            NodeKind::ScalarType
        } else if self.peek() == Some(TypeInstVariable) {
            self.bump(&mut children);
            NodeKind::TypeInstVariable
        } else {
            if matches!(self.peek(), Some(Tuple | Record)) {
                return Err("structured types are unsupported");
            }
            children.push(SyntaxElement::Node(self.precedence(Grammar::Domain, 1600)?));
            NodeKind::DomainType
        };
        Ok(self.node(kind, start, children))
    }

    fn expression(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        self.trivia(children);
        children.push(SyntaxElement::Node(
            self.precedence(Grammar::General, 1600)?,
        ));
        Ok(())
    }

    // The specification numbers tighter operators lower. Each recursive call
    // accepts operators at or below its loosest permitted precedence.
    // https://docs.minizinc.dev/en/2.10.1/spec.html#operators
    fn precedence(&mut self, grammar: Grammar, loosest: u16) -> Result<SyntaxNode, &'static str> {
        let start = self.position;
        let mut left = if grammar != Grammar::Numeric && self.peek().is_some_and(is_range) {
            if loosest < 700 {
                return Err("parentheses are required around this open-ended range");
            }
            let mut bounds = Vec::new();
            self.bump(&mut bounds);
            if self.range_end() {
                return Err("expected at least one range bound");
            }
            let bounds_grammar = if grammar == Grammar::Domain {
                Grammar::Numeric
            } else {
                grammar
            };
            bounds.push(SyntaxElement::Node(self.precedence(bounds_grammar, 699)?));
            self.node(NodeKind::RangeExpression, start, bounds)
        } else {
            self.atom_head(grammar, false)?
        };
        let mut previous = if left.kind == NodeKind::RangeExpression {
            Some(700)
        } else {
            None
        };
        loop {
            if self.peek() == Some(TokenKind::AnnotationMarker) {
                let annotation = self.annotation()?;
                left = self.node(
                    NodeKind::AnnotatedExpression,
                    start,
                    vec![SyntaxElement::Node(left), SyntaxElement::Node(annotation)],
                );
                continue;
            }
            if self.peek() == Some(TokenKind::Inverse) && 400 <= loosest {
                let mut children = vec![SyntaxElement::Node(left)];
                self.bump(&mut children);
                left = self.node(NodeKind::UnaryExpression, start, children);
                previous = Some(400);
                continue;
            }
            let Some((precedence, associativity)) = self.peek().and_then(operator) else {
                break;
            };
            if precedence > loosest
                || (grammar == Grammar::Numeric && !self.peek().is_some_and(is_numeric_operator))
            {
                break;
            }
            if associativity == Associativity::None && previous == Some(precedence) {
                return Err("non-associative operators require parentheses");
            }
            let range = self.peek().is_some_and(is_range);
            if range && grammar == Grammar::Domain && !is_numeric_expression(&left, self.tokens) {
                return Err("expected a numeric expression for range type bound");
            }
            let mut children = vec![SyntaxElement::Node(left)];
            self.bump(&mut children);
            let right_limit = if associativity == Associativity::Right {
                precedence
            } else {
                precedence - 1
            };
            if !range || !self.range_end() {
                let right_grammar = if range && grammar == Grammar::Domain {
                    Grammar::Numeric
                } else {
                    grammar
                };
                children.push(SyntaxElement::Node(
                    self.precedence(right_grammar, right_limit)?,
                ));
            }
            left = self.node(
                if range {
                    NodeKind::RangeExpression
                } else {
                    NodeKind::BinaryExpression
                },
                start,
                children,
            );
            previous = Some(precedence);
        }
        Ok(left)
    }

    fn atom_head(
        &mut self,
        grammar: Grammar,
        annotation_head: bool,
    ) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let start = self.position;
        let mut children = Vec::new();
        let kind = match self.peek() {
            Some(Plus | Minus | Not) if grammar != Grammar::Numeric || self.peek() != Some(Not) => {
                let limit = if self.peek() == Some(Not) { 349 } else { 499 };
                self.bump(&mut children);
                let operand = if annotation_head {
                    let operand_start = self.position;
                    let mut operand = self.atom_head(Grammar::General, true)?;
                    while self.peek() == Some(AnnotationMarker) {
                        let annotation = self.annotation()?;
                        operand = self.node(
                            NodeKind::AnnotatedExpression,
                            operand_start,
                            vec![
                                SyntaxElement::Node(operand),
                                SyntaxElement::Node(annotation),
                            ],
                        );
                    }
                    operand
                } else {
                    self.precedence(grammar, limit)?
                };
                children.push(SyntaxElement::Node(operand));
                NodeKind::UnaryExpression
            }
            Some(If) => {
                self.conditional(&mut children)?;
                NodeKind::ConditionalExpression
            }
            Some(Let) => {
                self.let_expression(&mut children)?;
                NodeKind::LetExpression
            }
            Some(LeftParen) => {
                self.bump(&mut children);
                children.push(SyntaxElement::Node(self.precedence(grammar, 1600)?));
                self.expect(
                    RightParen,
                    &mut children,
                    "expected ')' after expression; tuple and record literals are unsupported",
                )?;
                NodeKind::ParenthesizedExpression
            }
            Some(Identifier | QuotedIdentifier) => {
                self.bump(&mut children);
                let inverse_tokens = self.inverse_call_tokens();
                if let Some(count) = inverse_tokens {
                    for _ in 0..count {
                        self.bump(&mut children);
                    }
                }
                if self.peek() == Some(LeftParen) {
                    if inverse_tokens.is_none() && self.after_matching_paren() == Some(LeftParen) {
                        self.generator_call(&mut children)?;
                        NodeKind::GeneratorCallExpression
                    } else {
                        self.call_arguments(&mut children, true)?;
                        NodeKind::CallExpression
                    }
                } else {
                    NodeKind::Expression
                }
            }
            Some(IntegerLiteral | FloatLiteral) => {
                self.bump(&mut children);
                NodeKind::Expression
            }
            Some(StringLiteral | True | False | Infinity | Anonymous | Absent)
                if grammar != Grammar::Numeric =>
            {
                self.bump(&mut children);
                NodeKind::Expression
            }
            Some(StringHead) if grammar != Grammar::Numeric => {
                self.interpolated_string(&mut children)?;
                NodeKind::InterpolatedString
            }
            Some(LeftBrace) if grammar != Grammar::Numeric => {
                if self.collection_entries(&mut children, RightBrace, false)? {
                    NodeKind::SetComprehension
                } else {
                    NodeKind::SetLiteral
                }
            }
            Some(LeftBracket) if grammar != Grammar::Numeric => {
                self.array_entries(&mut children)?
            }
            Some(MatrixStart) if grammar != Grammar::Numeric => {
                self.matrix(&mut children)?;
                NodeKind::MatrixLiteral
            }
            token => return Err(unsupported_expression(token).unwrap_or(
                if grammar == Grammar::Numeric {
                    "expected a numeric expression (identifier, number, call or numeric operator)"
                } else {
                    "expected an expression"
                },
            )),
        };
        let mut node = self.node(kind, start, children);
        while self.peek() == Some(LeftBracket) {
            let mut access = vec![SyntaxElement::Node(node)];
            self.collection_entries(&mut access, RightBracket, true)?;
            node = self.node(NodeKind::ArrayAccessExpression, start, access);
        }
        Ok(node)
    }

    fn interpolated_string(
        &mut self,
        children: &mut Vec<SyntaxElement>,
    ) -> Result<(), &'static str> {
        self.bump(children); // head, including the first written \(
        loop {
            self.expression(children)?;
            match self.peek() {
                Some(TokenKind::StringMiddle) => self.bump(children),
                Some(TokenKind::StringTail) => {
                    self.bump(children);
                    return Ok(());
                }
                _ => return Err("expected ')' after string interpolation expression"),
            }
        }
    }

    // Both productions use general expressions even within numeric atoms.
    // The let body consumes the full expression after `in`.
    // https://docs.minizinc.dev/en/2.10.1/spec.html#let-expressions
    fn conditional(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        use TokenKind::*;
        loop {
            self.trivia(children);
            let start = self.position;
            let mut branch = Vec::new();
            self.bump(&mut branch); // if or elseif
            self.expression(&mut branch)?;
            self.expect(Then, &mut branch, "expected 'then' after condition")?;
            self.expression(&mut branch)?;
            children.push(SyntaxElement::Node(self.node(
                NodeKind::ConditionalBranch,
                start,
                branch,
            )));
            if self.peek() != Some(ElseIf) {
                break;
            }
        }
        if self.peek() == Some(Else) {
            self.trivia(children);
            let start = self.position;
            let mut branch = Vec::new();
            self.bump(&mut branch);
            self.expression(&mut branch)?;
            children.push(SyntaxElement::Node(self.node(
                NodeKind::ElseBranch,
                start,
                branch,
            )));
        }
        self.expect(
            EndIf,
            children,
            "expected 'elseif', 'else' or 'endif' after branch",
        )
    }

    fn let_expression(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        use TokenKind::*;
        self.bump(children);
        self.trivia(children);
        let start = self.position;
        let mut block = Vec::new();
        self.expect(LeftBrace, &mut block, "expected '{' after 'let'")?;
        loop {
            self.trivia(&mut block);
            let item_start = self.position;
            if self.peek() != Some(Constraint) && !self.peek().is_some_and(starts_type) {
                return Err("unsupported local item; expected a declaration or constraint");
            }
            let mut item = Vec::new();
            let kind = self.item_body(&mut item)?;
            if !matches!(kind, NodeKind::Declaration | NodeKind::Constraint) {
                return Err("only declarations and constraints are allowed in a let block");
            }
            block.push(SyntaxElement::Node(self.node(kind, item_start, item)));
            match self.peek() {
                Some(Semicolon | Comma) => self.bump(&mut block),
                Some(RightBrace) => break,
                _ => return Err("expected ';', ',' or '}' after local item"),
            }
            if self.peek() == Some(RightBrace) {
                break;
            }
        }
        self.expect(RightBrace, &mut block, "expected '}' after local items")?;
        children.push(SyntaxElement::Node(self.node(
            NodeKind::LetBlock,
            start,
            block,
        )));
        self.expect(In, children, "expected 'in' after let block")?;
        self.expression(children)
    }

    fn collection_entries(
        &mut self,
        children: &mut Vec<SyntaxElement>,
        closing: TokenKind,
        access: bool,
    ) -> Result<bool, &'static str> {
        use TokenKind::*;
        self.bump(children);
        if access && self.peek() == Some(closing) {
            return Err("expected at least one array index expression");
        }
        let mut entries = 0;
        let mut comprehension = false;
        while self.peek() != Some(closing) {
            if access
                && self.peek().is_some_and(is_range)
                && matches!(self.peek_after(1), Some(Comma | RightBracket))
            {
                self.trivia(children);
                let start = self.position;
                let mut slice = Vec::new();
                self.bump(&mut slice);
                children.push(SyntaxElement::Node(self.node(
                    NodeKind::RangeExpression,
                    start,
                    slice,
                )));
            } else {
                self.expression(children)?;
            }
            entries += 1;
            match self.peek() {
                Some(Pipe) => {
                    if access || entries != 1 {
                        return Err("expected a single comprehension head");
                    }
                    self.bump(children);
                    children.push(SyntaxElement::Node(self.generators(closing)?));
                    comprehension = true;
                    break;
                }
                Some(Colon) => return Err("indexed collection literals are unsupported"),
                Some(Comma) => self.bump(children),
                _ => break,
            }
        }
        self.expect(
            closing,
            children,
            "expected ',' or closing collection delimiter",
        )?;
        Ok(comprehension)
    }

    fn array_entries(
        &mut self,
        children: &mut Vec<SyntaxElement>,
    ) -> Result<NodeKind, &'static str> {
        use TokenKind::*;
        self.bump(children);
        let mut keyed_entries = 0;
        let mut has_bare_entries = false;
        let mut entries = 0;
        let mut kind = NodeKind::ArrayLiteral;
        while self.peek() != Some(RightBracket) {
            self.trivia(children);
            let start = self.position;
            let key = self.array_entry_head()?;
            if self.peek() == Some(Colon) {
                if has_bare_entries {
                    return Err(
                        "written indices must occur on every entry or only the first entry",
                    );
                }
                let mut entry = vec![SyntaxElement::Node(key)];
                self.bump(&mut entry);
                self.expression(&mut entry)?;
                children.push(SyntaxElement::Node(self.node(
                    NodeKind::IndexedArrayEntry,
                    start,
                    entry,
                )));
                keyed_entries += 1;
            } else {
                if keyed_entries > 1 {
                    return Err("expected a written index on this array entry");
                }
                children.push(SyntaxElement::Node(key));
                has_bare_entries = true;
            }
            entries += 1;
            match self.peek() {
                Some(Pipe) => {
                    if entries != 1 {
                        return Err("expected a single comprehension head");
                    }
                    self.bump(children);
                    children.push(SyntaxElement::Node(self.generators(RightBracket)?));
                    kind = if keyed_entries == 1 {
                        NodeKind::IndexedArrayComprehension
                    } else {
                        NodeKind::ArrayComprehension
                    };
                    break;
                }
                Some(Comma) => self.bump(children),
                _ => break,
            }
        }
        self.expect(
            RightBracket,
            children,
            "expected ',' or ']' after array entry",
        )?;
        Ok(kind)
    }

    fn array_entry_head(&mut self) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        if !self.starts_index_tuple() {
            return self.precedence(Grammar::General, 1600);
        }
        let start = self.position;
        let mut children = Vec::new();
        self.bump(&mut children);
        while self.peek() != Some(RightParen) {
            self.expression(&mut children)?;
            if self.peek() != Some(Comma) {
                break;
            }
            self.bump(&mut children);
        }
        self.expect(RightParen, &mut children, "expected ')' after index tuple")?;
        Ok(self.node(NodeKind::IndexTuple, start, children))
    }

    // The tuple-key alternative applies only when the matching ')' is followed
    // by ':'. Other parentheses keep the ordinary expression grammar.
    // https://docs.minizinc.dev/en/2.10.1/spec.html#indexed-array-literals
    fn starts_index_tuple(&self) -> bool {
        self.after_matching_paren() == Some(TokenKind::Colon)
    }

    fn after_matching_paren(&self) -> Option<TokenKind> {
        if self.peek() != Some(TokenKind::LeftParen) {
            return None;
        }
        let mut depth = 0;
        let mut strings = 0;
        for index in self.position..self.tokens.len() {
            match self.tokens[index].kind {
                TokenKind::StringHead => strings += 1,
                TokenKind::StringTail if strings > 0 => strings -= 1,
                TokenKind::StringMiddle | TokenKind::StringTail if strings == 0 => break,
                _ if strings > 0 => {}
                TokenKind::LeftParen => depth += 1,
                TokenKind::RightParen => {
                    depth -= 1;
                    if depth == 0 {
                        return self
                            .significant(index + 1)
                            .map(|next| self.tokens[next].kind);
                    }
                }
                TokenKind::Semicolon => break,
                _ => {}
            }
        }
        None
    }

    fn generators(&mut self, closing: TokenKind) -> Result<SyntaxNode, &'static str> {
        let start = self.position;
        let mut children = Vec::new();
        loop {
            self.trivia(&mut children);
            children.push(SyntaxElement::Node(self.generator()?));
            if self.peek() != Some(TokenKind::Comma) {
                break;
            }
            self.bump(&mut children);
            if self.peek() == Some(closing) {
                break;
            }
        }
        self.trivia(&mut children);
        Ok(self.node(NodeKind::GeneratorList, start, children))
    }

    // One optional filter belongs to each generator, not to the whole tail.
    // https://docs.minizinc.dev/en/2.10.1/spec.html#set-comprehensions
    fn generator(&mut self) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let start = self.position;
        let mut children = Vec::new();
        let mut bindings = 0;
        loop {
            if !matches!(self.peek(), Some(Identifier | QuotedIdentifier | Anonymous)) {
                return Err("expected a generator binding name");
            }
            self.bump(&mut children);
            bindings += 1;
            if self.peek() != Some(Comma) {
                break;
            }
            self.bump(&mut children);
            if self.peek() == Some(In) {
                break;
            }
        }
        match self.peek() {
            Some(In) => self.bump(&mut children),
            Some(Equal) if bindings == 1 => self.bump(&mut children),
            _ => return Err("expected 'in' or a single binding followed by '='"),
        }
        self.expression(&mut children)?;
        if self.peek() == Some(Where) {
            self.trivia(&mut children);
            let filter_start = self.position;
            let mut filter = Vec::new();
            self.bump(&mut filter);
            self.expression(&mut filter)?;
            children.push(SyntaxElement::Node(self.node(
                NodeKind::WhereFilter,
                filter_start,
                filter,
            )));
        }
        Ok(self.node(NodeKind::Generator, start, children))
    }

    fn generator_call(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        use TokenKind::*;
        self.bump(children);
        children.push(SyntaxElement::Node(self.generators(RightParen)?));
        self.expect(RightParen, children, "expected ',' or ')' after generator")?;
        self.trivia(children);
        let start = self.position;
        let mut body = Vec::new();
        self.expect(
            LeftParen,
            &mut body,
            "expected '(' before generator call body",
        )?;
        self.expression(&mut body)?;
        self.expect(
            RightParen,
            &mut body,
            "expected ')' after generator call body",
        )?;
        children.push(SyntaxElement::Node(self.node(
            NodeKind::ParenthesizedExpression,
            start,
            body,
        )));
        Ok(())
    }

    fn matrix(&mut self, children: &mut Vec<SyntaxElement>) -> Result<(), &'static str> {
        use TokenKind::*;
        self.bump(children);
        while self.peek() != Some(MatrixEnd) {
            self.trivia(children);
            let first = !children
                .iter()
                .any(|child| matches!(child, SyntaxElement::Node(_)));
            children.push(SyntaxElement::Node(self.matrix_row_or_header(first)?));
            if self.peek() != Some(Pipe) {
                break;
            }
            self.bump(children);
        }
        self.expect(MatrixEnd, children, "expected '|' or '|]' after matrix row")
    }

    fn matrix_row_or_header(&mut self, first: bool) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        let start = self.position;
        let mut children = Vec::new();
        if self.peek() != Some(Pipe) {
            self.expression(&mut children)?;
            if self.peek() == Some(Colon) {
                self.bump(&mut children);
                if matches!(self.peek(), Some(Pipe | MatrixEnd)) {
                    if !first {
                        return Err("expected a cell after matrix row index");
                    }
                    return Ok(self.node(NodeKind::MatrixColumnIndices, start, children));
                }
                self.expression(&mut children)?;
                if self.peek() == Some(Colon) {
                    if !first {
                        return Err("column indices must precede matrix rows");
                    }
                    loop {
                        self.bump(&mut children);
                        if matches!(self.peek(), Some(Pipe | MatrixEnd)) {
                            return Ok(self.node(NodeKind::MatrixColumnIndices, start, children));
                        }
                        self.expression(&mut children)?;
                        if self.peek() != Some(Colon) {
                            return Err("expected ':' after matrix column index");
                        }
                    }
                }
            }
            while self.peek() == Some(Comma) {
                self.bump(&mut children);
                if matches!(self.peek(), Some(Pipe | MatrixEnd)) {
                    break;
                }
                self.expression(&mut children)?;
            }
        }
        // An empty written row has a zero-width range at its pipe delimiter.
        if children.is_empty() {
            return Ok(SyntaxNode {
                kind: NodeKind::MatrixRow,
                range: self.tokens[start].range.start..self.tokens[start].range.start,
                children,
            });
        }
        Ok(self.node(NodeKind::MatrixRow, start, children))
    }

    fn annotation(&mut self) -> Result<SyntaxNode, &'static str> {
        let start = self.position;
        let mut children = Vec::new();
        self.bump(&mut children);
        children.push(SyntaxElement::Node(self.atom_head(Grammar::General, true)?));
        Ok(self.node(NodeKind::Annotation, start, children))
    }

    fn output_annotation(&mut self) -> Result<SyntaxNode, &'static str> {
        use TokenKind::*;
        // Unlike general annotations, output headers require a string, ordinary
        // call with expression arguments or parenthesized expression. A following output
        // body must not turn the annotation call into a generator call.
        // https://docs.minizinc.dev/en/2.10.1/spec.html#output-items
        let start = self.position;
        let mut children = Vec::new();
        self.bump(&mut children);
        self.trivia(&mut children);
        let value = match self.peek() {
            Some(Identifier | QuotedIdentifier) => {
                let call_start = self.position;
                let mut call = Vec::new();
                self.bump(&mut call);
                if self.peek() != Some(LeftParen) {
                    return Err("expected a call in output annotation");
                }
                self.call_arguments(&mut call, false)?;
                self.node(NodeKind::CallExpression, call_start, call)
            }
            Some(StringLiteral | StringHead | LeftParen) => {
                let value_start = self.position;
                let mut value = Vec::new();
                let kind = match self.peek() {
                    Some(StringLiteral) => {
                        self.bump(&mut value);
                        NodeKind::Expression
                    }
                    Some(StringHead) => {
                        self.interpolated_string(&mut value)?;
                        NodeKind::InterpolatedString
                    }
                    Some(LeftParen) => {
                        self.bump(&mut value);
                        self.expression(&mut value)?;
                        self.expect(
                            RightParen,
                            &mut value,
                            "expected ')' after output annotation",
                        )?;
                        NodeKind::ParenthesizedExpression
                    }
                    _ => unreachable!(),
                };
                self.node(kind, value_start, value)
            }
            _ => return Err("expected a string, call or parenthesized output annotation"),
        };
        children.push(SyntaxElement::Node(value));
        Ok(self.node(NodeKind::Annotation, start, children))
    }

    fn call_arguments(
        &mut self,
        children: &mut Vec<SyntaxElement>,
        allow_named_arguments: bool,
    ) -> Result<(), &'static str> {
        use TokenKind::*;
        self.bump(children);
        while self.peek() != Some(RightParen) {
            self.trivia(children);
            if allow_named_arguments
                && matches!(self.peek(), Some(Identifier | QuotedIdentifier))
                && self.peek_after(1) == Some(Colon)
            {
                let start = self.position;
                let mut argument = Vec::new();
                self.bump(&mut argument);
                self.bump(&mut argument);
                self.expression(&mut argument)?;
                children.push(SyntaxElement::Node(self.node(
                    NodeKind::NamedArgument,
                    start,
                    argument,
                )));
            } else {
                self.expression(children)?;
            }
            if self.peek() != Some(Comma) {
                break;
            }
            self.bump(children);
        }
        self.expect(
            RightParen,
            children,
            "expected ',' or ')' after call argument",
        )
    }

    // Inverse constructor heads are written C⁻¹(...) or C^-1(...). Retain the
    // original tokens; do not invent an identifier or exponent for either form.
    fn inverse_call_tokens(&self) -> Option<usize> {
        if self.peek() == Some(TokenKind::Inverse)
            && self.peek_after(1) == Some(TokenKind::LeftParen)
        {
            return Some(1);
        }
        if self.peek() == Some(TokenKind::Power)
            && self.peek_after(1) == Some(TokenKind::Minus)
            && self.peek_after(2) == Some(TokenKind::IntegerLiteral)
            && self.peek_after(3) == Some(TokenKind::LeftParen)
        {
            let index = self.significant_offset(2)?;
            if &self.source[self.tokens[index].range.clone()] == "1" {
                return Some(3);
            }
        }
        None
    }

    fn range_end(&self) -> bool {
        matches!(
            self.peek(),
            None | Some(
                TokenKind::Semicolon
                    | TokenKind::Comma
                    | TokenKind::Colon
                    | TokenKind::RightParen
                    | TokenKind::RightBracket
                    | TokenKind::RightBrace
                    | TokenKind::Pipe
                    | TokenKind::MatrixEnd
                    | TokenKind::Where
                    | TokenKind::Then
                    | TokenKind::ElseIf
                    | TokenKind::Else
                    | TokenKind::EndIf
                    | TokenKind::StringMiddle
                    | TokenKind::StringTail
            )
        ) || self
            .peek()
            .and_then(operator)
            .is_some_and(|(precedence, _)| precedence > 700)
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

    fn peek_after(&self, offset: usize) -> Option<TokenKind> {
        self.significant_offset(offset)
            .map(|index| self.tokens[index].kind)
    }

    fn significant_offset(&self, offset: usize) -> Option<usize> {
        let mut index = self.significant(self.position)?;
        for _ in 0..offset {
            index = self.significant(index + 1)?;
        }
        Some(index)
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
            .unwrap_or(self.source.len()..self.source.len());
        self.diagnostics.push(Diagnostic {
            range,
            message: message.to_owned(),
        });
    }

    fn recover(&mut self, start: usize) {
        // Before the declaration name, scalar tokens may still belong to an
        // unsupported type. Do not recover to one of those as a new item.
        let can_find_next_item = matches!(
            self.tokens[start].kind,
            TokenKind::Constraint
                | TokenKind::Include
                | TokenKind::Output
                | TokenKind::Solve
                | TokenKind::Function
                | TokenKind::Predicate
                | TokenKind::Test
                | TokenKind::Annotation
        ) || self.tokens[start..self.position]
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Colon | TokenKind::Equal));
        let (mut depth, mut strings) = self.tokens[start..self.position]
            .iter()
            .fold((0, 0), |depth, token| recovery_depth(depth, token.kind));
        // Closed malformed let blocks and interpolations may contain item
        // separators. Consume those until enclosing delimiters close; unmatched
        // delimiters retain the semicolon fallback to reach later items.
        let recover_closed_delimiters = self.tokens[start..self.position]
            .iter()
            .any(|token| matches!(token.kind, TokenKind::Let | TokenKind::StringHead))
            && self.tokens[self.position..]
                .iter()
                .scan((depth, strings), |remaining, token| {
                    *remaining = recovery_depth(*remaining, token.kind);
                    Some(*remaining)
                })
                .any(|remaining| remaining == (0, 0));
        while self.position < self.tokens.len() {
            let kind = self.tokens[self.position].kind;
            if depth == 0 && strings == 0 && can_find_next_item && self.starts_item(self.position) {
                break;
            }
            self.position += 1;
            if kind == TokenKind::Semicolon
                && ((depth == 0 && strings == 0) || !recover_closed_delimiters)
            {
                break;
            }
            (depth, strings) = recovery_depth((depth, strings), kind);
        }
    }

    fn starts_item(&self, position: usize) -> bool {
        use TokenKind::*;
        let mut index = position;
        if matches!(
            self.tokens[index].kind,
            Constraint
                | Include
                | Output
                | Solve
                | Function
                | Predicate
                | Test
                | Annotation
                | Array
                | List
                | Set
                | Opt
        ) {
            return true;
        }
        if matches!(self.tokens[index].kind, Var | Par) {
            let Some(next) = self.significant(index + 1) else {
                return false;
            };
            index = next;
        }
        if matches!(self.tokens[index].kind, Set | Opt) {
            return true;
        }
        if !matches!(
            self.tokens[index].kind,
            Bool | Int | Float | String | Ann | Any | Identifier | QuotedIdentifier
        ) {
            return false;
        }
        self.significant(index + 1)
            .is_some_and(|next| self.tokens[next].kind == Colon)
    }
}

fn recovery_depth((depth, strings): (usize, usize), kind: TokenKind) -> (usize, usize) {
    use TokenKind::*;
    match kind {
        StringHead => (depth, strings + 1),
        StringTail => (depth, strings.saturating_sub(1)),
        // The scanner identifies complete string boundaries even when the
        // embedded expression has mismatched delimiters or item separators.
        _ if strings > 0 => (depth, strings),
        LeftParen | LeftBracket | LeftBrace | MatrixStart => (depth + 1, 0),
        RightParen | RightBracket | RightBrace | MatrixEnd => (depth.saturating_sub(1), 0),
        _ => (depth, 0),
    }
}

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Grammar {
    General,
    Numeric,
    Domain,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Associativity {
    Left,
    Right,
    None,
}

fn operator(kind: TokenKind) -> Option<(u16, Associativity)> {
    use Associativity::{Left, None as NonAssociative, Right};
    use TokenKind::*;
    Some(match kind {
        Equivalence => (1600, Left),
        Implies | ReverseImplies => (1500, Left),
        Or | Xor => (1400, Left),
        And => (1300, Left),
        Less | Greater | LessEqual | GreaterEqual => (1200, NonAssociative),
        Equal | DoubleEqual | NotEqual | WeakEqual | WeakNotEqual => (1100, NonAssociative),
        In | Subset | Superset => (1000, NonAssociative),
        Union | Diff | SymDiff => (900, Left),
        Intersect => (800, Left),
        RangeInclusive | RangeExclusiveStart | RangeExclusiveEnd | RangeExclusive => {
            (700, NonAssociative)
        }
        Plus | Minus | WeakPlus | WeakMinus => (600, Left),
        Star | Slash | Div | Mod | WeakStar | WeakSlash | WeakDiv => (500, Left),
        Power => (400, Left),
        Concat => (300, Right),
        Default => (200, Left),
        InfixIdentifier => (100, Left),
        _ => return Option::None,
    })
}

fn is_numeric_operator(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        Plus | Minus
            | Star
            | Slash
            | Div
            | Mod
            | Power
            | WeakPlus
            | WeakMinus
            | WeakStar
            | WeakSlash
            | WeakDiv
            | InfixIdentifier
    )
}

fn starts_type(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        Identifier
            | TypeInstVariable
            | QuotedIdentifier
            | IntegerLiteral
            | FloatLiteral
            | LeftParen
            | If
            | Let
            | Plus
            | Minus
            | Var
            | Par
            | Opt
            | Bool
            | Int
            | Float
            | String
            | Ann
            | Any
            | Array
            | List
            | Set
            | LeftBrace
            | Anonymous
    ) || is_range(kind)
}

fn is_numeric_expression(node: &SyntaxNode, tokens: &[Token]) -> bool {
    match node.kind {
        NodeKind::CallExpression
        | NodeKind::GeneratorCallExpression
        | NodeKind::ConditionalExpression
        | NodeKind::LetExpression => true,
        NodeKind::ArrayAccessExpression | NodeKind::AnnotatedExpression => node
            .child_nodes()
            .next()
            .is_some_and(|child| is_numeric_expression(child, tokens)),
        NodeKind::Expression
        | NodeKind::UnaryExpression
        | NodeKind::BinaryExpression
        | NodeKind::ParenthesizedExpression => node.children.iter().all(|child| match child {
            SyntaxElement::Node(child) => is_numeric_expression(child, tokens),
            SyntaxElement::Token(index) => {
                let kind = tokens[*index].kind;
                is_trivia(kind)
                    || matches!(
                        kind,
                        TokenKind::Identifier
                            | TokenKind::QuotedIdentifier
                            | TokenKind::IntegerLiteral
                            | TokenKind::FloatLiteral
                            | TokenKind::LeftParen
                            | TokenKind::RightParen
                            | TokenKind::Inverse
                    )
                    || is_numeric_operator(kind)
            }
        }),
        _ => false,
    }
}

fn is_range(kind: TokenKind) -> bool {
    use TokenKind::*;
    matches!(
        kind,
        RangeInclusive | RangeExclusiveStart | RangeExclusiveEnd | RangeExclusive
    )
}

fn unsupported_expression(kind: Option<TokenKind>) -> Option<&'static str> {
    use TokenKind::*;
    match kind? {
        LeftBracket | LeftBrace | MatrixStart | Dot | Pipe | Case | LeftParen => {
            Some("this expression syntax is unsupported")
        }
        _ => None,
    }
}
