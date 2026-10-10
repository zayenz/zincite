//! Bounded enum membership reduction over original data and read-only model facts.
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

use zincite_lint::{
    BindingFacts, BindingResolution, DeclarationId, DeclarationRole, Domain, DomainFacts, EditPart,
    ExpressionType, ModelContext, SourceLocation, TextEdit, TypeInst, TypeKind, resolve_bindings,
    resolve_callables, resolve_domains,
};
use zincite_syntax::{FileMode, NodeKind, SyntaxNode, TokenKind};

use crate::language::Retention;
use crate::structured::{self, ArrayIndexing, Key};
use crate::{ErrorLocation, Input, Limits, QueryError, Work, assignment_rhs, identifier_identity};

#[derive(Debug)]
pub struct UnresolvedDependency {
    pub location: SourceLocation,
    pub message: String,
}

/// An inspectable, reparsed candidate. Only complete candidates may be written.
#[derive(Debug)]
pub struct ReductionResult {
    candidate: Box<Input>,
    unresolved: Vec<UnresolvedDependency>,
}

impl ReductionResult {
    pub fn candidate(&self) -> &Input {
        &self.candidate
    }
    pub fn unresolved(&self) -> &[UnresolvedDependency] {
        &self.unresolved
    }
    pub fn is_complete(&self) -> bool {
        self.unresolved.is_empty()
    }
}

// Written domains accompany expanded types: an enum element type alone does
// not establish the membership of an array's index subset.
struct Shape {
    ty: TypeInst,
    indices: Vec<Domain>,
    components: Vec<(Option<String>, Shape)>,
}

impl Shape {
    fn contains_enum(&self, target: Option<DeclarationId>) -> bool {
        target.is_some_and(|target| match &self.ty.kind {
            TypeKind::Enum(id) => *id == target,
            TypeKind::Array { indices, .. } => {
                indices.iter().any(|ty| ty.kind == TypeKind::Enum(target))
            }
            _ => false,
        }) || self
            .components
            .iter()
            .any(|(_, shape)| shape.contains_enum(target))
    }
}

fn declaration_node<'a>(
    model: &'a ModelContext,
    bindings: &BindingFacts,
    id: DeclarationId,
) -> Option<&'a SyntaxNode> {
    let declaration = &bindings.declarations[id.0];
    model.files[declaration.file]
        .parsed
        .tree()
        .child_nodes()
        .nth(declaration.item)
}

struct ShapeReader<'a, 'w> {
    model: &'a ModelContext,
    bindings: &'a BindingFacts,
    domains: &'a DomainFacts,
    work: &'w mut Work,
    range: &'w Range<usize>,
    limits: Limits,
}

