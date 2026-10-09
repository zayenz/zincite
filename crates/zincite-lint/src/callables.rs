//! Bounded callable facts over retained lexical identities, independent of lint.
use std::collections::{BTreeMap, HashMap};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

use crate::bindings::symbolic_operator;
use crate::types::{Match, aggregate, coerces, join, match_type, pattern_narrower, substitute};
use crate::{
    BindingFacts, BindingResolution, DeclarationId, DeclarationRole, FileId, Instantiation,
    ModelContext, SourceLocation, TypeInst, TypeKind,
};

#[derive(Clone, Debug)]
pub struct CallableParameter {
    pub name: Option<String>,
    pub ty: TypeInst,
    pub has_default: bool,
}

#[derive(Clone, Debug)]
pub struct CallableSignature {
    pub declaration: DeclarationId,
    pub parameters: Vec<CallableParameter>,
    pub return_type: TypeInst,
}

#[derive(Clone, Debug)]
pub struct DeclarationType {
    pub declaration: DeclarationId,
    pub ty: TypeInst,
}

#[derive(Clone, Debug)]
pub struct ExpressionType {
    pub file: FileId,
    pub location: SourceLocation,
    pub ty: TypeInst,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CallOutcome {
    /// A language operation without a standard-library declaration.
    Intrinsic {
        name: String,
        return_type: TypeInst,
    },
    Resolved {
        declaration: DeclarationId,
        parameters: Vec<TypeInst>,
        return_type: TypeInst,
    },
    NoMatch {
        reason: String,
    },
    Unresolved {
        reason: String,
    },
    Ambiguous {
        candidates: Vec<DeclarationId>,
    },
    Unsupported {
        reason: String,
        candidates: Vec<DeclarationId>,
    },
}

#[derive(Clone, Debug)]
pub struct CallFact {
    /// Symbolic operator heads, distinct from written callable invocations.
    pub symbolic_operator: bool,
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub name: String,
    pub outcome: CallOutcome,
    /// The independently inferred collection actual of a generator invocation.
    /// Its retained node's expression type is the callable result instead.
    pub generator_argument: Option<TypeInst>,
}

#[derive(Clone, Debug, Default)]
pub struct CallableFacts {
    pub declarations: Vec<DeclarationType>,
    pub signatures: Vec<CallableSignature>,
    pub expressions: Vec<ExpressionType>,
    pub calls: Vec<CallFact>,
}

/// Produce facts without selecting a lint rule. The supplied bindings must
/// belong to this context. Supported types retain enum identity, qualifiers,
/// array indices and structured fields. Unknown domains, alias cycles and
/// unsupported operations stay explicit; no parameter data is evaluated.
/// Every inspected user call has an outcome. Matching uses component coercions
/// and a unique lowest signature, never a summed specificity score.
pub fn resolve_callables(context: &ModelContext, bindings: &BindingFacts) -> CallableFacts {
    let mut reference_index = HashMap::with_capacity(bindings.references.len());
    for (row, reference) in bindings.references.iter().enumerate() {
        reference_index
            .entry((reference.file, reference.location.range.start))
            .or_insert(row);
    }
    let mut engine = Engine {
        context,
        bindings,
        reference_index: Some(reference_index),
        nodes: Vec::new(),
        declared: vec![None; bindings.declarations.len()],
        active: Vec::new(),
        signatures: BTreeMap::new(),
        expressions: BTreeMap::new(),
        calls: BTreeMap::new(),
        active_calls: Vec::new(),
    };
    for declaration in &bindings.declarations {
        engine.nodes.push(find_node(
            context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ));
    }
    for declaration in &bindings.declarations {
        engine.declared_type(declaration.id);
    }
    for declaration in &bindings.declarations {
        engine.signature(declaration.id);
    }
    for (file, source) in context.files.iter().enumerate() {
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            if source.warnings_enabled()
                || matches!(
                    node.kind(),
                    NodeKind::Constraint
                        | NodeKind::Solve
                        | NodeKind::SolveMinimize
                        | NodeKind::SolveMaximize
                        | NodeKind::Output
                )
            {
                engine.inspect(file, item, node);
            }
        }
    }
    // Retain interpretation inside selected standard implementations as well.
    // Native declarations are endpoints; this follows only demanded bodies.
    let mut inspected = vec![false; bindings.declarations.len()];
    loop {
        let selected: Vec<_> = engine
            .calls
            .values()
            .filter_map(|call| {
                if let CallOutcome::Resolved { declaration, .. } = call.outcome {
                    let d = &bindings.declarations[declaration.0];
                    (context.files[d.file].kind == crate::SourceKind::StandardLibrary
                        && !inspected[declaration.0])
                        .then_some(declaration)
                } else {
                    None
                }
            })
            .collect();
        if selected.is_empty() {
            break;
        }
        for id in selected {
            if inspected[id.0] {
                continue;
            }
            inspected[id.0] = true;
            let d = &bindings.declarations[id.0];
            if let Some(node) = engine.nodes[id.0]
                && node.child_nodes().any(|n| is_expression(n.kind()))
            {
                engine.inspect(d.file, d.item, node);
            }
        }
    }
    CallableFacts {
        declarations: engine
            .declared
            .into_iter()
            .enumerate()
            .map(|(id, ty)| DeclarationType {
                declaration: DeclarationId(id),
                ty: ty.unwrap(),
            })
            .collect(),
        signatures: engine.signatures.into_values().flatten().collect(),
        expressions: engine
            .expressions
            .into_iter()
            .map(|((file, start, end), ty)| ExpressionType {
                file,
                location: context.files[file].location(start..end),
                ty,
            })
            .collect(),
        calls: engine.calls.into_values().collect(),
    }
}

pub(super) fn find_node<'a>(
    node: &'a SyntaxNode,
    range: &std::ops::Range<usize>,
    role: DeclarationRole,
) -> Option<&'a SyntaxNode> {
    use DeclarationRole as R;
    use NodeKind as N;
    let kind = match role {
        R::Value | R::Local => N::Declaration,
        R::Function => N::FunctionDeclaration,
        R::Predicate => N::PredicateDeclaration,
        R::Test => N::TestDeclaration,
        R::Annotation => N::AnnotationDeclaration,
        R::TypeAlias => N::TypeAlias,
        R::Enum => N::EnumDeclaration,
        R::EnumMember => N::EnumCase,
        R::EnumConstructor => N::EnumConstructor,
        R::Parameter => N::Parameter,
        R::Generator => N::Generator,
        R::Index => N::ArrayIndexBinding,
    };
    if node.range() == *range && node.kind() == kind {
        return Some(node);
    }
    node.child_nodes()
        .find_map(|child| find_node(child, range, role))
}

fn identity(text: &str) -> String {
    text.trim_matches(['\'', '`']).to_owned()
}

struct Engine<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    reference_index: Option<HashMap<(FileId, usize), usize>>,
    nodes: Vec<Option<&'a SyntaxNode>>,
    declared: Vec<Option<TypeInst>>,
    active: Vec<DeclarationId>,
    signatures: BTreeMap<usize, Option<CallableSignature>>,
    expressions: BTreeMap<(FileId, usize, usize), TypeInst>,
    calls: BTreeMap<(FileId, usize), CallFact>,
    active_calls: Vec<(FileId, usize)>,
}

