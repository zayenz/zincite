use super::*;
use zincite_syntax::{FileMode, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

mod annotations;

type Result<T> = std::result::Result<T, Diagnostic>;

/// Parse and validate the admitted FlatZinc dialect once for repeated checks.
pub fn prepare(name: &str, source: &str) -> std::result::Result<PreparedModel, Vec<Diagnostic>> {
    let parsed = zincite_syntax::parse_flatzinc(source, FileMode::Model);
    let source = Source {
        name,
        parsed: &parsed,
    };
    source.validate_syntax()?;
    Lowerer::new(&source)
        .and_then(Lowerer::lower)
        .map_err(|error| vec![error])
}

/// Accept Boolean, signed integer and finite integer set literals, and arrays.
pub fn parse_assignment(
    name: &str,
    text: &str,
) -> std::result::Result<Assignment, Vec<Diagnostic>> {
    let parsed = zincite_syntax::parse_flatzinc(text, FileMode::Data);
    let source = Source {
        name,
        parsed: &parsed,
    };
    source.validate_syntax()?;
    let mut entries = Vec::new();
    for item in parsed.tree().child_nodes() {
        let entry = || -> Result<AssignmentEntry> {
            if item.kind() != NodeKind::Assignment {
                return Err(source.error(item, "expected a literal assignment"));
            }
            let name = source.name(item)?;
            let expression = item
                .child_nodes()
                .next()
                .ok_or_else(|| source.error(item, "assignment needs a value"))?;
            let value = if expression.kind() == NodeKind::ArrayLiteral {
                AssignmentValue::Array(
                    expression
                        .child_nodes()
                        .map(|node| source.literal(node))
                        .collect::<Result<_>>()?,
                )
            } else {
                AssignmentValue::Scalar(source.literal(expression)?)
            };
            Ok(AssignmentEntry {
                name,
                value,
                location: Some(source.location(item)),
            })
        };
        entries.push(entry().map_err(|error| vec![error])?);
    }
    Ok(Assignment { entries })
}

struct Source<'a> {
    name: &'a str,
    parsed: &'a ParsedFile,
}

