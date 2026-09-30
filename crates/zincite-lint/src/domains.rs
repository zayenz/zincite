//! Declared domains and located array index sets, independent of lint policy.
use crate::{
    BindingFacts, BindingResolution, DeclarationId, DeclarationRole, FileId, ModelContext,
    SourceLocation,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

/// An integer bound or retained binding identity. Unknown bounds may be open,
/// unbounded or data-dependent;
/// unsupported bounds exceed this producer's bounded integer interpretation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericBound {
    Integer(i64),
    Symbol(DeclarationId),
    /// A source-defined parameter retains identity alongside its written value.
    Defined {
        declaration: DeclarationId,
        value: Box<NumericBound>,
    },
    /// Supported arithmetic retains operands when data prevents evaluation.
    Arithmetic {
        operator: TokenKind,
        operands: Vec<NumericBound>,
    },
    Unknown,
    Unsupported(String),
}

/// Written domains retain named identity even when their definition is known.
/// Ranges use inclusive integer endpoints after adjusting half-open spellings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Domain {
    Range {
        lower: NumericBound,
        upper: NumericBound,
    },
    LiteralSet(Vec<NumericBound>),
    Named {
        declaration: DeclarationId,
        domain: Box<Domain>,
    },
    Enum(DeclarationId),
    UnconstrainedInt,
    UnconstrainedFloat,
    NonNumeric,
    /// Set element type; the type alone does not establish actual membership.
    Set(Box<Domain>),
    Array {
        indices: Vec<Domain>,
        element: Box<Domain>,
    },
    Unknown,
    Unsupported(String),
}

#[derive(Clone, Debug)]
pub struct DeclarationDomain {
    pub declaration: DeclarationId,
    pub domain: Domain,
}
#[derive(Clone, Debug)]
pub struct ArrayIndexSet {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub domain: Domain,
}
#[derive(Clone, Debug)]
pub struct DomainFacts {
    pub declarations: Vec<DeclarationDomain>,
    pub array_indices: Vec<ArrayIndexSet>,
}

/// Resolve written domains without evaluating data assignments. Symbolic
/// arithmetic retains its operands rather than requiring data. Source-defined
/// integer parameters and closed +, -, *, div and mod operations are checked;
/// aliases retain declaration IDs.
/// BindingFacts must belong to this same retained ModelContext; declaration IDs
/// are meaningful only within those sources. Existing binding, callable and
/// instantiation producers remain independent.
pub fn resolve_domains(context: &ModelContext, bindings: &BindingFacts) -> DomainFacts {
    let mut walker = Walker {
        context,
        bindings,
        active: Vec::new(),
    };
    let declarations = bindings
        .declarations
        .iter()
        .map(|declaration| DeclarationDomain {
            declaration: declaration.id,
            domain: walker.declared(declaration.id),
        })
        .collect();
    let mut array_indices = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            walker.inspect(file, item, node, &mut array_indices);
        }
    }
    DomainFacts {
        declarations,
        array_indices,
    }
}