impl<'a> Engine<'a> {
    fn tokens(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> Vec<(TokenKind, String, std::ops::Range<usize>)> {
        let parsed = &self.context.files[file].parsed;
        node.children()
            .iter()
            .filter_map(|child| {
                if let SyntaxElement::Token(index) = child {
                    let token = &parsed.tokens()[*index];
                    (!matches!(
                        token.kind,
                        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                    ))
                    .then(|| {
                        (
                            token.kind,
                            parsed.source()[token.range.clone()].to_owned(),
                            token.range.clone(),
                        )
                    })
                } else {
                    None
                }
            })
            .collect()
    }
    fn name(&self, file: FileId, node: &SyntaxNode) -> Option<String> {
        self.tokens(file, node)
            .into_iter()
            .find(|(kind, _, _)| {
                matches!(
                    kind,
                    TokenKind::Identifier
                        | TokenKind::QuotedIdentifier
                        | TokenKind::InfixIdentifier
                )
            })
            .map(|(_, name, _)| identity(&name))
    }
    fn resolution(&self, file: FileId, node: &SyntaxNode) -> BindingResolution {
        let Some((_, _, range)) = self.tokens(file, node).into_iter().find(|(kind, _, _)| {
            matches!(
                kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
            ) || symbolic_operator(*kind).is_some()
        }) else {
            return BindingResolution::Unresolved;
        };
        let start = range.start + self.context.files[file].byte_offset;
        if let Some(index) = &self.reference_index {
            return index
                .get(&(file, start))
                .map(|row| self.bindings.references[*row].resolution.clone())
                .unwrap_or(BindingResolution::Unresolved);
        }
        self.bindings
            .references
            .iter()
            .find(|r| r.file == file && r.location.range.start == start)
            .map(|r| r.resolution.clone())
            .unwrap_or(BindingResolution::Unresolved)
    }
    fn declared_type(&mut self, id: DeclarationId) -> TypeInst {
        if let Some(ty) = &self.declared[id.0] {
            return ty.clone();
        }
        if self.active.contains(&id) {
            return TypeInst::unknown("cyclic declaration type");
        }
        self.active.push(id);
        let declaration = &self.bindings.declarations[id.0];
        let file = declaration.file;
        let ty = match (declaration.role, self.nodes[id.0]) {
            (DeclarationRole::Enum, _) => {
                TypeInst::par(TypeKind::Set(Box::new(TypeInst::par(TypeKind::Enum(id)))))
            }
            (DeclarationRole::EnumMember, _) => self
                .bindings
                .declarations
                .iter()
                .find(|d| {
                    d.file == file && d.item == declaration.item && d.role == DeclarationRole::Enum
                })
                .map(|d| TypeInst::par(TypeKind::Enum(d.id)))
                .unwrap_or_else(|| TypeInst::unknown("enum owner is unknown")),
            (DeclarationRole::Annotation, _) => TypeInst::par(TypeKind::Annotation),
            (DeclarationRole::Generator, Some(node)) => {
                let source = node
                    .child_nodes()
                    .next()
                    .map(|n| self.infer(file, declaration.item, n))
                    .unwrap_or_else(|| TypeInst::unknown("generator source is missing"));
                if self
                    .tokens(file, node)
                    .iter()
                    .any(|(kind, _, _)| *kind == TokenKind::In)
                {
                    match source.kind {
                        TypeKind::Set(element) | TypeKind::Array { element, .. } => *element,
                        _ => TypeInst::unknown("generator domain is unsupported"),
                    }
                } else {
                    source
                }
            }
            (DeclarationRole::TypeAlias, Some(node)) => node
                .child_nodes()
                .find(|n| n.kind() != NodeKind::Annotation)
                .map(|n| self.type_node(file, declaration.item, n))
                .unwrap_or_else(|| TypeInst::unknown("alias target is missing")),
            (DeclarationRole::Value | DeclarationRole::Local, Some(node))
                if node.child_nodes().next().is_some_and(|ty| {
                    ty.kind() == NodeKind::ScalarType
                        && matches!(self.tokens(file, ty).as_slice(), [(TokenKind::Any, _, _)])
                }) =>
            {
                node.child_nodes()
                    .filter(|n| is_expression(n.kind()))
                    .last()
                    .map(|n| self.infer(file, declaration.item, n))
                    .unwrap_or_else(|| {
                        TypeInst::unknown("inferred declaration initializer is missing")
                    })
            }
            (
                DeclarationRole::Value
                | DeclarationRole::Local
                | DeclarationRole::Parameter
                | DeclarationRole::Function
                | DeclarationRole::Index,
                Some(node),
            ) => node
                .child_nodes()
                .next()
                .map(|n| self.type_node(file, declaration.item, n))
                .unwrap_or_else(|| TypeInst::unknown("declared type is missing")),
            (DeclarationRole::Predicate, _) => {
                TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
            }
            (DeclarationRole::Test, _) => TypeInst::par(TypeKind::Bool),
            _ => TypeInst::unknown("declaration type is unsupported"),
        };
        self.active.pop();
        self.declared[id.0] = Some(ty.clone());
        ty
    }
    fn type_node(&mut self, file: FileId, item: usize, node: &'a SyntaxNode) -> TypeInst {
        use NodeKind::*;
        let tokens = self.tokens(file, node);
        let has = |kind| tokens.iter().any(|(k, _, _)| *k == kind);
        let children: Vec<_> = node.child_nodes().collect();
        let mut ty = match node.kind() {
            ScalarType => TypeInst::par(if has(TokenKind::Bool) {
                TypeKind::Bool
            } else if has(TokenKind::Int) {
                TypeKind::Int
            } else if has(TokenKind::Float) {
                TypeKind::Float
            } else if has(TokenKind::String) {
                TypeKind::String
            } else {
                TypeKind::Annotation
            }),
            TypeInstVariable => TypeInst {
                instantiation: if has(TokenKind::Any) {
                    Instantiation::Unknown
                } else {
                    Instantiation::Parameter
                },
                optional: false,
                kind: TypeKind::Variable {
                    name: tokens
                        .iter()
                        .find(|(kind, _, _)| *kind == TokenKind::TypeInstVariable)
                        .map(|(_, name, _)| name.clone())
                        .unwrap_or_default(),
                    enum_only: tokens.iter().any(|(_, name, _)| name.starts_with("$$")),
                    any: has(TokenKind::Any),
                },
            },
            DomainType => {
                if let Some(expression) = children.first() {
                    match self.resolution(file, expression) {
                        BindingResolution::Resolved(id)
                            if self.bindings.declarations[id.0].role
                                == DeclarationRole::TypeAlias =>
                        {
                            self.declared_type(id)
                        }
                        _ => {
                            let domain = self.infer(file, item, expression);
                            match domain.kind {
                                TypeKind::Set(element)
                                    if domain.instantiation == Instantiation::Parameter
                                        && !domain.optional =>
                                {
                                    *element
                                }
                                _ => TypeInst::unknown("domain is not a supported parameter set"),
                            }
                        }
                    }
                } else {
                    TypeInst::unknown("type domain is missing")
                }
            }
            ArrayIndexBinding | RecordField => children
                .first()
                .map(|n| self.type_node(file, item, n))
                .unwrap_or_else(|| TypeInst::unknown("component type is missing")),
            ArrayType | ListType => {
                let Some(element) = children.last() else {
                    return TypeInst::unknown("array element type is missing");
                };
                let element = self.type_node(file, item, element);
                let indices = if node.kind() == ListType {
                    vec![TypeInst::par(TypeKind::Int)]
                } else {
                    children[..children.len() - 1]
                        .iter()
                        .map(|n| self.type_node(file, item, n))
                        .collect()
                };
                TypeInst {
                    instantiation: element.instantiation,
                    optional: false,
                    kind: TypeKind::Array {
                        indices,
                        element: Box::new(element),
                    },
                }
            }
            SetType => TypeInst::par(TypeKind::Set(Box::new(
                children
                    .last()
                    .map(|n| self.type_node(file, item, n))
                    .unwrap_or_else(|| TypeInst::unknown("set element type is missing")),
            ))),
            TupleType => {
                let fields: Vec<_> = children
                    .iter()
                    .map(|n| self.type_node(file, item, n))
                    .collect();
                TypeInst {
                    instantiation: aggregate(&fields),
                    optional: false,
                    kind: TypeKind::Tuple(fields),
                }
            }
            RecordType => {
                let mut fields: Vec<_> = children
                    .iter()
                    .map(|n| {
                        (
                            self.name(file, n).unwrap_or_default(),
                            self.type_node(file, item, n),
                        )
                    })
                    .collect();
                fields.sort_by(|(a, _), (b, _)| a.cmp(b));
                TypeInst {
                    instantiation: aggregate(
                        &fields.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(),
                    ),
                    optional: false,
                    kind: TypeKind::Record(fields),
                }
            }
            _ => TypeInst::unknown("type form is unsupported"),
        };
        if has(TokenKind::Var) {
            ty = ty.with_inst(Instantiation::Decision);
        } else if has(TokenKind::Par) {
            ty = ty.with_inst(Instantiation::Parameter);
        }
        if has(TokenKind::Opt) {
            ty.optional = true;
        }
        ty
    }
    fn signature(&mut self, id: DeclarationId) -> Option<CallableSignature> {
        if let Some(signature) = self.signatures.get(&id.0) {
            return signature.clone();
        }
        let declaration = &self.bindings.declarations[id.0];
        if !matches!(
            declaration.role,
            DeclarationRole::Function
                | DeclarationRole::Predicate
                | DeclarationRole::Test
                | DeclarationRole::Annotation
        ) {
            self.signatures.insert(id.0, None);
            return None;
        }
        let node = self.nodes[id.0]?;
        let file = declaration.file;
        let item = declaration.item;
        let parameters = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::ParameterList)
            .map(|list| {
                list.child_nodes()
                    .map(|parameter| CallableParameter {
                        name: self.name(file, parameter),
                        ty: parameter
                            .child_nodes()
                            .next()
                            .map(|n| self.type_node(file, item, n))
                            .unwrap_or_else(|| TypeInst::unknown("parameter type is missing")),
                        has_default: self
                            .tokens(file, parameter)
                            .iter()
                            .any(|(kind, _, _)| *kind == TokenKind::Equal),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let signature = CallableSignature {
            declaration: id,
            parameters,
            return_type: self.declared_type(id),
        };
        self.signatures.insert(id.0, Some(signature.clone()));
        Some(signature)
    }
    fn inspect(&mut self, file: FileId, item: usize, node: &'a SyntaxNode) {
        // Full-axis selectors get their type from the owning array axis before
        // ordinary child inference sees them as standalone range operations.
        if node.kind() == NodeKind::ArrayAccessExpression {
            self.infer(file, item, node);
        }
        if matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) || self
            .tokens(file, node)
            .iter()
            .any(|(kind, _, _)| *kind == TokenKind::InfixIdentifier)
        {
            self.call(file, item, node);
        }
        for child in node.child_nodes() {
            self.inspect(file, item, child);
        }
        if is_expression(node.kind()) {
            self.infer(file, item, node);
        }
    }
    fn infer(&mut self, file: FileId, item: usize, node: &'a SyntaxNode) -> TypeInst {
        let range = node.range();
        let key = (file, range.start, range.end);
        if let Some(ty) = self.expressions.get(&key) {
            return ty.clone();
        }
        use NodeKind::*;
        let children: Vec<_> = node.child_nodes().collect();
        let tokens = self.tokens(file, node);
        let ty = match node.kind() {
            Expression => {
                let kind = tokens.first().map(|(kind, _, _)| *kind);
                match kind {
                    Some(TokenKind::True | TokenKind::False) => TypeInst::par(TypeKind::Bool),
                    Some(TokenKind::IntegerLiteral | TokenKind::Infinity) => {
                        TypeInst::par(TypeKind::Int)
                    }
                    Some(TokenKind::FloatLiteral) => TypeInst::par(TypeKind::Float),
                    Some(TokenKind::StringLiteral) => TypeInst::par(TypeKind::String),
                    Some(TokenKind::Absent) => TypeInst {
                        optional: true,
                        ..TypeInst::par(TypeKind::Bottom)
                    },
                    Some(TokenKind::Identifier | TokenKind::QuotedIdentifier) => {
                        match self.resolution(file, node) {
                            BindingResolution::Resolved(id) => self.declared_type(id),
                            _ => TypeInst::unknown("value reference is unresolved or ambiguous"),
                        }
                    }
                    _ => TypeInst::unknown("atom type is unsupported"),
                }
            }
            ParenthesizedExpression
            | AnnotatedExpression
            | NamedArgument
            | RecordLiteralField
            | WhereFilter => children
                .first()
                .map(|n| self.infer(file, item, n))
                .unwrap_or_else(|| TypeInst::unknown("expression is missing")),
            InterpolatedString => TypeInst::par(TypeKind::String),
            CallExpression | GeneratorCallExpression => match self.call(file, item, node) {
                CallOutcome::Resolved { return_type, .. } => return_type,
                _ => TypeInst::unknown("call return type is not resolved"),
            },
            ArrayAccessExpression => {
                let subject = children
                    .first()
                    .map(|n| self.infer(file, item, n))
                    .unwrap_or_else(|| TypeInst::unknown("array subject is missing"));
                match subject.kind {
                    TypeKind::Array { indices, element } if indices.len() == children.len() - 1 => {
                        let mut retained = Vec::new();
                        let mut decision_index = false;
                        let mut supported = true;
                        for (selector, index) in children[1..].iter().zip(&indices) {
                            if selector.kind() == NodeKind::RangeExpression
                                && selector.child_nodes().next().is_none()
                                && matches!(
                                    self.tokens(file, selector).as_slice(),
                                    [(TokenKind::RangeInclusive, _, _)]
                                )
                            {
                                let range = selector.range();
                                self.expressions.insert(
                                    (file, range.start, range.end),
                                    TypeInst::par(TypeKind::Set(Box::new(index.clone()))),
                                );
                                retained.push(index.clone());
                                continue;
                            }
                            let actual = self.infer(file, item, selector);
                            supported &= !actual.optional
                                && coerces(
                                    &actual.clone().with_inst(Instantiation::Parameter),
                                    index,
                                );
                            decision_index |= actual.instantiation == Instantiation::Decision;
                        }
                        if !supported || !retained.is_empty() && decision_index {
                            TypeInst::unknown("array index or slicing type is unsupported")
                        } else if !retained.is_empty() {
                            // A full-axis slice retains each selected index type.
                            // MiniZinc forbids decision selectors when any axis is sliced.
                            TypeInst {
                                instantiation: subject.instantiation,
                                optional: subject.optional,
                                kind: TypeKind::Array {
                                    indices: retained,
                                    element,
                                },
                            }
                        } else if decision_index {
                            element.with_inst(Instantiation::Decision)
                        } else {
                            *element
                        }
                    }
                    TypeKind::Set(element)
                        if subject.instantiation == Instantiation::Parameter
                            && !subject.optional
                            && element.instantiation == Instantiation::Parameter
                            && !element.optional
                            && element.kind == TypeKind::Int
                            && children.len() == 2 =>
                    {
                        let index = self.infer(file, item, children[1]);
                        if index.instantiation == Instantiation::Parameter
                            && !index.optional
                            && index.kind == TypeKind::Int
                        {
                            // MiniZinc coerces a present parameter set to a rank-one
                            // array for ordinal selection. This establishes type only.
                            TypeInst::par(TypeKind::Int)
                        } else {
                            TypeInst::unknown("set ordinal index type is unsupported")
                        }
                    }
                    _ => TypeInst::unknown("array access type is unsupported"),
                }
            }
            FieldAccessExpression => {
                let subject = children
                    .first()
                    .map(|n| self.infer(file, item, n))
                    .unwrap_or_else(|| TypeInst::unknown("field subject is missing"));
                let field = tokens
                    .iter()
                    .find(|(kind, _, _)| {
                        matches!(
                            kind,
                            TokenKind::IntegerLiteral
                                | TokenKind::Identifier
                                | TokenKind::QuotedIdentifier
                        )
                    })
                    .map(|(_, name, _)| identity(name))
                    .unwrap_or_default();
                match subject.kind {
                    TypeKind::Tuple(fields) => field
                        .parse::<usize>()
                        .ok()
                        .and_then(|i| i.checked_sub(1))
                        .and_then(|i| fields.get(i).cloned())
                        .unwrap_or_else(|| TypeInst::unknown("tuple field is unknown")),
                    TypeKind::Record(fields) => fields
                        .into_iter()
                        .find(|(n, _)| *n == field)
                        .map(|(_, t)| t)
                        .unwrap_or_else(|| TypeInst::unknown("record field is unknown")),
                    _ => TypeInst::unknown("field access type is unsupported"),
                }
            }
            TupleLiteral => {
                let fields: Vec<_> = children.iter().map(|n| self.infer(file, item, n)).collect();
                TypeInst {
                    instantiation: aggregate(&fields),
                    optional: false,
                    kind: TypeKind::Tuple(fields),
                }
            }
            RecordLiteral => {
                let mut fields: Vec<_> = children
                    .iter()
                    .map(|n| {
                        (
                            self.name(file, n).unwrap_or_default(),
                            self.infer(file, item, n),
                        )
                    })
                    .collect();
                fields.sort_by(|(a, _), (b, _)| a.cmp(b));
                TypeInst {
                    instantiation: aggregate(
                        &fields.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>(),
                    ),
                    optional: false,
                    kind: TypeKind::Record(fields),
                }
            }
            RangeExpression | SetLiteral => {
                if node.kind() == RangeExpression
                    && let CallOutcome::Resolved { return_type, .. } = self.call(file, item, node)
                {
                    self.expressions.insert(key, return_type.clone());
                    return return_type;
                }
                let element = self.common(file, item, &children);
                TypeInst {
                    instantiation: element.instantiation,
                    optional: false,
                    kind: TypeKind::Set(Box::new(element.with_inst(Instantiation::Parameter))),
                }
            }
            ArrayLiteral => {
                let indexed = children.iter().any(|n| n.kind() == IndexedArrayEntry);
                let mut indices = vec![TypeInst::par(TypeKind::Int)];
                let values: Vec<_> = if indexed {
                    let mut keys = Vec::new();
                    let mut values = Vec::new();
                    for entry in &children {
                        let parts: Vec<_> = entry.child_nodes().collect();
                        if parts.len() != 2 {
                            return TypeInst::unknown("mixed array entries are unsupported");
                        }
                        keys.push(parts[0]);
                        values.push(parts[1]);
                    }
                    let key = self.common(file, item, &keys);
                    indices = match key.kind {
                        TypeKind::Tuple(fields) => fields,
                        TypeKind::Unknown(_) => vec![key],
                        _ => vec![key],
                    };
                    values
                } else {
                    children.clone()
                };
                let element = self.common(file, item, &values);
                TypeInst {
                    instantiation: element.instantiation,
                    optional: false,
                    kind: TypeKind::Array {
                        indices,
                        element: Box::new(element),
                    },
                }
            }
            MatrixLiteral => {
                let rows: Vec<_> = children
                    .iter()
                    .filter(|n| n.kind() == MatrixRow)
                    .copied()
                    .collect();
                let mut cells = Vec::new();
                let mut row_indices = Vec::new();
                for row in rows {
                    let parts: Vec<_> = row.child_nodes().collect();
                    if self
                        .tokens(file, row)
                        .iter()
                        .any(|(kind, _, _)| *kind == TokenKind::Colon)
                    {
                        if let Some(key) = parts.first() {
                            row_indices.push(*key);
                        }
                        cells.extend(parts.into_iter().skip(1));
                    } else {
                        cells.extend(parts);
                    }
                }
                let row = if row_indices.is_empty() {
                    TypeInst::par(TypeKind::Int)
                } else {
                    self.common(file, item, &row_indices)
                };
                let column = if let Some(header) =
                    children.iter().find(|n| n.kind() == MatrixColumnIndices)
                {
                    self.common(file, item, &header.child_nodes().collect::<Vec<_>>())
                } else {
                    TypeInst::par(TypeKind::Int)
                };
                let element = self.common(file, item, &cells);
                TypeInst {
                    instantiation: element.instantiation,
                    optional: false,
                    kind: TypeKind::Array {
                        indices: vec![row, column],
                        element: Box::new(element),
                    },
                }
            }
            SetComprehension | ArrayComprehension | IndexedArrayComprehension => {
                self.comprehension(file, item, node, false)
            }
            UnaryExpression => {
                if tokens
                    .first()
                    .is_some_and(|(kind, _, _)| symbolic_operator(*kind).is_some())
                    && let CallOutcome::Resolved { return_type, .. } = self.call(file, item, node)
                {
                    self.expressions.insert(key, return_type.clone());
                    return return_type;
                }
                let mut value = children
                    .first()
                    .map(|n| self.infer(file, item, n))
                    .unwrap_or_else(|| TypeInst::unknown("operand is missing"));
                let operator = tokens.first().map(|(kind, _, _)| *kind);
                if !value.known() || value.optional {
                    TypeInst::unknown("unary operand type is unsupported")
                } else if operator == Some(TokenKind::Not) && value.kind == TypeKind::Bool {
                    value
                } else if matches!(operator, Some(TokenKind::Plus | TokenKind::Minus))
                    && matches!(
                        value.kind,
                        TypeKind::Bool | TypeKind::Int | TypeKind::Enum(_) | TypeKind::Float
                    )
                {
                    if matches!(value.kind, TypeKind::Bool | TypeKind::Enum(_)) {
                        value.kind = TypeKind::Int;
                    }
                    value
                } else {
                    TypeInst::unknown("unary operand is unsupported")
                }
            }
            BinaryExpression
                if tokens
                    .iter()
                    .any(|(kind, _, _)| *kind == TokenKind::InfixIdentifier) =>
            {
                match self.call(file, item, node) {
                    CallOutcome::Resolved { return_type, .. } => return_type,
                    _ => TypeInst::unknown("infix call is not resolved"),
                }
            }
            BinaryExpression => {
                if tokens
                    .first()
                    .is_some_and(|(kind, _, _)| symbolic_operator(*kind).is_some())
                    && let CallOutcome::Resolved { return_type, .. } = self.call(file, item, node)
                {
                    self.expressions.insert(key, return_type.clone());
                    return return_type;
                }
                let values: Vec<_> = children.iter().map(|n| self.infer(file, item, n)).collect();
                let operator = tokens
                    .iter()
                    .find(|(kind, _, _)| {
                        !matches!(kind, TokenKind::LeftParen | TokenKind::RightParen)
                    })
                    .map(|(kind, _, _)| *kind);
                if values.iter().any(|t| !t.known() || t.optional) {
                    TypeInst::unknown("operator operand is unknown")
                } else if matches!(
                    operator,
                    Some(
                        TokenKind::Equal
                            | TokenKind::DoubleEqual
                            | TokenKind::NotEqual
                            | TokenKind::Less
                            | TokenKind::LessEqual
                            | TokenKind::Greater
                            | TokenKind::GreaterEqual
                            | TokenKind::In
                            | TokenKind::Subset
                            | TokenKind::Superset
                            | TokenKind::And
                            | TokenKind::Or
                            | TokenKind::Xor
                            | TokenKind::Implies
                            | TokenKind::ReverseImplies
                            | TokenKind::Equivalence
                    )
                ) {
                    let supported = match operator {
                        Some(
                            TokenKind::And
                            | TokenKind::Or
                            | TokenKind::Xor
                            | TokenKind::Implies
                            | TokenKind::ReverseImplies
                            | TokenKind::Equivalence,
                        ) => values.iter().all(|t| t.kind == TypeKind::Bool),
                        Some(TokenKind::In) => {
                            matches!(&values[1].kind,TypeKind::Set(element) if coerces(&values[0].clone().with_inst(Instantiation::Parameter),element))
                        }
                        Some(TokenKind::Subset | TokenKind::Superset) => {
                            values.iter().all(|t| matches!(t.kind, TypeKind::Set(_)))
                                && join(&values[0], &values[1]).known()
                        }
                        Some(
                            TokenKind::Less
                            | TokenKind::LessEqual
                            | TokenKind::Greater
                            | TokenKind::GreaterEqual,
                        ) => {
                            values.iter().all(|t| {
                                matches!(
                                    t.kind,
                                    TypeKind::Bool
                                        | TypeKind::Int
                                        | TypeKind::Float
                                        | TypeKind::Enum(_)
                                        | TypeKind::String
                                )
                            }) && join(&values[0], &values[1]).known()
                        }
                        _ => join(&values[0], &values[1]).known(),
                    };
                    if supported {
                        TypeInst::par(TypeKind::Bool).with_inst(aggregate(&values))
                    } else {
                        TypeInst::unknown("operator operand types are unsupported")
                    }
                } else if matches!(
                    operator,
                    Some(
                        TokenKind::Plus
                            | TokenKind::Minus
                            | TokenKind::Star
                            | TokenKind::Div
                            | TokenKind::Mod
                            | TokenKind::Power
                            | TokenKind::Slash
                    )
                ) && values.iter().all(|t| {
                    matches!(
                        t.kind,
                        TypeKind::Bool | TypeKind::Int | TypeKind::Float | TypeKind::Enum(_)
                    )
                }) {
                    if matches!(operator, Some(TokenKind::Div | TokenKind::Mod))
                        && values.iter().any(|t| t.kind == TypeKind::Float)
                    {
                        return TypeInst::unknown("integer operator has a float operand");
                    }
                    let mut ty = values
                        .into_iter()
                        .map(|mut t| {
                            if matches!(t.kind, TypeKind::Bool | TypeKind::Enum(_)) {
                                t.kind = TypeKind::Int;
                            }
                            t
                        })
                        .reduce(|a, b| join(&a, &b))
                        .unwrap();
                    if operator == Some(TokenKind::Slash) {
                        ty.kind = TypeKind::Float;
                    } else if matches!(ty.kind, TypeKind::Bool | TypeKind::Enum(_)) {
                        ty.kind = TypeKind::Int;
                    }
                    ty
                } else if matches!(
                    operator,
                    Some(
                        TokenKind::Union
                            | TokenKind::Intersect
                            | TokenKind::Diff
                            | TokenKind::SymDiff
                    )
                ) {
                    if values.iter().all(|t| matches!(t.kind, TypeKind::Set(_))) {
                        values.into_iter().reduce(|a, b| join(&a, &b)).unwrap()
                    } else {
                        TypeInst::unknown("set operator has a non-set operand")
                    }
                } else if operator == Some(TokenKind::Concat)
                    && values.iter().all(|t| t.kind == TypeKind::String)
                {
                    TypeInst::par(TypeKind::String)
                } else {
                    TypeInst::unknown("operator type is unsupported")
                }
            }
            LetExpression => children
                .last()
                .map(|n| self.infer(file, item, n))
                .unwrap_or_else(|| TypeInst::unknown("let body is missing")),
            ConditionalExpression => {
                let branches: Vec<_> = children
                    .iter()
                    .filter_map(|branch| branch.child_nodes().last())
                    .collect();
                let mut ty = self.common(file, item, &branches);
                for branch in children.iter().filter(|n| n.kind() == ConditionalBranch) {
                    if let Some(condition) = branch.child_nodes().next() {
                        let condition = self.infer(file, item, condition);
                        if !condition.known() {
                            ty = TypeInst::unknown("conditional guard type is unknown");
                            break;
                        }
                        if condition.instantiation == Instantiation::Decision {
                            ty = ty.with_inst(Instantiation::Decision);
                        }
                    }
                }
                ty
            }
            IndexTuple => {
                let fields: Vec<_> = children.iter().map(|n| self.infer(file, item, n)).collect();
                TypeInst {
                    instantiation: aggregate(&fields),
                    optional: false,
                    kind: TypeKind::Tuple(fields),
                }
            }
            _ => TypeInst::unknown("expression form is unsupported"),
        };
        self.expressions.insert(key, ty.clone());
        ty
    }
    fn common(&mut self, file: FileId, item: usize, nodes: &[&'a SyntaxNode]) -> TypeInst {
        nodes
            .iter()
            .map(|n| self.infer(file, item, n))
            .reduce(|a, b| join(&a, &b))
            .unwrap_or_else(|| TypeInst::par(TypeKind::Bottom))
    }
    fn comprehension(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        generator_call: bool,
    ) -> TypeInst {
        let Some(head) = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
        else {
            return TypeInst::unknown("comprehension head is missing");
        };
        let mut element = if head.kind() == NodeKind::IndexedArrayEntry {
            head.child_nodes()
                .last()
                .map(|n| self.infer(file, item, n))
                .unwrap_or_else(|| TypeInst::unknown("indexed head is missing"))
        } else {
            self.infer(file, item, head)
        };
        for list in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::GeneratorList)
        {
            for generator in list.child_nodes() {
                for source in generator.child_nodes() {
                    let ty = self.infer(file, item, source);
                    if !ty.known() {
                        return TypeInst::unknown("generator domain or filter type is unknown");
                    }
                    if ty.instantiation == Instantiation::Decision {
                        // Only decision-set membership or a decision filter
                        // gates presence. Nonoptional var-array iteration just
                        // changes instantiation, not optionality.
                        if source.kind() == NodeKind::WhereFilter
                            || matches!(ty.kind, TypeKind::Set(_))
                                && self
                                    .tokens(file, generator)
                                    .iter()
                                    .any(|(kind, _, _)| *kind == TokenKind::In)
                        {
                            element.optional = true;
                        }
                        element = element.with_inst(Instantiation::Decision);
                    }
                }
            }
        }
        if node.kind() == NodeKind::SetComprehension && !generator_call {
            return TypeInst {
                instantiation: element.instantiation,
                optional: false,
                kind: TypeKind::Set(Box::new(element.with_inst(Instantiation::Parameter))),
            };
        }
        let indices = if head.kind() == NodeKind::IndexedArrayEntry {
            let key = self.infer(file, item, head.child_nodes().next().unwrap());
            match key.kind {
                TypeKind::Tuple(fields) => fields,
                _ => vec![key],
            }
        } else {
            vec![TypeInst::par(TypeKind::Int)]
        };
        TypeInst {
            instantiation: element.instantiation,
            optional: false,
            kind: TypeKind::Array {
                indices,
                element: Box::new(element),
            },
        }
    }
    fn call(&mut self, file: FileId, item: usize, node: &'a SyntaxNode) -> CallOutcome {
        let tokens = self.tokens(file, node);
        let Some((_, written, range)) = tokens.iter().find(|(kind, _, _)| {
            matches!(
                kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
            ) || symbolic_operator(*kind).is_some()
        }) else {
            return CallOutcome::Unsupported {
                reason: "call head is unsupported".into(),
                candidates: Vec::new(),
            };
        };
        let key = (file, range.start);
        if let Some(fact) = self.calls.get(&key) {
            return fact.outcome.clone();
        }
        if self.active_calls.contains(&key) {
            return CallOutcome::Unsupported {
                reason: "recursive call typing is unsupported".into(),
                candidates: Vec::new(),
            };
        }
        if node.kind() == NodeKind::UnaryExpression
            && tokens
                .first()
                .is_some_and(|(kind, _, _)| *kind == TokenKind::Plus)
        {
            let mut ty = node
                .child_nodes()
                .next()
                .map(|operand| self.infer(file, item, operand))
                .unwrap_or_else(|| TypeInst::unknown("operand is missing"));
            let outcome = if ty.optional
                || !matches!(
                    ty.kind,
                    TypeKind::Bool | TypeKind::Int | TypeKind::Enum(_) | TypeKind::Float
                ) {
                CallOutcome::Unsupported {
                    reason: "intrinsic unary plus operand is unsupported".into(),
                    candidates: Vec::new(),
                }
            } else {
                if matches!(ty.kind, TypeKind::Bool | TypeKind::Enum(_)) {
                    ty.kind = TypeKind::Int;
                }
                CallOutcome::Intrinsic {
                    name: "unary+".into(),
                    return_type: ty,
                }
            };
            self.calls.insert(
                key,
                CallFact {
                    symbolic_operator: true,
                    file,
                    item,
                    location: self.context.files[file].location(range.clone()),
                    name: "unary+".into(),
                    outcome: outcome.clone(),
                    generator_argument: None,
                },
            );
            return outcome;
        }
        self.active_calls.push(key);
        let resolution = self.resolution(file, node);
        let mut arguments = Vec::new();
        let generator_argument = if node.kind() == NodeKind::GeneratorCallExpression {
            let ty = self.comprehension(file, item, node, true);
            arguments.push((None, ty.clone()));
            Some(ty)
        } else {
            for argument in node.child_nodes() {
                arguments.push((
                    if argument.kind() == NodeKind::NamedArgument {
                        self.name(file, argument)
                    } else {
                        None
                    },
                    self.infer(file, item, argument),
                ));
            }
            None
        };
        let outcome = match resolution {
            BindingResolution::Unresolved => CallOutcome::Unresolved {
                reason: "callable name is unresolved".into(),
            },
            BindingResolution::Ambiguous(candidates) => CallOutcome::Ambiguous { candidates },
            BindingResolution::Resolved(id) => self.choose(&[id], &arguments),
            BindingResolution::Overloads(ids) => self.choose(&ids, &arguments),
        };
        self.active_calls.pop();
        self.calls.insert(
            key,
            CallFact {
                symbolic_operator: tokens
                    .iter()
                    .find(|(_, _, r)| r == range)
                    .is_some_and(|(kind, _, _)| symbolic_operator(*kind).is_some()),
                file,
                item,
                location: self.context.files[file].location(range.clone()),
                name: symbolic_operator(tokens.iter().find(|(_, _, r)| r == range).unwrap().0)
                    .map(str::to_owned)
                    .unwrap_or_else(|| identity(written)),
                outcome: outcome.clone(),
                generator_argument,
            },
        );
        outcome
    }
    fn choose(
        &mut self,
        ids: &[DeclarationId],
        arguments: &[(Option<String>, TypeInst)],
    ) -> CallOutcome {
        let mut matches = Vec::new();
        let mut unknown = Vec::new();
        for &id in ids {
            let Some(signature) = self.signature(id) else {
                if self.bindings.declarations[id.0].role == DeclarationRole::EnumConstructor {
                    unknown.push(id);
                }
                continue;
            };
            let mut assigned = vec![None; signature.parameters.len()];
            let mut next = 0;
            let mut valid = true;
            let mut named = false;
            for (name, ty) in arguments {
                let position = if let Some(name) = name {
                    named = true;
                    signature
                        .parameters
                        .iter()
                        .position(|p| p.name.as_ref() == Some(name))
                } else if named {
                    None
                } else {
                    let position = next;
                    next += 1;
                    Some(position)
                };
                if let Some(position) =
                    position.filter(|&p| p < assigned.len() && assigned[p].is_none())
                {
                    assigned[position] = Some(ty);
                } else {
                    valid = false;
                    break;
                }
            }
            if !valid
                || assigned
                    .iter()
                    .zip(&signature.parameters)
                    .any(|(a, p)| a.is_none() && !p.has_default)
            {
                continue;
            }
            // Omitted defaults use their retained enclosing-scope references.
            // They can constrain type-inst variables just like supplied values.
            let defaults: Vec<_> = self.nodes[id.0]
                .unwrap()
                .child_nodes()
                .find(|node| node.kind() == NodeKind::ParameterList)
                .map(|list| {
                    list.child_nodes()
                        .enumerate()
                        .map(|(position, parameter)| {
                            if assigned[position].is_none() {
                                parameter.child_nodes().last().map(|expression| {
                                    self.infer(
                                        self.bindings.declarations[id.0].file,
                                        self.bindings.declarations[id.0].item,
                                        expression,
                                    )
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                })
                .unwrap_or_default();
            for (actual, default) in assigned.iter_mut().zip(&defaults) {
                if actual.is_none() {
                    *actual = default.as_ref();
                }
            }
            // Pinned standard searches, array1d and integer sum coerce multidimensional
            // arrays to row-major one-dimensional inputs. Keep this limited
            // to retained standard identities.
            let d = &self.bindings.declarations[id.0];
            let standard = self.context.files[d.file].kind == crate::SourceKind::StandardLibrary
                && self.context.files[d.file].implicit;
            let standard_view = standard
                && matches!(
                    d.name.as_str(),
                    "array1d" | "int_search" | "bool_search" | "float_search" | "set_search"
                );
            let coerced: Vec<_> = assigned
                .iter()
                .zip(&signature.parameters)
                .map(|(actual, formal)| {
                    actual.map(|actual| {
                        let integer_sum_view = standard
                            && d.name == "sum"
                            && signature.parameters.len() == 1
                            && signature.return_type.kind == TypeKind::Int
                            && !crate::value_safety::optional(&signature.return_type)
                            && actual.known()
                            && !crate::value_safety::optional(actual)
                            && matches!(&actual.kind, TypeKind::Array { indices, element }
                                if indices.len() == 2 && element.kind == TypeKind::Int)
                            && matches!(&formal.ty.kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && element.known() && element.kind == TypeKind::Int
                                    && !crate::value_safety::optional(&formal.ty));
                        let mut actual = actual.clone();
                        // MiniZinc inserts set2array for present parameter Int/Enum
                        // sets supplied to rank-one array formals. This matching
                        // view preserves element identity and proves no value safety.
                        if actual.instantiation == Instantiation::Parameter
                            && !crate::value_safety::optional(&actual)
                            && let (
                                TypeKind::Set(element),
                                TypeKind::Array { indices, .. },
                            ) = (&actual.kind, &formal.ty.kind)
                            && indices.len() == 1
                            && element.instantiation == Instantiation::Parameter
                            && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                        {
                            actual.kind = TypeKind::Array {
                                indices: vec![TypeInst::par(TypeKind::Int)],
                                element: element.clone(),
                            };
                        }
                        if (standard_view || integer_sum_view)
                            && let (
                                TypeKind::Array { indices, .. },
                                TypeKind::Array {
                                    indices: target, ..
                                },
                            ) = (&mut actual.kind, &formal.ty.kind)
                            && indices.len() > 1
                            && target.len() == 1
                        {
                            *indices = vec![TypeInst::par(TypeKind::Int)];
                        }
                        actual
                    })
                })
                .collect();
            let assigned: Vec<_> = coerced.iter().map(Option::as_ref).collect();
            let mut variables = BTreeMap::new();
            let mut state = Match::Yes;
            for (actual, formal) in assigned.iter().zip(&signature.parameters) {
                if let Some(actual) = actual {
                    let result = match_type(actual, &formal.ty, &mut variables);
                    if result == Match::No {
                        state = Match::No;
                        break;
                    }
                    if result == Match::Unknown {
                        state = Match::Unknown;
                    }
                }
            }
            if state == Match::No {
                continue;
            }
            let targets: Vec<_> = signature
                .parameters
                .iter()
                .zip(&assigned)
                .map(|(p, actual)| {
                    let mut target = substitute(&p.ty, &variables);
                    // Presence tests return bool independently of the absent
                    // scalar's unconstrained $T. Keep Bottom instead of making
                    // that core call unresolved; do not invent a concrete type.
                    if matches!(d.name.as_str(), "occurs" | "absent")
                        && matches!(d.role, DeclarationRole::Function | DeclarationRole::Test)
                        && self.context.files[d.file].kind == crate::SourceKind::StandardLibrary
                        && self.context.files[d.file].implicit
                        && matches!(p.ty.kind, TypeKind::Variable { .. })
                        && matches!(target.kind, TypeKind::Unknown(_))
                        && actual.is_some_and(|t| t.kind == TypeKind::Bottom)
                    {
                        target = p.ty.clone();
                        target.kind = TypeKind::Bottom;
                    }
                    // A scalar any occurrence accepts the actual qualifiers;
                    // the shared variable still determines its common type
                    // and the callable's return type.
                    if matches!(p.ty.kind, TypeKind::Variable { any: true, .. })
                        && !matches!(
                            target.kind,
                            TypeKind::Array { .. } | TypeKind::Tuple(_) | TypeKind::Record(_)
                        )
                        && let Some(actual) = actual
                    {
                        target.instantiation = actual.instantiation;
                        target.optional = actual.optional;
                    }
                    target
                })
                .collect();
            for (actual, target) in assigned.iter().zip(&targets) {
                if let Some(actual) = actual {
                    if !target.known() || !actual.known() {
                        state = Match::Unknown;
                    } else if !coerces(actual, target) {
                        state = Match::No;
                        break;
                    }
                }
            }
            if state == Match::No {
                continue;
            }
            if state == Match::Unknown {
                unknown.push(id);
                continue;
            }
            // Rank only supplied arguments; defaults remain in the complete targets.
            let actual_targets: Vec<_> = arguments
                .iter()
                .enumerate()
                .map(|(position, (name, _))| {
                    targets[if let Some(name) = name {
                        signature
                            .parameters
                            .iter()
                            .position(|p| p.name.as_ref() == Some(name))
                            .unwrap()
                    } else {
                        position
                    }]
                    .clone()
                })
                .collect();
            let patterns: Vec<_> = arguments
                .iter()
                .enumerate()
                .map(|(position, (name, _))| {
                    signature.parameters[if let Some(name) = name {
                        signature
                            .parameters
                            .iter()
                            .position(|p| p.name.as_ref() == Some(name))
                            .unwrap()
                    } else {
                        position
                    }]
                    .ty
                    .clone()
                })
                .collect();
            let return_type = substitute(&signature.return_type, &variables);
            matches.push((id, actual_targets, patterns, return_type, targets));
        }
        if !unknown.is_empty() {
            unknown.extend(matches.iter().map(|(id, _, _, _, _)| *id));
            return CallOutcome::Unsupported {
                reason: "a potentially matching signature or argument type is unknown".into(),
                candidates: unknown,
            };
        }
        if matches.is_empty() {
            return CallOutcome::NoMatch {
                reason: "no declaration accepts the supplied arity, names and supported types"
                    .into(),
            };
        }
        // A prototype and its unique implementation denote one operation.
        // Defaults remain separate: equal presence does not establish equal
        // default expressions or their lexical bindings.
        let prototypes: Vec<_> = matches
            .iter()
            .filter_map(|candidate| {
                let declaration = &self.bindings.declarations[candidate.0.0];
                let signature = self.signatures.get(&candidate.0.0)?.as_ref()?;
                let has_body = |id: DeclarationId| {
                    self.nodes[id.0]
                        .is_some_and(|n| n.child_nodes().any(|c| is_expression(c.kind())))
                };
                if !declaration.top_level
                    || has_body(candidate.0)
                    || signature.parameters.iter().any(|p| p.has_default)
                {
                    return None;
                }
                let implementations = matches
                    .iter()
                    .filter(|other| {
                        let d = &self.bindings.declarations[other.0.0];
                        let Some(s) = self.signatures.get(&other.0.0).and_then(Option::as_ref)
                        else {
                            return false;
                        };
                        d.top_level
                            && d.name == declaration.name
                            && d.role == declaration.role
                            && self.context.files[d.file].kind
                                == self.context.files[declaration.file].kind
                            && has_body(other.0)
                            && s.return_type == signature.return_type
                            && s.parameters.len() == signature.parameters.len()
                            && s.parameters
                                .iter()
                                .zip(&signature.parameters)
                                .all(|(a, b)| !a.has_default && a.name == b.name && a.ty == b.ty)
                    })
                    .count();
                (implementations == 1).then_some(candidate.0)
            })
            .collect();
        matches.retain(|candidate| !prototypes.contains(&candidate.0));
        // Written patterns break ties between equivalent instantiated inputs.
        // A bounded par/nonoptional input precedes its var/optional lift, but
        // scalar `any` accepts actual qualifiers without declaring them narrower.
        // A supplied parameter set prefers a direct set formal over a
        // set2array view, including a var set lift. Other input ordering stays
        // componentwise; no preference is added for unknown arguments.
        let set_before_array = |from: &TypeInst, to: &TypeInst, actual: &TypeInst| {
            actual.known()
                && actual.instantiation == Instantiation::Parameter
                && !crate::value_safety::optional(actual)
                && matches!(&actual.kind, TypeKind::Set(element)
                    if element.instantiation == Instantiation::Parameter
                        && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)))
                && matches!(from.kind, TypeKind::Set(_))
                && matches!(&to.kind, TypeKind::Array { indices, .. } if indices.len() == 1)
        };
        let minima: Vec<_> = matches
            .iter()
            .filter(|candidate| {
                matches.iter().all(|other| {
                    candidate.1.len() == other.1.len()
                        && candidate.1.iter().zip(&other.1).zip(arguments).all(
                            |((a, b), (_, actual))| coerces(a, b) || set_before_array(a, b, actual),
                        )
                        && (!candidate
                            .1
                            .iter()
                            .zip(&other.1)
                            .zip(&candidate.2)
                            .zip(arguments)
                            .all(|(((a, b), pattern), (_, actual))| {
                                coerces(b, a)
                                    || set_before_array(b, a, actual)
                                    || (matches!(
                                        pattern.kind,
                                        TypeKind::Variable { any: true, .. }
                                    ) && a.kind == b.kind)
                            })
                            || candidate
                                .2
                                .iter()
                                .zip(&other.2)
                                .all(|(a, b)| pattern_narrower(a, b)))
                })
            })
            .collect();
        if let [winner] = minima.as_slice() {
            CallOutcome::Resolved {
                declaration: winner.0,
                parameters: winner.4.clone(),
                return_type: winner.3.clone(),
            }
        } else {
            CallOutcome::Ambiguous {
                candidates: matches.iter().map(|(id, _, _, _, _)| *id).collect(),
            }
        }
    }
}

pub(super) fn is_expression(kind: NodeKind) -> bool {
    use NodeKind::*;
    matches!(
        kind,
        Expression
            | InterpolatedString
            | UnaryExpression
            | BinaryExpression
            | ParenthesizedExpression
            | CallExpression
            | GeneratorCallExpression
            | ConditionalExpression
            | LetExpression
            | SetComprehension
            | ArrayComprehension
            | IndexedArrayComprehension
            | SetLiteral
            | ArrayLiteral
            | MatrixLiteral
            | ArrayAccessExpression
            | FieldAccessExpression
            | AnnotatedExpression
            | RangeExpression
            | TupleLiteral
            | RecordLiteral
            | IndexedArrayEntry
            | IndexTuple
            | NamedArgument
            | RecordLiteralField
    )
}

/// The exact operation head, excluding heads nested in its arguments.
pub(super) fn operation_head_start(
    context: &ModelContext,
    file: FileId,
    node: &SyntaxNode,
) -> Option<usize> {
    let parsed = &context.files[file].parsed;
    let head = node.children().iter().find_map(|child| {
        let SyntaxElement::Token(index) = child else {
            return None;
        };
        let token = &parsed.tokens()[*index];
        (matches!(
            token.kind,
            TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
        ) || symbolic_operator(token.kind).is_some())
        .then_some(token)
    })?;
    Some(head.range.start + context.files[file].byte_offset)
}
pub(super) fn operation_fact<'a>(
    context: &ModelContext,
    calls: &'a CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<&'a CallFact> {
    let start = operation_head_start(context, file, node)?;
    calls
        .calls
        .iter()
        .find(|fact| fact.file == file && fact.location.range.start == start)
}
/// Selected integer or parameter integer-set array concat shape, not an evaluation proof.
pub(super) fn array_concatenation<'a>(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &'a SyntaxNode,
) -> Option<[&'a SyntaxNode; 2]> {
    let parsed = &context.files[file].parsed;
    if node.kind() != NodeKind::BinaryExpression
        || !node.children().iter().any(|child| {
            let SyntaxElement::Token(index) = child else {
                return false;
            };
            let token = &parsed.tokens()[*index];
            token.kind == TokenKind::Concat
                || (matches!(
                    token.kind,
                    TokenKind::Identifier
                        | TokenKind::QuotedIdentifier
                        | TokenKind::InfixIdentifier
                ) && parsed.source()[token.range.clone()].trim_matches(['\'', '`']) == "++")
        })
    {
        return None;
    }
    let fact = operation_fact(context, calls, file, node)?;
    if core_operation_outcome(context, bindings, Some(&fact.outcome), node.kind(), "++") != Ok(true)
    {
        return None;
    }
    let arguments: Vec<_> = node.child_nodes().collect();
    let [left, right] = arguments.as_slice() else {
        return None;
    };
    let CallOutcome::Resolved {
        parameters,
        return_type,
        ..
    } = &fact.outcome
    else {
        return None;
    };
    let array = |ty: &TypeInst| {
        ty.known()
            && !crate::value_safety::optional(ty)
            && matches!(&ty.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known()
                    && indices[0].instantiation == Instantiation::Parameter
                    && !crate::value_safety::optional(&indices[0])
                    && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
                    && element.known() && !crate::value_safety::optional(element)
                    && (element.kind == TypeKind::Int
                        || ty.instantiation == Instantiation::Parameter
                            && indices[0].kind == TypeKind::Int
                            && element.instantiation == Instantiation::Parameter
                            && matches!(&element.kind, TypeKind::Set(member)
                                if member.known()
                                    && !crate::value_safety::optional(member)
                                    && member.instantiation == Instantiation::Parameter
                                    && member.kind == TypeKind::Int)))
    };
    let ty = |value: &SyntaxNode| {
        let range = context.files[file].location(value.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if parameters.len() != 2
        || !parameters.iter().all(array)
        || !array(return_type)
        || ty(node) != Some(return_type)
        || !matches!(&return_type.kind, TypeKind::Array { indices, .. }
            if indices[0].kind == TypeKind::Int)
    {
        return None;
    }
    for (argument, formal) in arguments.iter().zip(parameters) {
        let actual = ty(argument)?;
        if !array(actual) || !crate::types::coerces(actual, formal) {
            return None;
        }
        let (
            TypeKind::Array {
                indices: actual, ..
            },
            TypeKind::Array {
                indices: formal, ..
            },
        ) = (&actual.kind, &formal.kind)
        else {
            return None;
        };
        if actual != formal {
            return None;
        }
    }
    let elements: Vec<_> = parameters
        .iter()
        .chain(std::iter::once(return_type))
        .filter_map(|ty| match &ty.kind {
            TypeKind::Array { element, .. } => Some(element),
            _ => None,
        })
        .collect();
    if !elements.windows(2).all(|pair| pair[0] == pair[1]) {
        return None;
    }
    Some([*left, *right])
}

pub(super) fn core_operation(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &SyntaxNode,
    name: &str,
) -> Result<bool, String> {
    core_operation_outcome(
        context,
        bindings,
        operation_fact(context, calls, file, node).map(|fact| &fact.outcome),
        node.kind(),
        name,
    )
}
pub(super) fn core_operation_outcome(
    context: &ModelContext,
    bindings: &BindingFacts,
    outcome: Option<&CallOutcome>,
    kind: NodeKind,
    name: &str,
) -> Result<bool, String> {
    match outcome {
        Some(CallOutcome::Resolved { declaration, .. }) => Ok(crate::definitions::core_callable(
            context,
            bindings,
            *declaration,
            name,
        )),
        Some(CallOutcome::Intrinsic {
            name: intrinsic, ..
        }) => Ok(name == "+" && intrinsic == "unary+" && kind == NodeKind::UnaryExpression),
        Some(
            CallOutcome::NoMatch { reason }
            | CallOutcome::Unresolved { reason }
            | CallOutcome::Unsupported { reason, .. },
        ) => Err(format!("operation {name}: {reason}")),
        Some(CallOutcome::Ambiguous { .. }) => Err(format!("operation {name} is ambiguous")),
        None => Err(format!("operation {name} identity is unavailable")),
    }
}

/// Match a prospective equality using the existing concrete type-inst matcher.
/// Local operator names are conservatively withheld rather than reconstructed
/// from source spelling. The ordinary fact resolver remains unchanged.
pub(super) fn prospective_core_equality(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &CallableFacts,
    file: FileId,
    operands: &[TypeInst],
) -> bool {
    if bindings.declarations.iter().any(|d| {
        d.name == "="
            && ((!d.top_level && d.file == file)
                || (d.top_level
                    && !matches!(
                        d.role,
                        DeclarationRole::Function
                            | DeclarationRole::Predicate
                            | DeclarationRole::Annotation
                    )))
    }) {
        return false;
    }
    let ids: Vec<_> = bindings
        .declarations
        .iter()
        .filter(|d| d.top_level && d.name == "=")
        .map(|d| d.id)
        .collect();
    let mut engine = engine_from_facts(context, bindings, facts);
    let arguments: Vec<_> = operands.iter().cloned().map(|ty| (None, ty)).collect();
    matches!(
        engine.choose(&ids, &arguments),
        CallOutcome::Resolved { declaration, .. }
            if crate::definitions::core_callable(context, bindings, declaration, "=")
    )
}
fn engine_from_facts<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    facts: &CallableFacts,
) -> Engine<'a> {
    Engine {
        context,
        bindings,
        reference_index: None,
        nodes: bindings
            .declarations
            .iter()
            .map(|d| find_node(context.files[d.file].parsed.tree(), &d.syntax_range, d.role))
            .collect(),
        declared: facts
            .declarations
            .iter()
            .map(|d| Some(d.ty.clone()))
            .collect(),
        active: Vec::new(),
        signatures: facts
            .signatures
            .iter()
            .map(|s| (s.declaration.0, Some(s.clone())))
            .collect(),
        expressions: BTreeMap::new(),
        calls: BTreeMap::new(),
        active_calls: Vec::new(),
    }
}

/// Interpret one actually selected body with its concrete formal types. The
/// original context-free facts remain unchanged, including unknown candidates.
pub(super) fn instantiated_body(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &CallableFacts,
    id: DeclarationId,
    parameters: &[TypeInst],
) -> CallableFacts {
    let declaration = &bindings.declarations[id.0];
    let node = find_node(
        context.files[declaration.file].parsed.tree(),
        &declaration.syntax_range,
        declaration.role,
    )
    .unwrap();
    let mut engine = engine_from_facts(context, bindings, facts);
    let mut body_declarations = Vec::new();
    for d in &bindings.declarations {
        if d.file == declaration.file
            && node.range().start <= d.syntax_range.start
            && d.syntax_range.end <= node.range().end
            && matches!(d.role, DeclarationRole::Local | DeclarationRole::Generator)
        {
            engine.declared[d.id.0] = None;
            body_declarations.push(d.id);
        }
    }
    for (position, ty) in parameters.iter().enumerate() {
        if let Some(formal) = formal_parameter(context, bindings, id, position) {
            engine.declared[formal.0] = Some(ty.clone());
        }
    }
    // Defaults retain the same concrete formal view as the selected body.
    if let Some(list) = node
        .child_nodes()
        .find(|n| n.kind() == NodeKind::ParameterList)
    {
        for parameter in list.child_nodes() {
            for default in parameter.child_nodes().filter(|n| is_expression(n.kind())) {
                engine.inspect(declaration.file, declaration.item, default);
            }
        }
    }
    if let Some(body) = node
        .child_nodes()
        .filter(|n| is_expression(n.kind()))
        .last()
    {
        engine.inspect(declaration.file, declaration.item, body);
    }
    // Unreferenced locals still evaluate their initializers. Keep their declared
    // types in this concrete body view, rather than losing them on projection.
    for id in body_declarations {
        engine.declared_type(id);
    }
    CallableFacts {
        declarations: engine
            .declared
            .into_iter()
            .enumerate()
            .map(|(i, ty)| DeclarationType {
                declaration: DeclarationId(i),
                ty: ty.unwrap_or_else(|| TypeInst::unknown("body declaration type unavailable")),
            })
            .collect(),
        signatures: engine.signatures.into_values().flatten().collect(),
        expressions: engine
            .expressions
            .into_iter()
            .map(|((file, start, end), ty)| ExpressionType {
                file,
                location: context.files[file].location(start..end),
                ty,
            })
            .collect(),
        calls: engine.calls.into_values().collect(),
    }
}
pub(super) fn formal_parameter(
    context: &ModelContext,
    bindings: &BindingFacts,
    id: DeclarationId,
    position: usize,
) -> Option<DeclarationId> {
    let d = &bindings.declarations[id.0];
    let node = find_node(context.files[d.file].parsed.tree(), &d.syntax_range, d.role)?;
    let parameter = node
        .child_nodes()
        .find(|n| n.kind() == NodeKind::ParameterList)?
        .child_nodes()
        .nth(position)?;
    bindings
        .declarations
        .iter()
        .find(|p| {
            p.file == d.file
                && p.role == DeclarationRole::Parameter
                && p.syntax_range == parameter.range()
        })
        .map(|p| p.id)
}
/// Written actual or retained default in its declaration's lexical scope.
/// A generator's collection actual has no ordinary expression node: position
/// zero is handled with its separate type and source by invocation inspection.
pub(super) fn call_argument<'a>(
    context: &'a ModelContext,
    bindings: &BindingFacts,
    facts: &CallableFacts,
    file: FileId,
    call: &'a SyntaxNode,
    id: DeclarationId,
    position: usize,
) -> Option<(FileId, &'a SyntaxNode)> {
    let signature = facts.signatures.iter().find(|s| s.declaration == id)?;
    let name = signature.parameters.get(position)?.name.as_deref();
    if call.kind() == NodeKind::GeneratorCallExpression && position == 0 {
        return None;
    }
    let mut index = 0;
    for value in call
        .child_nodes()
        .filter(|_| call.kind() != NodeKind::GeneratorCallExpression)
    {
        if value.kind() == NodeKind::NamedArgument {
            let written = value.children().iter().find_map(|c| {
                if let SyntaxElement::Token(i) = c {
                    let t = &context.files[file].parsed.tokens()[*i];
                    matches!(t.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier).then(
                        || context.files[file].parsed.source()[t.range.clone()].trim_matches('\''),
                    )
                } else {
                    None
                }
            });
            if written == name {
                return value.child_nodes().next().map(|n| (file, n));
            }
        } else {
            if index == position {
                return Some((file, value));
            }
            index += 1;
        }
    }
    let d = &bindings.declarations[id.0];
    let declaration = find_node(context.files[d.file].parsed.tree(), &d.syntax_range, d.role)?;
    let parameter = declaration
        .child_nodes()
        .find(|n| n.kind() == NodeKind::ParameterList)?
        .child_nodes()
        .nth(position)?;
    parameter
        .child_nodes()
        .find(|n| is_expression(n.kind()))
        .map(|n| (d.file, n))
}
