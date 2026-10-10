//! Declaration identity and lexical references, independent of lint policy.
use std::collections::BTreeMap;
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

use crate::{FileId, ModelContext, SourceLocation};

/// An index into one BindingFacts result, valid only with that result/context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeclarationId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationRole {
    Value,
    Function,
    Predicate,
    Test,
    Annotation,
    TypeAlias,
    Enum,
    EnumMember,
    EnumConstructor,
    Parameter,
    Local,
    Generator,
    Index,
}

impl DeclarationRole {
    fn callable(self) -> bool {
        matches!(
            self,
            Self::Function
                | Self::Predicate
                | Self::Test
                | Self::Annotation
                | Self::EnumConstructor
        )
    }
}

/// Declared instantiation only; no expression evaluation or parameter data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instantiation {
    Parameter,
    Decision,
    Unknown,
}

#[derive(Debug)]
pub struct Declaration {
    pub id: DeclarationId,
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    /// Owning CST node range in the parsed (BOM-stripped) source.
    pub syntax_range: std::ops::Range<usize>,
    /// Lexical identity without quote delimiters; source retains exact spelling.
    pub name: String,
    pub role: DeclarationRole,
    pub top_level: bool,
    /// Lookup before this declaration is bound, excluding its own lexical frame.
    /// Callable formals use the enclosing scope, as their defaults do. Top-level
    /// declarations have no enclosing binding; ambiguity is retained explicitly.
    pub shadowed: BindingResolution,
    pub instantiation: Instantiation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReferenceKind {
    Value,
    Callable,
    Type,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BindingResolution {
    Resolved(DeclarationId),
    /// Callable candidates are retained without guessing a signature match.
    Overloads(Vec<DeclarationId>),
    Unresolved,
    Ambiguous(Vec<DeclarationId>),
}

#[derive(Debug)]
pub struct Reference {
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub name: String,
    pub kind: ReferenceKind,
    pub resolution: BindingResolution,
    /// Present only inside a callable body, not its signature or defaults.
    pub callable: Option<DeclarationId>,
}

#[derive(Debug, Default)]
pub struct BindingFacts {
    pub declarations: Vec<Declaration>,
    pub references: Vec<Reference>,
}

/// Resolve retained sources without enabling rules or producing diagnostics.
/// Unknown aliases and unresolved/ambiguous references remain explicit facts.
/// Files with syntax errors are omitted; ModelContext retains those errors.
pub fn resolve_bindings(context: &ModelContext) -> BindingFacts {
    let mut builder = Builder {
        context,
        facts: BindingFacts::default(),
        globals: BTreeMap::new(),
        types: Vec::new(),
        top_nodes: BTreeMap::new(),
    };
    for (file, source) in context.files.iter().enumerate() {
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            builder.collect_top(file, item, node);
        }
    }
    for index in 0..builder.facts.declarations.len() {
        let status = builder.declared_status(DeclarationId(index), &mut Vec::new());
        builder.facts.declarations[index].instantiation = status;
    }
    for (file, source) in context.files.iter().enumerate() {
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            builder.walk(file, item, node, &mut Vec::new(), None);
        }
    }
    builder.facts
}

type Namespace = BTreeMap<String, Vec<DeclarationId>>;

struct Builder<'a> {
    context: &'a ModelContext,
    facts: BindingFacts,
    globals: Namespace,
    types: Vec<Option<(FileId, &'a SyntaxNode)>>,
    top_nodes: BTreeMap<(FileId, usize), DeclarationId>,
}