struct Walker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    active: Vec<DeclarationId>,
}
impl Walker<'_> {
    fn inspect(
        &mut self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        out: &mut Vec<ArrayIndexSet>,
    ) {
        if node.kind() == NodeKind::ArrayType {
            let children: Vec<_> = node.child_nodes().collect();
            for index in &children[..children.len().saturating_sub(1)] {
                out.push(ArrayIndexSet {
                    file,
                    item,
                    location: self.context.files[file].location(index.range()),
                    domain: self.domain(file, index),
                });
            }
        }
        for child in node.child_nodes() {
            self.inspect(file, item, child, out);
        }
    }
    fn declared(&mut self, id: DeclarationId) -> Domain {
        if self.active.contains(&id) {
            return Domain::Unsupported("cyclic domain alias".into());
        }
        let declaration = &self.bindings.declarations[id.0];
        if declaration.role == DeclarationRole::Enum {
            return Domain::Enum(id);
        }
        let Some(node) = crate::callables::find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) else {
            return Domain::Unknown;
        };
        self.active.push(id);
        let children: Vec<_> = node.child_nodes().collect();
        let declared_type = children
            .iter()
            .find(|n| n.kind() != NodeKind::Annotation)
            .map(|n| self.domain(declaration.file, n))
            .unwrap_or(Domain::Unknown);
        let result = if declaration.role != DeclarationRole::TypeAlias && declared_type.is_set() {
            // Actual membership comes from a parameter initializer, including
            // declarations whose set type is reached through named aliases.
            let elements = if declaration.instantiation == crate::Instantiation::Parameter {
                children
                    .iter()
                    .find(|n| crate::callables::is_expression(n.kind()))
                    .map(|n| self.domain(declaration.file, n))
                    .unwrap_or(Domain::Unknown)
            } else {
                self.active.pop();
                return declared_type;
            };
            declared_type.with_set_elements(elements)
        } else {
            declared_type
        };
        self.active.pop();
        result
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let start = tokens(&self.context.files[file].parsed, node)
            .first()?
            .range
            .start
            + self.context.files[file].byte_offset;
        self.bindings
            .references
            .iter()
            .find(|r| r.file == file && r.location.range.start == start)
            .and_then(|r| {
                if let BindingResolution::Resolved(id) = r.resolution {
                    Some(id)
                } else {
                    None
                }
            })
    }
    fn domain(&mut self, file: FileId, node: &SyntaxNode) -> Domain {
        use NodeKind::*;
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            DomainType | ArrayIndexBinding | ParenthesizedExpression => children
                .first()
                .map(|n| self.domain(file, n))
                .unwrap_or(Domain::Unknown),
            ScalarType
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|t| t.kind == TokenKind::Int) =>
            {
                Domain::UnconstrainedInt
            }
            ScalarType => {
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|t| t.kind == TokenKind::Float)
                {
                    Domain::UnconstrainedFloat
                } else {
                    Domain::NonNumeric
                }
            }
            TupleType | RecordType => Domain::NonNumeric,
            ListType => Domain::Array {
                indices: vec![Domain::UnconstrainedInt],
                element: Box::new(
                    children
                        .last()
                        .map(|n| self.domain(file, n))
                        .unwrap_or(Domain::Unknown),
                ),
            },
            ArrayType => Domain::Array {
                indices: children[..children.len().saturating_sub(1)]
                    .iter()
                    .map(|n| self.domain(file, n))
                    .collect(),
                element: Box::new(
                    children
                        .last()
                        .map(|n| self.domain(file, n))
                        .unwrap_or(Domain::Unknown),
                ),
            },
            RangeExpression => {
                let marker = tokens(&self.context.files[file].parsed, node)
                    .into_iter()
                    .find(|t| {
                        matches!(
                            t.kind,
                            TokenKind::RangeInclusive
                                | TokenKind::RangeExclusiveStart
                                | TokenKind::RangeExclusiveEnd
                                | TokenKind::RangeExclusive
                        )
                    });
                let mut lower = NumericBound::Unknown;
                let mut upper = NumericBound::Unknown;
                if let Some(marker) = marker {
                    for child in children {
                        if child.range().start < marker.range.start {
                            lower = self.bound(file, child);
                        } else {
                            upper = self.bound(file, child);
                        }
                    }
                }
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|t| {
                        matches!(
                            t.kind,
                            TokenKind::RangeExclusiveStart | TokenKind::RangeExclusive
                        )
                    })
                {
                    lower = adjust(lower, 1);
                }
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|t| {
                        matches!(
                            t.kind,
                            TokenKind::RangeExclusiveEnd | TokenKind::RangeExclusive
                        )
                    })
                {
                    upper = adjust(upper, -1);
                }
                Domain::Range { lower, upper }
            }
            SetType => Domain::Set(Box::new(
                children
                    .last()
                    .map(|n| self.domain(file, n))
                    .unwrap_or(Domain::Unknown),
            )),
            SetLiteral => {
                Domain::LiteralSet(children.iter().map(|n| self.bound(file, n)).collect())
            }
            Expression => {
                if let Some(id) = self.reference(file, node) {
                    Domain::Named {
                        declaration: id,
                        domain: Box::new(self.declared(id)),
                    }
                } else {
                    Domain::Unsupported("unresolved or non-domain index expression".into())
                }
            }
            _ => Domain::Unsupported("domain form is outside bounded integer/set analysis".into()),
        }
    }
    fn parameter_bound(&mut self, id: DeclarationId) -> NumericBound {
        let declaration = &self.bindings.declarations[id.0];
        if declaration.instantiation != crate::Instantiation::Parameter
            || !matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Local
            )
        {
            return NumericBound::Symbol(id);
        }
        let Some(node) = crate::callables::find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) else {
            return NumericBound::Symbol(id);
        };
        let Some(value) = node
            .child_nodes()
            .find(|n| crate::callables::is_expression(n.kind()))
        else {
            return NumericBound::Symbol(id);
        };
        if self.active.contains(&id) {
            return NumericBound::Unsupported("cyclic numeric parameter definition".into());
        }
        self.active.push(id);
        let value = self.bound(declaration.file, value);
        self.active.pop();
        NumericBound::Defined {
            declaration: id,
            value: Box::new(value),
        }
    }
    fn bound(&mut self, file: FileId, node: &SyntaxNode) -> NumericBound {
        use NumericBound::*;
        let children: Vec<_> = node.child_nodes().collect();
        let direct = tokens(&self.context.files[file].parsed, node);
        match node.kind() {
            NodeKind::Expression => {
                if let Some(token) = direct.first() {
                    if token.kind == TokenKind::IntegerLiteral {
                        let text = &self.context.files[file].parsed.source()[token.range.clone()];
                        let value = if let Some(n) = text.strip_prefix("0x") {
                            i64::from_str_radix(n, 16)
                        } else if let Some(n) = text.strip_prefix("0o") {
                            i64::from_str_radix(n, 8)
                        } else if let Some(n) = text.strip_prefix("0b") {
                            i64::from_str_radix(n, 2)
                        } else {
                            text.parse()
                        };
                        return value
                            .map(Integer)
                            .unwrap_or_else(|_| Unsupported("integer bound exceeds i64".into()));
                    }
                    if matches!(
                        token.kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier
                    ) {
                        return self
                            .reference(file, node)
                            .map(|id| self.parameter_bound(id))
                            .unwrap_or_else(|| Unsupported("unresolved numeric bound".into()));
                    }
                    if token.kind == TokenKind::Infinity {
                        return Unknown;
                    }
                }
                Unsupported("non-integer bound".into())
            }
            NodeKind::ParenthesizedExpression => children
                .first()
                .map(|n| self.bound(file, n))
                .unwrap_or(Unknown),
            NodeKind::UnaryExpression if children.len() == 1 => {
                if !matches!(
                    direct.first().map(|t| t.kind),
                    Some(TokenKind::Plus | TokenKind::Minus)
                ) {
                    return Unsupported(
                        "numeric unary operator is outside bounded integer arithmetic".into(),
                    );
                }
                match self.bound(file, children[0]) {
                    Integer(n) if direct.iter().any(|t| t.kind == TokenKind::Minus) => n
                        .checked_neg()
                        .map(Integer)
                        .unwrap_or_else(arithmetic_failure),
                    Integer(n) if direct.iter().any(|t| t.kind == TokenKind::Plus) => Integer(n),
                    Unsupported(reason) => Unsupported(reason),
                    bound => Arithmetic {
                        operator: direct.first().map(|t| t.kind).unwrap_or(TokenKind::Plus),
                        operands: vec![bound],
                    },
                }
            }
            NodeKind::BinaryExpression if children.len() == 2 => {
                let left = self.bound(file, children[0]);
                let right = self.bound(file, children[1]);
                if let Unsupported(reason) = &left {
                    return Unsupported(reason.clone());
                }
                if let Unsupported(reason) = &right {
                    return Unsupported(reason.clone());
                }
                let operator = direct.first().map(|t| t.kind);
                if !matches!(
                    operator,
                    Some(
                        TokenKind::Plus
                            | TokenKind::Minus
                            | TokenKind::Star
                            | TokenKind::Div
                            | TokenKind::Mod
                    )
                ) {
                    return Unsupported(
                        "numeric operator is outside bounded integer arithmetic".into(),
                    );
                }
                if matches!(operator, Some(TokenKind::Div | TokenKind::Mod)) && right == Integer(0)
                {
                    return arithmetic_failure();
                }
                let (Integer(a), Integer(b)) = (&left, &right) else {
                    return Arithmetic {
                        operator: operator.unwrap(),
                        operands: vec![left, right],
                    };
                };
                let value = match direct.first().map(|t| t.kind) {
                    Some(TokenKind::Plus) => a.checked_add(*b),
                    Some(TokenKind::Minus) => a.checked_sub(*b),
                    Some(TokenKind::Star) => a.checked_mul(*b),
                    Some(TokenKind::Div) => a.checked_div(*b),
                    Some(TokenKind::Mod) => a.checked_rem(*b),
                    _ => {
                        return Unsupported(
                            "numeric operator is outside bounded integer arithmetic".into(),
                        );
                    }
                };
                value.map(Integer).unwrap_or_else(arithmetic_failure)
            }
            _ => Unsupported("numeric expression is outside bounded integer arithmetic".into()),
        }
    }
}
fn tokens<'a>(
    parsed: &'a zincite_syntax::ParsedFile,
    node: &SyntaxNode,
) -> Vec<&'a zincite_syntax::Token> {
    node.children()
        .iter()
        .filter_map(|child| {
            if let SyntaxElement::Token(index) = child {
                let token = &parsed.tokens()[*index];
                (!matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                ))
                .then_some(token)
            } else {
                None
            }
        })
        .collect()
}
fn arithmetic_failure() -> NumericBound {
    NumericBound::Unsupported("integer arithmetic overflow or division by zero".into())
}
fn adjust(bound: NumericBound, delta: i64) -> NumericBound {
    match bound {
        NumericBound::Integer(n) => n
            .checked_add(delta)
            .map(NumericBound::Integer)
            .unwrap_or_else(arithmetic_failure),
        bound @ (NumericBound::Symbol(_)
        | NumericBound::Defined { .. }
        | NumericBound::Arithmetic { .. }) => NumericBound::Arithmetic {
            operator: TokenKind::Plus,
            operands: vec![bound, NumericBound::Integer(delta)],
        },
        other => other,
    }
}