impl Source<'_> {
    fn location(&self, node: &SyntaxNode) -> Location {
        let range = node.range();
        let (line, column) = self.parsed.line_column(range.start);
        Location {
            source: self.name.into(),
            start: range.start,
            end: range.end,
            line,
            column,
        }
    }

    fn error(&self, node: &SyntaxNode, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            location: Some(self.location(node)),
            message: message.into(),
        }
    }

    fn validate_syntax(&self) -> std::result::Result<(), Vec<Diagnostic>> {
        let diagnostics: Vec<_> = self
            .parsed
            .diagnostics()
            .iter()
            .map(|diagnostic| {
                let (line, column) = self.parsed.line_column(diagnostic.range.start);
                Diagnostic {
                    location: Some(Location {
                        source: self.name.into(),
                        start: diagnostic.range.start,
                        end: diagnostic.range.end,
                        line,
                        column,
                    }),
                    message: diagnostic.message.clone(),
                }
            })
            .collect();
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }

    fn tokens<'a>(&'a self, node: &'a SyntaxNode) -> impl Iterator<Item = (TokenKind, &'a str)> {
        node.children().iter().filter_map(|element| match element {
            SyntaxElement::Token(index) => {
                let token = &self.parsed.tokens()[*index];
                (!matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                ))
                .then(|| (token.kind, &self.parsed.source()[token.range.clone()]))
            }
            _ => None,
        })
    }

    fn name(&self, node: &SyntaxNode) -> Result<String> {
        if node.kind() == NodeKind::CallExpression {
            let mut tokens = self.tokens(node);
            if !matches!(
                (tokens.next(), tokens.next()),
                (
                    Some((TokenKind::Identifier, _)),
                    Some((TokenKind::LeftParen, _))
                )
            ) {
                return Err(self.error(node, "expected a plain identifier call head"));
            }
        }
        self.tokens(node)
            .find(|(kind, _)| *kind == TokenKind::Identifier)
            .map(|(_, name)| name.into())
            .ok_or_else(|| self.error(node, "expected an unquoted FlatZinc identifier"))
    }

    fn literal(&self, node: &SyntaxNode) -> Result<Value> {
        if node.kind() == NodeKind::Expression {
            match self.tokens(node).next() {
                Some((TokenKind::True, _)) => return Ok(Value::Boolean(true)),
                Some((TokenKind::False, _)) => return Ok(Value::Boolean(false)),
                _ => {}
            }
        }
        if matches!(
            node.kind(),
            NodeKind::SetLiteral | NodeKind::RangeExpression
        ) {
            return self.integer_set(node).map(Value::IntegerSet);
        }
        if self.is_float_literal(node) {
            return self.float(node).map(Value::Float);
        }
        self.integer(node).map(Value::Integer)
    }

    fn is_float_literal(&self, node: &SyntaxNode) -> bool {
        let atom = if node.kind() == NodeKind::UnaryExpression {
            node.child_nodes().next().unwrap_or(node)
        } else {
            node
        };
        atom.kind() == NodeKind::Expression
            && self
                .tokens(atom)
                .next()
                .is_some_and(|(kind, _)| kind == TokenKind::FloatLiteral)
    }

    fn float(&self, node: &SyntaxNode) -> Result<FiniteFloat> {
        let (negative, atom) = if node.kind() == NodeKind::UnaryExpression {
            if !self.tokens(node).map(|(_, text)| text).eq(["-"]) {
                return Err(self.error(node, "expected a signed decimal float literal"));
            }
            (
                true,
                node.child_nodes()
                    .next()
                    .ok_or_else(|| self.error(node, "expected a float literal"))?,
            )
        } else {
            (false, node)
        };
        if atom.kind() != NodeKind::Expression {
            return Err(self.error(node, "expected a decimal float literal"));
        }
        let tokens: Vec<_> = self.tokens(atom).collect();
        let [(TokenKind::FloatLiteral, text)] = tokens.as_slice() else {
            return Err(self.error(node, "expected a decimal float literal"));
        };
        if text.starts_with("0x") {
            return Err(self.error(node, "hexadecimal float literals are unsupported"));
        }
        let value: f64 = text
            .parse()
            .map_err(|_| self.error(node, "invalid decimal float literal"))?;
        FiniteFloat::new(if negative { -value } else { value })
            .map_err(|message| self.error(node, message))
    }

    fn float_interval(&self, node: &SyntaxNode) -> Result<(FiniteFloat, FiniteFloat)> {
        let bounds: Vec<_> = node.child_nodes().collect();
        if node.kind() != NodeKind::RangeExpression
            || bounds.len() != 2
            || !self.tokens(node).map(|(_, text)| text).eq([".."])
        {
            return Err(self.error(node, "expected a literal float interval"));
        }
        Ok((self.float(bounds[0])?, self.float(bounds[1])?))
    }

    fn float_set(&self, node: &SyntaxNode) -> Result<FloatSet> {
        let ranges = if node.kind() == NodeKind::SetLiteral {
            node.child_nodes()
                .map(|value| {
                    if value.kind() == NodeKind::RangeExpression {
                        self.float_interval(value)
                    } else {
                        let value = self.float(value)?;
                        Ok((value, value))
                    }
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            vec![self.float_interval(node)?]
        };
        Ok(FloatSet::from_ranges(ranges))
    }

    fn typed_literal(&self, node: &SyntaxNode, kind: ValueType) -> Result<Value> {
        if kind == ValueType::FloatSet {
            self.float_set(node).map(Value::FloatSet)
        } else {
            self.literal(node)
        }
    }

    fn integer_set(&self, node: &SyntaxNode) -> Result<IntegerSet> {
        if node.kind() == NodeKind::SetLiteral {
            let values = node
                .child_nodes()
                .map(|value| self.integer(value))
                .collect::<Result<Vec<_>>>()?;
            Ok(IntegerSet::from_values(values))
        } else {
            let (lower, upper) = self.interval(node)?;
            Ok(IntegerSet::interval(lower, upper))
        }
    }

    fn predicate_declaration(&self, node: &SyntaxNode, name: &str) -> Result<Vec<TypeSpec>> {
        let scalar = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: false,
        };
        let array = |kind, is_var| ArgumentType {
            kind,
            is_var,
            is_array: true,
        };
        if !name.as_bytes()[0].is_ascii_alphabetic() {
            return Err(self.error(node, "predicate names must start with a letter"));
        }
        let children: Vec<_> = node.child_nodes().collect();
        if children.len() != 1 || children[0].kind() != NodeKind::ParameterList {
            return Err(self.error(
                node,
                "predicate declarations require parameters and cannot have a body or annotations",
            ));
        }
        let parameters: Vec<_> = children[0].child_nodes().collect();
        let expected = if let Some(counting) = CountingName::from_name(name) {
            counting.signature()
        } else if let Some(selection) = SelectionName::from_name(name) {
            selection.signature()
        } else if let Some(set) = SetName::from_name(name) {
            let element = if matches!(set.kind, SetKind::Membership { .. }) {
                parameters
                    .first()
                    .and_then(|parameter| parameter.child_nodes().next())
                    .map(|ty| self.type_spec(ty, true))
                    .transpose()?
                    .map_or(ValueType::Integer, |ty| ty.kind)
            } else {
                ValueType::Integer
            };
            if matches!(set.kind, SetKind::Membership { .. })
                && !matches!(element, ValueType::Integer | ValueType::Boolean)
            {
                return Err(self.error(node, "membership elements must be integer or Boolean"));
            }
            set.signature(element)
        } else if let Some(float) = FloatName::from_name(name) {
            float.signature()
        } else if let Some(boolean) = BooleanName::from_name(name) {
            boolean
                .signature(parameters.len())
                .ok_or_else(|| self.error(node, "predicate declaration has the wrong arity"))?
        } else if let Some(operation) = ArithmeticOperation::from_name(name) {
            let mut arguments = vec![scalar(ValueType::Integer, true); operation.input_count() + 1];
            if operation.has_fixed_exponent() {
                arguments[1].is_var = false;
            }
            arguments
        } else if let Some(relation) = RelationName::from_name(name) {
            let mut arguments = if relation.linear {
                vec![
                    array(ValueType::Integer, false),
                    array(ValueType::Integer, true),
                    scalar(ValueType::Integer, false),
                ]
            } else {
                vec![scalar(ValueType::Integer, true); 2]
            };
            if relation.mode != RelationMode::Plain {
                arguments.push(scalar(ValueType::Boolean, true));
            }
            arguments
        } else {
            match name {
                "fzn_disjunctive_strict" => vec![array(ValueType::Integer, true); 2],
                _ => {
                    return Err(
                        self.error(node, format!("unsupported predicate declaration '{name}'"))
                    );
                }
            }
        };
        if parameters.len() != expected.len() {
            return Err(self.error(node, "predicate declaration has the wrong arity"));
        }
        let mut names = HashSet::new();
        let mut signature = Vec::new();
        for (index, (parameter, expected)) in parameters.iter().zip(expected).enumerate() {
            let formal_name = self.name(parameter)?;
            if !formal_name.as_bytes()[0].is_ascii_alphabetic() || !names.insert(formal_name) {
                return Err(self.error(
                    parameter,
                    "predicate formal names must be distinct and start with a letter",
                ));
            }
            let allow_fixed = (SetName::from_name(name)
                .is_some_and(|set| matches!(set.kind, SetKind::Membership { .. }))
                && index == 1)
                || (name == "bool_lin_le" && index == 2);
            signature.push(self.formal_parameter(parameter, expected, allow_fixed)?);
        }
        Ok(signature)
    }

    fn formal_parameter(
        &self,
        parameter: &SyntaxNode,
        expected: ArgumentType,
        allow_fixed: bool,
    ) -> Result<TypeSpec> {
        let types: Vec<_> = parameter.child_nodes().collect();
        if types.len() != 1
            || !self
                .tokens(parameter)
                .any(|(kind, _)| kind == TokenKind::Colon)
        {
            return Err(self.error(
                parameter,
                "predicate parameters need a type and name without annotations or initialization",
            ));
        }
        let ty = self.type_spec(types[0], true)?;
        let valid_role = ty.is_var == expected.is_var || (allow_fixed && !ty.is_var);
        if ty.kind != expected.kind || !valid_role || ty.is_array != expected.is_array {
            return Err(self.error(parameter, "predicate parameter type, fixed/variable role or array shape does not match the supported operation"));
        }
        Ok(ty)
    }

    fn integer(&self, node: &SyntaxNode) -> Result<i64> {
        let (negative, atom) = if node.kind() == NodeKind::UnaryExpression {
            let negative = match self.tokens(node).next() {
                Some((TokenKind::Minus, _)) => true,
                _ => return Err(self.error(node, "expected a signed integer literal")),
            };
            (
                negative,
                node.child_nodes()
                    .next()
                    .ok_or_else(|| self.error(node, "expected an integer literal"))?,
            )
        } else {
            (false, node)
        };
        if atom.kind() != NodeKind::Expression {
            return Err(self.error(
                node,
                "expected a signed integer literal; computed expressions are unsupported",
            ));
        }
        let Some((TokenKind::IntegerLiteral, text)) = self.tokens(atom).next() else {
            return Err(self.error(node, "expected a signed integer literal"));
        };
        let (digits, radix) = if let Some(digits) = text.strip_prefix("0x") {
            (digits, 16)
        } else if let Some(digits) = text.strip_prefix("0o") {
            (digits, 8)
        } else {
            (text, 10)
        };
        let magnitude = i128::from_str_radix(digits, radix)
            .map_err(|_| self.error(node, "integer is outside the signed 64-bit range"))?;
        i64::try_from(if negative { -magnitude } else { magnitude })
            .map_err(|_| self.error(node, "integer is outside the signed 64-bit range"))
    }

    fn string(&self, node: &SyntaxNode) -> Result<String> {
        let Some((TokenKind::StringLiteral, text)) = self.tokens(node).next() else {
            return Err(self.error(node, "expected a literal string"));
        };
        if node.kind() != NodeKind::Expression {
            return Err(self.error(node, "expected a literal string"));
        }
        let mut decoded = Vec::new();
        let mut chars = text[1..text.len() - 1].chars().peekable();
        while let Some(character) = chars.next() {
            if character != '\\' {
                decoded.extend_from_slice(character.encode_utf8(&mut [0; 4]).as_bytes());
                continue;
            }
            let escape = chars
                .next()
                .ok_or_else(|| self.error(node, "incomplete string escape"))?;
            let value = match escape {
                'a' => '\u{7}',
                'b' => '\u{8}',
                'f' => '\u{c}',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                'v' => '\u{b}',
                '0'..='7' | 'x' => {
                    let radix = if escape == 'x' { 16 } else { 8 };
                    if escape == 'x' && !chars.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
                        decoded.push(b'x');
                        continue;
                    }
                    let mut value = if escape == 'x' {
                        0
                    } else {
                        escape.to_digit(8).unwrap()
                    };
                    let limit = 2;
                    for _ in 0..limit {
                        let Some(digit) = chars.peek().and_then(|c| c.to_digit(radix)) else {
                            break;
                        };
                        chars.next();
                        value = value * radix + digit;
                    }
                    decoded.push(value as u8);
                    continue;
                }
                other => other,
            };
            decoded.extend_from_slice(value.encode_utf8(&mut [0; 4]).as_bytes());
        }
        String::from_utf8(decoded)
            .map_err(|_| self.error(node, "decoded annotation strings must be UTF-8"))
    }

    fn interval(&self, node: &SyntaxNode) -> Result<(i64, i64)> {
        let bounds: Vec<_> = node.child_nodes().collect();
        if node.kind() != NodeKind::RangeExpression
            || bounds.len() != 2
            || !self
                .tokens(node)
                .any(|(kind, _)| kind == TokenKind::RangeInclusive)
        {
            return Err(self.error(node, "expected a literal inclusive integer interval"));
        }
        Ok((self.integer(bounds[0])?, self.integer(bounds[1])?))
    }

    fn integer_array(&self, node: &SyntaxNode) -> Result<Vec<i64>> {
        if node.kind() != NodeKind::ArrayLiteral {
            return Err(self.error(node, "expected a literal integer array"));
        }
        node.child_nodes()
            .map(|value| self.integer(value))
            .collect()
    }
}

#[derive(Clone)]
pub(super) struct ArgumentType {
    pub(super) kind: ValueType,
    pub(super) is_var: bool,
    pub(super) is_array: bool,
}

