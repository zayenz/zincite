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
#[derive(Clone, Copy, PartialEq, Eq)]
enum AnnotationUse {
    Search,
    Candidate,
    Heuristic,
}
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
                self.annotation(file, value, &[], AnnotationUse::Search);
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
    fn annotation(
        &mut self,
        file: FileId,
        node: &'a SyntaxNode,
        mapping: &[Argument<'a>],
        usage: AnnotationUse,
    ) {
        if usage != AnnotationUse::Search
            || matches!(
                unwrap(node).kind(),
                NodeKind::ArrayAccessExpression | NodeKind::ConditionalExpression
            )
        {
            let mut source = node;
            loop {
                if !crate::definitions::annotations_safe(self.context, file, source) {
                    self.uncertain(file, source, "candidate annotation source is unsupported");
                    return;
                }
                if !matches!(
                    source.kind(),
                    NodeKind::ParenthesizedExpression
                        | NodeKind::AnnotatedExpression
                        | NodeKind::NamedArgument
                ) {
                    break;
                }
                let Some(child) = source.child_nodes().next() else {
                    break;
                };
                source = child;
            }
        }
        let written = node;
        let node = unwrap(node);
        if node.kind() == NodeKind::ArrayLiteral {
            for child in node.child_nodes() {
                self.annotation(file, child, mapping, usage);
            }
            return;
        }
        if usage != AnnotationUse::Search {
            let range = self.context.files[file].location(node.range()).range;
            if !self.calls.expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
                    && e.ty.kind == TypeKind::Annotation
            }) {
                self.uncertain(file, node, "candidate annotation type is unsupported");
                return;
            }
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
                    self.annotation(*file, value, mapping, usage);
                    return;
                }
                if usage == AnnotationUse::Heuristic && self.core(id) {
                    let d = &self.bindings.declarations[id.0];
                    let primitive = d.role == DeclarationRole::Annotation
                        && matches!(
                            d.name.as_str(),
                            "input_order"
                                | "first_fail"
                                | "smallest"
                                | "largest"
                                | "indomain_min"
                                | "indomain_max"
                                | "indomain_median"
                                | "indomain_random"
                                | "complete"
                        )
                        && self.calls.signatures.iter().any(|s| {
                            s.declaration == id
                                && s.parameters.is_empty()
                                && s.return_type == self.calls.declarations[id.0].ty
                        })
                        && find_node(
                            self.context.files[d.file].parsed.tree(),
                            &d.syntax_range,
                            d.role,
                        )
                        .is_some_and(|written| written.child_nodes().next().is_none());
                    if !primitive {
                        self.uncertain(file, node, "search heuristic semantics are unsupported");
                    }
                    return;
                }
                self.alias(file, node, id, None, mapping, usage);
                return;
            }
            self.uncertain(
                file,
                node,
                "solve annotation identity is unresolved or ambiguous",
            );
            return;
        }
        if node.kind() == NodeKind::ArrayAccessExpression {
            self.annotation_selection(file, node, mapping, usage);
            return;
        }
        if node.kind() == NodeKind::ConditionalExpression {
            self.conditional_annotation(file, node, mapping, usage);
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
        if usage == AnnotationUse::Heuristic {
            self.uncertain(file, node, "search heuristic callable is unsupported");
            return;
        }
        let id = *declaration;
        let d = &self.bindings.declarations[id.0];
        if self.core(id) && matches!(d.name.as_str(), "warm_start" | "constraint_name") {
            if let Err(reason) = self.non_search_annotation(file, written, id, mapping) {
                self.uncertain(file, node, &reason);
            }
            return;
        }
        if self.core(id)
            && matches!(
                d.role,
                DeclarationRole::Annotation | DeclarationRole::Function
            )
        {
            if d.name == "seq_search" {
                if let Some((file, value)) = self.argument(file, node, id, 0) {
                    self.annotation(file, value, mapping, usage);
                } else {
                    self.uncertain(file, node, "seq_search arguments are unavailable");
                }
                return;
            }
            if matches!(
                d.name.as_str(),
                "int_search" | "bool_search" | "float_search" | "set_search"
            ) {
                if let Some((value_file, value)) = self.argument(file, node, id, 0) {
                    if usage == AnnotationUse::Candidate {
                        if !mapping.is_empty() {
                            self.uncertain(
                                value_file,
                                value,
                                "candidate searched formal source is unsupported",
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
                                (value_file, value, &[], true),
                                None,
                            )
                        {
                            self.uncertain(value_file, value, &reason);
                        }
                        for position in 1..4 {
                            if let Some((argument_file, argument)) =
                                self.argument(file, node, id, position)
                            {
                                self.annotation(
                                    argument_file,
                                    argument,
                                    mapping,
                                    AnnotationUse::Heuristic,
                                );
                            } else {
                                self.uncertain(
                                    file,
                                    node,
                                    "search heuristic argument is unavailable",
                                );
                            }
                        }
                    }
                    self.values(value_file, value, mapping);
                } else {
                    self.uncertain(file, node, "typed search values are unavailable");
                }
                return;
            }
        }
        self.alias(file, node, id, Some(node), mapping, usage);
    }
    fn conditional_annotation(
        &mut self,
        file: FileId,
        node: &'a SyntaxNode,
        mapping: &[Argument<'a>],
        usage: AnnotationUse,
    ) {
        if usage == AnnotationUse::Heuristic {
            self.uncertain(file, node, "conditional search heuristic is unsupported");
            return;
        }
        let checked = (|| {
            let typed = |value: &SyntaxNode, kind: TypeKind| {
                let range = self.context.files[file].location(value.range()).range;
                self.calls.expressions.iter().any(|e| {
                    e.file == file
                        && e.location.range == range
                        && e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.instantiation == Instantiation::Parameter
                        && e.ty.kind == kind
                })
            };
            let branches: Vec<_> = node.child_nodes().collect();
            let mut guards = Vec::new();
            let mut bodies = Vec::new();
            let mut complete = false;
            for (position, branch) in branches.iter().enumerate() {
                let parts: Vec<_> = branch.child_nodes().collect();
                let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                    if !typed(parts[0], TypeKind::Bool) {
                        return Err("conditional annotation guard type is unsupported");
                    }
                    guards.push(parts[0]);
                    parts[1]
                } else if branch.kind() == NodeKind::ElseBranch
                    && parts.len() == 1
                    && position + 1 == branches.len()
                {
                    complete = true;
                    parts[0]
                } else {
                    return Err("conditional annotation branch is unsupported");
                };
                if !typed(body, TypeKind::Annotation) {
                    return Err("conditional annotation branch type is unsupported");
                }
                bodies.push(body);
            }
            if !complete || guards.is_empty() || !typed(node, TypeKind::Annotation) {
                return Err("conditional annotation requires a present complete else");
            }
            Ok((guards, bodies))
        })();
        let (guards, bodies) = match checked {
            Ok(parts) => parts,
            Err(reason) => {
                self.uncertain(file, node, reason);
                return;
            }
        };
        for guard in guards {
            if let DefinitionSafety::Unsupported(reason) =
                self.reindex_safety(file, guard, mapping, Some(&[]))
            {
                self.uncertain(file, guard, &reason);
            }
        }
        let searched = self.facts.searched.len();
        let coverage: Vec<_> = self
            .facts
            .declarations
            .iter()
            .map(|declaration| declaration.coverage)
            .collect();
        let mut all_non_search = true;
        for body in bodies {
            let selected =
                operation_fact(self.context, self.calls, file, unwrap(body)).and_then(|call| {
                    match &call.outcome {
                        CallOutcome::Resolved { declaration, .. }
                            if self.core(*declaration)
                                && matches!(
                                    self.bindings.declarations[declaration.0].name.as_str(),
                                    "warm_start" | "constraint_name"
                                ) =>
                        {
                            Some(*declaration)
                        }
                        _ => None,
                    }
                });
            if let Some(id) = selected {
                match self.non_search_annotation(file, body, id, mapping) {
                    Ok(()) => continue,
                    Err(reason) => self.uncertain(file, body, &reason),
                }
            } else {
                self.annotation(
                    file,
                    body,
                    mapping,
                    if usage == AnnotationUse::Search {
                        AnnotationUse::Candidate
                    } else {
                        usage
                    },
                );
            }
            all_non_search = false;
        }
        // Inspect every alternative without choosing a branch or seeding its values.
        self.facts.searched.truncate(searched);
        for (declaration, coverage) in self.facts.declarations.iter_mut().zip(coverage) {
            declaration.coverage = coverage;
        }
        if !all_non_search {
            self.unknown_annotation = true;
        }
    }
    fn non_search_annotation(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        id: DeclarationId,
        mapping: &[Argument<'a>],
    ) -> Result<(), String> {
        let error = || "non-search annotation selected source or tuple is unsupported".to_owned();
        let mut source = node;
        loop {
            if !crate::definitions::annotations_safe(self.context, file, source) {
                return Err(error());
            }
            if !matches!(
                source.kind(),
                NodeKind::ParenthesizedExpression | NodeKind::AnnotatedExpression
            ) {
                break;
            }
            source = source.child_nodes().next().ok_or_else(error)?;
        }
        let node = unwrap(node);
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &operation_fact(self.context, self.calls, file, node)
            .ok_or_else(error)?
            .outcome
        else {
            return Err(error());
        };
        let typed = |value: &SyntaxNode| {
            let range = self.context.files[file].location(value.range()).range;
            self.calls
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        let present = |ty: &crate::TypeInst| ty.known() && !optional(ty);
        let parameter = |ty: &crate::TypeInst, kind: TypeKind| {
            present(ty) && ty.instantiation == Instantiation::Parameter && ty.kind == kind
        };
        let owner = &self.bindings.declarations[id.0];
        let arguments: Vec<_> = node.child_nodes().collect();
        if *declaration != id
            || !mapping.is_empty()
            || !parameter(return_type, TypeKind::Annotation)
            || typed(node) != Some(return_type)
            || arguments.len() != parameters.len()
            || arguments.iter().zip(parameters).any(|(actual, formal)| {
                actual.kind() == NodeKind::NamedArgument
                    || !present(formal)
                    || typed(actual).is_none_or(|ty| !present(ty) || ty != formal)
            })
        {
            return Err(error());
        }
        let shape = match (owner.name.as_str(), parameters.as_slice()) {
            ("constraint_name", [name]) => {
                owner.role == DeclarationRole::Annotation && parameter(name, TypeKind::String)
            }
            ("warm_start", [variables, values]) => {
                owner.role == DeclarationRole::Function
                    && variables.instantiation == Instantiation::Decision
                    && values.instantiation == Instantiation::Parameter
                    && matches!((&variables.kind, &values.kind),
                    (TypeKind::Array { indices: left, element: variable },
                     TypeKind::Array { indices: right, element: value })
                    if left == right && left.len() == 1
                        && present(&left[0]) && left[0].instantiation == Instantiation::Parameter
                        && matches!(left[0].kind, TypeKind::Int | TypeKind::Enum(_))
                        && variable.instantiation == Instantiation::Decision
                        && value.instantiation == Instantiation::Parameter
                        && variable.kind == value.kind
                        && matches!(&value.kind, TypeKind::Int | TypeKind::Enum(_) | TypeKind::Bool | TypeKind::Float
                            | TypeKind::Set(_) )
                        && match &value.kind {
                            TypeKind::Set(member) => parameter(member, TypeKind::Int)
                                || present(member) && member.instantiation == Instantiation::Parameter
                                    && matches!(member.kind, TypeKind::Enum(_)),
                            _ => true,
                        })
            }
            _ => false,
        };
        if !shape || !self.core(id) {
            return Err(error());
        }
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or_else(error)?;
        let lists: Vec<_> = written
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::ParameterList)
            .collect();
        if !matches!(lists.as_slice(), [list] if list.child_nodes().count() == parameters.len())
            || self
                .calls
                .signatures
                .iter()
                .find(|s| s.declaration == id)
                .is_none_or(|s| {
                    s.parameters.len() != parameters.len()
                        || s.parameters.iter().any(|p| p.has_default)
                        || s.return_type != *return_type
                })
        {
            return Err(error());
        }
        let mut parts = vec![written];
        while let Some(part) = parts.pop() {
            if !crate::definitions::annotations_safe(self.context, owner.file, part) {
                return Err(error());
            }
            if part.kind() == NodeKind::Annotation {
                continue;
            }
            if is_expression(part.kind()) || part.kind() == NodeKind::Error {
                return Err(error());
            }
            parts.extend(part.child_nodes());
        }
        for actual in &arguments {
            if let DefinitionSafety::Unsupported(reason) =
                crate::callable_definitions::initialized_expression_safety(
                    self.context,
                    self.bindings,
                    self.calls,
                    self.instantiations,
                    self.domains,
                    (file, actual, &[], true),
                    None,
                )
            {
                return Err(reason);
            }
        }
        if owner.name == "warm_start" {
            let axis = |actual: &SyntaxNode| {
                let actual = unwrap(actual);
                (actual.kind() == NodeKind::Expression)
                    .then(|| self.reference(file, actual))
                    .flatten()
                    .and_then(|id| {
                        match crate::domains::bare_index_domain(
                            &self.domains.declarations[id.0].domain,
                        ) {
                            crate::Domain::Array { indices, .. } if indices.len() == 1 => {
                                indices.first()
                            }
                            _ => None,
                        }
                    })
            };
            if let (Some(left), Some(right)) = (axis(arguments[0]), axis(arguments[1]))
                && crate::domains::same_members(left, right)? == Some(false)
            {
                return Err("warm_start arrays have incompatible index sets".into());
            }
        }
        Ok(())
    }
    fn annotation_selection(
        &mut self,
        file: FileId,
        node: &'a SyntaxNode,
        mapping: &[Argument<'a>],
        usage: AnnotationUse,
    ) {
        let checked = (|| {
            let typed = |value: &SyntaxNode| {
                let range = self.context.files[file].location(value.range()).range;
                self.calls
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .map(|e| &e.ty)
            };
            let present_annotation = |ty: &crate::TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && ty.kind == TypeKind::Annotation
            };
            let children: Vec<_> = node.child_nodes().collect();
            let [literal, selector] = children.as_slice() else {
                return Err("annotation selection requires one selector");
            };
            let literal = *literal;
            let selector = *selector;
            if literal.kind() != NodeKind::ArrayLiteral
                || selector.kind() != NodeKind::Expression
                || !crate::definitions::annotations_safe(self.context, file, node)
                || typed(node).is_none_or(|ty| !present_annotation(ty))
                || typed(literal).is_none_or(|ty| {
                    !ty.known()
                        || optional(ty)
                        || ty.instantiation != Instantiation::Parameter
                        || !matches!(&ty.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && indices[0].known()
                                && !optional(&indices[0])
                                && indices[0].instantiation == Instantiation::Parameter
                                && indices[0].kind == TypeKind::Int
                                && present_annotation(element))
                })
            {
                return Err("annotation selection source or type is unsupported");
            }
            let cells: Vec<_> = literal.child_nodes().collect();
            if cells.iter().any(|cell| {
                cell.kind() == NodeKind::IndexedArrayEntry
                    || typed(cell).is_none_or(|ty| !present_annotation(ty))
            }) {
                return Err("annotation selection literal cell is unsupported");
            }
            let selector_id = self
                .reference(file, selector)
                .ok_or("annotation selector identity is unavailable")?;
            let selector_type = typed(selector).ok_or("annotation selector type is unavailable")?;
            if !selector_type.known()
                || optional(selector_type)
                || selector_type.instantiation != Instantiation::Parameter
                || *selector_type != self.calls.declarations[selector_id.0].ty
                || !self.bindings.declarations[selector_id.0].top_level
            {
                return Err("annotation selector must be a present parameter enum");
            }
            let TypeKind::Enum(enum_id) = selector_type.kind else {
                return Err("annotation selector must retain its enum identity");
            };
            let declaration = &self.bindings.declarations[enum_id.0];
            if declaration.role != DeclarationRole::Enum {
                return Err("annotation selector enum declaration is unavailable");
            }
            let written = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("annotation selector enum source is unavailable")?;
            let definition = written
                .child_nodes()
                .find(|part| part.kind() == NodeKind::EnumDefinition)
                .ok_or("annotation selector enum alternatives are unavailable")?;
            let parts: Vec<_> = definition.child_nodes().collect();
            let [cases] = parts.as_slice() else {
                return Err("annotation selector requires closed enum cases");
            };
            if cases.kind() != NodeKind::EnumCases
                || cases.child_nodes().count() != cells.len()
                || cases.child_nodes().any(|case| {
                    case.kind() != NodeKind::EnumCase || case.child_nodes().next().is_some()
                })
            {
                return Err("annotation literal length must match closed enum cases");
            }
            Ok((cells, selector))
        })();
        let (cells, selector) = match checked {
            Ok(checked) => checked,
            Err(reason) => {
                self.uncertain(file, node, reason);
                return;
            }
        };
        if let DefinitionSafety::Unsupported(reason) =
            crate::callable_definitions::initialized_expression_safety(
                self.context,
                self.bindings,
                self.calls,
                self.instantiations,
                self.domains,
                (file, selector, &[], true),
                None,
            )
        {
            self.uncertain(file, selector, &reason);
        }
        let searched = self.facts.searched.len();
        let coverage: Vec<_> = if usage == AnnotationUse::Heuristic {
            // Heuristic inspection accepts no search calls and cannot seed values.
            Vec::new()
        } else {
            self.facts
                .declarations
                .iter()
                .map(|declaration| declaration.coverage)
                .collect()
        };
        for cell in cells {
            self.annotation(
                file,
                cell,
                mapping,
                if usage == AnnotationUse::Heuristic {
                    AnnotationUse::Heuristic
                } else {
                    AnnotationUse::Candidate
                },
            );
        }
        // Every possible cell was inspected, but none was selected. Keep source
        // refusals while restoring any coverage seeded before this selection.
        self.facts.searched.truncate(searched);
        for (declaration, coverage) in self.facts.declarations.iter_mut().zip(coverage) {
            declaration.coverage = coverage;
        }
        self.unknown_annotation = true;
    }
    fn alias(
        &mut self,
        file: FileId,
        node: &SyntaxNode,
        id: DeclarationId,
        call: Option<&'a SyntaxNode>,
        mapping: &[Argument<'a>],
        usage: AnnotationUse,
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
        if (usage != AnnotationUse::Search
            || unwrap(value).kind() == NodeKind::ArrayAccessExpression)
            && !crate::definitions::annotations_safe(self.context, d.file, declaration)
        {
            self.uncertain(file, node, "annotation alias source is unsupported");
            return;
        }
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
                if usage != AnnotationUse::Search
                    && (!crate::definitions::annotations_safe(self.context, d.file, parameter)
                        || parameter.child_nodes().any(|n| is_expression(n.kind())))
                {
                    self.uncertain(
                        file,
                        node,
                        "candidate annotation formal source is unsupported",
                    );
                    return;
                }
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
        self.annotation(value_file, value, &arguments, usage);
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
                    (self.calls, self.calls),
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
                    if let Some(reason) = match (&safety, &d.enforcement) {
                        (DefinitionSafety::Unsupported(reason), _)
                        | (_, DefinitionEnforcement::Unsupported(reason)) => Some(reason),
                        _ => None,
                    } {
                        self.facts.limitations.push(SourceDiagnostic {
                            location: d.location.clone(),
                            message: format!("search-coverage: direct definition safety or enforcement is unsupported: {reason}"),
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
            (self.calls, self.calls),
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
