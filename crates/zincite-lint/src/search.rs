//! Direct search interpretation, independent of missing-coverage advice.
use crate::callables::{core_operation, find_node, is_expression, operation_fact};
use crate::definitions::complete_array_coverage;
use crate::value_safety::{expression_safety, optional, traversal_expression_safety};
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableDefinitionFacts, CallableFacts,
    DeclarationId, DeclarationRole, DefinitionCoverage, DefinitionEnforcement, DefinitionFacts,
    DefinitionSafety, DomainFacts, FileId, Instantiation, InstantiationFacts, ModelContext,
    ModelRootState, SourceDiagnostic, SourceKind, SourceLocation, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SearchCoverage {
    Scalar,
    WholeArray,
    PartialArray,
    Uncovered,
    Unknown,
}
#[derive(Debug)]
pub struct SearchValue {
    pub declaration: Option<DeclarationId>,
    pub location: SourceLocation,
    pub coverage: SearchCoverage,
}
#[derive(Debug)]
pub struct SearchDeclaration {
    pub declaration: DeclarationId,
    pub location: SourceLocation,
    pub coverage: SearchCoverage,
}
#[derive(Debug)]
pub struct SearchFacts {
    pub root_state: ModelRootState,
    pub searched: Vec<SearchValue>,
    pub declarations: Vec<SearchDeclaration>,
    pub limitations: Vec<SourceDiagnostic>,
}
/// Decode retained standard searches and identity-preserving annotation/view
/// syntax, then close safe direct definitions only when every dependency is
/// covered. All prerequisites belong to this context. Numeric domains are not
/// seeds. Partial selections do not cover an entire array, and computed search
/// values do not seed their inputs. Unknown annotations withhold missing claims.
/// Exact selected callable bodies contribute bounded output guarantees. Options,
/// opaque bodies and unproved iteration/enforcement retain specific limitations.
/// Advice addresses top-level solve-visible values (thesis 4.9, p.28).
/// Checked positive model lets contribute functional local definitions. Other
/// scoped decisions retain Unknown and their required boundary.
pub fn resolve_search_coverage(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
) -> SearchFacts {
    let callable =
        crate::resolve_callable_definitions(context, bindings, calls, instantiations, domains);
    resolve_search_with_callable(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        definitions,
        &callable,
    )
}
pub(super) fn resolve_search_with_callable(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
    callable: &CallableDefinitionFacts,
) -> SearchFacts {
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        facts: SearchFacts {
            root_state: ModelRootState::Complete,
            searched: Vec::new(),
            declarations: bindings
                .declarations
                .iter()
                .map(|d| SearchDeclaration {
                    declaration: d.id,
                    location: d.location.clone(),
                    coverage: SearchCoverage::Uncovered,
                })
                .collect(),
            limitations: Vec::new(),
        },
        unknown_annotation: false,
        active: Vec::new(),
    };
    producer.searches();
    if producer.facts.root_state != ModelRootState::Complete {
        for d in &mut producer.facts.declarations {
            d.coverage = SearchCoverage::Unknown;
        }
        return producer.facts;
    }
    for missing in &callable.unavailable {
        for id in &missing.targets {
            if !covered(producer.facts.declarations[id.0].coverage) {
                producer.facts.declarations[id.0].coverage = SearchCoverage::Unknown;
            }
        }
    }
    producer.close(
        definitions,
        &callable.definitions,
        &callable.inspected_locals,
    );
    for missing in &callable.unavailable {
        if missing.targets.is_empty()
            || missing
                .targets
                .iter()
                .any(|id| !covered(producer.facts.declarations[id.0].coverage))
        {
            let limit = SourceDiagnostic {
                location: missing.location.clone(),
                message: format!("search-coverage: {}", missing.reason),
            };
            if !producer.facts.limitations.contains(&limit) {
                producer.facts.limitations.push(limit);
            }
        }
    }
    producer.facts
}
type Argument<'a> = (DeclarationId, FileId, &'a SyntaxNode);
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    facts: SearchFacts,
    unknown_annotation: bool,
    active: Vec<DeclarationId>,
}
impl<'a> Producer<'a> {
    fn searches(&mut self) {
        let mut solves = Vec::new();
        for (file, source) in self.context.files.iter().enumerate() {
            if source.kind != SourceKind::User {
                continue;
            }
            for node in source.parsed.tree().child_nodes() {
                if matches!(
                    node.kind(),
                    NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize
                ) {
                    solves.push((file, node));
                }
            }
        }
        self.facts.root_state = if !self.context.errors.is_empty()
            || !self.context.limitations.is_empty()
            || self.context.root_file.is_none()
            || solves.len() > 1
        {
            ModelRootState::Incomplete
        } else if solves.is_empty() {
            ModelRootState::Fragment
        } else {
            ModelRootState::Complete
        };
        if self.facts.root_state != ModelRootState::Complete {
            return;
        }
        let (file, solve) = solves[0];
        for annotation in solve
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
        {
            if let Some(value) = annotation.child_nodes().next() {
                self.annotation(file, value, &[]);
            }
        }
    }
    fn limit(&mut self, file: FileId, node: &SyntaxNode, reason: &str) {
        let diagnostic = SourceDiagnostic {
            location: self.context.files[file].location(node.range()),
            message: format!("search-coverage: {reason}"),
        };
        if !self.facts.limitations.contains(&diagnostic) {
            self.facts.limitations.push(diagnostic);
        }
    }
    fn uncertain(&mut self, file: FileId, node: &SyntaxNode, reason: &str) {
        self.unknown_annotation = true;
        self.limit(file, node, reason);
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let location = self.context.files[file].location(node.range());
        self.bindings
            .references
            .iter()
            .find(|r| r.file == file && location.range.contains(&r.location.range.start))
            .and_then(|r| {
                if let BindingResolution::Resolved(id) = r.resolution {
                    Some(id)
                } else {
                    None
                }
            })
    }
    fn core(&self, id: DeclarationId) -> bool {
        let d = &self.bindings.declarations[id.0];
        let source = &self.context.files[d.file];
        source.kind == SourceKind::StandardLibrary && source.implicit
    }
    fn argument(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        id: DeclarationId,
        position: usize,
    ) -> Option<(FileId, &'a SyntaxNode)> {
        crate::callables::call_argument(
            self.context,
            self.bindings,
            self.calls,
            file,
            node,
            id,
            position,
        )
    }
    fn annotation(&mut self, file: FileId, node: &'a SyntaxNode, mapping: &[Argument<'a>]) {
        let node = unwrap(node);
        if node.kind() == NodeKind::ArrayLiteral {
            for child in node.child_nodes() {
                self.annotation(file, child, mapping);
            }
            return;
        }
        if node.kind() == NodeKind::Expression {
            if node.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::StringLiteral)
            }) {
                return;
            }
            if let Some(id) = self.reference(file, node) {
                if let Some((_, file, value)) = mapping.iter().find(|(formal, _, _)| *formal == id)
                {
                    self.annotation(*file, value, mapping);
                    return;
                }
                self.alias(file, node, id, None, mapping);
                return;
            }
            self.uncertain(
                file,
                node,
                "solve annotation identity is unresolved or ambiguous",
            );
            return;
        }
        if node.kind() != NodeKind::CallExpression {
            self.uncertain(file, node, "solve annotation form is unsupported");
            return;
        }
        let Some(CallOutcome::Resolved { declaration, .. }) =
            operation_fact(self.context, self.calls, file, node).map(|c| &c.outcome)
        else {
            self.uncertain(
                file,
                node,
                "solve annotation callable selection is unresolved, ambiguous or unsupported",
            );
            return;
        };
        let id = *declaration;
        let d = &self.bindings.declarations[id.0];
        if self.core(id)
            && matches!(
                d.role,
                DeclarationRole::Annotation | DeclarationRole::Function
            )
        {
            if d.name == "seq_search" {
                if let Some((file, value)) = self.argument(file, node, id, 0) {
                    self.annotation(file, value, mapping);
                } else {
                    self.uncertain(file, node, "seq_search arguments are unavailable");
                }
                return;
            }
            if matches!(
                d.name.as_str(),
                "int_search" | "bool_search" | "float_search" | "set_search"
            ) {
                if let Some((file, value)) = self.argument(file, node, id, 0) {
                    self.values(file, value, mapping);
                } else {
                    self.uncertain(file, node, "typed search values are unavailable");
                }
                return;
            }
        }
        self.alias(file, node, id, Some(node), mapping);
    }
    fn alias(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        id: DeclarationId,
        call: Option<&'a SyntaxNode>,
        mapping: &[Argument<'a>],
    ) {
        if self.active.contains(&id) {
            self.uncertain(file, node, "recursive annotation alias is unsupported");
            return;
        }
        let d = &self.bindings.declarations[id.0];
        if self.core(id) {
            self.uncertain(
                file,
                node,
                "standard solve annotation semantics are unsupported",
            );
            return;
        }
        let Some(declaration) = find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            self.uncertain(file, node, "annotation alias body is unavailable");
            return;
        };
        let Some((value_file, value)) = declaration
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()
            .map(|value| (d.file, value))
            .or_else(|| self.assigned_annotation(id, declaration))
        else {
            self.uncertain(
                file,
                node,
                "opaque annotation does not establish search coverage",
            );
            return;
        };
        if self.calls.declarations[id.0].ty.kind != TypeKind::Annotation
            && !matches!(self.calls.declarations[id.0].ty.kind,TypeKind::Array{ref element,..} if element.kind==TypeKind::Annotation)
        {
            self.uncertain(file, node, "annotation alias type is unsupported");
            return;
        }
        let mut arguments = mapping.to_vec();
        if let Some(call) = call
            && let Some(list) = declaration
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
        {
            for (position, parameter) in list.child_nodes().enumerate() {
                let formal = self.bindings.declarations.iter().find(|p| {
                    p.file == d.file
                        && p.role == DeclarationRole::Parameter
                        && p.syntax_range == parameter.range()
                });
                if let (Some(formal), Some((file, value))) =
                    (formal, self.argument(file, call, id, position))
                {
                    arguments.push((formal.id, file, value));
                } else {
                    self.uncertain(
                        file,
                        node,
                        "annotation formal argument mapping is unsupported",
                    );
                    return;
                }
            }
        }
        self.active.push(id);
        self.annotation(value_file, value, &arguments);
        self.active.pop();
    }
    fn assigned_annotation(
        &self,
        id: DeclarationId,
        declaration: &SyntaxNode,
    ) -> Option<(FileId, &'a SyntaxNode)> {
        let d = &self.bindings.declarations[id.0];
        let ty = &self.calls.declarations[id.0].ty;
        if d.role != DeclarationRole::Annotation
            || !d.top_level
            || self.context.files[d.file].kind != SourceKind::User
            || !ty.known()
            || optional(ty)
            || ty.instantiation != Instantiation::Parameter
            || ty.kind != TypeKind::Annotation
            || declaration
                .child_nodes()
                .any(|node| node.kind() == NodeKind::ParameterList || is_expression(node.kind()))
        {
            return None;
        }
        let mut selected = None;
        for (file, source) in self.context.files.iter().enumerate() {
            if source.kind != SourceKind::User || !source.parsed.diagnostics().is_empty() {
                continue;
            }
            for (item, assignment) in source.parsed.tree().child_nodes().enumerate() {
                if assignment.kind() != NodeKind::Assignment {
                    continue;
                }
                let target = assignment.children().iter().find_map(|child| match child {
                    SyntaxElement::Token(index) => {
                        let token = &source.parsed.tokens()[*index];
                        matches!(
                            token.kind,
                            TokenKind::Identifier | TokenKind::QuotedIdentifier
                        )
                        .then_some(token)
                    }
                    _ => None,
                })?;
                let target_range = source.location(target.range.clone()).range;
                let reference = self.bindings.references.iter().find(|reference| {
                    reference.file == file
                        && reference.item == item
                        && reference.kind == crate::ReferenceKind::Value
                        && reference.callable.is_none()
                        && reference.location.range == target_range
                })?;
                match &reference.resolution {
                    BindingResolution::Resolved(target) if *target == id => {}
                    BindingResolution::Ambiguous(targets)
                    | BindingResolution::Overloads(targets)
                        if targets.contains(&id) =>
                    {
                        return None;
                    }
                    BindingResolution::Unresolved if reference.name == d.name => return None,
                    _ => continue,
                }
                if selected.is_some() {
                    return None;
                }
                let mut values = assignment
                    .child_nodes()
                    .filter(|node| is_expression(node.kind()));
                let value = values.next()?;
                if values.next().is_some() {
                    return None;
                }
                let range = source.location(value.range()).range;
                let ty = &self
                    .calls
                    .expressions
                    .iter()
                    .find(|expression| {
                        expression.file == file && expression.location.range == range
                    })?
                    .ty;
                if !ty.known()
                    || optional(ty)
                    || ty.instantiation != Instantiation::Parameter
                    || ty.kind != TypeKind::Annotation
                {
                    return None;
                }
                // Do not strip annotations whose effect on this assigned body is unexamined.
                let mut pending = vec![declaration, value];
                while let Some(node) = pending.pop() {
                    if matches!(
                        node.kind(),
                        NodeKind::Annotation | NodeKind::AnnotatedExpression
                    ) {
                        return None;
                    }
                    pending.extend(node.child_nodes());
                }
                selected = Some((file, value));
            }
        }
        selected
    }
    fn reindex_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        mapping: &[Argument<'a>],
        generators: Option<&[&'a SyntaxNode]>,
    ) -> DefinitionSafety {
        let mut safety = match generators {
            Some(generators) => crate::callable_definitions::initialized_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                self.instantiations,
                self.domains,
                (file, node, generators, true),
                None,
            ),
            None => expression_safety(self.context, self.bindings, self.calls, file, node),
        };
        if matches!(safety, DefinitionSafety::Unsupported(_))
            || (generators.is_none() && safety != DefinitionSafety::Supported)
        {
            return safety;
        }
        let mut nodes = vec![(file, node, mapping.len() + 1)];
        while let Some((file, node, remaining)) = nodes.pop() {
            let node = unwrap(node);
            if node.kind() == NodeKind::Expression
                && let Some(id) = self.reference(file, node)
                && let Some((_, actual_file, actual)) =
                    mapping.iter().find(|(formal, _, _)| *formal == id)
            {
                if remaining == 0 {
                    return DefinitionSafety::Unsupported(
                        "recursive reindex argument mapping is unsupported".into(),
                    );
                }
                let range = self.context.files[*actual_file]
                    .location(actual.range())
                    .range;
                let actual_type = self
                    .calls
                    .expressions
                    .iter()
                    .find(|e| e.file == *actual_file && e.location.range == range)
                    .map(|e| &e.ty);
                if actual_type.is_none_or(|ty| {
                    !ty.known()
                        || optional(ty)
                        || ty.instantiation != Instantiation::Parameter
                        || ty.kind != self.calls.declarations[id.0].ty.kind
                }) {
                    return DefinitionSafety::Unsupported(
                        "reindex actual type or instantiation is unsupported".into(),
                    );
                }
                let actual_safety = if generators.is_some() {
                    let mut actual_generators = Vec::new();
                    collect_generators(
                        self.context.files[*actual_file].parsed.tree(),
                        &actual.range(),
                        &mut actual_generators,
                    );
                    actual_generators.retain(|generator| {
                        generator
                            .child_nodes()
                            .find(|n| is_expression(n.kind()))
                            .is_some_and(|source| source.range().end <= actual.range().start)
                    });
                    crate::callable_definitions::initialized_expression_safety(
                        self.context,
                        self.bindings,
                        self.calls,
                        self.instantiations,
                        self.domains,
                        (*actual_file, actual, &actual_generators, true),
                        None,
                    )
                } else {
                    expression_safety(
                        self.context,
                        self.bindings,
                        self.calls,
                        *actual_file,
                        actual,
                    )
                };
                match actual_safety {
                    unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                    unknown @ DefinitionSafety::Unknown(_) => {
                        if generators.is_none() {
                            return unknown;
                        }
                        safety = unknown;
                    }
                    DefinitionSafety::Supported => {}
                }
                nodes.push((*actual_file, actual, remaining - 1));
                continue;
            }
            if matches!(
                node.kind(),
                NodeKind::UnaryExpression | NodeKind::BinaryExpression | NodeKind::RangeExpression
            ) {
                let name = node.children().iter().find_map(|child| match child {
                    SyntaxElement::Token(index) => crate::bindings::symbolic_operator(
                        self.context.files[file].parsed.tokens()[*index].kind,
                    ),
                    _ => None,
                });
                if name.is_none_or(|name| {
                    !core_operation(self.context, self.bindings, self.calls, file, node, name)
                        .is_ok_and(|core| core)
                }) {
                    return DefinitionSafety::Unsupported(
                        "reindex operator identity is unsupported".into(),
                    );
                }
            }
            nodes.extend(node.child_nodes().map(|child| (file, child, remaining)));
        }
        safety
    }
    fn values(&mut self, file: FileId, node: &'a SyntaxNode, mapping: &[Argument<'a>]) {
        let node = unwrap(node);
        let location = self.context.files[file].location(node.range());
        let ty = self
            .calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == location.range)
            .map(|e| &e.ty);
        if ty.is_none_or(|ty| !ty.known() || optional(ty)) {
            self.uncertain(
                file,
                node,
                "search value type, optionality or view is unsupported",
            );
            return;
        }
        if matches!(
            node.kind(),
            NodeKind::ArrayLiteral | NodeKind::MatrixLiteral
        ) {
            for child in node.child_nodes() {
                if child.kind() == NodeKind::MatrixRow {
                    for cell in child.child_nodes() {
                        self.values(file, cell, mapping);
                    }
                } else {
                    self.values(file, child, mapping);
                }
            }
            return;
        }
        if node.kind() == NodeKind::BinaryExpression
            && matches!(ty.map(|ty| &ty.kind), Some(TypeKind::Array { .. }))
        {
            let children: Vec<_> = node.child_nodes().collect();
            if children.len() == 2
                && core_operation(self.context, self.bindings, self.calls, file, node, "++")
                    .is_ok_and(|core| core)
                && children.iter().all(|child| {
                    let range = self.context.files[file].location(child.range()).range;
                    self.calls.expressions.iter().any(|e| {
                        e.file == file
                            && e.location.range == range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && matches!(&e.ty.kind, TypeKind::Array { indices, .. } if indices.len() == 1)
                    })
                })
            {
                for child in children {
                    self.values(file, child, mapping);
                }
            } else {
                self.uncertain(file, node, "array search operation identity is unsupported");
            }
            return;
        }
        if node.kind() == NodeKind::ArrayComprehension {
            let body = node.child_nodes().find(|n| is_expression(n.kind()));
            let list = node
                .child_nodes()
                .find(|n| n.kind() == NodeKind::GeneratorList);
            let (Some(body), Some(list)) = (body, list) else {
                self.uncertain(file, node, "search traversal syntax is unavailable");
                return;
            };
            let generators: Vec<_> = list.child_nodes().collect();
            let mut strict_traversal = true;
            for (position, generator) in generators.iter().enumerate() {
                for child in generator.child_nodes() {
                    let value = if child.kind() == NodeKind::WhereFilter {
                        child.child_nodes().next().unwrap_or(child)
                    } else {
                        child
                    };
                    let range = self.context.files[file].location(value.range()).range;
                    let known_parameter = self.calls.expressions.iter().any(|e| {
                        e.file == file
                            && e.location.range == range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.instantiation == Instantiation::Parameter
                    });
                    if !known_parameter {
                        self.uncertain(file, value, "search traversal evaluation is unavailable");
                        return;
                    }
                    let lexical = if child.kind() == NodeKind::WhereFilter {
                        &generators[..=position]
                    } else {
                        &generators[..position]
                    };
                    match self.reindex_safety(file, value, mapping, Some(lexical)) {
                        DefinitionSafety::Supported => {}
                        DefinitionSafety::Unknown(_) => strict_traversal = false,
                        DefinitionSafety::Unsupported(_) => {
                            self.uncertain(
                                file,
                                value,
                                "search traversal evaluation is unavailable",
                            );
                            return;
                        }
                    }
                }
            }
            let body = unwrap(body);
            let children: Vec<_> = body.child_nodes().collect();
            if strict_traversal
                && body.kind() == NodeKind::ArrayAccessExpression
                && let Some(subject) = children.first()
                && subject.kind() == NodeKind::Expression
                && let Some(id) = self.reference(file, subject)
                && let TypeKind::Array { indices, .. } = &self.calls.declarations[id.0].ty.kind
                && indices.len() == children.len() - 1
                && indices.iter().zip(&children[1..]).all(|(expected, index)| {
                    let index = unwrap(index);
                    let range = self.context.files[file].location(index.range()).range;
                    index.kind() == NodeKind::Expression
                        && expected.known()
                        && !optional(expected)
                        && expected.instantiation == Instantiation::Parameter
                        && self.calls.expressions.iter().any(|e| {
                            e.file == file
                                && e.location.range == range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.instantiation == Instantiation::Parameter
                                && e.ty.kind == expected.kind
                        })
                })
            {
                match complete_array_coverage(
                    self.context,
                    self.bindings,
                    self.instantiations,
                    self.domains,
                    (file, id),
                    (&children[1..], &generators),
                ) {
                    DefinitionCoverage::WholeArray
                        if traversal_expression_safety(
                            self.context,
                            self.bindings,
                            self.calls,
                            file,
                            body,
                            body,
                        ) == DefinitionSafety::Supported =>
                    {
                        self.values(file, subject, mapping);
                        return;
                    }
                    DefinitionCoverage::Unsupported(reason) => {
                        self.uncertain(file, node, &reason);
                        return;
                    }
                    _ => {}
                }
            }
            // A filtered or computed traversal does not prove whole-source
            // coverage. It supplies no seed for the declarations it mentions.
            self.facts.searched.push(SearchValue {
                declaration: None,
                location,
                coverage: SearchCoverage::Uncovered,
            });
            return;
        }
        if node.kind() == NodeKind::CallExpression {
            if let Some(CallOutcome::Resolved { declaration, .. }) =
                operation_fact(self.context, self.calls, file, node).map(|c| &c.outcome)
            {
                let d = &self.bindings.declarations[declaration.0];
                if self.core(*declaration) && d.name == "array1d" {
                    let len = self
                        .calls
                        .signatures
                        .iter()
                        .find(|s| s.declaration == *declaration)
                        .map_or(0, |s| s.parameters.len());
                    if len == 1
                        && let Some((file, value)) = self.argument(file, node, *declaration, 0)
                    {
                        self.values(file, value, mapping);
                        return;
                    }
                    if len == 2
                        && let (Some((index_file, indices)), Some((value_file, value))) = (
                            self.argument(file, node, *declaration, 0),
                            self.argument(file, node, *declaration, 1),
                        )
                    {
                        let range = self.context.files[index_file]
                            .location(indices.range())
                            .range;
                        let known_indices = self.calls.expressions.iter().any(|e| {
                            e.file == index_file
                                && e.location.range == range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.instantiation == Instantiation::Parameter
                                && matches!(&e.ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
                        });
                        if known_indices {
                            match self.reindex_safety(index_file, indices, mapping, None) {
                                DefinitionSafety::Supported => {
                                    // Reindexing retains every source value when the builtin
                                    // succeeds. It does not prove index equality or size validity.
                                    self.values(value_file, value, mapping);
                                }
                                DefinitionSafety::Unknown(reason)
                                | DefinitionSafety::Unsupported(reason) => self.uncertain(
                                    index_file,
                                    indices,
                                    &format!("search reindex evaluation is unavailable: {reason}"),
                                ),
                            }
                            return;
                        }
                    }
                }
            }
            self.uncertain(file, node, "search view does not establish source identity");
            return;
        }
        let (subject, extent) = if node.kind() == NodeKind::ArrayAccessExpression {
            (
                node.child_nodes().next().unwrap_or(node),
                SearchCoverage::PartialArray,
            )
        } else {
            (node, SearchCoverage::Scalar)
        };
        if subject.kind() == NodeKind::Expression
            && let Some(id) = self.reference(file, subject)
        {
            if let Some((_, file, value)) = mapping.iter().find(|(formal, _, _)| *formal == id) {
                if extent == SearchCoverage::PartialArray {
                    let actual = unwrap(value);
                    if let Some(actual_id) = self.partial_source(*file, actual, mapping) {
                        self.record_value(actual_id, location, SearchCoverage::PartialArray);
                    } else {
                        self.uncertain(*file, actual, "partial formal selection through a view or computed actual is unsupported");
                    }
                } else {
                    self.values(*file, value, mapping);
                }
                return;
            }
            let extent = if extent == SearchCoverage::Scalar
                && matches!(
                    self.calls.declarations[id.0].ty.kind,
                    TypeKind::Array { .. }
                ) {
                SearchCoverage::WholeArray
            } else {
                extent
            };
            self.record_value(id, location, extent);
            if extent == SearchCoverage::WholeArray {
                self.array_sources(id, file, subject, mapping);
            }
            return;
        }
        // A fresh computed search value has no source declaration identity.
        // Its operands are not searched merely because they occur in its syntax.
        self.facts.searched.push(SearchValue {
            declaration: None,
            location,
            coverage: SearchCoverage::Uncovered,
        });
    }
    fn array_sources(
        &mut self,
        id: DeclarationId,
        file: FileId,
        reference: &'a SyntaxNode,
        mapping: &[Argument<'a>],
    ) {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        if !declaration.top_level
            || declaration.role != DeclarationRole::Value
            || source.kind != SourceKind::User
            || !source.parsed.diagnostics().is_empty()
        {
            return;
        }
        let Some(written) = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) else {
            return;
        };
        if written.kind() != NodeKind::Declaration {
            return;
        }
        let mut values = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()));
        let Some(value) = values.next() else {
            return;
        };
        if values.next().is_some() {
            return;
        }
        let value = unwrap(value);
        let value_range = source.location(value.range()).range;
        if self
            .calls
            .expressions
            .iter()
            .find(|e| e.file == declaration.file && e.location.range == value_range)
            .is_none_or(|e| {
                !e.ty.known() || optional(&e.ty) || !matches!(e.ty.kind, TypeKind::Array { .. })
            })
        {
            return;
        }
        let next = match value.kind() {
            NodeKind::Expression | NodeKind::ArrayLiteral => value,
            NodeKind::CallExpression
                if core_operation(
                    self.context,
                    self.bindings,
                    self.calls,
                    declaration.file,
                    value,
                    "array1d",
                ) == Ok(true) =>
            {
                value
            }
            NodeKind::CallExpression => {
                let Some(values) = crate::callable_definitions::set_axis_integer_reshape_source(
                    self.context,
                    self.bindings,
                    self.calls,
                    self.instantiations,
                    self.domains,
                    (declaration.file, value),
                    None,
                ) else {
                    return;
                };
                values
            }
            _ => return,
        };
        if self.active.contains(&id) {
            self.uncertain(
                file,
                reference,
                "cyclic searched array initializer is unsupported",
            );
            return;
        }
        if let DefinitionSafety::Unsupported(reason) =
            crate::callable_definitions::initialized_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                self.instantiations,
                self.domains,
                (file, reference, &[], false),
                None,
            )
        {
            self.uncertain(
                file,
                reference,
                &format!("searched array initializer is unsupported: {reason}"),
            );
            return;
        }
        // Cardinality uncertainty does not change the values of a successful pure view.
        self.active.push(id);
        self.values(declaration.file, next, mapping);
        self.active.pop();
    }
    fn partial_source(
        &self,
        mut file: FileId,
        mut node: &'a SyntaxNode,
        mapping: &[Argument<'a>],
    ) -> Option<DeclarationId> {
        for _ in 0..=mapping.len() {
            node = unwrap(node);
            if node.kind() != NodeKind::Expression {
                return None;
            }
            let id = self.reference(file, node)?;
            if let Some((_, actual_file, actual)) =
                mapping.iter().find(|(formal, _, _)| *formal == id)
            {
                file = *actual_file;
                node = actual;
            } else {
                return Some(id);
            }
        }
        None
    }
    fn record_value(
        &mut self,
        id: DeclarationId,
        location: SourceLocation,
        extent: SearchCoverage,
    ) {
        self.facts.searched.push(SearchValue {
            declaration: Some(id),
            location,
            coverage: extent,
        });
        let state = &mut self.facts.declarations[id.0].coverage;
        if !covered(*state) || extent != SearchCoverage::PartialArray {
            *state = extent;
        }
    }
    fn close(
        &mut self,
        definitions: &DefinitionFacts,
        callable: &[crate::Definition],
        inspected_locals: &[DeclarationId],
    ) {
        for d in &self.bindings.declarations {
            let ty = &self.calls.declarations[d.id.0].ty;
            if d.role == DeclarationRole::Local && ty.instantiation == Instantiation::Decision {
                self.facts.declarations[d.id.0].coverage =
                    if callable.iter().any(|definition| definition.target == d.id) {
                        SearchCoverage::Uncovered
                    } else {
                        SearchCoverage::Unknown
                    };
                continue;
            }
            if ty.instantiation == Instantiation::Parameter && ty.known() {
                self.facts.declarations[d.id.0].coverage =
                    if matches!(ty.kind, TypeKind::Array { .. }) {
                        SearchCoverage::WholeArray
                    } else {
                        SearchCoverage::Scalar
                    };
            }
        }
        let mut admitted = Vec::new();
        for (d, callable_record) in definitions
            .definitions
            .iter()
            .map(|d| (d, false))
            .chain(callable.iter().map(|d| (d, true)))
        {
            // Solve searches name model declarations. Local definitions inside
            // an uncalled body are not model search requirements (base-044
            // owns callable-output propagation).
            let target = &self.bindings.declarations[d.target.0];
            let scoped = callable_record && target.role == DeclarationRole::Local;
            let scoped_integer =
                scoped && self.calls.declarations[d.target.0].ty.kind == TypeKind::Int;
            if !scoped && (!target.top_level || target.role != DeclarationRole::Value) {
                continue;
            }
            if d.enforcement == DefinitionEnforcement::Conditional
                || !matches!(
                    d.coverage,
                    DefinitionCoverage::Scalar | DefinitionCoverage::WholeArray
                )
            {
                continue;
            }
            let safety = if d.safety == DefinitionSafety::Supported {
                d.safety.clone()
            } else {
                self.rhs_safety(d).unwrap_or_else(|| d.safety.clone())
            };
            if d.enforcement != DefinitionEnforcement::Enforced
                || safety != DefinitionSafety::Supported
            {
                if !covered(self.facts.declarations[d.target.0].coverage) {
                    self.facts.declarations[d.target.0].coverage = SearchCoverage::Unknown;
                    if matches!(safety, DefinitionSafety::Unsupported(_))
                        || matches!(d.enforcement, DefinitionEnforcement::Unsupported(_))
                    {
                        self.facts.limitations.push(SourceDiagnostic {
                            location: d.location.clone(),
                            message: "search-coverage: direct definition safety or enforcement is unsupported".into(),
                        });
                    }
                }
                continue;
            }
            let Some(node) = locate(
                self.context.files[d.file].parsed.tree(),
                &(d.location.range.start - self.context.files[d.file].byte_offset
                    ..d.location.range.end - self.context.files[d.file].byte_offset),
            ) else {
                continue;
            };
            if (node.kind() == NodeKind::BinaryExpression
                && !scoped_integer
                && !core_operation(
                    self.context,
                    self.bindings,
                    self.calls,
                    d.file,
                    node,
                    if scoped { "<->" } else { "=" },
                )
                .is_ok_and(|core| core))
                || !self.conjunctions_enforced(
                    d.file,
                    self.context.files[d.file].parsed.tree(),
                    &node.range(),
                )
            {
                if !covered(self.facts.declarations[d.target.0].coverage) {
                    self.facts.declarations[d.target.0].coverage = SearchCoverage::Unknown;
                    self.limit(
                        d.file,
                        node,
                        "direct definition operation identity is unsupported",
                    );
                }
                continue;
            }
            admitted.push(d);
        }
        loop {
            let mut changed = false;
            for d in &admitted {
                if !covered(self.facts.declarations[d.target.0].coverage)
                    && d.dependencies
                        .iter()
                        .all(|id| covered(self.facts.declarations[id.0].coverage))
                {
                    self.facts.declarations[d.target.0].coverage =
                        if d.coverage == DefinitionCoverage::WholeArray {
                            SearchCoverage::WholeArray
                        } else {
                            SearchCoverage::Scalar
                        };
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        // An unsupported prerequisite also prevents a confident claim about
        // its direct dependents. This never supplies a coverage seed.
        loop {
            let mut changed = false;
            for d in &admitted {
                let state = self.facts.declarations[d.target.0].coverage;
                if !covered(state)
                    && state != SearchCoverage::Unknown
                    && d.dependencies
                        .iter()
                        .any(|id| self.facts.declarations[id.0].coverage == SearchCoverage::Unknown)
                {
                    self.facts.declarations[d.target.0].coverage = SearchCoverage::Unknown;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if self.unknown_annotation {
            for d in &mut self.facts.declarations {
                if !covered(d.coverage) {
                    d.coverage = SearchCoverage::Unknown;
                }
            }
        }
        for d in &self.bindings.declarations {
            if d.role != DeclarationRole::Local
                || self.calls.declarations[d.id.0].ty.instantiation != Instantiation::Decision
                || callable.iter().any(|definition| definition.target == d.id)
                || inspected_locals.contains(&d.id)
                || covered(self.facts.declarations[d.id.0].coverage)
            {
                continue;
            }
            let source = &self.context.files[d.file];
            let item = source.parsed.tree().child_nodes().nth(d.item).unwrap();
            if source.warnings_enabled()
                && !matches!(
                    item.kind(),
                    NodeKind::FunctionDeclaration
                        | NodeKind::PredicateDeclaration
                        | NodeKind::TestDeclaration
                        | NodeKind::AnnotationDeclaration
                )
            {
                self.facts.limitations.push(SourceDiagnostic {
                    location: d.location.clone(),
                    message: concat!(
                        "search-coverage: model-level local decision coverage is unsupported; ",
                        "its scoped name is not available to the solve search"
                    )
                    .into(),
                });
            }
        }
    }
    // DefinitionFacts retains syntactic conjunction enforcement. Search needs
    // the selected core operation too: a user '/\\' need not enforce either child.
    fn conjunctions_enforced(
        &self,
        file: FileId,
        node: &SyntaxNode,
        range: &std::ops::Range<usize>,
    ) -> bool {
        if node.range().start > range.start || node.range().end < range.end {
            return true;
        }
        if node.kind() == NodeKind::BinaryExpression
            && node.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::And)
            })
            && !core_operation(self.context, self.bindings, self.calls, file, node, "/\\")
                .is_ok_and(|core| core)
        {
            return false;
        }
        node.child_nodes()
            .all(|child| self.conjunctions_enforced(file, child, range))
    }
    fn rhs_safety(&self, d: &crate::Definition) -> Option<DefinitionSafety> {
        let source = &self.context.files[d.file];
        let value = locate(
            source.parsed.tree(),
            &(d.value.range.start - source.byte_offset..d.value.range.end - source.byte_offset),
        )?;
        let mut generators = Vec::new();
        collect_generators(source.parsed.tree(), &value.range(), &mut generators);
        generators.retain(|g| g.range().end <= value.range().start);
        let direct = crate::callable_definitions::direct_expression_safety(
            self.context,
            self.bindings,
            self.calls,
            self.instantiations,
            self.domains,
            (d.file, value),
            &generators,
        );
        if !matches!(direct, DefinitionSafety::Unsupported(_)) {
            return Some(direct);
        }
        if d.coverage != DefinitionCoverage::WholeArray {
            return None;
        }
        let access = unwrap(value);
        if access.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let children: Vec<_> = access.child_nodes().collect();
        let id = self.reference(d.file, *children.first()?)?;
        if complete_array_coverage(
            self.context,
            self.bindings,
            self.instantiations,
            self.domains,
            (d.file, id),
            (&children[1..], &generators),
        ) != DefinitionCoverage::WholeArray
        {
            return None;
        }
        Some(traversal_expression_safety(
            self.context,
            self.bindings,
            self.calls,
            d.file,
            value,
            access,
        ))
    }
}
pub(super) fn covered(coverage: SearchCoverage) -> bool {
    matches!(
        coverage,
        SearchCoverage::Scalar | SearchCoverage::WholeArray
    )
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression | NodeKind::NamedArgument
    ) {
        let Some(child) = node.child_nodes().next() else {
            break;
        };
        node = child;
    }
    node
}
fn locate<'a>(node: &'a SyntaxNode, range: &std::ops::Range<usize>) -> Option<&'a SyntaxNode> {
    if node.range() == *range {
        return Some(node);
    }
    node.child_nodes().find_map(|n| locate(n, range))
}
fn collect_generators<'a>(
    node: &'a SyntaxNode,
    range: &std::ops::Range<usize>,
    out: &mut Vec<&'a SyntaxNode>,
) {
    if node.range().start > range.start || node.range().end < range.end {
        return;
    }
    if matches!(
        node.kind(),
        NodeKind::GeneratorCallExpression | NodeKind::ArrayComprehension
    ) && let Some(list) = node
        .child_nodes()
        .find(|n| n.kind() == NodeKind::GeneratorList)
    {
        out.extend(list.child_nodes());
    }
    for child in node.child_nodes() {
        collect_generators(child, range, out);
    }
}