impl<'a> Builder<'a> {
    fn names(&self, file: FileId, node: &SyntaxNode) -> Vec<usize> {
        let parsed = &self.context.files[file].parsed;
        node.children()
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Token(index)
                    if matches!(
                        parsed.tokens()[*index].kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier
                    ) =>
                {
                    Some(*index)
                }
                _ => None,
            })
            .collect()
    }

    fn declare(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        role: DeclarationRole,
        scopes: Option<&[Namespace]>,
    ) -> Vec<DeclarationId> {
        let top_level = scopes.is_none();
        let ty = match role {
            DeclarationRole::TypeAlias => node
                .child_nodes()
                .find(|child| child.kind() != NodeKind::Annotation),
            DeclarationRole::Value
            | DeclarationRole::Function
            | DeclarationRole::Parameter
            | DeclarationRole::Local => node.child_nodes().next(),
            _ => None,
        };
        let mut ids = Vec::new();
        for index in self.names(file, node) {
            let source = &self.context.files[file];
            let token = &source.parsed.tokens()[index];
            let name = identity(&source.parsed.source()[token.range.clone()]);
            let shadowed = match scopes {
                None => BindingResolution::Unresolved,
                Some(scopes) => {
                    let outer = if role == DeclarationRole::Parameter {
                        scopes
                    } else {
                        &scopes[..scopes.len().saturating_sub(1)]
                    };
                    self.lookup(&name, outer)
                }
            };
            let id = DeclarationId(self.facts.declarations.len());
            self.facts.declarations.push(Declaration {
                id,
                file,
                item,
                location: source.location(token.range.clone()),
                syntax_range: node.range(),
                name: name.clone(),
                role,
                top_level,
                shadowed,
                instantiation: Instantiation::Unknown,
            });
            self.types.push(ty.map(|ty| (file, ty)));
            if top_level {
                self.globals.entry(name).or_default().push(id);
                self.top_nodes.insert((file, node.range().start), id);
            } else {
                let status = if let Some((file, ty)) = self.types[id.0] {
                    self.type_status(file, ty, &mut Vec::new(), scopes.unwrap())
                } else {
                    self.declared_status(id, &mut Vec::new())
                };
                self.facts.declarations[id.0].instantiation = status;
            }
            ids.push(id);
        }
        ids
    }

    fn collect_top(&mut self, file: FileId, item: usize, node: &'a SyntaxNode) {
        use DeclarationRole as R;
        let role = match node.kind() {
            NodeKind::Declaration => Some(R::Value),
            NodeKind::FunctionDeclaration => Some(R::Function),
            NodeKind::PredicateDeclaration => Some(R::Predicate),
            NodeKind::TestDeclaration => Some(R::Test),
            NodeKind::AnnotationDeclaration => Some(R::Annotation),
            NodeKind::TypeAlias => Some(R::TypeAlias),
            NodeKind::EnumDeclaration => Some(R::Enum),
            NodeKind::EnumCase => Some(R::EnumMember),
            NodeKind::EnumConstructor => Some(R::EnumConstructor),
            _ => None,
        };
        if let Some(role) = role {
            self.declare(file, item, node, role, None);
        }
        if matches!(
            node.kind(),
            NodeKind::EnumDeclaration | NodeKind::EnumDefinition | NodeKind::EnumCases
        ) {
            for child in node.child_nodes() {
                self.collect_top(file, item, child);
            }
        }
    }

    fn declared_status(&self, id: DeclarationId, active: &mut Vec<DeclarationId>) -> Instantiation {
        if active.contains(&id) {
            return Instantiation::Unknown;
        }
        active.push(id);
        let status = if let Some((file, ty)) = self.types[id.0] {
            self.type_status(file, ty, active, &[])
        } else {
            match self.facts.declarations[id.0].role {
                DeclarationRole::Enum
                | DeclarationRole::EnumMember
                | DeclarationRole::EnumConstructor
                | DeclarationRole::Annotation
                | DeclarationRole::Index => Instantiation::Parameter,
                _ => Instantiation::Unknown,
            }
        };
        active.pop();
        status
    }

    fn type_status(
        &self,
        file: FileId,
        node: &SyntaxNode,
        active: &mut Vec<DeclarationId>,
        scopes: &[Namespace],
    ) -> Instantiation {
        let parsed = &self.context.files[file].parsed;
        let has = |kind| {
            node.children().iter().any(|child| matches!(child, SyntaxElement::Token(index) if parsed.tokens()[*index].kind == kind))
        };
        // Explicit qualifiers override a synonym's underlying instantiation.
        // https://docs.minizinc.dev/en/2.10.1/spec.html#type-inst-synonyms
        if has(TokenKind::Var) {
            return Instantiation::Decision;
        }
        if has(TokenKind::Par) {
            return Instantiation::Parameter;
        }
        if has(TokenKind::Any) {
            return Instantiation::Unknown;
        }
        match node.kind() {
            NodeKind::ScalarType => Instantiation::Parameter,
            NodeKind::TypeInstVariable => Instantiation::Unknown,
            NodeKind::ArrayType | NodeKind::ListType | NodeKind::SetType => node
                .child_nodes()
                .last()
                .map_or(Instantiation::Unknown, |element| {
                    self.type_status(file, element, active, scopes)
                }),
            NodeKind::RecordField => node
                .child_nodes()
                .next()
                .map_or(Instantiation::Unknown, |ty| {
                    self.type_status(file, ty, active, scopes)
                }),
            NodeKind::TupleType | NodeKind::RecordType | NodeKind::TypeInstConcatenation => {
                let statuses: Vec<_> = node
                    .child_nodes()
                    .map(|ty| self.type_status(file, ty, active, scopes))
                    .collect();
                if statuses.contains(&Instantiation::Decision) {
                    Instantiation::Decision
                } else if statuses.contains(&Instantiation::Unknown) {
                    Instantiation::Unknown
                } else {
                    Instantiation::Parameter
                }
            }
            NodeKind::DomainType => {
                let Some(expression) = node.child_nodes().next() else {
                    return Instantiation::Unknown;
                };
                let Some(index) = self.atom_name(file, expression) else {
                    return Instantiation::Parameter;
                };
                let name = identity(&parsed.source()[parsed.tokens()[index].range.clone()]);
                match self.lookup(&name, scopes) {
                    BindingResolution::Resolved(id) => match self.facts.declarations[id.0].role {
                        DeclarationRole::TypeAlias => self.declared_status(id, active),
                        DeclarationRole::Enum => Instantiation::Parameter,
                        DeclarationRole::Value
                            if self.declared_status(id, active) == Instantiation::Parameter =>
                        {
                            Instantiation::Parameter
                        }
                        DeclarationRole::Parameter
                        | DeclarationRole::Local
                        | DeclarationRole::Generator
                        | DeclarationRole::Index
                            if self.facts.declarations[id.0].instantiation
                                == Instantiation::Parameter =>
                        {
                            Instantiation::Parameter
                        }
                        _ => Instantiation::Unknown,
                    },
                    _ => Instantiation::Unknown,
                }
            }
            _ => Instantiation::Unknown,
        }
    }

    fn atom_name(&self, file: FileId, mut node: &SyntaxNode) -> Option<usize> {
        while node.kind() == NodeKind::ParenthesizedExpression {
            node = node.child_nodes().next()?;
        }
        (node.kind() == NodeKind::Expression)
            .then(|| self.names(file, node).first().copied())
            .flatten()
    }

    fn lookup(&self, name: &str, scopes: &[Namespace]) -> BindingResolution {
        let candidates = scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .or_else(|| self.globals.get(name));
        match candidates.map(Vec::as_slice).unwrap_or_default() {
            [] => BindingResolution::Unresolved,
            [id] => BindingResolution::Resolved(*id),
            ids if ids
                .iter()
                .all(|id| self.facts.declarations[id.0].role.callable()) =>
            {
                BindingResolution::Overloads(ids.to_vec())
            }
            ids => BindingResolution::Ambiguous(ids.to_vec()),
        }
    }

    fn reference(
        &mut self,
        file: FileId,
        item: usize,
        index: usize,
        kind: ReferenceKind,
        scopes: &[Namespace],
        callable: Option<DeclarationId>,
    ) {
        let source = &self.context.files[file];
        let token = &source.parsed.tokens()[index];
        let name = symbolic_operator(token.kind)
            .map(str::to_owned)
            .unwrap_or_else(|| identity(&source.parsed.source()[token.range.clone()]));
        let resolution = match self.lookup(&name, scopes) {
            BindingResolution::Ambiguous(ids) if kind == ReferenceKind::Value => {
                // A value and a callable may share a name. Prefer the unique
                // non-callable in value context; keep bare callable/annotation
                // and genuinely ambiguous cases in their existing form.
                let values: Vec<_> = ids
                    .iter()
                    .copied()
                    .filter(|id| !self.facts.declarations[id.0].role.callable())
                    .collect();
                match values.as_slice() {
                    [id] => BindingResolution::Resolved(*id),
                    _ => BindingResolution::Ambiguous(ids),
                }
            }
            BindingResolution::Ambiguous(ids) if kind == ReferenceKind::Callable => {
                // Keep overload selection separate from the same-name value.
                // Other mixed declarations remain ambiguous in this scope.
                let mut values = 0;
                let mut candidates = Vec::new();
                let mut supported = true;
                for id in &ids {
                    let declaration = &self.facts.declarations[id.0];
                    match declaration.role {
                        DeclarationRole::Value if declaration.top_level => values += 1,
                        DeclarationRole::Function
                        | DeclarationRole::Predicate
                        | DeclarationRole::Test
                        | DeclarationRole::Annotation => candidates.push(*id),
                        _ => {
                            supported = false;
                            break;
                        }
                    }
                }
                if supported && values == 1 {
                    match candidates.as_slice() {
                        [id] => BindingResolution::Resolved(*id),
                        [] => BindingResolution::Ambiguous(ids),
                        _ => BindingResolution::Overloads(candidates),
                    }
                } else {
                    BindingResolution::Ambiguous(ids)
                }
            }
            resolution => resolution,
        };
        self.facts.references.push(Reference {
            file,
            item,
            location: source.location(token.range.clone()),
            name,
            kind,
            resolution,
            callable,
        });
    }

    fn bind(&self, scope: &mut Namespace, ids: Vec<DeclarationId>) {
        for id in ids {
            scope
                .entry(self.facts.declarations[id.0].name.clone())
                .or_default()
                .push(id);
        }
    }

    fn walk(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scopes: &mut Vec<Namespace>,
        callable: Option<DeclarationId>,
    ) {
        use NodeKind::*;
        match node.kind() {
            FunctionDeclaration
            | PredicateDeclaration
            | TestDeclaration
            | AnnotationDeclaration => self.walk_callable(file, item, node, scopes),
            LetExpression => {
                scopes.push(Namespace::new());
                for child in node.child_nodes() {
                    if child.kind() == LetBlock {
                        for local in child.child_nodes() {
                            self.walk(file, item, local, scopes, callable);
                            if local.kind() == Declaration {
                                let ids = self.declare(
                                    file,
                                    item,
                                    local,
                                    DeclarationRole::Local,
                                    Some(scopes),
                                );
                                self.bind(scopes.last_mut().unwrap(), ids);
                            }
                        }
                    } else {
                        self.walk(file, item, child, scopes, callable);
                    }
                }
                scopes.pop();
            }
            SetComprehension
            | ArrayComprehension
            | IndexedArrayComprehension
            | GeneratorCallExpression => {
                if node.kind() == GeneratorCallExpression {
                    for index in self.names(file, node) {
                        self.reference(
                            file,
                            item,
                            index,
                            ReferenceKind::Callable,
                            scopes,
                            callable,
                        );
                    }
                }
                scopes.push(Namespace::new());
                for list in node
                    .child_nodes()
                    .filter(|child| child.kind() == GeneratorList)
                {
                    for generator in list.child_nodes() {
                        let mut children = generator.child_nodes();
                        if let Some(source) = children.next() {
                            self.walk(file, item, source, scopes, callable);
                        }
                        let ids = self.declare(
                            file,
                            item,
                            generator,
                            DeclarationRole::Generator,
                            Some(scopes),
                        );
                        self.bind(scopes.last_mut().unwrap(), ids);
                        for filter in children {
                            self.walk(file, item, filter, scopes, callable);
                        }
                    }
                }
                for head in node
                    .child_nodes()
                    .filter(|child| child.kind() != GeneratorList)
                {
                    self.walk(file, item, head, scopes, callable);
                }
                scopes.pop();
            }
            ArrayType => {
                scopes.push(Namespace::new());
                for child in node.child_nodes() {
                    self.walk(file, item, child, scopes, callable);
                    if child.kind() == ArrayIndexBinding {
                        let ids =
                            self.declare(file, item, child, DeclarationRole::Index, Some(scopes));
                        self.bind(scopes.last_mut().unwrap(), ids);
                    }
                }
                scopes.pop();
            }
            DomainType => {
                if let Some(expression) = node.child_nodes().next() {
                    if let Some(index) = self.atom_name(file, expression) {
                        let parsed = &self.context.files[file].parsed;
                        let name = identity(&parsed.source()[parsed.tokens()[index].range.clone()]);
                        let kind = match self.lookup(&name, scopes) {
                            BindingResolution::Resolved(id)
                                if matches!(
                                    self.facts.declarations[id.0].role,
                                    DeclarationRole::Value
                                        | DeclarationRole::Parameter
                                        | DeclarationRole::Local
                                        | DeclarationRole::Generator
                                        | DeclarationRole::Index
                                ) =>
                            {
                                ReferenceKind::Value
                            }
                            BindingResolution::Ambiguous(ids) => {
                                let mut values = ids
                                    .iter()
                                    .filter(|id| !self.facts.declarations[id.0].role.callable());
                                match (values.next(), values.next()) {
                                    (Some(id), None)
                                        if matches!(
                                            self.facts.declarations[id.0].role,
                                            DeclarationRole::Value
                                                | DeclarationRole::Parameter
                                                | DeclarationRole::Local
                                                | DeclarationRole::Generator
                                                | DeclarationRole::Index
                                        ) =>
                                    {
                                        ReferenceKind::Value
                                    }
                                    _ => ReferenceKind::Type,
                                }
                            }
                            _ => ReferenceKind::Type,
                        };
                        self.reference(file, item, index, kind, scopes, callable);
                    } else {
                        self.walk(file, item, expression, scopes, callable);
                    }
                }
            }
            ArrayAccessExpression => {
                for (position, child) in node.child_nodes().enumerate() {
                    if position > 0
                        && child.kind() == RangeExpression
                        && child.child_nodes().next().is_none()
                    {
                        let parsed = &self.context.files[file].parsed;
                        let mut tokens = child.children().iter().filter_map(|element| {
                            let SyntaxElement::Token(index) = element else {
                                return None;
                            };
                            let kind = parsed.tokens()[*index].kind;
                            (!matches!(
                                kind,
                                TokenKind::Whitespace
                                    | TokenKind::LineComment
                                    | TokenKind::BlockComment
                            ))
                            .then_some(kind)
                        });
                        // A bare '..' selector denotes the full axis, not a call.
                        if matches!(
                            (tokens.next(), tokens.next()),
                            (Some(TokenKind::RangeInclusive), None)
                        ) {
                            continue;
                        }
                    }
                    self.walk(file, item, child, scopes, callable);
                }
            }
            Expression | CallExpression | Assignment => {
                let kind = if node.kind() == CallExpression {
                    ReferenceKind::Callable
                } else {
                    ReferenceKind::Value
                };
                for index in self.names(file, node) {
                    self.reference(file, item, index, kind, scopes, callable);
                }
                for child in node.child_nodes() {
                    self.walk(file, item, child, scopes, callable);
                }
            }
            _ => {
                // Direct field/named-argument labels and declaration names never
                // become references. Expressions beneath them still do.
                let parsed = &self.context.files[file].parsed;
                for child in node.children() {
                    if let SyntaxElement::Token(index) = child
                        && (parsed.tokens()[*index].kind == TokenKind::InfixIdentifier
                            || (matches!(
                                node.kind(),
                                UnaryExpression | BinaryExpression | RangeExpression
                            ) && symbolic_operator(parsed.tokens()[*index].kind).is_some()))
                    {
                        self.reference(
                            file,
                            item,
                            *index,
                            ReferenceKind::Callable,
                            scopes,
                            callable,
                        );
                    }
                }
                for child in node.child_nodes() {
                    self.walk(file, item, child, scopes, callable);
                }
            }
        }
    }

    fn walk_callable(
        &mut self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        scopes: &mut Vec<Namespace>,
    ) {
        let owner = self.top_nodes.get(&(file, node.range().start)).copied();
        let mut parameters_scope = Namespace::new();
        let mut in_body = false;
        for child in node.children() {
            match child {
                SyntaxElement::Token(index)
                    if self.context.files[file].parsed.tokens()[*index].kind
                        == TokenKind::Equal =>
                {
                    scopes.push(std::mem::take(&mut parameters_scope));
                    in_body = true;
                }
                SyntaxElement::Node(parameters) if parameters.kind() == NodeKind::ParameterList => {
                    for parameter in parameters.child_nodes() {
                        // Defaults use the enclosing scope, never earlier formals.
                        // https://docs.minizinc.dev/en/2.10.1/spec.html#user-defined-operations
                        self.walk(file, item, parameter, scopes, None);
                        let ids = self.declare(
                            file,
                            item,
                            parameter,
                            DeclarationRole::Parameter,
                            Some(scopes),
                        );
                        self.bind(&mut parameters_scope, ids);
                    }
                }
                SyntaxElement::Node(child) => self.walk(
                    file,
                    item,
                    child,
                    scopes,
                    if in_body { owner } else { None },
                ),
                _ => {}
            }
        }
        if in_body {
            scopes.pop();
        }
    }
}

