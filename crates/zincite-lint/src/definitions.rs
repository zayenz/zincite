//! Definition candidates and dependencies, independent of diagnostic policy.
use crate::callables::{find_node, is_expression};
use crate::domains::{expression_domain, same_members};
use crate::value_safety::optional;
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    Domain, DomainFacts, FileId, Instantiation, InstantiationFacts, ModelContext, ReferenceKind,
    SourceKind, SourceLocation, TypeKind,
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
        resolved_reference(self.context, self.bindings, file, node)
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
        annotations_safe(self.context, file, node)
    }
    fn call(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        resolved_call(self.context, self.calls, file, node)
    }
    fn core_forall(&self, id: DeclarationId) -> bool {
        core_callable(self.context, self.bindings, id, "forall")
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
        transparent_boolean_argument(
            self.context,
            self.bindings,
            self.calls,
            (file, item, call),
            id,
        )
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
        complete_array_coverage(
            self.context,
            self.bindings,
            self.instantiations,
            self.domains,
            (file, id),
            (indices, generators),
        )
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
        let mut safety =
            crate::expression_safety(self.context, self.bindings, self.calls, file, value);
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

/// Exact, unfiltered index correspondence; arithmetic consumers also rely on
/// distinct binders and no dependent/extra generators to preserve multiplicity.
pub(super) fn complete_array_coverage(
    context: &ModelContext,
    bindings_facts: &BindingFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    scope: (FileId, DeclarationId),
    syntax: (&[&SyntaxNode], &[&SyntaxNode]),
) -> DefinitionCoverage {
    let (file, id) = scope;
    let (indices, generators) = syntax;
    let reference = |node: &SyntaxNode| {
        let start = significant_tokens(context, file, node).first()?.range.start
            + context.files[file].byte_offset;
        bindings_facts
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
    };
    let instantiation = |node: &SyntaxNode| {
        let range = context.files[file].location(node.range()).range;
        instantiations
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map_or(Instantiation::Unknown, |e| e.instantiation)
    };
    let mut domain = &domains.declarations[id.0].domain;
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
            || !significant_tokens(context, file, generator)
                .iter()
                .any(|t| t.kind == TokenKind::In)
        {
            return DefinitionCoverage::ArrayElement;
        }
        let Some(source) = generator.child_nodes().next() else {
            return DefinitionCoverage::ArrayElement;
        };
        if instantiation(source) != Instantiation::Parameter {
            return DefinitionCoverage::ArrayElement;
        }
        let source_range = context.files[file].location(source.range()).range;
        if bindings_facts.references.iter().any(|r| {
            r.file == file
                && source_range.start <= r.location.range.start
                && r.location.range.end <= source_range.end
                && match r.resolution {
                    BindingResolution::Resolved(id) => {
                        bindings_facts.declarations[id.0].role == DeclarationRole::Generator
                    }
                    _ => false,
                }
        }) {
            return DefinitionCoverage::ArrayElement;
        }
        let domain = expression_domain(context, bindings_facts, file, source);
        for declaration in bindings_facts.declarations.iter().filter(|d| {
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
        let Some(id) = reference(unwrap_parentheses(index)) else {
            return DefinitionCoverage::ArrayElement;
        };
        if used.contains(&id) {
            return DefinitionCoverage::ArrayElement;
        }
        let Some((_, actual)) = bindings.iter().find(|(binder, _)| *binder == id) else {
            return DefinitionCoverage::ArrayElement;
        };
        used.push(id);
        let membership = same_members(actual, expected).and_then(|_| {
            same_members(
                &index_membership(actual, bindings_facts),
                &index_membership(expected, bindings_facts),
            )
        });
        match membership {
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
// Keep parameter-set identities and alias chains, without substituting their
// initializers into an unrelated literal traversal. Written type aliases and
// unnamed literal sets/ranges still have their closed membership.
fn index_membership(domain: &Domain, bindings: &BindingFacts) -> Domain {
    match domain {
        Domain::Named {
            declaration,
            domain,
        } => Domain::Named {
            declaration: *declaration,
            domain: Box::new(
                if bindings.declarations[declaration.0].role == DeclarationRole::TypeAlias
                    || matches!(domain.as_ref(), Domain::Named { .. })
                {
                    index_membership(domain, bindings)
                } else {
                    Domain::Unknown
                },
            ),
        },
        other => other.clone(),
    }
}
fn significant_tokens<'a>(
    context: &'a ModelContext,
    file: FileId,
    node: &SyntaxNode,
) -> Vec<&'a zincite_syntax::Token> {
    node.children()
        .iter()
        .filter_map(|child| {
            let SyntaxElement::Token(index) = child else {
                return None;
            };
            let token = &context.files[file].parsed.tokens()[*index];
            (!matches!(
                token.kind,
                TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
            ))
            .then_some(token)
        })
        .collect()
}

/// Shared callable boundaries for definition and guarded interpretation.
pub(super) fn core_callable(
    context: &ModelContext,
    bindings: &BindingFacts,
    id: DeclarationId,
    name: &str,
) -> bool {
    let declaration = &bindings.declarations[id.0];
    let source = &context.files[declaration.file];
    declaration.name == name
        && declaration.role == DeclarationRole::Function
        && source.kind == SourceKind::StandardLibrary
        && source.implicit
}
pub(super) fn resolved_call(
    context: &ModelContext,
    calls: &CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<DeclarationId> {
    let start = crate::domains::tokens(&context.files[file].parsed, node)
        .into_iter()
        .find(|t| {
            matches!(
                t.kind,
                TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::InfixIdentifier
            )
        })?
        .range
        .start
        + context.files[file].byte_offset;
    calls
        .calls
        .iter()
        .find(|c| c.file == file && c.location.range.start == start)
        .and_then(|c| match c.outcome {
            CallOutcome::Resolved { declaration, .. } => Some(declaration),
            _ => None,
        })
}
pub(super) fn resolved_reference(
    context: &ModelContext,
    bindings: &BindingFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<DeclarationId> {
    let start = crate::domains::tokens(&context.files[file].parsed, node)
        .first()?
        .range
        .start
        + context.files[file].byte_offset;
    bindings
        .references
        .iter()
        .find(|r| r.file == file && r.location.range.start == start)
        .and_then(|r| match r.resolution {
            BindingResolution::Resolved(id) => Some(id),
            _ => None,
        })
}
pub(super) fn transparent_boolean_argument<'a>(
    context: &'a ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    site: (FileId, usize, &'a SyntaxNode),
    id: DeclarationId,
) -> Option<(FileId, usize, &'a SyntaxNode)> {
    let (file, item, call) = site;
    let declaration = &bindings.declarations[id.0];
    let source: &'a crate::ModelFile = &context.files[declaration.file];
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
    let signature = calls.signatures.iter().find(|s| s.declaration == id)?;
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
    let formal = resolved_reference(context, bindings, declaration.file, body)?;
    let formal = &bindings.declarations[formal.0];
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
            let token = crate::domains::tokens(&context.files[file].parsed, argument)
                .first()
                .copied()?;
            let name = source_name(&context.files[file].parsed, token);
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

pub(super) fn annotations_safe(context: &ModelContext, file: FileId, node: &SyntaxNode) -> bool {
    node.child_nodes()
        .filter(|n| n.kind() == NodeKind::Annotation)
        .all(|annotation| {
            annotation.child_nodes().next().is_some_and(|value| {
                value.kind() == NodeKind::Expression
                    && crate::domains::tokens(&context.files[file].parsed, value)
                        .first()
                        .is_some_and(|t| t.kind == TokenKind::StringLiteral)
            })
        })
}