#[derive(Clone)]
struct TypeSpec {
    kind: ValueType,
    domain: Domain,
    is_var: bool,
    length: Option<usize>,
    is_array: bool,
}

impl Source<'_> {
    fn type_spec(&self, node: &SyntaxNode, formal: bool) -> Result<TypeSpec> {
        if node.kind() == NodeKind::ArrayType {
            let children: Vec<_> = node.child_nodes().collect();
            if children.len() != 2 {
                return Err(self.error(
                    node,
                    "only one-dimensional arrays indexed by literal 1..n are supported",
                ));
            }
            if children[1].kind() == NodeKind::ArrayType {
                return Err(self.error(node, "nested arrays are unsupported"));
            }
            if formal
                && children[0].kind() == NodeKind::ScalarType
                && self.tokens(children[0]).map(|(_, text)| text).eq(["int"])
            {
                let mut element = self.type_spec(children[1], formal)?;
                element.is_array = true;
                return Ok(element);
            }
            if children[0].kind() != NodeKind::DomainType
                || self.tokens(children[0]).next().is_some()
            {
                return Err(
                    self.error(children[0], "array indexes require a literal 1..n interval")
                );
            }
            let index = children[0]
                .child_nodes()
                .next()
                .ok_or_else(|| self.error(children[0], "expected array index interval"))?;
            let (lower, upper) = self.interval(index)?;
            if lower != 1 || upper < 0 {
                return Err(self.error(index, "array indexes must be 1..n, with n >= 0"));
            }
            let mut element = self.type_spec(children[1], formal)?;
            if element.is_array {
                return Err(self.error(node, "nested arrays are unsupported"));
            }
            element.is_array = true;
            element.length = Some(usize::try_from(upper).map_err(|_| {
                self.error(index, "array length exceeds this target's addressable size")
            })?);
            return Ok(element);
        }
        let tokens: Vec<_> = self.tokens(node).map(|(kind, _)| kind).collect();
        let is_var = tokens.first() == Some(&TokenKind::Var);
        let base_tokens = if is_var { &tokens[1..] } else { &tokens[..] };
        let valid_tokens = match node.kind() {
            NodeKind::ScalarType => matches!(
                base_tokens,
                [TokenKind::Int] | [TokenKind::Bool] | [TokenKind::Float]
            ),
            NodeKind::SetType => base_tokens == [TokenKind::Set, TokenKind::Of],
            NodeKind::DomainType => base_tokens.is_empty(),
            _ => false,
        };
        if !valid_tokens {
            return Err(self.error(node, "unsupported type in FlatZinc"));
        }

        let (kind, domain) = match node.kind() {
            NodeKind::ScalarType if tokens.contains(&TokenKind::Int) => {
                (ValueType::Integer, Domain::Integer)
            }
            NodeKind::ScalarType if tokens.contains(&TokenKind::Float) => {
                (ValueType::Float, Domain::Float)
            }
            NodeKind::ScalarType if tokens.contains(&TokenKind::Bool) => {
                (ValueType::Boolean, Domain::Boolean)
            }
            NodeKind::SetType => {
                let element = node
                    .child_nodes()
                    .next()
                    .ok_or_else(|| self.error(node, "expected an integer set type"))?;
                if element.kind() == NodeKind::ScalarType
                    && self.tokens(element).map(|(_, text)| text).eq(["float"])
                {
                    if is_var {
                        return Err(self.error(node, "assignable float sets are unsupported"));
                    }
                    return Ok(TypeSpec {
                        kind: ValueType::FloatSet,
                        domain: Domain::FloatSet,
                        is_var,
                        length: None,
                        is_array: false,
                    });
                }
                let upper = if element.kind() == NodeKind::ScalarType
                    && self.tokens(element).map(|(_, text)| text).eq(["int"])
                {
                    None
                } else if element.kind() == NodeKind::DomainType {
                    let expression = element
                        .child_nodes()
                        .next()
                        .ok_or_else(|| self.error(element, "expected an upper set domain"))?;
                    Some(self.integer_set(expression)?)
                } else {
                    return Err(self.error(node, "only finite integer sets are supported"));
                };
                (ValueType::IntegerSet, Domain::IntegerSet { upper })
            }
            NodeKind::DomainType => {
                let expression = node
                    .child_nodes()
                    .next()
                    .ok_or_else(|| self.error(node, "expected a literal integer domain"))?;
                if expression.kind() == NodeKind::RangeExpression
                    && expression
                        .child_nodes()
                        .any(|bound| self.is_float_literal(bound))
                {
                    let (lower, upper) = self.float_interval(expression)?;
                    return Ok(TypeSpec {
                        kind: ValueType::Float,
                        domain: Domain::FloatInterval { lower, upper },
                        is_var,
                        length: None,
                        is_array: false,
                    });
                }
                let domain = if expression.kind() == NodeKind::SetLiteral {
                    let mut values: Vec<_> = expression
                        .child_nodes()
                        .map(|value| self.integer(value))
                        .collect::<Result<_>>()?;
                    values.sort_unstable();
                    values.dedup();
                    Domain::Enumeration { values }
                } else {
                    let (lower, upper) = self.interval(expression)?;
                    Domain::Interval { lower, upper }
                };
                (ValueType::Integer, domain)
            }
            _ => {
                return Err(self.error(
                    node,
                    "unsupported type; expected Boolean, integer or finite integer set values",
                ));
            }
        };
        Ok(TypeSpec {
            kind,
            domain,
            is_var,
            length: None,
            is_array: false,
        })
    }
}

struct Declaration<'a> {
    node: &'a SyntaxNode,
    name: String,
    ty: TypeSpec,
    initializer: Option<&'a SyntaxNode>,
    metadata: Metadata,
    annotations: Vec<&'a SyntaxNode>,
}

#[derive(Clone, Copy)]
enum AnnotationContext {
    Value {
        is_array: bool,
        is_var: bool,
        length: Option<usize>,
    },
    Constraint,
    Solve,
}