impl ShapeReader<'_, '_> {
    fn shape(
        &mut self,
        ty: &TypeInst,
        file: usize,
        node: Option<&SyntaxNode>,
        depth: usize,
    ) -> Result<Shape, QueryError> {
        let model = self.model;
        let bindings = self.bindings;
        let domains = self.domains;
        self.work.visit(self.range)?;
        if depth >= self.limits.nesting.min(64) {
            return Err(QueryError::query(
                self.range.clone(),
                "model shape nesting limit exceeded",
            ));
        }
        let mut shape = Shape {
            ty: ty.clone(),
            indices: Vec::new(),
            components: Vec::new(),
        };
        if let Some(node) = node
            && node.kind() == NodeKind::DomainType
        {
            let location = model.files[file].location(node.range());
            self.work.spend(bindings.references.len(), self.range)?;
            if let Some(id) = bindings.references.iter().find_map(|reference| {
                if reference.file == file
                    && location.range.start <= reference.location.range.start
                    && reference.location.range.end <= location.range.end
                    && let BindingResolution::Resolved(id) = reference.resolution
                {
                    return (bindings.declarations[id.0].role == DeclarationRole::TypeAlias)
                        .then_some(id);
                }
                None
            }) {
                let alias = &bindings.declarations[id.0];
                let node = declaration_node(model, bindings, id).and_then(|node| {
                    node.child_nodes()
                        .find(|node| node.kind() != NodeKind::Annotation)
                });
                return self.shape(ty, alias.file, node, depth + 1);
            }
        }
        if let Some(node) = node {
            self.work.spend(node.children().len(), self.range)?;
            if node.child_nodes().count() > self.limits.collection {
                return Err(QueryError::query(
                    self.range.clone(),
                    "query collection limit exceeded",
                ));
            }
        }
        let children: Vec<_> = node.into_iter().flat_map(SyntaxNode::child_nodes).collect();
        match &ty.kind {
            TypeKind::Array { indices, element } => {
                for (axis, _) in indices.iter().enumerate() {
                    self.work.spend(domains.array_indices.len(), self.range)?;
                    let domain = children
                        .get(axis)
                        .and_then(|node| {
                            let location = model.files[file].location(node.range());
                            domains.array_indices.iter().find(|index| {
                                index.file == file && index.location.range == location.range
                            })
                        })
                        .map_or(Domain::Unknown, |index| index.domain.clone());
                    shape.indices.push(domain);
                }
                shape.components.push((
                    None,
                    self.shape(element, file, children.last().copied(), depth + 1)?,
                ));
            }
            TypeKind::Record(fields) => {
                for (name, ty) in fields {
                    self.work.spend(
                        children.iter().map(|node| node.children().len()).sum(),
                        self.range,
                    )?;
                    let field = children.iter().copied().find(|field| {
                        let source = &model.files[file].parsed;
                        field
                            .children()
                            .iter()
                            .filter_map(|child| match child {
                                zincite_syntax::SyntaxElement::Token(index) => {
                                    Some(&source.tokens()[*index])
                                }
                                _ => None,
                            })
                            .any(|token| {
                                matches!(
                                    token.kind,
                                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                                ) && identifier_identity(&source.source()[token.range.clone()])
                                    == name
                            })
                    });
                    shape.components.push((
                        Some(name.clone()),
                        self.shape(
                            ty,
                            file,
                            field.and_then(|field| field.child_nodes().next()),
                            depth + 1,
                        )?,
                    ));
                }
            }
            TypeKind::Tuple(fields) => {
                for (index, ty) in fields.iter().enumerate() {
                    shape.components.push((
                        None,
                        self.shape(ty, file, children.get(index).copied(), depth + 1)?,
                    ));
                }
            }
            TypeKind::Set(element) => shape.components.push((
                None,
                self.shape(element, file, children.last().copied(), depth + 1)?,
            )),
            _ => {}
        }
        Ok(shape)
    }
}
pub(crate) fn reduce(
    input: &Input,
    model: Option<&ModelContext>,
    name: &str,
    retention: &Retention,
    range: &Range<usize>,
    work: &mut Work,
    limits: Limits,
) -> Result<ReductionResult, QueryError> {
    if input.mode != FileMode::Data {
        return Err(QueryError::query(
            range.clone(),
            "reduce_enum requires data mode",
        ));
    }
    let name = identifier_identity(name);
    let items = crate::collect_items(input, limits.collection, work, range, true)?;
    let targets: Vec<_> = items
        .iter()
        .filter(|item| item.name == Some(name))
        .collect();
    if targets.len() != 1 {
        return Err(QueryError::input(
            targets.first().map_or(0..0, |item| item.range.clone()),
            "reduce_enum requires one named enum assignment",
        ));
    }
    let target = targets[0];
    let rhs = assignment_rhs(target.node).unwrap();
    let members_node = structured::unwrapped(rhs);
    if members_node.kind() != NodeKind::SetLiteral {
        return Err(QueryError::input(
            input.original_range(rhs.range()),
            "enum reduction requires an explicit member set; constructed and anonymous enums are unsupported",
        ));
    }
    work.spend(members_node.children().len(), range)?;
    if members_node.child_nodes().count() > limits.collection {
        return Err(QueryError::query(
            range.clone(),
            "query collection limit exceeded",
        ));
    }
    let member_nodes: Vec<_> = members_node.child_nodes().collect();
    let mut members = Vec::new();
    let mut identities = HashSet::new();
    for node in &member_nodes {
        crate::inspection::check_collection(members.len(), limits.collection, range)?;
        work.visit(range)?;
        if let Some(member) = member(input, node) {
            if !identities.insert(member) {
                return Err(QueryError::input(
                    input.original_range(node.range()),
                    "enum member identities must be unique",
                ));
            }
            members.push(member);
        } else {
            return Err(QueryError::input(
                input.original_range(node.range()),
                "enum reduction requires named explicit members",
            ));
        }
    }
    let retained = match retention {
        Retention::First(count) => (0..members.len())
            .map(|index| index < *count)
            .collect::<Vec<_>>(),
        Retention::Keep(names) => {
            let mut requested_members = HashSet::new();
            for (requested, requested_range) in names {
                work.visit(range)?;
                requested_members.insert(identifier_identity(requested));
                if !identities.contains(identifier_identity(requested)) {
                    return Err(QueryError::query(
                        requested_range.clone(),
                        format!("unknown requested enum member '{requested}'"),
                    ));
                }
            }
            members
                .iter()
                .map(|member| requested_members.contains(member))
                .collect()
        }
    };
    if let Some(error) = model.and_then(|model| model.errors.first()) {
        return Err(QueryError {
            location: ErrorLocation::Model(error.location.clone()),
            range: error.location.range.clone(),
            message: error.message.clone(),
        });
    }
    if retained.iter().all(|keep| *keep) {
        return Ok(ReductionResult {
            candidate: Box::new(crate::finish_candidate(input, &[], &[], work, range)?),
            unresolved: Vec::new(),
        });
    }
    let removed: HashSet<_> = members
        .iter()
        .zip(&retained)
        .filter_map(|(member, keep)| (!keep).then_some(*member))
        .collect();
    let mut dependent_names = HashSet::new();
    if model.is_none() {
        for item in &items {
            if item.range == target.range {
                continue;
            }
            if let Some(name) = item.name
                && target_keyed(input, item.node, &identities, work, range, limits)?
            {
                dependent_names.insert(name.to_owned());
            }
        }
    }
    let mut reducer = Reducer {
        input,
        name,
        members,
        removed,
        target: None,
        model_supplied: model.is_some(),
        full_domain_names: HashSet::new(),
        shapes: BTreeMap::new(),
        dependent_names,
        unresolved: Vec::new(),
        work,
        range,
        limits,
        visited: 0,
    };
    if let Some(model) = model {
        reducer.work.spend(
            model
                .files
                .iter()
                .map(|file| file.parsed.tokens().len())
                .sum(),
            range,
        )?;
        let bindings = resolve_bindings(model);
        let calls = resolve_callables(model, &bindings);
        let domains = resolve_domains(model, &bindings);
        reducer.full_domain_names = bindings
            .declarations
            .iter()
            .filter(|declaration| {
                matches!(
                    declaration.role,
                    DeclarationRole::Enum | DeclarationRole::TypeAlias
                )
            })
            .map(|declaration| declaration.id.0)
            .collect();
        let types: BTreeMap<_, _> = calls
            .declarations
            .iter()
            .map(|ty| (ty.declaration.0, &ty.ty))
            .collect();
        let mut named_declarations = BTreeMap::<&str, Vec<_>>::new();
        for declaration in &bindings.declarations {
            reducer.work.visit(range)?;
            if declaration.top_level
                && matches!(
                    declaration.role,
                    DeclarationRole::Value | DeclarationRole::Enum
                )
            {
                named_declarations
                    .entry(&declaration.name)
                    .or_default()
                    .push(declaration);
            }
        }
        let enums: Vec<_> = bindings
            .declarations
            .iter()
            .filter(|declaration| {
                declaration.top_level
                    && declaration.name == name
                    && declaration.role == DeclarationRole::Enum
            })
            .collect();
        reducer.target = (enums.len() == 1).then(|| enums[0].id);
        if reducer.target.is_none() {
            reducer.unresolved(rhs, "model enum association is unavailable or ambiguous");
        }
        for item in &items {
            let Some(name) = item.name else {
                continue;
            };
            let Some(declarations) = named_declarations.get(name) else {
                continue;
            };
            if declarations.len() != 1 {
                continue;
            }
            let declaration = declarations[0];
            if let Some(ty) = types.get(&declaration.id.0) {
                let node = declaration_node(model, &bindings, declaration.id).and_then(|node| {
                    node.child_nodes()
                        .find(|node| node.kind() != NodeKind::Annotation)
                });
                let shape = ShapeReader {
                    model,
                    bindings: &bindings,
                    domains: &domains,
                    work: reducer.work,
                    range,
                    limits,
                }
                .shape(ty, declaration.file, node, 0)?;
                if shape.contains_enum(reducer.target) {
                    reducer.dependent_names.insert(name.to_owned());
                }
                reducer.shapes.insert(name.to_owned(), shape);
            }
        }
        for reference in &bindings.references {
            reducer.work.visit(range)?;
            if !reducer.removed.contains(reference.name.as_str()) {
                continue;
            }
            let relevant = match reference.resolution {
                BindingResolution::Resolved(id) => types.get(&id.0).is_some_and(
                    |ty| matches!(ty.kind, TypeKind::Enum(owner) if Some(owner) == reducer.target),
                ),
                BindingResolution::Unresolved | BindingResolution::Ambiguous(_) => true,
                BindingResolution::Overloads(_) => false,
            };
            if relevant {
                reducer.unresolved.push(UnresolvedDependency {
                    location: reference.location.clone(),
                    message: format!(
                        "removed enum member '{}' remains in the read-only model",
                        reference.name
                    ),
                });
            }
        }
        reducer.model_accesses(model, &bindings, &calls.expressions)?;
    }
    let mut edits = vec![structured::collection_edit(
        input,
        members_node,
        &member_nodes,
        &retained,
        TokenKind::Comma,
        reducer.work,
        range,
    )?];
    // Shapes are immutable request-local facts. Removing them here lets the walk
    // borrow the reducer mutably without cloning the expanded type tree.
    let shapes = std::mem::take(&mut reducer.shapes);
    for item in &items {
        if item.range == target.range {
            continue;
        }
        let rhs = assignment_rhs(item.node).unwrap();
        edits.extend(reducer.walk(rhs, item.name.and_then(|name| shapes.get(name)), 0)?);
    }
    let candidate = crate::finish_candidate(input, &edits, &[], reducer.work, range)?;
    Ok(ReductionResult {
        candidate: Box::new(candidate),
        unresolved: reducer.unresolved,
    })
}

