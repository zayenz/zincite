use super::*;
use zincite_syntax::{FileMode, NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

type Result<T> = std::result::Result<T, Diagnostic>;

/// Parse and validate the admitted FlatZinc dialect once for repeated checks.
pub fn prepare(name: &str, source: &str) -> std::result::Result<PreparedModel, Vec<Diagnostic>> {
    let parsed = zincite_syntax::parse(source);
    let source = Source {
        name,
        parsed: &parsed,
    };
    source.validate_syntax()?;
    Lowerer::new(&source)
        .and_then(Lowerer::lower)
        .map_err(|error| vec![error])
}

/// Accept only Boolean/signed-decimal-integer literals and one-dimensional arrays.
pub fn parse_assignment(
    name: &str,
    text: &str,
) -> std::result::Result<Assignment, Vec<Diagnostic>> {
    let parsed = zincite_syntax::parse_with_mode(text, FileMode::Data);
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
        self.integer(node).map(Value::Integer)
    }

    fn integer(&self, node: &SyntaxNode) -> Result<i64> {
        let (negative, atom) = if node.kind() == NodeKind::UnaryExpression {
            let negative = match self.tokens(node).next() {
                Some((TokenKind::Minus, _)) => true,
                Some((TokenKind::Plus, _)) => false,
                _ => return Err(self.error(node, "expected a signed decimal integer literal")),
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
            return Err(self.error(node, "expected a Boolean or signed decimal integer literal; computed expressions are unsupported"));
        }
        let Some((TokenKind::IntegerLiteral, text)) = self.tokens(atom).next() else {
            return Err(self.error(node, "expected a Boolean or signed decimal integer literal"));
        };
        if !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(self.error(node, "only decimal integer spelling is supported"));
        }
        let magnitude: i128 = text
            .parse()
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
        // JSON and the admitted metadata strings share these escape sequences.
        serde_json::from_str(text)
            .map_err(|_| self.error(node, "unsupported string escape in annotation"))
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
struct TypeSpec {
    kind: ValueType,
    domain: Domain,
    is_var: bool,
    length: Option<usize>,
}

impl Source<'_> {
    fn type_spec(&self, node: &SyntaxNode) -> Result<TypeSpec> {
        if node.kind() == NodeKind::ArrayType {
            let children: Vec<_> = node.child_nodes().collect();
            if children.len() != 2 || children[0].kind() != NodeKind::DomainType {
                return Err(self.error(
                    node,
                    "only one-dimensional arrays indexed by literal 1..n are supported",
                ));
            }
            let index = children[0]
                .child_nodes()
                .next()
                .ok_or_else(|| self.error(children[0], "expected array index interval"))?;
            let (lower, upper) = self.interval(index)?;
            if lower != 1 || upper < 0 {
                return Err(self.error(index, "array indexes must be 1..n, with n >= 0"));
            }
            let mut element = self.type_spec(children[1])?;
            if element.length.is_some() {
                return Err(self.error(node, "nested arrays are unsupported"));
            }
            element.length = Some(usize::try_from(upper).map_err(|_| {
                self.error(index, "array length exceeds this target's addressable size")
            })?);
            return Ok(element);
        }
        let tokens: Vec<_> = self.tokens(node).map(|(kind, _)| kind).collect();
        if tokens.contains(&TokenKind::Opt) {
            return Err(self.error(node, "optional values are unsupported"));
        }
        let is_var = tokens.contains(&TokenKind::Var);
        let (kind, domain) = match node.kind() {
            NodeKind::ScalarType if tokens.contains(&TokenKind::Int) => {
                (ValueType::Integer, Domain::Integer)
            }
            NodeKind::ScalarType if tokens.contains(&TokenKind::Bool) => {
                (ValueType::Boolean, Domain::Boolean)
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
                return Err(
                    self.error(node, "unsupported type; expected Boolean or integer values")
                );
            }
        };
        Ok(TypeSpec {
            kind,
            domain,
            is_var,
            length: None,
        })
    }
}

impl Domain {
    fn singleton(&self) -> Option<Value> {
        match self {
            Self::Interval { lower, upper } if lower == upper => Some(Value::Integer(*lower)),
            Self::Enumeration { values } if values.len() == 1 => Some(Value::Integer(values[0])),
            Self::Fixed { value } => Some(*value),
            _ => None,
        }
    }
}

struct Declaration<'a> {
    node: &'a SyntaxNode,
    name: String,
    ty: TypeSpec,
    initializer: Option<&'a SyntaxNode>,
    metadata: Metadata,
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

impl Source<'_> {
    fn metadata(
        &self,
        annotations: &[&SyntaxNode],
        context: AnnotationContext,
    ) -> Result<Metadata> {
        let mut metadata = Metadata::default();
        let mut seen = HashSet::new();
        for annotation in annotations {
            let expression = annotation
                .child_nodes()
                .next()
                .ok_or_else(|| self.error(annotation, "malformed annotation"))?;
            let name = self.name(expression)?;
            let arguments: Vec<_> = expression.child_nodes().collect();
            let is_call = expression.kind() == NodeKind::CallExpression;
            if !seen.insert(name.clone()) && name != "mzn_path" {
                return Err(self.error(annotation, format!("duplicate annotation '{name}'")));
            }
            match (name.as_str(), context) {
                ("checker_value", AnnotationContext::Value { .. })
                | ("checker_constraint", AnnotationContext::Constraint)
                    if is_call && arguments.len() == 2 =>
                {
                    metadata.application = Some(Application {
                        id: self.string(arguments[0])?,
                        entities: self.integer_array(arguments[1])?,
                    });
                }
                ("mzn_path", AnnotationContext::Value { .. } | AnnotationContext::Constraint)
                    if is_call && arguments.len() == 1 =>
                {
                    metadata.paths.push(self.string(arguments[0])?)
                }
                ("mzn_constraint_name", AnnotationContext::Constraint)
                    if is_call && arguments.len() == 1 =>
                {
                    metadata.label = Some(self.string(arguments[0])?)
                }
                (
                    "is_defined_var",
                    AnnotationContext::Value {
                        is_array: false,
                        is_var: true,
                        ..
                    },
                ) if !is_call => metadata.is_defined = true,
                ("defines_var", AnnotationContext::Constraint)
                    if is_call
                        && arguments.len() == 1
                        && arguments[0].kind() == NodeKind::Expression =>
                {
                    metadata.defines = Some(self.name(arguments[0])?)
                }
                (
                    "output_var",
                    AnnotationContext::Value {
                        is_array: false,
                        is_var: true,
                        ..
                    },
                ) if !is_call => metadata.output = true,
                (
                    "output_array",
                    AnnotationContext::Value {
                        is_array: true,
                        is_var: true,
                        length,
                    },
                ) if is_call && arguments.len() == 1 => {
                    let indexes: Vec<_> = arguments[0].child_nodes().collect();
                    if arguments[0].kind() != NodeKind::ArrayLiteral || indexes.len() != 1 {
                        return Err(
                            self.error(annotation, "output_array needs one literal index interval")
                        );
                    }
                    let (lower, upper) = self.interval(indexes[0])?;
                    if lower != 1 || usize::try_from(upper).ok() != length {
                        return Err(self.error(
                            annotation,
                            "output_array index range must match the declaration",
                        ));
                    }
                    metadata.output = true;
                }
                ("int_search" | "bool_search" | "seq_search", AnnotationContext::Solve) => {
                    self.search_annotation(expression)?
                }
                _ => {
                    return Err(self.error(
                        annotation,
                        format!("unsupported or malformed annotation '{name}' in this context"),
                    ));
                }
            }
        }
        Ok(metadata)
    }

    fn search_annotation(&self, expression: &SyntaxNode) -> Result<()> {
        let name = self.name(expression)?;
        let arguments: Vec<_> = expression.child_nodes().collect();
        if expression.kind() != NodeKind::CallExpression {
            return Err(self.error(expression, "malformed search annotation"));
        }
        match name.as_str() {
            "int_search" | "bool_search" if arguments.len() == 4 => {
                if !matches!(
                    arguments[0].kind(),
                    NodeKind::ArrayLiteral | NodeKind::Expression
                ) {
                    return Err(self.error(
                        arguments[0],
                        "search variables must be a literal array or array reference",
                    ));
                }
                for argument in &arguments[1..] {
                    if argument.kind() != NodeKind::Expression {
                        return Err(self.error(argument, "expected a search strategy name"));
                    }
                    self.name(argument)?;
                }
                Ok(())
            }
            "seq_search"
                if arguments.len() == 1 && arguments[0].kind() == NodeKind::ArrayLiteral =>
            {
                for search in arguments[0].child_nodes() {
                    self.search_annotation(search)?;
                }
                Ok(())
            }
            _ => Err(self.error(expression, "unsupported or malformed search annotation")),
        }
    }
}