impl<'a> Source<'a> {
    fn declaration(&self, node: &'a SyntaxNode) -> Result<Declaration<'a>> {
        let children: Vec<_> = node.child_nodes().collect();
        let ty = self.type_spec(children[0], false)?;
        let annotations: Vec<_> = children
            .iter()
            .copied()
            .filter(|node| node.kind() == NodeKind::Annotation)
            .collect();
        if !ty.is_var
            && (!annotations.is_empty()
                || !matches!(
                    ty.domain,
                    Domain::Float
                        | Domain::FloatSet
                        | Domain::Boolean
                        | Domain::Integer
                        | Domain::IntegerSet { upper: None }
                ))
        {
            return Err(self.error(
                node,
                "parameters require a basic parameter type and cannot have annotations",
            ));
        }
        if self
            .tokens(node)
            .any(|(kind, _)| kind == TokenKind::DoubleEqual)
        {
            return Err(self.error(node, "FlatZinc initialization uses '='"));
        }
        let initializer = children
            .iter()
            .copied()
            .skip(1)
            .find(|node| node.kind() != NodeKind::Annotation);
        Ok(Declaration {
            node,
            name: self.name(node)?,
            ty,
            initializer,
            metadata: Metadata::default(),
            annotations,
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ItemPhase {
    Predicates,
    Parameters,
    Variables,
    Constraints,
    Solve,
}

struct Lowerer<'a> {
    source: &'a Source<'a>,
    declarations: Vec<Declaration<'a>>,
    names: HashMap<String, usize>,
    resolved: Vec<Option<Binding>>,
    model: PreparedModel,
    predicates: HashMap<String, Vec<TypeSpec>>,
}

impl<'a> Lowerer<'a> {
    fn new(source: &'a Source<'a>) -> Result<Self> {
        let mut declarations = Vec::new();
        let mut names = HashMap::new();
        let mut predicates = HashMap::new();
        let mut phase = ItemPhase::Predicates;
        for node in source.parsed.tree().child_nodes() {
            let next = match node.kind() {
                NodeKind::PredicateDeclaration => ItemPhase::Predicates,
                NodeKind::Declaration => {
                    let ty = source.type_spec(node.child_nodes().next().unwrap(), false)?;
                    if ty.is_var {
                        ItemPhase::Variables
                    } else {
                        ItemPhase::Parameters
                    }
                }
                NodeKind::Constraint => ItemPhase::Constraints,
                NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize => {
                    ItemPhase::Solve
                }
                _ => return Err(source.error(node, "unsupported FlatZinc item")),
            };
            if next < phase || phase == ItemPhase::Solve {
                return Err(source.error(node, "FlatZinc items must follow predicate, parameter, variable, constraint, solve order"));
            }
            phase = next;
            if node.kind() == NodeKind::PredicateDeclaration {
                let name = source.name(node)?;
                let signature = source.predicate_declaration(node, &name)?;
                if predicates.insert(name.clone(), signature).is_some() {
                    return Err(
                        source.error(node, format!("duplicate predicate declaration '{name}'"))
                    );
                }
                continue;
            }
            if node.kind() != NodeKind::Declaration {
                continue;
            }
            let declaration = source.declaration(node)?;
            if names
                .insert(declaration.name.clone(), declarations.len())
                .is_some()
            {
                return Err(source.error(
                    node,
                    format!("duplicate declaration '{}'", declaration.name),
                ));
            }
            declarations.push(declaration);
        }
        let resolved = vec![None; declarations.len()];
        Ok(Self {
            source,
            predicates,
            declarations,
            names,
            resolved,
            model: PreparedModel {
                slots: Vec::new(),
                bindings: HashMap::new(),
                variables: Vec::new(),
                constraints: Vec::new(),
                definitions: Vec::new(),
                dependents: Vec::new(),
                objective: None,
                float_tolerance: FloatTolerance::default(),
            },
        })
    }

    fn lower(mut self) -> Result<PreparedModel> {
        for index in 0..self.declarations.len() {
            self.resolve(index)?;
        }
        let mut solve = false;
        for node in self.source.parsed.tree().child_nodes() {
            match node.kind() {
                NodeKind::Declaration | NodeKind::PredicateDeclaration => {}
                NodeKind::Constraint => self.constraint(node)?,
                NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize => {
                    if solve { return Err(self.source.error(node, "expected exactly one solve item")); }
                    solve = true;
                    self.solve(node)?;
                }
                _ => return Err(self.source.error(node, "unsupported FlatZinc item; expected a declaration, direct builtin constraint or solve item")),
            }
        }
        if !solve {
            return Err(self
                .source
                .error(self.source.parsed.tree(), "expected exactly one solve item"));
        }
        self.model.prepare_definitions()?;
        self.model.variables.sort_by_key(|variable| {
            (
                variable.reference.location.start,
                variable.reference.position,
            )
        });
        self.model.link_aliases();
        Ok(self.model)
    }

    fn resolve(&mut self, index: usize) -> Result<Binding> {
        if let Some(binding) = &self.resolved[index] {
            return Ok(binding.clone());
        }
        let mut ty = self.declarations[index].ty.clone();
        let annotations = self.declarations[index].annotations.clone();
        let context = AnnotationContext::Value {
            is_array: ty.is_array,
            is_var: ty.is_var,
            length: ty.length,
        };
        self.declarations[index].metadata = self.metadata(&annotations, context)?;
        if annotations.iter().any(|annotation| {
            annotation
                .child_nodes()
                .next()
                .is_some_and(|node| self.source.name(node).as_deref() == Ok("computed_domain"))
        }) {
            ty.domain = match ty.kind {
                ValueType::Float => Domain::Float,
                ValueType::FloatSet => Domain::FloatSet,
                ValueType::Integer => Domain::Integer,
                ValueType::Boolean => Domain::Boolean,
                ValueType::IntegerSet => Domain::IntegerSet { upper: None },
            };
        }
        let mut elements = self.initial_values(index, &ty)?;
        if ty.is_var {
            self.bind_variables(index, &ty, &mut elements);
        } else {
            self.validate_parameter(index, &ty, &elements)?;
        }
        let binding = Binding {
            elements,
            is_array: ty.length.is_some(),
            kind: ty.kind,
            assignable: ty.is_var,
        };
        self.model
            .bindings
            .insert(self.declarations[index].name.clone(), binding.clone());
        self.resolved[index] = Some(binding.clone());
        Ok(binding)
    }

    fn initial_values(&mut self, index: usize, ty: &TypeSpec) -> Result<Vec<Operand>> {
        let declaration = &self.declarations[index];
        if !ty.is_var {
            let initializer = declaration.initializer.ok_or_else(|| {
                self.source.error(
                    declaration.node,
                    "parameters require literal initialization",
                )
            })?;
            let values = if ty.is_array {
                if initializer.kind() != NodeKind::ArrayLiteral {
                    return Err(self.source.error(
                        initializer,
                        "parameter arrays require literal initialization",
                    ));
                }
                initializer
                    .child_nodes()
                    .map(|node| {
                        self.source
                            .typed_literal(node, ty.kind)
                            .map(Operand::Literal)
                    })
                    .collect::<Result<Vec<_>>>()?
            } else {
                vec![Operand::Literal(
                    self.source.typed_literal(initializer, ty.kind)?,
                )]
            };
            if ty.length.is_some_and(|length| length != values.len()) {
                return Err(self.source.error(
                    initializer,
                    "array initializer length does not match its index range",
                ));
            }
            if values.iter().any(|operand| !matches!(operand, Operand::Literal(value) if value.value_type() == ty.kind)) {
                return Err(self.source.error(initializer, "parameter initializer has the wrong type"));
            }
            return Ok(values);
        }
        match (ty.length, declaration.initializer) {
            (Some(length), Some(expression)) if expression.kind() == NodeKind::ArrayLiteral => {
                let values: Vec<_> = expression
                    .child_nodes()
                    .map(|value| self.scalar(value, ty.kind))
                    .collect::<Result<_>>()?;
                if values.len() != length {
                    return Err(self.source.error(
                        expression,
                        "array initializer length does not match its index range",
                    ));
                }
                Ok(values)
            }
            (Some(length), Some(expression)) if expression.kind() == NodeKind::Expression => {
                let binding = self.reference(expression)?;
                if !binding.is_array || binding.kind != ty.kind || binding.elements.len() != length
                {
                    return Err(self
                        .source
                        .error(expression, "array alias type or shape mismatch"));
                }
                Ok(binding.elements)
            }
            (Some(_), _) => Err(self.source.error(
                declaration.node,
                "arrays require a literal initializer or array alias",
            )),
            (None, Some(expression)) => Ok(vec![self.scalar(expression, ty.kind)?]),
            (None, None) if ty.is_var => Ok(Vec::new()),
            (None, None) => Err(self
                .source
                .error(declaration.node, "parameters require a fixed initializer")),
        }
    }

    fn validate_parameter(&self, index: usize, ty: &TypeSpec, elements: &[Operand]) -> Result<()> {
        let node = self.declarations[index].node;
        for operand in elements {
            let Operand::Literal(value) = operand else {
                return Err(self
                    .source
                    .error(node, "a parameter cannot refer to a variable"));
            };
            if !ty.domain.contains(value) {
                return Err(self
                    .source
                    .error(node, "parameter value is outside its declared domain"));
            }
        }
        Ok(())
    }

    fn bind_variables(&mut self, index: usize, ty: &TypeSpec, elements: &mut Vec<Operand>) {
        if elements.is_empty() && ty.length.is_none() {
            let reference = self.variable_ref(index, None);
            let slot = self.model.slots.len();
            self.model.slots.push(Slot {
                kind: ty.kind,
                default: ty.domain.singleton(),
                reference,
            });
            elements.push(Operand::Variable(slot));
        }
        for (position, operand) in elements.iter_mut().enumerate() {
            let reference = self.variable_ref(index, ty.length.map(|_| position + 1));
            let mut domains = vec![ty.domain.clone()];
            let slot = match operand {
                Operand::Literal(value) => {
                    domains.push(Domain::Fixed {
                        value: value.clone(),
                    });
                    let slot = self.model.slots.len();
                    self.model.slots.push(Slot {
                        kind: ty.kind,
                        default: Some(value.clone()),
                        reference: reference.clone(),
                    });
                    *operand = Operand::Variable(slot);
                    slot
                }
                Operand::Variable(slot) => {
                    if self.model.slots[*slot].default.is_none() {
                        self.model.slots[*slot].default = ty.domain.singleton();
                    }
                    *slot
                }
            };
            self.model.variables.push(Variable {
                reference,
                slot,
                domains,
            });
        }
    }

    fn variable_ref(&self, index: usize, position: Option<usize>) -> VariableRef {
        let declaration = &self.declarations[index];
        VariableRef {
            name: declaration.name.clone(),
            position,
            location: self.source.location(declaration.node),
            metadata: declaration.metadata.clone(),
            aliases: Vec::new(),
        }
    }

    fn reference(&mut self, node: &SyntaxNode) -> Result<Binding> {
        let name = self.source.name(node)?;
        let index = self.names.get(&name).copied().ok_or_else(|| {
            self.source
                .error(node, format!("unknown reference '{name}'"))
        })?;
        self.resolved[index].clone().ok_or_else(|| {
            self.source.error(
                node,
                format!("reference '{name}' must follow its declaration"),
            )
        })
    }

    fn scalar(&mut self, node: &SyntaxNode, kind: ValueType) -> Result<Operand> {
        let operand = if node.kind() == NodeKind::ArrayAccessExpression {
            let children: Vec<_> = node.child_nodes().collect();
            if children.len() != 2 || children[0].kind() != NodeKind::Expression {
                return Err(self.source.error(node, "expected one literal array index"));
            }
            let binding = self.reference(children[0])?;
            let index = self.source.integer(children[1])?;
            let position = usize::try_from(index).ok().and_then(|n| n.checked_sub(1));
            if !binding.is_array || binding.kind != kind {
                return Err(self
                    .source
                    .error(node, "array access has the wrong type or shape"));
            }
            position
                .and_then(|position| binding.elements.get(position))
                .cloned()
                .ok_or_else(|| self.source.error(node, "array index is outside 1..n"))?
        } else if node.kind() == NodeKind::Expression
            && self
                .source
                .tokens(node)
                .next()
                .is_some_and(|(token, _)| token == TokenKind::Identifier)
        {
            let binding = self.reference(node)?;
            if binding.is_array || binding.kind != kind {
                return Err(self.source.error(
                    node,
                    "reference type or shape does not match scalar argument",
                ));
            }
            binding.elements[0].clone()
        } else {
            Operand::Literal(self.source.typed_literal(node, kind)?)
        };
        let actual = match &operand {
            Operand::Literal(value) => value.value_type(),
            Operand::Variable(slot) => self.model.slots[*slot].kind,
        };
        if actual != kind {
            return Err(self.source.error(node, "scalar value has the wrong type"));
        }
        Ok(operand)
    }

    fn array(&mut self, node: &SyntaxNode, kind: ValueType) -> Result<Vec<Operand>> {
        if node.kind() == NodeKind::ArrayLiteral {
            return node
                .child_nodes()
                .map(|value| self.scalar(value, kind))
                .collect();
        }
        if node.kind() != NodeKind::Expression {
            return Err(self
                .source
                .error(node, "expected a literal array or declared array reference"));
        }
        let binding = self.reference(node)?;
        if !binding.is_array || binding.kind != kind {
            return Err(self
                .source
                .error(node, "array argument has the wrong type or shape"));
        }
        Ok(binding.elements)
    }

    fn constant_integer(&mut self, node: &SyntaxNode) -> Result<i64> {
        match self.scalar(node, ValueType::Integer)? {
            Operand::Literal(Value::Integer(value)) => Ok(value),
            _ => Err(self
                .source
                .error(node, "expected a fixed integer parameter")),
        }
    }

    fn constraint(&mut self, node: &SyntaxNode) -> Result<()> {
        let expression = node
            .child_nodes()
            .next()
            .ok_or_else(|| self.source.error(node, "expected a builtin predicate call"))?;
        let mut annotations = Vec::new();
        let call = unannotated(expression, &mut annotations);
        if call.kind() != NodeKind::CallExpression {
            return Err(self
                .source
                .error(call, "expected a direct builtin predicate call"));
        }
        let name = self.source.name(call)?;
        let args: Vec<_> = call.child_nodes().collect();
        let boolean = BooleanName::from_name(&name);
        let counting = CountingName::from_name(&name);
        let float = FloatName::from_name(&name);
        let selection = SelectionName::from_name(&name);
        let set = SetName::from_name(&name);
        let arity = if let Some(counting) = counting {
            counting.signature().len()
        } else if let Some(selection) = selection {
            selection.arity()
        } else if let Some(set) = set {
            set.signature(ValueType::Integer).len()
        } else if let Some(float) = float {
            float.signature().len()
        } else if let Some(boolean) = boolean {
            boolean.mode_for_arity(args.len()).ok_or_else(|| {
                self.source
                    .error(call, format!("'{name}' has an unsupported argument count"))
            })?;
            args.len()
        } else if let Some(relation) = RelationName::from_name(&name) {
            relation.arity()
        } else {
            match name.as_str() {
                "fzn_disjunctive_strict" => 2,
                _ => ArithmeticOperation::from_name(&name)
                    .map(|operation| operation.input_count() + 1)
                    .ok_or_else(|| {
                        self.source
                            .error(call, format!("unsupported predicate '{name}'"))
                    })?,
            }
        };
        if args.len() != arity {
            return Err(self
                .source
                .error(call, format!("'{name}' expects {arity} arguments")));
        }
        let predicate = if let Some(counting) = counting {
            self.counting_predicate(call, counting, &args)?
        } else if let Some(selection) = selection {
            self.selection_predicate(call, selection, &args)?
        } else if let Some(set) = set {
            self.set_predicate(set, &args)?
        } else if let Some(float) = float {
            self.float_predicate(call, float, &args)?
        } else if let Some(boolean) = boolean {
            self.boolean_predicate(call, boolean, &args)?
        } else if let Some(operation) = ArithmeticOperation::from_name(&name) {
            self.integer_arithmetic(operation, &args)?
        } else if let Some(relation) = RelationName::from_name(&name) {
            self.integer_relation(call, relation, &args)?
        } else {
            match name.as_str() {
                "fzn_disjunctive_strict" => self.strict_disjunctive(call, &args)?,
                _ => {
                    return Err(self
                        .source
                        .error(call, format!("unsupported predicate '{name}'")));
                }
            }
        };
        self.validate_declared_call(call, &name, &args)?;
        let metadata = self.metadata(&annotations, AnnotationContext::Constraint)?;
        let reference = ConstraintRef {
            ordinal: self.model.constraints.len() + 1,
            predicate: name,
            location: self.source.location(node),
            metadata,
        };
        self.model.constraints.push(Constraint {
            reference,
            predicate,
        });
        Ok(())
    }

    fn counting_predicate(
        &mut self,
        call: &SyntaxNode,
        name: CountingName,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        use counting::{CardinalityCounts, CountSelector};
        let fixed_ints = |this: &mut Self, node| -> Result<Vec<i64>> {
            Ok(this
                .fixed_array(node, ValueType::Integer)?
                .iter()
                .map(Value::integer)
                .collect())
        };
        let counting = match name {
            CountingName::Count(mode) => Counting::Count {
                entries: self.array(args[0], ValueType::Integer)?,
                selector: CountSelector::Needle(self.scalar(args[1], ValueType::Integer)?),
                count: self.scalar(args[2], ValueType::Integer)?,
                relation: IntegerRelation::Equal,
                condition: match mode {
                    RelationMode::Plain => RelationCondition::Plain,
                    RelationMode::Reified => RelationCondition::Reified {
                        result: self.scalar(args[3], ValueType::Boolean)?,
                    },
                    RelationMode::Implied => RelationCondition::Implied {
                        guard: self.scalar(args[3], ValueType::Boolean)?,
                    },
                },
                code: None,
            },
            CountingName::AtLeast | CountingName::AtMost => Counting::Count {
                count: Operand::Literal(Value::Integer(self.constant_integer(args[0])?)),
                entries: self.array(args[1], ValueType::Integer)?,
                selector: CountSelector::Needle(Operand::Literal(Value::Integer(
                    self.constant_integer(args[2])?,
                ))),
                relation: if name == CountingName::AtLeast {
                    IntegerRelation::GreaterEqual
                } else {
                    IntegerRelation::LessEqual
                },
                condition: RelationCondition::Plain,
                code: None,
            },
            CountingName::Among | CountingName::Nvalue => Counting::Count {
                count: self.scalar(args[0], ValueType::Integer)?,
                entries: self.array(args[1], ValueType::Integer)?,
                selector: if name == CountingName::Among {
                    let Operand::Literal(Value::IntegerSet(set)) =
                        self.scalar(args[2], ValueType::IntegerSet)?
                    else {
                        return Err(self.source.error(args[2], "among requires a fixed set"));
                    };
                    CountSelector::Members(set)
                } else {
                    CountSelector::Distinct
                },
                relation: IntegerRelation::Equal,
                condition: RelationCondition::Plain,
                code: None,
            },
            CountingName::Coded { matches } => {
                let code = self.constant_integer(args[2])?;
                let relation = relations::coded_relation(code).ok_or_else(|| {
                    self.source
                        .error(args[2], "count relation code must be in 1..6")
                })?;
                Counting::Count {
                    entries: self.array(args[0], ValueType::Integer)?,
                    selector: if matches {
                        CountSelector::Matches(fixed_ints(self, args[1])?)
                    } else {
                        CountSelector::Needle(self.scalar(args[1], ValueType::Integer)?)
                    },
                    count: self.scalar(args[3], ValueType::Integer)?,
                    relation,
                    condition: RelationCondition::Plain,
                    code: Some(code),
                }
            }
            CountingName::Cardinality { closed, bounded } => Counting::Cardinality {
                entries: self.array(args[0], ValueType::Integer)?,
                cover: fixed_ints(self, args[1])?,
                counts: if bounded {
                    CardinalityCounts::Bounds {
                        lower: fixed_ints(self, args[2])?,
                        upper: fixed_ints(self, args[3])?,
                    }
                } else {
                    CardinalityCounts::Exact(self.array(args[2], ValueType::Integer)?)
                },
                closed,
            },
            CountingName::Window { boolean } => {
                let membership_type = if boolean {
                    ValueType::Boolean
                } else {
                    ValueType::IntegerSet
                };
                let Operand::Literal(membership) = self.scalar(args[1], membership_type)? else {
                    return Err(self
                        .source
                        .error(args[1], "count windows require fixed membership"));
                };
                Counting::Window {
                    entries: self.array(
                        args[0],
                        if boolean {
                            ValueType::Boolean
                        } else {
                            ValueType::Integer
                        },
                    )?,
                    membership,
                    length: self.constant_integer(args[2])?,
                    lower: self.constant_integer(args[3])?,
                    upper: self.constant_integer(args[4])?,
                }
            }
        };
        counting
            .validate_shape()
            .map_err(|message| self.source.error(call, message))?;
        Ok(Predicate::Counting(counting))
    }

    fn set_predicate(&mut self, name: SetName, args: &[&SyntaxNode]) -> Result<Predicate> {
        let condition = match name.mode {
            RelationMode::Plain => RelationCondition::Plain,
            RelationMode::Reified => RelationCondition::Reified {
                result: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
            RelationMode::Implied => RelationCondition::Implied {
                guard: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
        };
        match name.kind {
            SetKind::Membership { fixed } => {
                let element = self
                    .scalar(args[0], ValueType::Integer)
                    .or_else(|_| self.scalar(args[0], ValueType::Boolean))?;
                let set = self.scalar(args[1], ValueType::IntegerSet)?;
                let boolean = matches!(&element, Operand::Literal(Value::Boolean(_)))
                    || matches!(&element, Operand::Variable(slot) if self.model.slots[*slot].kind == ValueType::Boolean);
                if (fixed || boolean) && !matches!(set, Operand::Literal(_)) {
                    return Err(self
                        .source
                        .error(args[1], "this membership form requires a fixed integer set"));
                }
                Ok(if name.mode == RelationMode::Plain {
                    Predicate::SetIn { element, set }
                } else {
                    Predicate::SetMembership {
                        element,
                        set,
                        condition,
                    }
                })
            }
            SetKind::Relation(relation) => {
                let left = self.scalar(args[0], ValueType::IntegerSet)?;
                let right = self.scalar(args[1], ValueType::IntegerSet)?;
                Ok(match (relation, name.mode) {
                    (SetRelation::Equal, RelationMode::Plain) => {
                        Predicate::SetEqual { left, right }
                    }
                    (SetRelation::Subset, RelationMode::Plain) => {
                        Predicate::SetSubset { left, right }
                    }
                    _ => Predicate::SetRelation {
                        relation,
                        left,
                        right,
                        condition,
                        native_code: None,
                    },
                })
            }
            SetKind::NativeRelation => {
                let code = self.constant_integer(args[1])?;
                let relation = SetRelation::from_code(code).ok_or_else(|| {
                    self.source
                        .error(args[1], "set relation code must be in 1..10")
                })?;
                Ok(Predicate::SetRelation {
                    relation,
                    left: self.scalar(args[0], ValueType::IntegerSet)?,
                    right: self.scalar(args[2], ValueType::IntegerSet)?,
                    condition,
                    native_code: Some(code),
                })
            }
            SetKind::Cardinality => Ok(Predicate::SetCardinality {
                set: self.scalar(args[0], ValueType::IntegerSet)?,
                cardinality: self.scalar(args[1], ValueType::Integer)?,
            }),
            SetKind::Operation(operation) => {
                let left = self.scalar(args[0], ValueType::IntegerSet)?;
                let right = self.scalar(args[1], ValueType::IntegerSet)?;
                let result = self.scalar(args[2], ValueType::IntegerSet)?;
                Ok(match operation {
                    SetOperation::Union => Predicate::SetUnion {
                        left,
                        right,
                        result,
                    },
                    SetOperation::Intersection => Predicate::SetIntersection {
                        left,
                        right,
                        result,
                    },
                    _ => Predicate::SetOperation {
                        operation,
                        left,
                        right,
                        result,
                    },
                })
            }
        }
    }

    fn selection_predicate(
        &mut self,
        call: &SyntaxNode,
        name: SelectionName,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        if name.form == SelectionForm::Conditional {
            return Ok(Predicate::Conditional {
                condition: self.scalar(args[0], ValueType::Boolean)?,
                then_value: self.scalar(args[1], name.kind)?,
                else_value: self.scalar(args[2], name.kind)?,
                result: self.scalar(args[3], name.kind)?,
            });
        }
        struct ElementArguments<'a> {
            indices: Vec<&'a SyntaxNode>,
            array: &'a SyntaxNode,
            result: &'a SyntaxNode,
            layout: ElementLayout,
        }
        let element = match name.form {
            SelectionForm::OneBased => ElementArguments {
                indices: vec![args[0]],
                array: args[1],
                result: args[2],
                layout: ElementLayout::OneBased,
            },
            SelectionForm::Offset => ElementArguments {
                indices: vec![args[0]],
                array: args[2],
                result: args[3],
                layout: ElementLayout::Offset {
                    offset: self.constant_integer(args[1])?,
                },
            },
            SelectionForm::Matrix => {
                let mut fixed_set = |node| match self.scalar(node, ValueType::IntegerSet)? {
                    Operand::Literal(Value::IntegerSet(set)) => Ok(set),
                    _ => Err(self
                        .source
                        .error(node, "expected a fixed integer index set")),
                };
                ElementArguments {
                    indices: vec![args[0], args[1]],
                    array: args[2],
                    result: args[5],
                    layout: ElementLayout::Matrix {
                        rows: fixed_set(args[3])?,
                        columns: fixed_set(args[4])?,
                    },
                }
            }
            SelectionForm::SetMatrix => ElementArguments {
                indices: vec![args[3], args[1]],
                array: args[0],
                result: args[5],
                layout: ElementLayout::SetMatrix {
                    width: self.constant_integer(args[2])?,
                    height: self.constant_integer(args[4])?,
                },
            },
            SelectionForm::Conditional => unreachable!("conditional handled above"),
        };
        let elements = if name.allows_variables {
            self.array(element.array, name.kind)?
        } else {
            self.fixed_array(element.array, name.kind)?
                .into_iter()
                .map(Operand::Literal)
                .collect()
        };
        element
            .layout
            .validate(elements.len())
            .map_err(|message| self.source.error(call, message))?;
        Ok(Predicate::Element {
            indices: element
                .indices
                .into_iter()
                .map(|index| self.scalar(index, ValueType::Integer))
                .collect::<Result<_>>()?,
            elements,
            result: self.scalar(element.result, name.kind)?,
            layout: element.layout,
        })
    }

    fn fixed_array(&mut self, node: &SyntaxNode, kind: ValueType) -> Result<Vec<Value>> {
        self.array(node, kind)?
            .into_iter()
            .map(|operand| match operand {
                Operand::Literal(value) => Ok(value),
                _ => Err(self
                    .source
                    .error(node, "expected an array of fixed parameters")),
            })
            .collect()
    }

    fn constant_float(&mut self, node: &SyntaxNode) -> Result<FiniteFloat> {
        match self.scalar(node, ValueType::Float)? {
            Operand::Literal(Value::Float(value)) => Ok(value),
            _ => Err(self.source.error(node, "expected a fixed float parameter")),
        }
    }

    fn float_predicate(
        &mut self,
        call: &SyntaxNode,
        name: FloatName,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        Ok(match name {
            FloatName::Relation {
                relation,
                linear,
                mode,
            } => {
                let condition = match mode {
                    RelationMode::Plain => RelationCondition::Plain,
                    RelationMode::Reified => RelationCondition::Reified {
                        result: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
                    },
                    RelationMode::Implied => RelationCondition::Implied {
                        guard: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
                    },
                };
                if linear {
                    let coefficients = self
                        .fixed_array(args[0], ValueType::Float)?
                        .iter()
                        .map(Value::float)
                        .collect::<Vec<_>>();
                    let terms = self.array(args[1], ValueType::Float)?;
                    if coefficients.len() != terms.len() {
                        return Err(self.source.error(
                            call,
                            "linear coefficient and variable arrays must have equal lengths",
                        ));
                    }
                    Predicate::FloatLinear {
                        relation,
                        coefficients,
                        terms,
                        bound: self.constant_float(args[2])?,
                        condition,
                    }
                } else {
                    Predicate::FloatRelation {
                        relation,
                        left: self.scalar(args[0], ValueType::Float)?,
                        right: self.scalar(args[1], ValueType::Float)?,
                        condition,
                        code: None,
                    }
                }
            }
            FloatName::CodedImplication => {
                let code = self.constant_integer(args[1])?;
                let relation = relations::coded_relation(code).ok_or_else(|| {
                    self.source
                        .error(args[1], "float relation code must be in 1..6")
                })?;
                Predicate::FloatRelation {
                    relation,
                    left: self.scalar(args[0], ValueType::Float)?,
                    right: self.scalar(args[2], ValueType::Float)?,
                    condition: RelationCondition::Implied {
                        guard: self.scalar(args[3], ValueType::Boolean)?,
                    },
                    code: Some(code),
                }
            }
            FloatName::Arithmetic(operation) => {
                let inputs = args[..operation.input_count()]
                    .iter()
                    .map(|arg| self.scalar(arg, ValueType::Float))
                    .collect::<Result<_>>()?;
                if operation == FloatOperation::FixedBasePower {
                    let base = self.constant_float(args[0])?;
                    if base.get() <= 0.0 {
                        return Err(self
                            .source
                            .error(args[0], "fixed-base power requires a positive base"));
                    }
                }
                Predicate::FloatArithmetic {
                    operation,
                    inputs,
                    result: self.scalar(args[operation.input_count()], ValueType::Float)?,
                }
            }
            FloatName::IntegerPower => {
                let exponent = self.constant_integer(args[1])?;
                floats::validate_integer_exponent(exponent)
                    .map_err(|message| self.source.error(args[1], message))?;
                Predicate::FloatIntegerPower {
                    base: self.scalar(args[0], ValueType::Float)?,
                    exponent,
                    result: self.scalar(args[2], ValueType::Float)?,
                }
            }
            FloatName::ArrayExtremum {
                extremum,
                result_first,
            } => {
                let (inputs, result) = if result_first {
                    (args[1], args[0])
                } else {
                    (args[0], args[1])
                };
                let elements = self.array(inputs, ValueType::Float)?;
                if elements.is_empty() {
                    return Err(self
                        .source
                        .error(inputs, "float array extremum needs a nonempty array"));
                }
                Predicate::FloatArrayExtremum {
                    extremum,
                    inputs: elements,
                    result: self.scalar(result, ValueType::Float)?,
                }
            }
            FloatName::Conversion(operation) => Predicate::FloatConversion {
                operation,
                input: self.scalar(args[0], operation.input_type())?,
                result: self.scalar(args[1], operation.output_type())?,
            },
            FloatName::Membership(membership) => {
                let element = self.scalar(args[0], ValueType::Float)?;
                let intervals = match membership {
                    FloatMembership::Interval => {
                        vec![(self.constant_float(args[1])?, self.constant_float(args[2])?)]
                    }
                    FloatMembership::Domain => self
                        .fixed_array(args[1], ValueType::Float)?
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|pair| (pair[0].float(), pair[1].float()))
                        .collect(),
                    FloatMembership::Set => {
                        let Operand::Literal(Value::FloatSet(set)) =
                            self.scalar(args[1], ValueType::FloatSet)?
                        else {
                            return Err(self.source.error(args[1], "expected a fixed float set"));
                        };
                        set.ranges().to_vec()
                    }
                };
                Predicate::FloatMembership {
                    membership,
                    element,
                    intervals,
                }
            }
        })
    }

    fn boolean_predicate(
        &mut self,
        call: &SyntaxNode,
        name: BooleanName,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        let condition = match name
            .mode_for_arity(args.len())
            .expect("admitted Boolean arity")
        {
            RelationMode::Plain => RelationCondition::Plain,
            RelationMode::Reified => RelationCondition::Reified {
                result: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
            RelationMode::Implied => RelationCondition::Implied {
                guard: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
        };
        Ok(match name.kind {
            BooleanKind::Relation(relation) => Predicate::BooleanRelation {
                relation,
                left: self.scalar(args[0], ValueType::Boolean)?,
                right: self.scalar(args[1], ValueType::Boolean)?,
                condition,
            },
            BooleanKind::Linear(relation) => {
                let coefficients = self
                    .array(args[0], ValueType::Integer)?
                    .into_iter()
                    .map(|operand| match operand {
                        Operand::Literal(Value::Integer(value)) => Ok(value),
                        _ => Err(self.source.error(
                            args[0],
                            "linear coefficients must be fixed integer parameters",
                        )),
                    })
                    .collect::<Result<Vec<_>>>()?;
                let terms = self.array(args[1], ValueType::Boolean)?;
                if coefficients.len() != terms.len() {
                    return Err(self.source.error(
                        call,
                        "linear coefficient and variable arrays must have equal lengths",
                    ));
                }
                Predicate::BooleanLinear {
                    relation,
                    coefficients,
                    terms,
                    bound: self.scalar(args[2], ValueType::Integer)?,
                    condition,
                }
            }
            BooleanKind::Scalar(operation) => Predicate::BooleanOperation {
                operation,
                inputs: args[..operation.input_count()]
                    .iter()
                    .map(|node| self.scalar(node, ValueType::Boolean))
                    .collect::<Result<_>>()?,
                condition,
            },
            BooleanKind::Reduction(operation) => Predicate::BooleanOperation {
                operation,
                inputs: self.array(args[0], ValueType::Boolean)?,
                condition,
            },
            BooleanKind::Clause => Predicate::BooleanClause {
                positive: self.array(args[0], ValueType::Boolean)?,
                negative: self.array(args[1], ValueType::Boolean)?,
                condition,
            },
            BooleanKind::ToInteger => Predicate::BooleanToInteger {
                boolean: self.scalar(args[0], ValueType::Boolean)?,
                integer: self.scalar(args[1], ValueType::Integer)?,
            },
        })
    }

    fn integer_relation(
        &mut self,
        call: &SyntaxNode,
        name: RelationName,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        let condition = match name.mode {
            RelationMode::Plain => RelationCondition::Plain,
            RelationMode::Reified => RelationCondition::Reified {
                result: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
            RelationMode::Implied => RelationCondition::Implied {
                guard: self.scalar(args[args.len() - 1], ValueType::Boolean)?,
            },
        };
        if name.linear {
            let coefficients = self
                .array(args[0], ValueType::Integer)?
                .into_iter()
                .map(|operand| match operand {
                    Operand::Literal(Value::Integer(value)) => Ok(value),
                    _ => Err(self.source.error(
                        args[0],
                        "linear coefficients must be fixed integer parameters",
                    )),
                })
                .collect::<Result<Vec<_>>>()?;
            let terms = self.array(args[1], ValueType::Integer)?;
            if coefficients.len() != terms.len() {
                return Err(self.source.error(
                    call,
                    "linear coefficient and variable arrays must have equal lengths",
                ));
            }
            let bound = self.constant_integer(args[2])?;
            if name.mode == RelationMode::Plain
                && matches!(
                    name.relation,
                    IntegerRelation::Equal | IntegerRelation::LessEqual
                )
            {
                Ok(Predicate::Linear {
                    coefficients,
                    terms,
                    bound,
                    equality: name.relation == IntegerRelation::Equal,
                })
            } else {
                Ok(Predicate::LinearRelation {
                    relation: name.relation,
                    coefficients,
                    terms,
                    bound,
                    condition,
                })
            }
        } else {
            let left = self.scalar(args[0], ValueType::Integer)?;
            let right = self.scalar(args[1], ValueType::Integer)?;
            if name.relation == IntegerRelation::LessEqual
                && let RelationCondition::Reified { result } = condition
            {
                Ok(Predicate::ReifiedLessEqual {
                    left,
                    right,
                    result,
                })
            } else {
                Ok(Predicate::IntegerRelation {
                    relation: name.relation,
                    left,
                    right,
                    condition,
                })
            }
        }
    }

    fn integer_arithmetic(
        &mut self,
        operation: ArithmeticOperation,
        args: &[&SyntaxNode],
    ) -> Result<Predicate> {
        let mut inputs = Vec::new();
        for (index, node) in args[..operation.input_count()].iter().enumerate() {
            let operand = if index == 1 && operation.has_fixed_exponent() {
                let exponent = self.constant_integer(node)?;
                operation
                    .validate_exponent(exponent)
                    .map_err(|message| self.source.error(node, message))?;
                Operand::Literal(Value::Integer(exponent))
            } else {
                self.scalar(node, ValueType::Integer)?
            };
            inputs.push(operand);
        }
        Ok(Predicate::IntegerArithmetic {
            operation,
            inputs,
            result: self.scalar(args[operation.input_count()], ValueType::Integer)?,
        })
    }

    fn validate_declared_call(
        &mut self,
        call: &SyntaxNode,
        name: &str,
        args: &[&SyntaxNode],
    ) -> Result<()> {
        let Some(signature) = self.predicates.get(name).cloned() else {
            return Ok(());
        };
        if args.len() != signature.len() {
            return Err(self.source.error(
                call,
                "call argument count does not match its predicate declaration",
            ));
        }
        for (argument, formal) in args.iter().zip(signature) {
            let operands = if formal.is_array {
                self.array(argument, formal.kind)?
            } else {
                vec![self.scalar(argument, formal.kind)?]
            };
            if formal.length.is_some_and(|length| length != operands.len()) {
                return Err(self.source.error(
                    argument,
                    "call array length does not match its predicate declaration",
                ));
            }
            for operand in operands {
                match operand {
                    Operand::Literal(value) if formal.domain.contains(&value) => {}
                    Operand::Variable(slot)
                        if formal.is_var && self.slot_fits_domain(slot, &formal.domain) => {}
                    _ => return Err(self.source.error(
                        call,
                        "call argument does not satisfy its declared fixed/variable role or domain",
                    )),
                }
            }
        }
        Ok(())
    }

    fn slot_fits_domain(&self, slot: usize, formal: &Domain) -> bool {
        if matches!(
            formal,
            Domain::Float | Domain::Boolean | Domain::Integer | Domain::IntegerSet { upper: None }
        ) {
            return true;
        }
        let domains = self
            .model
            .variables
            .iter()
            .filter(|variable| variable.slot == slot)
            .flat_map(|variable| &variable.domains);
        match formal {
            Domain::FloatInterval { lower, upper } => {
                let mut actual_lower = -f64::MAX;
                let mut actual_upper = f64::MAX;
                for domain in domains {
                    let restriction = match domain {
                        Domain::FloatInterval { lower, upper } => Some((lower.get(), upper.get())),
                        Domain::Fixed {
                            value: Value::Float(value),
                        } => Some((value.get(), value.get())),
                        _ => None,
                    };
                    if let Some((lower, upper)) = restriction {
                        actual_lower = actual_lower.max(lower);
                        actual_upper = actual_upper.min(upper);
                    }
                }
                actual_lower > actual_upper
                    || (lower.get() <= actual_lower && actual_upper <= upper.get())
            }
            Domain::Interval { .. } | Domain::Enumeration { .. } => {
                let mut actual = IntegerSet::interval(i64::MIN, i64::MAX);
                for domain in domains {
                    let restriction = match domain {
                        Domain::Interval { lower, upper } => IntegerSet::interval(*lower, *upper),
                        Domain::Enumeration { values } => IntegerSet::from_values(values.clone()),
                        Domain::Fixed {
                            value: Value::Integer(value),
                        } => IntegerSet::from_values(vec![*value]),
                        _ => continue,
                    };
                    actual = actual.intersection(&restriction);
                }
                let allowed = match formal {
                    Domain::Interval { lower, upper } => IntegerSet::interval(*lower, *upper),
                    Domain::Enumeration { values } => IntegerSet::from_values(values.clone()),
                    _ => unreachable!(),
                };
                actual.is_subset(&allowed)
            }
            Domain::IntegerSet {
                upper: Some(allowed),
            } => {
                let mut actual: Option<IntegerSet> = None;
                for domain in domains {
                    let restriction = match domain {
                        Domain::IntegerSet { upper: Some(upper) } => upper,
                        Domain::Fixed {
                            value: Value::IntegerSet(value),
                        } => value,
                        _ => continue,
                    };
                    actual = Some(actual.map_or_else(
                        || restriction.clone(),
                        |current| current.intersection(restriction),
                    ));
                }
                actual.is_some_and(|actual| actual.is_subset(allowed))
            }
            _ => false,
        }
    }

    fn strict_disjunctive(&mut self, call: &SyntaxNode, args: &[&SyntaxNode]) -> Result<Predicate> {
        if !self.predicates.contains_key("fzn_disjunctive_strict") {
            return Err(self.source.error(
                call,
                "fzn_disjunctive_strict requires its bodyless declaration",
            ));
        }
        let starts = self.array(args[0], ValueType::Integer)?;
        let durations = self.array(args[1], ValueType::Integer)?;
        if starts.len() != durations.len() {
            return Err(self
                .source
                .error(call, "strict disjunctive arrays must have equal lengths"));
        }
        if !durations.iter().all(
            |operand| matches!(operand, Operand::Literal(Value::Integer(value)) if *value >= 0),
        ) {
            return Err(self.source.error(
                args[1],
                "strict disjunctive durations must be fixed nonnegative integers",
            ));
        }
        Ok(Predicate::DisjunctiveStrict { starts, durations })
    }

    fn solve(&mut self, node: &SyntaxNode) -> Result<()> {
        let annotations: Vec<_> = node
            .child_nodes()
            .filter(|child| child.kind() == NodeKind::Annotation)
            .collect();
        self.metadata(&annotations, AnnotationContext::Solve)?;
        if node.kind() != NodeKind::Solve {
            let objective = node
                .child_nodes()
                .find(|child| child.kind() != NodeKind::Annotation)
                .ok_or_else(|| {
                    self.source
                        .error(node, "expected a scalar integer objective")
                })?;
            let objective = self
                .scalar(objective, ValueType::Integer)
                .or_else(|_| self.scalar(objective, ValueType::Float))?;
            let direction = if node.kind() == NodeKind::SolveMinimize {
                ObjectiveDirection::Minimize
            } else {
                ObjectiveDirection::Maximize
            };
            self.model.objective = Some((direction, objective));
        }
        Ok(())
    }
}

fn unannotated<'a>(node: &'a SyntaxNode, annotations: &mut Vec<&'a SyntaxNode>) -> &'a SyntaxNode {
    if node.kind() != NodeKind::AnnotatedExpression {
        return node;
    }
    let mut children = node.child_nodes();
    let subject = children
        .next()
        .expect("the parser gives annotated expressions a subject");
    let subject = unannotated(subject, annotations);
    annotations.extend(children);
    subject
}