fn member<'a>(input: &'a Input, node: &SyntaxNode) -> Option<&'a str> {
    let node = structured::unwrapped(node);
    if node.kind() != NodeKind::Expression {
        return None;
    }
    let index = structured::direct_token(input, node)?;
    matches!(
        input.syntax().tokens()[index].kind,
        TokenKind::Identifier | TokenKind::QuotedIdentifier
    )
    .then(|| identifier_identity(std::str::from_utf8(input.token_bytes(index)).unwrap()))
}

// Written keys can establish a dependency without model declarations. Gather
// names before walking values so computed uses do not depend on item order.
fn target_keyed(
    input: &Input,
    node: &SyntaxNode,
    members: &HashSet<&str>,
    work: &mut Work,
    range: &Range<usize>,
    limits: Limits,
) -> Result<bool, QueryError> {
    let mut pending = vec![(node, false)];
    while let Some((node, key)) = pending.pop() {
        work.spend(node.children().len().max(1), range)?;
        if node.child_nodes().count() > limits.collection {
            return Err(QueryError::query(
                range.clone(),
                "query collection limit exceeded",
            ));
        }
        if key && member(input, node).is_some_and(|name| members.contains(name)) {
            return Ok(true);
        }
        for (index, child) in node.child_nodes().enumerate() {
            let written_key = match node.kind() {
                NodeKind::IndexedArrayEntry => index == 0,
                NodeKind::MatrixColumnIndices => true,
                NodeKind::MatrixRow => index == 0 && structured::has_colon(input, node),
                _ => false,
            };
            pending.push((child, key || written_key));
        }
    }
    Ok(false)
}