struct Lowerer<'a> {
    source: &'a Source<'a>,
    declarations: Vec<Declaration<'a>>,
    names: HashMap<String, usize>,
    resolved: Vec<Option<Binding>>,
    resolving: HashSet<usize>,
    model: PreparedModel,
}

impl<'a> Lowerer<'a> {
    fn new(source: &'a Source<'a>) -> Result<Self> {
        let mut declarations = Vec::new();
        let mut names = HashMap::new();
        for node in source
            .parsed
            .tree()
            .child_nodes()
            .filter(|node| node.kind() == NodeKind::Declaration)
        {
            let children: Vec<_> = node.child_nodes().collect();
            let ty = source.type_spec(children[0])?;
            let name = source.name(node)?;
            if names.insert(name.clone(), declarations.len()).is_some() {
                return Err(source.error(node, format!("duplicate declaration '{name}'")));
            }
            let annotations: Vec<_> = children
                .iter()
                .copied()
                .filter(|node| node.kind() == NodeKind::Annotation)
                .collect();
            let metadata = source.metadata(
                &annotations,
                AnnotationContext::Value {
                    is_array: ty.length.is_some(),
                    is_var: ty.is_var,
                    length: ty.length,
                },
            )?;
            let initializer = children
                .iter()
                .copied()
                .skip(1)
                .find(|node| node.kind() != NodeKind::Annotation);
            declarations.push(Declaration {
                node,
                name,
                ty,
                initializer,
                metadata,
            });
        }
        let resolved = vec![None; declarations.len()];
        Ok(Self {
            source,
            declarations,
            names,
            resolved,
            resolving: HashSet::new(),
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
                NodeKind::Declaration => {}
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
        self.prepare_definitions()?;
        self.model.variables.sort_by_key(|variable| {
            (
                variable.reference.location.start,
                variable.reference.position,
            )
        });
        self.link_aliases();
        Ok(self.model)
    }

    fn resolve(&mut self, index: usize) -> Result<Binding> {
        if let Some(binding) = &self.resolved[index] {
            return Ok(binding.clone());
        }
        if !self.resolving.insert(index) {
            return Err(self
                .source
                .error(self.declarations[index].node, "cyclic declaration alias"));
        }
        let ty = self.declarations[index].ty.clone();
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
        self.resolving.remove(&index);
        Ok(binding)
    }

    fn initial_values(&mut self, index: usize, ty: &TypeSpec) -> Result<Vec<Operand>> {
        let declaration = &self.declarations[index];
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
            if !ty.domain.contains(*value) {
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
            let slot = match *operand {
                Operand::Literal(value) => {
                    domains.push(Domain::Fixed { value });
                    let slot = self.model.slots.len();
                    self.model.slots.push(Slot {
                        kind: ty.kind,
                        default: Some(value),
                        reference: reference.clone(),
                    });
                    *operand = Operand::Variable(slot);
                    slot
                }
                Operand::Variable(slot) => {
                    if self.model.slots[slot].default.is_none() {
                        self.model.slots[slot].default = ty.domain.singleton();
                    }
                    slot
                }
            };
            self.model.variables.push(Variable {
                reference,
                slot,
                domains,
            });
        }
    }

    fn link_aliases(&mut self) {
        let mut aliases = vec![Vec::new(); self.model.slots.len()];
        for variable in &self.model.variables {
            aliases[variable.slot].push(VariableAlias {
                name: variable.reference.name.clone(),
                position: variable.reference.position,
            });
        }
        let link = |reference: &mut VariableRef, slot: usize| {
            reference.aliases = aliases[slot]
                .iter()
                .filter(|alias| {
                    alias.name != reference.name || alias.position != reference.position
                })
                .cloned()
                .collect();
        };
        for variable in &mut self.model.variables {
            link(&mut variable.reference, variable.slot);
        }
        for (slot, value) in self.model.slots.iter_mut().enumerate() {
            link(&mut value.reference, slot);
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
        self.resolve(index)
    }

    fn scalar(&mut self, node: &SyntaxNode, kind: ValueType) -> Result<Operand> {
        let operand = if node.kind() == NodeKind::Expression
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
            binding.elements[0]
        } else {
            Operand::Literal(self.source.literal(node)?)
        };
        let actual = match operand {
            Operand::Literal(value) => value.value_type(),
            Operand::Variable(slot) => self.model.slots[slot].kind,
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
        if args.len() != 3 {
            return Err(self
                .source
                .error(call, format!("'{name}' expects three arguments")));
        }
        let predicate = match name.as_str() {
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
        };
        let metadata = self
            .source
            .metadata(&annotations, AnnotationContext::Constraint)?;
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

    fn solve(&mut self, node: &SyntaxNode) -> Result<()> {
        let annotations: Vec<_> = node
            .child_nodes()
            .filter(|child| child.kind() == NodeKind::Annotation)
            .collect();
        self.source
            .metadata(&annotations, AnnotationContext::Solve)?;
        for annotation in annotations {
            let search = annotation
                .child_nodes()
                .next()
                .expect("validated solve annotations have a search expression");
            self.validate_search_inputs(search)?;
        }
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

    fn validate_search_inputs(&mut self, node: &SyntaxNode) -> Result<()> {
        let name = self.source.name(node)?;
        let arguments: Vec<_> = node.child_nodes().collect();
        match name.as_str() {
            "seq_search" => {
                for search in arguments[0].child_nodes() {
                    self.validate_search_inputs(search)?;
                }
            }
            "int_search" => {
                self.array(arguments[0], ValueType::Integer)?;
            }
            "bool_search" => {
                self.array(arguments[0], ValueType::Boolean)?;
            }
            _ => unreachable!("solve annotations were validated before resolving search inputs"),
        }
        Ok(())
    }

    fn prepare_definitions(&mut self) -> Result<()> {
        let mut defined = HashSet::new();
        self.model.dependents = vec![Vec::new(); self.model.slots.len()];
        for (constraint_index, constraint) in self.model.constraints.iter().enumerate() {
            let Some(name) = &constraint.reference.metadata.defines else {
                continue;
            };
            let fail = |message: &str| Diagnostic {
                location: Some(constraint.reference.location.clone()),
                message: message.into(),
            };
            let binding = self
                .model
                .bindings
                .get(name)
                .ok_or_else(|| fail("defines_var names an unknown variable"))?;
            let [Operand::Variable(slot)] = binding.elements.as_slice() else {
                return Err(fail("defines_var needs a scalar variable"));
            };
            if binding.is_array || !binding.assignable {
                return Err(fail("defines_var needs a scalar variable"));
            }
            let declaration = &self.declarations[self.names[name]];
            if !declaration.metadata.is_defined {
                return Err(fail("defines_var target must have is_defined_var"));
            }
            if !defined.insert(*slot) {
                return Err(fail("a variable has multiple defines_var annotations"));
            }
            let direction = definition_direction(constraint, *slot)?;
            // A definition supplies its output even when an alias has a singleton domain.
            self.model.slots[*slot].default = None;
            if let Some(direction) = direction {
                let inputs: &[Operand] = match &constraint.predicate {
                    Predicate::Linear { terms, .. } => terms,
                    Predicate::ReifiedLessEqual { left, right, .. } => &[*left, *right],
                };
                let definition_index = self.model.definitions.len();
                let mut inputs_seen = HashSet::new();
                for operand in inputs {
                    if let Operand::Variable(input) = operand
                        && input != slot
                        && inputs_seen.insert(*input)
                    {
                        self.model.dependents[*input].push(definition_index);
                    }
                }
                self.model.definitions.push(Definition {
                    constraint: constraint_index,
                    output: *slot,
                    direction,
                });
            }
        }
        for declaration in &self.declarations {
            if declaration.metadata.is_defined {
                let binding = &self.model.bindings[&declaration.name];
                if !matches!(binding.elements.as_slice(), [Operand::Variable(slot)] if defined.contains(slot))
                {
                    return Err(self.source.error(
                        declaration.node,
                        "is_defined_var has no matching defines_var relation",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn definition_direction(
    constraint: &Constraint,
    output: usize,
) -> Result<Option<DefinitionDirection>> {
    let absent = || Diagnostic {
        location: Some(constraint.reference.location.clone()),
        message: "defines_var target does not occur in its relation".into(),
    };
    match &constraint.predicate {
        Predicate::Linear {
            coefficients,
            terms,
            equality,
            ..
        } => {
            let occurrences: Vec<_> = terms
                .iter()
                .enumerate()
                .filter_map(|(index, operand)| {
                    matches!(operand, Operand::Variable(slot) if *slot == output).then_some(index)
                })
                .collect();
            let &term = occurrences.first().ok_or_else(absent)?;
            Ok(
                (*equality && occurrences.len() == 1 && coefficients[term] != 0)
                    .then_some(DefinitionDirection::Linear { term }),
            )
        }
        Predicate::ReifiedLessEqual {
            left,
            right,
            result,
        } => {
            if ![left, right, result]
                .iter()
                .any(|operand| matches!(operand, Operand::Variable(slot) if *slot == output))
            {
                return Err(absent());
            }
            Ok(matches!(result, Operand::Variable(slot) if *slot == output)
                .then_some(DefinitionDirection::ReifiedLessEqual))
        }
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
