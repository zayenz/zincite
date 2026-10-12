//! Retained source facts and checked inspection; body-aware coordination uses the interpreter.
use super::interpretation::CheckedSourceGraph;
use super::*;
mod arithmetic;
mod collections;
mod fallback;
mod numeric;
mod standard;

pub(super) struct SourceInspector<'a> {
    pub(super) context: &'a ModelContext,
    pub(super) bindings: &'a BindingFacts,
    pub(super) calls: &'a crate::callables::CallableView<'a>,
    pub(super) instantiations: &'a InstantiationFacts,
    pub(super) domains: &'a DomainFacts,
    pub(super) lookups: Option<DirectSafetyLookups<'a>>,
}
impl<'a> SourceInspector<'a> {
    pub(super) fn new(
        context: &'a ModelContext,
        bindings: &'a BindingFacts,
        calls: &'a crate::callables::CallableView<'a>,
        instantiations: &'a InstantiationFacts,
        domains: &'a DomainFacts,
        lookups: Option<DirectSafetyLookups<'a>>,
    ) -> Self {
        Self {
            context,
            bindings,
            calls,
            instantiations,
            domains,
            lookups,
        }
    }
}

pub(super) struct LookupIndexes {
    expressions: HashMap<(FileId, usize, usize), usize>,
    calls: HashMap<(FileId, usize), usize>,
    value_references: Vec<Vec<usize>>,
}
impl LookupIndexes {
    pub(super) fn new(
        context: &ModelContext,
        bindings: &BindingFacts,
        calls: &crate::callables::CallableView<'_>,
    ) -> Self {
        let mut call_indices = HashMap::with_capacity(calls.calls.len());
        for (row, call) in calls.calls.iter().enumerate() {
            call_indices
                .entry((call.file, call.location.range.start))
                .or_insert(row);
        }
        let mut expression_indices = HashMap::with_capacity(calls.expressions.len());
        for (row, expression) in calls.expressions.iter().enumerate() {
            expression_indices
                .entry((
                    expression.file,
                    expression.location.range.start,
                    expression.location.range.end,
                ))
                .or_insert(row);
        }
        let mut value_reference_indices = vec![Vec::new(); context.files.len()];
        for (row, reference) in bindings.references.iter().enumerate() {
            if reference.kind == ReferenceKind::Value {
                value_reference_indices[reference.file].push(row);
            }
        }
        for indices in &mut value_reference_indices {
            indices
                .sort_unstable_by_key(|&row| (bindings.references[row].location.range.start, row));
        }
        Self {
            expressions: expression_indices,
            calls: call_indices,
            value_references: value_reference_indices,
        }
    }
    pub(super) fn borrowed(&self) -> DirectSafetyLookups<'_> {
        DirectSafetyLookups {
            expressions: &self.expressions,
            calls: &self.calls,
            value_references: &self.value_references,
        }
    }
}
impl<'a> SourceInspector<'a> {
    pub(in crate::callable_definitions) fn reference(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<DeclarationId> {
        let range = self.context.files[file].location(node.range()).range;
        let reference = if let Some(lookups) = &self.lookups {
            let indices = &lookups.value_references[file];
            let first = indices.partition_point(|&row| {
                self.bindings.references[row].location.range.start < range.start
            });
            let last = indices.partition_point(|&row| {
                self.bindings.references[row].location.range.start <= range.end
            });
            indices[first..last]
                .iter()
                .copied()
                .filter(|&row| self.bindings.references[row].location.range.end <= range.end)
                .min()
                .map(|row| &self.bindings.references[row])
        } else {
            self.bindings.references.iter().find(|r| {
                r.file == file
                    && range.start <= r.location.range.start
                    && r.location.range.end <= range.end
                    && r.kind == ReferenceKind::Value
            })
        };
        reference.and_then(|r| match r.resolution {
            BindingResolution::Resolved(id) => Some(id),
            _ => None,
        })
    }
    pub(in crate::callable_definitions) fn expression_type<'b>(
        &'b self,
        facts: &crate::callables::CallableView<'b>,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<&'b ExpressionType> {
        let range = node.range();
        let offset = self.context.files[file].byte_offset;
        let range = range.start + offset..range.end + offset;
        if facts.same_observations(self.calls)
            && let Some(lookups) = &self.lookups
        {
            return lookups
                .expressions
                .get(&(file, range.start, range.end))
                .map(|&row| &facts.expressions[row]);
        }
        facts
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
    }
    pub(in crate::callable_definitions) fn operation_fact<'b>(
        &'b self,
        facts: &crate::callables::CallableView<'b>,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<&'b CallFact> {
        if facts.same_observations(self.calls)
            && let Some(lookups) = &self.lookups
        {
            let start = crate::callables::operation_head_start(self.context, file, node)?;
            return lookups
                .calls
                .get(&(file, start))
                .map(|&row| &facts.calls[row]);
        }
        operation_fact(self.context, facts, file, node)
    }
    pub(in crate::callable_definitions) fn view<'b, 'v>(
        &'b self,
        file: FileId,
        node: &SyntaxNode,
        view: &'v crate::callables::CallableView<'b>,
    ) -> &'v crate::callables::CallableView<'b> {
        if view.same_observations(self.calls) {
            return view;
        }
        let range = self.context.files[file].location(node.range()).range;
        if view
            .expressions
            .iter()
            .any(|e| e.file == file && e.location.range == range)
        {
            view
        } else {
            self.calls
        }
    }
    pub(in crate::callable_definitions) fn resolved(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &crate::callables::CallableView<'_>,
    ) -> Option<(DeclarationId, Vec<TypeInst>)> {
        match &self
            .operation_fact(self.view(file, node, view), file, node)?
            .outcome
        {
            CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } if return_type.kind == TypeKind::Bool && !optional(return_type) => {
                let mut body_parameters = parameters.clone();
                for (position, formal) in body_parameters.iter_mut().enumerate() {
                    if formal.instantiation != Instantiation::Decision || optional(formal) {
                        continue;
                    }
                    let Some((actual_file, actual)) = call_argument(
                        self.context,
                        self.bindings,
                        self.view(file, node, view),
                        file,
                        node,
                        *declaration,
                        position,
                    ) else {
                        continue;
                    };
                    let range = self.context.files[actual_file]
                        .location(actual.range())
                        .range;
                    let Some(ty) = self
                        .view(actual_file, actual, view)
                        .expressions
                        .iter()
                        .find(|e| e.file == actual_file && e.location.range == range)
                        .map(|e| &e.ty)
                    else {
                        continue;
                    };
                    let scalar = |kind: &TypeKind| {
                        matches!(
                            kind,
                            TypeKind::Bool
                                | TypeKind::Int
                                | TypeKind::Float
                                | TypeKind::String
                                | TypeKind::Enum(_)
                        )
                    };
                    let shape = scalar(&ty.kind)
                        || matches!(&ty.kind,
                        TypeKind::Array { indices, element } if scalar(&element.kind)
                            && indices.iter().all(|index| index.instantiation == Instantiation::Parameter));
                    if shape
                        && ty.known()
                        && !optional(ty)
                        && ty.instantiation == Instantiation::Parameter
                        && ty.clone().with_inst(Instantiation::Parameter) == *ty
                        && crate::types::coerces(ty, formal)
                    {
                        // Keep the selected coercion shape and identities. Only this
                        // body's instance sees the actual parameter qualifiers.
                        *formal = formal.clone().with_inst(Instantiation::Parameter);
                    }
                }
                Some((*declaration, body_parameters))
            }
            _ => None,
        }
    }
    pub(in crate::callable_definitions) fn core(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        name: &str,
    ) -> bool {
        let facts = self.view(file, node, view);
        if facts.same_observations(self.calls) && self.lookups.is_some() {
            return match self
                .operation_fact(facts, file, node)
                .map(|fact| &fact.outcome)
            {
                Some(CallOutcome::Resolved { declaration, .. }) => {
                    crate::definitions::core_callable(
                        self.context,
                        self.bindings,
                        *declaration,
                        name,
                    )
                }
                Some(CallOutcome::Intrinsic {
                    name: intrinsic, ..
                }) => {
                    name == "+" && intrinsic == "unary+" && node.kind() == NodeKind::UnaryExpression
                }
                _ => false,
            };
        }
        core_operation(self.context, self.bindings, facts, file, node, name).is_ok_and(|v| v)
    }
    pub(in crate::callable_definitions) fn callable_annotations_safe(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> bool {
        node.child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
            .all(|annotation| {
                let Some(value) = annotation.child_nodes().next().map(unwrap) else {
                    return false;
                };
                if value.kind() != NodeKind::Expression {
                    return false;
                }
                let tokens = crate::domains::tokens(&self.context.files[file].parsed, value);
                if tokens
                    .first()
                    .is_some_and(|t| t.kind == TokenKind::StringLiteral)
                {
                    return true;
                }
                // These standard hints permit body inspection; they supply no proof
                // of totality and do not bypass any body prerequisite.
                tokens.len() == 1
                    && self.reference(file, value).is_some_and(|id| {
                        let declaration = &self.bindings.declarations[id.0];
                        declaration.role == DeclarationRole::Annotation
                            && matches!(
                                declaration.name.as_str(),
                                "promise_total" | "promise_commutative"
                            )
                            && self.context.files[declaration.file].kind
                                == SourceKind::StandardLibrary
                            && self.context.files[declaration.file].implicit
                            && (declaration.name == "promise_total"
                                || find_node(
                                    self.context.files[declaration.file].parsed.tree(),
                                    &declaration.syntax_range,
                                    declaration.role,
                                )
                                .is_some_and(|written| {
                                    written.kind() == NodeKind::AnnotationDeclaration
                                        && written.child_nodes().next().is_none()
                                }))
                    })
            })
    }
    // Only an actual in-header can delegate its selected set source. The typed
    // helper retains required source errors and returns no membership facts.
    pub(in crate::callable_definitions) fn selected_generator_source_safety(
        &self,
        file: FileId,
        header: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        if header.kind() != NodeKind::Generator
            || !header.children().iter().any(|child| {
                matches!(child, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
            })
        {
            return None;
        }
        selected_integer_set_source_safety(
            self.context,
            self.bindings,
            (self.calls, view),
            self.instantiations,
            self.domains,
            (file, header.child_nodes().next()?, generators),
            None,
        )
        .map(|checked| match checked {
            unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
            _ => DefinitionSafety::Unknown("selected parameter set membership is unproved".into()),
        })
    }
    pub(in crate::callable_definitions) fn source_annotations_safe(
        &self,
        file: FileId,
        node: &SyntaxNode,
    ) -> bool {
        node.child_nodes()
            .filter(|n| n.kind() == NodeKind::Annotation)
            .all(|annotation| {
                let Some(value) = annotation.child_nodes().next() else {
                    return false;
                };
                if value.kind() != NodeKind::Expression {
                    return false;
                }
                let tokens = crate::domains::tokens(&self.context.files[file].parsed, value);
                if tokens
                    .first()
                    .is_some_and(|t| t.kind == TokenKind::StringLiteral)
                {
                    return true;
                }
                if tokens.len() != 1 {
                    return false;
                }
                let Some(id) = self.reference(file, value) else {
                    return false;
                };
                let declaration = &self.bindings.declarations[id.0];
                let source = &self.context.files[declaration.file];
                let annotation_type = TypeInst {
                    instantiation: Instantiation::Parameter,
                    optional: false,
                    kind: TypeKind::Annotation,
                };
                if declaration.name != "add_to_output"
                    || declaration.role != DeclarationRole::Annotation
                    || source.kind != SourceKind::StandardLibrary
                    || !source.implicit
                    || self.calls.declarations[id.0].ty != annotation_type
                {
                    return false;
                }
                let mut signatures = self.calls.signatures.iter().filter(|s| s.declaration == id);
                let Some(signature) = signatures.next() else {
                    return false;
                };
                if signatures.next().is_some()
                    || !signature.parameters.is_empty()
                    || signature.return_type != annotation_type
                {
                    return false;
                }
                // This checked atomic annotation permits source inspection only;
                // values, domains and output coverage still need their own proof.
                find_node(
                    source.parsed.tree(),
                    &declaration.syntax_range,
                    declaration.role,
                )
                .is_some_and(|written| {
                    written.kind() == NodeKind::AnnotationDeclaration
                        && written.child_nodes().next().is_none()
                })
            })
    }
}

impl<'a> BodyInterpreter<'a, '_> {
    // Inspection only: a bare source dependency is not proof that its written
    // initializer, domains and transitive initialized sources are supported.
    pub(in crate::callable_definitions) fn initialized_source_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> DefinitionSafety {
        let mut safety = DefinitionSafety::Supported;
        let mut contributions = Vec::new();
        let mut nodes = vec![node];
        while let Some(current) = nodes.pop() {
            if !self.source.source_annotations_safe(file, current) {
                return DefinitionSafety::Unsupported(
                    "initialized source annotation is unsupported".into(),
                );
            }

            if current.kind() == NodeKind::Generator
                && current
                    .child_nodes()
                    .next()
                    .is_some_and(|source| unwrap(source).kind() == NodeKind::ArrayAccessExpression)
                && let Some(scope) = generator_source_scope(node, current, generators)
                && let Some(checked) = self
                    .source
                    .selected_generator_source_safety(file, current, view, &scope)
            {
                match checked {
                    unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                    unknown => safety = unknown,
                }
                // The delegated source was fully inspected. Keep its attached
                // filters in the ordinary strict scan.
                nodes.extend(current.child_nodes().skip(1));
                continue;
            }
            nodes.extend(current.child_nodes());
            if current.kind() != NodeKind::Expression {
                continue;
            }
            let Some(id) = self.source.reference(file, current) else {
                continue;
            };
            let declaration = &self.source.bindings.declarations[id.0];
            let ty = &self.source.view(file, current, view).declarations[id.0].ty;
            let local_boolean = declaration.file == file
                && declaration.role == DeclarationRole::Local
                && self.source.context.files[file].kind == SourceKind::User
                && self.source.context.files[file]
                    .parsed
                    .tree()
                    .child_nodes()
                    .nth(declaration.item)
                    .is_some_and(|item| item.kind() == NodeKind::Constraint)
                && ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0] == TypeInst::par(TypeKind::Int)
                        && **element == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision));
            if !local_boolean
                && (!declaration.top_level
                    || !matches!(
                        declaration.role,
                        DeclarationRole::Value | DeclarationRole::Enum
                    ))
            {
                continue;
            }
            if active.contains(&id) {
                return DefinitionSafety::Unsupported("cyclic initialized source".into());
            }
            let ty = &self.source.calls.declarations[id.0].ty;
            if !ty.known() || optional(ty) {
                return DefinitionSafety::Unsupported(
                    "initialized source type or optionality is unsupported".into(),
                );
            }
            if !local_boolean
                && let Some(graph) = &self.checked_source_graph
                && graph.file == file
                && graph.view.same_observations(view)
                && graph.node.range().start <= current.range().start
                && current.range().end <= graph.node.range().end
                && let Some((_, contribution)) = graph
                    .contributions
                    .binary_search_by_key(&std::ptr::from_ref(current), |(reference, _)| {
                        std::ptr::from_ref(*reference)
                    })
                    .ok()
                    .map(|position| &graph.contributions[position])
            {
                // Preserve this physical reference's exact canonical contribution.
                // Local scopes, new source trees and substituted views inspect fully.
                match contribution {
                    unsupported @ DefinitionSafety::Unsupported(_) => return unsupported.clone(),
                    unknown @ DefinitionSafety::Unknown(_) => safety = unknown.clone(),
                    DefinitionSafety::Supported => {}
                }
                contributions.push((current, contribution.clone()));
                continue;
            }
            let Some(written) = find_node(
                self.source.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return DefinitionSafety::Unsupported(
                    "initialized source declaration unavailable".into(),
                );
            };
            let mut contribution = DefinitionSafety::Supported;
            if local_boolean {
                let initializers: Vec<_> = written
                    .child_nodes()
                    .filter(|value| is_expression(value.kind()))
                    .collect();
                let [initializer] = initializers.as_slice() else {
                    return DefinitionSafety::Unsupported(
                        "scoped Boolean source initializer is unavailable".into(),
                    );
                };
                if unwrap(initializer).kind() != NodeKind::ArrayComprehension {
                    return DefinitionSafety::Unsupported(
                        "scoped Boolean source requires an inspected comprehension".into(),
                    );
                }
                if let Some(reason) = opaque_source_error(
                    self.source.context,
                    self.source.bindings,
                    view,
                    self.source.instantiations,
                    self.source.domains,
                    file,
                    written,
                ) {
                    return DefinitionSafety::Unsupported(reason);
                }
                safety = DefinitionSafety::Unknown(
                    "scoped Boolean array values and extent are unproved".into(),
                );
            }

