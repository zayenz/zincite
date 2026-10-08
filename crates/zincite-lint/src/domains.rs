//! Declared domains and located array index sets, independent of lint policy.
use crate::{
    BindingFacts, BindingResolution, DeclarationId, DeclarationRole, FileId, ModelContext,
    SourceLocation,
};
use std::collections::{BTreeSet, HashMap, HashSet};
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
    /// Checked parameter integer-set union; operands retain source identity.
    Union {
        left: Box<Domain>,
        right: Box<Domain>,
    },
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
    resolve_domains_with_callables(context, bindings, None)
}
pub(super) fn resolve_domains_with_callables<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: Option<&'a crate::CallableFacts>,
) -> DomainFacts {
    let mut walker = Walker {
        context,
        bindings,
        calls,
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

pub(super) fn parameter_set_cardinality(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<DeclarationId> {
    let integer = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == crate::Instantiation::Parameter
            && ty.kind == crate::TypeKind::Int
    };
    let set = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == crate::Instantiation::Parameter
            && matches!(&ty.kind, crate::TypeKind::Set(element) if integer(element))
    };
    let ty = |node: &SyntaxNode| {
        let range = context.files[file].location(node.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if node.kind() != NodeKind::CallExpression
        || crate::callables::core_operation(context, bindings, calls, file, node, "card")
            != Ok(true)
        || !crate::definitions::annotations_safe(context, file, node)
        || ty(node).is_none_or(|ty| !integer(ty))
        || !crate::callables::operation_fact(context, calls, file, node).is_some_and(|fact| {
            matches!(&fact.outcome, crate::CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && set(&parameters[0]) && integer(return_type))
        })
    {
        return None;
    }
    let args: Vec<_> = node.child_nodes().collect();
    let [source] = args.as_slice() else {
        return None;
    };
    parameter_integer_set_source(context, bindings, calls, file, source)
}

pub(super) fn parameter_integer_set_source(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    file: FileId,
    source: &SyntaxNode,
) -> Option<DeclarationId> {
    let integer = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == crate::Instantiation::Parameter
            && ty.kind == crate::TypeKind::Int
    };
    let set = |ty: &crate::TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && ty.instantiation == crate::Instantiation::Parameter
            && matches!(&ty.kind, crate::TypeKind::Set(element) if integer(element))
    };
    let range = context.files[file].location(source.range()).range;
    let written_name = tokens(&context.files[file].parsed, source);
    if source.kind() != NodeKind::Expression
        || source.child_nodes().next().is_some()
        || written_name.len() != 1
        || !matches!(
            written_name[0].kind,
            TokenKind::Identifier | TokenKind::QuotedIdentifier
        )
        || !calls
            .expressions
            .iter()
            .any(|e| e.file == file && e.location.range == range && set(&e.ty))
    {
        return None;
    }
    let id = crate::definitions::resolved_reference(context, bindings, file, source)?;
    let d = &bindings.declarations[id.0];
    if d.file != file
        || d.instantiation != crate::Instantiation::Parameter
        || !matches!(d.role, DeclarationRole::Value | DeclarationRole::Parameter)
        || !set(&calls.declarations[id.0].ty)
    {
        return None;
    }
    let declaration =
        crate::callables::find_node(context.files[file].parsed.tree(), &d.syntax_range, d.role)?;
    if !crate::definitions::annotations_safe(context, file, declaration)
        || declaration
            .child_nodes()
            .any(|n| crate::callables::is_expression(n.kind()))
    {
        return None;
    }
    let written = declaration.child_nodes().next()?;
    let elements: Vec<_> = written.child_nodes().collect();
    (written.kind() == NodeKind::SetType
        && elements.len() == 1
        && elements[0].kind() == NodeKind::ScalarType
        && elements[0].child_nodes().next().is_none()
        && tokens(&context.files[file].parsed, elements[0])
            .iter()
            .any(|t| t.kind == TokenKind::Int)
        && crate::definitions::annotations_safe(context, file, written))
    .then_some(id)
}

struct Walker<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: Option<&'a crate::CallableFacts>,
    active: Vec<DeclarationId>,
}
impl<'a> Walker<'a> {
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
            BinaryExpression
                if tokens(&self.context.files[file].parsed, node)
                    .iter()
                    .any(|token| token.kind == TokenKind::Union) =>
            {
                self.union_set(file, node)
                    .and_then(|domain| {
                        union_minimum(&domain).map_err(str::to_owned)?;
                        Ok(domain)
                    })
                    .unwrap_or_else(Domain::Unsupported)
            }
            _ => Domain::Unsupported("domain form is outside bounded integer/set analysis".into()),
        }
    }
    fn union_expression_type(&self, file: FileId, node: &SyntaxNode, set: bool) -> bool {
        let range = self.context.files[file].location(node.range()).range;
        self.calls
            .and_then(|calls| {
                calls.expressions.iter().find(|expression| {
                    expression.file == file && expression.location.range == range
                })
            })
            .is_some_and(|expression| union_parameter_type(&expression.ty, set))
    }
    fn union_operation(
        &self,
        file: FileId,
        node: &SyntaxNode,
        name: &str,
        set_arguments: bool,
        set_result: bool,
    ) -> bool {
        let Some(calls) = self.calls else {
            return false;
        };
        let Some(fact) = crate::callables::operation_fact(self.context, calls, file, node) else {
            return false;
        };
        if crate::callables::core_operation_outcome(
            self.context,
            self.bindings,
            Some(&fact.outcome),
            node.kind(),
            name,
        ) != Ok(true)
        {
            return false;
        }
        let arity = node.child_nodes().count();
        match &fact.outcome {
            crate::CallOutcome::Resolved {
                parameters,
                return_type,
                ..
            } => {
                parameters.len() == arity
                    && parameters
                        .iter()
                        .all(|ty| union_parameter_type(ty, set_arguments))
                    && union_parameter_type(return_type, set_result)
            }
            crate::CallOutcome::Intrinsic { name, return_type } => {
                name == "unary+"
                    && node.kind() == NodeKind::UnaryExpression
                    && arity == 1
                    && !set_arguments
                    && !set_result
                    && union_parameter_type(return_type, false)
            }
            _ => false,
        }
    }
    fn union_declaration(&self, id: DeclarationId, set: bool) -> Result<&'a SyntaxNode, String> {
        let declaration = &self.bindings.declarations[id.0];
        if self.active.contains(&id) {
            return Err("cyclic union source declaration".into());
        }
        if !declaration.top_level
            || declaration.role != DeclarationRole::Value
            || declaration.instantiation != crate::Instantiation::Parameter
            || !self.calls.is_some_and(|calls| {
                calls.declarations.get(id.0).is_some_and(|fact| {
                    fact.declaration == id && union_parameter_type(&fact.ty, set)
                })
            })
        {
            return Err("union source requires a present parameter integer declaration".into());
        }
        let node = crate::callables::find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("union source declaration is unavailable")?;
        if !crate::definitions::annotations_safe(self.context, declaration.file, node) {
            return Err("union source declaration annotations are unsupported".into());
        }
        Ok(node)
    }
    fn union_written_type(&mut self, file: FileId, node: &SyntaxNode) -> Result<(), String> {
        if !crate::definitions::annotations_safe(self.context, file, node) {
            return Err("union source type annotations are unsupported".into());
        }
        let children: Vec<_> = node
            .child_nodes()
            .filter(|child| child.kind() != NodeKind::Annotation)
            .collect();
        match node.kind() {
            NodeKind::ScalarType
                if children.is_empty()
                    && tokens(&self.context.files[file].parsed, node)
                        .iter()
                        .any(|token| token.kind == TokenKind::Int) =>
            {
                Ok(())
            }
            NodeKind::SetType if children.len() == 1 => self.union_written_type(file, children[0]),
            NodeKind::DomainType if children.len() == 1 => {
                self.union_set(file, children[0]).map(|_| ())
            }
            _ => Err("union source written type is unsupported".into()),
        }
    }
    // Only this checked union path follows complete typed parameter sources.
    // The raw domain resolver and its existing range behavior stay independent.
    fn union_set(&mut self, file: FileId, node: &SyntaxNode) -> Result<Domain, String> {
        if !self.union_expression_type(file, node, true)
            || !crate::definitions::annotations_safe(self.context, file, node)
        {
            return Err("union operand type or annotations are unsupported".into());
        }
        let children: Vec<_> = node
            .child_nodes()
            .filter(|child| child.kind() != NodeKind::Annotation)
            .collect();
        let domain = match node.kind() {
            NodeKind::ParenthesizedExpression if children.len() == 1 => {
                self.union_set(file, children[0])?
            }
            NodeKind::RangeExpression
                if children.len() == 2
                    && tokens(&self.context.files[file].parsed, node)
                        .iter()
                        .any(|token| token.kind == TokenKind::RangeInclusive)
                    && self.union_operation(file, node, "..", false, true) =>
            {
                self.union_integer_source(file, children[0])?;
                self.union_integer_source(file, children[1])?;
                Domain::Range {
                    lower: self.bound(file, children[0]),
                    upper: self.bound(file, children[1]),
                }
            }
            NodeKind::SetLiteral => {
                for child in &children {
                    self.union_integer_source(file, child)?;
                }
                Domain::LiteralSet(
                    children
                        .iter()
                        .map(|child| self.bound(file, child))
                        .collect(),
                )
            }
            NodeKind::BinaryExpression
                if children.len() == 2 && self.union_operation(file, node, "union", true, true) =>
            {
                let left = self.union_set(file, children[0])?;
                let right = self.union_set(file, children[1])?;
                Domain::Union {
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }
            NodeKind::Expression if children.is_empty() => {
                let direct = tokens(&self.context.files[file].parsed, node);
                if direct.len() != 1
                    || !matches!(
                        direct[0].kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier
                    )
                {
                    return Err("union source is not a bare parameter set".into());
                }
                let id = self
                    .reference(file, node)
                    .ok_or("union source reference is unresolved")?;
                let declaration = self.union_declaration(id, true)?;
                let source_file = self.bindings.declarations[id.0].file;
                self.active.push(id);
                let result = (|| {
                    let written = declaration
                        .child_nodes()
                        .next()
                        .ok_or("union source type is unavailable")?;
                    self.union_written_type(source_file, written)?;
                    declaration
                        .child_nodes()
                        .find(|child| crate::callables::is_expression(child.kind()))
                        .map_or(Ok(Domain::Unknown), |value| {
                            self.union_set(source_file, value)
                        })
                })();
                self.active.pop();
                Domain::Named {
                    declaration: id,
                    domain: Box::new(result?),
                }
            }
            _ => return Err("union source form is unsupported".into()),
        };
        Ok(domain)
    }
    fn union_integer_source(&mut self, file: FileId, node: &SyntaxNode) -> Result<(), String> {
        if !self.union_expression_type(file, node, false)
            || !crate::definitions::annotations_safe(self.context, file, node)
        {
            return Err("union bound type or annotations are unsupported".into());
        }
        let children: Vec<_> = node
            .child_nodes()
            .filter(|child| child.kind() != NodeKind::Annotation)
            .collect();
        match node.kind() {
            NodeKind::ParenthesizedExpression if children.len() == 1 => {
                self.union_integer_source(file, children[0])?
            }
            NodeKind::Expression if children.is_empty() => {
                let direct = tokens(&self.context.files[file].parsed, node);
                if direct.len() != 1 {
                    return Err("union bound form is unsupported".into());
                }
                match direct[0].kind {
                    TokenKind::IntegerLiteral => {}
                    TokenKind::Identifier | TokenKind::QuotedIdentifier => {
                        let id = self
                            .reference(file, node)
                            .ok_or("union bound reference is unresolved")?;
                        let declaration = self.union_declaration(id, false)?;
                        let source_file = self.bindings.declarations[id.0].file;
                        self.active.push(id);
                        let result = (|| {
                            let written = declaration
                                .child_nodes()
                                .next()
                                .ok_or("union bound type is unavailable")?;
                            self.union_written_type(source_file, written)?;
                            if let Some(value) = declaration
                                .child_nodes()
                                .find(|child| crate::callables::is_expression(child.kind()))
                            {
                                self.union_integer_source(source_file, value)?;
                            }
                            Ok::<_, String>(())
                        })();
                        self.active.pop();
                        result?;
                    }
                    _ => return Err("union bound form is unsupported".into()),
                }
            }
            NodeKind::UnaryExpression | NodeKind::BinaryExpression => {
                let direct = tokens(&self.context.files[file].parsed, node);
                let name = direct
                    .first()
                    .and_then(|token| crate::bindings::symbolic_operator(token.kind));
                let Some(name @ ("+" | "-" | "*" | "div" | "mod")) = name else {
                    return Err("union bound operation is unsupported".into());
                };
                let arity = if node.kind() == NodeKind::UnaryExpression {
                    1
                } else {
                    2
                };
                if children.len() != arity || !self.union_operation(file, node, name, false, false)
                {
                    return Err("union bound operation identity or signature is unsupported".into());
                }
                for child in children {
                    self.union_integer_source(file, child)?;
                }
            }
            _ => return Err("union bound form is unsupported".into()),
        }
        // Visit every source child before ordinary symbolic uncertainty can
        // hide an independently closed overflow or zero divisor.
        integer(&self.bound(file, node)).map_err(str::to_owned)?;
        Ok(())
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
    fn parameter_set_cardinality(&self, file: FileId, node: &SyntaxNode) -> bool {
        self.calls.is_some_and(|calls| {
            parameter_set_cardinality(self.context, self.bindings, calls, file, node).is_some()
        })
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
            NodeKind::CallExpression
                if self.calls.is_some_and(|calls| {
                    crate::callable_definitions::parameter_index_extremum(
                        self.context,
                        self.bindings,
                        calls,
                        file,
                        node,
                    )
                    .is_some()
                }) =>
            {
                Unknown
            }
            NodeKind::CallExpression if self.parameter_set_cardinality(file, node) => Unknown,
            _ => Unsupported("numeric expression is outside bounded integer arithmetic".into()),
        }
    }
}
pub(super) fn tokens<'a>(
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
/// Written Cartesian slots include anonymous binders, which have no ID.
pub(super) fn generator_slots(parsed: &zincite_syntax::ParsedFile, node: &SyntaxNode) -> usize {
    tokens(parsed, node)
        .into_iter()
        .take_while(|t| !matches!(t.kind, TokenKind::In | TokenKind::Equal))
        .filter(|t| {
            matches!(
                t.kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::Anonymous
            )
        })
        .count()
}

