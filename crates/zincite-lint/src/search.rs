//! Direct search interpretation, independent of missing-coverage advice.
use crate::callables::{core_operation, find_node, is_expression, operation_fact};
use crate::definitions::complete_array_coverage;
use crate::value_safety::{optional, traversal_expression_safety};
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    DefinitionCoverage, DefinitionEnforcement, DefinitionFacts, DefinitionSafety, DomainFacts,
    FileId, Instantiation, InstantiationFacts, ModelContext, ModelRootState, SourceDiagnostic,
    SourceKind, SourceLocation, TypeKind,
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
/// Model-level local decisions retain Unknown plus a scoped limitation; callable
/// local/control-flow output proofs remain explicitly unsupported.
pub fn resolve_search_coverage(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
    definitions: &DefinitionFacts,
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
    let callable =
        crate::resolve_callable_definitions(context, bindings, calls, instantiations, domains);
    for missing in &callable.unavailable {
        for id in &missing.targets {
            if !covered(producer.facts.declarations[id.0].coverage) {
                producer.facts.declarations[id.0].coverage = SearchCoverage::Unknown;
            }
        }
    }
    let mut combined = DefinitionFacts {
        definitions: definitions.definitions.clone(),
    };
    combined.definitions.extend(callable.definitions);
    producer.close(&combined);
    for missing in callable.unavailable {
        if missing.targets.is_empty()
            || missing
                .targets
                .iter()
                .any(|id| !covered(producer.facts.declarations[id.0].coverage))
        {
            let limit = SourceDiagnostic {
                location: missing.location,
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
        let Some(value) = declaration
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()
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
        self.annotation(d.file, value, &arguments);
        self.active.pop();
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
    fn close(&mut self, definitions: &DefinitionFacts) {
        for d in &self.bindings.declarations {
            let ty = &self.calls.declarations[d.id.0].ty;
            if d.role == DeclarationRole::Local && ty.instantiation == Instantiation::Decision {
                self.facts.declarations[d.id.0].coverage = SearchCoverage::Unknown;
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
        for d in &definitions.definitions {
            // Solve searches name model declarations. Local definitions inside
            // an uncalled body are not model search requirements (base-044
            // owns callable-output propagation).
            let target = &self.bindings.declarations[d.target.0];
            if !target.top_level || target.role != DeclarationRole::Value {
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
                && !core_operation(self.context, self.bindings, self.calls, d.file, node, "=")
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
        if d.coverage != DefinitionCoverage::WholeArray {
            return None;
        }
        let source = &self.context.files[d.file];
        let value = locate(
            source.parsed.tree(),
            &(d.value.range.start - source.byte_offset..d.value.range.end - source.byte_offset),
        )?;
        let access = unwrap(value);
        if access.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let children: Vec<_> = access.child_nodes().collect();
        let id = self.reference(d.file, *children.first()?)?;
        let mut generators = Vec::new();
        collect_generators(source.parsed.tree(), &access.range(), &mut generators);
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