struct Reducer<'a, 'w> {
    input: &'a Input,
    name: &'a str,
    members: Vec<&'a str>,
    removed: HashSet<&'a str>,
    target: Option<DeclarationId>,
    model_supplied: bool,
    full_domain_names: HashSet<usize>,
    shapes: BTreeMap<String, Shape>,
    dependent_names: HashSet<String>,
    unresolved: Vec<UnresolvedDependency>,
    work: &'w mut Work,
    range: &'w Range<usize>,
    limits: Limits,
    visited: usize,
}

impl Reducer<'_, '_> {
    fn model_accesses(
        &mut self,
        model: &ModelContext,
        bindings: &BindingFacts,
        expressions: &[ExpressionType],
    ) -> Result<(), QueryError> {
        let Some(target) = self.target else {
            return Ok(());
        };
        self.work.spend(
            expressions.len().saturating_add(bindings.references.len()),
            self.range,
        )?;
        let types: BTreeMap<_, _> = expressions
            .iter()
            .map(|expression| {
                (
                    (
                        expression.file,
                        expression.location.range.start,
                        expression.location.range.end,
                    ),
                    &expression.ty,
                )
            })
            .collect();
        let references: BTreeMap<_, _> = bindings
            .references
            .iter()
            .map(|reference| ((reference.file, reference.location.range.start), reference))
            .collect();
        let members: HashSet<_> = self.members.iter().copied().collect();
        for (file, source) in model.files.iter().enumerate() {
            let mut pending = vec![source.parsed.tree()];
            while let Some(node) = pending.pop() {
                self.work.spend(node.children().len().max(1), self.range)?;
                if node.child_nodes().count() > self.limits.collection {
                    return Err(QueryError::query(
                        self.range.clone(),
                        "query collection limit exceeded",
                    ));
                }
                if node.kind() == NodeKind::ArrayAccessExpression {
                    let mut children = node.child_nodes();
                    let subject = children.next().unwrap();
                    let location = source.location(subject.range());
                    if let Some(TypeInst {
                        kind: TypeKind::Array { indices, .. },
                        ..
                    }) = types.get(&(file, location.range.start, location.range.end))
                    {
                        let mut computed = false;
                        for (selector, index) in children.zip(indices) {
                            if index.kind != TypeKind::Enum(target) {
                                continue;
                            }
                            let selector = structured::unwrapped(selector);
                            let fixed_member = selector.kind() == NodeKind::Expression
                                && selector.children().iter().any(|child| {
                                    let zincite_syntax::SyntaxElement::Token(index) = child else {
                                        return false;
                                    };
                                    let token = &source.parsed.tokens()[*index];
                                    if !matches!(
                                        token.kind,
                                        TokenKind::Identifier | TokenKind::QuotedIdentifier
                                    ) || !members.contains(identifier_identity(
                                        &source.parsed.source()[token.range.clone()],
                                    )) {
                                        return false;
                                    }
                                    match references
                                        .get(&(file, token.range.start + source.byte_offset))
                                        .map(|reference| &reference.resolution)
                                    {
                                        Some(BindingResolution::Unresolved) => true,
                                        Some(BindingResolution::Resolved(id))
                                            if bindings.declarations[id.0].role
                                                == DeclarationRole::EnumMember =>
                                        {
                                            let location = source.location(selector.range());
                                            types
                                                .get(&(
                                                    file,
                                                    location.range.start,
                                                    location.range.end,
                                                ))
                                                .is_some_and(|ty| ty.kind == TypeKind::Enum(target))
                                        }
                                        _ => false,
                                    }
                                });
                            computed |= !fixed_member;
                        }
                        if computed {
                            self.unresolved.push(UnresolvedDependency {
                                location: source.location(node.range()),
                                message:
                                    "computed enum-indexed access remains in the read-only model"
                                        .into(),
                            });
                        }
                    }
                }
                pending.extend(node.child_nodes());
            }
        }
        Ok(())
    }

