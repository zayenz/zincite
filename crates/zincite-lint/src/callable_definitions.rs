//! Bounded callable output guarantees, independent of search advice.
use crate::callables::{
    call_argument, core_operation, find_node, formal_parameter, instantiated_body, is_expression,
    operation_fact,
};
use crate::definitions::complete_array_coverage;
use crate::domains::expression_domain;
use crate::value_safety::optional;
use crate::{
    BindingFacts, BindingResolution, CallOutcome, CallableFacts, DeclarationId, DeclarationRole,
    Definition, DefinitionCoverage, DefinitionEnforcement, DefinitionSafety, DomainFacts, FileId,
    Instantiation, InstantiationFacts, ModelContext, ReferenceKind, SourceKind, SourceLocation,
    TypeInst, TypeKind,
};
use zincite_syntax::{NodeKind, SyntaxElement, SyntaxNode, TokenKind};

#[derive(Clone, Debug)]
pub struct CallableOutput {
    pub callable: DeclarationId,
    /// Concrete formal types of this selected body, never a context-free guess.
    pub parameters: Vec<TypeInst>,
    pub target: DeclarationId,
    pub dependencies: Vec<DeclarationId>,
    pub coverage: DefinitionCoverage,
    pub location: SourceLocation,
    /// This Boolean formal is itself enforced. Its written actual can therefore
    /// supply an enforced equality; this is not arbitrary value inversion.
    pub enforced_boolean: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnavailableCallableDefinition {
    pub location: SourceLocation,
    pub targets: Vec<DeclarationId>,
    pub reason: String,
}
#[derive(Debug, Default)]
pub struct CallableDefinitionFacts {
    pub outputs: Vec<CallableOutput>,
    /// Output guarantees substituted into actually enforced model calls.
    pub definitions: Vec<Definition>,
    pub unavailable: Vec<UnavailableCallableDefinition>,
}
/// Interpret exact selected Boolean bodies and close nested output guarantees
/// to a least fixed point. All prerequisites belong to this retained context.
/// Positional/named/default arguments keep their lexical declaration identities.
/// Core equality/conjunction, unfiltered matching forall, finite sum/index_set,
/// and nonoptional standard input conversions are bounded supported families.
/// Computed outputs are never inverted; partial selections never define owners.
/// Changing recursive type instantiations, opaque bodies, options and unproved
/// operations remain unavailable rather than assumed guarantees (thesis 4.9).
pub fn resolve_callable_definitions(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
) -> CallableDefinitionFacts {
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        instances: Vec::new(),
        boundaries: Vec::new(),
    };
    let mut roots = Vec::new();
    for (file, source) in context.files.iter().enumerate() {
        if source.kind != SourceKind::User || !source.parsed.diagnostics().is_empty() {
            continue;
        }
        for (item, node) in source
            .parsed
            .tree()
            .child_nodes()
            .enumerate()
            .filter(|(_, n)| n.kind() == NodeKind::Constraint)
        {
            producer.clauses(file, item, node, calls, &[], &mut roots);
        }
    }
    for clause in &roots {
        producer.discover(clause, calls, &[]);
    }
    let mut cursor = 0;
    while cursor < producer.instances.len() {
        let instance = &producer.instances[cursor];
        let mut ancestry = instance.ancestry.clone();
        ancestry.push(instance.id);
        let clauses = instance.clauses.clone();
        let view = instance.view.clone();
        for clause in &clauses {
            producer.discover(clause, &view, &ancestry);
        }
        cursor += 1;
    }
    let mut outputs = Vec::new();
    loop {
        let before = outputs.len();
        for instance in &producer.instances {
            let mut found = Vec::new();
            for clause in &instance.clauses {
                producer.interpret(
                    clause,
                    &instance.view,
                    &outputs,
                    &mut found,
                    &mut Vec::new(),
                );
            }
            for fact in found {
                let output = CallableOutput {
                    callable: instance.id,
                    parameters: instance.parameters.clone(),
                    target: fact.target,
                    dependencies: fact.dependencies,
                    coverage: fact.coverage,
                    location: fact.location,
                    enforced_boolean: fact.enforced_boolean,
                };
                if !outputs.iter().any(|old| same_output(old, &output)) {
                    outputs.push(output);
                }
            }
        }
        if outputs.len() == before {
            break;
        }
    }
    // Close unavailable boundaries separately, after guarantees stabilize.
    // A supported clause must not hide uncertainty in another required body.
    let mut boundaries = Vec::new();
    loop {
        let before = boundaries.len();
        producer.boundaries = boundaries.clone();
        for instance in &producer.instances {
            let mut unavailable = Vec::new();
            for clause in &instance.clauses {
                producer.interpret(
                    clause,
                    &instance.view,
                    &outputs,
                    &mut Vec::new(),
                    &mut unavailable,
                );
            }
            for unavailable in unavailable {
                let boundary = Boundary {
                    callable: instance.id,
                    parameters: instance.parameters.clone(),
                    unavailable,
                };
                if !boundaries.contains(&boundary) {
                    boundaries.push(boundary);
                }
            }
        }
        if boundaries.len() == before {
            break;
        }
    }
    producer.boundaries = boundaries;
    let mut facts = CallableDefinitionFacts {
        outputs,
        ..Default::default()
    };
    for clause in roots
        .iter()
        .filter(|c| !matches!(c.kind, ClauseKind::Equality))
    {
        let mut found = Vec::new();
        let mut unavailable = Vec::new();
        producer.interpret(clause, calls, &facts.outputs, &mut found, &mut unavailable);
        for output in found {
            facts.definitions.push(Definition {
                target: output.target,
                file: clause.file,
                item: clause.item,
                location: output.location.clone(),
                value: output.location,
                instantiation: Instantiation::Decision,
                dependencies: output.dependencies,
                enforcement: DefinitionEnforcement::Enforced,
                coverage: output.coverage,
                safety: DefinitionSafety::Supported,
                cyclic: false,
            });
        }
        facts.unavailable.extend(unavailable);
    }
    facts
}
#[derive(Clone)]
struct Clause<'a> {
    file: FileId,
    item: usize,
    node: &'a SyntaxNode,
    generators: Vec<&'a SyntaxNode>,
    kind: ClauseKind,
}
#[derive(Clone, Copy)]
enum ClauseKind {
    Equality,
    Call,
    Boolean,
    Unsupported,
}
struct Instance<'a> {
    id: DeclarationId,
    parameters: Vec<TypeInst>,
    ancestry: Vec<DeclarationId>,
    view: CallableFacts,
    clauses: Vec<Clause<'a>>,
}
#[derive(Clone, PartialEq, Eq)]
struct Boundary {
    callable: DeclarationId,
    parameters: Vec<TypeInst>,
    unavailable: UnavailableCallableDefinition,
}
struct Output {
    target: DeclarationId,
    dependencies: Vec<DeclarationId>,
    coverage: DefinitionCoverage,
    location: SourceLocation,
    enforced_boolean: bool,
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    instances: Vec<Instance<'a>>,
    boundaries: Vec<Boundary>,
}
impl<'a> Producer<'a> {
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
        let range = self.context.files[file].location(node.range()).range;
        self.bindings
            .references
            .iter()
            .find(|r| {
                r.file == file
                    && range.start <= r.location.range.start
                    && r.location.range.end <= range.end
                    && r.kind == ReferenceKind::Value
            })
            .and_then(|r| match r.resolution {
                BindingResolution::Resolved(id) => Some(id),
                _ => None,
            })
    }
    fn view<'b>(
        &'b self,
        file: FileId,
        node: &SyntaxNode,
        view: &'b CallableFacts,
    ) -> &'b CallableFacts {
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
    fn resolved(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(DeclarationId, Vec<TypeInst>)> {
        match &operation_fact(self.context, self.view(file, node, view), file, node)?.outcome {
            CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } if return_type.kind == TypeKind::Bool && !optional(return_type) => {
                Some((*declaration, parameters.clone()))
            }
            _ => None,
        }
    }
    fn core(&self, file: FileId, node: &SyntaxNode, view: &CallableFacts, name: &str) -> bool {
        core_operation(
            self.context,
            self.bindings,
            self.view(file, node, view),
            file,
            node,
            name,
        )
        .is_ok_and(|v| v)
    }
    fn clauses(
        &self,
        file: FileId,
        item: usize,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        out: &mut Vec<Clause<'a>>,
    ) {
        let add = |kind| Clause {
            file,
            item,
            node,
            generators: generators.to_vec(),
            kind,
        };
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Constraint => {
                // An interpolated header label precedes the actual expression.
                if let Some(n) = children.last() {
                    self.clauses(file, item, n, view, generators, out);
                }
            }
            NodeKind::ParenthesizedExpression | NodeKind::NamedArgument => {
                if let Some(n) = children.first() {
                    self.clauses(file, item, n, view, generators, out);
                }
            }
            NodeKind::AnnotatedExpression => {
                let harmless = children.iter().filter(|n| n.kind() == NodeKind::Annotation)
                    .all(|n| n.child_nodes().next().is_some_and(|v| v.children().iter().any(|c|
                        matches!(c, SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::StringLiteral))));
                if harmless {
                    if let Some(n) = children.first() {
                        self.clauses(file, item, n, view, generators, out);
                    }
                } else {
                    out.push(add(ClauseKind::Unsupported));
                }
            }
            NodeKind::BinaryExpression => {
                match operator(self.context, file, node) {
                    Some(TokenKind::Equal | TokenKind::DoubleEqual)
                        if self.core(file, node, view, "=") =>
                    {
                        out.push(add(ClauseKind::Equality))
                    }
                    Some(TokenKind::And) if self.core(file, node, view, "/\\") => {
                        for n in children {
                            self.clauses(file, item, n, view, generators, out);
                        }
                    }
                    // These controls do not unconditionally enforce operands.
                    Some(
                        TokenKind::Implies
                        | TokenKind::ReverseImplies
                        | TokenKind::Or
                        | TokenKind::Equivalence,
                    ) => {}
                    _ => out.push(add(ClauseKind::Unsupported)),
                }
            }
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                if !self.core(file, node, view, "forall") {
                    out.push(add(ClauseKind::Call));
                    return;
                }
                let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                    Some(node)
                } else {
                    children
                        .first()
                        .copied()
                        .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                };
                let Some(q) = quantified else {
                    out.push(add(ClauseKind::Unsupported));
                    return;
                };
                let Some(list) = q
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    out.push(add(ClauseKind::Unsupported));
                    return;
                };
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                if self.iterations(file, &all, view).is_ok() {
                    if let Some(body) = q
                        .child_nodes()
                        .find(|n| n.kind() != NodeKind::GeneratorList)
                    {
                        self.clauses(file, item, body, view, &all, out);
                    }
                } else {
                    // Direct filtered definitions already retain their partial
                    // extent. A demanded callable body needs a specific boundary.
                    let in_body = self.bindings.declarations.iter().any(|d| {
                        d.file == file
                            && matches!(
                                d.role,
                                DeclarationRole::Predicate
                                    | DeclarationRole::Function
                                    | DeclarationRole::Test
                            )
                            && d.syntax_range.start <= node.range().start
                            && node.range().end <= d.syntax_range.end
                    });
                    if in_body
                        || !all
                            .iter()
                            .any(|g| g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter))
                    {
                        out.push(add(ClauseKind::Unsupported));
                    }
                }
            }
            NodeKind::Expression => {
                if self.reference(file, node).is_some() {
                    out.push(add(ClauseKind::Boolean));
                }
            }
            _ => out.push(add(ClauseKind::Unsupported)),
        }
    }

    fn discover(&mut self, clause: &Clause<'a>, view: &CallableFacts, ancestry: &[DeclarationId]) {
        for node in clause.node.child_nodes() {
            let nested = Clause {
                file: clause.file,
                item: clause.item,
                node,
                generators: clause.generators.clone(),
                kind: ClauseKind::Call,
            };
            self.discover(&nested, view, ancestry);
        }
        if !matches!(
            clause.node.kind(),
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression
        ) || !matches!(clause.kind, ClauseKind::Call)
        {
            return;
        }
        let Some((id, parameters)) = self.resolved(clause.file, clause.node, view) else {
            return;
        };
        if parameters.iter().any(|t| !t.known() || optional(t))
            || self
                .instances
                .iter()
                .any(|i| i.id == id && i.parameters == parameters)
            || ancestry.contains(&id)
        {
            return;
        }
        let d = &self.bindings.declarations[id.0];
        let Some(node) = find_node(
            self.context.files[d.file].parsed.tree(),
            &d.syntax_range,
            d.role,
        ) else {
            return;
        };
        let Some(body) = node
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()
        else {
            return;
        };
        if node.child_nodes().any(|n| n.kind() == NodeKind::Annotation) {
            return;
        }
        let body_view = instantiated_body(self.context, self.bindings, self.calls, id, &parameters);
        let mut clauses = Vec::new();
        self.clauses(d.file, d.item, body, &body_view, &[], &mut clauses);
        self.instances.push(Instance {
            id,
            parameters,
            ancestry: ancestry.to_vec(),
            view: body_view,
            clauses,
        });
    }
    fn unavailable(
        &self,
        clause: &Clause<'a>,
        reason: &str,
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        let location = self.context.files[clause.file].location(clause.node.range());
        let mut targets = Vec::new();
        for r in &self.bindings.references {
            if r.file == clause.file
                && r.kind == ReferenceKind::Value
                && location.range.start <= r.location.range.start
                && r.location.range.end <= location.range.end
                && let BindingResolution::Resolved(id) = r.resolution
                && !targets.contains(&id)
            {
                targets.push(id);
            }
        }
        out.push(UnavailableCallableDefinition {
            location,
            targets,
            reason: reason.into(),
        });
    }
    fn interpret(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
    ) {
        self.interpret_forwarded((clause, view, known), out, unavailable, &mut Vec::new());
    }
    fn interpret_forwarded(
        &self,
        inputs: (&Clause<'a>, &CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
    ) {
        let (clause, _, _) = inputs;
        let key = (clause.file, clause.node.range());
        if active.contains(&key) {
            self.unavailable(
                clause,
                "recursive Boolean forwarding is unsupported",
                unavailable,
            );
            return;
        }
        active.push(key);
        self.interpret_body(inputs, out, unavailable, active);
        active.pop();
    }
    fn interpret_body(
        &self,
        inputs: (&Clause<'a>, &CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
    ) {
        let (clause, view, known) = inputs;
        match clause.kind {
            ClauseKind::Unsupported => self.unavailable(
                clause,
                "callable definition enforcement or control flow is unsupported",
                unavailable,
            ),
            ClauseKind::Boolean => {
                if !clause.generators.is_empty() {
                    return;
                }
                if let Some((target, coverage)) = self.target(clause.file, clause.node, view, &[]) {
                    out.push(Output {
                        target,
                        dependencies: Vec::new(),
                        coverage,
                        location: self.context.files[clause.file].location(clause.node.range()),
                        enforced_boolean: true,
                    });
                }
            }
            ClauseKind::Equality => {
                let nodes: Vec<_> = clause.node.child_nodes().collect();
                if nodes.len() != 2 {
                    return;
                }
                for (lhs, rhs) in [(nodes[0], nodes[1]), (nodes[1], nodes[0])] {
                    let Some((target, coverage)) =
                        self.target(clause.file, lhs, view, &clause.generators)
                    else {
                        continue;
                    };
                    if coverage == DefinitionCoverage::Scalar
                        && !clause.generators.is_empty()
                        && !clause.generators.iter().all(|g| {
                            g.child_nodes().next().is_some_and(|s| {
                                expression_domain(self.context, self.bindings, clause.file, s)
                                    .numeric_minimum()
                                    .is_ok_and(|m| m.is_some())
                            })
                        })
                    {
                        continue;
                    }
                    match self.dependencies(clause.file, rhs, view, &clause.generators) {
                        Ok(dependencies) => out.push(Output {
                            target,
                            dependencies,
                            coverage,
                            location: self.context.files[clause.file].location(clause.node.range()),
                            enforced_boolean: false,
                        }),
                        Err(reason) => self.unavailable(clause, &reason, unavailable),
                    }
                }
            }
            ClauseKind::Call => {
                let Some((id, parameters)) = self.resolved(clause.file, clause.node, view) else {
                    self.unavailable(
                        clause,
                        "callable selection is unresolved, ambiguous or unsupported",
                        unavailable,
                    );
                    return;
                };
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
                if selected.is_empty()
                    && instance
                        .clauses
                        .iter()
                        .any(|c| !matches!(c.kind, ClauseKind::Equality | ClauseKind::Boolean))
                {
                    self.unavailable(clause,"callable output guarantees are unsupported or have no independent recursive anchor",unavailable);
                }
                self.map_boundaries(clause, view, &self.boundaries, unavailable);
                for guarantee in selected {
                    let actual = self.actual(clause, id, guarantee.target, view);
                    let Some((file, node)) = actual else {
                        self.unavailable(
                            clause,
                            "output actual/default identity is unavailable",
                            unavailable,
                        );
                        continue;
                    };
                    if guarantee.enforced_boolean
                        && self.bindings.declarations[guarantee.target.0].role
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
                                active,
                            );
                        }
                        continue;
                    }
                    let mapped = if self.bindings.declarations[guarantee.target.0].role
                        != DeclarationRole::Parameter
                    {
                        Some((guarantee.target, guarantee.coverage.clone()))
                    } else {
                        self.target(file, node, view, &clause.generators)
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
                            self.view(file, node, view).declarations[target.0].ty.kind,
                            TypeKind::Array { .. }
                        )
                    {
                        coverage = DefinitionCoverage::ArrayElement;
                    }
                    let mut dependencies = Vec::new();
                    let mut safe = true;
                    for dep in &guarantee.dependencies {
                        if self.bindings.declarations[dep.0].role != DeclarationRole::Parameter {
                            extend(&mut dependencies, vec![*dep]);
                            continue;
                        }
                        if let Some((file, value)) = self.actual(clause, id, *dep, view) {
                            match self.dependencies(file, value, view, &[]) {
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
                        out.push(Output {
                            target,
                            dependencies,
                            coverage,
                            location: self.context.files[clause.file].location(clause.node.range()),
                            enforced_boolean: false,
                        });
                    }
                }
            }
        }
    }
    fn map_boundaries(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        boundaries: &[Boundary],
        out: &mut Vec<UnavailableCallableDefinition>,
    ) {
        if !matches!(clause.kind, ClauseKind::Call) {
            return;
        }
        let Some((id, parameters)) = self.resolved(clause.file, clause.node, view) else {
            return;
        };
        for boundary in boundaries
            .iter()
            .filter(|b| b.callable == id && b.parameters == parameters)
        {
            let mut targets = Vec::new();
            for target in &boundary.unavailable.targets {
                if self.bindings.declarations[target.0].role != DeclarationRole::Parameter {
                    extend(&mut targets, vec![*target]);
                } else if let Some((file, node)) = self.actual(clause, id, *target, view) {
                    let range = self.context.files[file].location(node.range()).range;
                    for reference in &self.bindings.references {
                        if reference.file == file
                            && reference.kind == ReferenceKind::Value
                            && range.start <= reference.location.range.start
                            && reference.location.range.end <= range.end
                            && let BindingResolution::Resolved(id) = reference.resolution
                        {
                            extend(&mut targets, vec![id]);
                        }
                    }
                }
            }
            let unavailable = UnavailableCallableDefinition {
                location: boundary.unavailable.location.clone(),
                targets,
                reason: boundary.unavailable.reason.clone(),
            };
            if !out.contains(&unavailable) {
                out.push(unavailable);
            }
        }
    }
    fn actual(
        &self,
        clause: &Clause<'a>,
        id: DeclarationId,
        formal: DeclarationId,
        view: &CallableFacts,
    ) -> Option<(FileId, &'a SyntaxNode)> {
        let d = &self.bindings.declarations[formal.0];
        if d.role == DeclarationRole::Parameter {
            let signature = view.signatures.iter().find(|s| s.declaration == id)?;
            let position = (0..signature.parameters.len())
                .find(|p| formal_parameter(self.context, self.bindings, id, *p) == Some(formal))?;
            call_argument(
                self.context,
                self.bindings,
                view,
                clause.file,
                clause.node,
                id,
                position,
            )
        } else {
            // Captured dependencies keep their defining scope; they are not
            // looked up again in the caller's namespace.
            find_node(
                self.context.files[d.file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )
            .map(|n| (d.file, n))
        }
    }
    fn target(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<(DeclarationId, DefinitionCoverage)> {
        let node = unwrap(node);
        if node.kind() == NodeKind::CallExpression && self.core(file, node, view, "array1d") {
            let argument = node.child_nodes().next()?;
            return self.target(file, argument, view, generators);
        }
        let (subject, indices) = if node.kind() == NodeKind::ArrayAccessExpression {
            let n: Vec<_> = node.child_nodes().collect();
            (*n.first()?, n[1..].to_vec())
        } else {
            (node, Vec::new())
        };
        if subject.kind() != NodeKind::Expression {
            return None;
        }
        let id = self.reference(file, subject)?;
        let ty = &self.view(file, subject, view).declarations[id.0].ty;
        if ty.instantiation != Instantiation::Decision || !ty.known() || optional(ty) {
            return None;
        }
        if let TypeKind::Array {
            indices: declared, ..
        } = &ty.kind
        {
            let coverage = if indices.is_empty()
                || declared.len() == 1
                    && indices.len() == 1
                    && self.exact_index(file, id, indices[0], generators, view)
            {
                DefinitionCoverage::WholeArray
            } else {
                complete_array_coverage(
                    self.context,
                    self.bindings,
                    self.instantiations,
                    self.domains,
                    (file, id),
                    (&indices, generators),
                )
            };
            Some((id, coverage))
        } else if indices.is_empty() {
            Some((id, DefinitionCoverage::Scalar))
        } else {
            None
        }
    }
    fn iterations(
        &self,
        file: FileId,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> Result<(), String> {
        for generator in generators {
            if generator.child_nodes().any(|n|n.kind()==NodeKind::WhereFilter) || !generator.children().iter().any(|c|matches!(c,SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind==TokenKind::In)){return Err("filtered or assigned iteration does not establish a whole output".into());}
            let source = generator
                .child_nodes()
                .next()
                .ok_or("iteration source is unavailable")?;
            let range = self.context.files[file].location(source.range()).range;
            let ty = self
                .view(file, source, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
                .ok_or("iteration source type is unavailable")?;
            if ty.instantiation != Instantiation::Parameter
                || optional(ty)
                || !matches!(ty.kind, TypeKind::Set(_))
            {
                return Err("iteration is not a supported finite parameter set".into());
            }
        }
        Ok(())
    }
    fn exact_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        if generators.len() != 1 {
            return false;
        }
        let g = generators[0];
        if self.iterations(file, generators, view).is_err() {
            return false;
        }
        let Some(source) = g.child_nodes().next() else {
            return false;
        };
        if !self.core(file, source, view, "index_set") {
            return false;
        }
        let Some(subject) = source.child_nodes().next() else {
            return false;
        };
        self.reference(file, unwrap(subject)) == Some(array)
            && self.reference(file, unwrap(index)).is_some_and(|binder| {
                self.bindings.declarations[binder.0].role == DeclarationRole::Generator
                    && self.bindings.declarations[binder.0].syntax_range == g.range()
            })
    }
    fn dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let range = self.context.files[file].location(node.range()).range;
        let ty = facts
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty);
        if ty.is_none_or(|t| !t.known() || optional(t)) {
            return Err("output dependency type or optionality is unsupported".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => {
                let mut ids = Vec::new();
                if let Some(id) = self.reference(file, node)
                    && self.bindings.declarations[id.0].role != DeclarationRole::Generator
                {
                    ids.push(id);
                }
                Ok(ids)
            }
            NodeKind::ArrayAccessExpression => {
                let id = self
                    .reference(
                        file,
                        unwrap(children.first().ok_or("array access subject unavailable")?),
                    )
                    .ok_or("array access identity unavailable")?;
                let supported = children.len() == 2
                    && self.exact_index(file, id, children[1], generators, view)
                    || complete_array_coverage(
                        self.context,
                        self.bindings,
                        self.instantiations,
                        self.domains,
                        (file, id),
                        (&children[1..], generators),
                    ) == DefinitionCoverage::WholeArray;
                if !supported {
                    return Err(
                        "output dependency array access has no exact traversal membership proof"
                            .into(),
                    );
                }
                Ok(vec![id])
            }
            NodeKind::BinaryExpression => {
                let name = crate::bindings::symbolic_operator(
                    operator(self.context, file, node).ok_or("operator unavailable")?,
                )
                .ok_or("operator unsupported")?;
                if !matches!(name, "=" | "!=" | "+" | "-" | "*" | "<" | "<=" | ">" | ">=")
                    || !self.core(file, node, view, name)
                {
                    return Err(
                        "output dependency operator identity or partiality is unsupported".into(),
                    );
                }
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::CallExpression => {
                if !(self.core(file, node, view, "array1d")
                    || self.core(file, node, view, "enum2int")
                    || self.core(file, node, view, "index_set"))
                {
                    return Err("arbitrary value calls do not prove output dependencies".into());
                }
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::GeneratorCallExpression => {
                if !self.core(file, node, view, "sum") {
                    return Err("output dependency iteration is unsupported".into());
                }
                let list = node
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                    .ok_or("sum generators unavailable")?;
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                self.iterations(file, &all, view)?;
                let body = node
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                    .ok_or("sum body unavailable")?;
                let mut ids = self.dependencies(file, body, view, &all)?;
                for g in list.child_nodes() {
                    extend(
                        &mut ids,
                        self.dependencies(
                            file,
                            g.child_nodes().next().ok_or("sum domain unavailable")?,
                            view,
                            generators,
                        )?,
                    );
                }
                Ok(ids)
            }
            NodeKind::ArrayLiteral => self.child_dependencies(file, &children, view, generators),
            _ => Err("output dependency control, option or value safety is unsupported".into()),
        }
    }
    fn child_dependencies(
        &self,
        file: FileId,
        children: &[&'a SyntaxNode],
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let mut ids = Vec::new();
        for child in children {
            extend(&mut ids, self.dependencies(file, child, view, generators)?);
        }
        Ok(ids)
    }
}
fn extend(ids: &mut Vec<DeclarationId>, other: Vec<DeclarationId>) {
    for id in other {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.sort_by_key(|id| id.0);
}
fn same_output(a: &CallableOutput, b: &CallableOutput) -> bool {
    a.callable == b.callable
        && a.parameters == b.parameters
        && a.target == b.target
        && a.dependencies == b.dependencies
        && a.coverage == b.coverage
        && a.enforced_boolean == b.enforced_boolean
}
fn unwrap(mut node: &SyntaxNode) -> &SyntaxNode {
    while matches!(
        node.kind(),
        NodeKind::ParenthesizedExpression | NodeKind::NamedArgument
    ) {
        let Some(n) = node.child_nodes().next() else {
            break;
        };
        node = n;
    }
    node
}
fn operator(context: &ModelContext, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
    node.children().iter().find_map(|c| {
        if let SyntaxElement::Token(i) = c {
            let kind = context.files[file].parsed.tokens()[*i].kind;
            crate::bindings::symbolic_operator(kind).map(|_| kind)
        } else {
            None
        }
    })
}