fn identity(written: &str) -> String {
    written
        .strip_prefix('\'')
        .and_then(|name| name.strip_suffix('\''))
        .or_else(|| {
            written
                .strip_prefix('`')
                .and_then(|name| name.strip_suffix('`'))
        })
        .unwrap_or(written)
        .to_owned()
}

/// Canonical symbolic callable names; spelling remains in the CST.
pub(super) fn symbolic_operator(kind: TokenKind) -> Option<&'static str> {
    Some(match kind {
        TokenKind::Equal | TokenKind::DoubleEqual => "=",
        TokenKind::Implies => "->",
        TokenKind::ReverseImplies => "<-",
        TokenKind::Plus => "+",
        TokenKind::Default => "default",
        TokenKind::Minus => "-",
        TokenKind::Star => "*",
        TokenKind::Div => "div",
        TokenKind::Mod => "mod",
        TokenKind::NotEqual => "!=",
        TokenKind::WeakEqual => "~=",
        TokenKind::WeakNotEqual => "~!=",
        TokenKind::Less => "<",
        TokenKind::LessEqual => "<=",
        TokenKind::Greater => ">",
        TokenKind::GreaterEqual => ">=",
        TokenKind::Equivalence => "<->",
        TokenKind::Or => "\\/",
        TokenKind::And => "/\\",
        TokenKind::Not => "not",
        TokenKind::Xor => "xor",
        TokenKind::Slash => "/",
        TokenKind::Power | TokenKind::Inverse => "^",
        TokenKind::Concat => "++",
        TokenKind::WeakPlus => "~+",
        TokenKind::WeakMinus => "~-",
        TokenKind::WeakStar => "~*",
        TokenKind::WeakSlash => "~/",
        TokenKind::WeakDiv => "~div",
        TokenKind::In => "in",
        TokenKind::Subset => "subset",
        TokenKind::Superset => "superset",
        TokenKind::Union => "union",
        TokenKind::Intersect => "intersect",
        TokenKind::Diff => "diff",
        TokenKind::SymDiff => "symdiff",
        TokenKind::RangeInclusive => "..",
        TokenKind::RangeExclusiveStart => "<..",
        TokenKind::RangeExclusiveEnd => "..<",
        TokenKind::RangeExclusive => "<..<",
        _ => return None,
    })
}
