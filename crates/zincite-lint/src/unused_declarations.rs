//! Declaration reachability and containment, independent of unused advice.
use std::collections::HashMap;

use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    FileFinding, FileId, Instantiation, ModelContext, Reference, ReferenceKind, Rule, Severity,
    SourceDiagnostic, SourceKind, SourceLocation, TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageOutcome {
    Reachable,
    Unreachable,
    Uncertain,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelRootState {
    Complete,
    Fragment,
    Incomplete,
}
#[derive(Debug)]
pub struct DeclarationUsage {
    pub declaration: DeclarationId,
    pub file: FileId,
    pub item: usize,
    pub location: SourceLocation,
    pub enclosing: Option<DeclarationId>,
    pub dependencies: Vec<DeclarationId>,
    pub possible_dependencies: Vec<DeclarationId>,
    pub outcome: UsageOutcome,
}
#[derive(Debug)]
pub struct UsageFacts {
    pub root_state: ModelRootState,
    pub roots: Vec<DeclarationId>,
    pub possible_roots: Vec<DeclarationId>,
    pub declarations: Vec<DeclarationUsage>,
    pub limitations: Vec<SourceDiagnostic>,
}

/// Follow declaration IDs from model-level constraints, solve and output.
/// Prerequisites belong to the same retained context. Containment never implies
/// use. Fragments cannot establish export unreachability. Unknown reachable
/// dependencies stay explicit, including possible overloads and output roots.
/// Default output follows MiniZinc 2.10.1's confirmed source annotation contract,
/// without evaluating data or consulting the filesystem after model loading.
pub fn resolve_unused_declarations(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
) -> UsageFacts {
    let mut main_call_indices = HashMap::with_capacity(calls.calls.len());
    for (row, call) in calls.calls.iter().enumerate() {
        main_call_indices
            .entry((call.file, call.location.range.start))
            .or_insert(row);
    }
    let mut producer = Producer {
        context,
        bindings,
        calls,
        main_call_indices: &main_call_indices,
        facts: UsageFacts {
            root_state: ModelRootState::Complete,
            roots: Vec::new(),
            possible_roots: Vec::new(),
            declarations: Vec::new(),
            limitations: Vec::new(),
        },
        uncertain: Vec::new(),
        demanded: Vec::new(),
        bodies: Vec::new(),
    };
    producer.collect();
    producer.entry_points();
    producer.follow();
    producer.facts
}
struct Uncertainty {
    owner: Option<DeclarationId>,
    diagnostic: SourceDiagnostic,
    unbounded: bool,
}
struct BodyDemand {
    owner: Option<DeclarationId>,
    declaration: DeclarationId,
    parameters: Option<Vec<TypeInst>>,
    ancestry: Vec<DeclarationId>,
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    main_call_indices: &'a HashMap<(FileId, usize), usize>,
    facts: UsageFacts,
    uncertain: Vec<Uncertainty>,
    demanded: Vec<BodyDemand>,
    bodies: Vec<(DeclarationId, Option<Vec<TypeInst>>)>,
}
impl Producer<'_> {
    fn collect(&mut self) {
        for d in &self.bindings.declarations {
            let enclosing = self
                .bindings
                .declarations
                .iter()
                .filter(|outer| {
                    outer.file == d.file
                        && outer.syntax_range != d.syntax_range
                        && outer.syntax_range.start <= d.syntax_range.start
                        && d.syntax_range.end <= outer.syntax_range.end
                })
                .min_by_key(|outer| outer.syntax_range.len())
                .map(|outer| outer.id);
            self.facts.declarations.push(DeclarationUsage {
                declaration: d.id,
                file: d.file,
                item: d.item,
                location: d.location.clone(),
                enclosing,
                dependencies: Vec::new(),
                possible_dependencies: Vec::new(),
                outcome: UsageOutcome::Unreachable,
            });
        }
        for d in &self.bindings.declarations {
            if matches!(
                d.role,
                DeclarationRole::EnumMember | DeclarationRole::EnumConstructor
            ) && let Some(enclosing) = self.facts.declarations[d.id.0].enclosing
            {
                self.edge(Some(d.id), enclosing, false);
            }
        }
        for reference in &self.bindings.references {
            if reference.callable.is_none() {
                self.collect_reference(reference, self.calls, Some(self.main_call_indices), &[]);
            }
        }
    }
    fn collect_reference(
        &mut self,
        reference: &Reference,
        view: &CallableFacts,
        call_indices: Option<&HashMap<(FileId, usize), usize>>,
        ancestry: &[DeclarationId],
    ) -> Vec<Option<DeclarationId>> {
        let source = &self.context.files[reference.file];
        let position = reference.location.range.start - source.byte_offset;
        let owner = self
            .bindings
            .declarations
            .iter()
            .filter(|d| d.file == reference.file && d.syntax_range.contains(&position))
            .min_by_key(|d| d.syntax_range.len())
            .map(|d| d.id);
        let item = source
            .parsed
            .tree()
            .child_nodes()
            .nth(reference.item)
            .unwrap();
        let owner = owner.or_else(|| {
            if item.kind() != NodeKind::Assignment {
                return None;
            }
            self.bindings
                .references
                .iter()
                .find(|r| r.file == reference.file && r.item == reference.item)
                .and_then(|r| {
                    if let BindingResolution::Resolved(id) = r.resolution {
                        Some(id)
                    } else {
                        None
                    }
                })
        });
        if owner.is_none() && !entry_item(item.kind()) {
            return Vec::new();
        }
        let mut owners = vec![owner];
        if let Some(id) = owner {
            let d = &self.bindings.declarations[id.0];
            if matches!(
                d.role,
                DeclarationRole::Parameter
                    | DeclarationRole::Generator
                    | DeclarationRole::Index
                    | DeclarationRole::EnumMember
                    | DeclarationRole::EnumConstructor
            ) {
                owners.push(self.facts.declarations[id.0].enclosing);
                for sibling in &self.bindings.declarations {
                    if sibling.file == d.file
                        && sibling.syntax_range == d.syntax_range
                        && sibling.id != id
                    {
                        owners.push(Some(sibling.id));
                    }
                }
            }
        }
        let mut known = Vec::new();
        let mut possible = Vec::new();
        let mut uncertainty = None;
        let mut parameters = None;
        if reference.kind == ReferenceKind::Callable {
            let call = if let Some(indices) = call_indices {
                indices
                    .get(&(reference.file, reference.location.range.start))
                    .map(|row| &view.calls[*row])
            } else {
                view.calls.iter().find(|call| {
                    call.file == reference.file
                        && call.location.range.start == reference.location.range.start
                })
            };
            match call.map(|c| &c.outcome) {
                Some(CallOutcome::Resolved {
                    declaration,
                    parameters: types,
                    ..
                }) => {
                    known.push(*declaration);
                    if types.iter().all(TypeInst::known) {
                        parameters = Some(types.clone());
                    }
                }
                Some(CallOutcome::Intrinsic { .. }) => {}
                Some(CallOutcome::Ambiguous { candidates }) => {
                    possible.extend(candidates);
                    uncertainty = Some("callable selection is ambiguous".to_owned());
                }
                Some(CallOutcome::Unsupported { reason, candidates }) => {
                    possible.extend(candidates);
                    uncertainty = Some(reason.clone());
                }
                Some(CallOutcome::Unresolved { reason } | CallOutcome::NoMatch { reason }) => {
                    uncertainty = Some(reason.clone());
                }
                None => uncertainty = Some("callable interpretation is unavailable".into()),
            }
            if uncertainty.is_some() {
                match &reference.resolution {
                    BindingResolution::Resolved(id) => push_unique(&mut possible, *id),
                    BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) => {
                        for &id in ids {
                            push_unique(&mut possible, id);
                        }
                    }
                    BindingResolution::Unresolved => {}
                }
            }
        } else {
            match &reference.resolution {
                BindingResolution::Resolved(id) => known.push(*id),
                BindingResolution::Overloads(ids) | BindingResolution::Ambiguous(ids) => {
                    possible.extend(ids);
                    uncertainty = Some("reference has possible declarations".into());
                }
                BindingResolution::Unresolved => {
                    uncertainty = Some("reference is unresolved".into())
                }
            }
        }
        for &owner in &owners {
            for &id in &known {
                self.edge(owner, id, false);
                if matches!(
                    self.bindings.declarations[id.0].role,
                    DeclarationRole::Function
                        | DeclarationRole::Predicate
                        | DeclarationRole::Test
                        | DeclarationRole::Annotation
                ) {
                    self.demanded.push(BodyDemand {
                        owner,
                        declaration: id,
                        parameters: parameters.clone(),
                        ancestry: ancestry.to_vec(),
                    });
                }
            }
            for &id in &possible {
                self.edge(owner, id, true);
                if matches!(
                    self.bindings.declarations[id.0].role,
                    DeclarationRole::Function
                        | DeclarationRole::Predicate
                        | DeclarationRole::Test
                        | DeclarationRole::Annotation
                ) {
                    self.demanded.push(BodyDemand {
                        owner,
                        declaration: id,
                        parameters: None,
                        ancestry: ancestry.to_vec(),
                    });
                }
            }
            if let Some(reason) = &uncertainty {
                self.uncertain.push(Uncertainty {
                    owner,
                    diagnostic: SourceDiagnostic {
                        location: reference.location.clone(),
                        message: format!("unused-declaration: {reason} for '{}'", reference.name),
                    },
                    unbounded: possible.is_empty(),
                });
            }
        }
        owners
    }
    fn body_dependencies(
        &mut self,
        owner: Option<DeclarationId>,
        state: UsageOutcome,
        pending: &mut Vec<(DeclarationId, UsageOutcome)>,
    ) {
        while let Some(position) = self.demanded.iter().position(|d| d.owner == owner) {
            let mut demand = self.demanded.remove(position);
            if state == UsageOutcome::Uncertain {
                demand.parameters = None;
            }
            let id = demand.declaration;
            if self.bodies.contains(&(id, demand.parameters.clone())) {
                continue;
            }
            if demand.parameters.is_some() && demand.ancestry.contains(&id) {
                self.uncertain.push(Uncertainty {
                    owner: Some(id),
                    unbounded: false,
                    diagnostic: SourceDiagnostic {
                        location: self.bindings.declarations[id.0].location.clone(),
                        message:
                            "unused-declaration: changing recursive body types are unsupported"
                                .into(),
                    },
                });
                demand.parameters = None;
                if self.bodies.contains(&(id, None)) {
                    continue;
                }
            }
            self.bodies.push((id, demand.parameters.clone()));
            let declaration = &self.bindings.declarations[id.0];
            let node = crate::callables::find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            );
            if node.is_none_or(|n| {
                !n.child_nodes()
                    .any(|n| crate::callables::is_expression(n.kind()))
            }) {
                continue;
            }
            let view = demand.parameters.as_ref().map(|types| {
                crate::callables::instantiated_body(
                    self.context,
                    self.bindings,
                    self.calls,
                    id,
                    types,
                )
            });
            let (view, call_indices) = match view.as_ref() {
                Some(view) => (view, None),
                None => (self.calls, Some(self.main_call_indices)),
            };
            demand.ancestry.push(id);
            for reference in self
                .bindings
                .references
                .iter()
                .filter(|r| r.callable == Some(id))
            {
                for changed in self
                    .collect_reference(reference, view, call_indices, &demand.ancestry)
                    .into_iter()
                    .flatten()
                {
                    let usage = &self.facts.declarations[changed.0];
                    if usage.outcome != UsageOutcome::Unreachable {
                        pending.push((changed, usage.outcome));
                        pending.extend(usage.dependencies.iter().map(|&d| (d, usage.outcome)));
                        pending.extend(
                            usage
                                .possible_dependencies
                                .iter()
                                .map(|&d| (d, UsageOutcome::Uncertain)),
                        );
                    }
                }
            }
        }
    }
    fn edge(&mut self, owner: Option<DeclarationId>, target: DeclarationId, possible: bool) {
        let out = if let Some(owner) = owner {
            let usage = &mut self.facts.declarations[owner.0];
            if possible {
                &mut usage.possible_dependencies
            } else {
                &mut usage.dependencies
            }
        } else if possible {
            &mut self.facts.possible_roots
        } else {
            &mut self.facts.roots
        };
        push_unique(out, target);
    }
    fn entry_points(&mut self) {
        let mut solves = Vec::new();
        let mut output = false;
        for source in &self.context.files {
            if source.kind != SourceKind::User {
                continue;
            }
            for node in source.parsed.tree().child_nodes() {
                if matches!(
                    node.kind(),
                    NodeKind::Solve | NodeKind::SolveMinimize | NodeKind::SolveMaximize
                ) {
                    solves.push(source.location(node.range()));
                }
                output |= node.kind() == NodeKind::Output;
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
        if solves.len() > 1 {
            self.facts.limitations.push(SourceDiagnostic {
                location: solves[1].clone(),
                message: "unused-declaration: multiple solve items prevent complete-root analysis"
                    .into(),
            });
        }
        if self.facts.root_state != ModelRootState::Complete || output {
            return;
        }
        let mut automatic = Vec::new();
        let mut forced = Vec::new();
        let mut supplement = Vec::new();
        let mut output_unknown = false;
        for d in &self.bindings.declarations {
            if !d.top_level
                || d.role != DeclarationRole::Value
                || self.context.files[d.file].kind != SourceKind::User
            {
                continue;
            }
            let Some(node) = crate::callables::find_node(
                self.context.files[d.file].parsed.tree(),
                &d.syntax_range,
                d.role,
            ) else {
                continue;
            };
            let mut hidden = false;
            let mut added = false;
            for annotation in node
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::Annotation)
            {
                match self.output_marker(d.file, annotation) {
                    Ok("no_output") => hidden = true,
                    Ok("add_to_output") => {
                        added = true;
                        push_unique(&mut forced, d.id);
                    }
                    Ok("output") => push_unique(&mut supplement, d.id),
                    Ok(_) => {}
                    Err(reason) => {
                        output_unknown = true;
                        self.uncertain.push(Uncertainty {
                            owner: None,
                            unbounded: false,
                            diagnostic: SourceDiagnostic {
                                location: self.context.files[d.file].location(annotation.range()),
                                message: format!("unused-declaration: {reason}"),
                            },
                        });
                    }
                }
            }
            if hidden && added {
                output_unknown = true;
                self.uncertain.push(Uncertainty {
                    owner: None,
                    unbounded: false,
                    diagnostic: SourceDiagnostic {
                        location: d.location.clone(),
                        message: "unused-declaration: conflicting default-output markers".into(),
                    },
                });
            }
            let ty = &self.calls.declarations[d.id.0].ty;
            if ty.instantiation == Instantiation::Unknown || !ty.known() {
                output_unknown = true;
                self.uncertain.push(Uncertainty {
                    owner: None,
                    unbounded: false,
                    diagnostic: SourceDiagnostic {
                        location: d.location.clone(),
                        message: "unused-declaration: default-output instantiation is unknown"
                            .into(),
                    },
                });
                continue;
            }
            if hidden || ty.instantiation != Instantiation::Decision {
                continue;
            }
            let initializer = node
                .child_nodes()
                .find(|n| crate::callables::is_expression(n.kind()));
            if initializer.is_none() {
                automatic.push(d.id);
            } else if let Some(initializer) = initializer {
                if matches!(ty.kind, TypeKind::Array { .. })
                    && matches!(
                        initializer.kind(),
                        NodeKind::ArrayLiteral | NodeKind::MatrixLiteral
                    )
                    && contains_anonymous(self.context, d.file, initializer)
                {
                    automatic.push(d.id);
                } else if contains_anonymous(self.context, d.file, initializer) {
                    output_unknown = true;
                    self.uncertain.push(Uncertainty {
                        owner: None,
                        unbounded: false,
                        diagnostic: SourceDiagnostic {
                            location: d.location.clone(),
                            message:
                                "unused-declaration: anonymous initializer output is unsupported"
                                    .into(),
                        },
                    });
                }
            }
        }
        let normal = if forced.is_empty() { automatic } else { forced };
        for id in normal.into_iter().chain(supplement) {
            self.edge(None, id, false);
        }
        if output_unknown {
            for d in &self.bindings.declarations {
                if d.top_level
                    && d.role == DeclarationRole::Value
                    && self.context.files[d.file].kind == SourceKind::User
                {
                    self.edge(None, d.id, true);
                }
            }
        }
    }
    fn output_marker(&self, file: FileId, annotation: &SyntaxNode) -> Result<&'static str, String> {
        let atom = annotation
            .child_nodes()
            .next()
            .ok_or("annotation expression is unavailable")?;
        let reserved = atom.children().iter().any(|c| {
            matches!(c,SyntaxElement::Token(i)
            if self.context.files[file].parsed.tokens()[*i].kind==TokenKind::Output)
        });
        if reserved {
            let supported = self.bindings.declarations.iter().any(|d| {
                d.name == "output"
                    && d.role == DeclarationRole::Function
                    && self.core(d.id)
                    && crate::callables::find_node(self.context.files[d.file].parsed.tree(), &d.syntax_range, d.role).is_some_and(|n| {
                        n.child_nodes().filter(|c| c.kind() == NodeKind::ParameterList).flat_map(|c| c.child_nodes()).any(|parameter| {
                            parameter.child_nodes().filter(|a| a.kind() == NodeKind::Annotation).any(|annotation| {
                                self.bindings.references.iter().any(|r| r.file == d.file && self.context.files[d.file].location(annotation.range()).range.contains(&r.location.range.start) && r.name == "annotated_expression" && matches!(r.resolution, BindingResolution::Resolved(id) if self.core(id)))
                            })
                        })
                    })
                    && self.calls.signatures.iter().any(|s| {
                        s.declaration == d.id
                            && s.parameters.len() == 1
                            && s.return_type.kind == TypeKind::Annotation
                    })
            });
            return if supported {
                Ok("output")
            } else {
                Err("reserved output annotation identity is unavailable".into())
            };
        }
        let location = self.context.files[file].location(atom.range());
        let reference = self
            .bindings
            .references
            .iter()
            .find(|r| {
                r.file == file
                    && location.range.start <= r.location.range.start
                    && r.location.range.end <= location.range.end
            })
            .ok_or("output annotation reference is unavailable")?;
        let id =
            if reference.kind == ReferenceKind::Callable {
                match self
                    .calls
                    .calls
                    .iter()
                    .find(|c| c.file == file && c.location == reference.location)
                    .map(|c| &c.outcome)
                {
                    Some(CallOutcome::Resolved { declaration, .. }) => *declaration,
                    _ => return Err(
                        "default-output annotation callable identity is unresolved or ambiguous"
                            .into(),
                    ),
                }
            } else if let BindingResolution::Resolved(id) = reference.resolution {
                id
            } else {
                return Err("default-output annotation identity is unresolved or ambiguous".into());
            };
        if !self.core(id) {
            return Err(format!(
                "default-output annotation '{}' is not interpreted",
                reference.name
            ));
        }
        match self.bindings.declarations[id.0].name.as_str() {
            "no_output" => Ok("no_output"),
            "add_to_output" => Ok("add_to_output"),
            "output_var" => Ok("output_var"),
            "output_array" => Ok("output_array"),
            "output_only" => Ok("output_only"),
            name => Err(format!(
                "default-output annotation '{name}' is not interpreted"
            )),
        }
    }
    fn core(&self, id: DeclarationId) -> bool {
        let d = &self.bindings.declarations[id.0];
        let source = &self.context.files[d.file];
        source.kind == SourceKind::StandardLibrary
            && source.implicit
            && crate::callables::find_node(source.parsed.tree(), &d.syntax_range, d.role)
                .is_some_and(|n| {
                    !n.child_nodes()
                        .any(|c| crate::callables::is_expression(c.kind()))
                })
    }
    fn follow(&mut self) {
        if self.facts.root_state != ModelRootState::Complete {
            for reference in self
                .bindings
                .references
                .iter()
                .filter(|r| r.callable.is_some())
            {
                self.collect_reference(reference, self.calls, Some(self.main_call_indices), &[]);
            }
            for d in &mut self.facts.declarations {
                d.outcome = UsageOutcome::Uncertain;
            }
            return;
        }
        let mut pending = Vec::new();
        self.body_dependencies(None, UsageOutcome::Reachable, &mut pending);
        pending.extend(
            self.facts
                .roots
                .iter()
                .map(|&id| (id, UsageOutcome::Reachable))
                .chain(
                    self.facts
                        .possible_roots
                        .iter()
                        .map(|&id| (id, UsageOutcome::Uncertain)),
                ),
        );
        let mut unbounded = false;
        while let Some((id, state)) = pending.pop() {
            self.body_dependencies(Some(id), state, &mut pending);
            let usage = &mut self.facts.declarations[id.0];
            if usage.outcome == UsageOutcome::Reachable || usage.outcome == state {
                continue;
            }
            usage.outcome = state;
            pending.extend(usage.dependencies.iter().map(|&d| (d, state)));
            pending.extend(
                usage
                    .possible_dependencies
                    .iter()
                    .map(|&d| (d, UsageOutcome::Uncertain)),
            );
        }
        for u in &self.uncertain {
            let state = u
                .owner
                .map(|id| self.facts.declarations[id.0].outcome)
                .unwrap_or(UsageOutcome::Reachable);
            if state == UsageOutcome::Reachable {
                push_diagnostic(&mut self.facts.limitations, &u.diagnostic);
            }
            if state != UsageOutcome::Unreachable {
                unbounded |= u.unbounded;
            }
        }
        if unbounded {
            for d in &mut self.facts.declarations {
                if d.outcome == UsageOutcome::Unreachable {
                    d.outcome = UsageOutcome::Uncertain;
                }
            }
        }
    }
}
fn entry_item(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Constraint
            | NodeKind::Solve
            | NodeKind::SolveMinimize
            | NodeKind::SolveMaximize
            | NodeKind::Output
    )
}
fn contains_anonymous(context: &ModelContext, file: FileId, node: &SyntaxNode) -> bool {
    node.children().iter().any(|c| match c {
        SyntaxElement::Token(i) => {
            context.files[file].parsed.tokens()[*i].kind == TokenKind::Anonymous
        }
        SyntaxElement::Node(n) => contains_anonymous(context, file, n),
    })
}
fn push_unique(ids: &mut Vec<DeclarationId>, id: DeclarationId) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}
fn push_diagnostic(out: &mut Vec<SourceDiagnostic>, diagnostic: &SourceDiagnostic) {
    if !out.contains(diagnostic) {
        out.push(diagnostic.clone());
    }
}
pub(super) fn check_unused_declarations(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &UsageFacts,
) -> Vec<FileFinding> {
    if facts.root_state != ModelRootState::Complete {
        return Vec::new();
    }
    let mut findings = Vec::new();
    for usage in &facts.declarations {
        let d = &bindings.declarations[usage.declaration.0];
        let source = &context.files[d.file];
        if usage.outcome != UsageOutcome::Unreachable
            || !source.warnings_enabled()
            || d.name.starts_with('_')
            || matches!(
                d.role,
                DeclarationRole::TypeAlias | DeclarationRole::Enum | DeclarationRole::EnumMember
            )
        {
            continue;
        }
        let Some(suppressed) = &source.suppressions else {
            continue;
        };
        if suppressed[d.item].contains(&Rule::UnusedDeclaration) {
            continue;
        }
        let mut enclosing = usage.enclosing;
        let mut unused_outer = false;
        while let Some(id) = enclosing {
            let outer = &facts.declarations[id.0];
            unused_outer |= outer.outcome == UsageOutcome::Unreachable;
            enclosing = outer.enclosing;
        }
        if unused_outer {
            continue;
        }
        findings.push(FileFinding { fix: None,location:d.location.clone(),rule:Rule::UnusedDeclaration,
            severity:Severity::Warning,message:format!("declaration '{}' is unreachable from this model's constraints, solve and output; consider whether it is needed",d.name)});
    }
    findings
}