    fn unresolved(&mut self, node: &SyntaxNode, message: impl Into<String>) {
        self.unresolved.push(UnresolvedDependency {
            location: SourceLocation::from_bytes(
                PathBuf::from(self.input.file()),
                self.input.source_bytes(),
                self.input.original_range(node.range()),
                0,
            ),
            message: message.into(),
        });
    }

    fn walk(
        &mut self,
        node: &SyntaxNode,
        shape: Option<&Shape>,
        depth: usize,
    ) -> Result<Vec<TextEdit>, QueryError> {
        if depth >= self.limits.nesting.min(64) {
            return Err(QueryError::query(
                self.range.clone(),
                "reduction nesting limit exceeded",
            ));
        }
        self.work.visit(self.range)?;
        crate::inspection::check_collection(self.visited, self.limits.collection, self.range)?;
        self.visited += 1;
        let node = structured::unwrapped(node);
        self.work.spend(node.children().len(), self.range)?;
        if node.child_nodes().count() > self.limits.collection {
            return Err(QueryError::query(
                self.range.clone(),
                "query collection limit exceeded",
            ));
        }
        if matches!(
            node.kind(),
            NodeKind::ArrayLiteral | NodeKind::MatrixLiteral
        ) && shape
            .is_some_and(|shape| !shape.contains_enum(self.target) && type_known(&shape.ty))
            && !self.references(node, true)?
        {
            return Ok(Vec::new());
        }
        match node.kind() {
            NodeKind::ArrayLiteral => self.array(node, shape, depth),
            NodeKind::MatrixLiteral => self.matrix(node, shape, depth),
            NodeKind::RecordLiteral => {
                let mut edits = Vec::new();
                for field in node.child_nodes() {
                    if let Some(shape) = shape {
                        self.work.spend(shape.components.len(), self.range)?;
                    }
                    let index = structured::direct_token(self.input, field).unwrap();
                    let label = identifier_identity(
                        std::str::from_utf8(self.input.token_bytes(index)).unwrap(),
                    );
                    let field_shape = shape
                        .and_then(|shape| {
                            shape
                                .components
                                .iter()
                                .find(|(name, _)| name.as_deref() == Some(label))
                        })
                        .map(|(_, shape)| shape);
                    edits.extend(self.walk(
                        field.child_nodes().next().unwrap(),
                        field_shape,
                        depth + 1,
                    )?);
                }
                Ok(edits)
            }
            NodeKind::TupleLiteral => {
                let mut edits = Vec::new();
                for (index, child) in node.child_nodes().enumerate() {
                    edits.extend(
                        self.walk(
                            child,
                            shape
                                .and_then(|shape| shape.components.get(index))
                                .map(|(_, shape)| shape),
                            depth + 1,
                        )?,
                    );
                }
                Ok(edits)
            }
            NodeKind::SetLiteral => {
                if self.references_removed(node)? {
                    self.unresolved(node, "removed members remain in a set; set cleanup and group compaction are deferred");
                } else if node.child_nodes().any(|child| {
                    !matches!(
                        structured::unwrapped(child).kind(),
                        NodeKind::Expression | NodeKind::UnaryExpression
                    )
                }) && self.relevant_computed(node, shape)?
                {
                    self.unresolved(node, "computed set dependence is unsupported");
                }
                Ok(Vec::new())
            }
            NodeKind::Expression
                if member(self.input, node).is_some_and(|name| self.removed.contains(name)) =>
            {
                self.unresolved(node, "removed enum member remains as a scalar value");
                Ok(Vec::new())
            }
            NodeKind::Expression => Ok(Vec::new()),
            _ => {
                if self.relevant_computed(node, shape)? {
                    self.unresolved(
                        node,
                        "computed enum value, access or index dependence is unsupported",
                    );
                }
                Ok(Vec::new())
            }
        }
    }

    fn references_removed(&mut self, node: &SyntaxNode) -> Result<bool, QueryError> {
        self.references(node, false)
    }

