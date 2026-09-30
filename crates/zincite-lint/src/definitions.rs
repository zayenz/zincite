//! Definition candidates and dependencies, independent of diagnostic policy.
use crate::callables::{find_node, is_expression};
use crate::domains::{expression_domain, expression_integer, same_members};
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    Domain, DomainFacts, FileId, Instantiation, InstantiationFacts, ModelContext, ReferenceKind,
    SourceKind, SourceLocation, TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionCoverage {
    Scalar,
    WholeArray,
    ArrayElement,
    /// Ordinary uncertainty, including different symbolic index domains.
    Unproved(String),
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionSafety {
    Supported,
    Unknown(String),
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionEnforcement {
    Enforced,
    Conditional,
    Unsupported(String),
}

#[derive(Clone, Debug)]
pub struct Definition {
    pub target: DeclarationId,
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub value: SourceLocation,
    pub instantiation: Instantiation,
    /// Direct resolved value references, without arbitrary call-body expansion.
    pub dependencies: Vec<DeclarationId>,
    pub enforcement: DefinitionEnforcement,
    pub coverage: DefinitionCoverage,
    pub safety: DefinitionSafety,
    pub cyclic: bool,
}

#[derive(Debug, Default)]
pub struct DefinitionFacts {
    pub definitions: Vec<Definition>,
}

impl DefinitionFacts {
    /// Values anchored by parameters, explicit numeric domains or complete
    /// supported enforced definitions. Facts must share the same ModelContext.
    /// Cyclic equality directions need already anchored dependencies; an
    /// unanchored or self cycle never establishes its own complete definition.
    pub fn bounded_or_defined_targets(
        &self,
        bindings: &BindingFacts,
        domains: &DomainFacts,
    ) -> Vec<DeclarationId> {
        let mut known: Vec<_> = bindings
            .declarations
            .iter()
            .filter(|d| {
                d.instantiation == Instantiation::Parameter
                    || domains.declarations[d.id.0]
                        .domain
                        .has_explicit_numeric_domain()
            })
            .map(|d| d.id)
            .collect();
        loop {
            let previous = known.len();
            for definition in &self.definitions {
                if known.contains(&definition.target)
                    || definition.enforcement != DefinitionEnforcement::Enforced
                    || !matches!(
                        definition.coverage,
                        DefinitionCoverage::Scalar | DefinitionCoverage::WholeArray
                    )
                    || definition.safety != DefinitionSafety::Supported
                {
                    continue;
                }
                if !definition.cyclic || definition.dependencies.iter().all(|id| known.contains(id))
                {
                    known.push(definition.target);
                }
            }
            if known.len() == previous {
                return known;
            }
        }
    }
}

/// Retain initializer/equality candidates without enabling lint or evaluating
/// data. Prerequisite facts must belong to this ModelContext. Enforcement admits
/// core conjunction/forall and exactly resolved Boolean forwarding bodies only.
/// Whole-array proof needs every declared index, without filters or extra binders.
/// Optional/partial/opaque values remain explicit unsupported safety facts.
pub fn resolve_definitions(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
) -> DefinitionFacts {
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        definitions: Vec::new(),
        active_forwarding: Vec::new(),
    };
    for declaration in &bindings.declarations {
        if !matches!(
            declaration.role,
            DeclarationRole::Value | DeclarationRole::Local
        ) {
            continue;
        }
        let source = &context.files[declaration.file];
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        let Some(node) = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) else {
            continue;
        };
        if let Some(value) = node.child_nodes().find(|n| is_expression(n.kind())) {
            let coverage = if matches!(
                calls.declarations[declaration.id.0].ty.kind,
                TypeKind::Array { .. }
            ) {
                DefinitionCoverage::WholeArray
            } else {
                DefinitionCoverage::Scalar
            };
            producer.record(
                declaration.file,
                declaration.item,
                node,
                value,
                DefinitionEnforcement::Enforced,
                (declaration.id, coverage),
            );
        }
    }
    for (file, source) in context.files.iter().enumerate() {
        if !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source.parsed.tree().child_nodes().enumerate() {
            let enforcement = if node.kind() == NodeKind::Constraint {
                DefinitionEnforcement::Enforced
            } else {
                DefinitionEnforcement::Conditional
            };
            producer.walk(file, item, node, enforcement, &[]);
        }
    }
    let edges: Vec<_> = producer
        .definitions
        .iter()
        .filter(|d| d.enforcement == DefinitionEnforcement::Enforced)
        .map(|d| (d.target, d.dependencies.clone()))
        .collect();
    for definition in &mut producer.definitions {
        definition.cyclic = definition
            .dependencies
            .iter()
            .any(|id| reaches(*id, definition.target, &edges, &mut Vec::new()));
    }
    DefinitionFacts {
        definitions: producer.definitions,
    }
}