            active.push(id);
            let mut expressions: Vec<_> = written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .collect();
            let mut types: Vec<_> = written.child_nodes().take(1).collect();
            let mut unsupported = None;
            if declaration.role == DeclarationRole::Enum {
                types.clear();
                let mut parts = vec![written];
                while let Some(part) = parts.pop() {
                    if !self.source.source_annotations_safe(declaration.file, part) {
                        unsupported = Some("enum source annotation is unsupported".into());
                    }
                    if matches!(
                        part.kind(),
                        NodeKind::EnumConstructor | NodeKind::CallExpression
                    ) {
                        let tokens = crate::domains::tokens(
                            &self.source.context.files[declaration.file].parsed,
                            part,
                        );
                        let name = tokens.first().map(|token| {
                            self.source.context.files[declaration.file].parsed.source()
                                [token.range.clone()]
                            .trim_matches('\'')
                        });
                        let arguments: Vec<_> = part.child_nodes().collect();
                        if name != Some("anon_enum") || arguments.len() != 1 {
                            unsupported = Some("enum source constructor is unsupported".into());
                            continue;
                        }
                        let count = unwrap(arguments[0]);
                        if self
                            .source
                            .expression_type(self.source.calls, declaration.file, count)
                            .is_none_or(|e| {
                                !e.ty.known()
                                    || optional(&e.ty)
                                    || e.ty.instantiation != Instantiation::Parameter
                                    || e.ty.kind != TypeKind::Int
                            })
                        {
                            unsupported = Some("anonymous enum count type is unsupported".into());
                        }
                        if let Some(reason) = self.source.closed_integer_source_error(
                            declaration.file,
                            count,
                            false,
                            false,
                        ) {
                            unsupported = Some(reason);
                        }
                        if crate::domains::invariant_expression_integer(
                            self.source.context,
                            self.source.bindings,
                            declaration.file,
                            count,
                        )
                        .is_ok_and(|value| value.is_some_and(|value| value < 0))
                        {
                            unsupported = Some("anonymous enum count is negative".into());
                        }
                        expressions.push(count);
                        contribution =
                            DefinitionSafety::Unknown("enum constructor extent is unproved".into());
                        safety = contribution.clone();
                    } else {
                        parts.extend(part.child_nodes());
                    }
                }
            }
            if !self
                .source
                .source_annotations_safe(declaration.file, written)
            {
                unsupported =
                    Some("initialized source declaration annotation is unsupported".to_owned());
            }
            while let Some(ty) = types.pop() {
                if !self.source.source_annotations_safe(declaration.file, ty) {
                    unsupported =
                        Some("initialized source written type annotation is unsupported".into());
                }
                if ty.kind() == NodeKind::DomainType {
                    expressions.extend(ty.child_nodes());
                } else {
                    types.extend(ty.child_nodes());
                }
            }
            for value in expressions {
                match self.initialized_source_safety(
                    declaration.file,
                    value,
                    if local_boolean {
                        view
                    } else {
                        self.source.calls
                    },
                    if local_boolean { generators } else { &[] },
                    active,
                ) {
                    DefinitionSafety::Unsupported(reason) => unsupported = Some(reason),
                    unknown @ DefinitionSafety::Unknown(_) => {
                        contribution = unknown.clone();
                        safety = unknown;
                    }
                    DefinitionSafety::Supported => {}
                }
            }
            active.pop();
            if let Some(reason) = unsupported {
                return DefinitionSafety::Unsupported(reason);
            }
            if !local_boolean {
                contributions.push((current, contribution));
            }
        }
        // Check the reference graph first: aggregate inspection can recursively
        // visit a source, so its written cycle must already have been rejected.
        // This table is lookup-only; the source walk above retains its reduction order.
        contributions.sort_unstable_by_key(|(reference, _)| std::ptr::from_ref(*reference));
        let inspected = BodyInterpreter {
            source: self.source,
            instances: self.instances,
            boundaries: self.boundaries,
            checked_source_graph: Some(CheckedSourceGraph {
                file,
                node,
                view,
                contributions: &contributions,
            }),
        };
        let selected = unwrap(node);
        let strict_set = self
            .source
            .expression_type(self.source.view(file, selected, view), file, selected)
            .is_some_and(|value| decision_integer_set(&value.ty))
            && (selected.kind() == NodeKind::GeneratorCallExpression
                && self.source.core(file, selected, view, "array_union")
                || selected.kind() == NodeKind::BinaryExpression
                    && self.source.core(file, selected, view, "intersect"));
        let checked = if strict_set {
            inspected
                .decision_set_source_safety(file, node, view)
                .unwrap_or_else(|| {
                    DefinitionSafety::Unsupported("decision set source type is unsupported".into())
                })
        } else if unwrap(node).kind() == NodeKind::MatrixLiteral {
            // Preserve the existing literal cell checker after transitive source
            // and annotation inspection, without adding a matrix evaluator.
            crate::expression_safety_view(
                self.source.context,
                self.source.bindings,
                view,
                file,
                unwrap(node),
            )
        } else {
            inspected.direct_safety(file, node, view, generators)
        };
        match checked {
            DefinitionSafety::Supported => safety,
            checked => checked,
        }
    }
    pub(in crate::callable_definitions) fn initialized_children_safety(
        &self,
        file: FileId,
        nodes: &[&'a SyntaxNode],
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let mut safety = DefinitionSafety::Supported;
        for node in nodes {
            match self.initialized_source_safety(file, node, view, generators, &mut Vec::new()) {
                unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                DefinitionSafety::Supported => {}
            }
        }
        safety
    }
    pub(in crate::callable_definitions) fn direct_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        if let Some(safety) = self.native_fixed_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.native_dom_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.native_generator_max_source_safety(file, node, view, generators)
        {
            return safety;
        }
        if let Some(safety) = self.integer_product_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.integer_array_set_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.floor_sum_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) =
            self.parameter_integer_array_value_safety(file, node, view, generators, false)
        {
            return safety;
        }
        if let Some(safety) = self.parameter_float_safety(file, node, view, generators) {
            return safety;
        }

        if let Some(safety) = self.full_axis_slice_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(arguments) = self
            .source
            .set_axis_integer_reshape_arguments(file, node, view, true)
        {
            let mut unsupported = None;
            for argument in arguments {
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    file,
                    argument,
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
            }
            return match unsupported {
                Some(reason) => DefinitionSafety::Unsupported(reason),
                None => DefinitionSafety::Unknown("array reshape cardinality is unproved".into()),
            };
        }
        if let Some(safety) = self.inline_reshape_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.integer_array_bound_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.decision_scalar_bound_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.native_integer_flat_backing_safety(file, node, view) {
            return safety;
        }
        if let Some(safety) = self.native_integer_flat_cell_safety(file, node, view, generators) {
            return safety;
        }
        // Let prerequisites still evaluate before a strict result can be safe.
        if unwrap(node).kind() == NodeKind::LetExpression
            && let Some(reason) = opaque_source_error(
                self.source.context,
                self.source.bindings,
                view,
                self.source.instantiations,
                self.source.domains,
                file,
                node,
            )
        {
            return DefinitionSafety::Unsupported(reason);
        }
        // A strict dependency on a named collection does not inspect its
        // initializer. Keep that evaluation check ahead of strict success.
        let aggregate = self.aggregate_source_safety(file, node, view, generators);
        let reason = match self.dependencies(file, node, view, generators) {
            Ok(_) => return aggregate.unwrap_or(DefinitionSafety::Supported),
            Err(reason) => reason,
        };
        if let Some(safety) = self.parameter_enum_set_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.decision_set_source_safety(file, node, view) {
            return safety;
        }

        if unwrap(node).kind() == NodeKind::LetExpression {
            return match opaque_source_error(
                self.source.context,
                self.source.bindings,
                view,
                self.source.instantiations,
                self.source.domains,
                file,
                node,
            ) {
                Some(reason) => DefinitionSafety::Unsupported(reason),
                None => crate::value_safety::opaque_expression_safety(
                    self.source.context,
                    self.source.bindings,
                    view,
                    file,
                    node,
                ),
            };
        }
        if let Some(safety) = self.parameter_set_extremum_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.parameter_enum_conversion_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.parameter_enum_successor_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = aggregate {
            return if safety == DefinitionSafety::Supported
                && !self.source.core(file, unwrap(node), view, "sum")
            {
                DefinitionSafety::Unknown("initialized collection cardinality is unproved".into())
            } else {
                safety
            };
        }
        let checked = self.fallback_source_safety(file, node, view, generators, reason);
        if !matches!(checked, DefinitionSafety::Unsupported(_)) {
            return checked;
        }
        if unwrap(node).kind() == NodeKind::CallExpression
            && self.source.operation_fact(view, file, unwrap(node)).is_some_and(|call| {
                matches!(call.outcome, CallOutcome::Resolved { declaration, .. }
                    if self.source.context.files[self.source.bindings.declarations[declaration.0].file].kind == SourceKind::User)
            }) {
            self.opaque_source_safety(file, node, view, generators)
        } else {
            checked
        }
    }
    pub(in crate::callable_definitions) fn opaque_source_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        for value in generators
            .iter()
            .flat_map(|generator| generator.child_nodes())
            .chain(std::iter::once(node))
        {
            if let Some(reason) = opaque_source_error(
                self.source.context,
                self.source.bindings,
                view,
                self.source.instantiations,
                self.source.domains,
                file,
                value,
            ) {
                return DefinitionSafety::Unsupported(reason);
            }
        }
        crate::value_safety::opaque_expression_safety(
            self.source.context,
            self.source.bindings,
            view,
            file,
            node,
        )
    }
    pub(in crate::callable_definitions) fn direct_children_safety(
        &self,
        file: FileId,
        nodes: &[&'a SyntaxNode],
        view: &crate::callables::CallableView<'_>,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let mut safety = DefinitionSafety::Supported;
        for node in nodes {
            match self.direct_safety(file, node, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                DefinitionSafety::Supported => {}
            }
        }
        safety
    }
}