    fn references(&mut self, node: &SyntaxNode, dependent: bool) -> Result<bool, QueryError> {
        let mut pending = vec![node];
        while let Some(node) = pending.pop() {
            self.work.spend(node.children().len(), self.range)?;
            for child in node.children().iter().rev() {
                match child {
                    zincite_syntax::SyntaxElement::Node(child) => pending.push(child),
                    zincite_syntax::SyntaxElement::Token(index) => {
                        if matches!(
                            node.kind(),
                            NodeKind::RecordLiteralField | NodeKind::FieldAccessExpression
                        ) {
                            continue;
                        }
                        let token = &self.input.syntax().tokens()[*index];
                        if !matches!(
                            token.kind,
                            TokenKind::Identifier | TokenKind::QuotedIdentifier
                        ) {
                            continue;
                        }
                        let identity =
                            identifier_identity(&self.input.syntax().source()[token.range.clone()]);
                        if self.removed.contains(identity)
                            || (dependent
                                && (identity == self.name
                                    || self.dependent_names.contains(identity)))
                        {
                            return Ok(true);
                        }
                    }
                }
            }
        }
        Ok(false)
    }

    fn relevant_computed(
        &mut self,
        node: &SyntaxNode,
        shape: Option<&Shape>,
    ) -> Result<bool, QueryError> {
        Ok(shape.is_some_and(|shape| shape.contains_enum(self.target))
            || self.references(node, true)?)
    }

    fn keys(
        &mut self,
        nodes: &[&SyntaxNode],
        owner: &SyntaxNode,
    ) -> Result<Option<Vec<Vec<Key>>>, QueryError> {
        let mut keys = Vec::new();
        for node in nodes {
            match structured::read_key(self.input, node, self.work, self.range, self.limits) {
                Ok(key) => keys.push(key),
                Err(error) if error.location == ErrorLocation::Input => {
                    self.unresolved(
                        node,
                        "computed or unsupported array index cannot be aligned",
                    );
                    return Ok(None);
                }
                Err(error) => return Err(error),
            }
        }
        if let Err(error) = structured::validate_keys(
            &keys,
            &self.input.original_range(owner.range()),
            self.work,
            self.range,
        ) {
            if error.location != ErrorLocation::Input {
                return Err(error);
            }
            self.unresolved(owner, error.message);
            return Ok(None);
        }
        Ok(Some(keys))
    }

    fn full_target_domain(&self, domain: &Domain) -> bool {
        match domain {
            Domain::Enum(id) => Some(*id) == self.target,
            Domain::Named {
                declaration,
                domain,
            } if self.full_domain_names.contains(&declaration.0) => self.full_target_domain(domain),
            _ => false,
        }
    }

    fn mask(
        &mut self,
        owner: &SyntaxNode,
        keys: Option<&[Vec<Key>]>,
        count: usize,
        axis: usize,
        shape: Option<&Shape>,
    ) -> Option<Vec<bool>> {
        let index_type = shape.and_then(|shape| match &shape.ty.kind {
            TypeKind::Array { indices, .. } => indices.get(axis),
            _ => None,
        });
        let target_axis = index_type
            .is_some_and(|ty| matches!(ty.kind, TypeKind::Enum(id) if Some(id) == self.target));
        if let Some(keys) = keys {
            if target_axis
                && !shape
                    .and_then(|shape| shape.indices.get(axis))
                    .is_some_and(|domain| self.full_target_domain(domain))
            {
                self.unresolved(owner, "enum index subset coverage is unavailable; explicit keys are reduced without assuming full-domain alignment");
            }
            if keys.iter().any(|key| key.len() <= axis) {
                self.unresolved(
                    owner,
                    "array key arity does not match its declared dimensions",
                );
                return None;
            }
            let identities: HashSet<_> = keys.iter().map(|key| &key[axis]).collect();
            if target_axis
                && shape
                    .and_then(|shape| shape.indices.get(axis))
                    .is_some_and(|domain| self.full_target_domain(domain))
                && (identities.len() != self.members.len()
                    || self
                        .members
                        .iter()
                        .any(|member| !identities.contains(&Key::Member((*member).into()))))
            {
                self.unresolved(
                    owner,
                    "enum-indexed keys do not cover the original enum membership",
                );
                return None;
            }
            return Some(keys.iter().map(|key| {
                let established = match index_type.map(|ty| &ty.kind) {
                    Some(TypeKind::Enum(id)) => Some(*id) == self.target,
                    Some(TypeKind::Int | TypeKind::Bool | TypeKind::Float | TypeKind::String) => false,
                    _ => true,
                };
                !(established && matches!(&key[axis], Key::Member(name) if self.removed.contains(name.as_str())))
            }).collect());
        }
        if target_axis {
            if count != self.members.len()
                || !shape
                    .and_then(|shape| shape.indices.get(axis))
                    .is_some_and(|domain| self.full_target_domain(domain))
            {
                self.unresolved(owner, "positional enum alignment requires the full original enum domain and exact coverage");
                return None;
            }
            return Some(
                self.members
                    .iter()
                    .map(|member| !self.removed.contains(member))
                    .collect(),
            );
        }
        if index_type.is_none()
            || index_type.is_some_and(|ty| {
                matches!(ty.kind, TypeKind::Unknown(_) | TypeKind::Variable { .. })
            })
        {
            self.unresolved(owner, "positional array alignment is unavailable; supply an unambiguous model declaration");
            return None;
        }
        Some(vec![true; count])
    }