impl Domain {
    /// Whether a numeric value or array element has no explicit written domain.
    /// Named aliases are followed; unsupported/unknown domain interpretation
    /// stays distinct from a known nonnumeric or explicitly bounded type.
    pub fn has_unbounded_numeric_elements(&self) -> Result<bool, &str> {
        match self {
            Self::UnconstrainedInt | Self::UnconstrainedFloat => Ok(true),
            Self::Named { domain, .. } => domain.has_unbounded_numeric_elements(),
            Self::Array { element, .. } => element.has_unbounded_numeric_elements(),
            Self::Unknown => Err("declared domain is unknown"),
            Self::Unsupported(reason) => Err(reason),
            _ => Ok(false),
        }
    }

    /// A written numeric element domain, including through array/type aliases.
    pub fn has_explicit_numeric_domain(&self) -> bool {
        match self {
            Self::Range { .. } | Self::LiteralSet(_) => true,
            Self::Named { domain, .. } => domain.has_explicit_numeric_domain(),
            Self::Array { element, .. } => element.has_explicit_numeric_domain(),
            _ => false,
        }
    }
    fn is_set(&self) -> bool {
        match self {
            Self::Set(_) => true,
            Self::Named { domain, .. } => domain.is_set(),
            _ => false,
        }
    }
    fn with_set_elements(self, elements: Domain) -> Domain {
        match self {
            Self::Named {
                declaration,
                domain,
            } => Self::Named {
                declaration,
                domain: Box::new(domain.with_set_elements(elements)),
            },
            _ => elements,
        }
    }

