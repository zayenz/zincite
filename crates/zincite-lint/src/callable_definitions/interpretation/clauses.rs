//! Each handler preserves one ClauseKind decision and its ordered effects.
use super::*;

impl<'a> BodyInterpreter<'a, '_> {
    pub(in crate::callable_definitions) fn interpret_unsupported_clause<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
    ) {
        if let Some(checked) = self
            .source
            .optional_float_weak_equality_safety(clause, view)
        {
            if let Err(boundary) = checked {
                unavailable.push(boundary);
            }
            // Inspection of this relation supplies no definition or output.
            return;
        }
        if clause.node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(clause.file, clause.node, view, "forall")
        {
            // Strict ambient headers still own their scope and membership.
            // Checked uncertainty in this forall supplies no outputs or locals.
            if let Err(reason) =
                self.relation_iterations(clause.file, &clause.generators, 0, view, false)
            {
                self.unavailable(clause, &reason, unavailable);
                return;
            }
            let mut sources = Vec::new();
            for (position, generator) in clause.generators.iter().enumerate() {
                for child in generator.child_nodes() {
                    let value = if child.kind() == NodeKind::WhereFilter {
                        let Some(value) = child.child_nodes().next() else {
                            self.unavailable(
                                clause,
                                "forall ambient filter unavailable",
                                unavailable,
                            );
                            return;
                        };
                        value
                    } else {
                        child
                    };
                    let ambient = if child.kind() == NodeKind::WhereFilter {
                        &clause.generators[..=position]
                    } else {
                        &clause.generators[..position]
                    };
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        clause.file,
                        value,
                        view,
                        ambient,
                        &mut sources,
                    ) {
                        self.unavailable(clause, &reason, unavailable);
                        return;
                    }
                }
            }
            match self.initialized_source_safety(
                clause.file,
                clause.node,
                view,
                &clause.generators,
                &mut sources,
            ) {
                DefinitionSafety::Unknown(_) => return,
                DefinitionSafety::Unsupported(reason) => {
                    self.unavailable(clause, &reason, unavailable);
                    return;
                }
                DefinitionSafety::Supported => {}
            }
        }
        self.unavailable(
            clause,
            "callable definition enforcement or control flow is unsupported",
            unavailable,
        );
    }
    pub(in crate::callable_definitions) fn interpret_relation_clause<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
    ) {
        let (clause, view, _) = inputs;
        if let Err(reason) =
            self.relation_iterations(clause.file, &clause.generators, 0, view, false)
        {
            self.unavailable(clause, &reason, unavailable);
            return;
        }
        if let Err(reason) =
            self.boolean_relation_dependencies(clause.file, clause.node, view, &clause.generators)
        {
            // Checked uncertainty can inspect a relation, never its outputs.
            let mut values = vec![clause.node];
            for generator in &clause.generators {
                for child in generator.child_nodes() {
                    let value = if child.kind() == NodeKind::WhereFilter {
                        let Some(value) = child.child_nodes().next() else {
                            self.unavailable(
                                clause,
                                "relation iteration filter unavailable",
                                unavailable,
                            );
                            return;
                        };
                        value
                    } else {
                        child
                    };
                    values.push(value);
                }
            }
            match self.initialized_children_safety(clause.file, &values, view, &clause.generators) {
                DefinitionSafety::Unknown(_) => {}
                DefinitionSafety::Unsupported(reason) => {
                    self.unavailable(clause, &reason, unavailable)
                }
                DefinitionSafety::Supported => self.unavailable(clause, &reason, unavailable),
            }
            return;
        }
        if self.source.core(clause.file, clause.node, view, "<->") {
            let nodes: Vec<_> = clause.node.child_nodes().collect();
            if nodes.len() == 2 {
                for (lhs, rhs) in [(nodes[0], nodes[1]), (nodes[1], nodes[0])] {
                    let lhs = unwrap(lhs);
                    let Some((target, coverage)) =
                        self.source
                            .target(clause.file, lhs, view, &clause.generators)
                    else {
                        continue;
                    };
                    let kind = if lhs.kind() == NodeKind::Expression {
                        TypeKind::Bool
                    } else if lhs.kind() == NodeKind::ArrayAccessExpression
                        && coverage == DefinitionCoverage::WholeArray
                        && matches!(&view.declarations[target.0].ty.kind,
                            TypeKind::Array { indices, element }
                                if indices.len() == 2 && element.kind == TypeKind::Bool)
                    {
                        view.declarations[target.0].ty.kind.clone()
                    } else {
                        continue;
                    };
                    if !self.source.scoped_local(clause, target, view, kind) {
                        continue;
                    }
                    let range = self.source.context.files[clause.file]
                        .location(rhs.range())
                        .range;
                    let boolean = self
                        .source
                        .view(clause.file, rhs, view)
                        .expressions
                        .iter()
                        .any(|e| {
                            e.file == clause.file
                                && e.location.range == range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.kind == TypeKind::Bool
                        });
                    if boolean
                        && let Ok(dependencies) =
                            self.dependencies(clause.file, rhs, view, &clause.generators)
                    {
                        out.push(Output {
                            target,
                            dependencies,
                            coverage,
                            location: self.source.context.files[clause.file]
                                .location(clause.node.range()),
                            enforced_boolean: false,
                            scoped_local: true,
                        });
                    }
                }
            }
        }
    }
    pub(in crate::callable_definitions) fn interpret_conditional_clause<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, known) = inputs;
        // Inspect a failed guard before any branch contributes outputs or
        // inspected locals. An unproved test value supplies no direction.
        let guard_error = invocation
            .is_none()
            .then(|| {
                clause.node.child_nodes().find_map(|branch| {
                    (branch.kind() == NodeKind::ConditionalBranch)
                        .then(|| branch.child_nodes().next())
                        .flatten()
                        .and_then(|guard| {
                            self.dependencies(clause.file, guard, view, &clause.generators)
                                .err()
                        })
                })
            })
            .flatten();
        if let Some(reason) = guard_error {
            let checked = (|| -> DefinitionSafety {
                if self
                    .source
                    .expression_type(
                        self.source.view(clause.file, clause.node, view),
                        clause.file,
                        clause.node,
                    )
                    .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
                {
                    return DefinitionSafety::Unsupported(reason.clone());
                }
                if let Err(reason) =
                    self.relation_iterations(clause.file, &clause.generators, 0, view, false)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let mut active = Vec::new();
                for (position, generator) in clause.generators.iter().enumerate() {
                    for child in generator.child_nodes() {
                        let value = if child.kind() == NodeKind::WhereFilter {
                            let Some(value) = child.child_nodes().next() else {
                                return DefinitionSafety::Unsupported(
                                    "conditional iteration filter unavailable".into(),
                                );
                            };
                            value
                        } else {
                            child
                        };
                        if let unsupported @ DefinitionSafety::Unsupported(_) = self
                            .initialized_source_safety(
                                clause.file,
                                value,
                                view,
                                &clause.generators[..=position],
                                &mut active,
                            )
                        {
                            return unsupported;
                        }
                    }
                }
                self.initialized_source_safety(
                    clause.file,
                    clause.node,
                    view,
                    &clause.generators,
                    &mut active,
                )
            })();
            match checked {
                DefinitionSafety::Unknown(_) => {}
                DefinitionSafety::Unsupported(reason) => {
                    self.unavailable(clause, &reason, unavailable)
                }
                DefinitionSafety::Supported => self.unavailable(clause, &reason, unavailable),
            }
            return;
        }
        let mut branches: Vec<Vec<Output>> = Vec::new();
        let mut guards = Vec::new();
        let mut has_else = false;
        let incoming = invocation.as_ref().and_then(|i| i.reachable);
        let mut remaining = incoming;
        for branch in clause.node.child_nodes() {
            let nodes: Vec<_> = branch.child_nodes().collect();
            let mut branch_reachable = incoming;
            let body = if branch.kind() == NodeKind::ConditionalBranch && nodes.len() == 2 {
                let range = self.source.context.files[clause.file]
                    .location(nodes[0].range())
                    .range;
                let guard = self
                    .source
                    .view(clause.file, nodes[0], view)
                    .expressions
                    .iter()
                    .find(|e| e.file == clause.file && e.location.range == range)
                    .map(|e| &e.ty);
                if guard.is_none_or(|t| {
                    t.kind != TypeKind::Bool
                        || t.optional
                        || t.instantiation != Instantiation::Parameter
                }) {
                    self.unavailable(
                        clause,
                        "conditional output guard is not a supported total parameter Boolean",
                        unavailable,
                    );
                    return;
                }
                let checked_guard = invocation.as_ref().map(|_| {
                    self.initialized_source_safety(
                        clause.file,
                        nodes[0],
                        view,
                        &clause.generators,
                        &mut Vec::new(),
                    )
                });
                if let Some(DefinitionSafety::Unsupported(reason)) = &checked_guard {
                    self.unavailable(clause, reason, unavailable);
                    return;
                }
                match self.dependencies(clause.file, nodes[0], view, &clause.generators) {
                    Ok(ids) => extend(&mut guards, ids),
                    Err(_) if matches!(checked_guard, Some(DefinitionSafety::Unknown(_))) => {}
                    Err(reason) => {
                        self.unavailable(clause, &reason, unavailable);
                        return;
                    }
                }
                if let Some(state) = invocation.as_deref_mut() {
                    let value = match self.invocation_guard(clause.file, nodes[0], view, state) {
                        Ok(value) => value,
                        Err(reason) => {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                    };
                    branch_reachable = match (remaining, value) {
                        (Some(false), _) | (_, Some(false)) => Some(false),
                        (Some(true), Some(true)) => Some(true),
                        _ => None,
                    };
                    remaining = match (remaining, value) {
                        (Some(false), _) | (_, Some(true)) => Some(false),
                        (value, Some(false)) => value,
                        _ => None,
                    };
                }
                nodes[1]
            } else if branch.kind() == NodeKind::ElseBranch && nodes.len() == 1 {
                has_else = true;
                branch_reachable = remaining;
                nodes[0]
            } else {
                self.unavailable(
                    clause,
                    "conditional branch form is unsupported",
                    unavailable,
                );
                return;
            };
            if invocation.is_some()
                && self
                    .source
                    .expression_type(self.source.view(clause.file, body, view), clause.file, body)
                    .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
            {
                self.unavailable(
                    clause,
                    "invoked conditional body type is unsupported",
                    unavailable,
                );
                return;
            }
            let mut clauses = Vec::new();
            self.clauses(
                clause.file,
                clause.item,
                body,
                view,
                &clause.generators,
                &mut clauses,
            );
            let mut values = Vec::new();
            let before = unavailable.len();
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = branch_reachable;
            }
            for branch in clauses {
                self.interpret_forwarded(
                    (&branch, view, known),
                    &mut values,
                    unavailable,
                    inspected,
                    active,
                    invocation.as_deref_mut(),
                );
            }
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = incoming;
            }
            if unavailable.len() != before && invocation.is_none() {
                return;
            }
            branches.push(values);
        }
        if let Some(state) = invocation {
            state.reachable = incoming;
        }
        // A missing Boolean else supplies false, never an output.
        if !has_else {
            branches.push(Vec::new());
        }
        if let Some(first) = branches.first() {
            for candidate in first {
                let mut dependencies = guards.clone();
                let mut present = true;
                for branch in &branches {
                    if let Some(output) = branch.iter().find(|g| {
                        g.target == candidate.target
                            && g.coverage == candidate.coverage
                            && g.enforced_boolean == candidate.enforced_boolean
                    }) {
                        extend(&mut dependencies, output.dependencies.clone());
                    } else {
                        present = false;
                        break;
                    }
                }
                if present {
                    out.push(Output {
                        target: candidate.target,
                        dependencies,
                        coverage: candidate.coverage.clone(),
                        location: self.source.context.files[clause.file]
                            .location(clause.node.range()),
                        enforced_boolean: candidate.enforced_boolean,
                        scoped_local: false,
                    });
                }
            }
        }
    }
    pub(in crate::callable_definitions) fn interpret_local_clause<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, known) = inputs;
        let range = self.source.context.files[clause.file]
            .location(clause.node.range())
            .range;
        if self
            .source
            .view(clause.file, clause.node, view)
            .expressions
            .iter()
            .find(|e| e.file == clause.file && e.location.range == range)
            .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
        {
            self.unavailable(
                clause,
                "model let result is not a supported present Boolean",
                unavailable,
            );
            return;
        }
        if let Some(result) =
            self.inspect_uncertain_index_let(clause, view, None, &[], invocation.as_deref())
        {
            if let Err(reason) = result {
                self.unavailable(clause, &reason, unavailable);
            }
            // No output holds when an initializer can abort the whole let.
            return;
        }
        if let Some(safety) =
            self.parameter_set_let_safety(clause.file, clause.node, view, &clause.generators)
        {
            if let DefinitionSafety::Unsupported(reason) = safety {
                self.unavailable(clause, &reason, unavailable);
            }
            // A relation over a local filtered set defines no array.
            return;
        }
        let mut clauses = Vec::new();
        let mut private = Vec::new();
        let mut initialization = Vec::new();
        let mut found = Vec::new();
        let mut uncertain_locals = Vec::new();
        let mut uncertain_initialization = false;
        for block in clause
            .node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
        {
            for local in block.child_nodes() {
                if !crate::definitions::annotations_safe(self.source.context, clause.file, local) {
                    self.unavailable(
                        clause,
                        "local annotation evaluation is unsupported",
                        unavailable,
                    );
                    return;
                }
                if local.kind() == NodeKind::Constraint {
                    self.clauses(
                        clause.file,
                        clause.item,
                        local,
                        view,
                        &clause.generators,
                        &mut clauses,
                    );
                    continue;
                }
                let declaration = self.source.bindings.declarations.iter().find(|d| {
                    d.file == clause.file
                        && d.syntax_range == local.range()
                        && d.role == DeclarationRole::Local
                });
                let Some(declaration) =
                    declaration.filter(|_| local.kind() == NodeKind::Declaration)
                else {
                    self.unavailable(clause, "local declaration is unsupported", unavailable);
                    return;
                };
                let ty = &view.declarations[declaration.id.0].ty;
                if let Some(safety) = self.optional_parameter_matrix_safety(
                    clause.file,
                    local,
                    declaration.id,
                    view,
                    &clause.generators,
                ) {
                    if let DefinitionSafety::Unsupported(reason) = safety {
                        self.unavailable(clause, &reason, unavailable);
                        return;
                    }
                    uncertain_initialization = true;
                    continue;
                }
                if !ty.known() || optional(ty) {
                    self.unavailable(
                        clause,
                        "local declaration type or optionality is unsupported",
                        unavailable,
                    );
                    return;
                }
                let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
                let boolean_comprehension = initializer
                    .is_some_and(|n| unwrap(n).kind() == NodeKind::ArrayComprehension)
                    && matches!(&ty.kind, TypeKind::Array { element, .. }
                        if element.kind == TypeKind::Bool);
                let scoped_array = ty.instantiation == Instantiation::Decision
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter
                            && element.known() && !optional(element)
                            && (element.kind == TypeKind::Int || boolean_comprehension))
                    && initializer.is_some();
                if scoped_array {
                    let initializer = unwrap(initializer.unwrap());
                    if initializer.kind() == NodeKind::ArrayComprehension {
                        if !self
                            .source
                            .scoped_local(clause, declaration.id, view, ty.kind.clone())
                        {
                            self.unavailable(
                                clause,
                                "local comprehension scope is unsupported",
                                unavailable,
                            );
                            return;
                        }
                        if let Err(reason) = self.uncertain_comprehension_type(
                            clause.file,
                            local,
                            initializer,
                            view,
                            &clause.generators,
                        ) {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                        let safety = if boolean_comprehension {
                            self.collection_construction_safety(
                                clause.file,
                                initializer,
                                view,
                                &clause.generators,
                            )
                        } else {
                            self.integer_comprehension_safety(
                                clause.file,
                                initializer,
                                view,
                                &clause.generators,
                            )
                        };
                        if let DefinitionSafety::Unsupported(reason) = safety {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                        uncertain_initialization = true;
                        uncertain_locals.push(declaration.id);
                        private.push(declaration.id);
                        continue;
                    }
                    let cells: Vec<_> = initializer.child_nodes().collect();
                    let range = self.source.context.files[clause.file]
                        .location(initializer.range())
                        .range;
                    let axis = match &self.source.domains.declarations[declaration.id.0].domain {
                        Domain::Array { indices, .. } if indices.len() == 1 => {
                            matches!(&indices[0], Domain::Range { lower, upper }
                                if crate::domains::invariant_integer(lower).ok() == Some(Some(1))
                                    && crate::domains::invariant_integer(upper).ok().flatten()
                                        == i64::try_from(cells.len()).ok())
                        }
                        _ => false,
                    };
                    if !self
                        .source
                        .scoped_local(clause, declaration.id, view, ty.kind.clone())
                        || initializer.kind() != NodeKind::ArrayLiteral
                        || cells.is_empty()
                        || cells
                            .iter()
                            .any(|cell| cell.kind() == NodeKind::IndexedArrayEntry)
                        || !axis
                        || !view.expressions.iter().any(|e| {
                            e.file == clause.file
                                && e.location.range == range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && matches!(&e.ty.kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                                        && indices[0].instantiation == Instantiation::Parameter
                                        && element.kind == TypeKind::Int)
                        })
                    {
                        self.unavailable(clause, "local integer array initializer scope, type or literal axis is unsupported", unavailable);
                        return;
                    }
                    let mut ids = match self.type_dependencies(
                        clause.file,
                        local,
                        view,
                        &clause.generators,
                    ) {
                        Ok(ids) => ids,
                        Err(reason) => {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                    };
                    match self.direct_children_safety(clause.file, &cells, view, &clause.generators)
                    {
                        DefinitionSafety::Unsupported(reason) => {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                        DefinitionSafety::Unknown(_) => uncertain_locals.push(declaration.id),
                        DefinitionSafety::Supported => {
                            // Exact 1..N plain literals define this scoped array
                            // forward from every cell, never their input owners.
                            match self.child_dependencies(
                                clause.file,
                                &cells,
                                view,
                                &clause.generators,
                            ) {
                                Ok(dependencies) => extend(&mut ids, dependencies),
                                Err(reason) => {
                                    self.unavailable(clause, &reason, unavailable);
                                    return;
                                }
                            }
                            found.push(Output {
                                target: declaration.id,
                                dependencies: ids,
                                coverage: DefinitionCoverage::WholeArray,
                                location: self.source.context.files[clause.file]
                                    .location(initializer.range()),
                                enforced_boolean: false,
                                scoped_local: true,
                            });
                        }
                    }
                    private.push(declaration.id);
                    continue;
                }
                let scoped_integer = ty.instantiation == Instantiation::Decision
                    && ty.kind == TypeKind::Int
                    && initializer.is_some();
                if scoped_integer {
                    let range = self.source.context.files[clause.file]
                        .location(initializer.unwrap().range())
                        .range;
                    if !self
                        .source
                        .scoped_local(clause, declaration.id, view, TypeKind::Int)
                        || !view.expressions.iter().any(|e| {
                            e.file == clause.file
                                && e.location.range == range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.kind == TypeKind::Int
                        })
                    {
                        self.unavailable(
                            clause,
                            "local integer initializer scope or type is unsupported",
                            unavailable,
                        );
                        return;
                    }
                }
                if ty.instantiation == Instantiation::Decision && initializer.is_none() {
                    let boolean_array_rank = match &ty.kind {
                        TypeKind::Array { indices, element }
                            if matches!(indices.len(), 1 | 2)
                                && indices.iter().all(|index| {
                                    index.kind == TypeKind::Int
                                        && index.instantiation == Instantiation::Parameter
                                })
                                && element.kind == TypeKind::Bool =>
                        {
                            Some(indices.len())
                        }
                        _ => None,
                    };
                    if let Some(rank) = boolean_array_rank {
                        let written = local
                            .child_nodes()
                            .find(|n| n.kind() == NodeKind::ArrayType)
                            .map(|n| n.child_nodes().collect::<Vec<_>>());
                        let indices = written
                            .as_deref()
                            .and_then(|nodes| nodes.split_last())
                            .map(|(_, indices)| indices)
                            .filter(|indices| indices.len() == rank);
                        let Some(indices) = indices else {
                            self.unavailable(
                                clause,
                                "private Boolean array requires explicit integer ranges",
                                unavailable,
                            );
                            return;
                        };
                        for index in indices {
                            let index = (index.kind() == NodeKind::DomainType)
                                .then(|| index.child_nodes().next())
                                .flatten()
                                .filter(|n| unwrap(n).kind() == NodeKind::RangeExpression);
                            let Some(index) = index else {
                                self.unavailable(
                                    clause,
                                    "private Boolean array requires an explicit integer range",
                                    unavailable,
                                );
                                return;
                            };
                            if let Err(reason) =
                                self.dependencies(clause.file, index, view, &clause.generators)
                            {
                                self.unavailable(
                                    clause,
                                    &format!(
                                        "private Boolean array range is unsupported: {reason}"
                                    ),
                                    unavailable,
                                );
                                return;
                            }
                        }
                    }
                    if ty.kind == TypeKind::Bool || boolean_array_rank.is_some() {
                        if let Err(reason) =
                            self.type_dependencies(clause.file, local, view, &clause.generators)
                        {
                            self.unavailable(clause, &reason, unavailable);
                            return;
                        }
                        private.push(declaration.id);
                        continue;
                    }
                }
                if (ty.instantiation != Instantiation::Parameter && !scoped_integer)
                    || initializer.is_none()
                {
                    self.unavailable(
                        clause,
                        "local output initializer or decision type is unsupported",
                        unavailable,
                    );
                    return;
                }
                match self
                    .type_dependencies(clause.file, local, view, &clause.generators)
                    .and_then(|mut ids| {
                        extend(
                            &mut ids,
                            self.dependencies(
                                clause.file,
                                initializer.unwrap(),
                                view,
                                &clause.generators,
                            )?,
                        );
                        Ok(ids)
                    }) {
                    Ok(ids) if scoped_integer => {
                        found.push(Output {
                            target: declaration.id,
                            dependencies: ids,
                            coverage: DefinitionCoverage::Scalar,
                            location: self.source.context.files[clause.file]
                                .location(initializer.unwrap().range()),
                            enforced_boolean: false,
                            scoped_local: true,
                        });
                        private.push(declaration.id);
                    }
                    Ok(ids) => extend(&mut initialization, ids),
                    Err(mut reason) => {
                        let safety = if (ty.instantiation == Instantiation::Parameter
                            || (scoped_integer
                                && unwrap(initializer.unwrap()).kind()
                                    == NodeKind::ArrayAccessExpression))
                            && self
                                .type_dependencies(clause.file, local, view, &clause.generators)
                                .is_ok()
                        {
                            Some(self.direct_safety(
                                clause.file,
                                initializer.unwrap(),
                                view,
                                &clause.generators,
                            ))
                        } else {
                            None
                        };
                        if matches!(&safety, Some(DefinitionSafety::Unknown(_))) {
                            uncertain_initialization = true;
                            if scoped_integer {
                                uncertain_locals.push(declaration.id);
                                private.push(declaration.id);
                            }
                        } else {
                            if let Some(DefinitionSafety::Unsupported(actual)) = safety {
                                reason = actual;
                            }
                            self.unavailable(
                                clause,
                                &format!(
                                    "local output initializer or domain is unsupported: {reason}"
                                ),
                                unavailable,
                            );
                            return;
                        }
                    }
                }
            }
        }
        for body in clause
            .node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::LetBlock)
        {
            self.clauses(
                clause.file,
                clause.item,
                body,
                view,
                &clause.generators,
                &mut clauses,
            );
        }
        let before = unavailable.len();
        for body in clauses {
            if uncertain_initialization
                && matches!(body.kind, ClauseKind::Equality)
                && self.source.core(body.file, body.node, view, "=")
            {
                let operands: Vec<_> = body.node.child_nodes().collect();
                let integers = operands.len() == 2
                    && operands.iter().all(|node| {
                        let range = self.source.context.files[body.file]
                            .location(node.range())
                            .range;
                        self.source
                            .view(body.file, node, view)
                            .expressions
                            .iter()
                            .any(|e| {
                                e.file == body.file
                                    && e.location.range == range
                                    && e.ty.known()
                                    && !optional(&e.ty)
                                    && e.ty.kind == TypeKind::Int
                            })
                    });
                if integers {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.direct_children_safety(body.file, &operands, view, &body.generators)
                    {
                        self.unavailable(&body, &reason, unavailable);
                    }
                    continue;
                }
            }
            let scoped_boolean_result = invocation.is_none()
                && uncertain_locals.iter().any(|id| {
                    matches!(&view.declarations[id.0].ty.kind,
                        TypeKind::Array { indices, element }
                            if indices.len() == 1 && element.kind == TypeKind::Bool)
                })
                && matches!(body.kind, ClauseKind::Call)
                && clause.node.child_nodes().any(|result| {
                    result.kind() != NodeKind::LetBlock
                        && unwrap(result).range() == body.node.range()
                })
                && self
                    .source
                    .resolved(body.file, body.node, view)
                    .is_some_and(|(id, parameters)| {
                        self.instances.iter().any(|instance| {
                            instance.id == id
                                && instance.parameters == parameters
                                && !instance.recursive
                        }) && !known
                            .iter()
                            .any(|output| output.callable == id && output.parameters == parameters)
                    });
            if scoped_boolean_result {
                // The checked owning scope supplies sources, never outputs.
                // Preserve the active forwarding stack while binding actuals.
                let mut sources = Invocation {
                    actuals: Vec::new(),
                    reachable: Some(true),
                };
                self.interpret_forwarded(
                    (&body, view, known),
                    &mut found,
                    unavailable,
                    inspected,
                    active,
                    Some(&mut sources),
                );
            } else {
                self.interpret_forwarded(
                    (&body, view, known),
                    &mut found,
                    unavailable,
                    inspected,
                    active,
                    invocation.as_deref_mut(),
                );
            }
        }
        if unavailable.len() == before {
            extend(inspected, uncertain_locals);
            for id in &private {
                let declaration = &self.source.bindings.declarations[id.0];
                let source = &self.source.context.files[declaration.file];
                if source.kind == SourceKind::User
                    && source
                        .parsed
                        .tree()
                        .child_nodes()
                        .nth(declaration.item)
                        .is_some_and(|item| item.kind() == NodeKind::Constraint)
                    && view.declarations[id.0].ty.kind == TypeKind::Bool
                    && !inspected.contains(id)
                {
                    inspected.push(*id);
                }
            }
            // These scoped values were inspected successfully, but an
            // unproved initializer can abort before any guarantee holds.
            if uncertain_initialization {
                extend(
                    inspected,
                    found
                        .iter()
                        .filter(|output| {
                            output.scoped_local
                                && self.source.scoped_local(
                                    clause,
                                    output.target,
                                    view,
                                    view.declarations[output.target.0].ty.kind.clone(),
                                )
                        })
                        .map(|output| output.target)
                        .collect(),
                );
                return;
            }
            let scoped: Vec<_> = found
                .iter()
                .filter(|output| output.scoped_local)
                .map(|output| output.target)
                .collect();
            // Ordinary outputs can depend on validated scoped values.
            // Other private decision values are not solved or exported.
            out.extend(
                found
                    .into_iter()
                    .map(|mut output| {
                        extend(&mut output.dependencies, initialization.clone());
                        output
                    })
                    .filter(|output| {
                        output.scoped_local
                            || (!private.contains(&output.target)
                                && output
                                    .dependencies
                                    .iter()
                                    .all(|id| !private.contains(id) || scoped.contains(id)))
                    }),
            );
        } else {
            // A private prerequisite can prevent this whole let from
            // establishing its external result; keep the inner cause too.
            self.unavailable(
                clause,
                "local constraints or result evaluation are unsupported",
                unavailable,
            );
        }
    }
    pub(in crate::callable_definitions) fn interpret_assertion_clause<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, known) = inputs;
        let Some(args) = self
            .source
            .assertion_arguments(clause.file, clause.node, view)
        else {
            self.unavailable(
                clause,
                "assertion argument correspondence is unsupported",
                unavailable,
            );
            return;
        };
        // A two-argument assertion has no result whose dependencies
        // can be exported. Inspecting it does not prove its condition.
        let inspection_only = args.len() == 2
            && crate::definitions::annotations_safe(
                self.source.context,
                clause.file,
                clause.node,
            )
            && self.source
                .operation_fact(
                    self.source.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                )
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [TypeInst::par(TypeKind::Bool), TypeInst::par(TypeKind::String)]
                            && *return_type == TypeInst::par(TypeKind::Bool))
                });
        let mut inspect_arguments = false;
        let mut dependencies = Vec::new();
        for (position, (file, argument)) in args.iter().take(2).enumerate() {
            let range = self.source.context.files[*file]
                .location(argument.range())
                .range;
            let ty = self
                .source
                .view(*file, argument, view)
                .expressions
                .iter()
                .find(|e| e.file == *file && e.location.range == range)
                .map(|e| &e.ty);
            let expected = if position == 0 {
                TypeKind::Bool
            } else {
                TypeKind::String
            };
            if ty.is_none_or(|t| {
                t.kind != expected || t.optional || t.instantiation != Instantiation::Parameter
            }) {
                self.unavailable(
                    clause,
                    "assertion condition or message type is unsupported",
                    unavailable,
                );
                return;
            }
            match self.dependencies(*file, argument, view, &clause.generators) {
                Ok(ids) => extend(&mut dependencies, ids),
                Err(_) if inspection_only => inspect_arguments = true,
                Err(reason) => {
                    self.unavailable(clause, &reason, unavailable);
                    return;
                }
            }
        }
        if inspect_arguments {
            let mut unsupported = None;
            // Inspect both arguments even if one strict dependency
            // check passed; a named message can hide an initializer.
            for (file, argument) in &args {
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    *file,
                    argument,
                    view,
                    &clause.generators,
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
                if let Some(reason) = self
                    .source
                    .closed_integer_source_error(*file, argument, true, true)
                {
                    unsupported = Some(reason);
                }
            }
            if let Some(reason) = unsupported {
                self.unavailable(clause, &reason, unavailable);
                return;
            }
        }
        let (condition_file, condition) = if let Some(state) = invocation.as_ref() {
            match self.invocation_source(args[0].0, args[0].1, state) {
                Ok(Some(actual)) => (actual.file, unwrap(actual.node)),
                Ok(None) => (args[0].0, unwrap(args[0].1)),
                Err(reason) => {
                    self.unavailable(clause, &reason, unavailable);
                    return;
                }
            }
        } else {
            (args[0].0, unwrap(args[0].1))
        };
        let literal = (condition.kind() == NodeKind::Expression)
            .then(|| {
                crate::domains::tokens(&self.source.context.files[condition_file].parsed, condition)
                    .first()
                    .map(|t| t.kind)
            })
            .flatten();
        if literal == Some(TokenKind::False) && invocation.is_none() {
            self.unavailable(
                clause,
                "false assertion condition aborts evaluation",
                unavailable,
            );
            return;
        }
        if let Some((file, body)) = args.get(2) {
            if invocation.is_some()
                && self
                    .source
                    .expression_type(self.source.view(*file, body, view), *file, body)
                    .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
            {
                self.unavailable(
                    clause,
                    "invoked assertion result type is unsupported",
                    unavailable,
                );
                return;
            }
            let mut clauses = Vec::new();
            self.clauses(
                *file,
                clause.item,
                body,
                view,
                &clause.generators,
                &mut clauses,
            );
            let boolean = |file, node: &SyntaxNode| {
                let node = unwrap(node);
                self.source
                    .expression_type(self.source.view(file, node, view), file, node)
                    .is_some_and(|e| {
                        e.ty.known() && !optional(&e.ty) && e.ty.kind == TypeKind::Bool
                    })
            };
            // Normal return from an enforced Boolean assertion enforces its
            // third argument. Unknown-condition forwarding requires a direct
            // equality or a complete array contract checked for this invocation.
            let direct_equality = clauses.len() == 1
                && matches!(clauses[0].kind, ClauseKind::Equality)
                && clauses[0].node.range() == unwrap(body).range()
                && boolean(*file, body)
                && boolean(clause.file, clause.node);
            let asserted_forall = (clause.generators.is_empty()
                && clauses.len() == 1
                && matches!(clauses[0].kind, ClauseKind::Equality))
            .then(|| self.direct_asserted_forall(clause.file, clause.node, view))
            .flatten();
            let nested_forall = (invocation.is_none()
                && clause.generators.is_empty()
                && clauses.len() == 1
                && matches!(clauses[0].kind, ClauseKind::Call | ClauseKind::Assertion))
            .then(|| self.nested_asserted_forall(clause.file, clause.node, view))
            .flatten();
            let nested_target = nested_forall.as_ref().map(|(output, _)| output.target);
            let mut found = Vec::new();
            let mut body_unavailable = Vec::new();
            if let Some((output, _)) = nested_forall {
                // This complete selected-body check belongs to this guard's
                // invocation. The unguarded child's summaries and causes remain.
                found.push(output);
            } else {
                for body in clauses {
                    // Every third argument remains inspected. A fresh boundary
                    // collection also retains causes already reported by siblings.
                    self.interpret_forwarded(
                        (&body, view, known),
                        &mut found,
                        &mut body_unavailable,
                        inspected,
                        active,
                        invocation.as_deref_mut(),
                    );
                }
            }
            let supported_body = body_unavailable.is_empty();
            unavailable.extend(body_unavailable);
            let whole_forall = asserted_forall
                .map(|(target, _)| target)
                .or(nested_target)
                .is_some_and(|target| {
                    found.len() == 1
                        && found[0].target == target
                        && found[0].coverage == DefinitionCoverage::WholeArray
                        && !found[0].enforced_boolean
                        && !found[0].scoped_local
                });
            if literal == Some(TokenKind::True)
                || (direct_equality || whole_forall) && supported_body
            {
                for mut output in found {
                    extend(&mut output.dependencies, dependencies.clone());
                    out.push(output);
                }
            }
        }
        if literal == Some(TokenKind::False)
            && invocation
                .as_ref()
                .is_some_and(|state| state.reachable == Some(true))
        {
            self.unavailable(
                clause,
                "false assertion condition aborts evaluation",
                unavailable,
            );
            if unavailable
                .last()
                .is_some_and(|row| !row.targets.is_empty())
            {
                // A reached abort remains unavailable even when the
                // affected actual result is already explicitly searched.
                unavailable.push(UnavailableCallableDefinition {
                    location: self.source.context.files[clause.file].location(clause.node.range()),
                    targets: Vec::new(),
                    reason: "false assertion condition aborts evaluation".into(),
                });
            }
        }
    }
    pub(in crate::callable_definitions) fn interpret_boolean_clause<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        out: &mut Vec<Output>,
    ) {
        if !clause.generators.is_empty() {
            return;
        }
        if let Some((target, coverage)) = self.source.target(clause.file, clause.node, view, &[]) {
            out.push(Output {
                target,
                dependencies: Vec::new(),
                coverage,
                location: self.source.context.files[clause.file].location(clause.node.range()),
                enforced_boolean: true,
                scoped_local: false,
            });
        }
    }
    pub(in crate::callable_definitions) fn interpret_equality_clause<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
    ) {
        let nodes: Vec<_> = clause.node.child_nodes().collect();
        if nodes.len() != 2 {
            return;
        }
        if nodes.iter().any(|node| {
            let range = self.source.context.files[clause.file]
                .location(node.range())
                .range;
            self.source
                .view(clause.file, node, view)
                .expressions
                .iter()
                .find(|e| e.file == clause.file && e.location.range == range)
                .is_some_and(|e| optional(&e.ty))
        }) {
            self.unavailable(
                clause,
                "optional equality outputs are unsupported",
                unavailable,
            );
            return;
        }
        let before = out.len();
        for (lhs, rhs) in [(nodes[0], nodes[1]), (nodes[1], nodes[0])] {
            let Some((target, coverage)) =
                self.source
                    .target(clause.file, lhs, view, &clause.generators)
            else {
                continue;
            };
            match self.dependencies(clause.file, rhs, view, &clause.generators) {
                Ok(dependencies) => out.push(Output {
                    target,
                    dependencies,
                    coverage,
                    location: self.source.context.files[clause.file].location(clause.node.range()),
                    enforced_boolean: false,
                    scoped_local: false,
                }),
                Err(reason) => {
                    let boolean_operands = nodes.iter().all(|node| {
                        let range = self.source.context.files[clause.file]
                            .location(node.range())
                            .range;
                        self.source
                            .view(clause.file, node, view)
                            .expressions
                            .iter()
                            .any(|e| {
                                e.file == clause.file
                                    && e.location.range == range
                                    && e.ty.kind == TypeKind::Bool
                                    && !optional(&e.ty)
                            })
                    });
                    if boolean_operands
                        && self
                            .boolean_relation_dependencies(
                                clause.file,
                                clause.node,
                                view,
                                &clause.generators,
                            )
                            .is_ok()
                    {
                        // Partial Boolean operands can express a supported relation,
                        // but establish neither direction of an output guarantee.
                        out.truncate(before);
                        return;
                    }
                    match self.uncertain_integer_equality(clause, view) {
                        Ok(true) => {
                            // Checked selection uncertainty inspects the relation,
                            // but cannot establish either output direction.
                            out.truncate(before);
                            return;
                        }
                        Err(actual) => {
                            out.truncate(before);
                            self.unavailable(clause, &actual, unavailable);
                            return;
                        }
                        Ok(false) => self.unavailable(clause, &reason, unavailable),
                    }
                }
            }
        }
    }
    pub(in crate::callable_definitions) fn interpret_call_clause<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, known) = inputs;
        let Some((id, parameters)) = self.source.resolved(clause.file, clause.node, view) else {
            self.unavailable(
                clause,
                "callable selection is unresolved, ambiguous or unsupported",
                unavailable,
            );
            return;
        };
        if clause.node.kind() == NodeKind::GeneratorCallExpression
            && self.source.core(clause.file, clause.node, view, "exists")
            && self.source.operation_fact(self.source.view(clause.file, clause.node, view), clause.file, clause.node)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                            && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Bool
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && indices[0].instantiation == Instantiation::Parameter
                                    && element.kind == TypeKind::Bool)))
        {
            if let Err(reason) = self.dependencies(clause.file, clause.node, view, &clause.generators) {
                self.unavailable(clause, &reason, unavailable);
            }
            // Existential bodies are inspected, never enforced directional outputs.
            return;
        }
        if let Some(DefinitionSafety::Unsupported(reason)) =
            self.optional_decreasing_actual_safety(clause, view, id, &parameters)
        {
            self.unavailable(clause, &reason, unavailable);
            return;
        }
        let mut computed_dependencies = Vec::new();
        let mut uncertain_actual = false;
        let mut unsupported_actual = false;
        for position in 0..parameters.len() {
            let Some((file, value)) = call_argument(
                self.source.context,
                self.source.bindings,
                self.source.view(clause.file, clause.node, view),
                clause.file,
                clause.node,
                id,
                position,
            ) else {
                continue;
            };
            if !self.source.default_collection(file, value) {
                continue;
            }
            let lexical = if file == clause.file
                && clause.node.range().start <= value.range().start
                && value.range().end <= clause.node.range().end
            {
                &clause.generators[..]
            } else {
                &[]
            };
            match self.direct_safety(file, value, view, lexical) {
                DefinitionSafety::Supported => {
                    match self.dependencies(file, value, view, lexical) {
                        Ok(ids) => extend(&mut computed_dependencies, ids),
                        Err(reason) => {
                            self.unavailable(clause, &reason, unavailable);
                            unsupported_actual = true;
                        }
                    }
                }
                DefinitionSafety::Unknown(_) => uncertain_actual = true,
                DefinitionSafety::Unsupported(reason) => {
                    self.unavailable(clause, &reason, unavailable);
                    unsupported_actual = true;
                }
            }
        }
        if let Some(call) = self.source.operation_fact(
            self.source.view(clause.file, clause.node, view),
            clause.file,
            clause.node,
        ) && let CallOutcome::Resolved {
            parameters: formal, ..
        } = &call.outcome
        {
            for (position, (declared, concrete)) in formal.iter().zip(&parameters).enumerate() {
                if declared.instantiation != Instantiation::Decision
                    || concrete.instantiation != Instantiation::Parameter
                {
                    continue;
                }
                let actual = call_argument(
                    self.source.context,
                    self.source.bindings,
                    self.source.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                    id,
                    position,
                );
                let checked = actual
                    .ok_or_else(|| "projected parameter actual is unavailable".to_owned())
                    .and_then(|(file, node)| {
                        if self.source.default_collection(file, node) {
                            return Ok(Vec::new());
                        }
                        self.dependencies(
                            file,
                            node,
                            view,
                            if file == clause.file
                                && clause.node.range().start <= node.range().start
                                && node.range().end <= clause.node.range().end
                            {
                                &clause.generators
                            } else {
                                &[]
                            },
                        )
                    });
                if let Err(reason) = checked {
                    self.unavailable(clause, &reason, unavailable);
                    return;
                }
            }
        }
        if let Some(checked) =
            self.inspect_ignored_constraint(clause, view, id, &parameters, known, active)
        {
            match checked {
                Ok(locals) => extend(inspected, locals),
                Err(reason) => self.unavailable(clause, &reason, unavailable),
            }
            // Checked source IDs supply no enforcement or output guarantee.
            return;
        }
        if let Some(checked) =
            self.inspect_bodyless_boolean_aggregate(clause, view, id, &parameters)
        {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
            }
            return;
        }
        if let Some(checked) = self.inspect_bodyless_integer_maximum(clause, view, id, &parameters)
        {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
            }
            return;
        }
        if let Some(checked) =
            self.inspect_literal_regular(clause, view, id, &parameters, invocation.as_deref())
        {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
            }
            return;
        }
        let Some(instance) = self
            .instances
            .iter()
            .find(|i| i.id == id && i.parameters == parameters)
        else {
            self.unavailable(
                clause,
                "selected body or recursive type instantiation is unsupported",
                unavailable,
            );
            return;
        };
        let selected: Vec<_> = known
            .iter()
            .filter(|g| g.callable == id && g.parameters == parameters)
            .collect();
        if selected.is_empty() && instance.recursive {
            self.unavailable(clause,"callable output guarantees are unsupported or have no independent recursive anchor",unavailable);
        }
        if !uncertain_actual && !unsupported_actual {
            if let Err(reason) =
                self.asserted_call_indices(clause, view, instance, invocation.is_none())
            {
                self.map_boundaries(clause, view, self.boundaries, unavailable);
                self.unavailable(clause, &reason, unavailable);
                return;
            }
            if let Some(checked) = self.literal_call_outputs(clause, view, instance) {
                match checked {
                    Ok(mut outputs) => {
                        if outputs.is_empty() {
                            self.map_boundaries(clause, view, self.boundaries, unavailable);
                        }
                        for output in &mut outputs {
                            extend(&mut output.dependencies, computed_dependencies.clone());
                        }
                        out.extend(outputs);
                    }
                    Err(reason) => {
                        self.map_boundaries(clause, view, self.boundaries, unavailable);
                        self.unavailable(clause, &reason, unavailable);
                    }
                }
                return;
            }
        }
        if !unsupported_actual
            && !instance.recursive
            && selected.is_empty()
            && let Some(state) = invocation.as_deref_mut()
        {
            let before = state.actuals.len();
            match self.invocation_actuals(clause, view, instance, state) {
                Ok(()) => {
                    let mut body_unavailable = Vec::new();
                    for body in &instance.clauses {
                        self.interpret_forwarded(
                            (body, &instance.view, known),
                            &mut Vec::new(),
                            &mut body_unavailable,
                            &mut Vec::new(),
                            active,
                            Some(&mut *state),
                        );
                    }
                    let boundaries: Vec<_> = body_unavailable
                        .into_iter()
                        .map(|unavailable| Boundary {
                            callable: id,
                            parameters: parameters.clone(),
                            unavailable,
                        })
                        .collect();
                    self.map_boundaries(clause, view, &boundaries, unavailable);
                }
                Err(reason) => self.unavailable(clause, &reason, unavailable),
            }
            state.actuals.truncate(before);
            return;
        }
        if uncertain_actual || unsupported_actual {
            // Inspect every computed actual and required body boundary before
            // withholding guarantees; uncertainty cannot hide unsupported siblings.
            self.map_boundaries(clause, view, self.boundaries, unavailable);
            return;
        }
        self.map_boundaries(clause, view, self.boundaries, unavailable);
        for guarantee in selected {
            let actual = self.source.actual(clause, id, guarantee.target, view);
            let Some((file, node)) = actual else {
                self.unavailable(
                    clause,
                    "output actual/default identity is unavailable",
                    unavailable,
                );
                continue;
            };
            if guarantee.enforced_boolean
                && self.source.bindings.declarations[guarantee.target.0].role
                    == DeclarationRole::Parameter
            {
                let mut clauses = Vec::new();
                self.clauses(
                    file,
                    clause.item,
                    node,
                    view,
                    &clause.generators,
                    &mut clauses,
                );
                if clauses
                    .iter()
                    .any(|c| c.node.range() == clause.node.range() && c.file == clause.file)
                {
                    self.unavailable(
                        clause,
                        "recursive Boolean forwarding is unsupported",
                        unavailable,
                    );
                    continue;
                }
                for actual_clause in clauses {
                    self.interpret_forwarded(
                        (&actual_clause, view, known),
                        out,
                        unavailable,
                        inspected,
                        active,
                        invocation.as_deref_mut(),
                    );
                }
                continue;
            }
            let constructed_row = if invocation.is_none()
                && guarantee.coverage == DefinitionCoverage::WholeArray
                && self.source.bindings.declarations[guarantee.target.0].role
                    == DeclarationRole::Parameter
            {
                match self.constructed_call_row(clause, view, instance, guarantee.target) {
                    Ok(actual) => actual,
                    Err(reason) => {
                        self.unavailable(clause, &reason, unavailable);
                        continue;
                    }
                }
            } else {
                None
            };
            let mapped = if let Some(actual) = &constructed_row {
                Some((actual.target, actual.coverage.clone()))
            } else if self.source.bindings.declarations[guarantee.target.0].role
                != DeclarationRole::Parameter
            {
                Some((guarantee.target, guarantee.coverage.clone()))
            } else {
                self.source
                    .target(file, node, view, &clause.generators)
                    .or_else(|| {
                        (guarantee.coverage == DefinitionCoverage::WholeArray)
                            .then(|| self.source.converted_array_target(file, node, view))
                            .flatten()
                            .map(|id| (id, DefinitionCoverage::WholeArray))
                    })
            };
            let Some((target, mut coverage)) = mapped else {
                self.unavailable(
                    clause,
                    "computed output actual cannot be inverted",
                    unavailable,
                );
                continue;
            };
            if guarantee.coverage != DefinitionCoverage::WholeArray
                && matches!(
                    self.source.view(file, node, view).declarations[target.0]
                        .ty
                        .kind,
                    TypeKind::Array { .. }
                )
            {
                coverage = DefinitionCoverage::ArrayElement;
            }
            let mut dependencies = Vec::new();
            if let Some(actual) = &constructed_row {
                // The output selection is inspected, not read as a new
                // prerequisite. Genuine mapped RHS dependencies remain below.
                extend(
                    &mut dependencies,
                    actual
                        .dependencies
                        .iter()
                        .copied()
                        .filter(|id| *id != actual.target)
                        .collect(),
                );
            }
            let mut safe = true;
            for dep in &guarantee.dependencies {
                if self.source.bindings.declarations[dep.0].role != DeclarationRole::Parameter {
                    extend(&mut dependencies, vec![*dep]);
                    continue;
                }
                if let Some((file, value)) = self.source.actual(clause, id, *dep, view) {
                    let formal = &instance.view.declarations[dep.0].ty;
                    let mapped = if formal.instantiation == Instantiation::Parameter
                        && optional(formal)
                    {
                        // Optional formals enter a supported output only
                        // through total presence guards, never deopt.
                        self.source.presence_dependencies(file, value, view)
                    } else if constructed_row.is_some() {
                        let lexical = if file == clause.file
                            && clause.node.range().start <= value.range().start
                            && value.range().end <= clause.node.range().end
                        {
                            &clause.generators[..]
                        } else {
                            &[]
                        };
                        self.constructed_array_actual(file, value, view, lexical)
                            .and_then(|actual| match actual {
                                Some(actual) => {
                                    if self.source.expression_type(self.source.view(file, value, view), file, value)
                                        .is_none_or(|e| &e.ty != formal) {
                                        return Err("constructed dependency does not retain its selected formal type".into());
                                    }
                                    Ok(actual.dependencies)
                                }
                                None => self.dependencies(file, value, view, lexical),
                            })
                    } else {
                        self.dependencies(file, value, view, &[])
                    };
                    match mapped {
                        Ok(ids) => extend(&mut dependencies, ids),
                        Err(reason) => {
                            self.unavailable(clause, &reason, unavailable);
                            safe = false;
                        }
                    }
                } else {
                    self.unavailable(
                        clause,
                        "formal/default dependency mapping is unavailable",
                        unavailable,
                    );
                    safe = false;
                }
            }
            if safe {
                extend(&mut dependencies, computed_dependencies.clone());
                out.push(Output {
                    target,
                    dependencies,
                    coverage,
                    location: self.source.context.files[clause.file].location(clause.node.range()),
                    enforced_boolean: false,
                    scoped_local: false,
                });
            }
        }
    }
}