    fn array(
        &mut self,
        node: &SyntaxNode,
        shape: Option<&Shape>,
        depth: usize,
    ) -> Result<Vec<TextEdit>, QueryError> {
        let entries: Vec<_> = node.child_nodes().collect();
        let indexing = structured::array_indexing(node);
        if indexing == ArrayIndexing::StartingKey {
            self.unresolved(node, "starting-key array alignment is unsupported");
            return Ok(Vec::new());
        }
        let keys = if indexing == ArrayIndexing::Explicit {
            let nodes: Vec<_> = entries
                .iter()
                .map(|entry| entry.child_nodes().next().unwrap())
                .collect();
            let Some(keys) = self.keys(&nodes, node)? else {
                return Ok(Vec::new());
            };
            Some(keys)
        } else {
            None
        };
        let arity = keys
            .as_ref()
            .and_then(|keys| keys.first())
            .map_or(1, Vec::len);
        if let Some(Shape {
            ty:
                TypeInst {
                    kind: TypeKind::Array { indices, .. },
                    ..
                },
            ..
        }) = shape
            && indices.len() != arity
        {
            self.unresolved(
                node,
                "array literal dimensions do not match the model declaration",
            );
            return Ok(Vec::new());
        }
        let member_identities: HashSet<_> = self.members.iter().copied().collect();
        self.work.spend(
            entries
                .len()
                .saturating_mul(arity)
                .saturating_add(self.members.len().saturating_mul(arity)),
            self.range,
        )?;
        if self.model_supplied
            && shape.is_none_or(|shape| match &shape.ty.kind {
                TypeKind::Array { indices, .. } => indices.iter().any(|ty| !type_known(ty)),
                _ => true,
            })
            && keys.as_ref().is_some_and(|keys| {
                keys.iter().flatten().any(
                |key| matches!(key, Key::Member(name) if member_identities.contains(name.as_str())),
            )
            })
        {
            self.unresolved(node, "model assignment/type association is unavailable or ambiguous for enum-indexed data");
        }
        let mut retained = vec![true; entries.len()];
        for axis in 0..arity {
            let Some(mask) = self.mask(node, keys.as_deref(), entries.len(), axis, shape) else {
                return Ok(Vec::new());
            };
            for (keep, mask) in retained.iter_mut().zip(mask) {
                *keep &= mask;
            }
        }
        if arity > 1 && !retained.iter().any(|keep| *keep) && !entries.is_empty() {
            self.unresolved(
                node,
                "empty tuple-key data cannot preserve the established multidimensional axes",
            );
            return Ok(Vec::new());
        }
        if let Some(keys) = &keys {
            let kept: Vec<_> = keys
                .iter()
                .zip(&retained)
                .filter_map(|(key, keep)| keep.then_some(key.clone()))
                .collect();
            if let Err(error) = structured::validate_keys(
                &kept,
                &self.input.original_range(node.range()),
                self.work,
                self.range,
            ) {
                if error.location != ErrorLocation::Input {
                    return Err(error);
                }
                self.unresolved(node, error.message);
                return Ok(Vec::new());
            }
        }
        let element = shape
            .and_then(|shape| shape.components.first())
            .map(|(_, shape)| shape);
        let mut children = Vec::new();
        for (entry, keep) in entries.iter().zip(&retained) {
            if *keep {
                let value = if indexing == ArrayIndexing::Explicit {
                    entry.child_nodes().nth(1).unwrap()
                } else {
                    entry
                };
                children.extend(self.walk(value, element, depth + 1)?);
            }
        }
        let parent = structured::collection_edit(
            self.input,
            node,
            &entries,
            &retained,
            TokenKind::Comma,
            self.work,
            self.range,
        )?;
        Ok(vec![compose(parent, children)])
    }