    /// Actual least integer element when closed bounds prove nonemptiness.
    /// None covers empty sets, enums, open/unbounded values and symbolic uncertainty;
    /// Err identifies an unsupported interpretation rather than guessing.
    pub fn numeric_minimum(&self) -> Result<Option<i64>, &str> {
        minimum(self)
    }
}
fn integer(bound: &NumericBound) -> Result<Option<i64>, &str> {
    match bound {
        NumericBound::Integer(n) => Ok(Some(*n)),
        NumericBound::Unsupported(reason) => Err(reason),
        NumericBound::Defined { value, .. } => integer(value),
        NumericBound::Arithmetic { operator, operands } => {
            if !matches!(operands.len(), 1 | 2) {
                return Err("unsupported integer arithmetic arity");
            }
            let values = operands
                .iter()
                .map(integer)
                .collect::<Result<Vec<_>, _>>()?;
            let Some(a) = values.first().copied().flatten() else {
                return Ok(None);
            };
            let result = if values.len() == 1 {
                match operator {
                    TokenKind::Plus => Some(a),
                    TokenKind::Minus => a.checked_neg(),
                    _ => return Err("unsupported integer operator"),
                }
            } else {
                let Some(b) = values.get(1).copied().flatten() else {
                    return Ok(None);
                };
                match operator {
                    TokenKind::Plus => a.checked_add(b),
                    TokenKind::Minus => a.checked_sub(b),
                    TokenKind::Star => a.checked_mul(b),
                    TokenKind::Div => a.checked_div(b),
                    TokenKind::Mod => a.checked_rem(b),
                    _ => return Err("unsupported integer operator"),
                }
            };
            result
                .map(Some)
                .ok_or("integer arithmetic overflow or division by zero")
        }
        _ => Ok(None),
    }
}
fn minimum(domain: &Domain) -> Result<Option<i64>, &str> {
    match domain {
        Domain::Named { domain, .. } => minimum(domain),
        Domain::Range { lower, upper } => {
            let lower = integer(lower)?;
            let upper = integer(upper)?;
            Ok(match (lower, upper) {
                (Some(l), Some(u)) if l <= u => Some(l),
                _ => None,
            })
        }
        Domain::LiteralSet(bounds) => {
            let values: Vec<_> = bounds.iter().map(integer).collect::<Result<_, _>>()?;
            Ok(if values.iter().any(Option::is_none) {
                None
            } else {
                values.into_iter().flatten().min()
            })
        }
        Domain::Unsupported(reason) => Err(reason),
        _ => Ok(None),
    }
}

