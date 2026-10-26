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
        self.integer(node).map(Value::Integer)
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
        let expected = if let Some(operation) = ArithmeticOperation::from_name(name) {
            let mut arguments = vec![scalar(ValueType::Integer, true); operation.input_count() + 1];
            if operation.has_fixed_exponent() {
                arguments[1].is_var = false;
            }
            arguments
        } else {
            match name {
                "int_lin_eq" | "int_lin_le" => vec![
                    array(ValueType::Integer, false),
                    array(ValueType::Integer, true),
                    scalar(ValueType::Integer, false),
                ],
                "int_le_reif" => vec![
                    scalar(ValueType::Integer, true),
                    scalar(ValueType::Integer, true),
                    scalar(ValueType::Boolean, true),
                ],
                "set_in" => vec![
                    scalar(ValueType::Integer, true),
                    scalar(ValueType::IntegerSet, true),
                ],
                "set_eq" | "set_subset" => vec![scalar(ValueType::IntegerSet, true); 2],
                "set_card" => vec![
                    scalar(ValueType::IntegerSet, true),
                    scalar(ValueType::Integer, true),
                ],
                "set_union" | "set_intersect" => vec![scalar(ValueType::IntegerSet, true); 3],
                "fzn_disjunctive_strict" => vec![array(ValueType::Integer, true); 2],
                _ => {
                    return Err(
                        self.error(node, format!("unsupported predicate declaration '{name}'"))
                    );
                }
            }
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
            let allow_fixed_set = name == "set_in" && index == 1;
            signature.push(self.formal_parameter(parameter, expected, allow_fixed_set)?);
        }
        Ok(signature)
    }

    fn formal_parameter(
        &self,
        parameter: &SyntaxNode,
        expected: ArgumentType,
        allow_fixed_set: bool,
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
        let valid_role = ty.is_var == expected.is_var || (allow_fixed_set && !ty.is_var);
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
struct ArgumentType {
    kind: ValueType,
    is_var: bool,
    is_array: bool,
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
            NodeKind::ScalarType => matches!(base_tokens, [TokenKind::Int] | [TokenKind::Bool]),
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
            NodeKind::ScalarType if tokens.contains(&TokenKind::Bool) => {
                (ValueType::Boolean, Domain::Boolean)
            }
            NodeKind::SetType => {
                let element = node
                    .child_nodes()
                    .next()
                    .ok_or_else(|| self.error(node, "expected an integer set type"))?;
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
                    Domain::Boolean | Domain::Integer | Domain::IntegerSet { upper: None }
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
                    .map(|node| self.source.literal(node).map(Operand::Literal))
                    .collect::<Result<Vec<_>>>()?
            } else {
                vec![Operand::Literal(self.source.literal(initializer)?)]
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
            Operand::Literal(self.source.literal(node)?)
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
        let arity = match name.as_str() {
            "set_in" | "set_eq" | "set_subset" | "set_card" | "fzn_disjunctive_strict" => 2,
            "int_lin_eq" | "int_lin_le" | "int_le_reif" | "set_union" | "set_intersect" => 3,
            _ => ArithmeticOperation::from_name(&name)
                .map(|operation| operation.input_count() + 1)
                .ok_or_else(|| {
                    self.source
                        .error(call, format!("unsupported predicate '{name}'"))
                })?,
        };
        if args.len() != arity {
            return Err(self
                .source
                .error(call, format!("'{name}' expects {arity} arguments")));
        }
        let predicate = if let Some(operation) = ArithmeticOperation::from_name(&name) {
            self.integer_arithmetic(operation, &args)?
        } else {
            match name.as_str() {
                "set_in" => Predicate::SetIn {
                    element: self.scalar(args[0], ValueType::Integer)?,
                    set: self.scalar(args[1], ValueType::IntegerSet)?,
                },
                "set_eq" => Predicate::SetEqual {
                    left: self.scalar(args[0], ValueType::IntegerSet)?,
                    right: self.scalar(args[1], ValueType::IntegerSet)?,
                },
                "set_subset" => Predicate::SetSubset {
                    left: self.scalar(args[0], ValueType::IntegerSet)?,
                    right: self.scalar(args[1], ValueType::IntegerSet)?,
                },
                "set_card" => Predicate::SetCardinality {
                    set: self.scalar(args[0], ValueType::IntegerSet)?,
                    cardinality: self.scalar(args[1], ValueType::Integer)?,
                },
                "set_union" => Predicate::SetUnion {
                    left: self.scalar(args[0], ValueType::IntegerSet)?,
                    right: self.scalar(args[1], ValueType::IntegerSet)?,
                    result: self.scalar(args[2], ValueType::IntegerSet)?,
                },
                "set_intersect" => Predicate::SetIntersection {
                    left: self.scalar(args[0], ValueType::IntegerSet)?,
                    right: self.scalar(args[1], ValueType::IntegerSet)?,
                    result: self.scalar(args[2], ValueType::IntegerSet)?,
                },
                "fzn_disjunctive_strict" => self.strict_disjunctive(call, &args)?,
                "int_lin_eq" | "int_lin_le" => {
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
                    Predicate::Linear {
                        coefficients,
                        terms,
                        bound,
                        equality: name == "int_lin_eq",
                    }
                }
                "int_le_reif" => Predicate::ReifiedLessEqual {
                    left: self.scalar(args[0], ValueType::Integer)?,
                    right: self.scalar(args[1], ValueType::Integer)?,
                    result: self.scalar(args[2], ValueType::Boolean)?,
                },
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
            Domain::Boolean | Domain::Integer | Domain::IntegerSet { upper: None }
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
            let objective = self.scalar(objective, ValueType::Integer)?;
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
