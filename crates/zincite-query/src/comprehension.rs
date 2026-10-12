//! MiniZinc CST lowering and native projection; Zirql evaluates every value.
use crate::{Input, QueryError, QueryResult, SemanticStream, pipeline, structured};
use serde_json::{Value, json};
use std::ops::Range;
use zincite_syntax::{NodeKind as N, ParsedFile, SyntaxElement, SyntaxNode, TokenKind as T};
use zirql::{
    Budget, ConstructedValue as V, Located, ValueBinaryOperator as B, ValueExpression as E,
    ValueExpressionKind as K, ValueGenerator, ValueUnaryOperator as U,
};

fn charge(budget: &mut Budget, units: usize, range: &Range<usize>) -> Result<(), QueryError> {
    budget
        .charge(units, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)
}
fn slots<T>(budget: &mut Budget, len: usize, range: &Range<usize>) -> Result<Vec<T>, QueryError> {
    if len
        .checked_mul(std::mem::size_of::<T>())
        .is_none_or(|n| n > isize::MAX as usize)
    {
        return Err(QueryError::query(range.clone(), "allocation size overflow"));
    }
    charge(budget, len, range)?;
    budget
        .charge_results(len, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(Vec::with_capacity(len))
}

/// Parse one complete expression using the host CST's assignment entry point.
pub(crate) fn lower(
    source: &str,
    range: Range<usize>,
    budget: &mut Budget,
    nesting: usize,
) -> Result<E, QueryError> {
    charge(
        budget,
        source
            .len()
            .checked_add(10)
            .ok_or_else(|| QueryError::query(range.clone(), "allocation size overflow"))?,
        &range,
    )?;
    let parsed = zincite_syntax::parse_with_mode(
        format!("value = {source};"),
        zincite_syntax::FileMode::Data,
    );
    if let Some(error) = parsed.diagnostics().first() {
        return Err(QueryError::query(
            map_range(error.range.clone(), range.start, 8),
            &error.message,
        ));
    }
    let rhs = parsed
        .tree()
        .child_nodes()
        .next()
        .and_then(crate::assignment_rhs)
        .ok_or_else(|| QueryError::query(range.clone(), "expected one value expression"))?;
    Lower {
        parsed: &parsed,
        offset: range.start,
        prefix: 8,
        budget,
        nesting,
        literal_only: false,
    }
    .node(rhs, 0)
}
fn map_range(range: Range<usize>, offset: usize, prefix: usize) -> Range<usize> {
    offset + range.start.saturating_sub(prefix)..offset + range.end.saturating_sub(prefix)
}
struct Lower<'a, 'b> {
    parsed: &'a ParsedFile,
    offset: usize,
    prefix: usize,
    budget: &'b mut Budget,
    nesting: usize,
    literal_only: bool,
}
impl Lower<'_, '_> {
    fn range(&self, node: &SyntaxNode) -> Range<usize> {
        map_range(node.range(), self.offset, self.prefix)
    }
    fn tokens<'a>(&'a self, node: &'a SyntaxNode) -> impl Iterator<Item = usize> + 'a {
        node.children().iter().filter_map(|c| match c {
            SyntaxElement::Token(i)
                if !matches!(
                    self.parsed.tokens()[*i].kind,
                    T::Whitespace | T::LineComment | T::BlockComment
                ) =>
            {
                Some(*i)
            }
            _ => None,
        })
    }
    fn text(&self, i: usize) -> &str {
        &self.parsed.source()[self.parsed.tokens()[i].range.clone()]
    }
    fn name(&mut self, i: usize) -> Result<Located<String>, QueryError> {
        let range = map_range(
            self.parsed.tokens()[i].range.clone(),
            self.offset,
            self.prefix,
        );
        charge(self.budget, self.text(i).len(), &range)?;
        Ok(Located::new(
            crate::identifier_identity(self.text(i)).to_owned(),
            range,
        ))
    }
    fn children(&mut self, node: &SyntaxNode, depth: usize) -> Result<Vec<E>, QueryError> {
        let mut values = slots(self.budget, node.child_nodes().count(), &self.range(node))?;
        for child in node.child_nodes() {
            values.push(self.node(child, depth + 1)?);
        }
        Ok(values)
    }
    fn node(&mut self, node: &SyntaxNode, depth: usize) -> Result<E, QueryError> {
        let range = self.range(node);
        if depth >= self.nesting.min(64) {
            return Err(QueryError::query(range, "value nesting limit exceeded"));
        }
        charge(self.budget, 1 + node.children().len(), &range)?;
        let child = || {
            node.child_nodes()
                .next()
                .expect("valid CST expression child")
        };
        let unsupported = || {
            QueryError::query(
                range.clone(),
                "unsupported expression in positional reports",
            )
        };
        if self.literal_only
            && !matches!(
                node.kind(),
                N::Expression
                    | N::UnaryExpression
                    | N::ParenthesizedExpression
                    | N::ArrayLiteral
                    | N::TupleLiteral
                    | N::RecordLiteral
                    | N::SetLiteral
            )
        {
            return Err(unsupported());
        }
        let kind = match node.kind() {
            N::ParenthesizedExpression => return self.node(child(), depth + 1),
            N::Expression => {
                let i = self.tokens(node).next().ok_or_else(unsupported)?;
                match self.parsed.tokens()[i].kind {
                    T::True => K::Literal(V::Boolean(true)),
                    T::False => K::Literal(V::Boolean(false)),
                    T::IntegerLiteral | T::FloatLiteral => {
                        charge(self.budget, self.text(i).len(), &range)?;
                        K::Literal(number(self.text(i), &range)?)
                    }
                    T::StringLiteral => {
                        charge(self.budget, self.text(i).len(), &range)?;
                        K::Literal(V::String(
                            structured::decode_string(self.text(i))
                                .map_err(|m| QueryError::query(range.clone(), m))?,
                        ))
                    }
                    T::Identifier | T::QuotedIdentifier if !self.literal_only => {
                        K::Name(self.name(i)?.value)
                    }
                    _ => return Err(unsupported()),
                }
            }
            N::UnaryExpression => {
                let i = self.tokens(node).next().unwrap();
                let operator = match self.parsed.tokens()[i].kind {
                    T::Plus => U::Positive,
                    T::Minus => U::Negative,
                    T::Not if !self.literal_only => U::Not,
                    _ => return Err(unsupported()),
                };
                // A written negative minimum cannot first become a positive i64.
                if matches!(operator, U::Positive | U::Negative) && child().kind() == N::Expression
                {
                    let atom = self.tokens(child()).next().unwrap();
                    if matches!(
                        self.parsed.tokens()[atom].kind,
                        T::IntegerLiteral | T::FloatLiteral
                    ) {
                        charge(self.budget, self.text(atom).len() + 1, &range)?;
                        let spelling = format!("{}{}", self.text(i), self.text(atom));
                        return Ok(Located::new(K::Literal(number(&spelling, &range)?), range));
                    }
                }
                K::Unary {
                    operator,
                    operand: Box::new(self.node(child(), depth + 1)?),
                }
            }
            N::BinaryExpression | N::RangeExpression => {
                if node.child_nodes().count() != 2 {
                    return Err(unsupported());
                }
                let i = self.tokens(node).next().unwrap();
                let mut children = node.child_nodes();
                let left = Box::new(self.node(children.next().unwrap(), depth + 1)?);
                let right = Box::new(self.node(children.next().unwrap(), depth + 1)?);
                if self.parsed.tokens()[i].kind == T::RangeInclusive {
                    K::Range {
                        lower: left,
                        upper: right,
                    }
                } else {
                    let operator = match self.parsed.tokens()[i].kind {
                        T::Plus => B::Add,
                        T::Minus => B::Subtract,
                        T::Star => B::Multiply,
                        T::Slash => B::Divide,
                        T::Div => B::IntegerDivide,
                        T::Mod => B::Modulo,
                        T::Less => B::Less,
                        T::LessEqual => B::LessEqual,
                        T::Greater => B::Greater,
                        T::GreaterEqual => B::GreaterEqual,
                        T::Equal => B::Equal,
                        T::NotEqual => B::NotEqual,
                        T::In => B::In,
                        T::And => B::And,
                        T::Or => B::Or,
                        T::Concat => B::Concatenate,
                        _ => return Err(unsupported()),
                    };
                    K::Binary {
                        operator,
                        left,
                        right,
                    }
                }
            }
            N::ArrayLiteral => {
                if node.child_nodes().any(|n| n.kind() == N::IndexedArrayEntry) {
                    return Err(unsupported());
                }
                K::Array(self.children(node, depth)?)
            }
            N::TupleLiteral => K::Tuple(self.children(node, depth)?),
            N::SetLiteral => K::Set(self.children(node, depth)?),
            N::RecordLiteral => {
                let mut fields = slots(self.budget, node.child_nodes().count(), &range)?;
                for field in node.child_nodes() {
                    charge(self.budget, 1 + field.children().len(), &self.range(field))?;
                    let i = self.tokens(field).next().unwrap();
                    let label = self.name(i)?;
                    fields.push((
                        label,
                        self.node(field.child_nodes().next().unwrap(), depth + 1)?,
                    ));
                }
                K::Record(fields)
            }
            N::FieldAccessExpression => {
                let i = self.tokens(node).last().unwrap();
                let value = Box::new(self.node(child(), depth + 1)?);
                if self.parsed.tokens()[i].kind == T::IntegerLiteral {
                    let position = self.text(i).parse().map_err(|_| unsupported())?;
                    K::TupleAccess {
                        value,
                        position: Located::new(
                            position,
                            map_range(
                                self.parsed.tokens()[i].range.clone(),
                                self.offset,
                                self.prefix,
                            ),
                        ),
                    }
                } else {
                    K::Field {
                        value,
                        field: self.name(i)?,
                    }
                }
            }
            N::ArrayAccessExpression => {
                if node.child_nodes().count() != 2 {
                    return Err(unsupported());
                }
                let mut children = node.child_nodes();
                K::ArrayAccess {
                    array: Box::new(self.node(children.next().unwrap(), depth + 1)?),
                    index: Box::new(self.node(children.next().unwrap(), depth + 1)?),
                }
            }
            N::CallExpression => {
                let i = self.tokens(node).next().unwrap();
                if self.text(i) != "index_set" || node.child_nodes().count() != 1 {
                    return Err(unsupported());
                }
                K::IndexSet(Box::new(self.node(child(), depth + 1)?))
            }
            N::ConditionalExpression => {
                let mut branches = slots(self.budget, node.child_nodes().count(), &range)?;
                let mut otherwise = None;
                for branch in node.child_nodes() {
                    charge(
                        self.budget,
                        branch.children().len() + 1,
                        &self.range(branch),
                    )?;
                    let mut children = branch.child_nodes();
                    if branch.kind() == N::ElseBranch {
                        otherwise = Some(Box::new(self.node(children.next().unwrap(), depth + 1)?));
                    } else {
                        branches.push((
                            self.node(children.next().unwrap(), depth + 1)?,
                            self.node(children.next().unwrap(), depth + 1)?,
                        ));
                    }
                }
                K::Conditional {
                    branches,
                    otherwise: otherwise.ok_or_else(|| {
                        QueryError::query(
                            range.clone(),
                            "positional reports require an else branch",
                        )
                    })?,
                }
            }
            N::ArrayComprehension | N::SetComprehension => {
                let mut children = node.child_nodes();
                let head = Box::new(self.node(children.next().unwrap(), depth + 1)?);
                let generators = children.next().unwrap();
                let mut clauses = slots(self.budget, generators.child_nodes().count(), &range)?;
                for generator in generators.child_nodes() {
                    let grange = self.range(generator);
                    charge(self.budget, 1 + generator.children().len(), &grange)?;
                    let mut binders = slots(
                        self.budget,
                        self.tokens(generator)
                            .filter(|i| {
                                matches!(
                                    self.parsed.tokens()[*i].kind,
                                    T::Identifier | T::QuotedIdentifier | T::Anonymous
                                )
                            })
                            .count(),
                        &grange,
                    )?;
                    let mut single = false;
                    for element in generator.children() {
                        let SyntaxElement::Token(i) = element else {
                            continue;
                        };
                        let i = *i;
                        match self.parsed.tokens()[i].kind {
                            T::Equal => single = true,
                            T::Identifier | T::QuotedIdentifier => {
                                let name = self.name(i)?;
                                binders.push(Located::new(Some(name.value), name.range));
                            }
                            T::Anonymous => binders.push(Located::new(
                                None,
                                map_range(
                                    self.parsed.tokens()[i].range.clone(),
                                    self.offset,
                                    self.prefix,
                                ),
                            )),
                            _ => {}
                        }
                    }
                    let mut children = generator.child_nodes();
                    let source = self.node(children.next().unwrap(), depth + 1)?;
                    let filter = children
                        .next()
                        .map(|filter| self.node(filter.child_nodes().next().unwrap(), depth + 1))
                        .transpose()?;
                    let clause = if single {
                        ValueGenerator::Bind {
                            binder: binders.pop().unwrap(),
                            value: source,
                            filter,
                        }
                    } else {
                        ValueGenerator::In {
                            binders,
                            source,
                            filter,
                        }
                    };
                    clauses.push(Located::new(clause, grange));
                }
                K::Comprehension {
                    constructor: if node.kind() == N::ArrayComprehension {
                        zirql::ValueConstructor::Array
                    } else {
                        zirql::ValueConstructor::Set
                    },
                    clauses,
                    head,
                }
            }
            _ => return Err(unsupported()),
        };
        Ok(Located::new(kind, range))
    }
}
fn number(text: &str, range: &Range<usize>) -> Result<V, QueryError> {
    match structured::parse_number(text).map_err(|m| QueryError::query(range.clone(), m))? {
        structured::Scalar::Integer(v) => Ok(V::Integer(v)),
        structured::Scalar::Float(v) => Ok(V::Float(v)),
        _ => unreachable!(),
    }
}

pub(crate) fn literal_source(
    input: &Input,
    name: &str,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<V, QueryError> {
    let mut found = None;
    for node in input.syntax().tree().child_nodes() {
        charge(budget, 1 + node.children().len(), range)?;
        if node.kind() != N::Assignment {
            continue;
        }
        let i = structured::direct_token(input, node).unwrap();
        charge(budget, input.token_bytes(i).len(), range)?;
        if crate::identifier_identity(std::str::from_utf8(input.token_bytes(i)).unwrap()) == name {
            if found.is_some() {
                return Err(QueryError::input(
                    input.original_range(node.range()),
                    "literal source requires exactly one original assignment",
                ));
            }
            found = crate::assignment_rhs(node);
        }
    }
    let node = found.ok_or_else(|| {
        QueryError::query(
            range.clone(),
            "literal source requires exactly one original assignment",
        )
    })?;
    literal_value(input, node, budget, nesting)
}
fn literal_value(
    input: &Input,
    node: &SyntaxNode,
    budget: &mut Budget,
    nesting: usize,
) -> Result<V, QueryError> {
    let expression = Lower {
        parsed: input.syntax(),
        offset: input.syntax_offset(),
        prefix: 0,
        budget,
        nesting,
        literal_only: true,
    }
    .node(node, 0)
    .map_err(input_error)?;
    zirql::evaluate_value(&expression, &zirql::Bindings::default(), budget)
        .map_err(pipeline::core_error)
        .map_err(input_error)
}
fn input_error(mut error: QueryError) -> QueryError {
    error.location = crate::ErrorLocation::Input;
    error
}

pub(crate) fn project(
    stream: &SemanticStream<'_>,
    budget: &mut Budget,
    range: &Range<usize>,
    nesting: usize,
) -> Result<V, QueryError> {
    let SemanticStream::Inspection(result) = stream else {
        return Err(QueryError::query(
            range.clone(),
            "native selections require an explicit names, count or values projection",
        ));
    };
    match result.as_ref() {
        QueryResult::Count(n) => {
            charge(budget, 1, range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            Ok(V::Integer(i64::try_from(*n).map_err(|_| {
                QueryError::query(range.clone(), "count exceeds signed 64-bit range")
            })?))
        }
        QueryResult::Strings(strings) => {
            let mut values = slots(budget, strings.len(), range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            for string in strings {
                charge(budget, string.len() + 1, range)?;
                values.push(V::String(string.to_owned()));
            }
            Ok(V::Array(values))
        }
        QueryResult::Literals(selection) => {
            let mut values = slots(budget, selection.values().len(), range)?;
            budget
                .charge_results(1, zirql::Location::Query(range.clone()))
                .map_err(pipeline::core_error)?;
            for literal in selection.values() {
                let input = selection.input();
                let mut node = input.syntax().tree();
                let mut depth = 0;
                // Stay in the original CST so opaque comment bytes need no
                // decoding and each inspected value keeps its input location.
                while input.original_range(node.range()) != literal.range {
                    if depth >= nesting.min(64) {
                        return Err(QueryError::input(
                            literal.range.clone(),
                            "literal nesting limit exceeded",
                        ));
                    }
                    charge(budget, 1 + node.children().len(), range)?;
                    node = node
                        .child_nodes()
                        .find(|child| {
                            let child = input.original_range(child.range());
                            child.start <= literal.range.start && child.end >= literal.range.end
                        })
                        .ok_or_else(|| {
                            QueryError::input(literal.range.clone(), "literal has no original CST")
                        })?;
                    depth += 1;
                }
                values.push(literal_value(input, node, budget, nesting)?);
            }
            Ok(V::Array(values))
        }
        _ => Err(QueryError::query(
            range.clone(),
            "native result requires an explicit supported scalar projection",
        )),
    }
}

pub(crate) fn json_value(
    value: &V,
    budget: &mut Budget,
    range: &Range<usize>,
) -> Result<Value, QueryError> {
    zirql::constructed_value_type(value, budget, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    tagged(value, budget, range)
}
fn object<const N: usize>(fields: [(&str, Value); N]) -> Value {
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}
fn tagged(value: &V, budget: &mut Budget, range: &Range<usize>) -> Result<Value, QueryError> {
    charge(budget, 64, range)?;
    budget
        .charge_results(8, zirql::Location::Query(range.clone()))
        .map_err(pipeline::core_error)?;
    Ok(match value {
        V::Integer(v) => json!({"kind":"integer", "value":v}),
        V::Float(v) => json!({"kind":"float", "value":v}),
        V::Boolean(v) => json!({"kind":"boolean", "value":v}),
        V::String(v) => {
            charge(budget, v.len(), range)?;
            json!({"kind":"string", "value":v})
        }
        V::Record(fields) => {
            let mut result = slots(budget, fields.len(), range)?;
            for (label, value) in fields {
                charge(budget, label.len() + 32, range)?;
                budget
                    .charge_results(5, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                result.push(object([
                    ("label", Value::String(label.to_owned())),
                    ("value", tagged(value, budget, range)?),
                ]));
            }
            object([
                ("kind", Value::String("record".into())),
                ("fields", Value::Array(result)),
            ])
        }
        V::Array(values) => {
            let mut axes = slots(budget, values.len(), range)?;
            let mut entries = slots(budget, values.len(), range)?;
            for (i, value) in values.iter().enumerate() {
                let position = i
                    .checked_add(1)
                    .and_then(|n| i64::try_from(n).ok())
                    .ok_or_else(|| QueryError::query(range.clone(), "positional key overflow"))?;
                charge(budget, 96, range)?;
                budget
                    .charge_results(22, zirql::Location::Query(range.clone()))
                    .map_err(pipeline::core_error)?;
                axes.push(json!({"kind":"integer", "value":position}));
                let key = Value::Array(vec![json!({"kind":"integer", "value":position})]);
                entries.push(object([
                    ("key", key),
                    ("value", tagged(value, budget, range)?),
                ]));
            }
            object([
                ("kind", Value::String("array".into())),
                ("indexing", Value::String("positional".into())),
                ("axes", Value::Array(vec![Value::Array(axes)])),
                ("entries", Value::Array(entries)),
            ])
        }
        V::Tuple(values)
        | V::Set {
            elements: values, ..
        } => {
            let mut elements = slots(budget, values.len(), range)?;
            for value in values {
                elements.push(tagged(value, budget, range)?);
            }
            object([
                (
                    "kind",
                    Value::String(
                        if matches!(value, V::Tuple(_)) {
                            "tuple"
                        } else {
                            "set"
                        }
                        .into(),
                    ),
                ),
                ("elements", Value::Array(elements)),
            ])
        }
    })
}