/// Reuse the domain walk for a retained generator source without deriving
/// membership from its set element type.
pub(super) fn expression_domain(
    context: &ModelContext,
    bindings: &BindingFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Domain {
    Walker {
        context,
        bindings,
        active: Vec::new(),
    }
    .domain(file, node)
}

/// Compare membership by named identity or closed supported values. None is
/// ordinary symbolic uncertainty. Closed ranges never need enumeration.
pub(super) fn same_members(left: &Domain, right: &Domain) -> Result<Option<bool>, String> {
    fn failure(domain: &Domain) -> Result<(), String> {
        match domain {
            Domain::Unsupported(reason) => Err(reason.clone()),
            Domain::Named { domain, .. } => failure(domain),
            Domain::Range { lower, upper } => {
                integer(lower).map_err(str::to_owned)?;
                integer(upper).map_err(str::to_owned)?;
                Ok(())
            }
            Domain::LiteralSet(values) => {
                for bound in values {
                    integer(bound).map_err(str::to_owned)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn bound_equal(a: &NumericBound, b: &NumericBound) -> Result<Option<bool>, String> {
        match (
            integer(a).map_err(str::to_owned)?,
            integer(b).map_err(str::to_owned)?,
        ) {
            (Some(a), Some(b)) => Ok(Some(a == b)),
            _ if a == b && !matches!(a, NumericBound::Unknown) => Ok(Some(true)),
            _ => Ok(None),
        }
    }
    fn closed_set(
        values: &[NumericBound],
    ) -> Result<Option<std::collections::BTreeSet<i64>>, String> {
        values
            .iter()
            .map(|v| integer(v).map_err(str::to_owned))
            .collect::<Result<Option<_>, _>>()
    }
    failure(left)?;
    failure(right)?;
    if let (Domain::Named { declaration: a, .. }, Domain::Named { declaration: b, .. }) =
        (left, right)
        && a == b
    {
        return Ok(Some(true));
    }
    if let Domain::Named { domain, .. } = left {
        return same_members(domain, right);
    }
    if let Domain::Named { domain, .. } = right {
        return same_members(left, domain);
    }
    match (left, right) {
        (Domain::Enum(a), Domain::Enum(b)) => Ok(Some(a == b)),
        (Domain::Range { lower: a, upper: b }, Domain::Range { lower: c, upper: d }) => {
            if let (Some(a), Some(b), Some(c), Some(d)) = (
                integer(a).map_err(str::to_owned)?,
                integer(b).map_err(str::to_owned)?,
                integer(c).map_err(str::to_owned)?,
                integer(d).map_err(str::to_owned)?,
            ) {
                return Ok(Some(if a > b || c > d {
                    a > b && c > d
                } else {
                    a == c && b == d
                }));
            }
            Ok(match (bound_equal(a, c)?, bound_equal(b, d)?) {
                (Some(true), Some(true)) => Some(true),
                (Some(false), _) | (_, Some(false)) => Some(false),
                _ => None,
            })
        }
        (Domain::LiteralSet(a), Domain::LiteralSet(b)) => {
            Ok(match (closed_set(a)?, closed_set(b)?) {
                (Some(a), Some(b)) => Some(a == b),
                _ => None,
            })
        }
        (Domain::Range { lower, upper }, Domain::LiteralSet(values))
        | (Domain::LiteralSet(values), Domain::Range { lower, upper }) => Ok(
            match (
                integer(lower).map_err(str::to_owned)?,
                integer(upper).map_err(str::to_owned)?,
                closed_set(values)?,
            ) {
                (Some(l), Some(u), Some(values)) if l > u => Some(values.is_empty()),
                (Some(l), Some(u), Some(values)) => Some(
                    values.first() == Some(&l)
                        && values.last() == Some(&u)
                        && values.len() as i128 == i128::from(u) - i128::from(l) + 1,
                ),
                _ => None,
            },
        ),
        _ => Ok(None),
    }
}

/// Checked closed integer value for this producer's existing arithmetic subset.
/// None retains ordinary symbolic uncertainty; Err is unsupported/invalid math.
pub(super) fn expression_integer(
    context: &ModelContext,
    bindings: &BindingFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Result<Option<i64>, String> {
    let bound = Walker {
        context,
        bindings,
        active: Vec::new(),
    }
    .bound(file, node);
    integer(&bound).map_err(str::to_owned)
}