    fn matrix(
        &mut self,
        node: &SyntaxNode,
        shape: Option<&Shape>,
        depth: usize,
    ) -> Result<Vec<TextEdit>, QueryError> {
        let header = node
            .child_nodes()
            .find(|node| node.kind() == NodeKind::MatrixColumnIndices);
        let columns: Vec<_> = header
            .into_iter()
            .flat_map(SyntaxNode::child_nodes)
            .collect();
        let rows: Vec<_> = node
            .child_nodes()
            .filter(|node| node.kind() == NodeKind::MatrixRow)
            .collect();
        let keyed = rows
            .first()
            .is_some_and(|row| structured::has_colon(self.input, row));
        let width = rows.first().map_or(columns.len(), |row| {
            row.child_nodes().count() - usize::from(keyed)
        });
        if rows.iter().any(|row| {
            structured::has_colon(self.input, row) != keyed
                || row.child_nodes().count() - usize::from(keyed) != width
        }) || (!columns.is_empty() && columns.len() != width)
        {
            self.unresolved(
                node,
                "matrix rows and columns do not form a rectangular shape",
            );
            return Ok(Vec::new());
        }
        if let Some(Shape {
            ty:
                TypeInst {
                    kind: TypeKind::Array { indices, .. },
                    ..
                },
            ..
        }) = shape
            && indices.len() != 2
        {
            self.unresolved(node, "matrix requires a two-dimensional declaration");
            return Ok(Vec::new());
        }
        let row_keys = if keyed {
            let keys: Vec<_> = rows
                .iter()
                .map(|row| row.child_nodes().next().unwrap())
                .collect();
            let Some(keys) = self.keys(&keys, node)? else {
                return Ok(Vec::new());
            };
            Some(keys)
        } else {
            None
        };
        let column_keys = if columns.is_empty() {
            None
        } else {
            let Some(keys) = self.keys(&columns, node)? else {
                return Ok(Vec::new());
            };
            Some(keys)
        };
        if row_keys
            .iter()
            .chain(column_keys.iter())
            .flatten()
            .any(|key| key.len() != 1)
        {
            self.unresolved(node, "matrix axis keys must be scalar identities");
            return Ok(Vec::new());
        }
        self.work.spend(
            rows.len()
                .saturating_add(width)
                .saturating_add(self.members.len().saturating_mul(2)),
            self.range,
        )?;
        if self.model_supplied && shape.is_none() {
            self.unresolved(
                node,
                "model assignment/type association is unavailable or ambiguous for matrix data",
            );
        }
        // Each written matrix axis is scalar; mask receives its own component.
        let Some(row_mask) = self.mask(node, row_keys.as_deref(), rows.len(), 0, shape) else {
            return Ok(Vec::new());
        };
        let adjusted_columns = column_keys.as_ref().map(|keys| {
            keys.iter()
                .map(|key| vec![Key::Integer(0), key[0].clone()])
                .collect::<Vec<_>>()
        });
        let Some(column_mask) = self.mask(node, adjusted_columns.as_deref(), width, 1, shape)
        else {
            return Ok(Vec::new());
        };
        if !row_mask.iter().any(|keep| *keep) || !column_mask.iter().any(|keep| *keep) {
            self.unresolved(
                node,
                "empty matrix data cannot preserve the established row and column axes",
            );
            return Ok(Vec::new());
        }
        let element = shape
            .and_then(|shape| shape.components.first())
            .map(|(_, shape)| shape);
        let mut children = Vec::new();
        if let Some(header) = header {
            children.push(structured::collection_edit(
                self.input,
                header,
                &columns,
                &column_mask,
                TokenKind::Colon,
                self.work,
                self.range,
            )?);
        }
        for (row, keep) in rows.iter().zip(&row_mask) {
            if !keep {
                continue;
            }
            let cells: Vec<_> = row.child_nodes().skip(usize::from(keyed)).collect();
            let mut cell_edits = Vec::new();
            for (cell, keep) in cells.iter().zip(&column_mask) {
                if *keep {
                    cell_edits.extend(self.walk(cell, element, depth + 1)?);
                }
            }
            children.push(compose(
                structured::collection_edit(
                    self.input,
                    row,
                    &cells,
                    &column_mask,
                    TokenKind::Comma,
                    self.work,
                    self.range,
                )?,
                cell_edits,
            ));
        }
        Ok(vec![compose(
            structured::collection_edit(
                self.input,
                node,
                &rows,
                &row_mask,
                TokenKind::Pipe,
                self.work,
                self.range,
            )?,
            children,
        )])
    }
}

// Splice child edits into the parent's retained original spans. Removed entries
// are never walked, and the final batch contains only disjoint outer edits.
fn compose(mut parent: TextEdit, mut children: Vec<TextEdit>) -> TextEdit {
    children.sort_by_key(|edit| edit.range.start);
    let mut replacement = Vec::new();
    for part in parent.replacement {
        let EditPart::Original(span) = part else {
            replacement.push(part);
            continue;
        };
        let mut cursor = span.start;
        for child in &children {
            if child.range.start < span.start || child.range.end > span.end {
                continue;
            }
            if cursor < child.range.start {
                replacement.push(EditPart::Original(cursor..child.range.start));
            }
            replacement.extend(child.replacement.clone());
            cursor = child.range.end;
        }
        if cursor < span.end {
            replacement.push(EditPart::Original(cursor..span.end));
        }
    }
    parent.replacement = replacement;
    parent
}

fn type_known(ty: &TypeInst) -> bool {
    match &ty.kind {
        TypeKind::Unknown(_) | TypeKind::Variable { .. } => false,
        TypeKind::Array { indices, element } => {
            indices.iter().all(type_known) && type_known(element)
        }
        TypeKind::Tuple(fields) => fields.iter().all(type_known),
        TypeKind::Record(fields) => fields.iter().all(|(_, field)| type_known(field)),
        TypeKind::Set(element) => type_known(element),
        _ => true,
    }
}