/// The selected standard index-set helper and its ordered array dimension.
pub(super) fn resolved_index_domain(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    domains: &DomainFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<(DeclarationId, usize, Domain)> {
    if node.kind() != NodeKind::CallExpression {
        return None;
    }
    let id = crate::definitions::resolved_call(context, calls, file, node)?;
    let name = &bindings.declarations[id.0].name;
    if !crate::definitions::core_callable(context, bindings, id, name) {
        return None;
    }
    let (dimension, rank) = if name == "index_set" {
        (1, 1)
    } else {
        let (d, r) = name.strip_prefix("index_set_")?.split_once("of")?;
        (d.parse::<usize>().ok()?, r.parse::<usize>().ok()?)
    };
    let mut argument = node.child_nodes().next()?;
    while matches!(
        argument.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
    ) {
        argument = argument.child_nodes().next()?;
    }
    if argument.kind() != NodeKind::Expression {
        return None;
    }
    let array = crate::definitions::resolved_reference(context, bindings, file, argument)?;
    let mut domain = &domains.declarations[array.0].domain;
    while let Domain::Named { domain: inner, .. } = domain {
        domain = inner;
    }
    if let Domain::Array { indices, .. } = domain {
        if indices.len() != rank {
            return None;
        }
        Some((
            array,
            dimension,
            indices.get(dimension.checked_sub(1)?)?.clone(),
        ))
    } else {
        None
    }
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

fn union_parameter_type(ty: &crate::TypeInst, set: bool) -> bool {
    ty.known()
        && !crate::value_safety::optional(ty)
        && ty.instantiation == crate::Instantiation::Parameter
        && if set {
            matches!(&ty.kind, crate::TypeKind::Set(element) if union_parameter_type(element, false))
        } else {
            ty.kind == crate::TypeKind::Int
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
            Self::Union { .. } => union_minimum(self).map(|_| false),
            Self::Unknown => Err("declared domain is unknown"),
            Self::Unsupported(reason) => Err(reason),
            _ => Ok(false),
        }
    }

    /// A written numeric element domain, including through array/type aliases.
    pub fn has_explicit_numeric_domain(&self) -> bool {
        match self {
            Self::Range { .. } | Self::LiteralSet(_) => true,
            Self::Union { .. } => union_minimum(self).is_ok(),
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

    /// Actual least integer element when closed bounds prove nonemptiness,
    /// including a checked union whose symbolic operand cannot lower its minimum.
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
        Domain::Union { .. } => union_minimum(domain).map(|proof| proof.minimum),
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

// A floor is conditional on nonemptiness; it is not necessarily attained.
// Keep it separate from an actual minimum so an uncertain empty operand cannot
// establish a union's least element on its own.
struct UnionMinimum {
    minimum: Option<i64>,
    floor: Option<i64>,
    empty: bool,
}
fn union_bound_value(bound: &NumericBound) -> Result<Option<i64>, &str> {
    fn symbolic(bound: &NumericBound) -> bool {
        match bound {
            NumericBound::Defined { .. } | NumericBound::Symbol(_) | NumericBound::Unknown => true,
            NumericBound::Arithmetic { operands, .. } => operands.iter().any(symbolic),
            _ => false,
        }
    }
    // Unlike the invariant query, this visits every retained operand for errors.
    let value = integer(bound)?;
    Ok(if symbolic(bound) { None } else { value })
}
fn union_minimum(domain: &Domain) -> Result<UnionMinimum, &str> {
    match domain {
        Domain::Named { domain, .. } => union_minimum(domain),
        Domain::Range { lower, upper } => {
            let lower = union_bound_value(lower)?;
            let upper = union_bound_value(upper)?;
            let minimum = match (lower, upper) {
                (Some(lower), Some(upper)) if lower <= upper => Some(lower),
                _ => None,
            };
            Ok(UnionMinimum {
                minimum,
                floor: lower,
                empty: matches!((lower, upper), (Some(lower), Some(upper)) if lower > upper),
            })
        }
        Domain::LiteralSet(values) => {
            let mut minimum: Option<i64> = None;
            let mut known = true;
            for value in values {
                if let Some(value) = union_bound_value(value)? {
                    minimum = Some(minimum.map_or(value, |previous| previous.min(value)));
                } else {
                    known = false;
                }
            }
            let minimum = known.then_some(minimum).flatten();
            Ok(UnionMinimum {
                minimum,
                floor: minimum,
                empty: values.is_empty(),
            })
        }
        Domain::Union { left, right } => {
            let left = union_minimum(left)?;
            let right = union_minimum(right)?;
            let minimum = match (left.minimum, right.minimum) {
                (Some(left), Some(right)) => Some(left.min(right)),
                (Some(minimum), None)
                    if right.empty || right.floor.is_some_and(|floor| floor >= minimum) =>
                {
                    Some(minimum)
                }
                (None, Some(minimum))
                    if left.empty || left.floor.is_some_and(|floor| floor >= minimum) =>
                {
                    Some(minimum)
                }
                _ => None,
            };
            let floor = if left.empty {
                right.floor
            } else if right.empty {
                left.floor
            } else {
                left.floor
                    .zip(right.floor)
                    .map(|(left, right)| left.min(right))
            };
            Ok(UnionMinimum {
                minimum,
                floor,
                empty: left.empty && right.empty,
            })
        }
        Domain::Unknown => Ok(UnionMinimum {
            minimum: None,
            floor: None,
            empty: false,
        }),
        Domain::Unsupported(reason) => Err(reason),
        _ => Err("union operand is outside integer-set domains"),
    }
}
fn union_error(domain: &Domain) -> Result<(), &str> {
    match domain {
        Domain::Named { domain, .. } => union_error(domain),
        Domain::Union { .. } => union_minimum(domain).map(|_| ()),
        Domain::Unsupported(reason) => Err(reason),
        _ => Ok(()),
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
        calls: None,
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
            Domain::Union { .. } => union_minimum(domain).map(|_| ()).map_err(str::to_owned),
            Domain::Range { lower, upper } => {
                invariant_integer(lower)?;
                invariant_integer(upper)?;
                Ok(())
            }
            Domain::LiteralSet(values) => {
                for bound in values {
                    invariant_integer(bound)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn bound_equal(a: &NumericBound, b: &NumericBound) -> Result<Option<bool>, String> {
        fn anonymous_unknown(bound: &NumericBound) -> bool {
            match bound {
                NumericBound::Unknown => true,
                NumericBound::Arithmetic { operands, .. } => operands.iter().any(anonymous_unknown),
                // Defined and Symbol retain their own declaration identity.
                _ => false,
            }
        }
        match (invariant_integer(a)?, invariant_integer(b)?) {
            (Some(a), Some(b)) => Ok(Some(a == b)),
            _ if a == b && !anonymous_unknown(a) => Ok(Some(true)),
            _ => Ok(None),
        }
    }
    fn closed_set(
        values: &[NumericBound],
    ) -> Result<Option<std::collections::BTreeSet<i64>>, String> {
        values
            .iter()
            .map(invariant_integer)
            .collect::<Result<Option<_>, _>>()
    }
    failure(left)?;
    failure(right)?;
    let mut alias = left;
    while let Domain::Named {
        declaration: a,
        domain,
    } = alias
    {
        let mut other = right;
        while let Domain::Named {
            declaration: b,
            domain,
        } = other
        {
            if a == b {
                return Ok(Some(true));
            }
            other = domain;
        }
        alias = domain;
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
                invariant_integer(a)?,
                invariant_integer(b)?,
                invariant_integer(c)?,
                invariant_integer(d)?,
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
                invariant_integer(lower)?,
                invariant_integer(upper)?,
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
        calls: None,
        active: Vec::new(),
    }
    .bound(file, node);
    integer(&bound).map_err(str::to_owned)
}

pub(super) fn invariant_expression_integer(
    context: &ModelContext,
    bindings: &BindingFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Result<Option<i64>, String> {
    let bound = Walker {
        context,
        bindings,
        calls: None,
        active: Vec::new(),
    }
    .bound(file, node);
    invariant_integer(&bound)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IntegerBoundsOutcome {
    Known { lower: i64, upper: i64 },
    Unknown(String),
    Unsupported(String),
}
#[derive(Clone, Debug)]
pub struct ExpressionBounds {
    pub file: FileId,
    pub location: SourceLocation,
    pub outcome: IntegerBoundsOutcome,
}
#[derive(Debug, Default)]
pub struct IntegerBoundsFacts {
    pub expressions: Vec<ExpressionBounds>,
}

/// A conservative integer value. Intervals are inclusive hulls, not a claim
/// that every enclosed integer is possible. Symbolic endpoints retain binding
/// identity and parameter-default dependence; they are never solved algebraically.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NumericOutcome {
    Exact(i64),
    Interval {
        lower: i64,
        upper: i64,
    },
    Symbolic {
        lower: NumericBound,
        upper: NumericBound,
    },
    Unknown(String),
    Unsupported(String),
}

/// A proof that all derived values are outside a declared domain. Overlapping
/// hulls, symbolic endpoints and lost correlations remain inconclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NumericDomainRelation {
    ExactOutside,
    Disjoint,
    Inconclusive,
}

impl NumericOutcome {
    /// Some(true) proves exclusion; Some(false) identifies the exact value.
    /// Interval containment is inconclusive, including holes in a sparse hull.
    pub fn excludes(&self, value: i64) -> Option<bool> {
        match self {
            Self::Exact(n) => Some(*n != value),
            Self::Interval { lower, upper } if value < *lower || value > *upper => Some(true),
            _ => None,
        }
    }

    /// Compare against an independently retained declared requirement. Exact
    /// sparse-set membership is checked without treating its hull as the set.
    /// A merely overlapping interval does not establish an actual failing value.
    pub fn domain_relation(&self, required: &NumericDeclaration) -> NumericDomainRelation {
        use NumericDomainRelation::*;
        let (Some((lower, upper)), Some((l, u))) = (self.interval(), required.required.interval())
        else {
            return Inconclusive;
        };
        let mut domain = &required.declared;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        if let Domain::LiteralSet(values) = domain {
            let values = values
                .iter()
                .map(invariant_integer)
                .collect::<Result<Option<Vec<_>>, _>>();
            let Ok(Some(values)) = values else {
                return Inconclusive;
            };
            if let Self::Exact(n) = self {
                return if values.contains(n) {
                    Inconclusive
                } else {
                    ExactOutside
                };
            }
            return if values.iter().all(|n| *n < lower || *n > upper) {
                Disjoint
            } else {
                Inconclusive
            };
        }
        if upper < l || lower > u {
            if matches!(self, Self::Exact(_)) {
                ExactOutside
            } else {
                Disjoint
            }
        } else {
            Inconclusive
        }
    }
    pub(super) fn interval(&self) -> Option<(i64, i64)> {
        match self {
            Self::Exact(n) => Some((*n, *n)),
            Self::Interval { lower, upper } => Some((*lower, *upper)),
            _ => None,
        }
    }
    fn endpoints(&self) -> Option<(NumericBound, NumericBound)> {
        match self {
            Self::Symbolic { lower, upper } => Some((lower.clone(), upper.clone())),
            _ => self
                .interval()
                .map(|(l, u)| (NumericBound::Integer(l), NumericBound::Integer(u))),
        }
    }
    fn invariant_bounds(self) -> IntegerBoundsOutcome {
        match self {
            Self::Exact(n) => IntegerBoundsOutcome::Known { lower: n, upper: n },
            Self::Interval { lower, upper } => IntegerBoundsOutcome::Known { lower, upper },
            Self::Symbolic { .. } => {
                IntegerBoundsOutcome::Unknown("numeric value depends on parameters".into())
            }
            Self::Unknown(reason) => IntegerBoundsOutcome::Unknown(reason),
            Self::Unsupported(reason) => IntegerBoundsOutcome::Unsupported(reason),
        }
    }
}
impl From<IntegerBoundsOutcome> for NumericOutcome {
    fn from(outcome: IntegerBoundsOutcome) -> Self {
        match outcome {
            IntegerBoundsOutcome::Known { lower, upper } => numeric_interval(lower, upper),
            IntegerBoundsOutcome::Unknown(reason) => Self::Unknown(reason),
            IntegerBoundsOutcome::Unsupported(reason) => Self::Unsupported(reason),
        }
    }
}

#[derive(Clone, Debug)]
pub struct NumericDeclaration {
    pub declaration: DeclarationId,
    pub location: SourceLocation,
    /// The original domain, including named identity and sparse membership.
    pub declared: Domain,
    pub required: NumericOutcome,
    /// Derived independently; never intersected with the declared requirement.
    pub value: NumericOutcome,
}
#[derive(Clone, Debug)]
pub struct NumericDefinition {
    pub target: DeclarationId,
    pub file: FileId,
    pub location: SourceLocation,
    /// A model parameter's written initializer can be replaced by instance data.
    /// Immutable let-local initializers are enforced definitions, not defaults.
    /// A default's RHS outcome is not an enforced value or target-conflict proof.
    pub parameter_default: bool,
    /// Interpretation of the written RHS, independently of the target's domain.
    pub outcome: NumericOutcome,
}
impl NumericDefinition {
    /// A parameter default cannot prove an instance-domain conflict. Other
    /// supported enforced RHS values can be compared with their own target.
    pub fn domain_relation(&self, required: &NumericDeclaration) -> NumericDomainRelation {
        if self.parameter_default || self.target != required.declaration {
            NumericDomainRelation::Inconclusive
        } else {
            self.outcome.domain_relation(required)
        }
    }
}
#[derive(Clone, Debug)]
pub struct NumericExpression {
    pub file: FileId,
    pub location: SourceLocation,
    pub instantiation: crate::Instantiation,
    pub outcome: NumericOutcome,
}
#[derive(Clone, Debug)]
pub struct NumericLimitation {
    pub file: FileId,
    pub location: SourceLocation,
    pub reason: String,
}
#[derive(Debug, Default)]
pub struct NumericFacts {
    pub declarations: Vec<NumericDeclaration>,
    pub definitions: Vec<NumericDefinition>,
    pub expressions: Vec<NumericExpression>,
    pub limitations: Vec<NumericLimitation>,
}

/// Interpret scalar integers without enabling lint. All prerequisites must come
/// from this retained ModelContext; their declaration IDs and locations cannot
/// be mixed with another model or a changed source.
///
/// Supports integer literals, aliases, parentheses/annotations, resolved core
/// unary +/-, +, -, *, and closed div/mod, plus supported scalar unconditional
/// definitions. Checked i64 arithmetic never wraps. Parameter defaults retain
/// identity instead of becoming instance-invariant values; independently known
/// parameter domains still bound arithmetic. A cycle cannot establish its own
/// value; independent domain or definition anchors remain usable.
///
/// Float, option, array-access, arbitrary call and control/iteration expressions
/// are outside this integer subset. Unsupported integer expressions and unsafe
/// definitions retain located limitations separately from facts and lint findings.
/// Ordinary uncertainty (missing data, cycles, conditional definitions, interval
/// division) is not an unsupported-operation finding. Sparse sets use hulls for
/// arithmetic; no correlation reasoning, solver or symbolic algebra is performed.
/// This producer does not change resolve_integer_bounds or selected-rule behavior.
pub fn resolve_numeric_facts(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    instantiations: &crate::InstantiationFacts,
    domains: &DomainFacts,
    definitions: &crate::DefinitionFacts,
) -> NumericFacts {
    let mut interpreter = Bounds::new(
        context,
        bindings,
        calls,
        domains,
        Some(definitions),
        Some(instantiations),
    );
    let mut instantiation_indices = HashMap::with_capacity(instantiations.expressions.len());
    for (index, expression) in instantiations.expressions.iter().enumerate() {
        instantiation_indices
            .entry((
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .or_insert(index);
    }
    let mut facts = NumericFacts::default();
    for declaration in &bindings.declarations {
        if !context.files[declaration.file].warnings_enabled()
            || !matches!(
                declaration.role,
                DeclarationRole::Value | DeclarationRole::Local | DeclarationRole::Parameter
            )
            || !matches!(
                calls.declarations[declaration.id.0].ty.kind,
                crate::TypeKind::Int | crate::TypeKind::Float | crate::TypeKind::Unknown(_)
            )
        {
            continue;
        }
        let required = interpreter.requirement(declaration.id);
        let value = interpreter.declaration(declaration.id);
        retain_numeric_limitation(
            &mut facts,
            declaration.file,
            &declaration.location,
            &required,
        );
        retain_numeric_limitation(&mut facts, declaration.file, &declaration.location, &value);
        facts.declarations.push(NumericDeclaration {
            declaration: declaration.id,
            location: declaration.location.clone(),
            declared: domains.declarations[declaration.id.0].domain.clone(),
            required,
            value,
        });
    }
    for definition in &definitions.definitions {
        if !context.files[definition.file].warnings_enabled()
            || !matches!(
                calls.declarations[definition.target.0].ty.kind,
                crate::TypeKind::Int | crate::TypeKind::Float | crate::TypeKind::Unknown(_)
            )
        {
            continue;
        }
        let outcome = interpreter.definition(definition);
        retain_numeric_limitation(&mut facts, definition.file, &definition.value, &outcome);
        facts.definitions.push(NumericDefinition {
            target: definition.target,
            file: definition.file,
            location: definition.value.clone(),
            parameter_default: calls.declarations[definition.target.0].ty.instantiation
                == crate::Instantiation::Parameter
                && bindings.declarations[definition.target.0].role != DeclarationRole::Local
                && definition.file == bindings.declarations[definition.target.0].file
                && definition.location.range
                    == context.files[definition.file]
                        .location(
                            bindings.declarations[definition.target.0]
                                .syntax_range
                                .clone(),
                        )
                        .range,
            outcome,
        });
    }
    // Preserve the first preorder expression for each retained range, including
    // duplicate supplied rows, without restarting at the file root for each row.
    let mut expression_nodes = vec![None; calls.expressions.len()];
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        let mut pending = vec![source.parsed.tree()];
        while let Some(node) = pending.pop() {
            if crate::callables::is_expression(node.kind()) {
                let range = node.range();
                let key = (
                    file,
                    range.start + source.byte_offset,
                    range.end + source.byte_offset,
                );
                if let Some(&row) = interpreter.expression_indices.get(&key) {
                    expression_nodes[row].get_or_insert(node);
                }
            }
            pending.extend(
                node.children()
                    .iter()
                    .rev()
                    .filter_map(|child| match child {
                        SyntaxElement::Node(node) => Some(node),
                        SyntaxElement::Token(_) => None,
                    }),
            );
        }
    }
    // Only this final pass reuses flat outcomes after declaration interpretation.
    interpreter.expression_memo = Some(vec![None; calls.expressions.len()]);
    for expression in &calls.expressions {
        let source = &context.files[expression.file];
        if !source.warnings_enabled()
            || !source.parsed.diagnostics().is_empty()
            || !matches!(
                expression.ty.kind,
                crate::TypeKind::Int | crate::TypeKind::Float | crate::TypeKind::Unknown(_)
            )
        {
            continue;
        }
        let key = (
            expression.file,
            expression.location.range.start,
            expression.location.range.end,
        );
        let Some(node) = interpreter
            .expression_indices
            .get(&key)
            .and_then(|&row| expression_nodes[row])
        else {
            continue;
        };
        let outcome = interpreter.expression(expression.file, node);
        retain_numeric_limitation(&mut facts, expression.file, &expression.location, &outcome);
        let instantiation = instantiation_indices
            .get(&(
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            ))
            .map_or(crate::Instantiation::Unknown, |&index| {
                instantiations.expressions[index].instantiation
            });
        facts.expressions.push(NumericExpression {
            file: expression.file,
            location: expression.location.clone(),
            instantiation,
            outcome,
        });
    }
    facts
}
fn retain_numeric_limitation(
    facts: &mut NumericFacts,
    file: FileId,
    location: &SourceLocation,
    outcome: &NumericOutcome,
) {
    if let NumericOutcome::Unsupported(reason) = outcome
        && !facts
            .limitations
            .iter()
            .any(|l| l.file == file && l.location.range == location.range && l.reason == *reason)
    {
        facts.limitations.push(NumericLimitation {
            file,
            location: location.clone(),
            reason: reason.clone(),
        });
    }
}
fn supported_definition(definition: &crate::Definition) -> bool {
    definition.enforcement == crate::DefinitionEnforcement::Enforced
        && definition.coverage == crate::DefinitionCoverage::Scalar
        && definition.safety == crate::DefinitionSafety::Supported
}
fn expression_node<'a>(
    node: &'a SyntaxNode,
    range: &std::ops::Range<usize>,
) -> Option<&'a SyntaxNode> {
    if node.range() == *range && crate::callables::is_expression(node.kind()) {
        return Some(node);
    }
    node.child_nodes()
        .filter(|n| n.range().start <= range.start && n.range().end >= range.end)
        .find_map(|n| expression_node(n, range))
}
fn numeric_interval(lower: i64, upper: i64) -> NumericOutcome {
    if lower == upper {
        NumericOutcome::Exact(lower)
    } else {
        NumericOutcome::Interval { lower, upper }
    }
}
fn numeric_arithmetic(
    operator: TokenKind,
    values: &[NumericOutcome],
    instance_invariant: bool,
) -> NumericOutcome {
    use NumericOutcome::*;
    let supported_operator = matches!(
        operator,
        TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Div | TokenKind::Mod
    );
    if !supported_operator && !instance_invariant {
        return Unsupported("integer operator is outside bounded arithmetic".into());
    }
    if let Some(reason) = values.iter().find_map(|v| {
        if let Unsupported(r) = v {
            Some(r)
        } else {
            None
        }
    }) {
        return Unsupported(reason.clone());
    }
    let unknown = values
        .iter()
        .find_map(|v| if let Unknown(r) = v { Some(r) } else { None });
    // Keep the invariant producer's uncertainty precedence. The definition-aware
    // path can still identify an invalid closed divisor with an unknown dividend.
    if matches!(operator, TokenKind::Div | TokenKind::Mod)
        && values.get(1) == Some(&Exact(0))
        && (!instance_invariant || unknown.is_none())
    {
        return Unsupported("integer division by zero".into());
    }
    if let Some(reason) = unknown {
        return Unknown(reason.clone());
    }
    if !supported_operator {
        return Unsupported("integer operator is outside bounded arithmetic".into());
    }
    let intervals: Option<Vec<_>> = values.iter().map(NumericOutcome::interval).collect();
    if let Some(intervals) = intervals {
        let result = match intervals.as_slice() {
            [(l, u)] if operator == TokenKind::Plus => Some((*l, *u)),
            [(l, u)] if operator == TokenKind::Minus => u.checked_neg().zip(l.checked_neg()),
            [(a, b), (c, d)] => match operator {
                TokenKind::Plus => a.checked_add(*c).zip(b.checked_add(*d)),
                TokenKind::Minus => a.checked_sub(*d).zip(b.checked_sub(*c)),
                TokenKind::Star => [
                    a.checked_mul(*c),
                    a.checked_mul(*d),
                    b.checked_mul(*c),
                    b.checked_mul(*d),
                ]
                .into_iter()
                .collect::<Option<Vec<_>>>()
                .map(|p| (*p.iter().min().unwrap(), *p.iter().max().unwrap())),
                TokenKind::Div | TokenKind::Mod if a == b && c == d => {
                    (if operator == TokenKind::Div {
                        a.checked_div(*c)
                    } else {
                        a.checked_rem(*c)
                    })
                    .map(|n| (n, n))
                }
                TokenKind::Div | TokenKind::Mod => {
                    return Unknown("division/remainder needs closed invariant operands".into());
                }
                _ => return Unsupported("integer operator arity is unsupported".into()),
            },
            _ => return Unsupported("integer operator arity is unsupported".into()),
        };
        return result
            .map(|(l, u)| numeric_interval(l, u))
            .unwrap_or_else(|| {
                Unsupported("integer arithmetic overflow or division by zero".into())
            });
    }
    let endpoints: Option<Vec<_>> = values.iter().map(NumericOutcome::endpoints).collect();
    let Some(endpoints) = endpoints else {
        return Unknown("numeric operands are unknown".into());
    };
    let arithmetic = |operands| NumericBound::Arithmetic { operator, operands };
    let (lower, upper) = match endpoints.as_slice() {
        [(l, u)] if operator == TokenKind::Plus => (l.clone(), u.clone()),
        [(l, u)] if operator == TokenKind::Minus => {
            (arithmetic(vec![u.clone()]), arithmetic(vec![l.clone()]))
        }
        [(a, b), (c, d)] if operator == TokenKind::Plus => (
            arithmetic(vec![a.clone(), c.clone()]),
            arithmetic(vec![b.clone(), d.clone()]),
        ),
        [(a, b), (c, d)] if operator == TokenKind::Minus => (
            arithmetic(vec![a.clone(), d.clone()]),
            arithmetic(vec![b.clone(), c.clone()]),
        ),
        [(a, b), (c, d)] if operator == TokenKind::Star && a == b && c == d => {
            let bound = arithmetic(vec![a.clone(), c.clone()]);
            (bound.clone(), bound)
        }
        [(..), (..)] if matches!(operator, TokenKind::Div | TokenKind::Mod) => {
            return Unknown("division/remainder needs closed invariant operands".into());
        }
        _ => return Unknown("symbolic product interval cannot be ordered without data".into()),
    };
    if let Err(reason) = invariant_integer(&lower).and_then(|_| invariant_integer(&upper)) {
        return Unsupported(reason);
    }
    Symbolic { lower, upper }
}

/// Retain nonempty integer intervals independently of lint. Prerequisites must
/// belong to this context. Numeric parameter definitions remain instance-dependent;
/// this producer never substitutes their defaults into an invariant bound.
/// Arithmetic uses resolved core identities, checked intervals and closed div/mod.
pub fn resolve_integer_bounds(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    domains: &DomainFacts,
) -> IntegerBoundsFacts {
    let eligible = calls
        .expressions
        .iter()
        .filter(|e| {
            matches!(
                e.ty.kind,
                crate::TypeKind::Int | crate::TypeKind::Unknown(_)
            )
        })
        .map(|e| (e.file, e.location.range.start, e.location.range.end))
        .collect();
    let mut interpreter = Bounds::new(context, bindings, calls, domains, None, None);
    interpreter.expression_memo = Some(vec![None; calls.expressions.len()]);
    let mut facts = IntegerBoundsFacts::default();
    for (file, source) in context.files.iter().enumerate() {
        if !source.warnings_enabled() || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        interpreter.walk(file, source.parsed.tree(), &eligible, &mut facts);
    }
    facts
}
struct Bounds<'a> {
    reference_indices: HashMap<(FileId, usize), usize>,
    call_indices: HashMap<(FileId, usize), usize>,
    expression_indices: HashMap<(FileId, usize, usize), usize>,
    expression_memo: Option<Vec<Option<(NodeKind, NumericOutcome)>>>,
    float_locations: HashSet<(FileId, usize, usize)>,
    value_reference_indices: Vec<Vec<usize>>,
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a crate::CallableFacts,
    domains: &'a DomainFacts,
    definitions: Option<&'a crate::DefinitionFacts>,
    instantiations: Option<&'a crate::InstantiationFacts>,
    declarations: Vec<Option<NumericOutcome>>,
    active: Vec<DeclarationId>,
}
impl<'a> Bounds<'a> {
    fn new(
        context: &'a ModelContext,
        bindings: &'a BindingFacts,
        calls: &'a crate::CallableFacts,
        domains: &'a DomainFacts,
        definitions: Option<&'a crate::DefinitionFacts>,
        instantiations: Option<&'a crate::InstantiationFacts>,
    ) -> Self {
        let mut reference_indices = HashMap::with_capacity(bindings.references.len());
        for (index, reference) in bindings.references.iter().enumerate() {
            reference_indices
                .entry((reference.file, reference.location.range.start))
                .or_insert(index);
        }
        let mut call_indices = HashMap::with_capacity(calls.calls.len());
        for (index, call) in calls.calls.iter().enumerate() {
            call_indices
                .entry((call.file, call.location.range.start))
                .or_insert(index);
        }
        let mut expression_indices = HashMap::with_capacity(calls.expressions.len());
        let mut float_locations = HashSet::new();
        for (index, expression) in calls.expressions.iter().enumerate() {
            let key = (
                expression.file,
                expression.location.range.start,
                expression.location.range.end,
            );
            expression_indices.entry(key).or_insert(index);
            if expression.ty.kind == crate::TypeKind::Float {
                float_locations.insert(key);
            }
        }
        let mut value_reference_indices = vec![Vec::new(); context.files.len()];
        if definitions.is_some() {
            for (row, reference) in bindings.references.iter().enumerate() {
                if reference.kind == crate::ReferenceKind::Value {
                    value_reference_indices[reference.file].push(row);
                }
            }
            for indices in &mut value_reference_indices {
                indices.sort_unstable_by_key(|&row| {
                    (bindings.references[row].location.range.start, row)
                });
            }
        }
        Self {
            reference_indices,
            call_indices,
            expression_indices,
            expression_memo: None,
            float_locations,
            value_reference_indices,
            context,
            bindings,
            calls,
            domains,
            definitions,
            instantiations,
            declarations: if definitions.is_some() {
                vec![None; bindings.declarations.len()]
            } else {
                Vec::new()
            },
            active: Vec::new(),
        }
    }
    fn walk(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        eligible: &BTreeSet<(FileId, usize, usize)>,
        facts: &mut IntegerBoundsFacts,
    ) {
        let location = self.context.files[file].location(node.range());
        if eligible.contains(&(file, location.range.start, location.range.end)) {
            facts.expressions.push(ExpressionBounds {
                file,
                location,
                outcome: self.expression(file, node).invariant_bounds(),
            });
        }
        for child in node.child_nodes() {
            self.walk(file, child, eligible, facts);
        }
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let start = tokens(&self.context.files[file].parsed, node)
            .first()?
            .range
            .start
            + self.context.files[file].byte_offset;
        let reference = &self.bindings.references[*self.reference_indices.get(&(file, start))?];
        match reference.resolution {
            BindingResolution::Resolved(id) => Some(id),
            _ => None,
        }
    }
    fn literal_bool_selection(&self, file: FileId, access: &SyntaxNode) -> bool {
        let children: Vec<_> = access.child_nodes().collect();
        let [subject, index] = children.as_slice() else {
            return false;
        };
        let parsed = &self.context.files[file].parsed;
        if subject.kind() != NodeKind::Expression
            || !matches!(tokens(parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || index.kind() != NodeKind::Expression
            || index.child_nodes().next().is_some()
            || !matches!(tokens(parsed, index).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            return false;
        }
        let Some(array) = self.reference(file, subject) else {
            return false;
        };
        let declared = &self.calls.declarations[array.0].ty;
        let type_of = |node: &SyntaxNode| {
            let range = node.range();
            let offset = self.context.files[file].byte_offset;
            let key = (file, range.start + offset, range.end + offset);
            self.expression_indices
                .get(&key)
                .filter(|_| !self.float_locations.contains(&key))
                .map(|&row| &self.calls.expressions[row].ty)
        };
        let optional = crate::value_safety::optional;
        if !declared.known()
            || optional(declared)
            || type_of(subject) != Some(declared)
            || type_of(access)
                .is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != crate::TypeKind::Bool)
            || type_of(index).is_none_or(|ty| {
                !ty.known()
                    || optional(ty)
                    || ty.instantiation != crate::Instantiation::Parameter
                    || ty.kind != crate::TypeKind::Int
            })
            || !matches!(&declared.kind, crate::TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == crate::Instantiation::Parameter
                    && indices[0].kind == crate::TypeKind::Int && element.kind == crate::TypeKind::Bool)
        {
            return false;
        }
        let Domain::Array { indices, .. } = &self.domains.declarations[array.0].domain else {
            return false;
        };
        let [Domain::Range { .. }] = indices.as_slice() else {
            return false;
        };
        let Some((lower, upper)) = index_domain_interval(&indices[0]) else {
            return false;
        };
        if !invariant_expression_integer(self.context, self.bindings, file, index)
            .is_ok_and(|value| value.is_some_and(|value| lower <= value && value <= upper))
        {
            return false;
        }
        let owner = &self.bindings.declarations[array.0];
        if owner.role != DeclarationRole::Value || !owner.top_level {
            return false;
        }
        let Some(written) = crate::callables::find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) else {
            return false;
        };
        if !crate::definitions::annotations_safe(self.context, owner.file, written)
            || written
                .child_nodes()
                .any(|node| crate::callables::is_expression(node.kind()))
        {
            return false;
        }
        let Some(array_type) = written
            .child_nodes()
            .next()
            .filter(|node| node.kind() == NodeKind::ArrayType)
        else {
            return false;
        };
        let types: Vec<_> = array_type.child_nodes().collect();
        if types.len() != 2 || types[0].kind() != NodeKind::DomainType {
            return false;
        }
        let axes: Vec<_> = types[0].child_nodes().collect();
        if axes.len() != 1
            || axes[0].kind() != NodeKind::RangeExpression
            || crate::callables::core_operation(
                self.context,
                self.bindings,
                self.calls,
                owner.file,
                axes[0],
                "..",
            ) != Ok(true)
            || self.type_operations(array).is_err()
        {
            return false;
        }
        true
    }
    fn length(&self, file: FileId, node: &SyntaxNode) -> Option<NumericOutcome> {
        // Only numeric interpretation has the actual instantiation facts needed
        // by the existing source inspector. Independent bounds stay unchanged.
        let instantiations = self.instantiations?;
        let outcome = crate::callables::operation_head_start(self.context, file, node)
            .and_then(|start| self.call_indices.get(&(file, start)))
            .map(|&index| &self.calls.calls[index].outcome);
        if crate::callables::core_operation_outcome(
            self.context,
            self.bindings,
            outcome,
            node.kind(),
            "length",
        ) != Ok(true)
        {
            return None;
        }
        let argument = crate::callable_definitions::length_argument(
            self.context,
            self.bindings,
            self.calls,
            file,
            node,
        )?;
        let written = tokens(&self.context.files[file].parsed, argument);
        if argument.kind() != NodeKind::Expression
            || argument.child_nodes().next().is_some()
            || !matches!(written.as_slice(), [token]
                if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        {
            return None;
        }
        let id = self.reference(file, argument)?;
        let declaration = &self.bindings.declarations[id.0];
        if declaration.role != DeclarationRole::Value || !declaration.top_level {
            return None;
        }
        let range = self.context.files[file].location(argument.range()).range;
        let actual = self
            .calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)?;
        if actual.ty != self.calls.declarations[id.0].ty {
            return None;
        }
        // Borrow retained syntax; inspect original initialization, written
        // domains, annotations and children before an ordinary numeric decline.
        let retained = expression_node(self.context.files[file].parsed.tree(), &node.range())?;
        Some(
            match crate::callable_definitions::direct_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                instantiations,
                self.domains,
                (file, retained),
                &[],
            ) {
                crate::DefinitionSafety::Supported => NumericOutcome::Unknown(
                    "collection length is not an established numeric value".into(),
                ),
                crate::DefinitionSafety::Unknown(reason) => NumericOutcome::Unknown(reason),
                crate::DefinitionSafety::Unsupported(reason) => NumericOutcome::Unsupported(reason),
            },
        )
    }
    fn expression(&mut self, file: FileId, node: &SyntaxNode) -> NumericOutcome {
        if !self.active.is_empty() || self.expression_memo.is_none() {
            return self.expression_uncached(file, node);
        }
        let source = &self.context.files[file];
        let range = node.range();
        let row = self
            .expression_indices
            .get(&(
                file,
                range.start + source.byte_offset,
                range.end + source.byte_offset,
            ))
            .copied();
        if let Some(row) = row
            && let Some(memo) = self.expression_memo.as_ref()
            && let Some((kind, outcome)) = &memo[row]
            && *kind == node.kind()
        {
            return outcome.clone();
        }
        let outcome = self.expression_uncached(file, node);
        // Symbolic trees can grow with every prefix. A lazy declaration fill
        // disables reuse permanently, so a previously unknown value cannot stick.
        if !matches!(&outcome, NumericOutcome::Symbolic { .. })
            && self.active.is_empty()
            && let Some(row) = row
            && let Some(memo) = self.expression_memo.as_mut()
        {
            memo[row] = Some((node.kind(), outcome.clone()));
        }
        outcome
    }
    fn expression_uncached(&mut self, file: FileId, node: &SyntaxNode) -> NumericOutcome {
        use NumericOutcome::*;
        let range = self.context.files[file].location(node.range()).range;
        let key = (file, range.start, range.end);
        let expression = self.expression_indices.get(&key);
        if expression
            .is_some_and(|&index| crate::value_safety::optional(&self.calls.expressions[index].ty))
        {
            return Unsupported("optional integer bounds require a presence proof".into());
        }
        if self.definitions.is_some() && self.float_locations.contains(&key) {
            return Unsupported("floating-point bounds are outside integer interpretation".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::AnnotatedExpression if self.definitions.is_some() => {
                if !crate::definitions::annotations_safe(self.context, file, node) {
                    let mut nodes: Vec<_> = children.first().copied().into_iter().collect();
                    while let Some(value) = nodes.pop() {
                        if value.kind() == NodeKind::CallExpression {
                            let outcome =
                                crate::callables::operation_head_start(self.context, file, value)
                                    .and_then(|start| self.call_indices.get(&(file, start)))
                                    .map(|&index| &self.calls.calls[index].outcome);
                            if matches!(outcome,
                            Some(crate::CallOutcome::Resolved { declaration, .. })
                                if ["length", "min", "max", "bool2int"].iter().any(|name| {
                                    crate::definitions::core_callable(
                                        self.context, self.bindings, *declaration, name,
                                    )
                                }))
                            {
                                return Unsupported(
                                    "numeric inspection call annotation is unsupported".into(),
                                );
                            }
                        }
                        nodes.extend(value.child_nodes());
                    }
                }
                children
                    .first()
                    .map(|n| self.expression(file, n))
                    .unwrap_or_else(|| Unsupported("integer operand is missing".into()))
            }
            NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression => children
                .first()
                .map(|n| self.expression(file, n))
                .unwrap_or_else(|| Unsupported("integer operand is missing".into())),
            NodeKind::Expression => {
                let direct = tokens(&self.context.files[file].parsed, node);
                if direct
                    .first()
                    .is_some_and(|t| t.kind == TokenKind::IntegerLiteral)
                {
                    let value = Walker {
                        context: self.context,
                        bindings: self.bindings,
                        calls: None,
                        active: Vec::new(),
                    }
                    .bound(file, node);
                    return match invariant_integer(&value) {
                        Ok(Some(n)) => Exact(n),
                        Ok(None) => Unknown("numeric value depends on parameters".into()),
                        Err(reason) => Unsupported(reason),
                    };
                }
                let Some(id) = self.reference(file, node) else {
                    return Unsupported("integer reference is unresolved or ambiguous".into());
                };
                if self.definitions.is_some() {
                    self.declaration(id)
                } else {
                    if let Err(reason) = self.type_operations(id) {
                        return Unsupported(reason);
                    }
                    self.domain(&self.domains.declarations[id.0].domain).into()
                }
            }
            NodeKind::ArrayAccessExpression => {
                if self.definitions.is_some() {
                    if self.literal_bool_selection(file, node) {
                        return Unknown("selected Boolean numeric value is unproved".into());
                    }
                    return Unsupported(
                        "numeric array access requires an index-membership proof".into(),
                    );
                }
                let Some(id) = children.first().and_then(|n| self.reference(file, n)) else {
                    return Unsupported("array identity is unavailable".into());
                };
                self.array_elements(id).into()
            }
            NodeKind::CallExpression if self.definitions.is_some() => {
                if let Some(value) = self.length(file, node) {
                    return value;
                }
                let integer = |t: &crate::TypeInst| {
                    t.known() && !t.optional && t.kind == crate::TypeKind::Int
                };
                let typed = |value: &SyntaxNode| {
                    let range = value.range();
                    let offset = self.context.files[file].byte_offset;
                    self.expression_indices
                        .get(&(file, range.start + offset, range.end + offset))
                        .map(|&index| &self.calls.expressions[index].ty)
                };
                let outcome = crate::callables::operation_head_start(self.context, file, node)
                    .and_then(|start| self.call_indices.get(&(file, start)))
                    .map(|&index| &self.calls.calls[index].outcome);
                if matches!(outcome,
                Some(crate::CallOutcome::Resolved { declaration, .. })
                    if crate::definitions::core_callable(
                        self.context, self.bindings, *declaration, "bool2int",
                    ))
                {
                    let boolean = |t: &crate::TypeInst| {
                        t.known()
                            && !t.optional
                            && t.kind == crate::TypeKind::Bool
                            && matches!(
                                t.instantiation,
                                crate::Instantiation::Parameter | crate::Instantiation::Decision
                            )
                    };
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || typed(node).is_none_or(|t| !integer(t))
                        || typed(children[0]).is_none_or(|t| !boolean(t))
                        || !matches!(outcome,
                            Some(crate::CallOutcome::Resolved { parameters, return_type, .. })
                                if parameters.len() == 1 && boolean(&parameters[0])
                                    && integer(return_type))
                    {
                        return Unsupported("bool2int numeric inspection requires a present scalar Boolean signature".into());
                    }
                    if !crate::definitions::annotations_safe(self.context, file, node) {
                        return Unsupported("Boolean conversion annotation is unsupported".into());
                    }
                    let Some(instantiations) = self.instantiations else {
                        return Unsupported(
                            "Boolean conversion instantiation facts are unavailable".into(),
                        );
                    };
                    let Some(argument) = expression_node(
                        self.context.files[file].parsed.tree(),
                        &children[0].range(),
                    ) else {
                        return Unsupported(
                            "Boolean conversion source argument is unavailable".into(),
                        );
                    };
                    return match crate::callable_definitions::initialized_expression_safety(
                        self.context,
                        self.bindings,
                        self.calls,
                        instantiations,
                        self.domains,
                        (file, argument, &[], false),
                        Some(crate::callable_definitions::DirectSafetyLookups {
                            expressions: &self.expression_indices,
                            calls: &self.call_indices,
                            value_references: &self.value_reference_indices,
                        }),
                    ) {
                        crate::DefinitionSafety::Supported => {
                            Unknown("scalar Boolean conversion value is unproved".into())
                        }
                        crate::DefinitionSafety::Unknown(reason) => Unknown(reason),
                        crate::DefinitionSafety::Unsupported(reason) => Unsupported(reason),
                    };
                }
                if children.len() != 2
                    || children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
                    || typed(node).is_none_or(|t| !integer(t))
                    || children
                        .iter()
                        .any(|child| typed(child).is_none_or(|t| !integer(t)))
                    || !matches!(outcome,
                    Some(crate::CallOutcome::Resolved { declaration, parameters, return_type })
                        if parameters.len() == 2
                            && parameters.iter().all(integer)
                            && integer(return_type)
                            && ["min", "max"].iter().any(|name| {
                                crate::definitions::core_callable(
                                    self.context, self.bindings, *declaration, name,
                                )
                            }))
                {
                    return Unsupported("integer call is outside numeric interpretation".into());
                }
                let mut nodes = vec![node];
                while let Some(value) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.context, file, value) {
                        return Unsupported(
                            "scalar extremum operand annotation is unsupported".into(),
                        );
                    }
                    nodes.extend(value.child_nodes());
                }
                let values: Vec<_> = children.iter().map(|n| self.expression(file, n)).collect();
                if let Some(reason) = values.iter().find_map(|value| {
                    if let Unsupported(reason) = value {
                        Some(reason)
                    } else {
                        None
                    }
                }) {
                    return Unsupported(reason.clone());
                }
                Unknown("scalar integer extremum value is unproved".into())
            }
            NodeKind::UnaryExpression | NodeKind::BinaryExpression => {
                let Some(operator) = tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|t| t.kind)
                else {
                    return Unsupported("integer operator is missing".into());
                };
                let Some(name) = crate::bindings::symbolic_operator(operator) else {
                    return Unsupported("integer operator is outside bounded arithmetic".into());
                };
                let outcome = crate::callables::operation_head_start(self.context, file, node)
                    .and_then(|start| self.call_indices.get(&(file, start)))
                    .map(|&index| &self.calls.calls[index].outcome);
                match crate::callables::core_operation_outcome(
                    self.context,
                    self.bindings,
                    outcome,
                    node.kind(),
                    name,
                ) {
                    Ok(true) => {}
                    Ok(false) => {
                        return Unsupported(
                            "user integer operation has no supported bounds".into(),
                        );
                    }
                    Err(reason) => return Unsupported(reason),
                }
                let values: Vec<_> = children.iter().map(|n| self.expression(file, n)).collect();
                numeric_arithmetic(operator, &values, self.definitions.is_none())
            }
            NodeKind::CallExpression => self.length(file, node).unwrap_or_else(|| {
                Unsupported("integer expression is outside bounded interpretation".into())
            }),
            _ => Unsupported("integer expression is outside bounded interpretation".into()),
        }
    }
    fn requirement(&self, id: DeclarationId) -> NumericOutcome {
        let ty = &self.calls.declarations[id.0].ty;
        if crate::value_safety::optional(ty) {
            return NumericOutcome::Unsupported(
                "optional integer bounds require a presence proof".into(),
            );
        }
        if let Err(reason) = self.type_operations(id) {
            return NumericOutcome::Unsupported(reason);
        }
        self.numeric_domain(&self.domains.declarations[id.0].domain)
    }
    fn numeric_domain(&self, domain: &Domain) -> NumericOutcome {
        if let Domain::Named {
            declaration,
            domain,
        } = domain
        {
            let ty = &self.calls.declarations[declaration.0].ty;
            if ty.instantiation == crate::Instantiation::Parameter
                && matches!(ty.kind, crate::TypeKind::Set(_))
            {
                return NumericOutcome::Unknown(
                    "numeric domain membership depends on a parameter set".into(),
                );
            }
            if let Err(reason) = self.type_operations(*declaration) {
                return NumericOutcome::Unsupported(reason);
            }
            return self.numeric_domain(domain);
        }
        if let Domain::Range { lower, upper } = domain {
            match (invariant_integer(lower), invariant_integer(upper)) {
                (Err(reason), _) | (_, Err(reason)) => return NumericOutcome::Unsupported(reason),
                (Ok(Some(_)), Ok(Some(_))) => {}
                _ => {
                    return NumericOutcome::Symbolic {
                        lower: lower.clone(),
                        upper: upper.clone(),
                    };
                }
            }
        }
        self.domain(domain).into()
    }
    fn declaration(&mut self, id: DeclarationId) -> NumericOutcome {
        if let Some(value) = &self.declarations[id.0] {
            return value.clone();
        }
        let required = self.requirement(id);
        if self.active.contains(&id) {
            // An active equality cannot establish its own value. Written bounds
            // are independent anchors and remain usable even in a cycle.
            return required;
        }
        let ty = &self.calls.declarations[id.0].ty;
        if !matches!(ty.kind, crate::TypeKind::Int) || crate::value_safety::optional(ty) {
            return NumericOutcome::Unsupported(
                "declaration is not a supported scalar integer".into(),
            );
        }
        self.active.push(id);
        let value = if ty.instantiation == crate::Instantiation::Parameter
            && self.bindings.declarations[id.0].role != DeclarationRole::Local
        {
            // Data can replace written defaults. Preserve identity, never the
            // default as an exact value of every model instance.
            if required.interval().is_some() {
                required
            } else {
                NumericOutcome::Symbolic {
                    lower: NumericBound::Symbol(id),
                    upper: NumericBound::Symbol(id),
                }
            }
        } else if ty.instantiation == crate::Instantiation::Unknown {
            NumericOutcome::Unknown("declaration instantiation is unknown".into())
        } else {
            let definitions = self.definitions.unwrap();
            definitions
                .definitions
                .iter()
                .filter(|d| d.target == id && supported_definition(d))
                .map(|d| self.definition(d))
                .find(|value| {
                    matches!(
                        value,
                        NumericOutcome::Exact(_)
                            | NumericOutcome::Interval { .. }
                            | NumericOutcome::Symbolic { .. }
                    )
                })
                .unwrap_or(required)
        };
        self.active.pop();
        self.expression_memo = None;
        self.declarations[id.0] = Some(value.clone());
        value
    }
    fn definition(&mut self, definition: &crate::Definition) -> NumericOutcome {
        use NumericOutcome::*;
        if definition.enforcement != crate::DefinitionEnforcement::Enforced {
            return match &definition.enforcement {
                crate::DefinitionEnforcement::Unsupported(reason) => Unsupported(reason.clone()),
                _ => Unknown("definition is conditional".into()),
            };
        }
        if definition.coverage != crate::DefinitionCoverage::Scalar {
            return Unsupported("numeric interpretation supports scalar definitions only".into());
        }
        match &definition.safety {
            crate::DefinitionSafety::Unsupported(reason) => return Unsupported(reason.clone()),
            crate::DefinitionSafety::Unknown(reason) => return Unknown(reason.clone()),
            crate::DefinitionSafety::Supported => {}
        }
        if definition.cyclic
            && definition
                .dependencies
                .iter()
                .any(|id| self.active.contains(id))
        {
            return Unknown("cyclic definition cannot establish its own value".into());
        }
        let source = &self.context.files[definition.file];
        let range = definition.value.range.start - source.byte_offset
            ..definition.value.range.end - source.byte_offset;
        let Some(node) = expression_node(source.parsed.tree(), &range) else {
            return Unsupported("definition value syntax is unavailable".into());
        };
        self.expression(definition.file, node)
    }
    fn type_operations(&self, id: DeclarationId) -> Result<(), String> {
        let d = &self.bindings.declarations[id.0];
        let Some(node) = crate::callables::find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            return Err("declared domain syntax is unavailable".into());
        };
        let Some(ty) = node
            .child_nodes()
            .find(|n| !matches!(n.kind(), NodeKind::Annotation | NodeKind::ParameterList))
        else {
            return Ok(());
        };
        core_arithmetic(self.context, self.bindings, self.calls, d.file, ty)
    }
    fn domain(&self, domain: &Domain) -> IntegerBoundsOutcome {
        use IntegerBoundsOutcome::*;
        match domain {
            Domain::Named {
                declaration,
                domain,
            } => {
                if let Err(reason) = union_error(domain) {
                    return Unsupported(reason.into());
                }
                let ty = &self.calls.declarations[declaration.0].ty;
                if ty.instantiation == crate::Instantiation::Parameter
                    && matches!(ty.kind, crate::TypeKind::Set(_))
                {
                    return Unknown("numeric domain membership depends on a parameter set".into());
                }
                if let Err(r) = self.type_operations(*declaration) {
                    return Unsupported(r);
                }
                self.domain(domain)
            }
            Domain::Range { lower, upper } => {
                match (invariant_integer(lower), invariant_integer(upper)) {
                    (Err(r), _) | (_, Err(r)) => Unsupported(r),
                    (Ok(Some(lower)), Ok(Some(upper))) if lower <= upper => Known { lower, upper },
                    (Ok(Some(_)), Ok(Some(_))) => Unknown("numeric domain is empty".into()),
                    _ => Unknown("numeric domain endpoints depend on parameters".into()),
                }
            }
            Domain::LiteralSet(values) => {
                let result = values
                    .iter()
                    .map(invariant_integer)
                    .collect::<Result<Option<Vec<_>>, _>>();
                match result {
                    Err(r) => Unsupported(r),
                    Ok(Some(v)) if !v.is_empty() => Known {
                        lower: *v.iter().min().unwrap(),
                        upper: *v.iter().max().unwrap(),
                    },
                    Ok(Some(_)) => Unknown("numeric domain is empty".into()),
                    Ok(None) => Unknown("numeric domain members depend on parameters".into()),
                }
            }
            Domain::Union { .. } => match union_minimum(domain) {
                Err(reason) => Unsupported(reason.into()),
                Ok(_) => Unknown("integer-set union has no proved numeric interval".into()),
            },
            Domain::Unsupported(r) => Unsupported(r.clone()),
            _ => Unknown("no invariant closed integer domain is available".into()),
        }
    }
    fn array_elements(&self, id: DeclarationId) -> IntegerBoundsOutcome {
        let mut domain = &self.domains.declarations[id.0].domain;
        if let Err(r) = self.type_operations(id) {
            return IntegerBoundsOutcome::Unsupported(r);
        }
        while let Domain::Named {
            declaration,
            domain: inner,
        } = domain
        {
            if let Err(r) = self.type_operations(*declaration) {
                return IntegerBoundsOutcome::Unsupported(r);
            }
            domain = inner;
        }
        match domain {
            Domain::Array { element, .. } => self.domain(element),
            _ => IntegerBoundsOutcome::Unsupported("array element domain is unavailable".into()),
        }
    }
}
pub(super) fn core_arithmetic(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &crate::CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Result<(), String> {
    if matches!(
        node.kind(),
        NodeKind::UnaryExpression | NodeKind::BinaryExpression
    ) {
        let name = tokens(&context.files[file].parsed, node)
            .first()
            .and_then(|t| crate::bindings::symbolic_operator(t.kind))
            .ok_or("domain operator is outside bounded arithmetic")?;
        if !crate::callables::core_operation(context, bindings, calls, file, node, name)? {
            return Err("user domain operation has no supported bounds".into());
        }
    }
    for child in node.child_nodes() {
        core_arithmetic(context, bindings, calls, file, child)?;
    }
    Ok(())
}
pub(super) fn invariant_integer(bound: &NumericBound) -> Result<Option<i64>, String> {
    match bound {
        NumericBound::Defined { .. } | NumericBound::Symbol(_) | NumericBound::Unknown => Ok(None),
        NumericBound::Arithmetic { operands, .. }
            if operands
                .iter()
                .any(|operand| invariant_integer(operand).is_ok_and(|n| n.is_none())) =>
        {
            Ok(None)
        }
        _ => integer(bound).map_err(str::to_owned),
    }
}

pub(super) fn bare_index_domain(mut d: &Domain) -> &Domain {
    while let Domain::Named { domain, .. } = d {
        d = domain;
    }
    d
}

pub(super) fn index_domain_interval(d: &Domain) -> Option<(i64, i64)> {
    match bare_index_domain(d) {
        Domain::Range { lower, upper } => Some((
            invariant_integer(lower).ok()??,
            invariant_integer(upper).ok()??,
        )),
        Domain::LiteralSet(v) => {
            let v: Option<Vec<_>> = v
                .iter()
                .map(|n| invariant_integer(n).ok().flatten())
                .collect();
            let v = v?;
            Some((*v.iter().min()?, *v.iter().max()?))
        }
        _ => None,
    }
}

pub(super) fn index_domain_member(d: &Domain, n: i64) -> Option<bool> {
    match bare_index_domain(d) {
        Domain::Range { .. } => index_domain_interval(d).map(|(l, u)| l <= n && n <= u),
        Domain::LiteralSet(v) => {
            let values: Option<Vec<_>> = v
                .iter()
                .map(|n| invariant_integer(n).ok().flatten())
                .collect();
            Some(values?.contains(&n))
        }
        _ => None,
    }
}

pub(super) fn intersect_index_domains(a: &Domain, b: &Domain) -> Option<Domain> {
    if let Domain::LiteralSet(values) = bare_index_domain(a) {
        let mut out = Vec::new();
        for v in values {
            let n = invariant_integer(v).ok()??;
            if index_domain_member(b, n)? {
                out.push(NumericBound::Integer(n));
            }
        }
        return Some(Domain::LiteralSet(out));
    }
    if matches!(bare_index_domain(a), Domain::Range { .. })
        && matches!(bare_index_domain(b), Domain::Range { .. })
    {
        let (l, u) = index_domain_interval(a)?;
        let (x, y) = index_domain_interval(b)?;
        return Some(Domain::Range {
            lower: NumericBound::Integer(l.max(x)),
            upper: NumericBound::Integer(u.min(y)),
        });
    }
    if matches!(bare_index_domain(b), Domain::LiteralSet(_)) {
        return intersect_index_domains(b, a);
    }
    None
}

pub(super) fn shift_index_domain(d: &Domain, delta: Option<i64>) -> Domain {
    let Some(delta) = delta else {
        return Domain::Unsupported("index offset overflow".into());
    };
    let shift = |b: &NumericBound| match invariant_integer(b) {
        Ok(Some(n)) => n
            .checked_add(delta)
            .map(NumericBound::Integer)
            .unwrap_or_else(|| NumericBound::Unsupported("index offset overflow".into())),
        _ => NumericBound::Arithmetic {
            operator: TokenKind::Plus,
            operands: vec![b.clone(), NumericBound::Integer(delta)],
        },
    };
    match bare_index_domain(d) {
        Domain::Range { lower, upper } => Domain::Range {
            lower: shift(lower),
            upper: shift(upper),
        },
        Domain::LiteralSet(values) => Domain::LiteralSet(values.iter().map(shift).collect()),
        _ if delta == 0 => d.clone(),
        _ => Domain::Unsupported("offset of an opaque index set".into()),
    }
}