fn reaches(
    id: DeclarationId,
    target: DeclarationId,
    edges: &[(DeclarationId, Vec<DeclarationId>)],
    active: &mut Vec<DeclarationId>,
) -> bool {
    if id == target {
        return true;
    }
    if active.contains(&id) {
        return false;
    }
    active.push(id);
    let found = edges
        .iter()
        .filter(|(from, _)| *from == id)
        .any(|(_, dependencies)| {
            dependencies
                .iter()
                .any(|next| reaches(*next, target, edges, active))
        });
    active.pop();
    found
}

struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    definitions: Vec<Definition>,
    active_forwarding: Vec<(FileId, usize)>,
}
impl<'context> Producer<'context> {
    fn tokens(&self, file: FileId, node: &SyntaxNode) -> Vec<&zincite_syntax::Token> {
        node.children()
            .iter()
            .filter_map(|child| match child {
                SyntaxElement::Token(index) => {
                    let token = &self.context.files[file].parsed.tokens()[*index];
                    (!matches!(
                        token.kind,
                        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                    ))
                    .then_some(token)
                }
                _ => None,
            })
            .collect()
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let start =
            self.tokens(file, node).first()?.range.start + self.context.files[file].byte_offset;
        self.bindings
            .references
            .iter()
            .find(|r| r.file == file && r.location.range.start == start)
            .and_then(|r| match r.resolution {
                BindingResolution::Resolved(id) => Some(id),
                _ => None,
            })
    }
    fn ty(&self, file: FileId, node: &SyntaxNode) -> Option<&TypeInst> {
        let location = self.context.files[file].location(node.range());
        self.calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map(|e| &e.ty)
    }
    fn instantiation(&self, file: FileId, node: &SyntaxNode) -> Instantiation {
        let location = self.context.files[file].location(node.range());
        self.instantiations
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map_or(Instantiation::Unknown, |e| e.instantiation)
    }
    fn annotations_safe(&self, file: FileId, node: &SyntaxNode) -> bool {
        node.child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
            .all(|annotation| {
                annotation.child_nodes().next().is_some_and(|value| {
                    value.kind() == NodeKind::Expression
                        && self
                            .tokens(file, value)
                            .first()
                            .is_some_and(|t| t.kind == TokenKind::StringLiteral)
                })
            })
    }
    fn call(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let head = self.tokens(file, node).into_iter().find(|t| {
            matches!(
                t.kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
            )
        })?;
        let start = head.range.start + self.context.files[file].byte_offset;
        self.calls
            .calls
            .iter()
            .find(|c| c.file == file && c.location.range.start == start)
            .and_then(|c| match c.outcome {
                CallOutcome::Resolved { declaration, .. } => Some(declaration),
                _ => None,
            })
    }
    fn core_forall(&self, id: DeclarationId) -> bool {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        declaration.name == "forall"
            && declaration.role == DeclarationRole::Function
            && source.kind == SourceKind::StandardLibrary
            && source.implicit
    }
    fn forwarded_argument<'node>(
        &self,
        file: FileId,
        item: usize,
        call: &'node SyntaxNode,
        id: DeclarationId,
    ) -> Option<(FileId, usize, &'node SyntaxNode)>
    where
        'context: 'node,
    {
        let declaration = &self.bindings.declarations[id.0];
        let source: &'context crate::ModelFile = &self.context.files[declaration.file];
        let node = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        // Compiler-switchable library wrappers can have identity bodies. Accept
        // ordinary user forwarding bodies without declaration annotations only.
        if source.kind == SourceKind::StandardLibrary
            || node.child_nodes().any(|n| n.kind() == NodeKind::Annotation)
        {
            return None;
        }
        let signature = self.calls.signatures.iter().find(|s| s.declaration == id)?;
        if signature.return_type.optional || signature.return_type.kind != TypeKind::Bool {
            return None;
        }
        let body = node
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()?;
        let body = unwrap_parentheses(body);
        if body.kind() != NodeKind::Expression {
            return None;
        }
        let formal = self.reference(declaration.file, body)?;
        let formal = &self.bindings.declarations[formal.0];
        if formal.role != DeclarationRole::Parameter
            || formal.file != declaration.file
            || formal.syntax_range.start < node.range().start
            || formal.syntax_range.end > node.range().end
        {
            return None;
        }
        let position = signature
            .parameters
            .iter()
            .position(|p| p.name.as_deref() == Some(formal.name.as_str()))?;
        let mut positional = 0;
        for argument in call.child_nodes() {
            if argument.kind() == NodeKind::NamedArgument {
                let token = self.tokens(file, argument).first().copied()?;
                let name = source_name(&self.context.files[file].parsed, token);
                if name == formal.name {
                    return argument
                        .child_nodes()
                        .next()
                        .map(|value| (file, item, value));
                }
            } else {
                if positional == position {
                    return Some((file, item, argument));
                }
                positional += 1;
            }
        }
        let parameter = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::ParameterList)?
            .child_nodes()
            .nth(position)?;
        let value = parameter.child_nodes().find(|n| is_expression(n.kind()))?;
        Some((declaration.file, declaration.item, value))
    }
    fn walk(
        &mut self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        enforcement: DefinitionEnforcement,
        generators: &[&SyntaxNode],
    ) {
        use DefinitionEnforcement::*;
        let children: Vec<_> = node.child_nodes().collect();
        let enforcement = if enforcement == Enforced && !self.annotations_safe(file, node) {
            Unsupported("constraint annotation enforcement is unsupported".into())
        } else {
            enforcement
        };
        if node.kind() == NodeKind::BinaryExpression {
            let operator = self.tokens(file, node).first().map(|t| t.kind);
            if matches!(operator, Some(TokenKind::Equal | TokenKind::DoubleEqual))
                && children.len() == 2
            {
                for (target, value) in [(children[0], children[1]), (children[1], children[0])] {
                    if let Some(candidate) = self.target(file, target, generators) {
                        // A scalar equation in a possibly empty forall need not
                        // hold. Complete matching array coverage is different:
                        // an empty index set defines an empty whole array.
                        let nonempty = generators.iter().all(|generator| {
                            !generator
                                .child_nodes()
                                .any(|n| n.kind() == NodeKind::WhereFilter)
                                && generator.child_nodes().next().is_some_and(|source| {
                                    expression_domain(self.context, self.bindings, file, source)
                                        .numeric_minimum()
                                        .is_ok_and(|minimum| minimum.is_some())
                                })
                        });
                        let own = if target.kind() != NodeKind::ArrayAccessExpression && !nonempty {
                            Conditional
                        } else {
                            enforcement.clone()
                        };
                        self.record(file, item, node, value, own, candidate);
                    }
                }
            }
            if operator != Some(TokenKind::And) {
                for child in children {
                    self.walk(file, item, child, Conditional, generators);
                }
                return;
            }
        }
        if matches!(
            node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) {
            if let Some(id) = self.call(file, node) {
                if self.core_forall(id) {
                    let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                        Some(node)
                    } else {
                        children
                            .first()
                            .copied()
                            .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                    };
                    if let Some(quantified) = quantified {
                        let mut all = generators.to_vec();
                        if let Some(list) = quantified
                            .child_nodes()
                            .find(|n| n.kind() == NodeKind::GeneratorList)
                        {
                            all.extend(list.child_nodes());
                        }
                        if let Some(body) = quantified
                            .child_nodes()
                            .find(|n| n.kind() != NodeKind::GeneratorList)
                        {
                            self.walk(file, item, body, enforcement, &all);
                            return;
                        }
                    }
                }
                if let Some((argument_file, argument_item, argument)) =
                    self.forwarded_argument(file, item, node, id)
                {
                    let key = (file, node.range().start);
                    if self.active_forwarding.contains(&key) {
                        return;
                    }
                    self.active_forwarding.push(key);
                    let own = if argument_file != file && !generators.is_empty() {
                        Unsupported(
                            "cross-file forwarded default under quantification is unsupported"
                                .into(),
                        )
                    } else {
                        enforcement
                    };
                    self.walk(
                        argument_file,
                        argument_item,
                        argument,
                        own,
                        if argument_file == file {
                            generators
                        } else {
                            &[]
                        },
                    );
                    self.active_forwarding.pop();
                    for child in children {
                        let actual = if child.kind() == NodeKind::NamedArgument {
                            child.child_nodes().next().unwrap_or(child)
                        } else {
                            child
                        };
                        if argument_file != file || actual.range() != argument.range() {
                            self.walk(file, item, actual, Conditional, generators);
                        }
                    }
                    return;
                }
            }
            // An unresolved selection cannot prove a transparent/core call.
            // Retain that missing boundary rather than claiming a completed
            // negative for an equality in its actual arguments.
            let own = if enforcement == Enforced && self.call(file, node).is_none() {
                Unsupported("constraint callable selection is unavailable".into())
            } else {
                Conditional
            };
            for child in children {
                self.walk(file, item, child, own.clone(), generators);
            }
            return;
        }
        let admitted = matches!(
            node.kind(),
            NodeKind::Constraint
                | NodeKind::ParenthesizedExpression
                | NodeKind::AnnotatedExpression
                | NodeKind::BinaryExpression
                | NodeKind::NamedArgument
        );
        for child in children {
            self.walk(
                file,
                item,
                child,
                if admitted {
                    enforcement.clone()
                } else {
                    Conditional
                },
                generators,
            );
        }
    }
    fn target(
        &self,
        file: FileId,
        node: &SyntaxNode,
        generators: &[&SyntaxNode],
    ) -> Option<(DeclarationId, DefinitionCoverage)> {
        let node = unwrap_parentheses(node);
        let (subject, indices) = if node.kind() == NodeKind::ArrayAccessExpression {
            let children: Vec<_> = node.child_nodes().collect();
            (*children.first()?, children[1..].to_vec())
        } else if node.kind() == NodeKind::Expression {
            (node, Vec::new())
        } else {
            return None;
        };
        let id = self.reference(file, subject)?;
        if !matches!(
            self.bindings.declarations[id.0].role,
            DeclarationRole::Value | DeclarationRole::Local
        ) {
            return None;
        }
        let ty = &self.calls.declarations[id.0].ty;
        if ty.instantiation != Instantiation::Decision {
            return None;
        }
        let coverage = if let TypeKind::Array { .. } = ty.kind {
            if indices.is_empty() {
                DefinitionCoverage::WholeArray
            } else {
                self.array_coverage(file, id, &indices, generators)
            }
        } else if indices.is_empty() {
            DefinitionCoverage::Scalar
        } else {
            return None;
        };
        Some((id, coverage))
    }
    fn array_coverage(
        &self,
        file: FileId,
        id: DeclarationId,
        indices: &[&SyntaxNode],
        generators: &[&SyntaxNode],
    ) -> DefinitionCoverage {
        let mut domain = &self.domains.declarations[id.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let Domain::Array {
            indices: declared, ..
        } = domain
        else {
            return DefinitionCoverage::Unsupported("array index domains are unavailable".into());
        };
        let mut bindings = Vec::new();
        for generator in generators {
            if generator
                .child_nodes()
                .any(|n| n.kind() == NodeKind::WhereFilter)
                || !self
                    .tokens(file, generator)
                    .iter()
                    .any(|t| t.kind == TokenKind::In)
            {
                return DefinitionCoverage::ArrayElement;
            }
            let Some(source) = generator.child_nodes().next() else {
                return DefinitionCoverage::ArrayElement;
            };
            if self.instantiation(file, source) != Instantiation::Parameter {
                return DefinitionCoverage::ArrayElement;
            }
            let domain = expression_domain(self.context, self.bindings, file, source);
            for declaration in self.bindings.declarations.iter().filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == generator.range()
            }) {
                bindings.push((declaration.id, domain.clone()));
            }
        }
        if declared.len() != indices.len() || bindings.len() != indices.len() {
            return DefinitionCoverage::ArrayElement;
        }
        let mut used = Vec::new();
        for (index, expected) in indices.iter().zip(declared) {
            let Some(id) = self.reference(file, unwrap_parentheses(index)) else {
                return DefinitionCoverage::ArrayElement;
            };
            if used.contains(&id) {
                return DefinitionCoverage::ArrayElement;
            }
            let Some((_, actual)) = bindings.iter().find(|(binder, _)| *binder == id) else {
                return DefinitionCoverage::ArrayElement;
            };
            used.push(id);
            match same_members(actual, expected) {
                Ok(Some(true)) => {}
                Ok(Some(false)) => return DefinitionCoverage::ArrayElement,
                Ok(None) => {
                    return DefinitionCoverage::Unproved(
                        "array index domains are not proved equal".into(),
                    );
                }
                Err(reason) => return DefinitionCoverage::Unsupported(reason),
            }
        }
        DefinitionCoverage::WholeArray
    }
    fn value_safety(&self, file: FileId, node: &SyntaxNode) -> DefinitionSafety {
        let Some(ty) = self.ty(file, node) else {
            return DefinitionSafety::Unsupported("value type is unavailable".into());
        };
        if optional(ty) {
            return DefinitionSafety::Unsupported(
                "optional definition values are not supported".into(),
            );
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => {
                if matches!(ty.kind, TypeKind::Unknown(_)) {
                    DefinitionSafety::Unsupported("definition value type is unsupported".into())
                } else {
                    DefinitionSafety::Supported
                }
            }
            NodeKind::MatrixLiteral => {
                for value in children.iter().flat_map(|row| row.child_nodes()) {
                    let safety = self.value_safety(file, value);
                    if safety != DefinitionSafety::Supported {
                        return safety;
                    }
                }
                DefinitionSafety::Supported
            }
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                DefinitionSafety::Unsupported(
                    "arbitrary calls do not establish definition value safety".into(),
                )
            }
            NodeKind::ArrayAccessExpression => DefinitionSafety::Unsupported(
                "definition array access requires an index-membership proof".into(),
            ),
            NodeKind::BinaryExpression
                if self
                    .tokens(file, node)
                    .iter()
                    .any(|t| matches!(t.kind, TokenKind::Div | TokenKind::Mod)) =>
            {
                match expression_integer(self.context, self.bindings, file, node) {
                    Ok(Some(_)) => DefinitionSafety::Supported,
                    Ok(None) => {
                        DefinitionSafety::Unknown("divisor/value needs parameter data".into())
                    }
                    Err(reason) => DefinitionSafety::Unsupported(reason),
                }
            }
            NodeKind::BinaryExpression
                if !self.tokens(file, node).iter().any(|t| {
                    matches!(
                        t.kind,
                        TokenKind::Plus
                            | TokenKind::Minus
                            | TokenKind::Star
                            | TokenKind::And
                            | TokenKind::Or
                            | TokenKind::Equal
                            | TokenKind::DoubleEqual
                            | TokenKind::Less
                            | TokenKind::LessEqual
                            | TokenKind::Greater
                            | TokenKind::GreaterEqual
                            | TokenKind::NotEqual
                    )
                }) =>
            {
                DefinitionSafety::Unsupported(
                    "definition operator partiality is unsupported".into(),
                )
            }
            NodeKind::LetExpression
            | NodeKind::ConditionalExpression
            | NodeKind::ArrayComprehension
            | NodeKind::SetComprehension
            | NodeKind::IndexedArrayComprehension => DefinitionSafety::Unsupported(
                "definition value control/iteration safety is unsupported".into(),
            ),
            _ => {
                for child in children {
                    let status = self.value_safety(file, child);
                    if status != DefinitionSafety::Supported {
                        return status;
                    }
                }
                DefinitionSafety::Supported
            }
        }
    }
    fn record(
        &mut self,
        file: FileId,
        item: usize,
        node: &SyntaxNode,
        value: &SyntaxNode,
        enforcement: DefinitionEnforcement,
        candidate: (DeclarationId, DefinitionCoverage),
    ) {
        let (target, coverage) = candidate;
        let location = self.context.files[file].location(node.range());
        let value_location = self.context.files[file].location(value.range());
        if let Some(index) = self.definitions.iter().position(|d| {
            d.target == target
                && d.location.path == location.path
                && d.location.range == location.range
                && d.value.range == value_location.range
        }) {
            // A default expression can first be seen as a declaration value,
            // then as the actual forwarded argument of an enforced call.
            if self.definitions[index].enforcement == DefinitionEnforcement::Enforced
                || enforcement == DefinitionEnforcement::Conditional
            {
                return;
            }
            self.definitions.remove(index);
        }
        let mut dependencies = Vec::new();
        for reference in &self.bindings.references {
            if reference.file == file
                && reference.kind == ReferenceKind::Value
                && value_location.range.start <= reference.location.range.start
                && reference.location.range.end <= value_location.range.end
                && let BindingResolution::Resolved(id) = reference.resolution
                && !dependencies.contains(&id)
            {
                dependencies.push(id);
            }
        }
        let instantiation = self.instantiation(file, value);
        let mut safety = self.value_safety(file, value);
        if !self.calls.declarations[target.0].ty.known() {
            safety = DefinitionSafety::Unsupported("definition target type is unsupported".into());
        } else if optional(&self.calls.declarations[target.0].ty) {
            safety = DefinitionSafety::Unsupported(
                "optional target definitions are not supported".into(),
            );
        }
        self.definitions.push(Definition {
            target,
            file,
            item,
            location,
            value: value_location,
            instantiation,
            dependencies,
            enforcement,
            coverage,
            safety,
            cyclic: false,
        });
    }
}
fn unwrap_parentheses(mut node: &SyntaxNode) -> &SyntaxNode {
    while node.kind() == NodeKind::ParenthesizedExpression {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
fn source_name(parsed: &zincite_syntax::ParsedFile, token: &zincite_syntax::Token) -> String {
    parsed.source()[token.range.clone()]
        .trim_matches(['\'', '`'])
        .to_owned()
}
fn optional(ty: &TypeInst) -> bool {
    ty.optional
        || match &ty.kind {
            TypeKind::Array { element, .. } | TypeKind::Set(element) => optional(element),
            TypeKind::Tuple(fields) => fields.iter().any(optional),
            TypeKind::Record(fields) => fields.iter().any(|(_, ty)| optional(ty)),
            _ => false,
        }
}
