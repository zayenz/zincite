//! Bounded callable output guarantees, independent of search advice.
use std::collections::HashMap;

use crate::callables::{
    array_concatenation, call_argument, core_operation, find_node, formal_parameter,
    instantiated_body, is_expression, operation_fact,
};
use crate::definitions::complete_array_coverage;
use crate::domains::expression_domain;
use crate::value_safety::optional;
use crate::{
    BindingFacts, BindingResolution, CallFact, CallOutcome, CallableFacts, DeclarationId,
    DeclarationRole, Definition, DefinitionCoverage, DefinitionEnforcement, DefinitionSafety,
    Domain, DomainFacts, ExpressionType, FileId, Instantiation, InstantiationFacts, ModelContext,
    ReferenceKind, SourceKind, SourceLocation, TypeInst, TypeKind,
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
    /// Supported definitions in enforced model calls and scoped model lets.
    pub definitions: Vec<Definition>,
    pub unavailable: Vec<UnavailableCallableDefinition>,
    /// Fully inspected model-let Boolean choices and integer initializers
    /// and comprehensions with unproved shape or membership. These supply no output or coverage.
    pub inspected_locals: Vec<DeclarationId>,
}
/// Interpret exact selected Boolean bodies and close nested output guarantees
/// to a least fixed point. All prerequisites belong to this retained context.
/// Positional/named/default arguments keep their lexical declaration identities.
/// Core equality/conjunction, matching forall, total parameter branches and
/// immutable parameter locals are bounded output families. Typed filtered
/// relations can be interpreted without establishing an output. Finite
/// sum/count/index_set and nonoptional standard conversions retain input identity.
/// Parameter presence tests retain optional declaration identity without extracting
/// a value. Relational scalar comparisons retain raw partiality without treating
/// uncertain array membership as a value or whole-output guarantee.
/// Computed outputs are never inverted; partial selections never define owners.
/// Root constraint lets retain checked integer initializers and functional
/// Boolean equivalences, without enforcing their inputs or exporting callable-private values.
/// Changing recursive type instantiations, opaque bodies, optional value evaluation and unproved
/// operations remain unavailable rather than assumed guarantees (thesis 4.9).
pub fn resolve_callable_definitions(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    instantiations: &InstantiationFacts,
    domains: &DomainFacts,
) -> CallableDefinitionFacts {
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
        indices.sort_unstable_by_key(|&row| (bindings.references[row].location.range.start, row));
    }
    let mut producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: Some(DirectSafetyLookups {
            expressions: &expression_indices,
            calls: &call_indices,
            value_references: &value_reference_indices,
        }),
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
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
                    &mut Vec::new(),
                );
            }
            if let Some(reciprocal) = producer.reciprocal_array_outputs(instance) {
                found = reciprocal;
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
                    &mut Vec::new(),
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
    let mut cursor = 0;
    while cursor < roots.len() {
        let first = &roots[cursor];
        let count = roots[cursor..]
            .iter()
            .take_while(|clause| clause.file == first.file && clause.item == first.item)
            .count();
        let clauses = &roots[cursor..cursor + count];
        // Equalities retain their direct producer, and core implication/disjunction
        // supply no ordinary output. Inspect these siblings only when a local
        // choice requires the entire constraint item to be understood.
        let inspection_only = |clause: &Clause<'_>| {
            matches!(clause.kind, ClauseKind::Equality)
                || operator(context, clause.file, clause.node)
                    .and_then(crate::bindings::symbolic_operator)
                    .is_some_and(|name| {
                        matches!(name, "->" | "<-" | "\\/")
                            && producer.core(clause.file, clause.node, calls, name)
                    })
        };
        let mut inspected = Vec::new();
        let mut complete = true;
        for clause in clauses.iter().filter(|clause| !inspection_only(clause)) {
            let mut found = Vec::new();
            let mut unavailable = Vec::new();
            producer.interpret_root(
                clause,
                calls,
                &facts.outputs,
                &mut found,
                &mut unavailable,
                &mut inspected,
            );
            complete &= unavailable.is_empty();
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
        if complete
            && !inspected.is_empty()
            && producer.inspected_item_iterations(first.file, first.item, calls)
            && clauses
                .iter()
                .filter(|clause| inspection_only(clause))
                .all(|clause| {
                    producer
                        .boolean_relation_dependencies(
                            clause.file,
                            clause.node,
                            calls,
                            &clause.generators,
                        )
                        .is_ok()
                })
        {
            // Conjunction splitting keeps clauses in source item order. A
            // failed sibling must not certify another let in that constraint.
            extend(&mut facts.inspected_locals, inspected);
        }
        cursor += count;
    }
    crate::definitions::mark_cycles(&mut facts.definitions);
    facts
}
pub(super) fn direct_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    generators: &[&'a SyntaxNode],
) -> DefinitionSafety {
    Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    }
    .direct_safety(source.0, source.1, calls, generators)
}
// Numeric inspection must check initialized references, not only dependencies.
pub(super) fn initialized_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode], bool),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> DefinitionSafety {
    let (file, node, generators, check_eager_integer) = source;
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let safety = producer.initialized_source_safety(file, node, calls, generators, &mut Vec::new());
    // Only newly inspected traversal syntax needs the eager arithmetic veto.
    // Preserve legacy raw safety; lazy sources supply no new admission.
    if check_eager_integer
        && !matches!(safety, DefinitionSafety::Unsupported(_))
        && crate::expression_safety(context, bindings, calls, file, node)
            != DefinitionSafety::Supported
        && let Some(reason) = producer.closed_integer_source_error(file, node, true, false)
    {
        return DefinitionSafety::Unsupported(reason);
    }
    safety
}
// A checked parameter conditional supplies an unknown bound, never its value.
// The caller owns the local declaration and formal-source correspondence.
pub(super) fn initialized_conditional_bound_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
) -> Option<DefinitionSafety> {
    let (file, node) = source;
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    };
    if node.kind() != NodeKind::ConditionalExpression
        || producer.expression_type(calls, file, node).is_none_or(|e| {
            !e.ty.known()
                || optional(&e.ty)
                || e.ty.instantiation != Instantiation::Parameter
                || e.ty.kind != TypeKind::Int
        })
    {
        return None;
    }
    let safety = producer.initialized_source_safety(file, node, calls, &[], &mut Vec::new());
    if let unsupported @ DefinitionSafety::Unsupported(_) = safety {
        return Some(unsupported);
    }
    // Inspect both branches and every closed fragment without choosing a branch
    // or waiving errors in a literal-inactive body.
    if let Some(reason) = producer.closed_integer_source_error(file, node, false, true) {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(DefinitionSafety::Unknown(
        "initialized conditional bound is unproved".into(),
    ))
}
// Inspect only a scalar set selected from a present rank-one parameter array.
// This supplies no membership or extent; all initialized sources remain checked.
pub(super) fn selected_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let (file, written, generators) = source;
    let (calls, view) = callables;
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: true,
        inactive_integer_body: None,
    };
    let node = unwrap(written);
    if node.kind() != NodeKind::ArrayAccessExpression {
        return None;
    }
    let children: Vec<_> = node.child_nodes().collect();
    let [subject, selector] = children.as_slice() else {
        return None;
    };
    let subject = unwrap(subject);
    if subject.kind() != NodeKind::Expression
        || !matches!(crate::domains::tokens(&context.files[file].parsed, subject).as_slice(),
            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
    {
        return None;
    }
    let id = producer.reference(file, subject)?;
    let declaration = &bindings.declarations[id.0];
    let declared = calls.declarations.get(id.0)?;
    let facts = producer.view(file, node, view);
    let ty = |node: &SyntaxNode| producer.expression_type(facts, file, node).map(|e| &e.ty);
    let integer = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    let integer_set = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if integer(element))
    };
    if !declaration.top_level
        || declaration.role != DeclarationRole::Value
        || declaration.instantiation != Instantiation::Parameter
        || declared.declaration != id
        || ty(subject) != Some(&declared.ty)
        || ty(selector).is_none_or(|t| !integer(t))
        || ty(node).is_none_or(|t| !integer_set(t))
        || !declared.ty.known()
        || optional(&declared.ty)
        || declared.ty.instantiation != Instantiation::Parameter
        || !matches!(&declared.ty.kind, TypeKind::Array { indices, element }
            if indices.len() == 1 && integer(&indices[0]) && integer_set(element)
                && ty(node) == Some(element.as_ref()))
    {
        return None;
    }
    // Actual preceding headers and filters are independent evaluated sources.
    for generator in generators {
        for value in generator.child_nodes() {
            let value = if value.kind() == NodeKind::WhereFilter {
                let Some(value) = value.child_nodes().next() else {
                    return Some(DefinitionSafety::Unsupported(
                        "generator filter is unavailable".into(),
                    ));
                };
                value
            } else {
                value
            };
            if let unsupported @ DefinitionSafety::Unsupported(_) =
                producer.initialized_source_safety(file, value, view, generators, &mut Vec::new())
            {
                return Some(unsupported);
            }
            if let Some(reason) = producer.closed_integer_source_error(file, value, true, true) {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
    }
    let safety =
        producer.initialized_source_safety(file, written, view, generators, &mut Vec::new());
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = producer.closed_integer_source_error(file, written, true, true)
    {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(safety)
}
// Inspect a typed integer-set source only after all semantic facts exist.
// Original calls own initialized declarations; the current view owns this source.
pub(super) fn initialized_integer_set_source_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> DefinitionSafety {
    let (file, node, generators) = source;
    let (calls, view) = callables;
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let present_integer = |ty: &TypeInst| {
        ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Parameter
            && ty.kind == TypeKind::Int
    };
    if producer
        .expression_type(producer.view(file, node, view), file, node)
        .is_none_or(|expression| {
            let ty = &expression.ty;
            !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if present_integer(element))
        })
    {
        return DefinitionSafety::Unsupported(
            "source requires a present parameter integer set".into(),
        );
    }
    let safety = producer.initialized_source_safety(file, node, view, generators, &mut Vec::new());
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = producer.closed_integer_source_error(file, node, true, true)
    {
        return DefinitionSafety::Unsupported(reason);
    }
    safety
}
// Admit only the selected present parameter integer-set extremum, never its value.
pub(super) fn initialized_parameter_set_extremum_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    callables: (&'a CallableFacts, &'a CallableFacts),
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
) -> Option<DefinitionSafety> {
    let (file, node, generators) = source;
    let written = unwrap(node);
    if written.kind() != NodeKind::CallExpression || written.child_nodes().count() != 1 {
        return None;
    }
    let (calls, view) = callables;
    let producer = Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: None,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    };
    let ty = &producer
        .expression_type(producer.view(file, written, view), file, written)?
        .ty;
    if !ty.known()
        || optional(ty)
        || ty.instantiation != Instantiation::Parameter
        || ty.kind != TypeKind::Int
    {
        return None;
    }
    let safety = producer.parameter_set_extremum_safety(file, node, view, generators)?;
    if !matches!(safety, DefinitionSafety::Unsupported(_))
        && let Some(reason) = producer.closed_integer_source_error(file, node, true, true)
    {
        return Some(DefinitionSafety::Unsupported(reason));
    }
    Some(safety)
}
// Recognize the written collection and exact selected standard length signature.
// A set-to-array matching view establishes no evaluation or cardinality fact.
pub(super) fn length_argument<'a>(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &'a SyntaxNode,
) -> Option<&'a SyntaxNode> {
    let integer = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && t.kind == TypeKind::Int
    };
    let index = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(t.kind, TypeKind::Int | TypeKind::Enum(_))
    };
    let typed = |value: &SyntaxNode| {
        let range = context.files[file].location(value.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if node.kind() != NodeKind::CallExpression
        || core_operation(context, bindings, calls, file, node, "length") != Ok(true)
        || !crate::definitions::annotations_safe(context, file, node)
        || typed(node).is_none_or(|t| !integer(t))
    {
        return None;
    }
    let arguments: Vec<_> = node.child_nodes().collect();
    let [argument] = arguments.as_slice() else {
        return None;
    };
    if argument.kind() == NodeKind::NamedArgument {
        return None;
    }
    let actual = typed(argument)?;
    if !actual.known() || optional(actual) {
        return None;
    }
    let (element, set) = match &actual.kind {
        TypeKind::Set(element)
            if actual.instantiation == Instantiation::Parameter && index(element) =>
        {
            (element.as_ref(), true)
        }
        TypeKind::Array { indices, element }
            if indices.len() == 1
                && index(&indices[0])
                && element.known()
                && !optional(element)
                && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)) =>
        {
            (element.as_ref(), false)
        }
        _ => return None,
    };
    if !operation_fact(context, calls, file, node).is_some_and(|call| {
        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
            if parameters.len() == 1 && integer(return_type)
                && parameters[0].known() && !optional(&parameters[0])
                && (!set || parameters[0].instantiation == Instantiation::Parameter)
                && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                    if indices.len() == 1 && index(&indices[0])
                        && (!set || indices[0].kind == TypeKind::Int)
                        && formal.known() && !optional(formal) && formal.kind == element.kind
                        && (!set || formal.instantiation == Instantiation::Parameter)))
    }) {
        return None;
    }
    Some(argument)
}

pub(super) struct DirectSafetyLookups<'a> {
    pub(super) expressions: &'a HashMap<(FileId, usize, usize), usize>,
    pub(super) calls: &'a HashMap<(FileId, usize), usize>,
    pub(super) value_references: &'a [Vec<usize>],
}
pub(super) fn indexed_expression_safety<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode, &[&'a SyntaxNode]),
    lookups: DirectSafetyLookups<'a>,
) -> DefinitionSafety {
    Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups: Some(lookups),
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    }
    .direct_safety(source.0, source.1, calls, source.2)
}
// Admit only this checked reshape to the raw-definition inspection fallback.
pub(super) fn indexed_set_axis_integer_reshape<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    lookups: DirectSafetyLookups<'a>,
) -> bool {
    set_axis_integer_reshape_source(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        source,
        Some(lookups),
    )
    .is_some()
}
// A successful standard reshape retains every value of this exact source.
pub(super) fn set_axis_integer_reshape_source<'a>(
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    source: (FileId, &'a SyntaxNode),
    lookups: Option<DirectSafetyLookups<'a>>,
) -> Option<&'a SyntaxNode> {
    Producer {
        context,
        bindings,
        calls,
        instantiations,
        domains,
        lookups,
        instances: Vec::new(),
        boundaries: Vec::new(),
        selected_set_source: false,
        inactive_integer_body: None,
    }
    .set_axis_integer_reshape_arguments(source.0, source.1, calls)
    .map(|arguments| arguments[2])
}
// A supported symbolic bound, not a nonempty-array or value guarantee.
pub(super) fn parameter_index_extremum(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    file: FileId,
    node: &SyntaxNode,
) -> Option<DeclarationId> {
    let node = unwrap(node);
    let integer = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && t.kind == TypeKind::Int
    };
    let set_type = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(&t.kind, TypeKind::Set(element) if integer(element))
    };
    let array_type = |t: &TypeInst| {
        t.known()
            && !optional(t)
            && matches!(&t.kind, TypeKind::Array { indices, element }
            if matches!(indices.len(), 1 | 2) && indices.iter().all(integer)
                && element.known() && !optional(element))
    };
    let ty = |n: &SyntaxNode| {
        let range = context.files[file].location(n.range()).range;
        calls
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty)
    };
    if node.kind() != NodeKind::CallExpression
        || !(core_operation(context, bindings, calls, file, node, "min") == Ok(true)
            || core_operation(context, bindings, calls, file, node, "max") == Ok(true))
        || !crate::definitions::annotations_safe(context, file, node)
        || ty(node).is_none_or(|t| !integer(t))
        || !operation_fact(context, calls, file, node).is_some_and(|c| {
            matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                if parameters.len() == 1 && set_type(&parameters[0]))
        })
    {
        return None;
    }
    let args: Vec<_> = node.child_nodes().collect();
    let [set] = args.as_slice() else {
        return None;
    };
    let set = unwrap(set);
    let rank = if core_operation(context, bindings, calls, file, set, "index_set") == Ok(true) {
        1
    } else if core_operation(context, bindings, calls, file, set, "index_set_1of2") == Ok(true) {
        2
    } else {
        return None;
    };
    if set.kind() != NodeKind::CallExpression
        || !crate::definitions::annotations_safe(context, file, set)
        || ty(set).is_none_or(|t| !set_type(t))
        || !operation_fact(context, calls, file, set).is_some_and(|c| {
            matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                if parameters.len() == 1 && array_type(&parameters[0])
                    && matches!(&parameters[0].kind, TypeKind::Array { indices, .. } if indices.len() == rank))
        })
    {
        return None;
    }
    let subjects: Vec<_> = set.child_nodes().collect();
    let [subject] = subjects.as_slice() else {
        return None;
    };
    let subject = unwrap(subject);
    if subject.kind() != NodeKind::Expression
        || ty(subject).is_none_or(|t| {
            !array_type(t)
                || !matches!(&t.kind, TypeKind::Array { indices, .. } if indices.len() == rank)
        })
        || (rank == 2
            && !matches!(crate::domains::tokens(&context.files[file].parsed, subject).as_slice(),
            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
    {
        return None;
    }
    let id = crate::definitions::resolved_reference(context, bindings, file, subject)?;
    let d = &bindings.declarations[id.0];
    if d.file != file || d.role != DeclarationRole::Parameter {
        return None;
    }
    let formal = find_node(context.files[file].parsed.tree(), &d.syntax_range, d.role)?;
    if !crate::definitions::annotations_safe(context, file, formal)
        || formal.child_nodes().any(|n| is_expression(n.kind()))
    {
        return None;
    }
    let array = formal.child_nodes().next()?;
    let types: Vec<_> = array.child_nodes().collect();
    if array.kind() != NodeKind::ArrayType
        || types.len() != rank + 1
        || types.iter().any(|n| n.kind() != NodeKind::ScalarType)
        || types.iter().any(|n| n.child_nodes().next().is_some())
        || types[..rank].iter().any(|n| {
            !crate::domains::tokens(&context.files[file].parsed, n)
                .iter()
                .any(|t| t.kind == TokenKind::Int)
        })
    {
        return None;
    }
    Some(id)
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
    Relation,
    Conditional,
    Local,
    Assertion,
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
    recursive: bool,
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
    /// A functional local Boolean for each active model let, not an enforced operand.
    scoped_local: bool,
}
// A checked caller comprehension; never retained in callable summaries or raw facts.
struct ConstructedArray {
    target: DeclarationId,
    item_source: Option<(FileId, DeclarationId, DeclarationId)>,
    dependencies: Vec<DeclarationId>,
    coverage: DefinitionCoverage,
}
// Exists only while one zero-output root invocation is inspected. These are
// written sources, never value assumptions or cached callable boundaries.
struct InvocationActual<'a, 'b> {
    formal: DeclarationId,
    file: FileId,
    node: &'a SyntaxNode,
    view: &'b CallableFacts,
    generators: Vec<&'a SyntaxNode>,
    // The node retains a generator call; its expression type is the call result.
    collection: bool,
}
struct Invocation<'a, 'b> {
    actuals: Vec<InvocationActual<'a, 'b>>,
    reachable: Option<bool>,
}
struct Producer<'a> {
    context: &'a ModelContext,
    bindings: &'a BindingFacts,
    calls: &'a CallableFacts,
    instantiations: &'a InstantiationFacts,
    domains: &'a DomainFacts,
    lookups: Option<DirectSafetyLookups<'a>>,
    instances: Vec<Instance<'a>>,
    boundaries: Vec<Boundary>,
    // Enabled only while a typed selected parameter set source is inspected.
    selected_set_source: bool,
    // Only value arithmetic inside a literal-inactive body may be skipped.
    inactive_integer_body: Option<(FileId, std::ops::Range<usize>)>,
}
impl<'a> Producer<'a> {
    fn reciprocal_array_outputs(&self, instance: &Instance<'a>) -> Option<Vec<Output>> {
        if instance.parameters.len() != 2 || instance.clauses.len() != 4 {
            return None;
        }
        let declaration = &self.bindings.declarations[instance.id.0];
        let file = declaration.file;
        let view = &instance.view;
        let integer = |ty: &TypeInst, instantiation| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == instantiation
                && ty.kind == TypeKind::Int
        };
        let boolean_operation = |node: &SyntaxNode, name: &str| {
            if !self.core(file, node, view, name)
                || self
                    .expression_type(view, file, node)
                    .is_none_or(|expression| {
                        !expression.ty.known()
                            || optional(&expression.ty)
                            || expression.ty.kind != TypeKind::Bool
                    })
            {
                return false;
            }
            let Some(CallFact {
                outcome:
                    CallOutcome::Resolved {
                        parameters,
                        return_type,
                        ..
                    },
                ..
            }) = self.operation_fact(view, file, node)
            else {
                return false;
            };
            let [left, right] = parameters.as_slice() else {
                return false;
            };
            return_type.known()
                && !optional(return_type)
                && return_type.kind == TypeKind::Bool
                && integer(left, left.instantiation)
                && if name == "=" {
                    integer(right, right.instantiation)
                } else {
                    right.known()
                        && !optional(right)
                        && right.instantiation == Instantiation::Parameter
                        && matches!(&right.kind, TypeKind::Set(element) if integer(element, Instantiation::Parameter))
                }
        };
        let mut formals = Vec::new();
        for (position, ty) in instance.parameters.iter().enumerate() {
            if !ty.known() || optional(ty) || ty.instantiation != Instantiation::Decision {
                return None;
            }
            let TypeKind::Array { indices, element } = &ty.kind else {
                return None;
            };
            if indices.len() != 1
                || !integer(&indices[0], Instantiation::Parameter)
                || !integer(element, Instantiation::Decision)
            {
                return None;
            }
            let id = formal_parameter(self.context, self.bindings, instance.id, position)?;
            let d = &self.bindings.declarations[id.0];
            let written = find_node(
                self.context.files[file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )?;
            if !crate::definitions::annotations_safe(self.context, file, written)
                || written.child_nodes().any(|n| is_expression(n.kind()))
            {
                return None;
            }
            let header = written.child_nodes().next()?;
            let components: Vec<_> = header.child_nodes().collect();
            if header.kind() != NodeKind::ArrayType
                || components.len() != 2
                || components.iter().any(|n| {
                    n.kind() != NodeKind::ScalarType
                        || n.child_nodes().next().is_some()
                        || !crate::domains::tokens(&self.context.files[file].parsed, n)
                            .iter()
                            .any(|t| t.kind == TokenKind::Int)
                })
            {
                return None;
            }
            formals.push(id);
        }
        let bare = |node: &SyntaxNode| {
            let node = unwrap(node);
            let tokens = crate::domains::tokens(&self.context.files[file].parsed, node);
            (node.kind() == NodeKind::Expression
                && tokens.len() == 1
                && matches!(
                    tokens[0].kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                ))
            .then(|| self.reference(file, node))
            .flatten()
        };
        let access = |node: &'a SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::ArrayAccessExpression
                || !integer(
                    &self.expression_type(view, file, node)?.ty,
                    Instantiation::Decision,
                )
            {
                return None;
            }
            let children: Vec<_> = node.child_nodes().collect();
            let [subject, index] = children.as_slice() else {
                return None;
            };
            Some((bare(subject)?, *index))
        };
        let index_set = |node: &SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "index_set")
            {
                return None;
            }
            let ty = &self.expression_type(view, file, node)?.ty;
            if !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if integer(element, Instantiation::Parameter))
            {
                return None;
            }
            let mut children = node.child_nodes();
            let subject = children.next()?;
            if children.next().is_some() {
                return None;
            }
            bare(subject)
        };
        let mut directions = Vec::new();
        for equality in instance
            .clauses
            .iter()
            .filter(|c| matches!(c.kind, ClauseKind::Equality))
        {
            if equality.file != file
                || equality.generators.len() != 1
                || !boolean_operation(equality.node, "=")
                || !crate::definitions::annotations_safe(self.context, file, equality.node)
            {
                return None;
            }
            let operands: Vec<_> = equality.node.child_nodes().collect();
            let [nested, result] = operands.as_slice() else {
                return None;
            };
            let (target, inner) = access(nested)?;
            let (source, index) = access(inner)?;
            let binder = bare(result)?;
            if source == target
                || !formals.contains(&source)
                || !formals.contains(&target)
                || bare(index) != Some(binder)
                || !integer(&view.declarations[binder.0].ty, Instantiation::Parameter)
                || !self.exact_index(file, source, index, &equality.generators, view)
            {
                return None;
            }
            let generator = equality.generators[0];
            let tokens = crate::domains::tokens(&self.context.files[file].parsed, generator);
            if tokens.iter().position(|t| t.kind == TokenKind::In) != Some(1)
                || !matches!(
                    tokens[0].kind,
                    TokenKind::Identifier | TokenKind::QuotedIdentifier
                )
                || generator.child_nodes().next().and_then(index_set) != Some(source)
                || !crate::definitions::annotations_safe(self.context, file, generator)
            {
                return None;
            }
            let membership = instance.clauses.iter().find(|c| {
                if !matches!(c.kind, ClauseKind::Relation)
                    || c.file != file
                    || c.generators.len() != 1
                    || c.generators[0].range() != generator.range()
                    || !boolean_operation(c.node, "in")
                    || !crate::definitions::annotations_safe(self.context, file, c.node)
                {
                    return false;
                }
                let operands: Vec<_> = c.node.child_nodes().collect();
                let [value, indices] = operands.as_slice() else {
                    return false;
                };
                access(value).is_some_and(|(array, position)| {
                    array == source && bare(position) == Some(binder)
                }) && index_set(indices) == Some(target)
            })?;
            directions.push((source, target, binder, membership.node.range()));
        }
        let [first, second] = directions.as_slice() else {
            return None;
        };
        if first.0 != second.1
            || first.1 != second.0
            || first.2 == second.2
            || first.3 == second.3
            || instance
                .clauses
                .iter()
                .filter(|c| matches!(c.kind, ClauseKind::Relation))
                .count()
                != 2
        {
            return None;
        }
        // The two memberships and inverse equations make both arrays bijections,
        // including empty axes. Each whole array still depends on its opposite.
        Some(
            instance
                .clauses
                .iter()
                .filter(|c| matches!(c.kind, ClauseKind::Equality))
                .zip(&directions)
                .map(|(clause, (source, target, _, _))| Output {
                    target: *target,
                    dependencies: vec![*source],
                    coverage: DefinitionCoverage::WholeArray,
                    location: self.context.files[file].location(clause.node.range()),
                    enforced_boolean: false,
                    scoped_local: false,
                })
                .collect(),
        )
    }
    fn reference(&self, file: FileId, node: &SyntaxNode) -> Option<DeclarationId> {
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
    fn expression_type<'b>(
        &'b self,
        facts: &'b CallableFacts,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<&'b ExpressionType> {
        let range = node.range();
        let offset = self.context.files[file].byte_offset;
        let range = range.start + offset..range.end + offset;
        if std::ptr::eq(facts, self.calls)
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
    fn operation_fact<'b>(
        &'b self,
        facts: &'b CallableFacts,
        file: FileId,
        node: &SyntaxNode,
    ) -> Option<&'b CallFact> {
        if std::ptr::eq(facts, self.calls)
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
    fn view<'b>(
        &'b self,
        file: FileId,
        node: &SyntaxNode,
        view: &'b CallableFacts,
    ) -> &'b CallableFacts {
        if std::ptr::eq(view, self.calls) {
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
    fn resolved(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
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
    fn core(&self, file: FileId, node: &SyntaxNode, view: &CallableFacts, name: &str) -> bool {
        let facts = self.view(file, node, view);
        if std::ptr::eq(facts, self.calls) && self.lookups.is_some() {
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
    fn inspected_item_iterations(&self, file: FileId, item: usize, view: &CallableFacts) -> bool {
        let Some(root) = self.context.files[file]
            .parsed
            .tree()
            .child_nodes()
            .nth(item)
            .filter(|n| n.kind() == NodeKind::Constraint)
        else {
            return false;
        };
        let mut pending = vec![(root, Vec::new())];
        while let Some((node, ambient)) = pending.pop() {
            let forall = matches!(
                node.kind(),
                NodeKind::CallExpression | NodeKind::GeneratorCallExpression
            ) && self.core(file, node, view, "forall");
            let quantified = if forall && node.kind() == NodeKind::CallExpression {
                node.child_nodes()
                    .next()
                    .filter(|n| n.kind() == NodeKind::ArrayComprehension)
            } else {
                Some(node)
            };
            if let Some(q) = quantified
                && let Some(list) = q
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
            {
                let mut all = ambient.clone();
                all.extend(list.child_nodes());
                // Model-root filtered traversals can be omitted from ordinary
                // output clauses. Such an omission must not certify a local choice.
                if forall
                    && self
                        .relation_iterations(file, &all, ambient.len(), view, false)
                        .is_err()
                {
                    return false;
                }
                for (position, generator) in all.iter().enumerate().skip(ambient.len()) {
                    for child in generator.child_nodes() {
                        let visible = if child.kind() == NodeKind::WhereFilter {
                            position + 1
                        } else {
                            position
                        };
                        pending.push((child, all[..visible].to_vec()));
                    }
                }
                pending.extend(
                    q.child_nodes()
                        .filter(|n| n.kind() != NodeKind::GeneratorList)
                        .map(|n| (n, all.clone())),
                );
                continue;
            }
            if forall {
                return false;
            }
            pending.extend(node.child_nodes().map(|n| (n, ambient.clone())));
        }
        true
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
                    .all(|n| n.child_nodes().next().is_some_and(|v| {
                        if v.children().iter().any(|c| matches!(c, SyntaxElement::Token(i)
                            if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::StringLiteral)) {
                            return true;
                        }
                        let value = unwrap(v);
                        value.kind() == NodeKind::Expression
                            && crate::domains::tokens(&self.context.files[file].parsed, value).len() == 1
                            && self.reference(file, value).is_some_and(|id| {
                                let declaration = &self.bindings.declarations[id.0];
                                declaration.role == DeclarationRole::Annotation
                                    && declaration.name == "domain"
                                    && self.context.files[declaration.file].kind == SourceKind::StandardLibrary
                                    && self.context.files[declaration.file].implicit
                            })
                    }));
                if harmless {
                    if let Some(n) = children.first() {
                        self.clauses(file, item, n, view, generators, out);
                    }
                } else {
                    out.push(add(ClauseKind::Unsupported));
                }
            }
            NodeKind::BinaryExpression => match operator(self.context, file, node) {
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
                Some(
                    TokenKind::Implies
                    | TokenKind::ReverseImplies
                    | TokenKind::Or
                    | TokenKind::Equivalence
                    | TokenKind::NotEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
                    | TokenKind::In,
                ) => out.push(add(ClauseKind::Relation)),
                _ => out.push(add(ClauseKind::Unsupported)),
            },
            NodeKind::CallExpression | NodeKind::GeneratorCallExpression => {
                if node.kind() == NodeKind::CallExpression && self.core(file, node, view, "assert")
                {
                    out.push(add(ClauseKind::Assertion));
                    return;
                }
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
                if self
                    .relation_iterations(file, &all, generators.len(), view, false)
                    .is_ok()
                {
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
                    // A failed conditional traversal still needs its complete
                    // source inspection even though filters establish no output.
                    let conditional_body = node.kind() == NodeKind::GeneratorCallExpression
                        && q.child_nodes()
                            .find(|n| n.kind() != NodeKind::GeneratorList)
                            .is_some_and(|body| {
                                unwrap(body).kind() == NodeKind::ConditionalExpression
                            });
                    if in_body
                        || conditional_body
                        || !all
                            .iter()
                            .any(|g| g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter))
                    {
                        out.push(add(ClauseKind::Unsupported));
                    }
                }
            }
            NodeKind::ArrayAccessExpression => {
                // An enforced Boolean member is a relation, never a whole-array output.
                let range = self.context.files[file].location(node.range()).range;
                let boolean = self.view(file, node, view).expressions.iter().any(|e| {
                    e.file == file
                        && e.location.range == range
                        && e.ty.kind == TypeKind::Bool
                        && !optional(&e.ty)
                });
                out.push(add(if boolean {
                    ClauseKind::Relation
                } else {
                    ClauseKind::Unsupported
                }));
            }
            NodeKind::ConditionalExpression => out.push(add(ClauseKind::Conditional)),
            NodeKind::LetExpression => out.push(add(ClauseKind::Local)),
            NodeKind::UnaryExpression => out.push(add(ClauseKind::Relation)),
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
        if ancestry.contains(&id) {
            if let Some(instance) = self
                .instances
                .iter_mut()
                .find(|i| i.id == id && i.parameters == parameters)
            {
                instance.recursive = true;
            }
            return;
        }
        if parameters.iter().any(|t| !t.known())
            || self
                .instances
                .iter()
                .any(|i| i.id == id && i.parameters == parameters)
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
        if !self.callable_annotations_safe(d.file, node) {
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
            recursive: false,
        });
    }
    fn callable_annotations_safe(&self, file: FileId, node: &SyntaxNode) -> bool {
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
                // This standard hint permits body inspection; it supplies no proof
                // of totality and does not bypass any body prerequisite.
                tokens.len() == 1
                    && self.reference(file, value).is_some_and(|id| {
                        let declaration = &self.bindings.declarations[id.0];
                        declaration.role == DeclarationRole::Annotation
                            && declaration.name == "promise_total"
                            && self.context.files[declaration.file].kind
                                == SourceKind::StandardLibrary
                            && self.context.files[declaration.file].implicit
                    })
            })
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
        inspected: &mut Vec<DeclarationId>,
    ) {
        self.interpret_forwarded(
            (clause, view, known),
            out,
            unavailable,
            inspected,
            &mut Vec::new(),
            None,
        );
    }
    fn interpret_root(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
    ) {
        let eligible = matches!(clause.kind, ClauseKind::Call)
            && self
                .resolved(clause.file, clause.node, view)
                .is_some_and(|(id, parameters)| {
                    self.instances.iter().any(|instance| {
                        instance.id == id
                            && instance.parameters == parameters
                            && !instance.recursive
                    }) && !known
                        .iter()
                        .any(|output| output.callable == id && output.parameters == parameters)
                });
        if !eligible {
            self.interpret(clause, view, known, out, unavailable, inspected);
            return;
        }
        let mut invocation = Invocation {
            actuals: Vec::new(),
            reachable: Some(true),
        };
        self.interpret_forwarded(
            (clause, view, known),
            out,
            unavailable,
            inspected,
            &mut Vec::new(),
            Some(&mut invocation),
        );
    }
    fn interpret_forwarded<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
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
        if let Some(checked) = self.inspect_private_index_conditional(
            inputs,
            unavailable,
            active,
            invocation.as_deref_mut(),
        ) {
            if let Err(reason) = checked {
                self.unavailable(clause, &reason, unavailable);
                if unavailable
                    .last()
                    .is_some_and(|row| !row.targets.is_empty())
                {
                    unavailable.push(UnavailableCallableDefinition {
                        location: self.context.files[clause.file].location(clause.node.range()),
                        targets: Vec::new(),
                        reason,
                    });
                }
            }
            active.pop();
            return;
        }
        // This owning let inspector checks locals and selectors in declaration
        // order. A context-free prescan must not reopen its partial initializer.
        if let Some(state) = invocation.as_ref()
            && (!matches!(clause.kind, ClauseKind::Local)
                || self
                    .inspect_uncertain_index_let(clause, inputs.1, None, &[])
                    .is_none())
        {
            let (_, view, _) = inputs;
            let mut nodes = vec![clause.node];
            let mut unsupported = None;
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, clause.file, node) {
                    unsupported = Some("invoked body annotation is unsupported".into());
                }
                if is_expression(node.kind())
                    && (node.kind() == NodeKind::Expression
                        || self
                            .expression_type(self.view(clause.file, node, view), clause.file, node)
                            .is_none_or(|e| e.ty.kind != TypeKind::Bool))
                {
                    let source = self.invocation_source(clause.file, node, state);
                    let checked = match source {
                        Ok(Some(actual)) => self.invocation_actual_safety(actual),
                        Ok(None) => self.initialized_source_safety(
                            clause.file,
                            node,
                            view,
                            &clause.generators,
                            &mut Vec::new(),
                        ),
                        Err(reason) => DefinitionSafety::Unsupported(reason),
                    };
                    if let DefinitionSafety::Unsupported(reason) = checked {
                        unsupported = Some(reason);
                    }
                    continue;
                }
                nodes.extend(node.child_nodes());
            }
            if let Some(reason) = unsupported {
                self.unavailable(clause, &reason, unavailable);
                if unavailable
                    .last()
                    .is_some_and(|row| !row.targets.is_empty())
                {
                    // A source failure still prevents inspection when its mapped
                    // dependencies are already searched or parameter values.
                    unavailable.push(UnavailableCallableDefinition {
                        location: self.context.files[clause.file].location(clause.node.range()),
                        targets: Vec::new(),
                        reason,
                    });
                }
                active.pop();
                return;
            }
        }
        let mut found = Vec::new();
        let mut local_choices = Vec::new();
        let before = unavailable.len();
        let direct_invocation = invocation.is_none();
        self.interpret_body(
            inputs,
            &mut found,
            unavailable,
            &mut local_choices,
            active,
            invocation,
        );
        if unavailable.len() == before {
            extend(inspected, local_choices);
        }
        // Filtered traversal does not unconditionally enforce a body output.
        // Membership may still prove its relation operands safe.
        let filtered = clause
            .generators
            .iter()
            .any(|g| g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter));
        for output in found {
            if !output.scoped_local
                && (filtered
                    || !clause.generators.is_empty()
                        && (matches!(clause.kind, ClauseKind::Call)
                            || output.coverage == DefinitionCoverage::Scalar)
                        && !clause
                            .generators
                            .iter()
                            .all(|g| self.nonempty_source(clause.file, g))
                        && !(direct_invocation
                            && matches!(clause.kind, ClauseKind::Call)
                            && output.coverage == DefinitionCoverage::WholeArray
                            && self.complete_constructed_call_output(
                                clause,
                                inputs.1,
                                inputs.2,
                                output.target,
                            )))
            {
                continue;
            }
            out.push(output);
        }
        active.pop();
    }
    fn interpret_body<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        out: &mut Vec<Output>,
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        inspected: &mut Vec<DeclarationId>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) {
        let (clause, view, known) = inputs;
        match clause.kind {
            ClauseKind::Unsupported => {
                if clause.node.kind() == NodeKind::GeneratorCallExpression
                    && self.core(clause.file, clause.node, view, "forall")
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
                            if let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    clause.file,
                                    value,
                                    view,
                                    ambient,
                                    &mut sources,
                                )
                            {
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
            ClauseKind::Relation => {
                if let Err(reason) =
                    self.relation_iterations(clause.file, &clause.generators, 0, view, false)
                {
                    self.unavailable(clause, &reason, unavailable);
                    return;
                }
                if let Err(reason) = self.boolean_relation_dependencies(
                    clause.file,
                    clause.node,
                    view,
                    &clause.generators,
                ) {
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
                    match self.initialized_children_safety(
                        clause.file,
                        &values,
                        view,
                        &clause.generators,
                    ) {
                        DefinitionSafety::Unknown(_) => {}
                        DefinitionSafety::Unsupported(reason) => {
                            self.unavailable(clause, &reason, unavailable)
                        }
                        DefinitionSafety::Supported => {
                            self.unavailable(clause, &reason, unavailable)
                        }
                    }
                    return;
                }
                if self.core(clause.file, clause.node, view, "<->") {
                    let nodes: Vec<_> = clause.node.child_nodes().collect();
                    if nodes.len() == 2 {
                        for (lhs, rhs) in [(nodes[0], nodes[1]), (nodes[1], nodes[0])] {
                            let lhs = unwrap(lhs);
                            let Some(target) = (lhs.kind() == NodeKind::Expression)
                                .then(|| self.reference(clause.file, lhs))
                                .flatten()
                            else {
                                continue;
                            };
                            if !self.scoped_local(clause, target, view, TypeKind::Bool) {
                                continue;
                            }
                            let range = self.context.files[clause.file].location(rhs.range()).range;
                            let boolean =
                                self.view(clause.file, rhs, view)
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
                                    coverage: DefinitionCoverage::Scalar,
                                    location: self.context.files[clause.file]
                                        .location(clause.node.range()),
                                    enforced_boolean: false,
                                    scoped_local: true,
                                });
                            }
                        }
                    }
                }
            }
            ClauseKind::Conditional => {
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
                            .expression_type(
                                self.view(clause.file, clause.node, view),
                                clause.file,
                                clause.node,
                            )
                            .is_none_or(|e| {
                                !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool
                            })
                        {
                            return DefinitionSafety::Unsupported(reason.clone());
                        }
                        if let Err(reason) = self.relation_iterations(
                            clause.file,
                            &clause.generators,
                            0,
                            view,
                            false,
                        ) {
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
                        DefinitionSafety::Supported => {
                            self.unavailable(clause, &reason, unavailable)
                        }
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
                        let range = self.context.files[clause.file]
                            .location(nodes[0].range())
                            .range;
                        let guard = self
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
                            self.unavailable(clause, "conditional output guard is not a supported total parameter Boolean", unavailable);
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
                            Err(_)
                                if matches!(checked_guard, Some(DefinitionSafety::Unknown(_))) => {}
                            Err(reason) => {
                                self.unavailable(clause, &reason, unavailable);
                                return;
                            }
                        }
                        if let Some(state) = invocation.as_deref_mut() {
                            let value =
                                match self.invocation_guard(clause.file, nodes[0], view, state) {
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
                            .expression_type(self.view(clause.file, body, view), clause.file, body)
                            .is_none_or(|e| {
                                !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool
                            })
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
                if let Some(state) = invocation.as_deref_mut() {
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
                                location: self.context.files[clause.file]
                                    .location(clause.node.range()),
                                enforced_boolean: candidate.enforced_boolean,
                                scoped_local: false,
                            });
                        }
                    }
                }
            }
            ClauseKind::Local => {
                let range = self.context.files[clause.file]
                    .location(clause.node.range())
                    .range;
                if self
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
                if let Some(result) = self.inspect_uncertain_index_let(clause, view, None, &[]) {
                    if let Err(reason) = result {
                        self.unavailable(clause, &reason, unavailable);
                    }
                    // No output holds when an initializer can abort the whole let.
                    return;
                }
                if let Some(safety) = self.parameter_set_let_safety(
                    clause.file,
                    clause.node,
                    view,
                    &clause.generators,
                ) {
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
                        if !crate::definitions::annotations_safe(self.context, clause.file, local) {
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
                        let declaration = self.bindings.declarations.iter().find(|d| {
                            d.file == clause.file
                                && d.syntax_range == local.range()
                                && d.role == DeclarationRole::Local
                        });
                        let Some(declaration) =
                            declaration.filter(|_| local.kind() == NodeKind::Declaration)
                        else {
                            self.unavailable(
                                clause,
                                "local declaration is unsupported",
                                unavailable,
                            );
                            return;
                        };
                        let ty = &view.declarations[declaration.id.0].ty;
                        if !ty.known() || optional(ty) {
                            self.unavailable(
                                clause,
                                "local declaration type or optionality is unsupported",
                                unavailable,
                            );
                            return;
                        }
                        let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
                        let scoped_array = ty.instantiation == Instantiation::Decision
                            && matches!(&ty.kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                    && indices[0].kind == TypeKind::Int
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && element.known() && !optional(element) && element.kind == TypeKind::Int)
                            && initializer.is_some();
                        if scoped_array {
                            let initializer = unwrap(initializer.unwrap());
                            if initializer.kind() == NodeKind::ArrayComprehension {
                                if !self.scoped_local(clause, declaration.id, view, ty.kind.clone())
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
                                if let DefinitionSafety::Unsupported(reason) = self
                                    .integer_comprehension_safety(
                                        clause.file,
                                        initializer,
                                        view,
                                        &clause.generators,
                                    )
                                {
                                    self.unavailable(clause, &reason, unavailable);
                                    return;
                                }
                                uncertain_initialization = true;
                                uncertain_locals.push(declaration.id);
                                private.push(declaration.id);
                                continue;
                            }
                            let cells: Vec<_> = initializer.child_nodes().collect();
                            let range = self.context.files[clause.file]
                                .location(initializer.range())
                                .range;
                            let axis = match &self.domains.declarations[declaration.id.0].domain {
                                Domain::Array { indices, .. } if indices.len() == 1 => {
                                    matches!(&indices[0], Domain::Range { lower, upper }
                                        if crate::domains::invariant_integer(lower).ok() == Some(Some(1))
                                            && crate::domains::invariant_integer(upper).ok().flatten()
                                                == i64::try_from(cells.len()).ok())
                                }
                                _ => false,
                            };
                            if !self.scoped_local(clause, declaration.id, view, ty.kind.clone())
                                || initializer.kind() != NodeKind::ArrayLiteral
                                || cells.is_empty()
                                || cells.iter().any(|cell| cell.kind() == NodeKind::IndexedArrayEntry)
                                || !axis
                                || !view.expressions.iter().any(|e| {
                                    e.file == clause.file && e.location.range == range
                                        && e.ty.known() && !optional(&e.ty)
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
                            match self.direct_children_safety(
                                clause.file,
                                &cells,
                                view,
                                &clause.generators,
                            ) {
                                DefinitionSafety::Unsupported(reason) => {
                                    self.unavailable(clause, &reason, unavailable);
                                    return;
                                }
                                DefinitionSafety::Unknown(_) => {
                                    uncertain_locals.push(declaration.id)
                                }
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
                                        location: self.context.files[clause.file]
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
                            let range = self.context.files[clause.file]
                                .location(initializer.unwrap().range())
                                .range;
                            if !self.scoped_local(clause, declaration.id, view, TypeKind::Int)
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
                            let boolean_array = matches!(&ty.kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && indices[0].kind == TypeKind::Int
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && element.kind == TypeKind::Bool);
                            if boolean_array {
                                let index = local
                                    .child_nodes()
                                    .find(|n| n.kind() == NodeKind::ArrayType)
                                    .and_then(|n| n.child_nodes().next())
                                    .filter(|n| n.kind() == NodeKind::DomainType)
                                    .and_then(|n| n.child_nodes().next())
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
                            if ty.kind == TypeKind::Bool || boolean_array {
                                if let Err(reason) = self.type_dependencies(
                                    clause.file,
                                    local,
                                    view,
                                    &clause.generators,
                                ) {
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
                                    location: self.context.files[clause.file]
                                        .location(initializer.unwrap().range()),
                                    enforced_boolean: false,
                                    scoped_local: true,
                                });
                                private.push(declaration.id);
                            }
                            Ok(ids) => extend(&mut initialization, ids),
                            Err(reason) => {
                                if (ty.instantiation == Instantiation::Parameter
                                    || (scoped_integer
                                        && unwrap(initializer.unwrap()).kind()
                                            == NodeKind::ArrayAccessExpression))
                                    && self
                                        .type_dependencies(
                                            clause.file,
                                            local,
                                            view,
                                            &clause.generators,
                                        )
                                        .is_ok()
                                    && matches!(
                                        self.direct_safety(
                                            clause.file,
                                            initializer.unwrap(),
                                            view,
                                            &clause.generators
                                        ),
                                        DefinitionSafety::Unknown(_)
                                    )
                                {
                                    uncertain_initialization = true;
                                    if scoped_integer {
                                        uncertain_locals.push(declaration.id);
                                        private.push(declaration.id);
                                    }
                                } else {
                                    self.unavailable(clause, &format!("local output initializer or domain is unsupported: {reason}"), unavailable);
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
                        && self.core(body.file, body.node, view, "=")
                    {
                        let operands: Vec<_> = body.node.child_nodes().collect();
                        let integers = operands.len() == 2
                            && operands.iter().all(|node| {
                                let range =
                                    self.context.files[body.file].location(node.range()).range;
                                self.view(body.file, node, view)
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
                            if let DefinitionSafety::Unsupported(reason) = self
                                .direct_children_safety(
                                    body.file,
                                    &operands,
                                    view,
                                    &body.generators,
                                )
                            {
                                self.unavailable(&body, &reason, unavailable);
                            }
                            continue;
                        }
                    }
                    self.interpret_forwarded(
                        (&body, view, known),
                        &mut found,
                        unavailable,
                        inspected,
                        active,
                        invocation.as_deref_mut(),
                    );
                }
                if unavailable.len() == before {
                    extend(inspected, uncertain_locals);
                    for id in &private {
                        let declaration = &self.bindings.declarations[id.0];
                        let source = &self.context.files[declaration.file];
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
                                        && self.scoped_local(
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
            ClauseKind::Assertion => {
                let Some(args) = self.assertion_arguments(clause.file, clause.node, view) else {
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
                        self.context,
                        clause.file,
                        clause.node,
                    )
                    && self
                        .operation_fact(
                            self.view(clause.file, clause.node, view),
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
                    let range = self.context.files[*file].location(argument.range()).range;
                    let ty = self
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
                        t.kind != expected
                            || t.optional
                            || t.instantiation != Instantiation::Parameter
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
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(
                                *file,
                                argument,
                                view,
                                &clause.generators,
                                &mut Vec::new(),
                            )
                        {
                            unsupported = Some(reason);
                        }
                        if let Some(reason) =
                            self.closed_integer_source_error(*file, argument, true, true)
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
                        crate::domains::tokens(
                            &self.context.files[condition_file].parsed,
                            condition,
                        )
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
                            .expression_type(self.view(*file, body, view), *file, body)
                            .is_none_or(|e| {
                                !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool
                            })
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
                        self.expression_type(self.view(file, node, view), file, node)
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
                            location: self.context.files[clause.file].location(clause.node.range()),
                            targets: Vec::new(),
                            reason: "false assertion condition aborts evaluation".into(),
                        });
                    }
                }
            }
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
                        scoped_local: false,
                    });
                }
            }
            ClauseKind::Equality => {
                let nodes: Vec<_> = clause.node.child_nodes().collect();
                if nodes.len() != 2 {
                    return;
                }
                if nodes.iter().any(|node| {
                    let range = self.context.files[clause.file].location(node.range()).range;
                    self.view(clause.file, node, view)
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
                        self.target(clause.file, lhs, view, &clause.generators)
                    else {
                        continue;
                    };
                    match self.dependencies(clause.file, rhs, view, &clause.generators) {
                        Ok(dependencies) => out.push(Output {
                            target,
                            dependencies,
                            coverage,
                            location: self.context.files[clause.file].location(clause.node.range()),
                            enforced_boolean: false,
                            scoped_local: false,
                        }),
                        Err(reason) => {
                            let boolean_operands = nodes.iter().all(|node| {
                                let range =
                                    self.context.files[clause.file].location(node.range()).range;
                                self.view(clause.file, node, view)
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
                            if self.uncertain_integer_equality(clause, view) {
                                // Checked selection uncertainty inspects the relation,
                                // but cannot establish either output direction.
                                out.truncate(before);
                                return;
                            }
                            self.unavailable(clause, &reason, unavailable);
                        }
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
                if clause.node.kind() == NodeKind::GeneratorCallExpression
                    && self.core(clause.file, clause.node, view, "exists")
                    && self.operation_fact(self.view(clause.file, clause.node, view), clause.file, clause.node)
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
                let mut computed_dependencies = Vec::new();
                let mut uncertain_actual = false;
                let mut unsupported_actual = false;
                for position in 0..parameters.len() {
                    let Some((file, value)) = call_argument(
                        self.context,
                        self.bindings,
                        self.view(clause.file, clause.node, view),
                        clause.file,
                        clause.node,
                        id,
                        position,
                    ) else {
                        continue;
                    };
                    if !self.default_collection(file, value) {
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
                if let Some(call) = self.operation_fact(
                    self.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                ) && let CallOutcome::Resolved {
                    parameters: formal, ..
                } = &call.outcome
                {
                    for (position, (declared, concrete)) in
                        formal.iter().zip(&parameters).enumerate()
                    {
                        if declared.instantiation != Instantiation::Decision
                            || concrete.instantiation != Instantiation::Parameter
                        {
                            continue;
                        }
                        let actual = call_argument(
                            self.context,
                            self.bindings,
                            self.view(clause.file, clause.node, view),
                            clause.file,
                            clause.node,
                            id,
                            position,
                        );
                        let checked = actual
                            .ok_or_else(|| "projected parameter actual is unavailable".to_owned())
                            .and_then(|(file, node)| {
                                if self.default_collection(file, node) {
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
                    if let Err(reason) = checked {
                        self.unavailable(clause, &reason, unavailable);
                    }
                    // The wrapper may be ignored: never forward outputs or locals.
                    return;
                }
                if let Some(checked) =
                    self.inspect_bodyless_integer_maximum(clause, view, id, &parameters)
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
                        self.map_boundaries(clause, view, &self.boundaries, unavailable);
                        self.unavailable(clause, &reason, unavailable);
                        return;
                    }
                    if let Some(mut outputs) = self.literal_call_outputs(clause, view, instance) {
                        for output in &mut outputs {
                            extend(&mut output.dependencies, computed_dependencies.clone());
                        }
                        out.extend(outputs);
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
                    self.map_boundaries(clause, view, &self.boundaries, unavailable);
                    return;
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
                                inspected,
                                active,
                                invocation.as_deref_mut(),
                            );
                        }
                        continue;
                    }
                    let constructed_row = if invocation.is_none()
                        && guarantee.coverage == DefinitionCoverage::WholeArray
                        && self.bindings.declarations[guarantee.target.0].role
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
                    } else if self.bindings.declarations[guarantee.target.0].role
                        != DeclarationRole::Parameter
                    {
                        Some((guarantee.target, guarantee.coverage.clone()))
                    } else {
                        self.target(file, node, view, &clause.generators)
                            .or_else(|| {
                                (guarantee.coverage == DefinitionCoverage::WholeArray)
                                    .then(|| self.converted_array_target(file, node, view))
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
                            self.view(file, node, view).declarations[target.0].ty.kind,
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
                        if self.bindings.declarations[dep.0].role != DeclarationRole::Parameter {
                            extend(&mut dependencies, vec![*dep]);
                            continue;
                        }
                        if let Some((file, value)) = self.actual(clause, id, *dep, view) {
                            let formal = &instance.view.declarations[dep.0].ty;
                            let mapped = if formal.instantiation == Instantiation::Parameter
                                && optional(formal)
                            {
                                // Optional formals enter a supported output only
                                // through total presence guards, never deopt.
                                self.presence_dependencies(file, value, view)
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
                                            if self.expression_type(self.view(file, value, view), file, value)
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
                            location: self.context.files[clause.file].location(clause.node.range()),
                            enforced_boolean: false,
                            scoped_local: false,
                        });
                    }
                }
            }
        }
    }
    fn uncertain_integer_equality(&self, clause: &Clause<'a>, view: &CallableFacts) -> bool {
        let file = clause.file;
        let node = clause.node;
        let operands: Vec<_> = node.child_nodes().collect();
        let typed = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|expression| &expression.ty)
        };
        let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
        if operands.len() != 2
            || !self.core(file, node, view, "=")
            || !crate::definitions::annotations_safe(self.context, file, node)
            || operands
                .iter()
                .any(|operand| typed(operand).is_none_or(|ty| !integer(ty)))
            || typed(node).is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
            || self
                .operation_fact(self.view(file, node, view), file, node)
                .is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && Some(return_type) == typed(node)
                        && parameters.iter().zip(&operands).all(|(formal, actual)| integer(formal)
                            && typed(actual).is_some_and(|ty| crate::types::coerces(ty, formal))))
                })
            || self
                .relation_iterations(file, &clause.generators, 0, view, false)
                .is_err()
        {
            return false;
        }
        // Only the demonstrated bare selection chain enters this uncertainty
        // path. Arithmetic selectors keep their original strict limitation.
        let mut selections = operands.clone();
        while let Some(value) = selections.pop() {
            let value = unwrap(value);
            match value.kind() {
                NodeKind::ArrayAccessExpression => selections.extend(value.child_nodes()),
                NodeKind::Expression => {
                    let tokens = crate::domains::tokens(&self.context.files[file].parsed, value);
                    if !matches!(tokens.as_slice(), [token] if matches!(token.kind,
                        TokenKind::Identifier | TokenKind::QuotedIdentifier | TokenKind::IntegerLiteral))
                    {
                        return false;
                    }
                    let Some(ty) = typed(value) else {
                        return false;
                    };
                    if integer(ty) {
                        if crate::domains::expression_integer(
                            self.context,
                            self.bindings,
                            file,
                            value,
                        )
                        .is_err()
                        {
                            return false;
                        }
                        if let Some(id) = self.reference(file, value) {
                            let declaration = &self.bindings.declarations[id.0];
                            if declaration.role == DeclarationRole::Value && declaration.top_level {
                                if self.domains.declarations[id.0]
                                    .domain
                                    .numeric_minimum()
                                    .is_err()
                                {
                                    return false;
                                }
                                let Some(written) = find_node(
                                    self.context.files[declaration.file].parsed.tree(),
                                    &declaration.syntax_range,
                                    declaration.role,
                                ) else {
                                    return false;
                                };
                                if written
                                    .child_nodes()
                                    .filter(|child| is_expression(child.kind()))
                                    .any(|initializer| {
                                        crate::domains::expression_integer(
                                            self.context,
                                            self.bindings,
                                            declaration.file,
                                            initializer,
                                        )
                                        .is_err()
                                    })
                                {
                                    return false;
                                }
                            } else if declaration.role != DeclarationRole::Generator {
                                return false;
                            }
                        }
                        continue;
                    }
                    let TypeKind::Array { indices, element } = &ty.kind else {
                        return false;
                    };
                    if !ty.known()
                        || optional(ty)
                        || !integer(element)
                        || !matches!(indices.len(), 1 | 2)
                        || indices.iter().any(|index| {
                            !integer(index) || index.instantiation != Instantiation::Parameter
                        })
                    {
                        return false;
                    }
                    let Some(array) = self.reference(file, value) else {
                        return false;
                    };
                    let declaration = &self.bindings.declarations[array.0];
                    if declaration.role != DeclarationRole::Value || !declaration.top_level {
                        return false;
                    }
                    let Domain::Array { element, .. } = &self.domains.declarations[array.0].domain
                    else {
                        return false;
                    };
                    if element.numeric_minimum().is_err() {
                        return false;
                    }
                    let Some(written) = find_node(
                        self.context.files[declaration.file].parsed.tree(),
                        &declaration.syntax_range,
                        declaration.role,
                    ) else {
                        return false;
                    };
                    // A literal source is inspected without treating its written
                    // defaults as values of every instance. The closed evaluator
                    // supplies only a veto, including overflow in a literal cell.
                    for initializer in written
                        .child_nodes()
                        .filter(|child| is_expression(child.kind()))
                    {
                        let initializer = unwrap(initializer);
                        if ty.instantiation != Instantiation::Parameter
                            || initializer.kind() != NodeKind::ArrayLiteral
                            || initializer.child_nodes().any(|cell| {
                                !matches!(crate::domains::tokens(&self.context.files[declaration.file].parsed,
                                    unwrap(cell)).as_slice(), [token] if token.kind == TokenKind::IntegerLiteral)
                                    || self.expression_type(self.calls, declaration.file, cell)
                                    .is_none_or(|expression| {
                                        !integer(&expression.ty)
                                            || expression.ty.instantiation
                                                != Instantiation::Parameter
                                    })
                                    || crate::domains::expression_integer(
                                        self.context,
                                        self.bindings,
                                        declaration.file,
                                        cell,
                                    )
                                    .is_err()
                            })
                        {
                            return false;
                        }
                    }
                }
                _ => return false,
            }
        }
        let mut unknown = false;
        for (position, generator) in clause.generators.iter().enumerate() {
            let Some(source) = generator.child_nodes().next() else {
                return false;
            };
            match self.initialized_source_safety(
                file,
                source,
                view,
                &clause.generators[..position],
                &mut Vec::new(),
            ) {
                DefinitionSafety::Unsupported(_) => return false,
                DefinitionSafety::Unknown(_) => unknown = true,
                DefinitionSafety::Supported => {}
            }
            if !self.uncertain_equality_header(file, source, view) {
                return false;
            }
            // Filter arithmetic remains on the strict route in this local
            // selection-only admission.
            if generator
                .child_nodes()
                .any(|child| child.kind() == NodeKind::WhereFilter)
            {
                return false;
            }
        }
        match self.initialized_children_safety(file, &operands, view, &clause.generators) {
            DefinitionSafety::Unknown(_) => true,
            DefinitionSafety::Unsupported(_) => false,
            DefinitionSafety::Supported => unknown,
        }
    }
    // A closed domain error must not disappear behind source-safety Unknown.
    // Admit only the demonstrated symbolic count header when the independent
    // domain walk cannot interpret length; no count or membership is derived.
    fn uncertain_equality_header(
        &self,
        file: FileId,
        source: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let numeric = |domain: Domain| {
            let mut domains = vec![domain];
            while let Some(domain) = domains.pop() {
                match domain {
                    Domain::Named { domain, .. } | Domain::Set(domain) => domains.push(*domain),
                    Domain::Array { indices, element } => {
                        domains.extend(indices);
                        domains.push(*element);
                    }
                    Domain::Range { lower, upper }
                        if !matches!(
                            (&lower, &upper),
                            (
                                crate::domains::NumericBound::Integer(_),
                                crate::domains::NumericBound::Integer(_)
                            )
                        ) =>
                    {
                        return false;
                    }
                    Domain::LiteralSet(bounds)
                        if bounds.is_empty()
                            || bounds.iter().any(|bound| {
                                !matches!(bound, crate::domains::NumericBound::Integer(_))
                            }) =>
                    {
                        return false;
                    }
                    domain if domain.numeric_minimum().is_err() => return false,
                    _ => {}
                }
            }
            true
        };
        let count = |file: FileId, node: &SyntaxNode| {
            let node = unwrap(node);
            let facts = self.view(file, node, view);
            let Some(argument) = length_argument(self.context, self.bindings, facts, file, node)
                .or_else(|| {
                    crate::domains::parameter_set_cardinality(
                        self.context,
                        self.bindings,
                        facts,
                        file,
                        node,
                    )
                    .and_then(|_| node.child_nodes().next())
                })
            else {
                return false;
            };
            let argument = unwrap(argument);
            if argument.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, argument).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return false;
            }
            let Some(id) = self.reference(file, argument) else {
                return false;
            };
            let declaration = &self.bindings.declarations[id.0];
            if declaration.role != DeclarationRole::Value
                || !declaration.top_level
                || !numeric(self.domains.declarations[id.0].domain.clone())
            {
                return false;
            }
            let Some(written) = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return false;
            };
            if written.child_nodes().take(1).any(|ty| {
                !numeric(expression_domain(
                    self.context,
                    self.bindings,
                    declaration.file,
                    ty,
                ))
            }) {
                return false;
            }
            // Length does not evaluate cells, but its source must not hide a
            // closed failure. Computed/reshaped initializers stay outside this
            // local admission; a literal retains only checked scalar cells.
            for initializer in written
                .child_nodes()
                .filter(|child| is_expression(child.kind()))
            {
                let initializer = unwrap(initializer);
                if matches!(self.calls.declarations[id.0].ty.kind, TypeKind::Set(_)) {
                    if !matches!(initializer.kind(), NodeKind::RangeExpression | NodeKind::SetLiteral)
                        || initializer.child_nodes().any(|bound| {
                            let bound = unwrap(bound);
                            !matches!(crate::domains::tokens(&self.context.files[declaration.file].parsed, bound).as_slice(),
                                [token] if token.kind == TokenKind::IntegerLiteral)
                        })
                    {
                        return false;
                    }
                    continue;
                }
                if initializer.kind() != NodeKind::ArrayLiteral
                    || initializer.child_nodes().next().is_none()
                {
                    return false;
                }
                for cell in initializer.child_nodes() {
                    let cell = unwrap(cell);
                    if cell.kind() != NodeKind::Expression
                        || crate::domains::expression_integer(
                            self.context,
                            self.bindings,
                            declaration.file,
                            cell,
                        )
                        .is_err()
                    {
                        return false;
                    }
                    if let Some(cell_id) = self.reference(declaration.file, cell) {
                        let cell_declaration = &self.bindings.declarations[cell_id.0];
                        if cell_declaration.role != DeclarationRole::Value
                            || !cell_declaration.top_level
                            || !numeric(self.domains.declarations[cell_id.0].domain.clone())
                        {
                            return false;
                        }
                        let Some(cell_written) = find_node(
                            self.context.files[cell_declaration.file].parsed.tree(),
                            &cell_declaration.syntax_range,
                            cell_declaration.role,
                        ) else {
                            return false;
                        };
                        if cell_written.child_nodes().any(|n| is_expression(n.kind())) {
                            return false;
                        }
                    }
                }
            }
            true
        };
        let mut source = unwrap(source);
        let mut file = file;
        let mut active = Vec::new();
        while source.kind() == NodeKind::Expression {
            let Some(id) = self.reference(file, source) else {
                return false;
            };
            let declaration = &self.bindings.declarations[id.0];
            if active.contains(&id)
                || declaration.role != DeclarationRole::Value
                || !declaration.top_level
            {
                return false;
            }
            active.push(id);
            let Some(written) = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return false;
            };
            if written.child_nodes().take(1).any(|ty| {
                !numeric(expression_domain(
                    self.context,
                    self.bindings,
                    declaration.file,
                    ty,
                ))
            }) {
                return false;
            }
            let ty = &self.calls.declarations[id.0].ty;
            if !ty.known()
                || optional(ty)
                || ty.instantiation != Instantiation::Parameter
                || !matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int
                    && element.known() && !optional(element))
            {
                return false;
            }
            let Some(initializer) = written
                .child_nodes()
                .find(|child| is_expression(child.kind()))
            else {
                return self.domains.declarations[id.0]
                    .domain
                    .numeric_minimum()
                    .is_ok();
            };
            source = unwrap(initializer);
            file = declaration.file;
        }
        let domain = expression_domain(self.context, self.bindings, file, source);
        let mut constants = vec![source];
        let mut closed = true;
        while let Some(value) = constants.pop() {
            if value.kind() == NodeKind::Expression
                && !matches!(crate::domains::tokens(&self.context.files[file].parsed, value).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
            {
                closed = false;
            }
            constants.extend(value.child_nodes());
        }
        if closed && domain.numeric_minimum().is_ok() {
            return true;
        }
        let bounds: Vec<_> = source.child_nodes().map(unwrap).collect();
        if source.kind() != NodeKind::RangeExpression
            || bounds.len() != 2
            || crate::domains::expression_integer(self.context, self.bindings, file, bounds[0])
                != Ok(Some(1))
        {
            return false;
        }
        let upper = bounds[1];
        if count(file, upper) {
            return true;
        }
        let operands: Vec<_> = upper.child_nodes().collect();
        upper.kind() == NodeKind::BinaryExpression
            && operands.len() == 2
            && self.core(file, upper, view, "div")
            && operands.iter().all(|operand| count(file, operand))
    }
    // Inspect the selected argument without treating an ignored wrapper as enforcement.
    fn inspect_ignored_constraint(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
        known: &[CallableOutput],
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
    ) -> Option<Result<(), String>> {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        if !matches!(
            declaration.name.as_str(),
            "redundant_constraint" | "symmetry_breaking_constraint"
        ) || declaration.role != DeclarationRole::Predicate
            || source.kind != SourceKind::StandardLibrary
            || !source.implicit
        {
            return None;
        }
        Some((|| {
            let written = find_node(
                source.parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("redundant constraint declaration unavailable")?;
            let boolean = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Bool;
            let formal = formal_parameter(self.context, self.bindings, id, 0)
                .ok_or("redundant constraint formal unavailable")?;
            let formals: Vec<_> = written
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
                .ok_or("redundant constraint parameters unavailable")?
                .child_nodes()
                .collect();
            let declared = &self.calls.declarations[formal.0].ty;
            if clause.node.kind() != NodeKind::CallExpression
                || clause.node.child_nodes().count() != 1
                || written.child_nodes().any(|n| is_expression(n.kind()))
                || formals.len() != 1
                || formals[0].child_nodes().any(|n| is_expression(n.kind()))
                || !boolean(declared) || declared.instantiation != Instantiation::Decision
                || parameters.len() != 1 || !boolean(&parameters[0])
                || self.operation_fact(self.view(clause.file, clause.node, view), clause.file, clause.node)
                    .is_none_or(|call| !matches!(&call.outcome,
                        CallOutcome::Resolved { declaration: selected, parameters, return_type }
                            if *selected == id && parameters.len() == 1 && parameters[0] == *declared
                                && boolean(return_type) && return_type.instantiation == Instantiation::Decision))
            {
                return Err("redundant constraint signature, body or defaults are unsupported".into());
            }
            for (file, root) in [(declaration.file, written), (clause.file, clause.node)] {
                let mut nodes = vec![root];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.context, file, node) {
                        return Err(
                            "redundant constraint declaration or call annotation is unsupported"
                                .into(),
                        );
                    }
                    nodes.extend(node.child_nodes());
                }
            }
            let (file, argument) = call_argument(
                self.context,
                self.bindings,
                self.view(clause.file, clause.node, view),
                clause.file,
                clause.node,
                id,
                0,
            )
            .ok_or("redundant constraint argument unavailable")?;
            let facts = self.view(file, argument, view);
            let ty = self
                .expression_type(facts, file, argument)
                .map(|e| &e.ty)
                .ok_or("redundant constraint argument type unavailable")?;
            if !boolean(ty) || !crate::types::coerces(ty, declared) {
                return Err(
                    "redundant constraint argument type or optionality is unsupported".into(),
                );
            }
            let lexical = if file == clause.file
                && clause.node.range().start <= argument.range().start
                && argument.range().end <= clause.node.range().end
            {
                &clause.generators[..]
            } else {
                &[]
            };
            // Inspect this complete owning scope before visiting naked Local
            // references. An ignored wrapper grants no outputs or membership.
            let quantified = unwrap(argument);
            if lexical.is_empty()
                && quantified.kind() == NodeKind::GeneratorCallExpression
                && self.core(file, quantified, facts, "forall")
            {
                let parts: Vec<_> = quantified.child_nodes().collect();
                let nested_rows = parts.get(1).is_some_and(|body| {
                    let body = unwrap(body);
                    let Some(first) = body.child_nodes().find(|n| n.kind() != NodeKind::LetBlock)
                    else {
                        return false;
                    };
                    let first = unwrap(first);
                    let Some(second) = first.child_nodes().nth(1) else {
                        return false;
                    };
                    let second = unwrap(second);
                    first.kind() == NodeKind::GeneratorCallExpression
                        && second.kind() == NodeKind::GeneratorCallExpression
                        && second
                            .child_nodes()
                            .nth(1)
                            .is_some_and(|inner| unwrap(inner).kind() == NodeKind::LetExpression)
                });
                if let [headers, body] = parts.as_slice()
                    && headers.kind() == NodeKind::GeneratorList
                    && (headers.child_nodes().count() == 2
                        || nested_rows && headers.child_nodes().count() == 1)
                    && unwrap(body).kind() == NodeKind::LetExpression
                {
                    let typed = self.expression_type(facts, file, quantified).map(|e| &e.ty);
                    let parameter_int = |t: &TypeInst| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && t.instantiation == Instantiation::Parameter
                    };
                    let decision_bool =
                        |t: &TypeInst| boolean(t) && t.instantiation == Instantiation::Decision;
                    if self.operation_fact(facts, file, quantified).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && decision_bool(return_type)
                                && Some(return_type) == typed && parameters[0].known()
                                && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && parameter_int(&indices[0])
                                        && decision_bool(element)
                                        && self.expression_type(self.view(file, body, facts), file, body)
                                            .is_some_and(|e| boolean(&e.ty)
                                                && crate::types::coerces(&e.ty, element)))))
                    {
                        return Err("redundant constraint row forall signature is unsupported".into());
                    }
                    let generators: Vec<_> = headers.child_nodes().collect();
                    match self.parameter_row_let_safety(file, body, facts, &generators, &[]) {
                        Some(DefinitionSafety::Unsupported(reason)) => return Err(reason),
                        Some(DefinitionSafety::Unknown(_)) => return Ok(()),
                        _ => {}
                    }
                }
            }
            // Boolean call bodies use the existing instance interpreter. Evaluate
            // their non-Boolean actuals and initialized references independently.
            let mut nodes = vec![argument];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, node) {
                    return Err("redundant constraint argument annotation is unsupported".into());
                }
                if node.kind() == NodeKind::Expression
                    && self.reference(file, node).is_some_and(|id| {
                        self.bindings.declarations[id.0].role == DeclarationRole::Local
                    })
                {
                    return Err(
                        "redundant constraint local source inspection is unsupported".into(),
                    );
                }
                if is_expression(node.kind())
                    && (node.kind() == NodeKind::Expression
                        || self
                            .expression_type(self.view(file, node, view), file, node)
                            .is_none_or(|e| !boolean(&e.ty)))
                {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, node, view, lexical, &mut Vec::new())
                    {
                        return Err(reason);
                    }
                    continue;
                }
                if node.kind() == NodeKind::CallExpression {
                    let (selected, concrete) = self
                        .resolved(file, node, view)
                        .ok_or("redundant constraint nested selection is unsupported")?;
                    for (position, formal) in concrete.iter().enumerate() {
                        let (actual_file, actual) = call_argument(
                            self.context,
                            self.bindings,
                            self.view(file, node, view),
                            file,
                            node,
                            selected,
                            position,
                        )
                        .ok_or("redundant constraint nested actual or default unavailable")?;
                        if actual_file == file
                            && node.range().start <= actual.range().start
                            && actual.range().end <= node.range().end
                        {
                            continue;
                        }
                        // An empty set has no evaluated cells. Its selected concrete
                        // formal supplies its type, not an instance-value certificate.
                        let empty_set = unwrap(actual).kind() == NodeKind::SetLiteral
                            && unwrap(actual).child_nodes().count() == 0
                            && formal.known()
                            && !optional(formal)
                            && formal.instantiation == Instantiation::Parameter
                            && matches!(formal.kind, TypeKind::Set(_))
                            && crate::definitions::annotations_safe(
                                self.context,
                                actual_file,
                                actual,
                            );
                        if !empty_set
                            && let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    actual_file,
                                    actual,
                                    view,
                                    &[],
                                    &mut Vec::new(),
                                )
                        {
                            return Err(reason);
                        }
                    }
                }
                nodes.extend(node.child_nodes());
            }
            let mut clauses = Vec::new();
            self.clauses(file, clause.item, argument, view, lexical, &mut clauses);
            if clauses.is_empty() {
                return Err("redundant constraint argument clauses unavailable".into());
            }
            let mut unavailable = Vec::new();
            for child in clauses {
                self.interpret_forwarded(
                    (&child, view, known),
                    &mut Vec::new(),
                    &mut unavailable,
                    &mut Vec::new(),
                    active,
                    None,
                );
            }
            match unavailable.into_iter().next() {
                Some(unavailable) => Err(unavailable.reason),
                None => Ok(()),
            }
        })())
    }
    fn inspect_bodyless_integer_maximum(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        id: DeclarationId,
        parameters: &[TypeInst],
    ) -> Option<Result<(), String>> {
        let declaration = &self.bindings.declarations[id.0];
        let source = &self.context.files[declaration.file];
        if declaration.name != "array_int_maximum"
            || declaration.role != DeclarationRole::Predicate
            || source.kind != SourceKind::StandardLibrary
            || !source.implicit
        {
            return None;
        }
        let written = find_node(
            source.parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        if written.child_nodes().any(|node| is_expression(node.kind())) {
            return None;
        }
        let integer = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.kind == TypeKind::Int
                && matches!(
                    ty.instantiation,
                    Instantiation::Parameter | Instantiation::Decision
                )
        };
        let array = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].kind == TypeKind::Int && indices[0].instantiation == Instantiation::Parameter
                    && integer(element))
        };
        Some((|| {
            let formals: Vec<_> = written
                .child_nodes()
                .find(|n| n.kind() == NodeKind::ParameterList)
                .ok_or("integer maximum primitive formals unavailable")?
                .child_nodes()
                .collect();
            let declared: Vec<_> = (0..2)
                .map(|position| {
                    formal_parameter(self.context, self.bindings, id, position)
                        .map(|formal| &self.calls.declarations[formal.0].ty)
                })
                .collect::<Option<Vec<_>>>()
                .ok_or("integer maximum declared formals unavailable")?;
            if !integer(declared[0])
                || declared[0].instantiation != Instantiation::Decision
                || !array(declared[1])
                || declared[1].instantiation != Instantiation::Decision
                || !matches!(&declared[1].kind, TypeKind::Array { element, .. } if element.instantiation == Instantiation::Decision)
            {
                return Err("integer maximum declared signature is unsupported".into());
            }
            if parameters.len() != 2 || !integer(&parameters[0]) || !array(&parameters[1])
                || formals.len() != 2 || formals.iter().any(|formal| formal.child_nodes().any(|n| is_expression(n.kind())))
                || !self.callable_annotations_safe(declaration.file, written)
                || self.operation_fact(self.view(clause.file, clause.node, view), clause.file, clause.node)
                    .is_none_or(|fact| !matches!(&fact.outcome, CallOutcome::Resolved { declaration: selected, parameters, return_type }
                        if *selected == id && parameters.len() == 2 && integer(&parameters[0]) && array(&parameters[1])
                            && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Bool))
            { return Err("integer maximum primitive signature or defaults are unsupported".into()); }
            let mut written_nodes = vec![written];
            while let Some(node) = written_nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, declaration.file, node) {
                    return Err("integer maximum primitive formal annotation is unsupported".into());
                }
                written_nodes.extend(node.child_nodes());
            }
            let mut unsupported = None;
            for position in 0..2 {
                let checked = (|| {
                    let (file, argument) = call_argument(
                        self.context,
                        self.bindings,
                        self.view(clause.file, clause.node, view),
                        clause.file,
                        clause.node,
                        id,
                        position,
                    )
                    .ok_or("integer maximum primitive argument unavailable")?;
                    let range = self.context.files[file].location(argument.range()).range;
                    if !self.view(file, argument, view).expressions.iter().any(|e| {
                        e.file == file
                            && e.location.range == range
                            && if position == 0 {
                                integer(&e.ty)
                            } else {
                                array(&e.ty)
                            }
                    }) {
                        return Err(
                            "integer maximum primitive actual type or optionality is unsupported"
                                .into(),
                        );
                    }
                    let mut nodes = vec![argument];
                    while let Some(node) = nodes.pop() {
                        if !crate::definitions::annotations_safe(self.context, file, node) {
                            return Err(
                                "integer maximum primitive argument annotation is unsupported"
                                    .into(),
                            );
                        }
                        nodes.extend(node.child_nodes());
                    }
                    let lexical = if file == clause.file
                        && clause.node.range().start <= argument.range().start
                        && argument.range().end <= clause.node.range().end
                    {
                        &clause.generators[..]
                    } else {
                        &[]
                    };
                    if let Err(reason) = self.dependencies(file, argument, view, lexical)
                        && !matches!(
                            self.direct_safety(file, argument, view, lexical),
                            DefinitionSafety::Unknown(_)
                        )
                    {
                        return Err(reason);
                    }
                    Ok(())
                })();
                if let Err(reason) = checked {
                    unsupported = Some(reason);
                }
            }
            if let Some(reason) = unsupported {
                return Err(reason);
            }
            // The primitive is inspected, not inverted. Its source may be empty;
            // no result, nonempty fact or output guarantee follows from this check.
            Ok(())
        })())
    }
    fn scoped_local(
        &self,
        clause: &Clause<'a>,
        target: DeclarationId,
        view: &CallableFacts,
        kind: TypeKind,
    ) -> bool {
        let declaration = &self.bindings.declarations[target.0];
        let ty = &view.declarations[target.0].ty;
        let source = &self.context.files[clause.file];
        if declaration.file != clause.file
            || declaration.item != clause.item
            || declaration.role != DeclarationRole::Local
            || !ty.known()
            || optional(ty)
            || ty.kind != kind
            || ty.instantiation != Instantiation::Decision
            || source.kind != SourceKind::User
        {
            return false;
        }
        let Some(mut node) = source
            .parsed
            .tree()
            .child_nodes()
            .nth(clause.item)
            .filter(|n| n.kind() == NodeKind::Constraint)
        else {
            return false;
        };
        let contains = |outer: &SyntaxNode, inner: &SyntaxNode| {
            outer.range().start <= inner.range().start && inner.range().end <= outer.range().end
        };
        let mut ancestors = Vec::new();
        while node.range() != clause.node.range() {
            ancestors.push(node);
            let Some(child) = node.child_nodes().find(|n| contains(n, clause.node)) else {
                return false;
            };
            node = child;
        }
        let Some(owning) = ancestors.iter().copied().chain([clause.node]).find(|n| {
            n.kind() == NodeKind::LetExpression
                && n.child_nodes().any(|block| {
                    block.kind() == NodeKind::LetBlock
                        && block
                            .child_nodes()
                            .any(|d| d.range() == declaration.syntax_range)
                })
        }) else {
            return false;
        };
        ancestors
            .iter()
            .enumerate()
            .all(|(position, node)| match node.kind() {
                NodeKind::Constraint
                | NodeKind::ParenthesizedExpression
                | NodeKind::LetExpression
                | NodeKind::LetBlock => true,
                NodeKind::BinaryExpression => self.core(clause.file, node, view, "/\\"),
                NodeKind::GeneratorCallExpression | NodeKind::CallExpression
                    if self.core(clause.file, node, view, "forall") =>
                {
                    let quantified = if node.kind() == NodeKind::GeneratorCallExpression {
                        Some(*node)
                    } else {
                        node.child_nodes()
                            .next()
                            .filter(|n| n.kind() == NodeKind::ArrayComprehension)
                    };
                    quantified
                        .and_then(|q| {
                            q.child_nodes()
                                .find(|n| n.kind() != NodeKind::GeneratorList)
                        })
                        .is_some_and(|body| contains(body, owning))
                }
                NodeKind::ArrayComprehension => {
                    position > 0 && self.core(clause.file, ancestors[position - 1], view, "forall")
                }
                _ => false,
            })
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
    fn literal_call_outputs(
        &self,
        clause: &Clause<'a>,
        caller: &CallableFacts,
        instance: &Instance<'a>,
    ) -> Option<Vec<Output>> {
        if instance.recursive {
            return None;
        }
        let declaration = &self.bindings.declarations[instance.id.0];
        let file = declaration.file;
        let written = find_node(
            self.context.files[file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        if !self.callable_annotations_safe(file, written) {
            return None;
        }
        let mut body = unwrap(
            written
                .child_nodes()
                .filter(|n| is_expression(n.kind()))
                .last()?,
        );
        let view = &instance.view;
        let type_of = |file, node: &SyntaxNode, view: &CallableFacts| {
            self.view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range == self.context.files[file].location(node.range()).range
                })
                .map(|e| e.ty.clone())
        };
        let boolean = |node: &SyntaxNode| {
            type_of(file, node, view)
                .is_some_and(|ty| ty.known() && !optional(&ty) && ty.kind == TypeKind::Bool)
        };
        if !boolean(body) {
            return None;
        }
        let mut generators = Vec::new();
        let mut old_array = None;
        let mut binder = None;
        if body.kind() == NodeKind::GeneratorCallExpression && self.core(file, body, view, "forall")
        {
            let list = body
                .child_nodes()
                .find(|n| n.kind() == NodeKind::GeneratorList)?;
            generators.extend(list.child_nodes());
            if generators.len() != 1 {
                return None;
            }
            self.iterations(file, &generators, view).ok()?;
            self.relation_iterations(file, &generators, 0, view, false)
                .ok()?;
            let mut bindings = self.bindings.declarations.iter().filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == generators[0].range()
            });
            binder = Some(bindings.next()?.id);
            if bindings.next().is_some() {
                return None;
            }
            let source = unwrap(generators[0].child_nodes().next()?);
            if source.kind() != NodeKind::CallExpression
                || !self.core(file, source, view, "index_set")
                || source.child_nodes().count() != 1
            {
                return None;
            }
            let array = unwrap(source.child_nodes().next()?);
            if array.kind() != NodeKind::Expression {
                return None;
            }
            old_array = Some(self.reference(file, array)?);
            body = unwrap(
                body.child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)?,
            );
        }
        if body.kind() != NodeKind::BinaryExpression
            || !self.core(file, body, view, "=")
            || !boolean(body)
        {
            return None;
        }
        let sides: Vec<_> = body.child_nodes().map(unwrap).collect();
        if sides.len() != 2 {
            return None;
        }
        let formals: Option<Vec<_>> = (0..instance.parameters.len())
            .map(|position| formal_parameter(self.context, self.bindings, instance.id, position))
            .collect();
        let formals = formals?;
        if formals.iter().any(|id| {
            let ty = &view.declarations[id.0].ty;
            !ty.known() || optional(ty)
        }) {
            return None;
        }
        let reference = |node: &SyntaxNode| {
            (node.kind() == NodeKind::Expression)
                .then(|| self.reference(file, node))
                .flatten()
        };
        let array_access = |node: &'a SyntaxNode| {
            if node.kind() != NodeKind::ArrayAccessExpression {
                return None;
            }
            let mut children = node.child_nodes().map(unwrap);
            let array = reference(children.next()?)?;
            let index = reference(children.next()?)?;
            children.next().is_none().then_some((array, index))
        };
        let (output, input, selector, rhs) = if let Some(old) = old_array {
            // Only the written index_set traversal supplies the literal's 1..N axis.
            // The existing two-cell value proof checks both values and its selector.
            let mut found = None;
            for (lhs, rhs) in [(sides[0], sides[1]), (sides[1], sides[0])] {
                let Some((output, index)) = array_access(lhs) else {
                    continue;
                };
                if Some(index) != binder || rhs.kind() != NodeKind::ArrayAccessExpression {
                    continue;
                }
                let parts: Vec<_> = rhs.child_nodes().map(unwrap).collect();
                if parts.len() != 2 || parts[0].kind() != NodeKind::ArrayLiteral {
                    continue;
                }
                let values: Vec<_> = parts[0].child_nodes().map(unwrap).collect();
                if values.len() == 2 && array_access(values[0]) == Some((old, index)) {
                    found = Some((output, old, None, rhs));
                    break;
                }
            }
            found?
        } else {
            let mut found = None;
            for (lhs, rhs) in [(sides[0], sides[1]), (sides[1], sides[0])] {
                if let (Some(output), Some((array, index))) = (reference(lhs), array_access(rhs)) {
                    found = Some((output, array, Some(index), rhs));
                    break;
                }
            }
            found?
        };
        if ![output, input].iter().all(|id| formals.contains(id))
            || selector.is_some_and(|id| !formals.contains(&id))
            || output == input
        {
            return None;
        }
        let TypeKind::Array { indices, element } = &view.declarations[input.0].ty.kind else {
            return None;
        };
        let kind = element.kind.clone();
        if indices.len() != 1
            || indices[0].kind != TypeKind::Int
            || !matches!(kind, TypeKind::Bool | TypeKind::Int)
        {
            return None;
        }
        let actual = |formal| self.actual(clause, instance.id, formal, caller);
        let lexical = |actual_file, node: &SyntaxNode| {
            if actual_file == clause.file
                && clause.node.range().start <= node.range().start
                && node.range().end <= clause.node.range().end
            {
                clause.generators.as_slice()
            } else {
                &[]
            }
        };
        let literal = |actual_file, node: &'a SyntaxNode| {
            let node = unwrap(node);
            let ty = type_of(actual_file, node, caller)?;
            if node.kind() != NodeKind::ArrayLiteral
                || !ty.known()
                || optional(&ty)
                || !matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == kind)
            {
                return None;
            }
            let cells: Vec<_> = node.child_nodes().map(unwrap).collect();
            cells
                .iter()
                .all(|cell| {
                    cell.kind() != NodeKind::IndexedArrayEntry
                        && type_of(actual_file, cell, caller)
                            .is_some_and(|ty| ty.known() && !optional(&ty) && ty.kind == kind)
                })
                .then_some(cells)
        };
        let (input_file, input_actual) = actual(input)?;
        let input_cells = literal(input_file, input_actual)?;
        let (output_file, output_actual) = actual(output)?;
        let targets = if let Some(selector) = selector {
            if view.declarations[output.0].ty.kind != kind {
                return None;
            }
            let (index_file, index_actual) = actual(selector)?;
            let index_actual = unwrap(index_actual);
            let ty = type_of(index_file, index_actual, caller)?;
            if !ty.known() || optional(&ty) || ty.kind != TypeKind::Int {
                return None;
            }
            let interval = if index_actual.kind() == NodeKind::Expression {
                if let Some(id) = self.reference(index_file, index_actual) {
                    crate::domains::index_domain_interval(&self.domains.declarations[id.0].domain)
                } else {
                    crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        index_file,
                        index_actual,
                    )
                    .ok()?
                    .map(|value| (value, value))
                }
            } else {
                None
            }?;
            if interval.0 < 1
                || interval.0 > interval.1
                || interval.1 > i64::try_from(input_cells.len()).ok()?
            {
                return None;
            }
            vec![unwrap(output_actual)]
        } else {
            let cells = literal(output_file, output_actual)?;
            if cells.len() != input_cells.len() {
                return None;
            }
            self.dependencies(file, rhs, view, &generators).ok()?;
            cells
        };
        // Validate every mapped input, including unused actuals/defaults. Output
        // lvalues enter dependencies only if the written RHS reads them too.
        let mut dependencies = Vec::new();
        for formal in &formals {
            let written = &self.bindings.declarations[formal.0];
            let parameter = find_node(
                self.context.files[file].parsed.tree(),
                &written.syntax_range,
                written.role,
            )?;
            if !crate::definitions::annotations_safe(self.context, file, parameter) {
                return None;
            }
            if matches!(view.declarations[formal.0].ty.kind, TypeKind::Array { .. }) {
                // These calls bind an unconstrained or polymorphic formal axis
                // directly to the literal. Written restricted axes need their
                // own call-contract proof and keep the ordinary fallback.
                let array_type = parameter.child_nodes().next()?;
                let axis = array_type.child_nodes().next()?;
                if array_type.kind() != NodeKind::ArrayType
                    || !(axis.kind() == NodeKind::TypeInstVariable
                        || axis.kind() == NodeKind::ScalarType
                            && crate::domains::tokens(&self.context.files[file].parsed, axis)
                                .iter()
                                .any(|token| token.kind == TokenKind::Int))
                {
                    return None;
                }
            }
            let type_ids = self.type_dependencies(file, parameter, view, &[]).ok()?;
            for id in type_ids {
                if formals.contains(&id) {
                    let (actual_file, value) = actual(id)?;
                    extend(
                        &mut dependencies,
                        self.dependencies(actual_file, value, caller, lexical(actual_file, value))
                            .ok()?,
                    );
                } else {
                    extend(&mut dependencies, vec![id]);
                }
            }
            if *formal != output || selector == Some(*formal) {
                let (actual_file, value) = actual(*formal)?;
                extend(
                    &mut dependencies,
                    self.dependencies(actual_file, value, caller, lexical(actual_file, value))
                        .ok()?,
                );
                let value = unwrap(value);
                if let Some(id) = (value.kind() == NodeKind::Expression)
                    .then(|| self.reference(actual_file, value))
                    .flatten()
                {
                    let d = &self.bindings.declarations[id.0];
                    let n = find_node(
                        self.context.files[d.file].parsed.tree(),
                        &d.syntax_range,
                        d.role,
                    )?;
                    if !crate::definitions::annotations_safe(self.context, d.file, n) {
                        return None;
                    }
                    extend(
                        &mut dependencies,
                        self.type_dependencies(d.file, n, caller, lexical(d.file, n))
                            .ok()?,
                    );
                }
            }
        }
        if selector.is_none() {
            for id in self.dependencies(file, rhs, view, &generators).ok()? {
                if !formals.contains(&id) {
                    extend(&mut dependencies, vec![id]);
                } else if id == output {
                    // Reading a destination is an actual dependency, never an anchor.
                    extend(
                        &mut dependencies,
                        self.dependencies(
                            output_file,
                            output_actual,
                            caller,
                            lexical(output_file, output_actual),
                        )
                        .ok()?,
                    );
                }
            }
        }
        let mut outputs = Vec::new();
        for node in targets {
            if node.kind() != NodeKind::Expression
                || type_of(output_file, node, caller)
                    .is_none_or(|ty| !ty.known() || optional(&ty) || ty.kind != kind)
            {
                return None;
            }
            let (target, coverage) =
                self.target(output_file, node, caller, lexical(output_file, node))?;
            if coverage != DefinitionCoverage::Scalar {
                return None;
            }
            let d = &self.bindings.declarations[target.0];
            let n = find_node(
                self.context.files[d.file].parsed.tree(),
                &d.syntax_range,
                d.role,
            )?;
            if !crate::definitions::annotations_safe(self.context, d.file, n) {
                return None;
            }
            let mut ids = dependencies.clone();
            extend(
                &mut ids,
                self.type_dependencies(d.file, n, caller, lexical(d.file, n))
                    .ok()?,
            );
            outputs.push(Output {
                target,
                dependencies: ids,
                coverage,
                location: self.context.files[clause.file].location(clause.node.range()),
                enforced_boolean: false,
                scoped_local: false,
            });
        }
        Some(outputs)
    }
    // Bind written actuals once per selected invocation. Captures retain their
    // defining file; only an exact formal reference follows these bindings.
    fn invocation_actuals<'b>(
        &'b self,
        clause: &Clause<'a>,
        view: &'b CallableFacts,
        instance: &'b Instance<'a>,
        invocation: &mut Invocation<'a, 'b>,
    ) -> Result<(), String> {
        let owner = &self.bindings.declarations[instance.id.0];
        let written = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("invoked declaration unavailable")?;
        if !self.callable_annotations_safe(owner.file, written) {
            return Err("invoked declaration annotation is unsupported".into());
        }
        self.type_dependencies(owner.file, written, &instance.view, &[])?;
        let first = invocation.actuals.len();
        for (position, parameter) in instance.parameters.iter().enumerate() {
            let formal = formal_parameter(self.context, self.bindings, instance.id, position)
                .ok_or("invoked formal identity unavailable")?;
            let formal_node = find_node(
                self.context.files[owner.file].parsed.tree(),
                &self.bindings.declarations[formal.0].syntax_range,
                DeclarationRole::Parameter,
            )
            .ok_or("invoked formal declaration unavailable")?;
            self.type_dependencies(owner.file, formal_node, &instance.view, &[])?;
            let mut nodes = vec![formal_node];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, owner.file, node) {
                    return Err("invoked formal annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            let collection =
                clause.node.kind() == NodeKind::GeneratorCallExpression && position == 0;
            let (file, node) = if collection {
                (clause.file, clause.node)
            } else {
                call_argument(
                    self.context,
                    self.bindings,
                    self.view(clause.file, clause.node, view),
                    clause.file,
                    clause.node,
                    instance.id,
                    position,
                )
                .ok_or("invoked actual or default unavailable")?
            };
            let default = file == owner.file
                && formal_node.range().start <= node.range().start
                && node.range().end <= formal_node.range().end;
            let facts = if default {
                &instance.view
            } else {
                self.view(file, node, view)
            };
            let ty = if collection {
                self.operation_fact(facts, file, node)
                    .and_then(|call| call.generator_argument.as_ref())
            } else {
                self.expression_type(facts, file, node)
                    .map(|e| &e.ty)
                    .or_else(|| {
                        self.reference(file, unwrap(node))
                            .map(|id| &facts.declarations[id.0].ty)
                    })
            }
            .ok_or("invoked actual type unavailable")?;
            if !parameter.known()
                || !ty.known()
                || optional(ty)
                || !crate::types::coerces(ty, parameter)
                || instance.view.declarations[formal.0].ty != *parameter
            {
                return Err("invoked actual type or optionality is unsupported".into());
            }
            invocation.actuals.push(InvocationActual {
                formal,
                file,
                node,
                view: facts,
                collection,
                generators: if file == clause.file
                    && clause.node.range().start <= node.range().start
                    && node.range().end <= clause.node.range().end
                {
                    clause.generators.clone()
                } else {
                    Vec::new()
                },
            });
        }
        let mut unsupported = None;
        for actual in &invocation.actuals[first..] {
            let checked = self
                .invocation_source(actual.file, actual.node, invocation)
                .and_then(|source| {
                    let source = source.unwrap_or(actual);
                    match self.invocation_actual_safety(source) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    }
                });
            if let Err(reason) = checked {
                unsupported = Some(reason);
            }
        }
        // Header sources and filters also evaluate, even when no outputs result.
        for (position, generator) in clause.generators.iter().enumerate() {
            for value in generator.child_nodes() {
                let value = if value.kind() == NodeKind::WhereFilter {
                    value
                        .child_nodes()
                        .next()
                        .ok_or("invoked generator filter unavailable")?
                } else {
                    value
                };
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    clause.file,
                    value,
                    view,
                    &clause.generators[..=position],
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
            }
        }
        match unsupported {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }
    // A generated array is inspected through its written construction, never
    // through the scalar result type of the retaining call node.
    fn invocation_actual_safety(&self, actual: &InvocationActual<'a, '_>) -> DefinitionSafety {
        if !actual.collection {
            return self.initialized_source_safety(
                actual.file,
                actual.node,
                actual.view,
                &actual.generators,
                &mut Vec::new(),
            );
        }
        let file = actual.file;
        let view = actual.view;
        let mut nodes = vec![actual.node];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return DefinitionSafety::Unsupported(
                    "invoked collection annotation is unsupported".into(),
                );
            }
            nodes.extend(node.child_nodes());
        }
        let Some(list) = actual
            .node
            .child_nodes()
            .find(|node| node.kind() == NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported("invoked collection headers unavailable".into());
        };
        let mut generators = actual.generators.clone();
        let mut unsupported = None;
        for generator in list.child_nodes() {
            let Some(source) = generator.child_nodes().next() else {
                return DefinitionSafety::Unsupported(
                    "invoked collection source unavailable".into(),
                );
            };
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, source, view, &generators, &mut Vec::new())
            {
                unsupported = Some(reason);
            }
            if let Some(reason) = self.closed_integer_source_error(file, source, true, true) {
                unsupported = Some(reason);
            }
            generators.push(generator);
            for filter in generator
                .child_nodes()
                .filter(|node| node.kind() == NodeKind::WhereFilter)
            {
                let Some(condition) = filter.child_nodes().next() else {
                    return DefinitionSafety::Unsupported(
                        "invoked collection filter unavailable".into(),
                    );
                };
                if self
                    .expression_type(view, file, condition)
                    .is_none_or(|expression| {
                        !expression.ty.known()
                            || optional(&expression.ty)
                            || expression.ty.kind != TypeKind::Bool
                    })
                {
                    unsupported = Some("invoked collection filter type is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                    file,
                    condition,
                    view,
                    &generators,
                    &mut Vec::new(),
                ) {
                    unsupported = Some(reason);
                }
                if let Some(reason) = self.closed_integer_source_error(file, condition, true, true)
                {
                    unsupported = Some(reason);
                }
            }
        }
        let Some(body) = actual
            .node
            .child_nodes()
            .find(|node| node.kind() != NodeKind::GeneratorList)
        else {
            return DefinitionSafety::Unsupported("invoked collection body unavailable".into());
        };
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_source_safety(file, body, view, &generators, &mut Vec::new())
        {
            unsupported = Some(reason);
        }
        if let Some(reason) = self.closed_integer_source_error(file, body, true, true) {
            unsupported = Some(reason);
        }
        match unsupported {
            Some(reason) => DefinitionSafety::Unsupported(reason),
            None => DefinitionSafety::Unknown("invoked collection extent is unproved".into()),
        }
    }
    fn invocation_source<'b, 'c>(
        &self,
        mut file: FileId,
        mut node: &'a SyntaxNode,
        invocation: &'c Invocation<'a, 'b>,
    ) -> Result<Option<&'c InvocationActual<'a, 'b>>, String> {
        let mut active = Vec::new();
        let mut source = None;
        while let Some(id) = (unwrap(node).kind() == NodeKind::Expression)
            .then(|| self.reference(file, unwrap(node)))
            .flatten()
        {
            let Some(actual) = invocation
                .actuals
                .iter()
                .rev()
                .find(|actual| actual.formal == id)
            else {
                break;
            };
            if active.contains(&id) {
                return Err("cyclic invoked actual correspondence".into());
            }
            active.push(id);
            source = Some(actual);
            file = actual.file;
            node = actual.node;
        }
        Ok(source)
    }
    // Only a literal Boolean or the existing exact length(array)==0 form gives
    // branch direction. This never establishes a member, cardinality or value.
    fn invocation_guard<'b>(
        &'b self,
        file: FileId,
        guard: &'a SyntaxNode,
        view: &'b CallableFacts,
        invocation: &Invocation<'a, 'b>,
    ) -> Result<Option<bool>, String> {
        let guard = unwrap(guard);
        let tokens = crate::domains::tokens(&self.context.files[file].parsed, guard);
        if guard.kind() == NodeKind::Expression && tokens.len() == 1 {
            return Ok(match tokens[0].kind {
                TokenKind::True => Some(true),
                TokenKind::False => Some(false),
                _ => None,
            });
        }
        for length in guard.child_nodes().map(unwrap) {
            if !self.core(file, length, view, "length") {
                continue;
            }
            let arguments: Vec<_> = length.child_nodes().collect();
            let [array] = arguments.as_slice() else {
                continue;
            };
            let Some(id) = self.reference(file, unwrap(array)) else {
                continue;
            };
            if self.length_equals(file, guard, id, 0, view) {
                return self.invocation_array_empty(
                    file,
                    array,
                    self.view(file, array, view),
                    invocation,
                    &mut Vec::new(),
                );
            }
        }
        Ok(None)
    }
    fn invocation_array_empty<'b>(
        &'b self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &'b CallableFacts,
        invocation: &Invocation<'a, 'b>,
        active: &mut Vec<DeclarationId>,
    ) -> Result<Option<bool>, String> {
        let actual = self.invocation_source(file, node, invocation)?;
        if actual.is_some_and(|actual| actual.collection) {
            return Ok(None);
        }
        let (file, node, view) = actual.map_or((file, node, view), |actual| {
            (actual.file, actual.node, actual.view)
        });
        let actual_generators = actual.map_or(&[][..], |actual| actual.generators.as_slice());
        let node = unwrap(node);
        match node.kind() {
            NodeKind::ArrayLiteral => Ok(
                if node
                    .child_nodes()
                    .any(|n| n.kind() == NodeKind::IndexedArrayEntry)
                {
                    None
                } else {
                    Some(node.child_nodes().count() == 0)
                },
            ),
            NodeKind::ArrayComprehension => {
                let generators: Vec<_> = node
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                    .map(|n| n.child_nodes().collect())
                    .unwrap_or_default();
                let [generator] = generators.as_slice() else {
                    return Ok(None);
                };
                if crate::domains::generator_slots(&self.context.files[file].parsed, generator) != 1
                    || generator
                        .child_nodes()
                        .any(|n| n.kind() == NodeKind::WhereFilter)
                {
                    return Ok(None);
                }
                let source = generator
                    .child_nodes()
                    .next()
                    .ok_or("invoked array extent source unavailable")?;
                // A fully inspected parameter set selection has unproved extent.
                // Retain its written caller's lexical generators for source checks.
                if unwrap(source).kind() == NodeKind::ArrayAccessExpression
                    && self.expression_type(view, file, source).is_some_and(|e| {
                        e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.instantiation == Instantiation::Parameter
                            && matches!(&e.ty.kind, TypeKind::Set(element)
                                if element.instantiation == Instantiation::Parameter
                                    && element.kind == TypeKind::Int)
                    })
                {
                    return match self.initialized_source_safety(
                        file,
                        source,
                        view,
                        actual_generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(None),
                    };
                }
                let domain = expression_domain(self.context, self.bindings, file, source);
                match crate::domains::bare_index_domain(&domain) {
                    Domain::Range { lower, upper } => {
                        let lower = crate::domains::invariant_integer(lower)?;
                        let upper = crate::domains::invariant_integer(upper)?;
                        Ok(lower.zip(upper).map(|(lower, upper)| lower > upper))
                    }
                    Domain::Unsupported(reason) => Err(reason.clone()),
                    _ => Ok(None),
                }
            }
            NodeKind::Expression => {
                let Some(id) = self.reference(file, node) else {
                    return Ok(None);
                };
                let owner = &self.bindings.declarations[id.0];
                if owner.role != DeclarationRole::Value || !owner.top_level {
                    return Ok(None);
                }
                if active.contains(&id) {
                    return Err("cyclic invoked array source".into());
                }
                let written = find_node(
                    self.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("invoked array declaration unavailable")?;
                let values: Vec<_> = written
                    .child_nodes()
                    .filter(|n| is_expression(n.kind()))
                    .collect();
                if values.is_empty() {
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, node, view, &[], &mut Vec::new())
                    {
                        return Err(reason);
                    }
                    let domain =
                        crate::domains::bare_index_domain(&self.domains.declarations[id.0].domain);
                    let Domain::Array { indices, .. } = domain else {
                        return match domain {
                            Domain::Unsupported(reason) => Err(reason.clone()),
                            _ => Ok(None),
                        };
                    };
                    let [axis] = indices.as_slice() else {
                        return Ok(None);
                    };
                    return match crate::domains::bare_index_domain(axis) {
                        Domain::Range { lower, upper } => {
                            let lower = crate::domains::invariant_integer(lower)?;
                            let upper = crate::domains::invariant_integer(upper)?;
                            Ok(lower.zip(upper).map(|(lower, upper)| lower > upper))
                        }
                        Domain::Unsupported(reason) => Err(reason.clone()),
                        _ => Ok(None),
                    };
                }
                let [value] = values.as_slice() else {
                    return Ok(None);
                };
                active.push(id);
                let result = self.invocation_array_empty(
                    owner.file,
                    value,
                    self.view(owner.file, value, view),
                    invocation,
                    active,
                );
                active.pop();
                result
            }
            _ => Ok(None),
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
                    (self.calls, self.view(file, node, view)),
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
    fn array_conversion_argument(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<&'a SyntaxNode> {
        let node = unwrap(node);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let facts = self.view(file, node, view);
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &self.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let name = &self.bindings.declarations[declaration.0].name;
        if !matches!(name.as_str(), "enum2int" | "index2int")
            || !crate::definitions::core_callable(self.context, self.bindings, *declaration, name)
        {
            return None;
        }
        let mut children = node.child_nodes();
        let argument = children.next()?;
        if children.next().is_some() || argument.kind() == NodeKind::NamedArgument {
            return None;
        }
        let input = &self
            .expression_type(self.view(file, argument, view), file, argument)?
            .ty;
        let result = &self.expression_type(facts, file, node)?.ty;
        if !input.known()
            || optional(input)
            || parameters.as_slice() != std::slice::from_ref(input)
            || return_type != result
        {
            return None;
        }
        let mut expected = input.clone();
        let TypeKind::Array { indices, element } = &mut expected.kind else {
            return None;
        };
        if indices.len() != 1
            || indices[0].instantiation != Instantiation::Parameter
            || !matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
        {
            return None;
        }
        if name == "enum2int" {
            if !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)) {
                return None;
            }
            element.kind = TypeKind::Int;
        } else {
            indices[0] = TypeInst::par(TypeKind::Int);
        }
        // These standard views retain every value; only the enum representation
        // or index type changes. Keep all other qualifiers and identities exact.
        (expected == *result).then_some(argument)
    }
    fn converted_array_target(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        if let Some(argument) = self.array_conversion_argument(file, node, view) {
            return self.converted_array_target(file, argument, view);
        }
        if node.kind() != NodeKind::Expression {
            return None;
        }
        let id = self.reference(file, node)?;
        let ty = &self.view(file, node, view).declarations[id.0].ty;
        (ty.known()
            && !optional(ty)
            && ty.instantiation == Instantiation::Decision
            && matches!(&ty.kind, TypeKind::Array { indices, .. } if indices.len() == 1))
        .then_some(id)
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
    fn nonempty_source(&self, file: FileId, generator: &SyntaxNode) -> bool {
        let Some(source) = generator.child_nodes().next() else {
            return false;
        };
        match expression_domain(self.context, self.bindings, file, source) {
            crate::Domain::Range { lower, upper } => {
                matches!((crate::domains::invariant_integer(&lower), crate::domains::invariant_integer(&upper)), (Ok(Some(lower)), Ok(Some(upper))) if lower <= upper)
            }
            crate::Domain::LiteralSet(values) => {
                !values.is_empty()
                    && values
                        .iter()
                        .all(|v| matches!(crate::domains::invariant_integer(v), Ok(Some(_))))
            }
            _ => false,
        }
    }
    fn relation_iterations(
        &self,
        file: FileId,
        generators: &[&'a SyntaxNode],
        first_new: usize,
        view: &CallableFacts,
        decision_arrays: bool,
    ) -> Result<(), String> {
        // Nested collections retain outer binders without re-evaluating their headers.
        for (position, generator) in generators.iter().enumerate().skip(first_new) {
            if !generator.children().iter().any(|c| matches!(c, SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)) { return Err("assigned iteration source is unsupported".into()); }
            let source = generator
                .child_nodes()
                .next()
                .ok_or("iteration source unavailable")?;
            let range = self.context.files[file].location(source.range()).range;
            let ty = self
                .view(file, source, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty);
            if ty.is_none_or(|t| {
                !t.known()
                    || optional(t)
                    || !(t.instantiation == Instantiation::Parameter
                        && matches!(t.kind, TypeKind::Set(_) | TypeKind::Array { .. })
                        || decision_arrays
                            && t.instantiation == Instantiation::Decision
                            && matches!(t.kind, TypeKind::Array { .. }))
            }) {
                return Err("iteration is not a supported finite parameter set or array".into());
            }
            self.dependencies(file, source, view, &generators[..position])?;
            for filter in generator
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::WhereFilter)
            {
                let condition = filter
                    .child_nodes()
                    .next()
                    .ok_or("iteration filter unavailable")?;
                let range = self.context.files[file].location(condition.range()).range;
                let ty = self
                    .view(file, condition, view)
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .map(|e| &e.ty);
                if ty.is_none_or(|t| {
                    t.kind != TypeKind::Bool
                        || t.optional
                        || t.instantiation != Instantiation::Parameter
                }) {
                    return Err("iteration filter is not a total parameter Boolean".into());
                }
                self.dependencies(file, condition, view, &generators[..=position])?;
            }
        }
        Ok(())
    }
    fn member_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        if index.kind() != NodeKind::Expression {
            return false;
        }
        let Some(binder) = self.reference(file, index) else {
            return false;
        };
        self.bindings.declarations[binder.0].role == DeclarationRole::Generator
            && generators.iter().any(|g| {
                self.bindings.declarations[binder.0].syntax_range == g.range()
                    && g.child_nodes().next().is_some_and(|source| {
                        self.core(file, source, view, "index_set")
                            && source.child_nodes().next().is_some_and(|subject| {
                                unwrap(subject).kind() == NodeKind::Expression
                                    && self.reference(file, unwrap(subject)) == Some(array)
                            })
                    })
            })
    }
    fn integer_index_set(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
        let set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if integer(element) && element.instantiation == Instantiation::Parameter)
        };
        let CallOutcome::Resolved {
            parameters,
            return_type,
            ..
        } = &self.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let [parameter] = parameters.as_slice() else {
            return None;
        };
        let subject = unwrap(node.child_nodes().next()?);
        let id = self.reference(file, subject)?;
        let actual = &facts.declarations[id.0].ty;
        (node.kind() == NodeKind::CallExpression
            && node.child_nodes().count() == 1
            && self.core(file, node, view, "index_set")
            && subject.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            && actual.known() && !optional(actual)
            && matches!(&actual.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && integer(element))
            && parameter.known() && !optional(parameter)
            && matches!(&parameter.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && integer(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && integer(element))
            && crate::types::coerces(actual, parameter)
            && set(return_type)
            && self.expression_type(facts, file, node).is_some_and(|e| e.ty == *return_type))
        .then_some(id)
    }
    // This first contract belongs to the callable's entire direct body. A
    // conditional, local or nested call cannot carry it into an output summary.
    fn asserted_index_pair(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<[DeclarationId; 2]> {
        let owner = self.bindings.declarations.iter().find(|d| {
            d.file == file
                && matches!(
                    d.role,
                    DeclarationRole::Predicate | DeclarationRole::Function | DeclarationRole::Test
                )
                && d.syntax_range.start <= assertion.range().start
                && assertion.range().end <= d.syntax_range.end
        })?;
        let written = find_node(
            self.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        let body = written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .last()?;
        if unwrap(body).range() != assertion.range() {
            return None;
        }
        let args = self.assertion_arguments(file, assertion, view)?;
        if args.len() != 3 || args.iter().any(|(source, _)| *source != file) {
            return None;
        }
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        let CallOutcome::Resolved {
            parameters,
            return_type,
            ..
        } = &self
            .operation_fact(self.view(file, assertion, view), file, assertion)?
            .outcome
        else {
            return None;
        };
        if parameters.len() != 3
            || !present(&parameters[0], TypeKind::Bool, Instantiation::Parameter)
            || !present(&parameters[1], TypeKind::String, Instantiation::Parameter)
            || !present(&parameters[2], TypeKind::Bool, Instantiation::Decision)
            || return_type != &parameters[2]
        {
            return None;
        }
        let condition = unwrap(args[0].1);
        let operands: Vec<_> = condition.child_nodes().collect();
        let set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if present(element, TypeKind::Int, Instantiation::Parameter))
        };
        if condition.kind() != NodeKind::BinaryExpression
            || operands.len() != 2
            || !self.core(file, condition, view, "=")
            || !self
                .operation_fact(self.view(file, condition, view), file, condition)
                .is_some_and(|call| {
                    matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(set)
                        && present(return_type, TypeKind::Bool, Instantiation::Parameter))
                })
            || self
                .expression_type(self.view(file, condition, view), file, condition)
                .is_none_or(|e| !present(&e.ty, TypeKind::Bool, Instantiation::Parameter))
        {
            return None;
        }
        let arrays = [
            self.integer_index_set(file, operands[0], view)?,
            self.integer_index_set(file, operands[1], view)?,
        ];
        if arrays.iter().any(|id| {
            let d = &self.bindings.declarations[id.0];
            d.file != file || d.item != owner.item || d.role != DeclarationRole::Parameter
        }) {
            return None;
        }
        let mut nodes = vec![assertion];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return None;
            }
            nodes.extend(node.child_nodes());
        }
        for (position, (_, value)) in args.iter().take(2).enumerate() {
            let kind = if position == 0 {
                TypeKind::Bool
            } else {
                TypeKind::String
            };
            if self
                .expression_type(self.view(file, value, view), file, value)
                .is_none_or(|e| !present(&e.ty, kind, Instantiation::Parameter))
                || self.dependencies(file, value, view, &[]).is_err()
            {
                return None;
            }
        }
        Some(arrays)
    }
    fn direct_forall_equality(
        &self,
        file: FileId,
        quantified: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(DeclarationId, &'a SyntaxNode, Vec<&'a SyntaxNode>)> {
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        let quantified = unwrap(quantified);
        if quantified.kind() != NodeKind::GeneratorCallExpression
            || !self.core(file, quantified, view, "forall")
            || !self.operation_fact(self.view(file, quantified, view), file, quantified)
                .is_some_and(|call| matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                        && parameters[0].instantiation == Instantiation::Decision
                        && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && present(&indices[0], TypeKind::Int, Instantiation::Parameter)
                                && present(element, TypeKind::Bool, Instantiation::Decision))
                        && present(return_type, TypeKind::Bool, Instantiation::Decision)))
        { return None; }
        let children: Vec<_> = quantified.child_nodes().collect();
        if children.len() != 2 {
            return None;
        }
        let list = *children
            .iter()
            .find(|n| n.kind() == NodeKind::GeneratorList)?;
        let generators: Vec<_> = list.child_nodes().collect();
        if generators.len() != 1
            || generators[0].child_nodes().count() != 1
            || self.iterations(file, &generators, view).is_err()
            || self
                .relation_iterations(file, &generators, 0, view, false)
                .is_err()
            || self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generators[0].range()
                })
                .count()
                != 1
        {
            return None;
        }
        let output = self.integer_index_set(file, generators[0].child_nodes().next()?, view)?;
        let equality = unwrap(
            *children
                .iter()
                .find(|n| n.kind() != NodeKind::GeneratorList)?,
        );
        let sides: Vec<_> = equality.child_nodes().collect();
        if equality.kind() != NodeKind::BinaryExpression || sides.len() != 2
            || !self.core(file, equality, view, "=")
            || !self.operation_fact(self.view(file, equality, view), file, equality)
                .is_some_and(|call| matches!(&call.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2
                        && parameters.iter().all(|ty| present(ty, TypeKind::Int, Instantiation::Decision))
                        && present(return_type, TypeKind::Bool, Instantiation::Decision)))
            || !sides.iter().any(|side| self.target(file, side, view, &generators)
                == Some((output, DefinitionCoverage::WholeArray)))
        { return None; }
        Some((output, equality, generators))
    }
    fn direct_asserted_forall(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(DeclarationId, [DeclarationId; 2])> {
        let arrays = self.asserted_index_pair(file, assertion, view)?;
        let args = self.assertion_arguments(file, assertion, view)?;
        let (output, _, _) = self.direct_forall_equality(file, args[2].1, view)?;
        Some((output, arrays))
    }
    fn nested_asserted_forall(
        &self,
        file: FileId,
        assertion: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<(Output, [DeclarationId; 2])> {
        let arrays = self.asserted_index_pair(file, assertion, view)?;
        let args = self.assertion_arguments(file, assertion, view)?;
        let mut call = unwrap(args[2].1);
        let mut weight_guard = None;
        if self.core(file, call, view, "assert") {
            // Carry the outer index relation only through this one fully checked
            // returning guard. Its ordered RHS is already inspected by the
            // existing reflection/length consumers; it proves no load extent.
            let inner = self.assertion_arguments(file, call, view)?;
            let par_bool = TypeInst::par(TypeKind::Bool);
            let var_bool = par_bool.clone().with_inst(Instantiation::Decision);
            let par_string = TypeInst::par(TypeKind::String);
            let par_int = TypeInst::par(TypeKind::Int);
            let var_int = par_int.clone().with_inst(Instantiation::Decision);
            let var_array = TypeInst::par(TypeKind::Array {
                indices: vec![par_int.clone()],
                element: Box::new(var_int),
            })
            .with_inst(Instantiation::Decision);
            let expected = [par_bool.clone(), par_string.clone(), var_bool.clone()];
            if inner.len() != 3
                || inner.iter().any(|(source, _)| *source != file)
                || self
                    .operation_fact(self.view(file, call, view), file, call)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == expected && *return_type == var_bool)
                    })
                || inner.iter().zip(&expected).any(|((_, node), ty)| {
                    self.expression_type(self.view(file, node, view), file, node)
                        .is_none_or(|e| e.ty != *ty)
                })
            {
                return None;
            }
            let condition = unwrap(inner[0].1);
            let sides: Vec<_> = condition.child_nodes().map(unwrap).collect();
            if condition.kind() != NodeKind::BinaryExpression
                || sides.len() != 2
                || !self.core(file, condition, view, "\\/")
                || self
                    .operation_fact(self.view(file, condition, view), file, condition)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [par_bool.clone(), par_bool.clone()]
                            && *return_type == par_bool)
                    })
            {
                return None;
            }
            let bound = sides[1];
            let operands: Vec<_> = bound.child_nodes().map(unwrap).collect();
            if bound.kind() != NodeKind::BinaryExpression || operands.len() != 2
                || !self.core(file, bound, view, ">=")
                || self.operation_fact(self.view(file, bound, view), file, bound)
                    .is_none_or(|fact| !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [TypeInst::par(TypeKind::Int), TypeInst::par(TypeKind::Int)] && *return_type == par_bool))
                || crate::domains::invariant_expression_integer(self.context, self.bindings, file, operands[1]) != Ok(Some(0))
            {
                return None;
            }
            let reflection = operands[0];
            let subject: Vec<_> = reflection.child_nodes().map(unwrap).collect();
            if reflection.kind() != NodeKind::CallExpression
                || subject.len() != 1
                || !self.core(file, reflection, view, "lb_array")
                || subject[0].kind() != NodeKind::Expression
                || self
                    .operation_fact(self.view(file, reflection, view), file, reflection)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [var_array] && *return_type == par_int)
                    })
                || self
                    .expression_type(self.view(file, reflection, view), file, reflection)
                    .is_none_or(|e| e.ty != par_int)
            {
                return None;
            }
            let weight = self.reference(file, subject[0])?;
            if !arrays.contains(&weight)
                || !self.length_equals(file, sides[0], weight, 0, view)
                || inner.iter().take(2).any(|(_, node)| {
                    self.dependencies(file, node, view, &[]).is_err()
                })
                // The exact ordered OR has been checked above. Inspect its
                // operands without asking an eager checker to traverse the OR.
                || [sides[0], sides[1], inner[1].1].into_iter().any(|node| {
                    self.closed_integer_source_error(file, node, true, false)
                        .is_some()
                })
            {
                return None;
            }
            weight_guard = Some(weight);
            call = unwrap(inner[2].1);
        }
        if call.kind() != NodeKind::CallExpression {
            return None;
        }
        let (id, parameters) = self.resolved(file, call, view)?;
        let array = |ty: &TypeInst, instantiation| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == instantiation
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0] == TypeInst::par(TypeKind::Int)
                        && element.kind == TypeKind::Int && element.instantiation == instantiation)
        };
        if parameters.len() != 3
            || !array(&parameters[0], Instantiation::Decision)
            || !array(&parameters[1], Instantiation::Decision)
            || !array(&parameters[2], Instantiation::Parameter)
            || self.expression_type(self.view(file, call, view), file, call)
                .is_none_or(|e| e.ty != TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision))
            || self.operation_fact(self.view(file, call, view), file, call)
                .is_none_or(|fact| !matches!(&fact.outcome,
                    CallOutcome::Resolved { return_type, .. } if *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)))
        {
            return None;
        }
        let owner = &self.bindings.declarations[arrays[0].0];
        let child = &self.bindings.declarations[id.0];
        let instance = self
            .instances
            .iter()
            .find(|instance| instance.id == id && instance.parameters == parameters)?;
        if instance.recursive
            || child.role != DeclarationRole::Predicate
            || child.file == owner.file && child.item == owner.item
        {
            return None;
        }
        let written = find_node(
            self.context.files[child.file].parsed.tree(),
            &child.syntax_range,
            child.role,
        )?;
        if !self.callable_annotations_safe(child.file, written) {
            return None;
        }
        let body = written
            .child_nodes()
            .filter(|node| is_expression(node.kind()))
            .last()?;
        let (equality_clause, quantified, full_body) = match instance.clauses.as_slice() {
            [equality]
                if matches!(equality.kind, ClauseKind::Equality) && weight_guard.is_none() =>
            {
                (equality, body, false)
            }
            [total, lower, upper, equality]
                if weight_guard.is_some()
                    && matches!(total.kind, ClauseKind::Equality)
                    && matches!(lower.kind, ClauseKind::Relation)
                    && matches!(upper.kind, ClauseKind::Relation)
                    && matches!(equality.kind, ClauseKind::Equality) =>
            {
                let conjunction = |node: &'a SyntaxNode| {
                    let node = unwrap(node);
                    let sides: Vec<_> = node.child_nodes().collect();
                    (node.kind() == NodeKind::BinaryExpression && sides.len() == 2
                        && self.core(child.file, node, &instance.view, "/\\")
                        && self.operation_fact(&instance.view, child.file, node)
                            .is_some_and(|fact| matches!(&fact.outcome,
                                CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.as_slice() == [TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision), TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)]
                                    && *return_type == parameters[0])))
                        .then_some(sides)
                };
                let outer = conjunction(body)?;
                let first = conjunction(outer[0])?;
                let bounds = unwrap(first[1]);
                let parts: Vec<_> = bounds.child_nodes().collect();
                let list = parts
                    .iter()
                    .find(|node| node.kind() == NodeKind::GeneratorList)?;
                let generators: Vec<_> = list.child_nodes().collect();
                let bound_body = *parts
                    .iter()
                    .find(|node| node.kind() != NodeKind::GeneratorList)?;
                let bound_sides = conjunction(bound_body)?;
                if unwrap(first[0]).range() != total.node.range() || !total.generators.is_empty()
                    || bounds.kind() != NodeKind::GeneratorCallExpression || parts.len() != 2
                    || !self.core(child.file, bounds, &instance.view, "forall")
                    || self.operation_fact(&instance.view, child.file, bounds)
                        .is_none_or(|fact| !matches!(&fact.outcome,
                            CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                                        && **element == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision))
                                && *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)))
                    || generators.len() != 1 || generators[0].child_nodes().count() != 1
                    || lower.generators.len() != 1 || upper.generators.len() != 1
                    || !std::ptr::eq(lower.generators[0], generators[0])
                    || !std::ptr::eq(upper.generators[0], generators[0])
                    || unwrap(bound_sides[0]).range() != lower.node.range()
                    || unwrap(bound_sides[1]).range() != upper.node.range()
                {
                    return None;
                }
                (equality, outer[1], true)
            }
            _ => return None,
        };
        let (output, equality, generators) =
            self.direct_forall_equality(child.file, quantified, &instance.view)?;
        if equality.range() != equality_clause.node.range()
            || equality_clause.generators.len() != generators.len()
            || equality_clause
                .generators
                .iter()
                .zip(&generators)
                .any(|(clause, source)| !std::ptr::eq(*clause, *source))
        {
            return None;
        }
        let invocation = Clause {
            file,
            item: owner.item,
            node: call,
            generators: Vec::new(),
            kind: ClauseKind::Call,
        };
        let mut mapped = Vec::new();
        for (position, parameter) in parameters.iter().enumerate() {
            let formal = formal_parameter(self.context, self.bindings, id, position)?;
            let (source, written_actual) = self.actual(&invocation, id, formal, view)?;
            if source != file {
                return None;
            }
            let mut value = unwrap(written_actual);
            if value.kind() == NodeKind::CallExpression {
                // Only these already-Int views preserve the asserted axis and
                // value identities. Enum-changing views keep ordinary inspection.
                for name in ["index2int", "enum2int"] {
                    if name == "enum2int"
                        && (position != 1 || value.kind() != NodeKind::CallExpression)
                    {
                        break;
                    }
                    if !self.core(file, value, view, name) {
                        return None;
                    }
                    let argument = self.array_conversion_argument(file, value, view)?;
                    if [value, argument].into_iter().any(|node| {
                        self.expression_type(self.view(file, node, view), file, node)
                            .is_none_or(|e| e.ty != *parameter)
                    }) {
                        return None;
                    }
                    value = unwrap(argument);
                }
                if self.dependencies(file, written_actual, view, &[]).is_err()
                    || self.initialized_source_safety(
                        file,
                        written_actual,
                        view,
                        &[],
                        &mut Vec::new(),
                    ) != DefinitionSafety::Supported
                    || self
                        .closed_integer_source_error(file, written_actual, true, false)
                        .is_some()
                {
                    return None;
                }
            }
            if value.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return None;
            }
            let actual = self.reference(file, value)?;
            let declaration = &self.bindings.declarations[actual.0];
            if declaration.file != file
                || declaration.item != owner.item
                || declaration.role != DeclarationRole::Parameter
                || instance.view.declarations[formal.0].ty != *parameter
                || self
                    .expression_type(self.view(file, value, view), file, value)
                    .is_none_or(|e| e.ty != *parameter)
                || mapped.iter().any(|(_, previous)| *previous == actual)
                || self.dependencies(file, value, view, &[]).is_err()
                || self.initialized_source_safety(file, value, view, &[], &mut Vec::new())
                    != DefinitionSafety::Supported
            {
                return None;
            }
            let parameter_node = find_node(
                self.context.files[file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )?;
            let formal_declaration = &self.bindings.declarations[formal.0];
            let formal_node = find_node(
                self.context.files[child.file].parsed.tree(),
                &formal_declaration.syntax_range,
                formal_declaration.role,
            )?;
            if self
                .type_dependencies(file, parameter_node, view, &[])
                .is_err()
                || self
                    .type_dependencies(child.file, formal_node, &instance.view, &[])
                    .is_err()
                || self
                    .closed_integer_source_error(file, parameter_node, true, false)
                    .is_some()
            {
                return None;
            }
            let mut nodes = vec![parameter_node];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, node) {
                    return None;
                }
                nodes.extend(node.child_nodes());
            }
            mapped.push((formal, actual));
        }
        let child_pair = [
            mapped.iter().find(|(_, actual)| *actual == arrays[0])?.0,
            mapped.iter().find(|(_, actual)| *actual == arrays[1])?.0,
        ];
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, child.file, node) {
                return None;
            }
            nodes.extend(node.child_nodes());
        }
        if full_body {
            // The complete conjunction is checked as four actual clauses. Keep
            // every clause and generator source in eager arithmetic inspection.
            for clause in &instance.clauses {
                if self
                    .closed_integer_source_error(child.file, clause.node, true, false)
                    .is_some()
                    || clause.generators.iter().any(|generator| {
                        generator.child_nodes().any(|node| {
                            self.closed_integer_source_error(child.file, node, true, false)
                                .is_some()
                        })
                    })
                {
                    return None;
                }
            }
        } else if self
            .closed_integer_source_error(child.file, body, true, false)
            .is_some()
        {
            return None;
        }
        if full_body {
            let formals = [
                formal_parameter(self.context, self.bindings, id, 0)?,
                formal_parameter(self.context, self.bindings, id, 1)?,
                formal_parameter(self.context, self.bindings, id, 2)?,
            ];
            if output != formals[0]
                || weight_guard != Some(mapped[2].1)
                || child_pair != [formals[1], formals[2]] && child_pair != [formals[2], formals[1]]
                || !self.full_weighted_siblings(
                    child.file,
                    &instance.clauses[..3],
                    formals,
                    &instance.view,
                )
            {
                return None;
            }
        }
        // This selected invocation keeps its own relation and source checks. A
        // failed prerequisite falls back to ordinary inspection and its full causes.
        let dependencies = self.nested_weighted_dependencies(
            child.file,
            equality,
            output,
            &generators,
            child_pair,
            &instance.view,
        )?;
        let dependencies = dependencies
            .into_iter()
            .map(|formal| {
                mapped
                    .iter()
                    .find(|(child, _)| *child == formal)
                    .map(|(_, actual)| *actual)
            })
            .collect::<Option<Vec<_>>>()?;
        Some((
            Output {
                target: mapped.iter().find(|(child, _)| *child == output)?.1,
                dependencies,
                coverage: DefinitionCoverage::WholeArray,
                location: self.context.files[file].location(call.range()),
                enforced_boolean: false,
                scoped_local: false,
            },
            arrays,
        ))
    }
    fn full_weighted_siblings(
        &self,
        file: FileId,
        clauses: &[Clause<'a>],
        [load, bin, weight]: [DeclarationId; 3],
        view: &CallableFacts,
    ) -> bool {
        let [total, lower, upper] = clauses else {
            return false;
        };
        let par_int = TypeInst::par(TypeKind::Int);
        let var_int = par_int.clone().with_inst(Instantiation::Decision);
        let var_bool = TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision);
        let par_set = TypeInst::par(TypeKind::Set(Box::new(par_int.clone())));
        let typed = |node: &SyntaxNode, ty: &TypeInst| {
            self.expression_type(self.view(file, node, view), file, node)
                .is_some_and(|e| e.ty == *ty)
        };
        let sides: Vec<_> = total.node.child_nodes().map(unwrap).collect();
        if sides.len() != 2 || !self.core(file, total.node, view, "=")
            || !typed(total.node, &var_bool)
            || self.operation_fact(self.view(file, total.node, view), file, total.node)
                .is_none_or(|fact| !matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.as_slice() == [var_int.clone(), par_int.clone()] && *return_type == var_bool))
        {
            return false;
        }
        // A targetless sum equality still has two evaluated sources. It grants
        // no output; neither aggregate may be skipped because target() fails.
        for (side, expected) in sides.iter().zip([load, weight]) {
            let arguments: Vec<_> = side.child_nodes().map(unwrap).collect();
            if side.kind() != NodeKind::CallExpression
                || arguments.len() != 1
                || !self.core(file, side, view, "sum")
                || arguments[0].kind() != NodeKind::Expression
                || self.reference(file, arguments[0]) != Some(expected)
                || self.dependencies(file, arguments[0], view, &[]).is_err()
                || self.aggregate_source_safety(file, side, view, &[])
                    != Some(DefinitionSafety::Supported)
            {
                return false;
            }
        }
        for (position, relation) in [lower, upper].iter().enumerate() {
            let operands: Vec<_> = relation.node.child_nodes().map(unwrap).collect();
            let Some(generator) = relation.generators.first() else {
                return false;
            };
            let Some(source) = generator.child_nodes().next() else {
                return false;
            };
            let expected_operands = if position == 0 {
                [par_int.clone(), var_int.clone()]
            } else {
                [var_int.clone(), par_int.clone()]
            };
            if operands.len() != 2
                || !self.core(file, relation.node, view, "<=")
                || self.integer_index_set(file, source, view) != Some(bin)
                || self
                    .relation_iterations(file, &relation.generators, 0, view, false)
                    .is_err()
                || !typed(relation.node, &var_bool)
                || self
                    .operation_fact(self.view(file, relation.node, view), file, relation.node)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == expected_operands
                            && *return_type == var_bool)
                    })
            {
                return false;
            }
            let extremum = operands[position];
            let arguments: Vec<_> = extremum.child_nodes().collect();
            let name = if position == 0 { "min" } else { "max" };
            let selected = operands[1 - position];
            let selection: Vec<_> = selected.child_nodes().map(unwrap).collect();
            if extremum.kind() != NodeKind::CallExpression
                || arguments.len() != 1
                || !self.core(file, extremum, view, name)
                || self.integer_index_set(file, arguments[0], view) != Some(load)
                || !typed(extremum, &par_int)
                || self
                    .operation_fact(self.view(file, extremum, view), file, extremum)
                    .is_none_or(|fact| {
                        !matches!(&fact.outcome,
                        CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.as_slice() == [par_set.clone()] && *return_type == par_int)
                    })
                || selected.kind() != NodeKind::ArrayAccessExpression
                || selection.len() != 2
                || selection
                    .iter()
                    .any(|node| node.kind() != NodeKind::Expression)
                || self.reference(file, selection[0]) != Some(bin)
                || !self.member_index(file, bin, selection[1], &relation.generators, view)
                || !typed(selected, &var_int)
                || !typed(selection[1], &par_int)
            {
                return false;
            }
            if self
                .boolean_relation_dependencies(file, relation.node, view, &relation.generators)
                .is_err()
                && !matches!(
                    self.initialized_children_safety(
                        file,
                        &[relation.node, source],
                        view,
                        &relation.generators
                    ),
                    DefinitionSafety::Unknown(_)
                )
            {
                return false;
            }
            // This is the existing nondefining relation inspection: checked
            // uncertainty about min/max supplies no nonempty-load certificate.
        }
        true
    }
    fn nested_weighted_dependencies(
        &self,
        file: FileId,
        equality: &'a SyntaxNode,
        output: DeclarationId,
        generators: &[&'a SyntaxNode],
        pair: [DeclarationId; 2],
        view: &CallableFacts,
    ) -> Option<Vec<DeclarationId>> {
        let present = |node: &SyntaxNode, kind, instantiation| {
            self.expression_type(self.view(file, node, view), file, node)
                .is_some_and(|e| {
                    e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.kind == kind
                        && e.ty.instantiation == instantiation
                })
        };
        let sides: Vec<_> = equality.child_nodes().map(unwrap).collect();
        let position = sides.iter().position(|side| {
            self.target(file, side, view, generators)
                == Some((output, DefinitionCoverage::WholeArray))
        })?;
        if sides.len() != 2
            || !present(equality, TypeKind::Bool, Instantiation::Decision)
            || !present(sides[position], TypeKind::Int, Instantiation::Decision)
        {
            return None;
        }
        let sum = sides[1 - position];
        if sum.kind() != NodeKind::GeneratorCallExpression || !self.core(file, sum, view, "sum")
            || !present(sum, TypeKind::Int, Instantiation::Decision)
            || !self.operation_fact(self.view(file, sum, view), file, sum)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if *return_type == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision) && parameters.len() == 1
                        && parameters[0].known() && !optional(&parameters[0])
                        && parameters[0].instantiation == Instantiation::Decision
                        && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                            if indices.as_slice() == [TypeInst::par(TypeKind::Int)]
                                && **element == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision))))
        {
            return None;
        }
        let parts: Vec<_> = sum.child_nodes().collect();
        if parts.len() != 2 {
            return None;
        }
        let list = *parts
            .iter()
            .find(|node| node.kind() == NodeKind::GeneratorList)?;
        let inner: Vec<_> = list.child_nodes().collect();
        if inner.len() != 1 || inner[0].child_nodes().count() != 1 {
            return None;
        }
        let source = inner[0].child_nodes().next()?;
        let bin = self.integer_index_set(file, source, view)?;
        let mut all = generators.to_vec();
        all.extend(inner.iter().copied());
        if self.iterations(file, &all, view).is_err()
            || self
                .relation_iterations(file, &all, generators.len(), view, false)
                .is_err()
            || self
                .bindings
                .declarations
                .iter()
                .filter(|declaration| {
                    declaration.file == file
                        && declaration.role == DeclarationRole::Generator
                        && declaration.syntax_range == inner[0].range()
                })
                .count()
                != 1
        {
            return None;
        }
        let product = unwrap(
            *parts
                .iter()
                .find(|node| node.kind() != NodeKind::GeneratorList)?,
        );
        let operands: Vec<_> = product.child_nodes().map(unwrap).collect();
        if product.kind() != NodeKind::BinaryExpression || operands.len() != 2
            || !self.core(file, product, view, "*")
            || !present(product, TypeKind::Int, Instantiation::Decision)
            || !self.operation_fact(self.view(file, product, view), file, product)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.as_slice() == [TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision), TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision)]
                        && *return_type == TypeInst::par(TypeKind::Int).with_inst(Instantiation::Decision)
                        && parameters.iter().zip(&operands).all(|(formal, actual)|
                            self.expression_type(self.view(file, actual, view), file, actual)
                                .is_some_and(|e| crate::types::coerces(&e.ty, formal)))))
        {
            return None;
        }
        let weight_position = operands.iter().position(|node| {
            node.kind() == NodeKind::ArrayAccessExpression
                && present(node, TypeKind::Int, Instantiation::Parameter)
        })?;
        let weight = operands[weight_position];
        let selection: Vec<_> = weight.child_nodes().map(unwrap).collect();
        if selection.len() != 2
            || selection
                .iter()
                .any(|node| node.kind() != NodeKind::Expression)
            || !present(selection[1], TypeKind::Int, Instantiation::Parameter)
            || self.reference(file, selection[1]).is_none_or(|id| {
                let d = &self.bindings.declarations[id.0];
                d.file != file
                    || d.role != DeclarationRole::Generator
                    || d.syntax_range != inner[0].range()
            })
        {
            return None;
        }
        let weight_array = self.reference(file, selection[0])?;
        if pair != [bin, weight_array] && pair != [weight_array, bin] {
            return None;
        }
        let comparison = operands[1 - weight_position];
        let compared: Vec<_> = comparison.child_nodes().map(unwrap).collect();
        if comparison.kind() != NodeKind::BinaryExpression || compared.len() != 2
            || !self.core(file, comparison, view, "=")
            || !present(comparison, TypeKind::Bool, Instantiation::Decision)
            || !self.operation_fact(self.view(file, comparison, view), file, comparison)
                .is_some_and(|fact| matches!(&fact.outcome,
                    CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && *return_type == TypeInst::par(TypeKind::Bool).with_inst(Instantiation::Decision)
                        && parameters.iter().zip(&compared).all(|(formal, actual)| formal.known()
                            && !optional(formal) && formal.kind == TypeKind::Int
                            && self.expression_type(self.view(file, actual, view), file, actual)
                                .is_some_and(|e| crate::types::coerces(&e.ty, formal)))))
        {
            return None;
        }
        let bin_position = compared.iter().position(|node| {
            let children: Vec<_> = node.child_nodes().map(unwrap).collect();
            node.kind() == NodeKind::ArrayAccessExpression
                && children.len() == 2
                && children[0].kind() == NodeKind::Expression
                && self.reference(file, children[0]) == Some(bin)
                && self.reference(file, children[1]) == self.reference(file, selection[1])
                && self.member_index(file, bin, children[1], &all, view)
                && present(node, TypeKind::Int, Instantiation::Decision)
        })?;
        let outer_index = compared[1 - bin_position];
        if outer_index.kind() != NodeKind::Expression
            || !present(outer_index, TypeKind::Int, Instantiation::Parameter)
            || self.reference(file, outer_index).is_none_or(|id| {
                let d = &self.bindings.declarations[id.0];
                d.file != file
                    || d.role != DeclarationRole::Generator
                    || !generators
                        .iter()
                        .any(|generator| generator.range() == d.syntax_range)
            })
        {
            return None;
        }
        // Only weight[i] needs the invocation's index-set equality. All headers
        // and remaining operands keep strict source/membership dependencies.
        let mut dependencies = self.dependencies(file, selection[0], view, &all).ok()?;
        extend(
            &mut dependencies,
            self.dependencies(file, selection[1], view, &all).ok()?,
        );
        extend(
            &mut dependencies,
            self.dependencies(file, comparison, view, &all).ok()?,
        );
        extend(
            &mut dependencies,
            self.dependencies(file, source, view, generators).ok()?,
        );
        for generator in generators {
            // This exact index_set(output) is inspected as the target's traversal,
            // not a dependency on the output values themselves.
            self.dependencies(file, generator.child_nodes().next()?, view, &[])
                .ok()?;
        }
        Some(dependencies)
    }
    fn asserted_index_membership(
        &self,
        file: FileId,
        access: &'a SyntaxNode,
        array: DeclarationId,
        index: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let Some(binder) = self
            .reference(file, index)
            .filter(|_| index.kind() == NodeKind::Expression)
        else {
            return false;
        };
        let declaration = &self.bindings.declarations[binder.0];
        if declaration.file != file || declaration.role != DeclarationRole::Generator {
            return false;
        }
        let Some(source) = generators
            .iter()
            .find(|g| g.range() == declaration.syntax_range)
            .and_then(|g| g.child_nodes().next())
            .and_then(|source| self.integer_index_set(file, source, view))
        else {
            return false;
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= access.range().start && access.range().end <= node.range().end
        };
        let mut nodes = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = nodes.pop() {
            if node.kind() == NodeKind::CallExpression
                && let Some((_, pair)) = self.direct_asserted_forall(file, node, view)
                && (pair == [array, source] || pair == [source, array])
                && self
                    .assertion_arguments(file, node, view)
                    .is_some_and(|args| contains(args[2].1))
            {
                // Normal return from this exact guard proves this selector's
                // membership only. Whole-array coverage keeps its direct check.
                return true;
            }
            nodes.extend(node.child_nodes().filter(|child| contains(child)));
        }
        false
    }
    // Only this complete owning assertion can admit a constructed actual pair.
    fn asserted_forall_contract(
        &self,
        instance: &Instance<'a>,
    ) -> Option<(DeclarationId, [DeclarationId; 2])> {
        let [body] = instance.clauses.as_slice() else {
            return None;
        };
        if !matches!(body.kind, ClauseKind::Assertion) {
            return None;
        }
        self.direct_asserted_forall(body.file, body.node, &instance.view)
            .or_else(|| {
                self.nested_asserted_forall(body.file, body.node, &instance.view)
                    .map(|(output, arrays)| (output.target, arrays))
            })
    }
    fn constructed_array_actual(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        lexical: &[&'a SyntaxNode],
    ) -> Result<Option<ConstructedArray>, String> {
        let written_actual = node;
        let node = unwrap(node);
        if node.kind() != NodeKind::ArrayComprehension {
            return Ok(None);
        }
        // The whole source graph remains an error veto. Its generic collection
        // uncertainty is resolved only by the exact shape/membership checks below.
        if let DefinitionSafety::Unsupported(reason) =
            self.initialized_source_safety(file, written_actual, view, lexical, &mut Vec::new())
        {
            return Err(reason);
        }
        if let Some(reason) = self.closed_integer_source_error(file, written_actual, true, true) {
            return Err(reason);
        }
        let present_int = |ty: &TypeInst, inst| {
            ty.known() && !optional(ty) && ty.instantiation == inst && ty.kind == TypeKind::Int
        };
        let integer_set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if present_int(element, Instantiation::Parameter))
        };
        let facts = self.view(file, node, view);
        let ty = &self
            .expression_type(facts, file, node)
            .ok_or("constructed array actual type is unavailable")?
            .ty;
        let TypeKind::Array { indices, element } = &ty.kind else {
            return Err("constructed actual must retain an integer array type".into());
        };
        if !ty.known()
            || optional(ty)
            || indices.len() != 1
            || !present_int(&indices[0], Instantiation::Parameter)
            || !element.known()
            || optional(element)
            || element.kind != TypeKind::Int
            || element.instantiation != ty.instantiation
        {
            return Err("constructed array qualifiers or axis are unsupported".into());
        }
        let [outer] = lexical else {
            return Err("constructed array requires one owning row traversal".into());
        };
        let children: Vec<_> = node.child_nodes().collect();
        let [body, list] = children.as_slice() else {
            return Err("constructed array body or headers are unsupported".into());
        };
        if list.kind() != NodeKind::GeneratorList {
            return Err("constructed array headers are unavailable".into());
        }
        let headers: Vec<_> = list.child_nodes().collect();
        let [inner] = headers.as_slice() else {
            return Err("constructed array requires one inner traversal".into());
        };
        let all = [*outer, *inner];
        let mut binders = Vec::new();
        let mut dependencies = Vec::new();
        for (position, generator) in all.iter().enumerate() {
            let declared: Vec<_> = self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generator.range()
                })
                .collect();
            let [binder] = declared.as_slice() else {
                return Err("constructed traversal binder identity is unsupported".into());
            };
            if crate::domains::generator_slots(&self.context.files[file].parsed, generator) != 1
                || generator.child_nodes().count() != 1
                || !present_int(
                    &facts.declarations[binder.id.0].ty,
                    Instantiation::Parameter,
                )
            {
                return Err(
                    "constructed traversal is filtered, assigned or not present Int".into(),
                );
            }
            let source = generator
                .child_nodes()
                .next()
                .ok_or("constructed source unavailable")?;
            if self
                .expression_type(self.view(file, source, facts), file, source)
                .is_none_or(|e| !integer_set(&e.ty))
            {
                return Err(
                    "constructed traversal source is not a present parameter Int set".into(),
                );
            }
            match self.initialized_source_safety(
                file,
                source,
                view,
                &all[..position],
                &mut Vec::new(),
            ) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
            extend(
                &mut dependencies,
                self.dependencies(file, source, view, &all[..position])?,
            );
            binders.push(binder.id);
        }
        if binders[0] == binders[1] {
            return Err("constructed row selectors must be distinct binders".into());
        }
        self.iterations(file, &all, view)?;
        self.relation_iterations(file, &all, 0, view, false)?;
        let mut inspected = vec![written_actual, *outer];
        while let Some(part) = inspected.pop() {
            if !crate::definitions::annotations_safe(self.context, file, part) {
                return Err("constructed actual or header annotation is unsupported".into());
            }
            inspected.extend(part.child_nodes());
        }
        let body = unwrap(body);
        if body.kind() != NodeKind::ArrayAccessExpression {
            return Err("constructed array element must retain a direct array selection".into());
        }
        let selected: Vec<_> = body.child_nodes().collect();
        let subject = unwrap(
            selected
                .first()
                .ok_or("constructed selection owner unavailable")?,
        );
        let bare = |value: &SyntaxNode| {
            value.kind() == NodeKind::Expression
                && matches!(crate::domains::tokens(&self.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
        };
        let target = self
            .reference(file, subject)
            .filter(|_| bare(subject))
            .ok_or("constructed selection requires a bare array owner")?;
        let owner = &self.bindings.declarations[target.0];
        if owner.file != file || owner.role != DeclarationRole::Value || !owner.top_level {
            return Err("constructed selection owner scope is unsupported".into());
        }
        let owner_type = &facts.declarations[target.0].ty;
        let TypeKind::Array {
            indices: owner_axes,
            element: owner_element,
        } = &owner_type.kind
        else {
            return Err("constructed selection owner type is unavailable".into());
        };
        if !owner_type.known()
            || optional(owner_type)
            || owner_type.instantiation != ty.instantiation
            || owner_element != element
            || !owner_axes
                .iter()
                .all(|axis| present_int(axis, Instantiation::Parameter))
            || self
                .expression_type(self.view(file, body, facts), file, body)
                .is_none_or(|e| &e.ty != element.as_ref())
        {
            return Err("constructed selection type or optionality is unsupported".into());
        }
        let written = find_node(
            self.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )
        .ok_or("constructed selection declaration unavailable")?;
        self.type_dependencies(file, written, view, &[])?;
        match self.initialized_source_safety(file, subject, view, lexical, &mut Vec::new()) {
            DefinitionSafety::Supported => {}
            DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                return Err(reason);
            }
        }
        if let Some(reason) = self.closed_integer_source_error(file, written, true, true) {
            return Err(reason);
        }
        let mut declarations = vec![written];
        while let Some(part) = declarations.pop() {
            if !crate::definitions::annotations_safe(self.context, file, part) {
                return Err("constructed selection declaration annotation is unsupported".into());
            }
            declarations.extend(part.child_nodes());
        }
        let selectors = &selected[1..];
        for selector in selectors {
            let selector = unwrap(selector);
            if !bare(selector)
                || self
                    .expression_type(self.view(file, selector, facts), file, selector)
                    .is_none_or(|e| !present_int(&e.ty, Instantiation::Parameter))
            {
                return Err(
                    "constructed element selector is not a bare parameter Int binder".into(),
                );
            }
            match self.initialized_source_safety(file, selector, view, &all, &mut Vec::new()) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
        }
        let source = unwrap(
            inner
                .child_nodes()
                .next()
                .ok_or("constructed inner source unavailable")?,
        );
        let (item_source, coverage) = if owner_axes.len() == 1 && selectors.len() == 1 {
            if self.reference(file, unwrap(selectors[0])) != Some(binders[1])
                || source.kind() != NodeKind::ArrayAccessExpression
            {
                return Err("constructed item selection lacks its exact set traversal".into());
            }
            let parts: Vec<_> = source.child_nodes().map(unwrap).collect();
            let [set_array, row] = parts.as_slice() else {
                return Err("constructed item set must retain one row selector".into());
            };
            let set_id = self
                .reference(file, set_array)
                .filter(|_| bare(set_array))
                .ok_or("constructed item set array identity is unavailable")?;
            if !bare(row) || self.reference(file, row) != Some(binders[0]) {
                return Err("constructed item set must retain the owning outer binder".into());
            }
            let set_type = &facts.declarations[set_id.0].ty;
            if !set_type.known()
                || optional(set_type)
                || set_type.instantiation != Instantiation::Parameter
                || !matches!(&set_type.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && present_int(&indices[0], Instantiation::Parameter)
                        && integer_set(element))
            {
                return Err(
                    "constructed item source is not a present parameter array of Int sets".into(),
                );
            }
            let set_owner = &self.bindings.declarations[set_id.0];
            if set_owner.file != file
                || set_owner.role != DeclarationRole::Value
                || !set_owner.top_level
            {
                return Err("constructed item set declaration scope is unsupported".into());
            }
            let set_written = find_node(
                self.context.files[file].parsed.tree(),
                &set_owner.syntax_range,
                set_owner.role,
            )
            .ok_or("constructed item set declaration unavailable")?;
            self.type_dependencies(file, set_written, view, &[])?;
            let Domain::Array { indices, element } =
                crate::domains::bare_index_domain(&self.domains.declarations[set_id.0].domain)
            else {
                return Err("constructed item set written domains are unavailable".into());
            };
            let Domain::Set(bound) = element.as_ref() else {
                return Err("constructed item set requires a written element bound".into());
            };
            let Domain::Array {
                indices: destination,
                ..
            } = crate::domains::bare_index_domain(&self.domains.declarations[target.0].domain)
            else {
                return Err("constructed item destination axis unavailable".into());
            };
            if indices.len() != 1
                || destination.len() != 1
                || !matches!(bound.as_ref(), Domain::Named { .. } | Domain::Range { .. })
                || crate::domains::same_members(bound, &destination[0])? != Some(true)
            {
                return Err(
                    "constructed item set bound does not prove destination membership".into(),
                );
            }
            // This is subset membership only. Neither the selected set's count
            // nor complete coverage of the input owner follows from its bound.
            (
                Some((file, set_id, binders[0])),
                DefinitionCoverage::ArrayElement,
            )
        } else if owner_axes.len() == 2
            && selectors.len() == 2
            && ty.instantiation == Instantiation::Decision
            && self.reference(file, unwrap(selectors[0])) == Some(binders[0])
            && self.reference(file, unwrap(selectors[1])) == Some(binders[1])
            && bare(source)
        {
            if dependencies.contains(&target) {
                return Err("constructed output traversal depends on destination values".into());
            }
            let coverage = complete_array_coverage(
                self.context,
                self.bindings,
                (self.calls, facts),
                self.instantiations,
                self.domains,
                (file, target),
                (selectors, &all),
            );
            if coverage != DefinitionCoverage::WholeArray {
                return Err("constructed output row lacks complete rectangular traversal".into());
            }
            self.dependencies(file, body, view, &all)?;
            (None, coverage)
        } else {
            return Err(
                "constructed actual is not a supported item array or complete output row".into(),
            );
        };
        extend(&mut dependencies, vec![target]);
        Ok(Some(ConstructedArray {
            target,
            item_source,
            dependencies,
            coverage,
        }))
    }
    fn constructed_call_row(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        instance: &Instance<'a>,
        formal: DeclarationId,
    ) -> Result<Option<ConstructedArray>, String> {
        if instance.recursive
            || self
                .asserted_forall_contract(instance)
                .is_none_or(|(output, _)| output != formal)
        {
            return Ok(None);
        }
        self.asserted_call_indices(clause, view, instance, true)?;
        let (file, node) = self
            .actual(clause, instance.id, formal, view)
            .ok_or("constructed output actual/default correspondence is unavailable")?;
        let lexical = if file == clause.file
            && clause.node.range().start <= node.range().start
            && node.range().end <= clause.node.range().end
        {
            &clause.generators[..]
        } else {
            &[]
        };
        let Some(actual) = self.constructed_array_actual(file, node, view, lexical)? else {
            return Ok(None);
        };
        if actual.item_source.is_some() || actual.coverage != DefinitionCoverage::WholeArray {
            return Ok(None);
        }
        Ok(Some(actual))
    }
    fn complete_constructed_call_output(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        known: &[CallableOutput],
        target: DeclarationId,
    ) -> bool {
        let Some((id, parameters)) = self.resolved(clause.file, clause.node, view) else {
            return false;
        };
        let Some(instance) = self
            .instances
            .iter()
            .find(|i| i.id == id && i.parameters == parameters)
        else {
            return false;
        };
        known
            .iter()
            .filter(|g| {
                g.callable == id
                    && g.parameters == parameters
                    && g.coverage == DefinitionCoverage::WholeArray
                    && self.bindings.declarations[g.target.0].role == DeclarationRole::Parameter
            })
            .any(|g| {
                self.constructed_call_row(clause, view, instance, g.target)
                    .is_ok_and(|actual| actual.is_some_and(|actual| actual.target == target))
            })
    }
    fn asserted_call_indices(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        instance: &Instance<'a>,
        allow_constructed: bool,
    ) -> Result<(), String> {
        let Some((_, arrays)) = self.asserted_forall_contract(instance) else {
            return Ok(());
        };
        if allow_constructed && !instance.recursive {
            let actuals: Vec<_> = arrays
                .iter()
                .map(|formal| {
                    self.actual(clause, instance.id, *formal, view)
                        .ok_or("asserted array actual/default mapping is unavailable")
                })
                .collect::<Result<_, _>>()?;
            if actuals
                .iter()
                .all(|(_, node)| unwrap(node).kind() == NodeKind::ArrayComprehension)
            {
                let mut sources = Vec::new();
                for ((file, node), formal) in actuals.into_iter().zip(arrays) {
                    if self
                        .expression_type(self.view(file, node, view), file, node)
                        .is_none_or(|e| e.ty != instance.view.declarations[formal.0].ty)
                    {
                        return Err(
                            "constructed asserted actual does not retain its selected formal type"
                                .into(),
                        );
                    }
                    let lexical = if file == clause.file
                        && clause.node.range().start <= node.range().start
                        && node.range().end <= clause.node.range().end
                    {
                        &clause.generators[..]
                    } else {
                        &[]
                    };
                    let actual = self
                        .constructed_array_actual(file, node, view, lexical)?
                        .ok_or("asserted constructed array actual unavailable")?;
                    sources.push(actual.item_source.ok_or(
                        "asserted constructed input requires a checked item-set selection",
                    )?);
                }
                // Both implicit axes are 1..card(this exact inspected set),
                // including zero and noncontiguous members. No count is guessed.
                return if sources[0] == sources[1] {
                    Ok(())
                } else {
                    Err("constructed asserted array axes have different source identities".into())
                };
            }
        }
        let mut axes = Vec::new();
        for formal in arrays {
            let (file, value) = self
                .actual(clause, instance.id, formal, view)
                .ok_or("asserted array actual/default mapping is unavailable")?;
            let value = unwrap(value);
            if value.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, value).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("asserted array actual/default must be a bare array identity".into());
            }
            let id = self
                .reference(file, value)
                .ok_or("asserted array actual identity is unavailable")?;
            let facts = self.view(file, value, view);
            let ty = &facts.declarations[id.0].ty;
            if !ty.known()
                || optional(ty)
                || !matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Int)
                || self
                    .expression_type(facts, file, value)
                    .is_none_or(|e| e.ty != *ty)
            {
                return Err("asserted array actual type or optionality is unsupported".into());
            }
            let owner = &self.bindings.declarations[id.0];
            if !matches!(
                owner.role,
                DeclarationRole::Value | DeclarationRole::Parameter
            ) || owner.role == DeclarationRole::Value && !owner.top_level
            {
                return Err("asserted array actual declaration scope is unsupported".into());
            }
            let written = find_node(
                self.context.files[owner.file].parsed.tree(),
                &owner.syntax_range,
                owner.role,
            )
            .ok_or("asserted array actual declaration is unavailable")?;
            let Domain::Array { indices, .. } = &self.domains.declarations[id.0].domain else {
                return Err("asserted array written axis is unavailable".into());
            };
            let [axis] = indices.as_slice() else {
                return Err("asserted array must retain one written axis".into());
            };
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, owner.file, node) {
                    return Err("asserted array source annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            self.type_dependencies(owner.file, written, view, &[])?;
            match self.initialized_source_safety(file, value, view, &[], &mut Vec::new()) {
                DefinitionSafety::Supported => {}
                DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                    return Err(reason);
                }
            }
            for initializer in written.child_nodes().filter(|n| is_expression(n.kind())) {
                let initializer = unwrap(initializer);
                if initializer.kind() != NodeKind::ArrayLiteral
                    || initializer
                        .child_nodes()
                        .any(|cell| cell.kind() == NodeKind::IndexedArrayEntry)
                {
                    return Err("asserted array initializer shape is unproved".into());
                }
                let TypeKind::Array { element, .. } = &ty.kind else {
                    return Err("asserted array literal element type is unavailable".into());
                };
                for cell in initializer.child_nodes() {
                    if self
                        .expression_type(self.view(owner.file, cell, view), owner.file, cell)
                        .is_none_or(|e| {
                            !e.ty.known()
                                || optional(&e.ty)
                                || e.ty.kind != TypeKind::Int
                                || !crate::types::coerces(&e.ty, element)
                        })
                    {
                        return Err("asserted array literal cell type is unsupported".into());
                    }
                }
                match self.initialized_source_safety(
                    owner.file,
                    initializer,
                    view,
                    &[],
                    &mut Vec::new(),
                ) {
                    DefinitionSafety::Supported => {}
                    DefinitionSafety::Unknown(reason) | DefinitionSafety::Unsupported(reason) => {
                        return Err(reason);
                    }
                }
                if let Some(reason) =
                    self.closed_integer_source_error(owner.file, initializer, true, false)
                {
                    return Err(reason);
                }
                let count = i64::try_from(initializer.child_nodes().count())
                    .map_err(|_| "asserted array literal extent is unsupported")?;
                let literal_axis = Domain::Range {
                    lower: crate::NumericBound::Integer(1),
                    upper: crate::NumericBound::Integer(count),
                };
                if crate::domains::same_members(axis, &literal_axis)? != Some(true) {
                    return Err(
                        "asserted array initializer does not retain its written axis".into(),
                    );
                }
            }
            axes.push(axis);
        }
        match crate::domains::same_members(axes[0], axes[1])? {
            Some(true) => Ok(()),
            Some(false) => Err("asserted array index equality is false for these actuals".into()),
            None => Err("asserted array actual index equality is unproved".into()),
        }
    }
    fn exact_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        generators: &[&SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        generators.len() == 1
            && self.iterations(file, generators, view).is_ok()
            && self.member_index(file, array, index, generators, view)
    }
    fn shifted_index_membership(
        &self,
        file: FileId,
        access: &'a SyntaxNode,
        array: DeclarationId,
        index: &'a SyntaxNode,
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let operands: Vec<_> = index.child_nodes().map(unwrap).collect();
        let parameter_int = |node: &SyntaxNode| {
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == self.context.files[file].location(node.range()).range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
                    && e.ty.kind == TypeKind::Int
            })
        };
        if index.kind() != NodeKind::BinaryExpression
            || operands.len() != 2
            || !self.core(file, index, view, "-")
            || !parameter_int(index)
            || !operands
                .iter()
                .all(|node| node.kind() == NodeKind::Expression && parameter_int(node))
        {
            return false;
        }
        let (Some(later), Some(earlier)) = (
            self.reference(file, operands[0]),
            self.reference(file, operands[1]),
        ) else {
            return false;
        };
        if earlier == later {
            return false;
        }
        let array_type = &view.declarations[array.0].ty;
        if !array_type.known()
            || optional(array_type)
            || !matches!(&array_type.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].kind == TypeKind::Int
                    && indices[0].instantiation == Instantiation::Parameter
                    && element.kind == TypeKind::Int)
        {
            return false;
        }
        let Domain::Array { indices, .. } = &self.domains.declarations[array.0].domain else {
            return false;
        };
        let upper_identity = |domain: &Domain| match domain {
            Domain::Range {
                lower,
                upper:
                    crate::NumericBound::Symbol(id)
                    | crate::NumericBound::Defined {
                        declaration: id, ..
                    },
            } if crate::domains::invariant_integer(lower).ok() == Some(Some(1)) => Some(*id),
            _ => None,
        };
        let Some(upper) = indices.first().and_then(upper_identity) else {
            return false;
        };
        let owner = &self.bindings.declarations[array.0];
        let Some(declaration) = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) else {
            return false;
        };
        if self
            .type_dependencies(owner.file, declaration, view, &[])
            .is_err()
        {
            return false;
        }
        for binder in [earlier, later] {
            let declaration = &self.bindings.declarations[binder.0];
            if declaration.file != file || declaration.role != DeclarationRole::Generator {
                return false;
            }
            let Some((position, generator)) = generators
                .iter()
                .enumerate()
                .find(|(_, g)| g.range() == declaration.syntax_range)
            else {
                return false;
            };
            let Some(source) = generator.child_nodes().next().map(unwrap) else {
                return false;
            };
            if source.kind() != NodeKind::RangeExpression
                || !self.core(file, source, view, "..")
                || generator
                    .child_nodes()
                    .any(|n| n.kind() == NodeKind::WhereFilter)
                || !generator.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
                || upper_identity(&expression_domain(
                    self.context,
                    self.bindings,
                    file,
                    source,
                )) != Some(upper)
                || self
                    .dependencies(file, source, view, &generators[..position])
                    .is_err()
            {
                return false;
            }
        }
        // Only the true body of this exact parameter comparison supplies the
        // bound: 1 <= j-i <= U-1 <= U. It says nothing about whole output coverage.
        let contains = |node: &SyntaxNode| {
            node.range().start <= access.range().start && access.range().end <= node.range().end
        };
        let mut containing = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
            {
                let condition = unwrap(children[0]);
                let operands: Vec<_> = condition.child_nodes().map(unwrap).collect();
                if operands.len() == 2
                    && operands.iter().all(|n| n.kind() == NodeKind::Expression)
                    && self.reference(file, operands[0]) == Some(earlier)
                    && self.reference(file, operands[1]) == Some(later)
                    && self.core(file, condition, view, "<")
                    && self
                        .view(file, condition, view)
                        .expressions
                        .iter()
                        .any(|e| {
                            e.file == file
                                && e.location.range
                                    == self.context.files[file].location(condition.range()).range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.instantiation == Instantiation::Parameter
                                && e.ty.kind == TypeKind::Bool
                        })
                    && self.dependencies(file, condition, view, generators).is_ok()
                {
                    return true;
                }
            }
            containing.extend(children.into_iter().filter(|child| contains(child)));
        }
        false
    }
    fn written_index_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        indices: &[&'a SyntaxNode],
        generators: &[&'a SyntaxNode],
        view: &CallableFacts,
    ) -> bool {
        let array_type = &view.declarations[array.0].ty;
        let TypeKind::Array { indices: types, .. } = &array_type.kind else {
            return false;
        };
        let mut domain = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let Domain::Array {
            indices: domains, ..
        } = domain
        else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || !matches!(indices.len(), 1 | 2)
            || indices.len() != domains.len()
            || indices.len() != types.len()
        {
            return false;
        }
        let difference_subset = |written_source: &'a SyntaxNode, axis: DeclarationId| {
            let source = unwrap(written_source);
            let source_type = self.expression_type(view, file, source).map(|e| &e.ty);
            if source.kind() != NodeKind::BinaryExpression {
                return false;
            }
            let operands: Vec<_> = source.child_nodes().collect();
            let [left, right] = operands.as_slice() else {
                return false;
            };
            let parameter_set = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element)
                                if element.kind == TypeKind::Int
                                    && element.instantiation == Instantiation::Parameter)
            };
            let operand_type = |operand| {
                self.expression_type(view, file, operand)
                    .map(|fact| &fact.ty)
            };
            if !self.core(file, source, view, "diff")
                || source_type.is_none_or(|ty| !parameter_set(ty))
                || operands.iter().any(|operand| {
                    let bare = unwrap(operand);
                    bare.kind() != NodeKind::Expression
                        || self.reference(file, bare).is_none_or(|id| {
                            let owner = &self.bindings.declarations[id.0];
                            !owner.top_level || owner.role != DeclarationRole::Value
                        })
                        || operand_type(operand).is_none_or(|ty| !parameter_set(ty))
                })
                || self.reference(file, unwrap(left)) != Some(axis)
                || self.operation_fact(view, file, source).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2
                                    && Some(&parameters[0]) == operand_type(left)
                                    && Some(&parameters[1]) == operand_type(right)
                                    && Some(return_type) == source_type)
                })
            {
                return false;
            }
            // A checked difference contains only members of its exact
            // left axis. Inspect both written sources before using this
            // one-selector proof; it establishes no array coverage.
            if matches!(
                self.initialized_source_safety(
                    file,
                    written_source,
                    view,
                    generators,
                    &mut Vec::new(),
                ),
                DefinitionSafety::Unsupported(_)
            ) {
                return false;
            }
            self.closed_integer_source_error(file, written_source, true, true)
                .is_none()
        };
        indices
            .iter()
            .zip(domains)
            .zip(types)
            .all(|((index, domain), expected)| {
                if !matches!(domain, Domain::Named { .. } | Domain::Range { .. }) {
                    return false;
                }
                let index = unwrap(index);
                let index_type = self.expression_type(view, file, index).map(|e| &e.ty);
                if index.kind() != NodeKind::Expression
                    || !expected.known()
                    || optional(expected)
                    || expected.instantiation != Instantiation::Parameter
                    || index_type.is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.instantiation != Instantiation::Parameter
                            || t.kind != expected.kind
                    })
                {
                    return false;
                }
                let Some(binder) = self.reference(file, index) else {
                    return false;
                };
                let declaration = &self.bindings.declarations[binder.0];
                if declaration.file != file || declaration.role != DeclarationRole::Generator {
                    return false;
                }
                let Some(written_source) = generators
                    .iter()
                    .find(|g| g.range() == declaration.syntax_range)
                    .and_then(|g| g.child_nodes().next())
                else {
                    return false;
                };
                let source = unwrap(written_source);
                let source_type = self.expression_type(view, file, source).map(|e| &e.ty);
                if source_type.is_none_or(|t| !t.known() || optional(t)
                    || t.instantiation != Instantiation::Parameter
                    || !matches!(&t.kind, TypeKind::Set(element) if element.kind == expected.kind
                        && element.instantiation == Instantiation::Parameter))
                || self.dependencies(file, source, view, generators).is_err()
            {
                return false;
            }
                if source.kind() == NodeKind::RangeExpression {
                    let owner = &self.bindings.declarations[array.0];
                    let Some(array_declaration) = find_node(
                        self.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    ) else {
                        return false;
                    };
                    return matches!(domain, Domain::Range { .. })
                        && expected.kind == TypeKind::Int
                        && self.core(file, source, view, "..")
                        && generators.iter().any(|g| {
                            g.range() == declaration.syntax_range
                                && !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                                && g.children().iter().any(|c| matches!(c, SyntaxElement::Token(i)
                                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In))
                        })
                        && self
                            .type_dependencies(owner.file, array_declaration, view, &[])
                            .is_ok()
                        && crate::domains::same_members(
                            domain,
                            &expression_domain(self.context, self.bindings, file, source),
                        )
                        .is_ok_and(|same| same == Some(true));
                }
                if source.kind() == NodeKind::BinaryExpression && expected.kind == TypeKind::Int {
                    return matches!(domain, Domain::Named { declaration: axis, .. }
                        if difference_subset(written_source, *axis));
                }
                if source.kind() != NodeKind::Expression {
                    return false;
                }
                let Some(source_id) = self.reference(file, source) else {
                    return false;
                };
                if matches!(domain, Domain::Range { .. }) {
                    let set = &self.bindings.declarations[source_id.0];
                    if !set.top_level
                        || set.role != DeclarationRole::Value
                        || view.declarations[source_id.0].ty.instantiation
                            != Instantiation::Parameter
                    {
                        return false;
                    }
                    let Some(declaration) = find_node(
                        self.context.files[set.file].parsed.tree(),
                        &set.syntax_range,
                        set.role,
                    ) else {
                        return false;
                    };
                    let Some(range) = declaration
                        .child_nodes()
                        .find(|n| is_expression(n.kind()))
                        .map(unwrap)
                        .filter(|n| n.kind() == NodeKind::RangeExpression)
                    else {
                        return false;
                    };
                    let owner = &self.bindings.declarations[array.0];
                    let Some(array_declaration) = find_node(
                        self.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    ) else {
                        return false;
                    };
                    return self.core(set.file, range, view, "..")
                        && crate::definitions::annotations_safe(
                            self.context,
                            set.file,
                            declaration,
                        )
                        && self.dependencies(set.file, range, view, &[]).is_ok()
                        && self
                            .type_dependencies(owner.file, array_declaration, view, &[])
                            .is_ok()
                        && crate::domains::same_members(
                            domain,
                            &expression_domain(self.context, self.bindings, set.file, range),
                        )
                        .is_ok_and(|same| same == Some(true));
                }
                let Domain::Named {
                    declaration: domain_id,
                    ..
                } = domain
                else {
                    return false;
                };
                if source_id == *domain_id {
                    return true;
                }
                let local = &self.bindings.declarations[source_id.0];
                if local.file != file
                    || local.role != DeclarationRole::Local
                    || local.syntax_range.end > source.range().start
                {
                    return false;
                }
                let Some(written_local) = find_node(
                    self.context.files[file].parsed.tree(),
                    &local.syntax_range,
                    local.role,
                ) else {
                    return false;
                };
                let Some(collection) = written_local
                    .child_nodes().find(|n| is_expression(n.kind()))
                    .map(unwrap)
                    .filter(|n| n.kind() == NodeKind::SetComprehension) else {
                    return false;
                };
                let Some(list) = collection
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    return false;
                };
                let parts: Vec<_> = list.child_nodes().collect();
                if parts.len() != 1 {
                    return false;
                }
                let Some(body) = collection
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                    .map(unwrap)
                    .filter(|n| n.kind() == NodeKind::Expression)
                else {
                    return false;
                };
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == parts[0].range()
                    })
                    .collect();
                if binders.len() != 1
                    || self.reference(file, body) != Some(binders[0].id)
                {
                    return false;
                }
                let Some(written_source) = parts[0].child_nodes().next() else {
                    return false;
                };
                let source = unwrap(written_source);
                if source.kind() == NodeKind::Expression {
                    return self.reference(file, source) == Some(*domain_id);
                }
                if expected.kind != TypeKind::Int
                    || !difference_subset(written_source, *domain_id)
                    || self.type_dependencies(file, written_local, view, generators).is_err()
                    || self.closed_integer_source_error(file, written_local, true, true).is_some()
                {
                    return false;
                }
                let mut nodes = vec![written_local];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.context, file, node) {
                        return false;
                    }
                    // This source proof cannot independently inspect earlier
                    // Local aliases; the demonstrated filter uses only its
                    // generator and initialized top-level declarations.
                    if node.kind() == NodeKind::Expression
                        && self.reference(file, node).is_some_and(|id|
                            self.bindings.declarations[id.0].role == DeclarationRole::Local)
                    {
                        return false;
                    }
                    nodes.extend(node.child_nodes());
                }
                // An unchanged comprehension binder can only remove members
                // from its inspected diff source. Check every header/filter;
                // the subset proof supplies no extent or traversal coverage.
                !matches!(
                    self.initialized_source_safety(
                        file, collection, view, generators, &mut Vec::new(),
                    ),
                    DefinitionSafety::Unsupported(_)
                )
            })
    }
    fn length_equals(
        &self,
        file: FileId,
        guard: &SyntaxNode,
        array: DeclarationId,
        value: i64,
        view: &CallableFacts,
    ) -> bool {
        let guard = unwrap(guard);
        let facts = self.view(file, guard, view);
        let range = self.context.files[file].location(guard.range()).range;
        if !self.core(file, guard, view, "=")
            || !facts.expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.kind == TypeKind::Bool
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
            })
        {
            return false;
        }
        let parts: Vec<_> = guard.child_nodes().collect();
        if parts.len() != 2 {
            return false;
        }
        [(parts[0], parts[1]), (parts[1], parts[0])]
            .into_iter()
            .any(|(length, amount)| {
                let length = unwrap(length);
                let args: Vec<_> = length.child_nodes().collect();
                self.core(file, length, view, "length")
                    && args.len() == 1
                    && unwrap(args[0]).kind() == NodeKind::Expression
                    && self.reference(file, unwrap(args[0])) == Some(array)
                    && crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        amount,
                    ) == Ok(Some(value))
            })
    }
    fn array_bound_index(
        &self,
        file: FileId,
        array: DeclarationId,
        index: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let index = unwrap(index);
        let facts = self.view(file, index, view);
        let array_type = &facts.declarations[array.0].ty;
        let TypeKind::Array { indices, .. } = &array_type.kind else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || indices.len() != 1
            || !matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
        {
            return false;
        }
        let parameter = |node: &SyntaxNode, kind: &TypeKind| {
            let range = self.context.files[file].location(node.range()).range;
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.instantiation == Instantiation::Parameter
                    && &e.ty.kind == kind
            })
        };
        let args: Vec<_> = index.child_nodes().collect();
        if !(self.core(file, index, view, "min") || self.core(file, index, view, "max"))
            || args.len() != 1
            || !parameter(index, &indices[0].kind)
        {
            return false;
        }
        let set = unwrap(args[0]);
        let subjects: Vec<_> = set.child_nodes().collect();
        if !self.core(file, set, view, "index_set")
            || subjects.len() != 1
            || unwrap(subjects[0]).kind() != NodeKind::Expression
            || self.reference(file, unwrap(subjects[0])) != Some(array)
        {
            return false;
        }
        self.array_nonempty_branch(file, index, array, view)
    }
    fn array_nonempty_branch(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut containing = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            let parameter_bool = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && ty.kind == TypeKind::Bool
            };
            if node.kind() == NodeKind::BinaryExpression
                && children.len() == 2
                && contains(children[1])
                && self.core(file, node, view, "\\/")
                && self
                    .operation_fact(self.view(file, node, view), file, node)
                    .is_some_and(|call| {
                        matches!(&call.outcome,
                            CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(parameter_bool)
                                && parameter_bool(return_type))
                    })
                && self.length_equals(file, children[0], array, 0, view)
            {
                // This ordered parameter OR evaluates its right operand only after
                // the exact same-array zero-length test has returned false.
                return true;
            }
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
                && self
                    .view(file, unwrap(children[0]), view)
                    .expressions
                    .iter()
                    .any(|e| {
                        e.file == file
                            && e.location.range
                                == self.context.files[file]
                                    .location(unwrap(children[0]).range())
                                    .range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.instantiation == Instantiation::Parameter
                            && e.ty.kind == TypeKind::Bool
                    })
            {
                let mut conjuncts = vec![unwrap(children[0])];
                while let Some(guard) = conjuncts.pop() {
                    let parts: Vec<_> = guard.child_nodes().collect();
                    if parts.len() == 2 && self.core(file, guard, view, "/\\") {
                        conjuncts.extend(parts.into_iter().map(unwrap));
                    } else if self.length_equals(file, guard, array, 1, view) {
                        return true;
                    }
                }
            }
            if node.kind() == NodeKind::ConditionalExpression {
                let mut nonempty = false;
                for branch in &children {
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let body = match branch.kind() {
                        NodeKind::ConditionalBranch if parts.len() == 2 => parts[1],
                        NodeKind::ElseBranch if parts.len() == 1 => parts[0],
                        _ => break,
                    };
                    if contains(branch) {
                        // Reaching this later body makes earlier exact zero-length
                        // guards false. A false conjunction supplies no such proof.
                        if contains(body) && nonempty {
                            return true;
                        }
                        break;
                    }
                    if branch.kind() == NodeKind::ConditionalBranch {
                        nonempty |= self.length_equals(file, parts[0], array, 0, view);
                    }
                }
            }
            containing.extend(children.into_iter().filter(|child| contains(child)));
        }
        false
    }
    fn assertion_arguments(
        &self,
        file: FileId,
        call: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<Vec<(FileId, &'a SyntaxNode)>> {
        // MiniZinc 2.10.1 rejects named arguments for this builtin.
        if call
            .child_nodes()
            .any(|n| n.kind() == NodeKind::NamedArgument)
        {
            return None;
        }
        let CallOutcome::Resolved {
            declaration: id,
            parameters,
            return_type,
        } = &self
            .operation_fact(self.view(file, call, view), file, call)?
            .outcome
        else {
            return None;
        };
        if !self.core(file, call, view, "assert") || !return_type.known() || optional(return_type) {
            return None;
        }
        if !matches!(parameters.len(), 2 | 3) || call.child_nodes().count() != parameters.len() {
            return None;
        }
        (0..parameters.len())
            .map(|position| {
                call_argument(
                    self.context,
                    self.bindings,
                    self.view(file, call, view),
                    file,
                    call,
                    *id,
                    position,
                )
            })
            .collect()
    }
    fn integer_assertion_arguments(
        &self,
        file: FileId,
        call: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<(FileId, &'a SyntaxNode)>, String> {
        let args = self
            .assertion_arguments(file, call, view)
            .filter(|args| args.len() == 3)
            .ok_or("returning assertion argument correspondence is unsupported")?;
        for (position, (file, node)) in args.iter().enumerate() {
            let range = self.context.files[*file].location(node.range()).range;
            let kind = match position {
                0 => TypeKind::Bool,
                1 => TypeKind::String,
                _ => TypeKind::Int,
            };
            if !self.view(*file, node, view).expressions.iter().any(|e| {
                e.file == *file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == kind
                    && e.ty.instantiation == Instantiation::Parameter
            }) {
                return Err("returning assertion requires present parameter condition, message and integer value".into());
            }
        }
        Ok(args)
    }
    fn assertion_literal(&self, file: FileId, node: &SyntaxNode) -> Option<TokenKind> {
        let node = unwrap(node);
        (node.kind() == NodeKind::Expression)
            .then(|| {
                crate::domains::tokens(&self.context.files[file].parsed, node)
                    .first()
                    .map(|token| token.kind)
            })
            .flatten()
    }
    fn integer_conditional_values(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<&'a SyntaxNode>, String> {
        let mut values = Vec::new();
        let mut has_else = false;
        for branch in node.child_nodes() {
            let children: Vec<_> = branch.child_nodes().collect();
            let body = if branch.kind() == NodeKind::ConditionalBranch && children.len() == 2 {
                let range = self.context.files[file].location(children[0].range()).range;
                if !self
                    .view(file, children[0], view)
                    .expressions
                    .iter()
                    .any(|e| {
                        e.file == file
                            && e.location.range == range
                            && e.ty.known()
                            && !optional(&e.ty)
                            && e.ty.kind == TypeKind::Bool
                            && e.ty.instantiation == Instantiation::Parameter
                    })
                {
                    return Err("integer conditional guard is unsupported".into());
                }
                values.push(children[0]);
                children[1]
            } else if branch.kind() == NodeKind::ElseBranch && children.len() == 1 {
                has_else = true;
                children[0]
            } else {
                return Err("integer conditional branch form is unsupported".into());
            };
            let range = self.context.files[file].location(body.range()).range;
            if !self.view(file, body, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == TypeKind::Int
                    && e.ty.instantiation == Instantiation::Parameter
            }) {
                return Err("integer conditional body is unsupported".into());
            }
            values.push(body);
        }
        if !has_else || values.len() < 3 {
            return Err("integer conditional requires a complete else".into());
        }
        Ok(values)
    }
    fn asserted_integer_bounds(
        &self,
        file: FileId,
        operation: &'a SyntaxNode,
        operand: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let present_integer = |node: &SyntaxNode| {
            let range = self.context.files[file].location(node.range()).range;
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == TypeKind::Int
            })
        };
        let mut operand = unwrap(operand);
        if !present_integer(operand) {
            return false;
        }
        if operand.kind() == NodeKind::CallExpression && self.core(file, operand, view, "enum2int")
        {
            let children: Vec<_> = operand.child_nodes().collect();
            if children.len() != 1 || !present_integer(children[0]) {
                return false;
            }
            operand = unwrap(children[0]);
        }
        if operand.kind() != NodeKind::Expression {
            return false;
        }
        let Some(id) = self.reference(file, operand) else {
            return false;
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut pending = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = pending.pop() {
            if node.kind() == NodeKind::CallExpression
                && self.core(file, node, view, "assert")
                && let Ok(args) = self.integer_assertion_arguments(file, node, view)
                && args[2].0 == file
                && contains(args[2].1)
            {
                let guard = unwrap(args[0].1);
                let children: Vec<_> = guard.child_nodes().collect();
                if args[0].0 == file
                    && self.core(file, guard, view, "has_bounds")
                    && children.len() == 1
                    && present_integer(children[0])
                    && unwrap(children[0]).kind() == NodeKind::Expression
                    && self.reference(file, unwrap(children[0])) == Some(id)
                {
                    return true;
                }
            }
            pending.extend(node.child_nodes().filter(|child| contains(child)));
        }
        false
    }
    fn reflection_array_nonempty(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        if self.array_nonempty_branch(file, operation, array, view) {
            return true;
        }
        let TypeKind::Array { indices, .. } = &view.declarations[array.0].ty.kind else {
            return false;
        };
        if indices.len() != 1 {
            return false;
        }
        let identity = |node: &SyntaxNode| {
            let node = unwrap(node);
            if node.kind() != NodeKind::Expression {
                return None;
            }
            let id = self.reference(file, node)?;
            let ty = &self.view(file, node, view).declarations[id.0].ty;
            (ty.known()
                && !optional(ty)
                && matches!(&ty.kind,
                TypeKind::Array { indices: source, .. } if source.len() == 1
                    && source[0].kind == indices[0].kind
                    && source[0].instantiation == Instantiation::Parameter))
            .then_some(id)
        };
        let index_array = |node: &SyntaxNode| {
            let node = unwrap(node);
            let children: Vec<_> = node.child_nodes().collect();
            if !self.core(file, node, view, "index_set") || children.len() != 1 {
                return None;
            }
            identity(children[0])
        };
        let parameter_bool = |node: &SyntaxNode| {
            let range = self.context.files[file].location(node.range()).range;
            self.view(file, node, view).expressions.iter().any(|e| {
                e.file == file
                    && e.location.range == range
                    && e.ty.known()
                    && !optional(&e.ty)
                    && e.ty.kind == TypeKind::Bool
                    && e.ty.instantiation == Instantiation::Parameter
            })
        };
        let contains = |node: &SyntaxNode| {
            node.range().start <= operation.range().start
                && operation.range().end <= node.range().end
        };
        let mut nonempty = Vec::new();
        let mut aligned = Vec::new();
        let mut ancestors = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = ancestors.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && contains(children[1])
                && parameter_bool(unwrap(children[0]))
            {
                let mut guards = vec![unwrap(children[0])];
                while let Some(guard) = guards.pop() {
                    let parts: Vec<_> = guard.child_nodes().collect();
                    if parts.len() == 2 && self.core(file, guard, view, "/\\") {
                        guards.extend(parts.into_iter().map(unwrap));
                    } else if parts.len() == 2
                        && self.core(file, guard, view, ">=")
                        && parameter_bool(guard)
                        && crate::domains::invariant_expression_integer(
                            self.context,
                            self.bindings,
                            file,
                            parts[1],
                        ) == Ok(Some(1))
                    {
                        let length = unwrap(parts[0]);
                        let arguments: Vec<_> = length.child_nodes().collect();
                        if self.core(file, length, view, "length")
                            && arguments.len() == 1
                            && let Some(id) = identity(arguments[0])
                        {
                            nonempty.push(id);
                        }
                    }
                }
            }
            if children.len() == 2 && self.core(file, node, view, "/\\") {
                let mut siblings: Vec<_> = children
                    .iter()
                    .copied()
                    .filter(|child| !contains(child))
                    .map(unwrap)
                    .collect();
                while let Some(sibling) = siblings.pop() {
                    let parts: Vec<_> = sibling.child_nodes().collect();
                    if parts.len() == 2 && self.core(file, sibling, view, "/\\") {
                        siblings.extend(parts.into_iter().map(unwrap));
                        continue;
                    }
                    if !self.core(file, sibling, view, "assert") {
                        continue;
                    }
                    let Some(args) = self
                        .assertion_arguments(file, sibling, view)
                        .filter(|args| args.len() == 2 && args.iter().all(|(f, _)| *f == file))
                    else {
                        continue;
                    };
                    let message = unwrap(args[1].1);
                    let tokens = crate::domains::tokens(&self.context.files[file].parsed, message);
                    if !parameter_bool(unwrap(args[0].1))
                        || tokens.len() != 1
                        || tokens[0].kind != TokenKind::StringLiteral
                        || !self.view(file, message, view).expressions.iter().any(|e| {
                            e.file == file
                                && e.location.range
                                    == self.context.files[file].location(message.range()).range
                                && e.ty.known()
                                && !optional(&e.ty)
                                && e.ty.kind == TypeKind::String
                                && e.ty.instantiation == Instantiation::Parameter
                        })
                    {
                        continue;
                    }
                    let mut conditions = vec![unwrap(args[0].1)];
                    let mut pairs = Vec::new();
                    let mut supported = true;
                    while let Some(condition) = conditions.pop() {
                        let parts: Vec<_> = condition.child_nodes().collect();
                        if parts.len() == 2 && self.core(file, condition, view, "/\\") {
                            conditions.extend(parts.into_iter().map(unwrap));
                        } else if parts.len() == 2
                            && self.core(file, condition, view, "=")
                            && let (Some(left), Some(right)) =
                                (index_array(parts[0]), index_array(parts[1]))
                        {
                            pairs.push((left, right));
                        } else {
                            supported = false;
                            break;
                        }
                    }
                    // This bounded shape prevents assertion proof checking from
                    // recursively depending on another reflected value.
                    if supported
                        && args.iter().all(|(_, argument)| {
                            self.dependencies(file, argument, view, &[]).is_ok()
                        })
                    {
                        aligned.extend(pairs);
                    }
                }
            }
            ancestors.extend(children.into_iter().filter(|child| contains(child)));
        }
        nonempty.iter().any(|source| {
            *source == array
                || aligned.iter().any(|(left, right)| {
                    (*left == *source && *right == array) || (*right == *source && *left == array)
                })
        })
    }
    fn finite_array_branch(
        &self,
        file: FileId,
        operation: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let range = operation.range();
        let mut containing = vec![self.context.files[file].parsed.tree()];
        while let Some(node) = containing.pop() {
            let children: Vec<_> = node.child_nodes().collect();
            if node.kind() == NodeKind::ConditionalBranch
                && children.len() == 2
                && children[1].range().start <= range.start
                && range.end <= children[1].range().end
            {
                let guard = unwrap(children[0]);
                let guard_range = self.context.files[file].location(guard.range()).range;
                let parameter_bool = self.view(file, guard, view).expressions.iter().any(|e| {
                    e.file == file
                        && e.location.range == guard_range
                        && e.ty.kind == TypeKind::Bool
                        && !e.ty.optional
                        && e.ty.instantiation == Instantiation::Parameter
                });
                let list = guard
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList);
                let body = guard
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList);
                if parameter_bool
                    && guard.kind() == NodeKind::GeneratorCallExpression
                    && self.core(file, guard, view, "forall")
                    && let (Some(list), Some(body)) = (list, body)
                {
                    let generators: Vec<_> = list.child_nodes().collect();
                    if generators.len() == 1
                        && crate::domains::generator_slots(
                            &self.context.files[file].parsed,
                            generators[0],
                        ) == 1
                        && self.iterations(file, &generators, view).is_ok()
                        && self.core(file, unwrap(body), view, "has_bounds")
                    {
                        let args: Vec<_> = unwrap(body).child_nodes().collect();
                        if args.len() == 1 {
                            let access = unwrap(args[0]);
                            let parts: Vec<_> = access.child_nodes().collect();
                            if access.kind() == NodeKind::ArrayAccessExpression
                                && parts.len() == 2
                                && unwrap(parts[0]).kind() == NodeKind::Expression
                                && self.reference(file, unwrap(parts[0])) == Some(array)
                                && self.member_index(file, array, parts[1], &generators, view)
                            {
                                // A complete traversal proves each element bounded. An
                                // empty array supplies the finite empty domain union.
                                return true;
                            }
                        }
                    }
                }
            }
            containing.extend(children.into_iter().filter(|child| {
                child.range().start <= range.start && range.end <= child.range().end
            }));
        }
        false
    }
    // This inspection-only branch never changes strict dependencies or outputs.
    fn inspect_private_index_conditional<'b>(
        &'b self,
        inputs: (&Clause<'a>, &'b CallableFacts, &[CallableOutput]),
        unavailable: &mut Vec<UnavailableCallableDefinition>,
        active: &mut Vec<(FileId, std::ops::Range<usize>)>,
        mut invocation: Option<&mut Invocation<'a, 'b>>,
    ) -> Option<Result<(), String>> {
        let (clause, view, known) = inputs;
        if !matches!(clause.kind, ClauseKind::Conditional) {
            return None;
        }
        let file = clause.file;
        let owner = self.bindings.declarations.iter().find(|d| {
            d.file == file
                && d.item == clause.item
                && d.role == DeclarationRole::Predicate
                && d.syntax_range.start <= clause.node.range().start
                && clause.node.range().end <= d.syntax_range.end
        })?;
        let written = find_node(
            self.context.files[file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        )?;
        if written
            .child_nodes()
            .filter(|n| is_expression(n.kind()))
            .count()
            != 1
            || written
                .child_nodes()
                .find(|n| is_expression(n.kind()))
                .is_none_or(|body| unwrap(body).range() != clause.node.range())
            || !self.instances.iter().any(|i| {
                i.id == owner.id
                    && !i.recursive
                    && i.clauses
                        .iter()
                        .any(|c| c.file == file && c.node.range() == clause.node.range())
            })
        {
            return None;
        }
        let branches: Vec<_> = clause.node.child_nodes().collect();
        if branches.len() != 2
            || branches[0].kind() != NodeKind::ConditionalBranch
            || branches[1].kind() != NodeKind::ElseBranch
        {
            return None;
        }
        let first: Vec<_> = branches[0].child_nodes().collect();
        let second: Vec<_> = branches[1].child_nodes().collect();
        if first.len() != 2
            || second.len() != 1
            || self.assertion_literal(file, first[1]) != Some(TokenKind::True)
        {
            return None;
        }
        let guard = unwrap(first[0]);
        let guard_parts: Vec<_> = guard.child_nodes().collect();
        if guard.kind() != NodeKind::BinaryExpression
            || guard_parts.len() != 2
            || !self.core(file, guard, view, "=")
        {
            return None;
        }
        let length = unwrap(guard_parts[0]);
        let subject = unwrap(length_argument(
            self.context,
            self.bindings,
            self.view(file, length, view),
            file,
            length,
        )?);
        let source = self.reference(file, subject)?;
        if !self.length_equals(file, guard, source, 0, view)
            || unwrap(guard_parts[1]).kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, unwrap(guard_parts[1])).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            return None;
        }
        let source_owner = &self.bindings.declarations[source.0];
        if source_owner.file != file
            || source_owner.item != clause.item
            || source_owner.role != DeclarationRole::Parameter
        {
            return None;
        }
        let local_node = unwrap(second[0]);
        if local_node.kind() != NodeKind::LetExpression {
            return None;
        }
        let locals: Vec<_> = local_node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|n| n.child_nodes())
            .collect();
        if locals.len() != 4 || locals.iter().any(|n| n.kind() != NodeKind::Declaration) {
            return None;
        }
        let ids: Vec<_> = locals
            .iter()
            .map(|node| {
                self.bindings
                    .declarations
                    .iter()
                    .find(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Local
                            && d.syntax_range == node.range()
                    })
                    .map(|d| d.id)
            })
            .collect::<Option<Vec<_>>>()?;
        let private = [source, ids[0], ids[1], ids[2], ids[3]];
        let values: Vec<_> = locals[..3]
            .iter()
            .map(|node| node.child_nodes().find(|n| is_expression(n.kind())))
            .collect::<Option<Vec<_>>>()?;
        for (value, name, argument) in [
            (values[0], "index_set", source),
            (values[1], "min", ids[0]),
            (values[2], "card", ids[0]),
        ] {
            let value = unwrap(value);
            let args: Vec<_> = value.child_nodes().collect();
            if value.kind() != NodeKind::CallExpression
                || args.len() != 1
                || !self.core(file, value, view, name)
                || self.reference(file, unwrap(args[0])) != Some(argument)
            {
                return None;
            }
        }
        if locals[3].child_nodes().any(|n| is_expression(n.kind())) {
            return None;
        }
        let array_type = locals[3].child_nodes().next()?;
        let domains: Vec<_> = array_type.child_nodes().collect();
        if array_type.kind() != NodeKind::ArrayType
            || domains.len() != 2
            || domains.iter().any(|d| d.kind() != NodeKind::DomainType)
        {
            return None;
        }
        let axes: Vec<_> = domains[0].child_nodes().collect();
        let elements: Vec<_> = domains[1].child_nodes().collect();
        if axes.len() != 1
            || self.reference(file, unwrap(axes[0])) != Some(ids[0])
            || elements.len() != 1
            || unwrap(elements[0]).kind() != NodeKind::RangeExpression
        {
            return None;
        }
        let range = unwrap(elements[0]);
        let endpoints: Vec<_> = range.child_nodes().collect();
        if endpoints.len() != 2
            || !self.core(file, range, view, "..")
            || unwrap(endpoints[0]).kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, unwrap(endpoints[0])).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
            || !crate::domains::invariant_expression_integer(
                self.context,
                self.bindings,
                file,
                endpoints[0],
            )
            .is_ok_and(|value| value == Some(1))
            || self.reference(file, unwrap(endpoints[1])) != Some(ids[2])
        {
            return None;
        }
        let result: Vec<_> = local_node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::LetBlock)
            .collect();
        let [body] = result.as_slice() else {
            return None;
        };
        let mut pending = vec![*body];
        let mut relations = Vec::new();
        while let Some(node) = pending.pop() {
            let node = unwrap(node);
            if node.kind() == NodeKind::BinaryExpression && self.core(file, node, view, "/\\") {
                let parts: Vec<_> = node.child_nodes().collect();
                if parts.len() != 2
                    || !crate::definitions::annotations_safe(self.context, file, node)
                {
                    return Some(Err("private index conjunction is unsupported".into()));
                }
                pending.extend(parts.into_iter().rev());
            } else {
                relations.push(node);
            }
        }
        if relations.len() != 5
            || relations[..2]
                .iter()
                .any(|n| n.kind() != NodeKind::CallExpression)
            || relations[2].kind() != NodeKind::GeneratorCallExpression
            || relations[3].kind() != NodeKind::BinaryExpression
            || relations[4].kind() != NodeKind::GeneratorCallExpression
            || !self.core(file, relations[2], view, "forall")
            || !self.core(file, relations[3], view, "=")
            || !self.core(file, relations[4], view, "forall")
        {
            return None;
        }
        for (node, argument) in relations[..2].iter().zip([source, ids[3]]) {
            let args: Vec<_> = node.child_nodes().collect();
            let (id, parameters) = self.resolved(file, node, view)?;
            let declaration = &self.bindings.declarations[id.0];
            if declaration.name != "all_different"
                || self.context.files[declaration.file].kind != SourceKind::StandardLibrary
                || !self.instances.iter().any(|instance| {
                    instance.id == id && instance.parameters == parameters && !instance.recursive
                })
                || args.len() != 1
                || self.reference(file, unwrap(args[0])) != Some(argument)
            {
                return None;
            }
        }
        Some((|| {
            let typed = |node: &SyntaxNode| {
                self.expression_type(self.view(file, node, view), file, node)
                    .map(|expression| &expression.ty)
            };
            let integer = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Int;
            let parameter_integer =
                |ty: &TypeInst| integer(ty) && ty.instantiation == Instantiation::Parameter;
            let set = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element) if parameter_integer(element))
            };
            let array = |ty: &TypeInst| {
                ty.known()
                    && !optional(ty)
                    && ty.instantiation == Instantiation::Decision
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && parameter_integer(&indices[0]) && integer(element)
                        && element.instantiation == Instantiation::Decision)
            };
            let boolean = |ty: &TypeInst| ty.known() && !optional(ty) && ty.kind == TypeKind::Bool;
            if !array(&view.declarations[source.0].ty)
                || typed(subject) != Some(&view.declarations[source.0].ty)
                || !set(&view.declarations[ids[0].0].ty)
                || !parameter_integer(&view.declarations[ids[1].0].ty)
                || !parameter_integer(&view.declarations[ids[2].0].ty)
                || !array(&view.declarations[ids[3].0].ty)
                || typed(clause.node).is_none_or(|t| !boolean(t))
                || typed(local_node).is_none_or(|t| !boolean(t))
                || typed(guard)
                    .is_none_or(|t| !boolean(t) || t.instantiation != Instantiation::Parameter)
                || !crate::definitions::annotations_safe(self.context, file, written)
                || !crate::definitions::annotations_safe(self.context, file, clause.node)
            {
                return Err("private index owner type or annotation is unsupported".into());
            }
            let formal = find_node(
                self.context.files[file].parsed.tree(),
                &source_owner.syntax_range,
                source_owner.role,
            )
            .ok_or("private index source formal unavailable")?;
            self.type_dependencies(file, formal, view, &clause.generators)?;
            let mut annotations = vec![written];
            while let Some(node) = annotations.pop() {
                if !crate::definitions::annotations_safe(self.context, file, node) {
                    return Err("private index source annotation is unsupported".into());
                }
                annotations.extend(node.child_nodes());
            }
            for node in &relations[..2] {
                if typed(node).is_none_or(|t| !boolean(t) || t.instantiation != Instantiation::Decision)
                    || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && array(&parameters[0]) && set(&parameters[1])
                                && typed(node) == Some(return_type))) {
                    return Err("private index selected relation tuple is unsupported".into());
                }
            }
            for value in [
                length,
                guard,
                unwrap(values[0]),
                unwrap(values[1]),
                unwrap(values[2]),
                range,
            ] {
                let args: Vec<_> = value.child_nodes().collect();
                if !crate::definitions::annotations_safe(self.context, file, value)
                    || self.operation_fact(self.view(file, value, view), file, value).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == args.len() && typed(value) == Some(return_type)
                                && parameters.iter().zip(&args).all(|(formal, actual)| formal.known()
                                    && !optional(formal) && typed(actual).is_some_and(|t| crate::types::coerces(t, formal))))) {
                    return Err("private index operation signature is unsupported".into());
                }
            }
            if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                file,
                first[0],
                view,
                &clause.generators,
                &mut Vec::new(),
            ) {
                return Err(reason);
            }
            let local = Clause {
                file,
                item: clause.item,
                node: local_node,
                generators: clause.generators.clone(),
                kind: ClauseKind::Local,
            };
            self.inspect_uncertain_index_let(&local, view, Some(&private), &relations)
                .ok_or("private index local scope is unsupported")??;
            let incoming = invocation.as_ref().and_then(|state| state.reachable);
            let reachable = match invocation.as_ref() {
                Some(state) => match self.invocation_guard(file, first[0], view, state)? {
                    Some(true) => Some(false),
                    Some(false) => incoming,
                    None => {
                        if incoming == Some(false) {
                            Some(false)
                        } else {
                            None
                        }
                    }
                },
                None => None,
            };
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = reachable;
            }
            let before = unavailable.len();
            // Retain normal selected actual/default/body/cycle traversal. Only
            // its inspection errors survive; none of its certificates escape.
            for node in &relations[..2] {
                let call = Clause {
                    file,
                    item: clause.item,
                    node,
                    generators: clause.generators.clone(),
                    kind: ClauseKind::Call,
                };
                self.interpret_forwarded(
                    (&call, view, known),
                    &mut Vec::new(),
                    unavailable,
                    &mut Vec::new(),
                    active,
                    invocation.as_deref_mut(),
                );
            }
            if let Some(state) = invocation.as_deref_mut() {
                state.reachable = incoming;
            }
            if unavailable.len() != before {
                return Err(unavailable[before].reason.clone());
            }
            Ok(())
        })())
    }
    fn inspect_uncertain_index_let(
        &self,
        clause: &Clause<'a>,
        view: &CallableFacts,
        private: Option<&[DeclarationId; 5]>,
        relations: &[&'a SyntaxNode],
    ) -> Option<Result<(), String>> {
        // This branch inspects a selected callable body only. It cannot certify
        // model-root locals through the stricter whole-item acceptance gates.
        if !self.bindings.declarations.iter().any(|d| {
            d.file == clause.file
                && d.item == clause.item
                && matches!(
                    d.role,
                    DeclarationRole::Predicate | DeclarationRole::Function | DeclarationRole::Test
                )
                && d.syntax_range.start <= clause.node.range().start
                && clause.node.range().end <= d.syntax_range.end
        }) {
            return None;
        }
        let locals: Vec<_> = clause
            .node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|n| n.child_nodes())
            .collect();
        let requested = private.is_some()
            || locals.iter().any(|local| {
                // An uncertain extremum can be written directly in a private array
                // range, without a separate initialized integer local.
                let mut values: Vec<_> = local
                    .child_nodes()
                    .filter(|n| is_expression(n.kind()) || n.kind() == NodeKind::ArrayType)
                    .collect();
                while let Some(value) = values.pop() {
                    if parameter_index_extremum(
                        self.context,
                        self.bindings,
                        self.view(clause.file, value, view),
                        clause.file,
                        value,
                    )
                    .is_some()
                        && matches!(
                            self.direct_safety(clause.file, value, view, &clause.generators),
                            DefinitionSafety::Unknown(_)
                        )
                    {
                        return true;
                    }
                    values.extend(value.child_nodes());
                }
                false
            });
        if !requested && private.is_none() {
            return None;
        }
        Some((|| {
            if !crate::definitions::annotations_safe(self.context, clause.file, clause.node) {
                return Err("uncertain-bound let annotation is unsupported".into());
            }
            let local_ids: Vec<_> = self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == clause.file
                        && d.role == DeclarationRole::Local
                        && locals.iter().any(|local| local.range() == d.syntax_range)
                })
                .map(|d| d.id)
                .collect();
            let mut checked_locals = Vec::new();
            let no_forward_reference = |value: &SyntaxNode| {
                let range = self.context.files[clause.file]
                    .location(value.range())
                    .range;
                !self.bindings.references.iter().any(|r|
                    r.file == clause.file && range.start <= r.location.range.start && r.location.range.end <= range.end
                        && matches!(r.resolution, BindingResolution::Resolved(id)
                            if local_ids.contains(&id)
                                && self.bindings.declarations[id.0].syntax_range.end
                                    + self.context.files[clause.file].byte_offset > r.location.range.start))
            };
            for local in locals {
                if !crate::definitions::annotations_safe(self.context, clause.file, local) {
                    return Err("uncertain-bound local annotation is unsupported".into());
                }
                if local.kind() == NodeKind::Constraint {
                    for body in local.child_nodes().filter(|n| is_expression(n.kind())) {
                        self.inspect_uncertain_index_expression(
                            clause.file,
                            body,
                            view,
                            &clause.generators,
                            &checked_locals,
                            private,
                        )?;
                    }
                    continue;
                }
                let d = self
                    .bindings
                    .declarations
                    .iter()
                    .find(|d| {
                        d.file == clause.file
                            && d.role == DeclarationRole::Local
                            && d.syntax_range == local.range()
                    })
                    .filter(|_| local.kind() == NodeKind::Declaration)
                    .ok_or("uncertain-bound local declaration is unsupported")?;
                let ty = &view.declarations[d.id.0].ty;
                if !ty.known() || optional(ty) {
                    return Err("uncertain-bound local type is unsupported".into());
                }
                let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
                let boolean_array = matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Bool);
                let integer_array = ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(|index| index.known() && !optional(index)
                            && index.instantiation == Instantiation::Parameter && index.kind == TypeKind::Int)
                            && element.known() && !optional(element) && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Int);
                let private_integer = ty.instantiation == Instantiation::Decision
                    && ty.kind == TypeKind::Int
                    && initializer.is_none();
                let private_set = private.is_some_and(|ids| d.id == ids[1]);
                let private_array = private.is_some_and(|ids| d.id == ids[4]);
                if !(private_set
                    || private_array
                    || (ty.instantiation == Instantiation::Parameter
                        && ty.kind == TypeKind::Int
                        && initializer.is_some())
                    || (integer_array && initializer.is_some())
                    || private_integer
                    || (ty.instantiation == Instantiation::Decision
                        && initializer.is_none()
                        && (ty.kind == TypeKind::Bool || boolean_array)))
                {
                    return Err("uncertain-bound local declaration shape is unsupported".into());
                }
                let written = local
                    .child_nodes()
                    .next()
                    .ok_or("written local type unavailable")?;
                if private_integer
                    && (written.kind() != NodeKind::DomainType
                        || written.child_nodes().count() != 1
                        || written
                            .child_nodes()
                            .next()
                            .is_none_or(|value| unwrap(value).kind() != NodeKind::RangeExpression))
                {
                    return Err(
                        "uncertain-bound private integer requires a written parameter range".into(),
                    );
                }
                if boolean_array
                    && written.child_nodes().next().is_none_or(|index| {
                        index.kind() != NodeKind::DomainType
                            || index.child_nodes().next().is_none_or(|value| {
                                unwrap(value).kind() != NodeKind::RangeExpression
                            })
                    })
                {
                    return Err(
                        "uncertain-bound private Boolean array requires a written integer range"
                            .into(),
                    );
                }
                let mut types = vec![written];
                while let Some(node) = types.pop() {
                    if !crate::definitions::annotations_safe(self.context, clause.file, node) {
                        return Err("uncertain-bound written type annotation is unsupported".into());
                    }
                    if node.kind() == NodeKind::DomainType {
                        for value in node.child_nodes() {
                            if !no_forward_reference(value) {
                                return Err("uncertain-bound domain has a forward or cyclic local dependency".into());
                            }
                            self.inspect_uncertain_index_expression(
                                clause.file,
                                value,
                                view,
                                &clause.generators,
                                &checked_locals,
                                private,
                            )?;
                        }
                    } else {
                        types.extend(node.child_nodes());
                    }
                }
                if let Some(value) = initializer {
                    if !no_forward_reference(value) {
                        return Err(
                            "uncertain-bound initializer has a forward or cyclic local dependency"
                                .into(),
                        );
                    }
                    if integer_array {
                        self.inspect_cartesian_parameter_array(
                            clause.file,
                            written,
                            value,
                            view,
                            &clause.generators,
                            &checked_locals,
                        )?;
                    } else if let Some(ids) = private {
                        self.inspect_uncertain_index_expression(
                            clause.file,
                            value,
                            view,
                            &clause.generators,
                            &checked_locals,
                            Some(ids),
                        )?;
                    } else if let DefinitionSafety::Unsupported(reason) =
                        self.direct_safety(clause.file, value, view, &clause.generators)
                    {
                        return Err(reason);
                    }
                }
                checked_locals.push(d.id);
            }
            let bodies: Vec<_> = clause
                .node
                .child_nodes()
                .filter(|n| n.kind() != NodeKind::LetBlock)
                .collect();
            if bodies.len() != 1 {
                return Err("uncertain-bound let result is unsupported".into());
            }
            if let Some(ids) = private {
                for relation in relations
                    .iter()
                    .filter(|n| n.kind() != NodeKind::CallExpression)
                {
                    self.inspect_uncertain_index_expression(
                        clause.file,
                        relation,
                        view,
                        &clause.generators,
                        &checked_locals,
                        Some(ids),
                    )?;
                }
            } else {
                self.inspect_uncertain_index_expression(
                    clause.file,
                    bodies[0],
                    view,
                    &clause.generators,
                    &checked_locals,
                    None,
                )?;
            }
            Ok(())
        })())
    }
    fn inspect_cartesian_parameter_array(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        value: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        checked_locals: &[DeclarationId],
    ) -> Result<(), String> {
        let node = unwrap(value);
        let typed = |n: &SyntaxNode| {
            self.expression_type(self.view(file, n, view), file, n)
                .map(|e| &e.ty)
        };
        let integer = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if integer(element))
        };
        let array = |t: &TypeInst, rank| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == rank && indices.iter().all(integer) && integer(element))
        };
        let args: Vec<_> = node.child_nodes().collect();
        if written.kind() != NodeKind::ArrayType
            || written.child_nodes().count() != 3
            || node.kind() != NodeKind::CallExpression
            || !self.core(file, node, view, "array2d")
            || args.len() != 3
            || !crate::definitions::annotations_safe(self.context, file, value)
            || typed(node).is_none_or(|t| !array(t, 2))
            || !self
                .operation_fact(self.view(file, node, view), file, node)
                .is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 3 && set(&parameters[0]) && set(&parameters[1])
                        && array(&parameters[2], 1) && array(return_type, 2))
                })
        {
            return Err(
                "uncertain-bound parameter array requires the core rank-two integer array2d".into(),
            );
        }
        let range_ids = |value: &SyntaxNode| {
            let node = unwrap(value);
            let ends: Vec<_> = node.child_nodes().collect();
            if node.kind() != NodeKind::RangeExpression
                || ends.len() != 2
                || !self.core(file, node, view, "..")
                || typed(node).is_none_or(|t| !set(t))
                || !crate::definitions::annotations_safe(self.context, file, value)
            {
                return None;
            }
            let mut ids = Vec::new();
            for end in ends {
                let end = unwrap(end);
                if end.kind() != NodeKind::Expression
                    || typed(end).is_none_or(|t| !integer(t))
                    || !crate::definitions::annotations_safe(self.context, file, end)
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, end).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                {
                    return None;
                }
                let id = self.reference(file, end)?;
                if !checked_locals.contains(&id) {
                    return None;
                }
                ids.push(id);
            }
            Some(ids)
        };
        let axes: Vec<_> = written.child_nodes().take(2).collect();
        let collection = unwrap(args[2]);
        let parts: Vec<_> = collection.child_nodes().collect();
        if collection.kind() != NodeKind::ArrayComprehension
            || typed(collection).is_none_or(|t| !array(t, 1))
            || parts.len() != 2
            || parts[1].kind() != NodeKind::GeneratorList
        {
            return Err(
                "uncertain-bound parameter array requires a Cartesian comprehension".into(),
            );
        }
        let headers: Vec<_> = parts[1].child_nodes().collect();
        if headers.len() != 2 {
            return Err("uncertain-bound parameter array requires two Cartesian headers".into());
        }
        for ((axis, argument), header) in axes.iter().zip(&args[..2]).zip(headers) {
            let written: Vec<_> = axis.child_nodes().collect();
            let source: Vec<_> = header.child_nodes().collect();
            let tokens = crate::domains::tokens(&self.context.files[file].parsed, header);
            if axis.kind() != NodeKind::DomainType
                || written.len() != 1
                || source.len() != 1
                || crate::domains::generator_slots(&self.context.files[file].parsed, header) != 1
                || !matches!(
                    tokens.first().map(|t| t.kind),
                    Some(TokenKind::Identifier | TokenKind::QuotedIdentifier)
                )
                || tokens.get(1).is_none_or(|t| t.kind != TokenKind::In)
                || range_ids(written[0]).is_none()
                || range_ids(written[0]) != range_ids(argument)
                || range_ids(argument) != range_ids(source[0])
            {
                return Err(
                    "uncertain-bound parameter array axes and Cartesian headers do not match"
                        .into(),
                );
            }
        }
        // Inspect every header and body even when an axis might be empty. Unknown
        // cardinality or membership supplies no strict dependency or output proof.
        match self.collection_construction_safety(file, collection, view, generators) {
            DefinitionSafety::Unsupported(reason) => Err(reason),
            _ => Ok(()),
        }
    }
    fn inspect_uncertain_index_expression(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        checked_locals: &[DeclarationId],
        private: Option<&[DeclarationId; 5]>,
    ) -> Result<(), String> {
        let node = unwrap(node);
        if !crate::definitions::annotations_safe(self.context, file, node) {
            return Err("uncertain-bound expression annotation is unsupported".into());
        }
        let typed = |n: &SyntaxNode| {
            let range = self.context.files[file].location(n.range()).range;
            self.view(file, n, view)
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        let ty = typed(node).ok_or("uncertain-bound expression type unavailable")?;
        if !ty.known() || optional(ty) {
            return Err("uncertain-bound expression type or optionality is unsupported".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        if let Some(ids) = private {
            let integer = |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
            let parameter_integer =
                |t: &TypeInst| integer(t) && t.instantiation == Instantiation::Parameter;
            if node.kind() == NodeKind::Expression {
                if let Some(id) = self.reference(file, node)
                    && checked_locals.contains(&id)
                    && [ids[1], ids[2], ids[3]].contains(&id)
                {
                    return if ty == &view.declarations[id.0].ty {
                        Ok(())
                    } else {
                        Err("private index alias type is unsupported".into())
                    };
                }
                if ty.kind == TypeKind::Int {
                    return match self.initialized_source_safety(
                        file,
                        node,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    };
                }
            }
            if node.kind() == NodeKind::CallExpression && children.len() == 1 {
                if self.core(file, node, view, "index_set")
                    && self.reference(file, unwrap(children[0])) == Some(ids[0])
                    && ty.instantiation == Instantiation::Parameter
                    && matches!(&ty.kind, TypeKind::Set(element) if parameter_integer(element))
                {
                    return match self.initialized_source_safety(
                        file,
                        children[0],
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => Err(reason),
                        _ => Ok(()),
                    };
                }
                if parameter_integer(ty)
                    && self.reference(file, unwrap(children[0])) == Some(ids[1])
                    && checked_locals.contains(&ids[1])
                {
                    if self.core(file, node, view, "min") {
                        return match self
                            .parameter_set_extremum_safety(file, node, view, generators)
                        {
                            Some(DefinitionSafety::Unsupported(reason)) => Err(reason),
                            Some(_) => Ok(()),
                            None => Err("private index extremum signature is unsupported".into()),
                        };
                    }
                    if self.core(file, node, view, "card")
                        && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call|
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 1 && typed(children[0]) == Some(&parameters[0])
                                    && return_type == ty)) {
                        return Ok(());
                    }
                }
            }
            if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 && integer(ty)
            {
                let subject = unwrap(children[0]);
                let array = self.reference(file, subject);
                if !matches!(array, Some(id) if id == ids[0] || id == ids[4] && checked_locals.contains(&id))
                    || typed(subject).is_none_or(|t| !t.known() || optional(t)
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && parameter_integer(&indices[0]) && integer(element)
                                && element.instantiation == Instantiation::Decision))
                    || typed(children[1]).is_none_or(|t| !integer(t)) {
                    return Err("private integer selection type or scope is unsupported".into());
                }
                self.inspect_uncertain_index_expression(
                    file,
                    children[1],
                    view,
                    generators,
                    checked_locals,
                    private,
                )?;
                if let Some(reason) =
                    self.closed_integer_source_error(file, children[1], false, false)
                {
                    return Err(reason);
                }
                if let Some(array) = array
                    && let Domain::Array { indices, .. } = crate::domains::bare_index_domain(
                        &self.domains.declarations[array.0].domain,
                    )
                    && crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        children[1],
                    )
                    .is_ok_and(|value| {
                        value.is_some_and(|value| {
                            crate::domains::index_domain_member(&indices[0], value) == Some(false)
                        })
                    })
                {
                    return Err("private integer selection is outside its declared axis".into());
                }
                // The declaration/actual source and every selector were inspected.
                // This is neither a membership nor an output dependency proof.
                return Ok(());
            }
            if node.kind() == NodeKind::ConditionalExpression && integer(ty) {
                let mut complete = false;
                for (position, branch) in children.iter().enumerate() {
                    if !crate::definitions::annotations_safe(self.context, file, branch) {
                        return Err("private integer conditional annotation is unsupported".into());
                    }
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                        if typed(parts[0])
                            .is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
                        {
                            return Err("private integer conditional guard is unsupported".into());
                        }
                        self.inspect_uncertain_index_expression(
                            file,
                            parts[0],
                            view,
                            generators,
                            checked_locals,
                            private,
                        )?;
                        parts[1]
                    } else if branch.kind() == NodeKind::ElseBranch
                        && parts.len() == 1
                        && position + 1 == children.len()
                    {
                        complete = true;
                        parts[0]
                    } else {
                        return Err("private integer conditional branch is unsupported".into());
                    };
                    if typed(body).is_none_or(|t| !integer(t) || !crate::types::coerces(t, ty)) {
                        return Err("private integer conditional body is unsupported".into());
                    }
                    self.inspect_uncertain_index_expression(
                        file,
                        body,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                }
                return if complete {
                    Ok(())
                } else {
                    Err("private integer conditional requires an else".into())
                };
            }
            if node.kind() == NodeKind::BinaryExpression && integer(ty) && children.len() == 2 {
                let name =
                    operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
                if matches!(name, Some("+" | "-")) && self.core(file, node, view, name.unwrap())
                    && children.iter().all(|n| typed(n).is_some_and(integer))
                    && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call|
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && return_type == ty && parameters.iter().zip(&children)
                                .all(|(formal, actual)| integer(formal) && typed(actual).is_some_and(|t| crate::types::coerces(t, formal))))) {
                    for child in &children {
                        self.inspect_uncertain_index_expression(file, child, view, generators, checked_locals, private)?;
                    }
                    if let Some(reason) = self.closed_integer_source_error(file, node, false, false) { return Err(reason); }
                    return Ok(());
                }
            }
            if integer(ty) {
                return match self.initialized_source_safety(
                    file,
                    node,
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    DefinitionSafety::Unsupported(reason) => Err(reason),
                    _ => Ok(()),
                };
            }
        }
        if node.kind() == NodeKind::RangeExpression
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
            && children.len() == 2
            && self.core(file, node, view, "..")
            && children.iter().all(|child| {
                typed(child).is_some_and(|t| {
                    t.known()
                        && !optional(t)
                        && t.instantiation == Instantiation::Parameter
                        && t.kind == TypeKind::Int
                })
            })
        {
            if private.is_some() {
                for child in &children {
                    self.inspect_uncertain_index_expression(
                        file,
                        child,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                }
                return Ok(());
            }
            return match self.direct_children_safety(file, &children, view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if node.kind() == NodeKind::CallExpression
            && self.core(file, node, view, "index_set")
            && children.len() == 1
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
        {
            let subject = unwrap(children[0]);
            let declaration = self
                .reference(file, subject)
                .map(|id| &self.bindings.declarations[id.0]);
            if subject.kind() != NodeKind::Expression
                || declaration.is_none_or(|d| {
                    d.file != file
                        || !matches!(d.role, DeclarationRole::Parameter | DeclarationRole::Local)
                })
                || typed(subject).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter
                            && element.kind == TypeKind::Bool)
                })
            {
                return Err("uncertain-bound index-set subject is unsupported".into());
            }
            self.dependencies(file, subject, view, generators)?;
            return Ok(());
        }
        if node.kind() == NodeKind::ArrayAccessExpression
            && children.len() == 3
            && ty.kind == TypeKind::Int
        {
            let subject = unwrap(children[0]);
            let array = self
                .reference(file, subject)
                .filter(|id| checked_locals.contains(id))
                .ok_or("uncertain-bound parameter array has not been inspected")?;
            if subject.kind() != NodeKind::Expression || typed(subject).is_none_or(|t|
                !t.known() || optional(t) || t.instantiation != Instantiation::Parameter
                    || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(|index| index.known() && !optional(index)
                            && index.instantiation == Instantiation::Parameter && index.kind == TypeKind::Int)
                            && element.known() && !optional(element) && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Int))
                || self.bindings.declarations[array.0].role != DeclarationRole::Local
                || children.iter().any(|n| unwrap(n).kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, unwrap(n)).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
            { return Err("uncertain-bound parameter array selection is unsupported".into()); }
            let binder = self
                .reference(file, unwrap(children[1]))
                .ok_or("uncertain-bound selector identity unavailable")?;
            let choice = self
                .reference(file, unwrap(children[2]))
                .filter(|id| checked_locals.contains(id))
                .ok_or("uncertain-bound private selector has not been inspected")?;
            if self.bindings.declarations[binder.0].role != DeclarationRole::Generator
                || !generators
                    .iter()
                    .any(|g| g.range() == self.bindings.declarations[binder.0].syntax_range)
                || self.bindings.declarations[choice.0].role != DeclarationRole::Local
                || typed(children[1]).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || t.instantiation != Instantiation::Parameter
                        || t.kind != TypeKind::Int
                })
                || typed(children[2]).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || t.instantiation != Instantiation::Decision
                        || t.kind != TypeKind::Int
                })
            {
                return Err("uncertain-bound parameter array selectors are unsupported".into());
            }
            // The initializer and private domain were checked in declaration order.
            // Inspect both selectors without reopening that partial initializer.
            return match self.direct_children_safety(file, &children[1..], view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if ty.kind != TypeKind::Bool {
            return Err("uncertain-bound body requires a present Boolean".into());
        }
        if self
            .boolean_relation_dependencies(file, node, view, generators)
            .is_ok()
        {
            return Ok(());
        }
        if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 {
            let subject = unwrap(children[0]);
            if subject.kind() != NodeKind::Expression || self.reference(file, subject).is_none()
                || typed(subject).is_none_or(|t| !t.known() || optional(t)
                    || !matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].kind == TypeKind::Int
                            && indices[0].instantiation == Instantiation::Parameter && element.kind == TypeKind::Bool))
                || typed(children[1]).is_none_or(|t| !t.known() || optional(t)
                    || t.instantiation != Instantiation::Parameter || t.kind != TypeKind::Int)
            { return Err("uncertain-bound Boolean selection type is unsupported".into()); }
            self.dependencies(file, subject, view, generators)?;
            return match self.direct_safety(file, children[1], view, generators) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            };
        }
        if node.kind() == NodeKind::GeneratorCallExpression && self.core(file, node, view, "forall")
        {
            let list = children
                .iter()
                .find(|n| n.kind() == NodeKind::GeneratorList)
                .ok_or("uncertain-bound forall generators unavailable")?;
            let mut all = generators.to_vec();
            for header in list.child_nodes() {
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if binders.len() != 1 || header.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                    || !header.children().iter().any(|c| matches!(c, SyntaxElement::Token(i) if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In))
                    || !crate::definitions::annotations_safe(self.context, file, header)
                    || !view.declarations[binders[0].id.0].ty.known()
                    || view.declarations[binders[0].id.0].ty.kind != TypeKind::Int
                    || view.declarations[binders[0].id.0].ty.instantiation != Instantiation::Parameter
                    || optional(&view.declarations[binders[0].id.0].ty)
                { return Err("uncertain-bound forall binder, annotation or filter is unsupported".into()); }
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("uncertain-bound forall source unavailable")?;
                self.inspect_uncertain_index_expression(
                    file,
                    source,
                    view,
                    &all,
                    checked_locals,
                    private,
                )?;
                all.push(header);
            }
            let body = children
                .iter()
                .find(|n| n.kind() != NodeKind::GeneratorList)
                .ok_or("uncertain-bound forall body unavailable")?;
            return self.inspect_uncertain_index_expression(
                file,
                body,
                view,
                &all,
                checked_locals,
                private,
            );
        }
        let name = operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
        let supported = match node.kind() {
            NodeKind::BinaryExpression => {
                children.len() == 2
                    && matches!(
                        name,
                        Some(
                            "=" | "!="
                                | "<"
                                | "<="
                                | ">"
                                | ">="
                                | "/\\"
                                | "\\/"
                                | "<->"
                                | "->"
                                | "<-"
                        )
                    )
            }
            NodeKind::UnaryExpression => children.len() == 1 && name == Some("not"),
            _ => false,
        };
        if !supported || !self.core(file, node, view, name.unwrap()) {
            return Err("uncertain-bound Boolean operation identity is unsupported".into());
        }
        if node.kind() == NodeKind::BinaryExpression
            && matches!(name, Some("=" | "!=" | "<" | "<=" | ">" | ">="))
            && children.iter().all(|child| {
                typed(child).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            })
        {
            // A supported partial numeric operand can collapse to false at this
            // core Boolean relation; it supplies no value or output guarantee.
            for child in children {
                let value = unwrap(child);
                if private.is_some()
                    || (value.kind() == NodeKind::ArrayAccessExpression
                        && value.child_nodes().count() == 3
                        && value
                            .child_nodes()
                            .next()
                            .and_then(|subject| self.reference(file, unwrap(subject)))
                            .is_some_and(|id| checked_locals.contains(&id)))
                {
                    self.inspect_uncertain_index_expression(
                        file,
                        value,
                        view,
                        generators,
                        checked_locals,
                        private,
                    )?;
                } else if let DefinitionSafety::Unsupported(reason) =
                    self.direct_safety(file, child, view, generators)
                {
                    return Err(reason);
                }
            }
            return Ok(());
        }
        for child in children {
            self.inspect_uncertain_index_expression(
                file,
                child,
                view,
                generators,
                checked_locals,
                private,
            )?;
        }
        Ok(())
    }
    fn type_dependencies(
        &self,
        file: FileId,
        declaration: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let mut types = vec![
            declaration
                .child_nodes()
                .next()
                .ok_or("written local type unavailable")?,
        ];
        let mut ids = Vec::new();
        while let Some(node) = types.pop() {
            if node.kind() == NodeKind::DomainType {
                for expression in node.child_nodes() {
                    extend(
                        &mut ids,
                        self.dependencies(file, expression, view, generators)?,
                    );
                }
            } else {
                types.extend(node.child_nodes());
            }
        }
        Ok(ids)
    }
    fn parameter_array_source(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Option<DeclarationId> {
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let ty = |node: &SyntaxNode| {
            let range = self.context.files[file].location(node.range()).range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        let children: Vec<_> = node.child_nodes().collect();
        if !self.core(file, node, view, "arrayXd")
            || children.len() != 2
            || children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || ty(node).is_none_or(|t| !parameter_integers(t))
            || children
                .iter()
                .any(|n| ty(n).is_none_or(|t| !parameter_integers(t)))
            || unwrap(children[0]).kind() != NodeKind::Expression
        {
            return None;
        }
        let source = self.reference(file, unwrap(children[0]))?;
        let source_type = &facts.declarations[source.0].ty;
        let TypeKind::Array { indices, .. } = &source_type.kind else {
            return None;
        };
        if !parameter_integers(source_type)
            || !matches!(&ty(node)?.kind, TypeKind::Array { indices: result, .. } if result == indices)
        {
            return None;
        }
        let comprehension = unwrap(children[1]);
        if comprehension.kind() != NodeKind::ArrayComprehension {
            return None;
        }
        let list = comprehension
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)?;
        let generators: Vec<_> = list.child_nodes().collect();
        if generators.len() != 1
            || generators[0]
                .child_nodes()
                .any(|n| n.kind() == NodeKind::WhereFilter)
            || self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == generators[0].range()
                })
                .count()
                != 1
        {
            return None;
        }
        let domain = unwrap(generators[0].child_nodes().next()?);
        (domain.kind() == NodeKind::Expression && self.reference(file, domain) == Some(source))
            .then_some(source)
    }
    fn integer_conversion_fallback(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Result<&'a SyntaxNode, String> {
        let parts: Vec<_> = node.child_nodes().map(unwrap).collect();
        let ty = |node: &SyntaxNode| {
            self.view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range == self.context.files[file].location(node.range()).range
                })
                .map(|e| &e.ty)
        };
        let present_int =
            |node| ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int);
        if parts.len() != 2 || parts.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || !self.core(file, node, view, "to_enum_internal")
            || !present_int(node) || ty(node).is_none_or(|t| t.instantiation != Instantiation::Parameter)
            || !present_int(parts[1]) || ty(parts[1]).is_none_or(|t| t.instantiation != Instantiation::Parameter)
            || parts[0].kind() != NodeKind::CallExpression
            || !self.core(file, parts[0], view, "enum_of")
            || ty(parts[0]).is_none_or(|t| !t.known() || optional(t) || t.instantiation != Instantiation::Parameter
                || !matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element) && element.kind == TypeKind::Int))
        {
            return Err("integer internal conversion requires a checked present Int composition".into());
        }
        let witness: Vec<_> = parts[0].child_nodes().map(unwrap).collect();
        if witness.len() != 1 || witness[0].kind() == NodeKind::NamedArgument || !present_int(witness[0])
            || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(|t| t.known() && !optional(t))
                        && matches!(&parameters[0].kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
                        && parameters[1].kind == TypeKind::Int && parameters[1].instantiation == Instantiation::Parameter
                        && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Int)
            })
            || self.operation_fact(self.view(file, parts[0], view), file, parts[0]).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && parameters[0].kind == TypeKind::Int)
            })
        {
            return Err("integer conversion witness or selected signature is unsupported".into());
        }
        // MiniZinc erases this entire Int composition to its second argument.
        // The witness is checked for type selection; only the fallback is evaluated.
        Ok(parts[1])
    }
    fn default_collection(&self, file: FileId, node: &SyntaxNode) -> bool {
        let node = unwrap(node);
        node.kind() == NodeKind::ArrayComprehension
            && node
                .child_nodes()
                .find(|n| n.kind() != NodeKind::GeneratorList)
                .is_some_and(|body| {
                    operator(self.context, file, unwrap(body)) == Some(TokenKind::Default)
                })
    }
    fn default_collection_arguments(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(Vec<DeclarationId>, &'a SyntaxNode, Vec<&'a SyntaxNode>), String> {
        let node = unwrap(node);
        let ty = |node: &SyntaxNode| {
            self.view(file, node, view)
                .expressions
                .iter()
                .find(|e| {
                    e.file == file
                        && e.location.range == self.context.files[file].location(node.range()).range
                })
                .map(|e| &e.ty)
        };
        let present_array = |t: &TypeInst| {
            t.known()
                && !t.optional
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                    && element.known() && !optional(element) && element.kind == TypeKind::Int)
        };
        if !self.default_collection(file, node) || ty(node).is_none_or(|t| !present_array(t)) {
            return Err("default collection requires a present rank-one integer result".into());
        }
        let list = node
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
            .ok_or("default generator is unavailable")?;
        let headers: Vec<_> = list.child_nodes().collect();
        if headers.len() != 1
            || headers[0]
                .child_nodes()
                .any(|n| n.kind() == NodeKind::WhereFilter)
            || !headers[0].children().iter().any(|c| {
                matches!(c, SyntaxElement::Token(i)
                if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
            })
        {
            return Err("default collection requires one unfiltered in generator".into());
        }
        let source = headers[0]
            .child_nodes()
            .next()
            .map(unwrap)
            .ok_or("default source is unavailable")?;
        let source_id = self
            .reference(file, source)
            .filter(|_| source.kind() == NodeKind::Expression)
            .ok_or("default collection source must be a bare array identity")?;
        let source_type = ty(source).ok_or("default source type is unavailable")?;
        if !source_type.known()
            || source_type.optional
            || !matches!(&source_type.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                    && element.known() && element.kind == TypeKind::Int)
        {
            return Err(
                "default collection source must retain known integer storage and axis types".into(),
            );
        }
        let declaration = &self.bindings.declarations[source_id.0];
        if !matches!(
            declaration.role,
            DeclarationRole::Value | DeclarationRole::Parameter
        ) {
            return Err("default collection source scope is unsupported".into());
        }
        let written = find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("default array declaration is unavailable")?;
        if !crate::definitions::annotations_safe(self.context, declaration.file, written) {
            return Err("default array source annotations are unsupported".into());
        }
        let mut ids = self.type_dependencies(
            declaration.file,
            written,
            view,
            if declaration.file == file {
                generators
            } else {
                &[]
            },
        )?;
        extend(&mut ids, vec![source_id]);
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::GeneratorList)
            .map(unwrap)
            .ok_or("default body is unavailable")?;
        let parts: Vec<_> = body.child_nodes().map(unwrap).collect();
        let binders: Vec<_> = self
            .bindings
            .declarations
            .iter()
            .filter(|d| {
                d.file == file
                    && d.role == DeclarationRole::Generator
                    && d.syntax_range == headers[0].range()
            })
            .collect();
        if parts.len() != 2 || binders.len() != 1 || parts[0].kind() != NodeKind::Expression
            || self.reference(file, parts[0]) != Some(binders[0].id)
            || !self.core(file, body, view, "default")
            || ty(parts[0]).is_none_or(|t| !t.known() || t.kind != TypeKind::Int)
            || ty(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
            || ty(parts[1]).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int
                || t.instantiation != Instantiation::Parameter)
            || self.operation_fact(self.view(file, body, view), file, body).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(|t| t.known() && t.kind == TypeKind::Int)
                        && !optional(&parameters[1]) && return_type.known() && !optional(return_type) && return_type.kind == TypeKind::Int)
            })
        {
            return Err("default collection body, binder identity or selected signature is unsupported".into());
        }
        let mut all = generators.to_vec();
        all.extend(headers);
        Ok((ids, parts[1], all))
    }
    fn parameter_test_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> Option<DefinitionSafety> {
        let written_call = node;
        let node = unwrap(node);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let facts = self.view(file, node, view);
        let CallOutcome::Resolved {
            declaration: id,
            parameters,
            return_type,
        } = &self.operation_fact(facts, file, node)?.outcome
        else {
            return None;
        };
        let declaration = &self.bindings.declarations[id.0];
        if declaration.role != DeclarationRole::Test
            || !declaration.top_level
            || self.context.files[declaration.file].kind != SourceKind::User
        {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let parameter = |t: &TypeInst, kind: TypeKind| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == kind
            };
            let signature = self
                .calls
                .signatures
                .iter()
                .find(|s| s.declaration == *id)
                .ok_or("parameter test signature unavailable")?;
            if active.contains(id)
                || !self.context.files[declaration.file]
                    .parsed
                    .diagnostics()
                    .is_empty()
                || parameters.len() != 1
                || signature.parameters.len() != 1
                || signature.parameters[0].has_default
                || !parameter(&signature.parameters[0].ty, TypeKind::Int)
                || signature.parameters[0].ty != parameters[0]
                || !parameter(&signature.return_type, TypeKind::Bool)
                || &signature.return_type != return_type
                || self
                    .expression_type(facts, file, node)
                    .is_none_or(|e| &e.ty != return_type)
            {
                return Err(
                    "parameter test specialization, default or cycle is unsupported".into(),
                );
            }
            let written = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("parameter test declaration unavailable")?;
            let formal = formal_parameter(self.context, self.bindings, *id, 0)
                .ok_or("parameter test formal unavailable")?;
            let formal_declaration = &self.bindings.declarations[formal.0];
            let formal_node = find_node(
                self.context.files[formal_declaration.file].parsed.tree(),
                &formal_declaration.syntax_range,
                formal_declaration.role,
            )
            .ok_or("parameter test written formal unavailable")?;
            let (actual_file, actual) =
                call_argument(self.context, self.bindings, facts, file, node, *id, 0)
                    .ok_or("parameter test actual unavailable")?;
            let body = written
                .child_nodes()
                .find(|n| is_expression(n.kind()))
                .ok_or("parameter test body unavailable")?;
            if node.child_nodes().count() != 1
                || self
                    .expression_type(self.view(actual_file, actual, view), actual_file, actual)
                    .is_none_or(|e| !parameter(&e.ty, TypeKind::Int))
                || self
                    .expression_type(self.calls, declaration.file, body)
                    .is_none_or(|e| !parameter(&e.ty, TypeKind::Bool))
            {
                return Err("parameter test actual or body type is unsupported".into());
            }
            let mut nodes = vec![written];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, declaration.file, current) {
                    return Err("parameter test annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut nodes = vec![written_call];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, current) {
                    return Err("parameter test call annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            active.push(*id);
            let inspected = (|| -> Result<(), String> {
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(actual_file, actual, view, generators, active)
                {
                    return Err(reason);
                }
                let mut types: Vec<_> = formal_node.child_nodes().take(1).collect();
                while let Some(ty) = types.pop() {
                    if ty.kind() == NodeKind::DomainType {
                        for domain in ty.child_nodes() {
                            if let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    declaration.file,
                                    domain,
                                    self.calls,
                                    &[],
                                    active,
                                )
                            {
                                return Err(reason);
                            }
                        }
                    } else {
                        types.extend(ty.child_nodes());
                    }
                }
                match self.initialized_source_safety(
                    declaration.file,
                    body,
                    self.calls,
                    &[],
                    active,
                ) {
                    DefinitionSafety::Unsupported(reason) => Err(reason),
                    _ => Ok(()),
                }
            })();
            active.pop();
            inspected
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown("parameter test value is unproved".into()),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Only an actual in-header can delegate its selected set source. The typed
    // helper inspects its complete initializer and returns no membership facts.
    fn selected_generator_source_safety(
        &self,
        file: FileId,
        header: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        if self.selected_set_source
            || header.kind() != NodeKind::Generator
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
        )
        .map(|checked| match checked {
            unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
            _ => DefinitionSafety::Unknown("selected parameter set membership is unproved".into()),
        })
    }
    fn source_annotations_safe(&self, file: FileId, node: &SyntaxNode) -> bool {
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
    // Inspection only: a bare source dependency is not proof that its written
    // initializer, domains and transitive initialized sources are supported.
    fn initialized_source_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        active: &mut Vec<DeclarationId>,
    ) -> DefinitionSafety {
        let mut safety = DefinitionSafety::Supported;
        let mut nodes = vec![node];
        while let Some(current) = nodes.pop() {
            if !self.source_annotations_safe(file, current) {
                return DefinitionSafety::Unsupported(
                    "initialized source annotation is unsupported".into(),
                );
            }
            if let Some(checked) =
                self.parameter_test_safety(file, current, view, generators, active)
            {
                match checked {
                    unsupported @ DefinitionSafety::Unsupported(_) => return unsupported,
                    unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                    DefinitionSafety::Supported => {}
                }
            }
            if !self.selected_set_source
                && current.kind() == NodeKind::Generator
                && current
                    .child_nodes()
                    .next()
                    .is_some_and(|source| unwrap(source).kind() == NodeKind::ArrayAccessExpression)
                && let Some(scope) = generator_source_scope(node, current, generators)
                && let Some(checked) =
                    self.selected_generator_source_safety(file, current, view, &scope)
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
            let Some(id) = self.reference(file, current) else {
                continue;
            };
            let declaration = &self.bindings.declarations[id.0];
            if !declaration.top_level
                || !matches!(
                    declaration.role,
                    DeclarationRole::Value | DeclarationRole::Enum
                )
            {
                continue;
            }
            if active.contains(&id) {
                return DefinitionSafety::Unsupported("cyclic initialized source".into());
            }
            let ty = &self.calls.declarations[id.0].ty;
            if !ty.known() || optional(ty) {
                return DefinitionSafety::Unsupported(
                    "initialized source type or optionality is unsupported".into(),
                );
            }
            let Some(written) = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return DefinitionSafety::Unsupported(
                    "initialized source declaration unavailable".into(),
                );
            };
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
                    if !self.source_annotations_safe(declaration.file, part) {
                        unsupported = Some("enum source annotation is unsupported".into());
                    }
                    if part.kind() == NodeKind::EnumConstructor {
                        let tokens = crate::domains::tokens(
                            &self.context.files[declaration.file].parsed,
                            part,
                        );
                        let name = tokens.first().map(|token| {
                            self.context.files[declaration.file].parsed.source()
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
                            .expression_type(self.calls, declaration.file, count)
                            .is_none_or(|e| {
                                !e.ty.known()
                                    || optional(&e.ty)
                                    || e.ty.instantiation != Instantiation::Parameter
                                    || e.ty.kind != TypeKind::Int
                            })
                        {
                            unsupported = Some("anonymous enum count type is unsupported".into());
                        }
                        if let Some(reason) =
                            self.closed_integer_source_error(declaration.file, count, false, false)
                        {
                            unsupported = Some(reason);
                        }
                        if crate::domains::invariant_expression_integer(
                            self.context,
                            self.bindings,
                            declaration.file,
                            count,
                        )
                        .is_ok_and(|value| value.is_some_and(|value| value < 0))
                        {
                            unsupported = Some("anonymous enum count is negative".into());
                        }
                        expressions.push(count);
                        safety =
                            DefinitionSafety::Unknown("enum constructor extent is unproved".into());
                    } else {
                        parts.extend(part.child_nodes());
                    }
                }
            }
            if !self.source_annotations_safe(declaration.file, written) {
                unsupported =
                    Some("initialized source declaration annotation is unsupported".to_owned());
            }
            while let Some(ty) = types.pop() {
                if !self.source_annotations_safe(declaration.file, ty) {
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
                    self.calls,
                    &[],
                    active,
                ) {
                    DefinitionSafety::Unsupported(reason) => unsupported = Some(reason),
                    unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                    DefinitionSafety::Supported => {}
                }
            }
            active.pop();
            if let Some(reason) = unsupported {
                return DefinitionSafety::Unsupported(reason);
            }
        }
        // Check the reference graph first: aggregate inspection can recursively
        // visit a source, so its written cycle must already have been rejected.
        let checked = if unwrap(node).kind() == NodeKind::MatrixLiteral {
            // Preserve the existing literal cell checker after transitive source
            // and annotation inspection, without adding a matrix evaluator.
            crate::expression_safety(self.context, self.bindings, view, file, unwrap(node))
        } else {
            self.direct_safety(file, node, view, generators)
        };
        match checked {
            DefinitionSafety::Supported => safety,
            checked => checked,
        }
    }
    // Inspect collection construction without turning unproved index
    // membership, cardinality or filtered iteration into output dependencies.
    fn collection_construction_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let checked = (|| -> Result<(), String> {
            let facts = self.view(file, node, view);
            let typed = |value: &SyntaxNode| {
                let range = self.context.files[file].location(value.range()).range;
                facts
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .map(|e| &e.ty)
            };
            let present_parameter = |t: &TypeInst| {
                t.known() && !optional(t) && t.instantiation == Instantiation::Parameter
            };
            let present = |t: &TypeInst| t.known() && !optional(t);
            let integer_set = |t: &TypeInst| {
                present_parameter(t)
                    && matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int)
            };
            let ty = typed(node).ok_or("parameter collection type unavailable")?;
            let decision_sets = node.kind() == NodeKind::ArrayComprehension
                && ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { element, .. }
                    if decision_integer_set(element));
            let element = match (&ty.kind, node.kind()) {
                (TypeKind::Set(element), NodeKind::SetComprehension) if integer_set(ty) => {
                    element.as_ref()
                }
                (TypeKind::Array { indices, element }, NodeKind::ArrayComprehension)
                    if present(ty)
                        && (ty.instantiation == Instantiation::Parameter
                            || matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                            || decision_sets)
                        && indices.len() == 1
                        && present_parameter(&indices[0])
                        && indices[0].kind == TypeKind::Int
                        && (matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                            || integer_set(element)
                            || decision_sets) =>
                {
                    element.as_ref()
                }
                (TypeKind::Bool, NodeKind::GeneratorCallExpression)
                    if present(ty)
                        && (self.core(file, node, view, "forall")
                            || self.core(file, node, view, "exists")) =>
                {
                    ty
                }
                (TypeKind::Int, NodeKind::GeneratorCallExpression)
                    if present(ty) && self.core(file, node, view, "sum") =>
                {
                    ty
                }
                _ => {
                    return Err(
                        "parameter collection shape, type or identity is unsupported".into(),
                    );
                }
            };
            let children: Vec<_> = node.child_nodes().collect();
            let [first, second] = children.as_slice() else {
                return Err("parameter collection body or headers are unsupported".into());
            };
            let (body, list) = if node.kind() == NodeKind::GeneratorCallExpression {
                (second, first)
            } else {
                (first, second)
            };
            if list.kind() != NodeKind::GeneratorList
                || list.child_nodes().count() == 0
                || typed(body).is_none_or(|t| {
                    !present(t)
                        || if decision_sets {
                            t != element
                        } else if node.kind() == NodeKind::GeneratorCallExpression
                            && element.kind == TypeKind::Int
                        {
                            !matches!(t.kind, TypeKind::Int | TypeKind::Bool)
                                || !crate::types::coerces(t, element)
                        } else {
                            t.kind != element.kind
                        }
                })
            {
                return Err("parameter collection body type is unsupported".into());
            }
            if decision_sets && unwrap(body).kind() != NodeKind::ArrayAccessExpression {
                return Err("decision set collection requires a selected array cell".into());
            }
            // Decision-set generators introduce absent cells; this exact core
            // overload may inspect the sum without proving which cells occur.
            let optional_sum = node.kind() == NodeKind::GeneratorCallExpression
                && self.core(file, node, view, "sum")
                && present(ty) && ty.kind == TypeKind::Int
                && ty.instantiation == Instantiation::Decision
                && typed(body).is_some_and(|t| present(t) && t.kind == TypeKind::Int)
                && self.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if return_type == ty && parameters.len() == 1
                            && parameters[0].known() && !parameters[0].optional
                            && parameters[0].instantiation == Instantiation::Decision
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && present_parameter(&indices[0])
                                    && indices[0].kind == TypeKind::Int && element.known()
                                    && element.optional && element.instantiation == Instantiation::Decision
                                    && element.kind == TypeKind::Int))
                });
            if optional_sum && list.child_nodes().count() != 1 {
                return Err("optional sum requires one decision enum set header".into());
            }
            if node.kind() == NodeKind::GeneratorCallExpression
                && self.operation_fact(facts, file, node).is_none_or(|call| {
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && return_type == ty
                            && (present(&parameters[0]) || optional_sum)
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                                if indices.len() == 1 && present_parameter(&indices[0])
                                    && indices[0].kind == TypeKind::Int
                                    && (present(formal) || optional_sum)
                                    && formal.kind == element.kind
                                    && typed(body).is_some_and(|actual| crate::types::coerces(actual, formal))))
                })
            {
                return Err("collection selected signature is unsupported".into());
            }
            let enum_sum = !optional_sum
                && node.kind() == NodeKind::GeneratorCallExpression
                && self.core(file, node, view, "sum")
                && list.child_nodes().any(|header| {
                    header.child_nodes().next().is_some_and(|source| {
                        typed(source).is_some_and(|t| {
                            present_parameter(t)
                                && matches!(&t.kind, TypeKind::Set(element)
                                    if present_parameter(element)
                                        && matches!(element.kind, TypeKind::Enum(_)))
                        })
                    })
                });
            let mut nodes = vec![node];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, current) {
                    return Err("parameter collection annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let conditional_relation = node.kind() == NodeKind::GeneratorCallExpression
                && self.core(file, node, view, "forall")
                && unwrap(body).kind() == NodeKind::ConditionalExpression;
            let mut all = generators.to_vec();
            let mut unsupported = None;
            if decision_sets || enum_sum {
                for (position, header) in generators.iter().enumerate() {
                    for value in header.child_nodes() {
                        let (value, scope) = if value.kind() == NodeKind::WhereFilter {
                            let conditions: Vec<_> = value.child_nodes().collect();
                            let [condition] = conditions.as_slice() else {
                                return Err("collection lexical filter is unsupported".into());
                            };
                            (*condition, &generators[..=position])
                        } else {
                            (value, &generators[..position])
                        };
                        if let DefinitionSafety::Unsupported(reason) = self
                            .initialized_source_safety(file, value, view, scope, &mut Vec::new())
                        {
                            unsupported = Some(reason);
                        }
                        if let Some(reason) =
                            self.closed_integer_source_error(file, value, true, true)
                        {
                            unsupported = Some(reason);
                        }
                    }
                }
            }
            for header in list.child_nodes() {
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if !header.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                }) || binders.is_empty()
                    || optional_sum && binders.len() != 1
                    || binders.len()
                        != crate::domains::generator_slots(&self.context.files[file].parsed, header)
                    || binders.iter().any(|d| {
                        let t = &facts.declarations[d.id.0].ty;
                        !present_parameter(t)
                            || if optional_sum {
                                !matches!(t.kind, TypeKind::Enum(_))
                            } else {
                                t.kind != TypeKind::Int
                                    && !(enum_sum && matches!(t.kind, TypeKind::Enum(_)))
                            }
                    })
                {
                    return Err(if optional_sum {
                        "optional sum requires named parameter enum binders".into()
                    } else {
                        "parameter collection requires named integer in binders".into()
                    });
                }
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("parameter collection source unavailable")?;
                if optional_sum {
                    let source_id = self
                        .reference(file, unwrap(source))
                        .filter(|id| {
                            unwrap(source).kind() == NodeKind::Expression
                                && self.bindings.declarations[id.0].top_level
                                && self.bindings.declarations[id.0].role == DeclarationRole::Value
                        })
                        .ok_or("optional sum requires a bare decision enum set source")?;
                    if typed(source).is_none_or(|t| !present(t)
                        || t != &self.calls.declarations[source_id.0].ty
                        || t.instantiation != Instantiation::Decision
                        || !matches!(&t.kind, TypeKind::Set(element)
                            if present_parameter(element) && matches!(element.kind, TypeKind::Enum(_))
                                && binders.iter().all(|binder| facts.declarations[binder.id.0].ty.kind == element.kind)))
                    {
                        return Err("optional sum source and binder enum identities differ".into());
                    }
                } else if typed(source).is_none_or(|t| {
                    !(integer_set(t)
                        && binders
                            .iter()
                            .all(|binder| facts.declarations[binder.id.0].ty.kind == TypeKind::Int))
                        && !(enum_sum
                            && present_parameter(t)
                            && matches!(&t.kind, TypeKind::Set(element)
                                if present_parameter(element)
                                    && matches!(element.kind, TypeKind::Enum(_))
                                    && binders.iter().all(|binder|
                                        &facts.declarations[binder.id.0].ty == element.as_ref())))
                }) {
                    return Err(if enum_sum {
                        "sum requires matching present parameter set sources and binders"
                    } else {
                        "parameter collection requires a present parameter integer set source"
                    }
                    .into());
                }
                let source_safety = self
                    .selected_generator_source_safety(file, header, view, &all)
                    .unwrap_or_else(|| {
                        self.initialized_source_safety(file, source, view, &all, &mut Vec::new())
                    });
                if let DefinitionSafety::Unsupported(reason) = source_safety {
                    unsupported = Some(reason);
                } else if (conditional_relation || optional_sum || decision_sets || enum_sum)
                    && let Some(reason) = self.closed_integer_source_error(file, source, true, true)
                {
                    unsupported = Some(reason);
                }
                all.push(header);
                for filter in header
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let conditions: Vec<_> = filter.child_nodes().collect();
                    let [condition] = conditions.as_slice() else {
                        return Err("parameter collection filter is unsupported".into());
                    };
                    if typed(condition)
                        .is_none_or(|t| !present_parameter(t) || t.kind != TypeKind::Bool)
                    {
                        return Err("parameter collection filter type is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) =
                        self.initialized_source_safety(file, condition, view, &all, &mut Vec::new())
                    {
                        unsupported = Some(reason);
                    } else if (conditional_relation || optional_sum || decision_sets || enum_sum)
                        && let Some(reason) =
                            self.closed_integer_source_error(file, condition, true, true)
                    {
                        unsupported = Some(reason);
                    }
                }
            }
            let body_safety = if decision_sets {
                // Exact traversal dependencies do not inspect the written set domain.
                self.decision_set_source_safety(file, body, view, &all)
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported(
                            "decision set collection body is unsupported".into(),
                        )
                    })
            } else if conditional_relation {
                self.boolean_relation_safety(file, body, view, &all)
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported(
                            "Boolean conditional inspection is unavailable".into(),
                        )
                    })
            } else if node.kind() == NodeKind::GeneratorCallExpression
                && self.core(file, node, view, "exists")
            {
                self.boolean_relation_safety(file, body, view, &all)
                    .unwrap_or_else(|| {
                        self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
                    })
            } else {
                self.initialized_source_safety(file, body, view, &all, &mut Vec::new())
            };
            if let DefinitionSafety::Unsupported(reason) = body_safety {
                unsupported = Some(reason);
            } else if (optional_sum || decision_sets || enum_sum)
                && let Some(reason) = self.closed_integer_source_error(file, body, true, true)
            {
                unsupported = Some(reason);
            }
            match unsupported {
                Some(reason) => Err(reason),
                None => Ok(()),
            }
        })();
        match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "parameter collection construction or membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        }
    }
    fn integer_comprehension_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        let checked = (|| -> Result<(), String> {
            let facts = self.view(file, node, view);
            let range = self.context.files[file].location(node.range()).range;
            if node.kind() != NodeKind::ArrayComprehension
                || !facts.expressions.iter().any(|e| e.file == file && e.location.range == range
                    && e.ty.known() && !optional(&e.ty)
                    && matches!(&e.ty.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].kind == TypeKind::Int && indices[0].instantiation == Instantiation::Parameter
                            && element.known() && !optional(element) && element.kind == TypeKind::Int))
            { return Err("integer comprehension shape or optionality is unsupported".into()); }
            let children: Vec<_> = node.child_nodes().collect();
            let [body, list] = children.as_slice() else {
                return Err("integer comprehension body or header is unsupported".into());
            };
            let headers: Vec<_> = list.child_nodes().collect();
            if list.kind() != NodeKind::GeneratorList
                || headers.len() != 1
                || headers[0].child_nodes().count() != 1
                || !headers[0].children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
            {
                return Err("integer comprehension requires one unfiltered in binder".into());
            }
            let binders: Vec<_> = self
                .bindings
                .declarations
                .iter()
                .filter(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Generator
                        && d.syntax_range == headers[0].range()
                })
                .collect();
            if binders.len() != 1
                || !matches!(
                    &facts.declarations[binders[0].id.0].ty,
                    TypeInst {
                        kind: TypeKind::Int,
                        instantiation: Instantiation::Parameter,
                        ..
                    }
                )
                || !facts.declarations[binders[0].id.0].ty.known()
                || optional(&facts.declarations[binders[0].id.0].ty)
            {
                return Err("integer comprehension binder type is unsupported".into());
            }
            let source = unwrap(headers[0].child_nodes().next().unwrap());
            if self.expression_type(facts, file, source).is_none_or(|e| {
                !e.ty.known()
                    || optional(&e.ty)
                    || e.ty.instantiation != Instantiation::Parameter
                    || !matches!(&e.ty.kind, TypeKind::Set(element)
                        if element.known() && !optional(element) && element.kind == TypeKind::Int
                            && element.instantiation == Instantiation::Parameter)
            }) {
                return Err("integer comprehension source is unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, source, view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            let mut nodes = vec![node];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, current) {
                    return Err("integer comprehension annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut all = generators.to_vec();
            all.push(headers[0]);
            match self.initialized_source_safety(file, body, view, &all, &mut Vec::new()) {
                DefinitionSafety::Unsupported(reason) => Err(reason),
                _ => Ok(()),
            }
        })();
        match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "integer comprehension shape or membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        }
    }
    fn uncertain_comprehension_type(
        &self,
        file: FileId,
        local: &'a SyntaxNode,
        initializer: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let written = local
            .child_nodes()
            .next()
            .ok_or("local array type unavailable")?;
        let parts: Vec<_> = written.child_nodes().collect();
        if written.kind() != NodeKind::ArrayType || parts.len() != 2 {
            return Err("local comprehension requires one written integer axis".into());
        }
        let axis = parts[0]
            .child_nodes()
            .next()
            .map(unwrap)
            .ok_or("local comprehension axis unavailable")?;
        let endpoints: Vec<_> = axis.child_nodes().collect();
        let source = initializer
            .child_nodes()
            .find(|n| n.kind() == NodeKind::GeneratorList)
            .and_then(|n| n.child_nodes().next())
            .and_then(|n| n.child_nodes().next())
            .map(unwrap)
            .ok_or("local comprehension source unavailable")?;
        let facts = self.view(file, initializer, view);
        let source_id = crate::domains::parameter_integer_set_source(
            self.context,
            self.bindings,
            facts,
            file,
            source,
        )
        .ok_or("local comprehension requires a bare parameter integer set")?;
        if axis.kind() != NodeKind::RangeExpression
            || !self.core(file, axis, view, "..")
            || endpoints.len() != 2
            || crate::domains::invariant_expression_integer(
                self.context,
                self.bindings,
                file,
                endpoints[0],
            ) != Ok(Some(1))
            || crate::domains::parameter_set_cardinality(
                self.context,
                self.bindings,
                facts,
                file,
                unwrap(endpoints[1]),
            ) != Some(source_id)
        {
            return Err("local comprehension axis lacks the same-source cardinality".into());
        }
        self.dependencies(file, endpoints[0], view, generators)?;
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return Err("local comprehension type annotation is unsupported".into());
            }
            if node.kind() == NodeKind::DomainType && node.range() != parts[0].range() {
                for expression in node.child_nodes() {
                    self.dependencies(file, expression, view, generators)?;
                }
            } else {
                nodes.extend(node.child_nodes());
            }
        }
        let range = self.context.files[file].location(local.range()).range;
        if self.bindings.references.iter().any(|r| {
            r.file == file
                && range.start <= r.location.range.start
                && r.location.range.end <= range.end
                && matches!(r.resolution, BindingResolution::Resolved(id)
                if self.bindings.declarations[id.0].role == DeclarationRole::Local
                    && self.bindings.declarations[id.0].syntax_range.start >= local.range().start)
        }) {
            return Err("local comprehension has a cyclic or forward local dependency".into());
        }
        Ok(())
    }
    fn aggregate_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let sum = self.core(file, node, view, "sum");
        let card = self.core(file, node, view, "card");
        if !(sum || card || self.core(file, node, view, "length")) {
            return None;
        }
        let facts = self.view(file, node, view);
        let typed = |value: &SyntaxNode| self.expression_type(facts, file, value).map(|e| &e.ty);
        let collection = |t: &TypeInst, actual: bool| {
            t.known()
                && !optional(t)
                && (!card || t.instantiation == Instantiation::Parameter || decision_integer_set(t))
                && if card {
                    matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element)
                        && element.instantiation == Instantiation::Parameter
                        && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_)))
                } else if !sum {
                    // Length is selected for any present one-dimensional array;
                    // inspecting its construction proves no element value or size.
                    matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                            && indices[0].instantiation == Instantiation::Parameter
                            && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
                            && element.known() && !optional(element))
                } else {
                    matches!(&t.kind, TypeKind::Array { indices, element }
                    if (indices.len() == 1 || actual && sum && indices.len() == 2)
                        && indices.iter().all(|index| index.known() && !optional(index)
                            && index.kind == TypeKind::Int && index.instantiation == Instantiation::Parameter)
                        && element.known() && !optional(element)
                        && (element.kind == TypeKind::Int
                            || actual && indices.len() == 1 && element.kind == TypeKind::Bool))
                }
        };
        let integer_result = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Int
                && (sum || card || t.instantiation == Instantiation::Parameter)
        };
        let children: Vec<_> = node.child_nodes().collect();
        // set2array is only a matching view. Inspect the written set and its
        // evaluation prerequisites without granting array extent or sum facts.
        let set_element = children
            .first()
            .and_then(|argument| typed(argument))
            .and_then(|t| {
                if t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && let TypeKind::Set(element) = &t.kind
                    && element.instantiation == Instantiation::Parameter
                    && matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
                {
                    Some(element.as_ref())
                } else {
                    None
                }
            });
        let set_argument = !card && set_element.is_some_and(|element| {
            typed(node).is_some_and(|t| integer_result(t) && t.instantiation == Instantiation::Parameter)
                && self.operation_fact(facts, file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1
                            && integer_result(return_type)
                            && return_type.instantiation == Instantiation::Parameter
                            && parameters[0].known() && !optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Parameter
                            && matches!(&parameters[0].kind, TypeKind::Array { indices, element: formal }
                                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                    && indices[0].kind == TypeKind::Int
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && formal.known() && !optional(formal)
                                    && formal.instantiation == Instantiation::Parameter
                                    && formal.kind == element.kind))
                })
        });
        if children.len() != 1
            || children[0].kind() == NodeKind::NamedArgument
            || !crate::definitions::annotations_safe(self.context, file, written)
            || typed(node).is_none_or(|t| !integer_result(t))
            || typed(children[0]).is_none_or(|t| !collection(t, true) && !set_argument)
            || self.operation_fact(facts, file, node).is_none_or(|c| {
                !matches!(&c.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && collection(&parameters[0], false)
                        && integer_result(return_type) && Some(return_type) == typed(node)
                        && (!card || parameters[0] == *typed(children[0]).unwrap()
                            && return_type.instantiation == parameters[0].instantiation)
                        && (!sum || set_argument || typed(children[0]).is_some_and(|actual|
                            crate::types::coerces(actual, &parameters[0])
                                // Preserve the existing core sum rank2 Int flattening view.
                                || matches!((&actual.kind, &parameters[0].kind),
                                    (TypeKind::Array { indices, element },
                                     TypeKind::Array { indices: formal_indices, element: formal })
                                        if indices.len() == 2 && formal_indices.len() == 1
                                            && element.kind == TypeKind::Int && formal.kind == TypeKind::Int
                                            && crate::types::coerces(element, formal)))))
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "aggregate selected signature, collection or annotation is unsupported".into(),
            ));
        }
        let safety =
            self.initialized_source_safety(file, children[0], view, generators, &mut Vec::new());
        if card
            && typed(children[0]).is_some_and(decision_integer_set)
            && !matches!(safety, DefinitionSafety::Unsupported(_))
        {
            if let Some(reason) = self.closed_integer_source_error(file, children[0], true, true) {
                return Some(DefinitionSafety::Unsupported(reason));
            }
            return Some(DefinitionSafety::Unknown(
                "decision set cardinality is unproved".into(),
            ));
        }
        let boolean_sum = sum && typed(children[0]).is_some_and(|actual|
            matches!(&actual.kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Bool));
        Some(
            if boolean_sum && !matches!(safety, DefinitionSafety::Unsupported(_)) {
                DefinitionSafety::Unknown("coerced Boolean sum value is unproved".into())
            } else if set_argument && safety == DefinitionSafety::Supported {
                DefinitionSafety::Unknown(
                    "parameter set aggregate value or cardinality is unproved".into(),
                )
            } else {
                safety
            },
        )
    }
    // Inspect decision sets without proving their values, cardinality or selector
    // membership. Strict dependencies and output contracts remain independent.
    fn decision_set_source_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let typed = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let result = typed(node).filter(|t| decision_integer_set(t))?;
        let children: Vec<_> = node.child_nodes().collect();
        let checked = (|| -> Result<(), String> {
            if !crate::definitions::annotations_safe(self.context, file, written) {
                return Err("decision set source annotation is unsupported".into());
            }
            if node.kind() == NodeKind::BinaryExpression && self.core(file, node, view, "intersect")
            {
                if children.len() != 2
                    || children.iter().any(|child| typed(child) != Some(result))
                    || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call| {
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && parameters.iter().all(|t| t == result)
                                && return_type == result)
                    })
                {
                    return Err("decision set intersection signature is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &children, view, generators)
                {
                    return Err(reason);
                }
                for child in &children {
                    if let Some(reason) = self.closed_integer_source_error(file, child, true, true)
                    {
                        return Err(reason);
                    }
                }
                return Ok(());
            }
            if node.kind() != NodeKind::ArrayAccessExpression || children.len() != 3 {
                return Err("decision set source form is unsupported".into());
            }
            let subject = unwrap(children[0]);
            if subject.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("decision set source identity is unsupported".into());
            }
            let id = self
                .reference(file, subject)
                .ok_or("decision set source identity unavailable")?;
            let declaration = &self.bindings.declarations[id.0];
            let source = typed(subject).ok_or("decision set source type unavailable")?;
            if declaration.role != DeclarationRole::Value
                || !declaration.top_level
                || !source.known()
                || optional(source)
                || source != &self.view(file, subject, view).declarations[id.0].ty
            {
                return Err("decision set source scope, type or optionality is unsupported".into());
            }
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            if source.instantiation != Instantiation::Decision
                || !matches!(&source.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && element.as_ref() == result)
                || children[1..]
                    .iter()
                    .any(|selector| typed(selector).is_none_or(|t| !integer(t)))
            {
                return Err("decision set selection shape or selectors are unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            let declared = find_node(
                self.context.files[declaration.file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("decision set source declaration unavailable")?;
            // The closed checker follows parameter aliases, but not decision arrays.
            // Start at the actual written array declaration to retain errors.
            if let Some(reason) =
                self.closed_integer_source_error(declaration.file, declared, true, true)
            {
                return Err(reason);
            }
            let domain = crate::domains::bare_index_domain(&self.domains.declarations[id.0].domain);
            let Domain::Array { indices, element } = domain else {
                return Err("decision set array source domain is unsupported".into());
            };
            if indices.len() != 2 {
                return Err("decision set array source rank is unsupported".into());
            }
            for (axis, selector) in indices.iter().zip(&children[1..]) {
                axis.numeric_minimum().map_err(str::to_owned)?;
                if let Some(reason) = self.closed_integer_source_error(file, selector, true, true) {
                    return Err(reason);
                }
                if crate::domains::invariant_expression_integer(
                    self.context,
                    self.bindings,
                    file,
                    selector,
                )
                .is_ok_and(|value| {
                    value.is_some_and(|value| {
                        crate::domains::index_domain_member(axis, value) == Some(false)
                            || matches!(crate::domains::bare_index_domain(axis), Domain::Range { lower, .. }
                                if crate::domains::invariant_integer(lower).is_ok_and(|bound|
                                    bound.is_some_and(|bound| value < bound)))
                    })
                }) {
                    return Err("decision set selection is outside its declared index set".into());
                }
            }
            let member = crate::domains::bare_index_domain(element);
            let Domain::Set(member) = member else {
                return Err("decision set written element domain is unsupported".into());
            };
            member.numeric_minimum().map_err(str::to_owned)?;
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("decision set values and membership are unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect a written full-axis slice without establishing its scalar
    // selectors' membership, total evaluation or output extent.
    fn full_axis_slice_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let full_axis = |selector: &SyntaxNode| {
            selector.kind() == NodeKind::RangeExpression
                && selector.child_nodes().next().is_none()
                && matches!(crate::domains::tokens(&self.context.files[file].parsed, selector).as_slice(),
                    [token] if token.kind == TokenKind::RangeInclusive)
        };
        if !children.iter().skip(1).any(|selector| full_axis(selector)) {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let typed = |value: &SyntaxNode| {
                self.expression_type(self.view(file, value, view), file, value)
                    .map(|e| &e.ty)
            };
            let subject = unwrap(children.first().ok_or("slice subject unavailable")?);
            if !crate::definitions::annotations_safe(self.context, file, written)
                || subject.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            {
                return Err("slice subject identity or annotation is unsupported".into());
            }
            let id = self
                .reference(file, subject)
                .ok_or("slice declaration identity unavailable")?;
            let declaration = &self.bindings.declarations[id.0];
            // Initial required cases use original model arrays. Local/default
            // array evaluation needs its own owning-scope inspection.
            if declaration.role != DeclarationRole::Value || !declaration.top_level {
                return Err("slice source declaration scope is unsupported".into());
            }
            let source = typed(subject).ok_or("slice source type unavailable")?;
            let result = typed(node).ok_or("slice result type unavailable")?;
            if !source.known()
                || optional(source)
                || !result.known()
                || optional(result)
                || source != &self.view(file, subject, view).declarations[id.0].ty
                || source.instantiation != result.instantiation
            {
                return Err("slice source or result qualifiers are unsupported".into());
            }
            let (
                TypeKind::Array { indices, element },
                TypeKind::Array {
                    indices: retained,
                    element: result_element,
                },
            ) = (&source.kind, &result.kind)
            else {
                return Err("slice source or result is not an array".into());
            };
            if indices.len() + 1 != children.len()
                || element != result_element
                || !matches!(
                    element.kind,
                    TypeKind::Int | TypeKind::Bool | TypeKind::Enum(_)
                )
                || indices.iter().any(|index| {
                    !index.known()
                        || optional(index)
                        || index.instantiation != Instantiation::Parameter
                        || !matches!(index.kind, TypeKind::Int | TypeKind::Enum(_))
                })
            {
                return Err("slice rank, element or axis identity is unsupported".into());
            }
            let mut count = 0;
            let mut scalars = Vec::new();
            for (selector, axis) in children[1..].iter().zip(indices) {
                let actual = typed(selector).ok_or("slice selector type unavailable")?;
                if full_axis(selector) {
                    if !actual.known()
                        || optional(actual)
                        || actual.instantiation != Instantiation::Parameter
                        || !matches!(&actual.kind, TypeKind::Set(element) if element.as_ref() == axis)
                        || retained.get(count) != Some(axis)
                    {
                        return Err(
                            "full slice marker or retained axis identity is unsupported".into()
                        );
                    }
                    count += 1;
                } else {
                    if !actual.known()
                        || optional(actual)
                        || actual.instantiation != Instantiation::Parameter
                        || actual.kind != axis.kind
                    {
                        return Err("slice scalar selector type or identity is unsupported".into());
                    }
                    scalars.push(*selector);
                }
            }
            if count != retained.len() {
                return Err("slice retained rank is unsupported".into());
            }
            let mut unsupported = None;
            // Check every source and scalar selector even when an earlier one
            // has uncertain membership. Unsupported evaluation must dominate.
            for value in std::iter::once(children[0]).chain(scalars) {
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, value, view, generators, &mut Vec::new())
                {
                    unsupported = Some(reason);
                }
            }
            match unsupported {
                Some(reason) => Err(reason),
                None => Ok(()),
            }
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "full-axis slice evaluation or scalar membership is unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    fn is_scalar_integer_extremum(
        &self,
        file: FileId,
        written: &SyntaxNode,
        view: &CallableFacts,
        operands: &[&SyntaxNode],
    ) -> bool {
        let node = unwrap(written);
        let facts = self.view(file, node, view);
        let integer = |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
        if operands.len() != 2
            || operands.iter().any(|n| n.kind() == NodeKind::NamedArgument)
            || !(self.core(file, node, view, "min") || self.core(file, node, view, "max"))
            || self
                .expression_type(facts, file, node)
                .is_none_or(|e| !integer(&e.ty))
            || operands.iter().any(|operand| {
                self.expression_type(self.view(file, operand, view), file, operand)
                    .is_none_or(|e| !integer(&e.ty))
            })
            || self.operation_fact(facts, file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 2 && parameters.iter().all(integer)
                        && integer(return_type))
            })
        {
            return false;
        }
        let mut nodes = vec![written];
        while let Some(node) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return false;
            }
            nodes.extend(node.child_nodes());
        }
        true
    }
    fn set_axis_integer_reshape_arguments(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
    ) -> Option<[&'a SyntaxNode; 3]> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "array2d") {
            return None;
        }
        let mut nodes = vec![written];
        while let Some(current) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, current) {
                return None;
            }
            nodes.extend(current.child_nodes());
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [rows, columns, values] = children.as_slice() else {
            return None;
        };
        let arguments = [*rows, *columns, *values];
        let unsupported_bare_value = |argument: &SyntaxNode| {
            argument.kind() != NodeKind::Expression
                || self.reference(file, argument).is_none_or(|id| {
                    let declaration = &self.bindings.declarations[id.0];
                    !declaration.top_level || declaration.role != DeclarationRole::Value
                })
        };
        let inline = unwrap(values);
        if arguments[..2]
            .iter()
            .any(|argument| unsupported_bare_value(argument))
            || unsupported_bare_value(values) && inline.kind() != NodeKind::ArrayComprehension
        {
            return None;
        }
        let facts = self.view(file, node, view);
        let typed = |value: &SyntaxNode| self.expression_type(facts, file, value).map(|e| &e.ty);
        let parameter_int = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let integer_array = |t: &TypeInst, rank: usize| {
            t.known()
                && !optional(t)
                && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == rank && indices.iter().all(parameter_int)
                        && element.known() && !optional(element) && element.kind == TypeKind::Int)
        };
        let actual = [typed(rows)?, typed(columns)?, typed(values)?];
        let result = typed(node)?;
        if !actual[..2].iter().all(|t| t.known() && !optional(t)
            && t.instantiation == Instantiation::Parameter
            && matches!(&t.kind, TypeKind::Set(element) if parameter_int(element)))
            || !integer_array(actual[2], 1) || !integer_array(result, 2)
            || !self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 3 && parameters.iter().zip(actual).all(|(formal, actual)| formal == actual)
                        && return_type == result)
            })
        {
            return None;
        }
        if inline.kind() == NodeKind::ArrayComprehension {
            let parts: Vec<_> = inline.child_nodes().collect();
            let [_, list] = parts.as_slice() else {
                return None;
            };
            if list.kind() != NodeKind::GeneratorList || list.child_nodes().count() != 2 {
                return None;
            }
            for (header, axis) in list.child_nodes().zip(arguments[..2].iter()) {
                let sources: Vec<_> = header.child_nodes().collect();
                let [source] = sources.as_slice() else {
                    return None;
                };
                let source = unwrap(source);
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if !header.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                        if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                }) || crate::domains::generator_slots(&self.context.files[file].parsed, header)
                    != 1
                    || binders.len() != 1
                    || !parameter_int(&facts.declarations[binders[0].id.0].ty)
                    || source.kind() != NodeKind::Expression
                    || self.reference(file, source) != self.reference(file, axis)
                {
                    return None;
                }
            }
        }
        Some(arguments)
    }
    fn inline_reshape_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() != NodeKind::CallExpression {
            return None;
        }
        let axes = if self.core(file, node, view, "array1d") {
            1
        } else if self.core(file, node, view, "array2d") {
            2
        } else if self.core(file, node, view, "array3d") {
            3
        } else {
            return None;
        };
        if children.len() != axes + 1 {
            return None;
        }
        let facts = self.view(file, node, view);
        let typed = |n: &SyntaxNode| self.expression_type(facts, file, n).map(|e| &e.ty);
        let parameter_int = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && t.kind == TypeKind::Int
        };
        let parameter_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if parameter_int(element))
        };
        let array = |t: &TypeInst, rank: usize| {
            t.known()
                && !optional(t)
                && matches!(&t.kind, TypeKind::Array { indices, element }
                if indices.len() == rank && indices.iter().all(parameter_int)
                    && element.known() && !optional(element)
                    && (matches!(element.kind, TypeKind::Int | TypeKind::Bool)
                        && (axes == 2 || element.kind == TypeKind::Bool)
                        || axes == 1 && t.instantiation == Instantiation::Parameter
                            && parameter_set(element)))
        };
        let parameter_set_array = |n: &SyntaxNode| {
            typed(n).is_some_and(|t| {
                array(t, 1)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { element, .. } if parameter_set(element))
            })
        };
        let source = unwrap(children[axes]);
        if source.kind() != NodeKind::ArrayComprehension
            && !(axes == 1
                && parameter_set_array(children[axes])
                && parameter_set_array(node)
                && array_concatenation(self.context, self.bindings, facts, file, source).is_some())
        {
            return None;
        }
        let valid = children.iter().all(|n| n.kind() != NodeKind::NamedArgument)
            && children[..axes]
                .iter()
                .all(|n| typed(n).is_some_and(parameter_set))
            && typed(children[axes]).is_some_and(|t| array(t, 1))
            && typed(node).is_some_and(|t| array(t, axes))
            && self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == axes + 1 && parameters.iter().zip(&children)
                        .all(|(formal, actual)| typed(actual) == Some(formal))
                        && typed(node) == Some(return_type))
            });
        let mut nodes = vec![written];
        while let Some(current) = nodes.pop() {
            if !crate::definitions::annotations_safe(self.context, file, current) {
                return Some(DefinitionSafety::Unsupported(
                    "inline reshape annotation is unsupported".into(),
                ));
            }
            nodes.extend(current.child_nodes());
        }
        if !valid {
            return Some(DefinitionSafety::Unsupported(
                "inline reshape selected signature or type is unsupported".into(),
            ));
        }
        // Set-array inspection checks its eager axis independently. The
        // source's existing initialized inspection preserves lazy branches.
        if axes == 1
            && typed(node).is_some_and(
                |t| matches!(&t.kind, TypeKind::Array { element, .. } if parameter_set(element)),
            )
            && let Some(reason) = self.closed_integer_source_error(file, children[0], true, true)
        {
            return Some(DefinitionSafety::Unsupported(reason));
        }
        Some(
            match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("inline reshape cardinality is unproved".into()),
            },
        )
    }
    fn decision_array_bound_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !(self.core(file, node, view, "lb_array") || self.core(file, node, view, "ub_array"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let source = unwrap(argument);
        let facts = self.view(file, node, view);
        let actual = self.expression_type(facts, file, argument).map(|e| &e.ty)?;
        if actual.instantiation != Instantiation::Decision {
            return None;
        }
        let parameter_int = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.kind == TypeKind::Int
                && ty.instantiation == Instantiation::Parameter
        };
        let result = self.expression_type(facts, file, node).map(|e| &e.ty);
        let Some(array) = self.reference(file, source) else {
            return Some(DefinitionSafety::Unsupported(
                "array reflection requires a bare array identity".into(),
            ));
        };
        let declaration = &self.bindings.declarations[array.0];
        if argument.kind() == NodeKind::NamedArgument
            || source.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, source).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || declaration.role != DeclarationRole::Value || !declaration.top_level
            || !actual.known() || optional(actual)
            || !matches!(&actual.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && parameter_int(&indices[0])
                    && element.known() && !optional(element) && element.kind == TypeKind::Int
                    && element.instantiation == Instantiation::Decision)
            || result.is_none_or(|ty| !parameter_int(ty))
            || !crate::definitions::annotations_safe(self.context, file, written)
            || !crate::definitions::annotations_safe(self.context, file, node)
            || !self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0] == *actual && Some(return_type) == result)
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "array reflection selected source or signature is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        if let Some(written) = find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        ) && let Some(value) = written
            .child_nodes()
            .find(|node| is_expression(node.kind()))
            .map(unwrap)
            && value.kind() == NodeKind::ArrayLiteral
            && value.child_nodes().next().is_none()
        {
            return Some(DefinitionSafety::Unsupported(
                "array reflection source is empty".into(),
            ));
        }
        let mut domain = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        if let Domain::Array { indices, .. } = domain {
            for axis in indices {
                match crate::domains::bare_index_domain(axis) {
                    Domain::Range { lower, upper } => {
                        match (
                            crate::domains::invariant_integer(lower),
                            crate::domains::invariant_integer(upper),
                        ) {
                            (Err(reason), _) | (_, Err(reason)) => {
                                return Some(DefinitionSafety::Unsupported(reason));
                            }
                            (Ok(Some(lower)), Ok(Some(upper))) if upper < lower => {
                                return Some(DefinitionSafety::Unsupported(
                                    "array reflection source is empty".into(),
                                ));
                            }
                            _ => {}
                        }
                    }
                    Domain::LiteralSet(values) if values.is_empty() => {
                        return Some(DefinitionSafety::Unsupported(
                            "array reflection source is empty".into(),
                        ));
                    }
                    Domain::Unsupported(reason) => {
                        return Some(DefinitionSafety::Unsupported(reason.clone()));
                    }
                    _ => {}
                }
            }
        }
        Some(DefinitionSafety::Unknown(
            "reflected array bound is unproved".into(),
        ))
    }
    fn parameter_float_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let facts = self.view(file, node, view);
        let typed = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let present = |ty: &TypeInst, kind, instantiation| {
            ty.known() && !optional(ty) && ty.kind == kind && ty.instantiation == instantiation
        };
        if node.kind() == NodeKind::Expression
            && matches!(crate::domains::tokens(&self.context.files[file].parsed, node).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            && typed(node).is_some_and(|t| present(t, TypeKind::Float, Instantiation::Parameter))
            && let Some(id) = self.reference(file, node)
            && self.bindings.declarations[id.0].role == DeclarationRole::Local
        {
            let declaration = &self.bindings.declarations[id.0];
            if declaration.file != file
                || declaration.syntax_range.end > node.range().start
                || !present(
                    &facts.declarations[id.0].ty,
                    TypeKind::Float,
                    Instantiation::Parameter,
                )
            {
                return Some(DefinitionSafety::Unsupported(
                    "Float local alias identity or order is unsupported".into(),
                ));
            }
            let Some(local) = find_node(
                self.context.files[file].parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            ) else {
                return Some(DefinitionSafety::Unsupported(
                    "Float local declaration unavailable".into(),
                ));
            };
            let lexical: Vec<_> = generators
                .iter()
                .copied()
                .filter(|g| g.range().end <= declaration.syntax_range.start)
                .collect();
            if !crate::definitions::annotations_safe(self.context, file, written)
                || !crate::definitions::annotations_safe(self.context, file, local)
                || self.type_dependencies(file, local, view, &lexical).is_err()
            {
                return Some(DefinitionSafety::Unsupported(
                    "Float local type or annotation is unsupported".into(),
                ));
            }
            let Some(value) = local.child_nodes().find(|n| is_expression(n.kind())) else {
                return Some(DefinitionSafety::Unsupported(
                    "Float local initializer unavailable".into(),
                ));
            };
            if typed(value).is_none_or(|t| !present(t, TypeKind::Float, Instantiation::Parameter)) {
                return Some(DefinitionSafety::Unsupported(
                    "Float local initializer type is unsupported".into(),
                ));
            }
            return Some(
                match self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new()) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("Float local value is unproved".into()),
                },
            );
        }
        let name = match node.kind() {
            NodeKind::BinaryExpression => operator(self.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                .filter(|name| matches!(*name, "*" | "/")),
            NodeKind::CallExpression => ["int2float", "ceil", "fix"]
                .into_iter()
                .find(|name| self.core(file, node, view, name)),
            _ => None,
        }?;
        let (input, result, instantiation, arity) = match name {
            "int2float" => (TypeKind::Int, TypeKind::Float, Instantiation::Parameter, 1),
            "ceil" => (TypeKind::Float, TypeKind::Int, Instantiation::Parameter, 1),
            "fix" => (TypeKind::Int, TypeKind::Int, Instantiation::Decision, 1),
            "*" | "/"
                if typed(node)
                    .is_some_and(|t| present(t, TypeKind::Float, Instantiation::Parameter)) =>
            {
                (
                    TypeKind::Float,
                    TypeKind::Float,
                    Instantiation::Parameter,
                    2,
                )
            }
            _ => return None,
        };
        let children: Vec<_> = node.child_nodes().collect();
        // Parameter fix retains its existing strict path; only the unproved
        // scalar decision value needs this inspection fallback.
        if name == "fix"
            && (children.len() != 1
                || typed(children[0])
                    .is_none_or(|t| !present(t, TypeKind::Int, Instantiation::Decision)))
        {
            return None;
        }
        if !self.core(file, node, view, name)
            || children.len() != arity
            || children.iter().any(|n| {
                n.kind() == NodeKind::NamedArgument
                    || typed(n).is_none_or(|t| !present(t, input.clone(), instantiation))
            })
            || typed(node).is_none_or(|t| !present(t, result.clone(), Instantiation::Parameter))
            || !crate::definitions::annotations_safe(self.context, file, written)
            || self.operation_fact(facts, file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == arity
                        && parameters.iter().all(|t| present(t, input.clone(), instantiation))
                        && present(return_type, result.clone(), Instantiation::Parameter)
                        && Some(return_type) == typed(node))
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "Float conversion or scalar fix signature is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        if name == "/" {
            let divisor = unwrap(children[1]);
            let tokens = crate::domains::tokens(&self.context.files[file].parsed, divisor);
            let literal_zero = divisor.kind() == NodeKind::Expression
                && tokens.len() == 1
                && tokens[0].kind == TokenKind::FloatLiteral
                && self.context.files[file].parsed.source()[tokens[0].range.clone()]
                    .split(['e', 'E'])
                    .next()
                    .is_some_and(|mantissa| mantissa.bytes().all(|b| matches!(b, b'0' | b'.')));
            if literal_zero {
                return Some(DefinitionSafety::Unsupported(
                    "Float division by literal zero is unsupported".into(),
                ));
            }
        }
        Some(DefinitionSafety::Unknown(
            "Float conversion, arithmetic or scalar fixedness is unproved".into(),
        ))
    }
    // Inspect disjoint closed arithmetic fragments and initialized integer
    // sources. Collection inspection follows initialized sets and arrays as well.
    // Existing evaluation supplies an error veto, never a value fact.
    fn closed_integer_source_error(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        eager_only: bool,
        follow_collections: bool,
    ) -> Option<String> {
        fn literal_parts(
            producer: &Producer<'_>,
            file: FileId,
            node: &SyntaxNode,
            eager_only: bool,
        ) -> Result<bool, String> {
            let context = producer.context;
            let bindings = producer.bindings;
            if eager_only
                && node.kind() == NodeKind::ConditionalExpression
                && producer.selected_set_source
            {
                let branches: Vec<_> = node.child_nodes().collect();
                if let [then, otherwise] = branches.as_slice() {
                    let first: Vec<_> = then.child_nodes().collect();
                    let last: Vec<_> = otherwise.child_nodes().collect();
                    if then.kind() == NodeKind::ConditionalBranch
                        && otherwise.kind() == NodeKind::ElseBranch
                        && let ([guard, body], [fallback]) = (first.as_slice(), last.as_slice())
                    {
                        let selected = producer.assertion_literal(file, guard);
                        literal_parts(producer, file, guard, eager_only)?;
                        for value in [
                            (*body, selected != Some(TokenKind::False)),
                            (*fallback, selected != Some(TokenKind::True)),
                        ] {
                            if value.1 && literal_parts(producer, file, value.0, eager_only)? {
                                crate::domains::invariant_expression_integer(
                                    context, bindings, file, value.0,
                                )?;
                            }
                        }
                        return Ok(false);
                    }
                }
            }
            let inspected_conjunction = eager_only
                && producer.selected_set_source
                && node.kind() == NodeKind::BinaryExpression
                && producer.core(file, node, producer.calls, "/\\")
                && {
                    let facts = producer.view(file, node, producer.calls);
                    let parameter_bool = |ty: &TypeInst| {
                        ty.known()
                            && !optional(ty)
                            && ty.instantiation == Instantiation::Parameter
                            && ty.kind == TypeKind::Bool
                    };
                    let operands: Vec<_> = node.child_nodes().collect();
                    operands.len() == 2
                        && operands.iter().all(|operand| {
                            producer
                                .expression_type(facts, file, operand)
                                .is_some_and(|expression| parameter_bool(&expression.ty))
                        })
                        && producer.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2 && parameters.iter().all(parameter_bool)
                                    && parameter_bool(return_type)
                                    && producer.expression_type(facts, file, node)
                                        .is_some_and(|expression| &expression.ty == return_type))
                        })
                };
            if eager_only
                && (node.kind() == NodeKind::ConditionalExpression
                    || node.kind() == NodeKind::BinaryExpression
                        && matches!(
                            operator(context, file, node)
                                .and_then(crate::bindings::symbolic_operator),
                            Some("/\\" | "\\/" | "<->" | "->" | "<-" | "default")
                        )
                        && !inspected_conjunction)
            {
                return Err("lazy initialized arithmetic inspection is unavailable".into());
            }
            let mut children = Vec::new();
            for child in node.child_nodes() {
                children.push((child, literal_parts(producer, file, child, eager_only)?));
            }
            let literal = match node.kind() {
                NodeKind::Expression => {
                    matches!(crate::domains::tokens(&context.files[file].parsed, node).as_slice(),
                    [token] if token.kind == TokenKind::IntegerLiteral)
                }
                NodeKind::ParenthesizedExpression => children.len() == 1 && children[0].1,
                NodeKind::UnaryExpression => {
                    children.len() == 1
                        && children[0].1
                        && matches!(
                            operator(context, file, node),
                            Some(TokenKind::Plus | TokenKind::Minus)
                        )
                }
                NodeKind::BinaryExpression => {
                    children.len() == 2
                        && children.iter().all(|(_, literal)| *literal)
                        && matches!(
                            operator(context, file, node),
                            Some(
                                TokenKind::Plus
                                    | TokenKind::Minus
                                    | TokenKind::Star
                                    | TokenKind::Div
                                    | TokenKind::Mod
                            )
                        )
                }
                _ => false,
            };
            if !literal {
                for (child, literal) in children {
                    if literal {
                        crate::domains::invariant_expression_integer(
                            context, bindings, file, child,
                        )?;
                    }
                }
            }
            Ok(literal)
        }
        let mut sources = Vec::new();
        let mut pending = vec![(file, node)];
        while let Some((file, root)) = pending.pop() {
            match literal_parts(self, file, root, eager_only) {
                Ok(true) => {
                    if let Err(reason) = crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        root,
                    ) {
                        return Some(reason);
                    }
                }
                Err(reason) => return Some(reason),
                Ok(false) => {}
            }
            let mut nodes = vec![root];
            while let Some(node) = nodes.pop() {
                nodes.extend(node.child_nodes());
                if self.selected_set_source && node.kind() == NodeKind::DomainType {
                    match literal_parts(self, file, node, true) {
                        Ok(true) => {
                            if let Err(reason) = crate::domains::invariant_expression_integer(
                                self.context,
                                self.bindings,
                                file,
                                node,
                            ) {
                                return Some(reason);
                            }
                        }
                        Err(reason) => return Some(reason),
                        Ok(false) => {}
                    }
                }
                if node.kind() == NodeKind::Expression
                    && let Some(id) = self.reference(file, node)
                    && !sources.contains(&id)
                {
                    let declaration = &self.bindings.declarations[id.0];
                    let ty = &self.calls.declarations[id.0].ty;
                    if declaration.top_level
                        && declaration.role == DeclarationRole::Value
                        && ty.known()
                        && !optional(ty)
                        && ty.instantiation == Instantiation::Parameter
                        && (ty.kind == TypeKind::Int
                            || follow_collections
                                && matches!(ty.kind, TypeKind::Set(_) | TypeKind::Array { .. }))
                        && let Some(written) = find_node(
                            self.context.files[declaration.file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                    {
                        sources.push(id);
                        pending.push((declaration.file, written));
                    }
                }
            }
        }
        None
    }
    // Written closed enum extent is used only to reject undefined operations.
    fn enum_extent(&self, id: DeclarationId) -> Option<i64> {
        let declaration = &self.bindings.declarations[id.0];
        if declaration.role != DeclarationRole::Enum {
            return None;
        }
        let written = find_node(
            self.context.files[declaration.file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )?;
        let definition = written
            .child_nodes()
            .find(|n| n.kind() == NodeKind::EnumDefinition)?;
        let parts: Vec<_> = definition.child_nodes().collect();
        let [part] = parts.as_slice() else {
            return None;
        };
        if part.kind() == NodeKind::EnumCases
            && part
                .child_nodes()
                .all(|n| n.kind() == NodeKind::EnumCase && n.child_nodes().next().is_none())
        {
            return i64::try_from(part.child_nodes().count()).ok();
        }
        if part.kind() != NodeKind::EnumConstructor {
            return None;
        }
        let tokens = crate::domains::tokens(&self.context.files[declaration.file].parsed, part);
        let name = tokens.first().map(|t| {
            self.context.files[declaration.file].parsed.source()[t.range.clone()].trim_matches('\'')
        });
        if name != Some("anon_enum") {
            return None;
        }
        let children: Vec<_> = part.child_nodes().collect();
        let [count] = children.as_slice() else {
            return None;
        };
        crate::domains::invariant_expression_integer(
            self.context,
            self.bindings,
            declaration.file,
            unwrap(count),
        )
        .ok()
        .flatten()
    }
    fn parameter_enum_conversion_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "to_enum") {
            return None;
        }
        let facts = self.view(file, node, view);
        let ty = |value: &SyntaxNode| self.expression_type(facts, file, value).map(|e| &e.ty);
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        let children: Vec<_> = node.child_nodes().collect();
        let checked = (|| -> Result<(), String> {
            let [set, value] = children.as_slice() else {
                return Err("enum conversion arity is unsupported".into());
            };
            let result = ty(node)
                .filter(|t| parameter(t))
                .ok_or("enum conversion result unavailable")?;
            let TypeKind::Enum(enum_id) = result.kind else {
                return Err("enum conversion result identity is unsupported".into());
            };
            if children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
                || !crate::definitions::annotations_safe(self.context, file, written)
                || ty(set).is_none_or(|t| !parameter(t)
                    || !matches!(&t.kind, TypeKind::Set(element)
                        if parameter(element) && element.kind == result.kind))
                || ty(value).is_none_or(|t| !parameter(t) || !matches!(t.kind, TypeKind::Int | TypeKind::Enum(_)))
                || self.operation_fact(facts, file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && return_type == result
                            && parameters[0] == *ty(set).unwrap()
                            && parameter(&parameters[1]) && parameters[1].kind == TypeKind::Int
                            && ty(value).is_some_and(|actual| crate::types::coerces(actual, &parameters[1]))))
            {
                return Err("enum conversion selected signature or annotation is unsupported".into());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            if let Some(reason) = self.closed_integer_source_error(file, value, false, false) {
                return Err(reason);
            }
            let ordinal = crate::domains::invariant_expression_integer(
                self.context,
                self.bindings,
                file,
                value,
            )
            .ok()
            .flatten();
            if ordinal
                .is_some_and(|n| n < 1 || self.enum_extent(enum_id).is_some_and(|size| n > size))
                || self.enum_extent(enum_id) == Some(0)
            {
                return Err("enum conversion ordinal is outside the declared enum".into());
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("enum conversion membership or value is unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    // Inspect the bodyless parameter builtin without proving a successor value
    // or that the value belongs to its enum universe.
    fn parameter_enum_successor_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "enum_next") {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [set, value] = children.as_slice() else {
            return None;
        };
        let facts = self.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let parameter =
            |t: &TypeInst| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter;
        let result = ty(node).filter(|t| parameter(t) && matches!(t.kind, TypeKind::Enum(_)))?;
        let checked = (|| -> Result<(), String> {
            let call = self
                .operation_fact(facts, file, node)
                .ok_or("enum successor selected signature unavailable")?;
            let CallOutcome::Resolved {
                declaration: id,
                parameters,
                return_type,
            } = &call.outcome
            else {
                return Err("enum successor selected signature is unsupported".into());
            };
            if children.iter().any(|n| n.kind() == NodeKind::NamedArgument)
                || !crate::definitions::annotations_safe(self.context, file, written)
                || ty(set).is_none_or(|t| {
                    !parameter(t)
                        || !matches!(&t.kind, TypeKind::Set(element)
                        if parameter(element) && element.kind == result.kind)
                })
                || ty(value) != Some(result)
                || parameters.len() != 2
                || return_type != result
                || parameters[0] != *ty(set).unwrap()
                || parameters[1] != *result
            {
                return Err("enum successor tuple or annotation is unsupported".into());
            }
            let signature = self
                .calls
                .signatures
                .iter()
                .find(|s| s.declaration == *id)
                .ok_or("enum successor written signature unavailable")?;
            let generic = &signature.return_type;
            if signature.parameters.len() != 2
                || signature.parameters.iter().any(|p| p.has_default)
                || optional(generic)
                || generic.instantiation != Instantiation::Parameter
                || !matches!(
                    generic.kind,
                    TypeKind::Variable {
                        enum_only: true,
                        any: false,
                        ..
                    }
                )
                || signature.parameters[1].ty != *generic
                || optional(&signature.parameters[0].ty)
                || signature.parameters[0].ty.instantiation != Instantiation::Parameter
                || !matches!(&signature.parameters[0].ty.kind, TypeKind::Set(element) if **element == *generic)
            {
                return Err("enum successor written generic signature is unsupported".into());
            }
            let declaration = &self.bindings.declarations[id.0];
            let source = &self.context.files[declaration.file];
            let written = find_node(
                source.parsed.tree(),
                &declaration.syntax_range,
                declaration.role,
            )
            .ok_or("enum successor declaration unavailable")?;
            if !source.parsed.diagnostics().is_empty()
                || written.child_nodes().any(|n| is_expression(n.kind()))
                || !self.callable_annotations_safe(declaration.file, written)
            {
                return Err("enum successor body or annotation is unsupported".into());
            }
            for position in 0..2 {
                let formal_id = formal_parameter(self.context, self.bindings, *id, position)
                    .ok_or("enum successor formal unavailable")?;
                let owner = &self.bindings.declarations[formal_id.0];
                let formal = find_node(
                    self.context.files[owner.file].parsed.tree(),
                    &owner.syntax_range,
                    owner.role,
                )
                .ok_or("enum successor written formal unavailable")?;
                if self.calls.declarations[formal_id.0].ty != signature.parameters[position].ty
                    || formal.child_nodes().any(|n| is_expression(n.kind()))
                {
                    return Err("enum successor formal type or default is unsupported".into());
                }
                let mut nodes = vec![formal];
                while let Some(node) = nodes.pop() {
                    if !crate::definitions::annotations_safe(self.context, owner.file, node) {
                        return Err("enum successor formal annotation is unsupported".into());
                    }
                    nodes.extend(node.child_nodes());
                }
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_children_safety(file, &children, view, generators)
            {
                return Err(reason);
            }
            for argument in &children {
                if let Some(reason) = self.closed_integer_source_error(file, argument, true, true) {
                    return Err(reason);
                }
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => {
                DefinitionSafety::Unknown("enum successor value or definedness is unproved".into())
            }
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    fn parameter_set_extremum_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        // Preserve the existing index-set extremum inspection and its owning scope checks.
        if parameter_index_extremum(
            self.context,
            self.bindings,
            self.view(file, node, view),
            file,
            node,
        )
        .is_some()
        {
            return None;
        }
        if node.kind() != NodeKind::CallExpression
            || !(self.core(file, node, view, "min") || self.core(file, node, view, "max"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let facts = self.view(file, node, view);
        let typed = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let parameter_element = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(t.kind, TypeKind::Int | TypeKind::Enum(_))
        };
        let parameter_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if parameter_element(element))
        };
        if typed(argument).is_none_or(|t| !parameter_set(t)) {
            return None;
        }
        if argument.kind() == NodeKind::NamedArgument
            || typed(node).is_none_or(|t| !parameter_element(t))
            || !crate::definitions::annotations_safe(self.context, file, written)
            || self.operation_fact(facts, file, node).is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameter_set(&parameters[0])
                        && parameter_element(return_type) && Some(return_type) == typed(node)
                        && parameters[0] == *typed(argument).unwrap()
                        && matches!(&parameters[0].kind, TypeKind::Set(element)
                            if element.kind == return_type.kind))
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "parameter set extremum signature is unsupported".into(),
            ));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        let written_domain = expression_domain(self.context, self.bindings, file, argument);
        let mut domain = &written_domain;
        while let Domain::Named { domain: inner, .. } = domain {
            domain = inner;
        }
        let empty = match domain {
            Domain::LiteralSet(values) => values.is_empty(),
            Domain::Range { lower, upper } => {
                matches!((crate::domains::invariant_integer(lower), crate::domains::invariant_integer(upper)),
                (Ok(Some(lower)), Ok(Some(upper))) if lower > upper)
            }
            Domain::Enum(id) => self.enum_extent(*id).is_some_and(|size| size == 0),
            _ => false,
        };
        Some(if empty {
            DefinitionSafety::Unsupported(
                "parameter set extremum is undefined for an empty set".into(),
            )
        } else {
            DefinitionSafety::Unknown(
                "parameter set extremum nonemptiness or value is unproved".into(),
            )
        })
    }
    fn parameter_set_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::LetExpression {
            return None;
        }
        let typed = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let integer_set = |ty: &TypeInst| {
            ty.known()
                && !optional(ty)
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if element.known() && !optional(element) && element.kind == TypeKind::Int
                        && element.instantiation == Instantiation::Parameter)
        };
        let bodies: Vec<_> = node
            .child_nodes()
            .filter(|n| n.kind() != NodeKind::LetBlock)
            .collect();
        let [body] = bodies.as_slice() else {
            return None;
        };
        if unwrap(body).kind() != NodeKind::ConditionalExpression
            || typed(node).is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
        {
            return None;
        }
        let locals: Vec<_> = node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
            .flat_map(|block| block.child_nodes())
            .collect();
        // Only initialized parameter integer-set comprehensions are admitted.
        // Other let shapes keep the existing strict Local interpreter.
        if locals.is_empty()
            || locals.iter().any(|local| {
                local.kind() != NodeKind::Declaration
                    || self
                        .bindings
                        .declarations
                        .iter()
                        .find(|d| {
                            d.file == file
                                && d.syntax_range == local.range()
                                && d.role == DeclarationRole::Local
                        })
                        .is_none_or(|d| !integer_set(&view.declarations[d.id.0].ty))
                    || local
                        .child_nodes()
                        .find(|n| is_expression(n.kind()))
                        .is_none_or(|value| unwrap(value).kind() != NodeKind::SetComprehension)
            })
        {
            return None;
        }
        let inspect = (|| -> Result<(), String> {
            let mut nodes = vec![written];
            while let Some(current) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, current) {
                    return Err("set let annotation is unsupported".into());
                }
                nodes.extend(current.child_nodes());
            }
            let mut lexical = Vec::new();
            for header in generators {
                if !crate::definitions::annotations_safe(self.context, file, header) {
                    return Err("set let generator annotation is unsupported".into());
                }
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == header.range()
                    })
                    .collect();
                if binders.is_empty()
                    || binders.len()
                        != crate::domains::generator_slots(&self.context.files[file].parsed, header)
                    || !header.children().iter().any(|c| {
                        matches!(c, SyntaxElement::Token(i)
                        if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                    })
                    || binders.iter().any(|d| {
                        let ty = &view.declarations[d.id.0].ty;
                        !ty.known()
                            || optional(ty)
                            || ty.instantiation != Instantiation::Parameter
                            || ty.kind != TypeKind::Int
                    })
                {
                    return Err("set let generator identity or type is unsupported".into());
                }
                let source = header
                    .child_nodes()
                    .next()
                    .ok_or("set let generator source is unavailable")?;
                if typed(source).is_none_or(|ty| !integer_set(ty)) {
                    return Err("set let generator source type is unsupported".into());
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, source, view, &lexical, &mut Vec::new())
                {
                    return Err(reason);
                }
                if let Some(reason) = self.closed_integer_source_error(file, source, true, true) {
                    return Err(reason);
                }
                lexical.push(*header);
                for filter in header
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let parts: Vec<_> = filter.child_nodes().collect();
                    let [condition] = parts.as_slice() else {
                        return Err("set let filter is unsupported".into());
                    };
                    if typed(condition).is_none_or(|ty| {
                        !ty.known()
                            || optional(ty)
                            || ty.instantiation != Instantiation::Parameter
                            || ty.kind != TypeKind::Bool
                    }) {
                        return Err("set let filter type is unsupported".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        condition,
                        view,
                        &lexical,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) =
                        self.closed_integer_source_error(file, condition, true, true)
                    {
                        return Err(reason);
                    }
                }
            }
            // Inspect every initializer, even unused locals. The initialized
            // source walker does not follow Local references, so reject forward
            // or cyclic local aliases before inspecting ordered initializers.
            for local in locals {
                if !crate::definitions::annotations_safe(self.context, file, local) {
                    return Err("set local annotation is unsupported".into());
                }
                self.type_dependencies(file, local, view, &lexical)?;
                let value = local
                    .child_nodes()
                    .find(|n| is_expression(n.kind()))
                    .ok_or("set local initializer is unavailable")?;
                let range = self.context.files[file].location(value.range()).range;
                if self.bindings.references.iter().any(|reference| reference.file == file
                    && range.start <= reference.location.range.start && reference.location.range.end <= range.end
                    && matches!(reference.resolution, BindingResolution::Resolved(id)
                        if self.bindings.declarations[id.0].role == DeclarationRole::Local
                            && self.bindings.declarations[id.0].syntax_range.end + self.context.files[file].byte_offset
                                > reference.location.range.start))
                { return Err("set local has a cyclic or forward source".into()); }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new())
                {
                    return Err(reason);
                }
                if let Some(reason) = self.closed_integer_source_error(file, local, true, true) {
                    return Err(reason);
                }
            }
            match self.boolean_relation_safety(file, body, view, &lexical) {
                Some(DefinitionSafety::Unsupported(reason)) => Err(reason),
                Some(_) => Ok(()),
                None => Err("set let Boolean relation inspection is unavailable".into()),
            }
        })();
        Some(match inspect {
            Err(reason) => DefinitionSafety::Unsupported(reason),
            Ok(()) => DefinitionSafety::Unknown("set let relation value is unproved".into()),
        })
    }
    fn parameter_row_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
        inspected_locals: &[DeclarationId],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        let integer = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Int
                && t.instantiation == Instantiation::Parameter
        };
        let nested_rows = generators.len() == 3 && inspected_locals.len() == 3;
        if node.kind() != NodeKind::LetExpression
            || ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool)
            || (!matches!(generators.len(), 1 | 2) && !nested_rows)
            || node
                .child_nodes()
                .filter(|n| n.kind() != NodeKind::LetBlock)
                .count()
                != 1
        {
            return None;
        }
        let body = node
            .child_nodes()
            .find(|n| n.kind() != NodeKind::LetBlock)?;
        let first = unwrap(body);
        let second = first.child_nodes().nth(1).map(unwrap);
        let nested_body = generators.len() == 1
            && inspected_locals.is_empty()
            && first.kind() == NodeKind::GeneratorCallExpression
            && second.is_some_and(|n| {
                n.kind() == NodeKind::GeneratorCallExpression
                    && n.child_nodes()
                        .nth(1)
                        .is_some_and(|inner| unwrap(inner).kind() == NodeKind::LetExpression)
            });
        let dual_rows = generators.len() == 2;
        let scoped_rows = dual_rows || nested_rows || nested_body;
        if nested_rows
            && inspected_locals.iter().any(|id| {
                let declaration = &self.bindings.declarations[id.0];
                declaration.file != file
                    || declaration.role != DeclarationRole::Local
                    || declaration.syntax_range.end > node.range().start
                    || !integer(&view.declarations[id.0].ty)
            })
        {
            return None;
        }
        let integer_set = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Set(element) if integer(element))
        };
        let integer_table = |t: &TypeInst| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && integer(element))
        };
        let mut sources = Vec::new();
        let mut tables = Vec::new();
        for (position, generator) in generators.iter().enumerate() {
            let source = generator.child_nodes().next()?;
            if (!scoped_rows && generator.child_nodes().count() != 1)
                || (nested_body && generator.child_nodes().count() != 1)
                || (nested_rows
                    && generator.child_nodes().count() != if position == 0 { 1 } else { 2 })
                || (scoped_rows
                    && generator
                        .child_nodes()
                        .skip(1)
                        .any(|n| n.kind() != NodeKind::WhereFilter))
                || !generator.children().iter().any(|c| {
                    matches!(c, SyntaxElement::Token(i)
                    if self.context.files[file].parsed.tokens()[*i].kind == TokenKind::In)
                })
                || !self.core(file, source, view, "index_set_1of2")
            {
                return None;
            }
            if scoped_rows {
                let binders: Vec<_> = self
                    .bindings
                    .declarations
                    .iter()
                    .filter(|d| {
                        d.file == file
                            && d.role == DeclarationRole::Generator
                            && d.syntax_range == generator.range()
                    })
                    .collect();
                let values: Vec<_> = source.child_nodes().collect();
                let [value] = values.as_slice() else {
                    return None;
                };
                let subject = unwrap(value);
                let array = self.reference(file, subject)?;
                let owner = &self.bindings.declarations[array.0];
                if binders.len() != 1
                    || !integer(&view.declarations[binders[0].id.0].ty)
                    || subject.kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || !(owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    || (dual_rows && tables.first().is_some_and(|previous| *previous != array))
                    || (nested_rows && position == 1 && tables[0] == array)
                    || (nested_rows && position == 2 && tables[1] != array)
                    || ty(value).is_none_or(|t| !integer_table(t))
                    || ty(source).is_none_or(|t| !integer_set(t))
                    || self.operation_fact(self.view(file, source, view), file, source).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && integer_table(&parameters[0])
                                && ty(value).is_some_and(|actual| crate::types::coerces(actual, &parameters[0]))
                                && integer_set(return_type) && Some(return_type) == ty(source)))
                {
                    return None;
                }
                tables.push(array);
            }
            sources.push(source);
        }
        let mut rows = Vec::new();
        let mut locals = Vec::new();
        let mut initializers = Vec::new();
        for block in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
        {
            for local in block.child_nodes() {
                let declaration = self.bindings.declarations.iter().find(|d| {
                    d.file == file
                        && d.role == DeclarationRole::Local
                        && d.syntax_range == local.range()
                })?;
                if local.kind() != NodeKind::Declaration
                    || !integer(&view.declarations[declaration.id.0].ty)
                {
                    return None;
                }
                let mut values = local.child_nodes().filter(|n| is_expression(n.kind()));
                let initializer = values.next()?;
                if values.next().is_some() {
                    return None;
                }
                let access = unwrap(initializer);
                let parts: Vec<_> = access.child_nodes().collect();
                if access.kind() != NodeKind::ArrayAccessExpression
                    || parts.len() != 3
                    || ty(access).is_none_or(|t| !integer(t))
                {
                    return None;
                }
                let subject = unwrap(parts[0]);
                let array = self.reference(file, subject)?;
                let owner = &self.bindings.declarations[array.0];
                let row = self.reference(file, unwrap(parts[1]))?;
                let position = generators.iter().position(|generator| {
                    self.bindings.declarations[row.0].syntax_range == generator.range()
                })?;
                let generator = generators[position];
                let source = sources[position];
                if subject.kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || !(owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    || ty(subject).is_none_or(|t| !t.known() || optional(t)
                        || t.instantiation != Instantiation::Parameter
                        || !matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 2 && indices.iter().all(integer) && integer(element)))
                    || unwrap(parts[1]).kind() != NodeKind::Expression
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, unwrap(parts[1])).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    || ty(parts[1]).is_none_or(|t| !integer(t))
                    || ty(parts[2]).is_none_or(|t| !integer(t))
                    || self.bindings.declarations[row.0].role != DeclarationRole::Generator
                    || self.bindings.declarations[row.0].syntax_range != generator.range()
                    || !matches!(crate::domains::tokens(&self.context.files[file].parsed, unwrap(parts[2])).as_slice(),
                        [token] if token.kind == TokenKind::IntegerLiteral)
                    || source.child_nodes().count() != 1
                    || source.child_nodes().next().is_none_or(|n| {
                        let n = unwrap(n);
                        n.kind() != NodeKind::Expression || self.reference(file, n) != Some(array)
                            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, n).as_slice(),
                                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    })
                {
                    return None;
                }
                if !crate::definitions::annotations_safe(self.context, file, local) {
                    return Some(DefinitionSafety::Unsupported(
                        "row local annotation is unsupported".into(),
                    ));
                }
                if let Err(reason) = self.type_dependencies(file, local, view, generators) {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                if self.domains.declarations[declaration.id.0]
                    .domain
                    .numeric_minimum()
                    .is_err()
                {
                    return Some(DefinitionSafety::Unsupported(
                        "row local domain is unsupported".into(),
                    ));
                }
                if !rows.contains(&array) {
                    rows.push(array);
                }
                locals.push(declaration.id);
                initializers.push(initializer);
            }
        }
        if initializers.is_empty()
            || nested_body && locals.len() != 3
            || nested_rows && locals.len() != 1
        {
            return None;
        }
        if !crate::definitions::annotations_safe(self.context, file, written)
            || generators.iter().any(|generator| {
                !crate::definitions::annotations_safe(self.context, file, generator)
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "row let or iteration annotation is unsupported".into(),
            ));
        }
        // Unknown cell values do not excuse a closed source-axis or element error.
        // Retain the original array identity; neither defaults nor row values are proofs.
        for array in rows {
            let mut domain = &self.domains.declarations[array.0].domain;
            while let Domain::Named {
                domain: original, ..
            } = domain
            {
                domain = original;
            }
            let Domain::Array { indices, element } = domain else {
                return Some(DefinitionSafety::Unsupported(
                    "row table domain is unavailable".into(),
                ));
            };
            if indices.len() != 2
                || indices.iter().any(|axis| axis.numeric_minimum().is_err())
                || element.numeric_minimum().is_err()
            {
                return Some(DefinitionSafety::Unsupported(
                    "row table index or element domain is unsupported".into(),
                ));
            }
        }
        // Inspect the exact selected header, every initialized local (including
        // unused ones), and the whole Boolean body in its original lexical scope.
        if scoped_rows {
            // Sources see prior headers; each filter also sees its own binder.
            // Check eager errors only after initialized source inspection.
            for (position, generator) in generators.iter().enumerate() {
                let source = sources[position];
                if let unsupported @ DefinitionSafety::Unsupported(_) = self
                    .initialized_source_safety(
                        file,
                        source,
                        view,
                        &generators[..position],
                        &mut Vec::new(),
                    )
                {
                    return Some(unsupported);
                }
                if let Some(reason) = self.closed_integer_source_error(file, source, true, true) {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
                for filter in generator
                    .child_nodes()
                    .filter(|n| n.kind() == NodeKind::WhereFilter)
                {
                    let values: Vec<_> = filter.child_nodes().collect();
                    let [condition] = values.as_slice() else {
                        return Some(DefinitionSafety::Unsupported(
                            "row filter is unsupported".into(),
                        ));
                    };
                    if !crate::definitions::annotations_safe(self.context, file, filter)
                        || ty(condition).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || t.kind != TypeKind::Bool
                                || t.instantiation != Instantiation::Parameter
                        })
                    {
                        return Some(DefinitionSafety::Unsupported(
                            "row filter type or annotation is unsupported".into(),
                        ));
                    }
                    if nested_rows {
                        let range = self.context.files[file].location(condition.range()).range;
                        if self.bindings.references.iter().any(|reference| {
                            reference.file == file && reference.kind == ReferenceKind::Value
                                && range.start <= reference.location.range.start
                                && reference.location.range.end <= range.end
                                && matches!(reference.resolution, BindingResolution::Resolved(id)
                                    if self.bindings.declarations[id.0].role == DeclarationRole::Local
                                        && !inspected_locals.contains(&id))
                        }) {
                            return Some(DefinitionSafety::Unsupported(
                                "nested row filter has an uninspected local source".into(),
                            ));
                        }
                        let condition = unwrap(condition);
                        let atoms: Vec<_> = if self.core(file, condition, view, "/\\") {
                            let parts: Vec<_> = condition.child_nodes().collect();
                            if parts.len() != 2 || parts.iter().any(|part| ty(part).is_none_or(|t|
                                !t.known() || optional(t) || t.kind != TypeKind::Bool
                                    || t.instantiation != Instantiation::Parameter)) || self.operation_fact(self.view(file, condition, view), file, condition)
                                .is_none_or(|call| !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                    if parameters.len() == 2 && parameters.iter().chain(std::iter::once(return_type))
                                        .all(|t| t.known() && !optional(t) && t.kind == TypeKind::Bool
                                            && t.instantiation == Instantiation::Parameter)
                                        && Some(return_type) == ty(condition))) {
                                return Some(DefinitionSafety::Unsupported(
                                    "nested row conjunction signature is unsupported".into(),
                                ));
                            }
                            parts
                        } else {
                            vec![condition]
                        };
                        // Both complete comparison atoms are inspected, but the
                        // closed arithmetic walk never treats a lazy AND as eager.
                        for atom in atoms {
                            let atom = unwrap(atom);
                            if atom.child_nodes().count() != 2
                                || atom.child_nodes().any(|child| ty(child).is_none_or(|t| !integer(t)))
                                || !self.core(file, atom, view, "=")
                                || self.operation_fact(self.view(file, atom, view), file, atom)
                                    .is_none_or(|call| !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                        if parameters.len() == 2 && parameters.iter().all(integer)
                                            && return_type.known() && !optional(return_type)
                                            && return_type.kind == TypeKind::Bool
                                            && return_type.instantiation == Instantiation::Parameter
                                            && Some(return_type) == ty(atom))) {
                                return Some(DefinitionSafety::Unsupported(
                                    "nested row comparison signature is unsupported".into(),
                                ));
                            }
                            if let unsupported @ DefinitionSafety::Unsupported(_) = self
                                .initialized_source_safety(
                                    file,
                                    atom,
                                    view,
                                    &generators[..=position],
                                    &mut Vec::new(),
                                )
                            {
                                return Some(unsupported);
                            }
                            if let Some(reason) =
                                self.closed_integer_source_error(file, atom, true, true)
                            {
                                return Some(DefinitionSafety::Unsupported(reason));
                            }
                        }
                    } else {
                        if let unsupported @ DefinitionSafety::Unsupported(_) = self
                            .initialized_source_safety(
                                file,
                                condition,
                                view,
                                &generators[..=position],
                                &mut Vec::new(),
                            )
                        {
                            return Some(unsupported);
                        }
                        if let Some(reason) =
                            self.closed_integer_source_error(file, condition, true, true)
                        {
                            return Some(DefinitionSafety::Unsupported(reason));
                        }
                    }
                }
            }
        } else {
            initializers.push(sources[0]);
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &initializers, view, generators)
        {
            return Some(unsupported);
        }
        if nested_body || nested_rows {
            for initializer in &initializers {
                if let Some(reason) =
                    self.closed_integer_source_error(file, initializer, true, true)
                {
                    return Some(DefinitionSafety::Unsupported(reason));
                }
            }
        }
        if nested_body {
            let quantified = |value: &'a SyntaxNode| -> Option<(&'a SyntaxNode, &'a SyntaxNode)> {
                let value = unwrap(value);
                let parts: Vec<_> = value.child_nodes().collect();
                let [headers, body] = parts.as_slice() else {
                    return None;
                };
                if value.kind() != NodeKind::GeneratorCallExpression
                    || headers.kind() != NodeKind::GeneratorList || headers.child_nodes().count() != 1
                    || !self.core(file, value, view, "forall")
                    || !crate::definitions::annotations_safe(self.context, file, value)
                    || self.operation_fact(self.view(file, value, view), file, value).is_none_or(|call|
                        !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && return_type.known() && !optional(return_type)
                                && return_type.kind == TypeKind::Bool && return_type.instantiation == Instantiation::Decision
                                && Some(return_type) == ty(value) && parameters[0].known() && !optional(&parameters[0])
                                && parameters[0].instantiation == Instantiation::Decision
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && integer(&indices[0]) && element.known() && !optional(element)
                                        && element.kind == TypeKind::Bool && element.instantiation == Instantiation::Decision
                                        && ty(body).is_some_and(|actual| actual.known() && !optional(actual)
                                            && crate::types::coerces(actual, element))))) {
                    return None;
                }
                Some((headers.child_nodes().next()?, *body))
            };
            let Some((first_header, second)) = quantified(body) else {
                return Some(DefinitionSafety::Unsupported(
                    "nested row forall signature is unsupported".into(),
                ));
            };
            let Some((second_header, inner)) = quantified(second) else {
                return Some(DefinitionSafety::Unsupported(
                    "nested row forall signature is unsupported".into(),
                ));
            };
            let mut lexical = generators.to_vec();
            lexical.extend([first_header, second_header]);
            // The inner let may use only these already fully inspected prior
            // Local identities. This is inspection state, never output facts.
            return Some(
                self.parameter_row_let_safety(file, inner, view, &lexical, &locals)
                    .unwrap_or_else(|| {
                        DefinitionSafety::Unsupported("nested row let scope is unsupported".into())
                    }),
            );
        }
        if dual_rows || nested_rows {
            let range = self.context.files[file].location(body.range()).range;
            for reference in self.bindings.references.iter().filter(|r| {
                r.file == file
                    && r.kind == ReferenceKind::Value
                    && range.start <= r.location.range.start
                    && r.location.range.end <= range.end
            }) {
                if let BindingResolution::Resolved(id) = reference.resolution
                    && self.bindings.declarations[id.0].role == DeclarationRole::Local
                    && (!locals.contains(&id) && !inspected_locals.contains(&id)
                        || self.bindings.declarations[id.0].syntax_range.end
                            + self.context.files[file].byte_offset
                            > reference.location.range.start)
                {
                    return Some(DefinitionSafety::Unsupported(
                        "row body has an uninspected or forward local source".into(),
                    ));
                }
            }
        }
        let inspected = self
            .boolean_relation_safety(file, body, view, generators)
            .unwrap_or_else(|| {
                self.initialized_source_safety(file, body, view, generators, &mut Vec::new())
            });
        Some(match inspected {
            unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
            _ => {
                DefinitionSafety::Unknown("row let selection or relation value is unproved".into())
            }
        })
    }
    // Inspect a checked local body, core forall conditional body or core
    // exists body. Numeric operands retain raw Bool-to-Int coercion partiality.
    fn boolean_relation_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        let ty = |value: &SyntaxNode| {
            self.expression_type(self.view(file, value, view), file, value)
                .map(|e| &e.ty)
        };
        if ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Bool) {
            return None;
        }
        if node.kind() == NodeKind::ConditionalExpression {
            if !crate::definitions::annotations_safe(self.context, file, written) {
                return Some(DefinitionSafety::Unsupported(
                    "Boolean conditional annotation is unsupported".into(),
                ));
            }
            return Some((|| -> DefinitionSafety {
                let branches: Vec<_> = node.child_nodes().collect();
                let mut complete = false;
                for (position, branch) in branches.iter().enumerate() {
                    if !crate::definitions::annotations_safe(self.context, file, branch) {
                        return DefinitionSafety::Unsupported(
                            "Boolean conditional branch annotation is unsupported".into(),
                        );
                    }
                    let parts: Vec<_> = branch.child_nodes().collect();
                    let branch_body =
                        if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                            let guard = parts[0];
                            if ty(guard).is_none_or(|t| {
                                !t.known()
                                    || optional(t)
                                    || t.instantiation != Instantiation::Parameter
                                    || t.kind != TypeKind::Bool
                            }) {
                                return DefinitionSafety::Unsupported(
                                    "Boolean conditional guard is unsupported".into(),
                                );
                            }
                            if let unsupported @ DefinitionSafety::Unsupported(_) = self
                                .initialized_source_safety(
                                    file,
                                    guard,
                                    view,
                                    generators,
                                    &mut Vec::new(),
                                )
                            {
                                return unsupported;
                            }
                            // Inspect eager guard sources independently of their
                            // value; never check the whole lazy conditional here.
                            if let Some(reason) =
                                self.closed_integer_source_error(file, guard, true, true)
                            {
                                return DefinitionSafety::Unsupported(reason);
                            }
                            parts[1]
                        } else if branch.kind() == NodeKind::ElseBranch
                            && parts.len() == 1
                            && position + 1 == branches.len()
                        {
                            complete = true;
                            parts[0]
                        } else {
                            return DefinitionSafety::Unsupported(
                                "Boolean conditional branch is unsupported".into(),
                            );
                        };
                    if ty(branch_body).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.kind != TypeKind::Bool
                            || !crate::types::coerces(t, ty(node).unwrap())
                    }) {
                        return DefinitionSafety::Unsupported(
                            "Boolean conditional body is unsupported".into(),
                        );
                    }
                    let inspected = self
                        .boolean_relation_safety(file, branch_body, view, generators)
                        .unwrap_or_else(|| {
                            self.initialized_source_safety(
                                file,
                                branch_body,
                                view,
                                generators,
                                &mut Vec::new(),
                            )
                        });
                    if let unsupported @ DefinitionSafety::Unsupported(_) = inspected {
                        return unsupported;
                    }
                }
                if complete && branches.len() >= 2 {
                    DefinitionSafety::Unknown(
                        "Boolean conditional relation value is unproved".into(),
                    )
                } else {
                    DefinitionSafety::Unsupported(
                        "Boolean conditional inspection requires a complete else".into(),
                    )
                }
            })());
        }
        let children: Vec<_> = node.child_nodes().collect();
        let name = operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
        let logical = match (node.kind(), name) {
            (NodeKind::BinaryExpression, Some("/\\" | "\\/" | "<->" | "->" | "<-" | "xor")) => {
                children.len() == 2
            }
            (NodeKind::UnaryExpression, Some("not")) => children.len() == 1,
            _ => false,
        };
        if logical {
            if !self.core(file, node, view, name.unwrap())
                || !crate::definitions::annotations_safe(self.context, file, written)
                || children.iter().any(|n| ty(n).is_none_or(|t|
                    !t.known() || optional(t) || t.kind != TypeKind::Bool))
                || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == children.len() && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)|
                                formal.known() && !optional(formal) && formal.kind == TypeKind::Bool
                                    && ty(actual).is_some_and(|t| crate::types::coerces(t, formal)))))
            {
                return Some(DefinitionSafety::Unsupported("Boolean relation identity or signature is unsupported".into()));
            }
            for child in children {
                let inspected = self
                    .boolean_relation_safety(file, child, view, generators)
                    .unwrap_or_else(|| {
                        self.initialized_source_safety(
                            file,
                            child,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    });
                if let unsupported @ DefinitionSafety::Unsupported(_) = inspected {
                    return Some(unsupported);
                }
            }
            return Some(DefinitionSafety::Unknown(
                "Boolean relation value is unproved".into(),
            ));
        }
        if node.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let subject = children.first().copied().map(unwrap)?;
        let array = self.reference(file, subject)?;
        if subject.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || !crate::definitions::annotations_safe(self.context, file, written)
            || ty(subject).is_none_or(|t| !t.known() || optional(t)
                || !matches!(&t.kind, TypeKind::Array { indices, element }
                    if matches!(indices.len(), 1 | 2) && children.len() == indices.len() + 1
                        && element.known() && !optional(element) && element.kind == TypeKind::Bool
                        && indices.iter().zip(&children[1..]).all(|(axis, selector)|
                            axis.known() && !optional(axis) && axis.instantiation == Instantiation::Parameter
                                && matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_))
                                && ty(selector).is_some_and(|index| index.known() && !optional(index)
                                    && index.instantiation == Instantiation::Parameter && index.kind == axis.kind))))
        {
            return Some(DefinitionSafety::Unsupported("Boolean relation selection type or source is unsupported".into()));
        }
        let mut domain = &self.domains.declarations[array.0].domain;
        while let Domain::Named {
            domain: original, ..
        } = domain
        {
            domain = original;
        }
        let Domain::Array { indices, element } = domain else {
            return Some(DefinitionSafety::Unsupported(
                "Boolean relation source domain is unavailable".into(),
            ));
        };
        if indices.len() + 1 != children.len()
            || indices.iter().any(|axis| axis.numeric_minimum().is_err())
            || element.numeric_minimum().is_err()
        {
            return Some(DefinitionSafety::Unsupported(
                "Boolean relation source domain is unsupported".into(),
            ));
        }
        for selector in &children[1..] {
            if let Some(reason) = self.closed_integer_source_error(file, selector, true, false) {
                return Some(DefinitionSafety::Unsupported(reason));
            }
        }
        // Undefined Boolean selection is false only in the checked Boolean
        // relation. Inspect its source and selectors; grant no raw membership.
        Some(
            match self.initialized_children_safety(file, &children, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => {
                    DefinitionSafety::Unknown("Boolean relation selection value is unproved".into())
                }
            },
        )
    }
    fn float_let_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::LetExpression
            || self
                .expression_type(self.view(file, node, view), file, node)
                .is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool)
        {
            return None;
        }
        let mut nodes = vec![node];
        let mut parameter_float = false;
        while let Some(current) = nodes.pop() {
            if self
                .expression_type(self.view(file, current, view), file, current)
                .is_some_and(|e| {
                    e.ty.known()
                        && !optional(&e.ty)
                        && e.ty.kind == TypeKind::Float
                        && e.ty.instantiation == Instantiation::Parameter
                })
            {
                parameter_float = true;
            }
            nodes.extend(current.child_nodes());
        }
        if !parameter_float {
            return None;
        }
        if !crate::definitions::annotations_safe(self.context, file, written) {
            return Some(DefinitionSafety::Unsupported(
                "Float let annotation is unsupported".into(),
            ));
        }
        // The Local interpreter's strict reference dependencies do not walk a
        // named global initializer. Inspect even unused local sources first;
        // nested lets perform this same check in their own lexical scope.
        for block in node
            .child_nodes()
            .filter(|n| n.kind() == NodeKind::LetBlock)
        {
            for local in block
                .child_nodes()
                .filter(|n| n.kind() == NodeKind::Declaration)
            {
                let lexical: Vec<_> = generators
                    .iter()
                    .copied()
                    .filter(|g| g.range().end <= local.range().start)
                    .collect();
                for value in local.child_nodes().filter(|n| is_expression(n.kind())) {
                    if let unsupported @ DefinitionSafety::Unsupported(_) =
                        self.initialized_source_safety(file, value, view, &lexical, &mut Vec::new())
                    {
                        return Some(unsupported);
                    }
                }
            }
        }
        let Some(item) = self.context.files[file]
            .parsed
            .tree()
            .child_nodes()
            .position(|item| {
                item.range().start <= node.range().start && node.range().end <= item.range().end
            })
        else {
            return Some(DefinitionSafety::Unsupported(
                "Float let owning item unavailable".into(),
            ));
        };
        let clause = Clause {
            file,
            item,
            node,
            generators: generators.to_vec(),
            kind: ClauseKind::Local,
        };
        let mut unavailable = Vec::new();
        // Use the existing ordered Local interpreter, and discard every temporary
        // output and inspected-local ID even when its body is supported.
        self.interpret(
            &clause,
            view,
            &[],
            &mut Vec::new(),
            &mut unavailable,
            &mut Vec::new(),
        );
        Some(match unavailable.into_iter().next() {
            Some(unavailable) => DefinitionSafety::Unsupported(unavailable.reason),
            None => {
                DefinitionSafety::Unknown("Float let initialization or value is unproved".into())
            }
        })
    }
    fn decision_scalar_bound_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression
            || !(self.core(file, node, view, "lb") || self.core(file, node, view, "ub"))
        {
            return None;
        }
        let children: Vec<_> = node.child_nodes().collect();
        let [argument] = children.as_slice() else {
            return None;
        };
        let selection = unwrap(argument);
        if selection.kind() != NodeKind::ArrayAccessExpression {
            return None;
        }
        let facts = self.view(file, node, view);
        let actual = self.expression_type(facts, file, argument).map(|e| &e.ty)?;
        if actual.instantiation != Instantiation::Decision {
            return None;
        }
        let result = self.expression_type(facts, file, node).map(|e| &e.ty);
        let subject = selection.child_nodes().next().map(unwrap);
        let array = subject.and_then(|n| self.reference(file, n));
        if argument.kind() == NodeKind::NamedArgument
            || !actual.known() || optional(actual) || actual.kind != TypeKind::Int
            || result.is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int
                || t.instantiation != Instantiation::Parameter)
            || subject.is_none_or(|n| n.kind() != NodeKind::Expression
                || !matches!(crate::domains::tokens(&self.context.files[file].parsed, n).as_slice(),
                    [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
            || array.is_none_or(|id| self.bindings.declarations[id.0].role != DeclarationRole::Value
                || !self.bindings.declarations[id.0].top_level)
            || !crate::definitions::annotations_safe(self.context, file, written)
            || !self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                    if parameters.len() == 1 && parameters[0] == *actual && Some(return_type) == result)
            })
        {
            return Some(DefinitionSafety::Unsupported("scalar reflection selected operand or signature is unsupported".into()));
        }
        if let unsupported @ DefinitionSafety::Unsupported(_) =
            self.initialized_children_safety(file, &children, view, generators)
        {
            return Some(unsupported);
        }
        let owner = &self.bindings.declarations[array.unwrap().0];
        if let Some(written) = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) && written
            .child_nodes()
            .find(|n| is_expression(n.kind()))
            .map(unwrap)
            .is_some_and(|value| {
                value.kind() == NodeKind::ArrayLiteral && value.child_nodes().next().is_none()
            })
        {
            return Some(DefinitionSafety::Unsupported(
                "scalar reflection source is empty".into(),
            ));
        }
        let mut domains = vec![&self.domains.declarations[owner.id.0].domain];
        while let Some(domain) = domains.pop() {
            match domain {
                Domain::Named { domain, .. } => domains.push(domain),
                Domain::Array { indices, element } => {
                    domains.extend(indices);
                    domains.push(element);
                }
                Domain::Range { lower, upper }
                    if matches!((crate::domains::invariant_integer(lower), crate::domains::invariant_integer(upper)),
                        (Ok(Some(lower)), Ok(Some(upper))) if upper < lower) =>
                {
                    return Some(DefinitionSafety::Unsupported(
                        "scalar reflection source domain is empty".into(),
                    ));
                }
                Domain::LiteralSet(values) if values.is_empty() => {
                    return Some(DefinitionSafety::Unsupported(
                        "scalar reflection source domain is empty".into(),
                    ));
                }
                _ => {}
            }
        }
        Some(DefinitionSafety::Unknown(
            "reflected scalar bound is unproved".into(),
        ))
    }
    fn integer_array_set_safety(
        &self,
        file: FileId,
        written: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Option<DefinitionSafety> {
        let node = unwrap(written);
        if node.kind() != NodeKind::CallExpression || !self.core(file, node, view, "array2set") {
            return None;
        }
        let integer = TypeInst::par(TypeKind::Int);
        let array = TypeInst::par(TypeKind::Array {
            indices: vec![integer.clone()],
            element: Box::new(integer.clone()),
        });
        let set = TypeInst::par(TypeKind::Set(Box::new(integer)));
        if self
            .operation_fact(self.view(file, node, view), file, node)
            .is_none_or(|call| {
                !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                if parameters.as_slice() == [array.clone()] && *return_type == set)
            })
        {
            return None;
        }
        let checked = (|| -> Result<(), String> {
            let children: Vec<_> = node.child_nodes().collect();
            let [argument] = children.as_slice() else {
                return Err("array2set argument correspondence unsupported".into());
            };
            if self
                .expression_type(self.view(file, node, view), file, node)
                .is_none_or(|e| e.ty != set)
                || self
                    .expression_type(self.view(file, argument, view), file, argument)
                    .map(|e| &e.ty)
                    .or_else(|| {
                        self.reference(file, unwrap(argument))
                            .map(|id| &self.view(file, argument, view).declarations[id.0].ty)
                    })
                    .is_none_or(|ty| *ty != array)
            {
                return Err(
                    "array2set requires selected present parameter integer array and set".into(),
                );
            }
            let mut nodes = vec![written];
            while let Some(node) = nodes.pop() {
                if !crate::definitions::annotations_safe(self.context, file, node) {
                    return Err("array2set source annotation is unsupported".into());
                }
                nodes.extend(node.child_nodes());
            }
            if let DefinitionSafety::Unsupported(reason) =
                self.initialized_source_safety(file, argument, view, generators, &mut Vec::new())
            {
                return Err(reason);
            }
            Ok(())
        })();
        Some(match checked {
            Ok(()) => DefinitionSafety::Unknown(
                "array2set membership and cardinality are unproved".into(),
            ),
            Err(reason) => DefinitionSafety::Unsupported(reason),
        })
    }
    fn initialized_children_safety(
        &self,
        file: FileId,
        nodes: &[&'a SyntaxNode],
        view: &CallableFacts,
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
    // These two rank-two metadata views may inspect a selected owning formal,
    // but its default, written type and annotations still evaluate independently.
    fn rank_two_formal_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<(), String> {
        let node = unwrap(node);
        let id = self
            .reference(file, node)
            .ok_or("rank-two formal source identity is unavailable")?;
        let declaration = &self.bindings.declarations[id.0];
        let range = self.context.files[file].location(node.range()).range;
        let callable = self
            .bindings
            .references
            .iter()
            .find(|reference| {
                reference.file == file
                    && reference.kind == ReferenceKind::Value
                    && range.start <= reference.location.range.start
                    && reference.location.range.end <= range.end
                    && reference.resolution == BindingResolution::Resolved(id)
            })
            .and_then(|reference| reference.callable)
            .ok_or("rank-two formal source owner is unavailable")?;
        let signature = self
            .calls
            .signatures
            .iter()
            .find(|signature| signature.declaration == callable)
            .ok_or("rank-two formal signature is unavailable")?;
        let position = (0..signature.parameters.len())
            .find(|position| {
                formal_parameter(self.context, self.bindings, callable, *position) == Some(id)
            })
            .ok_or("rank-two formal correspondence is unavailable")?;
        let facts = self.view(file, node, view);
        if declaration.file != file
            || declaration.role != DeclarationRole::Parameter
            || signature.parameters[position].has_default
            || self
                .expression_type(facts, file, node)
                .is_none_or(|expression| expression.ty != facts.declarations[id.0].ty)
        {
            return Err("rank-two formal type or default is unsupported".into());
        }
        let written = find_node(
            self.context.files[file].parsed.tree(),
            &declaration.syntax_range,
            declaration.role,
        )
        .ok_or("rank-two formal declaration is unavailable")?;
        if written
            .child_nodes()
            .any(|child| is_expression(child.kind()))
        {
            return Err("rank-two formal initializer is unsupported".into());
        }
        let mut pending = vec![written];
        while let Some(node) = pending.pop() {
            if !crate::definitions::annotations_safe(self.context, file, node) {
                return Err("rank-two formal annotation is unsupported".into());
            }
            if node.kind() == NodeKind::DomainType {
                for source in node.child_nodes() {
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        source,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) = self.closed_integer_source_error(file, source, true, true)
                    {
                        return Err(reason);
                    }
                }
            }
            pending.extend(node.child_nodes());
        }
        self.type_dependencies(file, written, view, generators)?;
        if let Some(reason) = self.closed_integer_source_error(file, written, true, false) {
            return Err(reason);
        }
        Ok(())
    }
    fn direct_safety(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> DefinitionSafety {
        if let Some(safety) = self.integer_array_set_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.parameter_float_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.float_let_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.parameter_set_let_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.full_axis_slice_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(arguments) = self.set_axis_integer_reshape_arguments(file, node, view) {
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
        if let Some(safety) = self.decision_array_bound_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.decision_scalar_bound_safety(file, node, view, generators) {
            return safety;
        }
        // A strict dependency on a named collection does not inspect its
        // initializer. Keep that evaluation check ahead of strict success.
        let aggregate = self.aggregate_source_safety(file, node, view, generators);
        let reason = match self.dependencies(file, node, view, generators) {
            Ok(_) => return aggregate.unwrap_or(DefinitionSafety::Supported),
            Err(reason) => reason,
        };
        if let Some(safety) = self.decision_set_source_safety(file, node, view, generators) {
            return safety;
        }
        if let Some(safety) = self.parameter_row_let_safety(file, node, view, generators, &[]) {
            return safety;
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
                && !self.core(file, unwrap(node), view, "sum")
            {
                DefinitionSafety::Unknown("initialized collection cardinality is unproved".into())
            } else {
                safety
            };
        }
        let written = node;
        let node = unwrap(node);
        if self.default_collection(file, node) {
            return match self.default_collection_arguments(file, node, view, generators) {
                Ok((_, fallback, all)) => self.direct_safety(file, fallback, view, &all),
                Err(reason) => DefinitionSafety::Unsupported(reason),
            };
        }
        let ty = |node: &SyntaxNode| {
            self.expression_type(self.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if ty(node).is_some_and(|t| {
            matches!(&t.kind, TypeKind::Array { element, .. }
                if matches!(element.kind, TypeKind::Set(_)))
        }) && let Some(arguments) = array_concatenation(
            self.context,
            self.bindings,
            self.view(file, node, view),
            file,
            node,
        ) {
            if !crate::definitions::annotations_safe(self.context, file, written) {
                return DefinitionSafety::Unsupported(
                    "set-array concatenation annotation is unsupported".into(),
                );
            }
            let sources = self.initialized_children_safety(file, &arguments, view, generators);
            for argument in arguments {
                if let Some(reason) = self.closed_integer_source_error(file, argument, true, true) {
                    return DefinitionSafety::Unsupported(reason);
                }
            }
            return match sources {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown(
                    "parameter set-array concatenation extent and values are unproved".into(),
                ),
            };
        }
        if node.kind() == NodeKind::ArrayLiteral
            && let Some(array) = ty(node)
            && array.known()
            && !optional(array)
            && let TypeKind::Array { indices, element } = &array.kind
            && indices.len() == 1
            && indices[0].known()
            && !optional(&indices[0])
            && indices[0].instantiation == Instantiation::Parameter
            && indices[0].kind == TypeKind::Int
            && element.known()
            && !optional(element)
            && element.kind == TypeKind::Int
            && crate::definitions::annotations_safe(self.context, file, written)
        {
            let cells: Vec<_> = node.child_nodes().collect();
            if cells.iter().all(|cell| {
                cell.kind() != NodeKind::IndexedArrayEntry
                    && ty(cell).is_some_and(|t| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && crate::types::coerces(t, element)
                    })
            }) {
                if let unsupported @ DefinitionSafety::Unsupported(_) =
                    self.initialized_children_safety(file, &cells, view, generators)
                {
                    return unsupported;
                }
                for cell in cells {
                    if let Some(reason) = self.closed_integer_source_error(file, cell, true, true) {
                        return DefinitionSafety::Unsupported(reason);
                    }
                }
                // Inspecting each written cell does not prove its dependencies.
                return DefinitionSafety::Unknown("integer array literal value is unproved".into());
            }
        }
        if node.kind() == NodeKind::CallExpression
            && (self.core(file, node, view, "index_set_1of2")
                || self.core(file, node, view, "index_set_2of2"))
        {
            let children: Vec<_> = node.child_nodes().collect();
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            let array = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { indices, element }
                    if indices.len() == 2 && indices.iter().all(integer) && integer(element))
            };
            let set = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Set(element) if integer(element))
            };
            let subject = children.first().copied().map(unwrap);
            let owner = subject.and_then(|n| self.reference(file, n));
            if children.len() != 1
                || subject.is_none_or(|n| n.kind() != NodeKind::Expression)
                || owner.is_none_or(|id| {
                    let declaration = &self.bindings.declarations[id.0];
                    !(declaration.role == DeclarationRole::Value && declaration.top_level
                        || self.core(file, node, view, "index_set_2of2")
                            && declaration.role == DeclarationRole::Parameter)
                })
                || !crate::definitions::annotations_safe(self.context, file, written)
                || ty(children[0]).is_none_or(|t| !array(t))
                || ty(node).is_none_or(|t| !set(t))
                || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call|
                    !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && array(&parameters[0])
                            && ty(children[0]).is_some_and(|actual| crate::types::coerces(actual, &parameters[0]))
                            && Some(return_type) == ty(node)))
            {
                return DefinitionSafety::Unsupported("rank-two integer index-set source is unsupported".into());
            }
            if owner.is_some_and(|id| {
                self.bindings.declarations[id.0].role == DeclarationRole::Parameter
            }) && let Err(reason) =
                self.rank_two_formal_safety(file, children[0], view, generators)
            {
                return DefinitionSafety::Unsupported(reason);
            }
            return match self.initialized_source_safety(
                file,
                children[0],
                view,
                generators,
                &mut Vec::new(),
            ) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("rank-two index-set membership is unproved".into()),
            };
        }
        if node.kind() == NodeKind::CallExpression && self.core(file, node, view, "index2int") {
            let children: Vec<_> = node.child_nodes().collect();
            let integer = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && t.kind == TypeKind::Int
            };
            let array = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.instantiation == Instantiation::Parameter
                    && matches!(&t.kind, TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(integer) && integer(element))
            };
            let subject = children.first().copied().map(unwrap);
            let owner = subject.and_then(|node| self.reference(file, node));
            if children.len() == 1
                && subject.is_some_and(|node| node.kind() == NodeKind::Expression)
                && owner.is_some_and(|id| {
                    let declaration = &self.bindings.declarations[id.0];
                    declaration.role == DeclarationRole::Parameter
                        || declaration.role == DeclarationRole::Value && declaration.top_level
                })
                && crate::definitions::annotations_safe(self.context, file, written)
                && ty(children[0]).is_some_and(array)
                && ty(node) == ty(children[0])
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call|
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 1 && Some(&parameters[0]) == ty(children[0])
                            && Some(return_type) == ty(node)))
            {
                if owner.is_some_and(|id| {
                    self.bindings.declarations[id.0].role == DeclarationRole::Parameter
                }) && let Err(reason) = self.rank_two_formal_safety(file, children[0], view, generators)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                return match self.initialized_source_safety(
                    file,
                    children[0],
                    view,
                    generators,
                    &mut Vec::new(),
                ) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("rank-two integer conversion value is unproved".into()),
                };
            }
        }
        if let Some(safety) =
            self.parameter_test_safety(file, written, view, generators, &mut Vec::new())
        {
            return safety;
        }
        if node.kind() == NodeKind::ConditionalExpression
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (matches!(t.kind, TypeKind::Int | TypeKind::Bool)
                        || t.instantiation == Instantiation::Parameter
                            && matches!(&t.kind, TypeKind::Set(element)
                            if element.known() && !optional(element)
                                && element.instantiation == Instantiation::Parameter
                                && element.kind == TypeKind::Int))
            })
        {
            let result = ty(node).unwrap();
            let branches: Vec<_> = node.child_nodes().collect();
            let mut values = Vec::new();
            let mut complete = false;
            for (position, branch) in branches.iter().enumerate() {
                let parts: Vec<_> = branch.child_nodes().collect();
                let body = if branch.kind() == NodeKind::ConditionalBranch && parts.len() == 2 {
                    if ty(parts[0]).is_none_or(|t| {
                        !t.known()
                            || optional(t)
                            || t.kind != TypeKind::Bool
                            || t.instantiation != Instantiation::Parameter
                    }) {
                        return DefinitionSafety::Unsupported(
                            "conditional inspection guard is unsupported".into(),
                        );
                    }
                    values.push(parts[0]);
                    parts[1]
                } else if branch.kind() == NodeKind::ElseBranch
                    && parts.len() == 1
                    && position + 1 == branches.len()
                {
                    complete = true;
                    parts[0]
                } else {
                    return DefinitionSafety::Unsupported(
                        "conditional inspection branch is unsupported".into(),
                    );
                };
                if ty(body).is_none_or(|t| {
                    !t.known()
                        || optional(t)
                        || (t.kind != result.kind
                            && !(matches!(&result.kind, TypeKind::Set(_))
                                && unwrap(body).kind() == NodeKind::SetLiteral
                                && unwrap(body).child_nodes().count() == 0
                                && t.instantiation == Instantiation::Parameter
                                && matches!(&t.kind, TypeKind::Set(element)
                                    if element.known() && !optional(element)
                                        && element.instantiation == Instantiation::Parameter
                                        && element.kind == TypeKind::Bottom)))
                        || !crate::types::coerces(t, result)
                }) {
                    return DefinitionSafety::Unsupported(
                        "conditional inspection body is unsupported".into(),
                    );
                }
                values.push(body);
            }
            if !complete
                || values.len() < 3
                || !crate::definitions::annotations_safe(self.context, file, written)
            {
                return DefinitionSafety::Unsupported(
                    "conditional inspection requires a safe complete else".into(),
                );
            }
            if self.selected_set_source && branches.len() == 2 && values.len() == 3 {
                let inactive = match self.assertion_literal(file, values[0]) {
                    Some(TokenKind::False) => Some(1),
                    Some(TokenKind::True) => Some(2),
                    _ => None,
                };
                let mut unsupported = None;
                for (position, value) in values.iter().enumerate() {
                    let checked = if inactive == Some(position) {
                        let producer = Producer {
                            context: self.context,
                            bindings: self.bindings,
                            calls: self.calls,
                            instantiations: self.instantiations,
                            domains: self.domains,
                            lookups: None,
                            instances: Vec::new(),
                            boundaries: Vec::new(),
                            selected_set_source: true,
                            inactive_integer_body: Some((file, value.range())),
                        };
                        producer.initialized_source_safety(
                            file,
                            value,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    } else {
                        self.initialized_source_safety(
                            file,
                            value,
                            view,
                            generators,
                            &mut Vec::new(),
                        )
                    };
                    if let DefinitionSafety::Unsupported(reason) = checked {
                        unsupported = Some(reason);
                    }
                }
                return match unsupported {
                    Some(reason) => DefinitionSafety::Unsupported(reason),
                    None => DefinitionSafety::Unknown("conditional value is unproved".into()),
                };
            }
            return match self.initialized_children_safety(file, &values, view, generators) {
                unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                _ => DefinitionSafety::Unknown("conditional value is unproved".into()),
            };
        }
        if node.kind() == NodeKind::ArrayAccessExpression
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (matches!(t.kind, TypeKind::Int | TypeKind::Bool | TypeKind::Enum(_))
                        || t.kind == TypeKind::Float && t.instantiation == Instantiation::Parameter)
            })
        {
            let children: Vec<_> = node.child_nodes().collect();
            if let Some(subject) = children.first().copied().map(unwrap)
                && subject.kind() == NodeKind::Expression
                && let Some(array) = self.reference(file, subject)
                && let Some(source) = ty(subject)
                && source.known()
                && !optional(source)
                && let TypeKind::Array { indices, element } = &source.kind
                && matches!(indices.len(), 1 | 2)
                && children.len() == indices.len() + 1
                && element.known()
                && !optional(element)
                && Some(&element.kind) == ty(node).map(|t| &t.kind)
                && (element.kind != TypeKind::Float
                    || indices.len() == 1
                        && source.instantiation == Instantiation::Parameter
                        && element.instantiation == Instantiation::Parameter
                        && self.bindings.declarations[array.0].role == DeclarationRole::Value
                        && self.bindings.declarations[array.0].top_level
                        && matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                            [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier)))
                && indices.iter().zip(&children[1..]).enumerate().all(
                    |(position, (axis, selector))| {
                        axis.known()
                            && !optional(axis)
                            && axis.kind == TypeKind::Int
                            && axis.instantiation == Instantiation::Parameter
                            && ty(selector).is_some_and(|t| {
                                t.known()
                                    && !optional(t)
                                    && t.kind == TypeKind::Int
                                    && (t.instantiation == Instantiation::Parameter
                                        || indices.len() == 2
                                            && position == 1
                                            && element.kind == TypeKind::Int
                                            && t.instantiation == Instantiation::Decision)
                            })
                    },
                )
            {
                if let unsupported @ DefinitionSafety::Unsupported(_) =
                    self.initialized_children_safety(file, &children, view, generators)
                {
                    return unsupported;
                }
                let inspected_selection = indices.len() == 2
                    && (matches!(element.kind, TypeKind::Enum(_))
                        || ty(children[2])
                            .is_some_and(|t| t.instantiation == Instantiation::Decision));
                if inspected_selection {
                    if !crate::definitions::annotations_safe(self.context, file, written) {
                        return DefinitionSafety::Unsupported(
                            "scalar selection annotation is unsupported".into(),
                        );
                    }
                    let (axes, source_element) = match crate::domains::bare_index_domain(
                        &self.domains.declarations[array.0].domain,
                    ) {
                        Domain::Array { indices, element }
                            if indices.len() == children.len() - 1 =>
                        {
                            (indices, element)
                        }
                        Domain::Unsupported(reason) => {
                            return DefinitionSafety::Unsupported(reason.clone());
                        }
                        _ => {
                            return DefinitionSafety::Unsupported(
                                "scalar selection source domain is unsupported".into(),
                            );
                        }
                    };
                    // The closed source checker does not follow decision arrays.
                    // Inspect every retained axis and element before uncertainty.
                    for domain in axes.iter().chain(std::iter::once(source_element.as_ref())) {
                        if let Err(reason) = domain.numeric_minimum() {
                            return DefinitionSafety::Unsupported(reason.into());
                        }
                    }
                    for selector in &children[1..] {
                        if let Some(reason) =
                            self.closed_integer_source_error(file, selector, false, false)
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                    }
                }
                if matches!(element.kind, TypeKind::Enum(_)) && !inspected_selection {
                    if let Domain::Array { indices, .. } =
                        &self.domains.declarations[array.0].domain
                        && indices.iter().any(|axis| axis.numeric_minimum().is_err())
                    {
                        return DefinitionSafety::Unsupported(
                            "enum array source index domain is unsupported".into(),
                        );
                    }
                    for selector in &children[1..] {
                        if let Some(reason) =
                            self.closed_integer_source_error(file, selector, false, false)
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                    }
                }
                if let Domain::Array { indices, .. } = &self.domains.declarations[array.0].domain
                    && indices.iter().zip(&children[1..]).any(|(axis, selector)| {
                        crate::domains::invariant_expression_integer(
                            self.context,
                            self.bindings,
                            file,
                            selector,
                        )
                        .is_ok_and(|value| {
                            value.is_some_and(|value| {
                                crate::domains::index_domain_member(axis, value) == Some(false)
                            })
                        })
                    })
                {
                    return DefinitionSafety::Unsupported(
                        "scalar selection is outside its declared index set".into(),
                    );
                }
                let owner = &self.bindings.declarations[array.0];
                if element.kind == TypeKind::Bool
                    && matches!(crate::domains::tokens(&self.context.files[file].parsed, subject).as_slice(),
                        [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
                    && (owner.role == DeclarationRole::Parameter
                        || owner.role == DeclarationRole::Value && owner.top_level)
                    && let Some(written) = find_node(
                        self.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    )
                    && !written.child_nodes().any(|node| is_expression(node.kind()))
                    && !self.literal_bool_index_membership(
                        file,
                        node,
                        array,
                        self.view(file, node, view),
                    )
                {
                    return DefinitionSafety::Unsupported(
                        "Boolean selection requires an index-membership proof".into(),
                    );
                }
                return DefinitionSafety::Unknown("scalar selection membership is unproved".into());
            }
        }
        if node.kind() == NodeKind::BinaryExpression
            && operator(self.context, file, node).and_then(crate::bindings::symbolic_operator)
                == Some("<=")
            && self.core(file, node, view, "<=")
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Bool
                    && t.instantiation == Instantiation::Parameter
            })
        {
            let children: Vec<_> = node.child_nodes().collect();
            let float = |t: &TypeInst| {
                t.known()
                    && !optional(t)
                    && t.kind == TypeKind::Float
                    && t.instantiation == Instantiation::Parameter
            };
            if children.len() == 2
                && crate::definitions::annotations_safe(self.context, file, written)
                && children.iter().all(|child| ty(child).is_some_and(float))
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters.iter().all(float)
                            && Some(return_type) == ty(node))
                })
            {
                return match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("parameter Float comparison value is unproved".into()),
                };
            }
        }
        if ty(node).is_some_and(|t| {
            t.known()
                && !optional(t)
                && t.kind == TypeKind::Bool
                && t.instantiation == Instantiation::Decision
        }) {
            let children: Vec<_> = node.child_nodes().collect();
            let name =
                operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
            if node.kind() == NodeKind::BinaryExpression
                && name == Some("in")
                && children.len() == 2
                && self.core(file, node, view, "in")
                && crate::definitions::annotations_safe(self.context, file, written)
                && let (Some(value), Some(choices)) = (ty(children[0]), ty(children[1]))
                && value.known()
                && !optional(value)
                && value.instantiation == Instantiation::Parameter
                && value.kind == TypeKind::Int
                && decision_integer_set(choices)
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters[0].known() && !optional(&parameters[0])
                            && parameters[0].instantiation == Instantiation::Decision
                            && parameters[0].kind == TypeKind::Int && parameters[1] == *choices
                            && crate::types::coerces(value, &parameters[0])
                            && Some(return_type) == ty(node))
                })
            {
                let mut sources = children.clone();
                for generator in generators {
                    for source in generator.child_nodes() {
                        let source = if source.kind() == NodeKind::WhereFilter {
                            let Some(condition) = source.child_nodes().next() else {
                                return DefinitionSafety::Unsupported(
                                    "membership iteration filter unavailable".into(),
                                );
                            };
                            condition
                        } else {
                            source
                        };
                        sources.push(source);
                    }
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &sources, view, generators)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                for source in sources {
                    if let Some(reason) = self.closed_integer_source_error(file, source, true, true) {
                        return DefinitionSafety::Unsupported(reason);
                    }
                }
                return DefinitionSafety::Unknown("integer membership value is unproved".into());
            }
            if node.kind() == NodeKind::BinaryExpression
                && name == Some("in")
                && children.len() == 2
                && self.core(file, node, view, "in")
                && crate::definitions::annotations_safe(self.context, file, written)
                && let (Some(value), Some(choices)) = (ty(children[0]), ty(children[1]))
                && value.known()
                && !optional(value)
                && value.instantiation == Instantiation::Decision
                && value.kind == TypeKind::Int
                && choices.known()
                && !optional(choices)
                && choices.instantiation == Instantiation::Parameter
                && matches!(&choices.kind, TypeKind::Set(element)
                    if element.known() && !optional(element)
                        && element.instantiation == Instantiation::Parameter
                        && element.kind == TypeKind::Int)
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && parameters[0] == *value
                            && parameters[1] == *choices && Some(return_type) == ty(node))
                })
            {
                return match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("integer membership value is unproved".into()),
                };
            }
            if node.kind() == NodeKind::BinaryExpression
                && matches!(name, Some("=" | "!=")) && children.len() == 2
                && self.core(file, node, view, name.unwrap())
                && crate::definitions::annotations_safe(self.context, file, written)
                && let (Some(left), Some(right)) = (ty(children[0]), ty(children[1]))
                && left.known() && !optional(left) && right.known() && !optional(right)
                && matches!(left.kind, TypeKind::Enum(_)) && left.kind == right.kind
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call|
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == 2 && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)|
                                formal.known() && !optional(formal) && formal.kind == left.kind
                                    && ty(actual).is_some_and(|actual| crate::types::coerces(actual, formal)))))
            {
                return match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("enum equality value is unproved".into()),
                };
            }
            let operand_kind = match (node.kind(), name) {
                (NodeKind::BinaryExpression, Some("=" | "!=" | "<" | "<=" | ">" | ">="))
                    if children.len() == 2 =>
                {
                    Some(TypeKind::Int)
                }
                (NodeKind::BinaryExpression, Some("/\\" | "\\/" | "<->" | "->" | "<-"))
                    if children.len() == 2 =>
                {
                    Some(TypeKind::Bool)
                }
                (NodeKind::UnaryExpression, Some("not")) if children.len() == 1 => {
                    Some(TypeKind::Bool)
                }
                _ => None,
            };
            if let Some(kind) = operand_kind
                && self.core(file, node, view, name.unwrap())
                && crate::definitions::annotations_safe(self.context, file, written)
                && children.iter().all(|n| ty(n).is_some_and(|t| t.known() && !optional(t) && t.kind == kind))
                && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call| {
                    matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                        if parameters.len() == children.len() && Some(return_type) == ty(node)
                            && parameters.iter().zip(&children).all(|(formal, actual)| formal.known()
                                && !optional(formal) && formal.kind == kind
                                && ty(actual).is_some_and(|t| crate::types::coerces(t, formal))))
                }) {
                return match self.initialized_children_safety(file, &children, view, generators) {
                    unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                    _ => DefinitionSafety::Unknown("Boolean operation value is unproved".into()),
                };
            }
        }
        if (matches!(
            node.kind(),
            NodeKind::ArrayComprehension | NodeKind::SetComprehension
        ) || node.kind() == NodeKind::GeneratorCallExpression
            && (self.core(file, node, view, "forall")
                || self.core(file, node, view, "exists")
                || self.core(file, node, view, "sum")))
            && ty(node).is_some_and(|t| {
                t.known()
                    && !optional(t)
                    && (t.instantiation == Instantiation::Parameter
                        || t.kind == TypeKind::Bool
                        || matches!(&t.kind, TypeKind::Array { element, .. }
                        if element.kind == TypeKind::Bool
                            || node.kind() == NodeKind::ArrayComprehension
                                && (element.kind == TypeKind::Int || decision_integer_set(element)))
                        || node.kind() == NodeKind::GeneratorCallExpression
                            && self.core(file, node, view, "sum"))
            })
        {
            if !crate::definitions::annotations_safe(self.context, file, written) {
                return DefinitionSafety::Unsupported(
                    "parameter collection annotation is unsupported".into(),
                );
            }
            return self.collection_construction_safety(file, node, view, generators);
        }
        if node.kind() == NodeKind::ArrayComprehension {
            return self.integer_comprehension_safety(file, node, view, generators);
        }
        if ty(node).is_some_and(|t| {
            t.known()
                && !optional(t)
                && t.instantiation == Instantiation::Parameter
                && (t.kind == TypeKind::Bool
                    || matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int))
        }) {
            let children: Vec<_> = node.child_nodes().collect();
            if !crate::definitions::annotations_safe(self.context, file, written) {
                return DefinitionSafety::Unsupported(
                    "parameter collection operation annotation is unsupported".into(),
                );
            }
            let parameter_int = |node: &SyntaxNode| {
                ty(node).is_some_and(|t| {
                    t.known()
                        && !optional(t)
                        && t.instantiation == Instantiation::Parameter
                        && t.kind == TypeKind::Int
                })
            };
            let parameter_set = |node: &SyntaxNode| {
                ty(node).is_some_and(|t| t.known() && !optional(t)
                && t.instantiation == Instantiation::Parameter && matches!(&t.kind, TypeKind::Set(element) if element.kind == TypeKind::Int))
            };
            let parameter_bool = |node: &SyntaxNode| {
                ty(node).is_some_and(|t| {
                    t.known()
                        && !optional(t)
                        && t.instantiation == Instantiation::Parameter
                        && t.kind == TypeKind::Bool
                })
            };
            let checked_children = || {
                let mut safety = DefinitionSafety::Supported;
                let mut unsupported = None;
                for child in &children {
                    match self.initialized_source_safety(
                        file,
                        child,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Unsupported(reason) => unsupported = Some(reason),
                        unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                        DefinitionSafety::Supported => {}
                    }
                }
                match unsupported {
                    Some(reason) => DefinitionSafety::Unsupported(reason),
                    None => safety,
                }
            };
            if node.kind() == NodeKind::SetLiteral && children.iter().all(|n| parameter_int(n)) {
                return checked_children();
            }
            if node.kind() == NodeKind::ArrayAccessExpression && children.len() == 2 {
                let subject = unwrap(children[0]);
                if subject.kind() == NodeKind::Expression && self.reference(file, subject).is_some()
                    && parameter_int(children[1])
                    && ty(subject).is_some_and(|t| t.known() && !optional(t) && t.instantiation == Instantiation::Parameter
                        && matches!(&t.kind, TypeKind::Array { indices, element }
                            if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                && indices[0].instantiation == Instantiation::Parameter && indices[0].kind == TypeKind::Int
                                && Some(&element.kind) == ty(node).map(|t| &t.kind)))
                {
                    return match checked_children() {
                        unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                        _ => DefinitionSafety::Unknown("parameter selection membership is unproved".into()),
                    };
                }
            }
            let name =
                operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
            let supported = match node.kind() {
                NodeKind::RangeExpression => {
                    children.len() == 2
                        && name == Some("..")
                        && children.iter().all(|n| parameter_int(n))
                }
                NodeKind::BinaryExpression if children.len() == 2 => match name {
                    Some("intersect" | "union" | "diff") => {
                        children.iter().all(|n| parameter_set(n))
                    }
                    Some("in") => parameter_int(children[0]) && parameter_set(children[1]),
                    Some("=" | "!=") => {
                        children.iter().all(|n| parameter_set(n))
                            || children.iter().all(|n| parameter_int(n))
                    }
                    Some("<" | "<=" | ">" | ">=") => children.iter().all(|n| parameter_int(n)),
                    Some("/\\" | "\\/" | "<->" | "->" | "<-") => {
                        children.iter().all(|n| parameter_bool(n))
                    }
                    _ => false,
                },
                NodeKind::UnaryExpression => {
                    children.len() == 1 && name == Some("not") && parameter_bool(children[0])
                }
                _ => false,
            };
            if supported && self.core(file, node, view, name.unwrap()) {
                return checked_children();
            }
            return DefinitionSafety::Unsupported(reason);
        }
        if ty(node).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int) {
            return DefinitionSafety::Unsupported(reason);
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => {
                if let Some(id) = self.reference(file, node) {
                    let declaration = &self.bindings.declarations[id.0];
                    if declaration.role == DeclarationRole::Local
                        && declaration.syntax_range.end <= node.range().start
                        && view.declarations[id.0].ty.instantiation == Instantiation::Parameter
                        && let Some(local) = find_node(
                            self.context.files[file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                        && crate::definitions::annotations_safe(self.context, file, local)
                        && let Some(value) = local.child_nodes().find(|n| is_expression(n.kind()))
                    {
                        let lexical: Vec<_> = generators
                            .iter()
                            .copied()
                            .filter(|g| g.range().end <= declaration.syntax_range.start)
                            .collect();
                        return self.direct_safety(file, value, view, &lexical);
                    }
                }
                DefinitionSafety::Unsupported(reason)
            }
            NodeKind::ConditionalExpression => {
                match self.integer_conditional_values(file, node, view) {
                    Ok(values) => self.direct_children_safety(file, &values, view, generators),
                    Err(reason) => DefinitionSafety::Unsupported(reason),
                }
            }
            NodeKind::ArrayAccessExpression => {
                let Some(subject) = children.first().copied().map(unwrap) else {
                    return DefinitionSafety::Unsupported(reason);
                };
                let Some(array) = ty(subject) else {
                    return DefinitionSafety::Unsupported(reason);
                };
                let TypeKind::Array { indices, element } = &array.kind else {
                    return DefinitionSafety::Unsupported(reason);
                };
                if subject.kind() != NodeKind::Expression
                    || self.reference(file, subject).is_none()
                    || !array.known()
                    || optional(array)
                    || element.kind != TypeKind::Int
                    || !matches!(indices.len(), 1..=3)
                    || children.len() != indices.len() + 1
                    || indices.iter().zip(&children[1..]).enumerate().any(
                        |(position, (index, actual))| {
                            !index.known()
                                || optional(index)
                                || index.instantiation != Instantiation::Parameter
                                || ty(actual).is_none_or(|t| {
                                    !t.known()
                                        || optional(t)
                                        || (t.instantiation != Instantiation::Parameter
                                            && !(indices.len() == 3
                                                && position == 2
                                                && t.instantiation == Instantiation::Decision))
                                        || t.kind != index.kind
                                })
                        },
                    )
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                if indices.len() == 3 {
                    let safety = self.direct_children_safety(file, &children, view, generators);
                    if safety != DefinitionSafety::Supported {
                        return safety;
                    }
                    let array = self.reference(file, subject).unwrap();
                    if self.decision_selector_membership(
                        file,
                        array,
                        &children[1..],
                        view,
                        generators,
                    ) {
                        return DefinitionSafety::Supported;
                    }
                    return DefinitionSafety::Unknown(
                        "numeric array index membership is unproved".into(),
                    );
                }
                // A checked enum successor is a source inspection, not an
                // index-membership proof for this array.
                if indices.iter().zip(&children[1..]).any(|(axis, selector)| {
                    matches!(axis.kind, TypeKind::Enum(_))
                        && self
                            .parameter_enum_successor_safety(file, selector, view, generators)
                            .is_some()
                }) {
                    if !crate::definitions::annotations_safe(self.context, file, written)
                        || ty(node).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || t.kind != TypeKind::Int
                                || t.instantiation != element.instantiation
                        })
                    {
                        return DefinitionSafety::Unsupported(
                            "enum successor selection type or annotation is unsupported".into(),
                        );
                    }
                    let array_id = self.reference(file, subject).unwrap();
                    let owner = &self.bindings.declarations[array_id.0];
                    if owner.role != DeclarationRole::Value
                        || !owner.top_level
                        || self.calls.declarations[array_id.0].ty != *array
                    {
                        return DefinitionSafety::Unsupported(
                            "enum successor array source identity is unsupported".into(),
                        );
                    }
                    let Some(source) = find_node(
                        self.context.files[owner.file].parsed.tree(),
                        &owner.syntax_range,
                        owner.role,
                    ) else {
                        return DefinitionSafety::Unsupported(
                            "enum successor array source unavailable".into(),
                        );
                    };
                    if let unsupported @ DefinitionSafety::Unsupported(_) =
                        self.initialized_children_safety(file, &children, view, generators)
                    {
                        return unsupported;
                    }
                    if let Some(reason) =
                        self.closed_integer_source_error(owner.file, source, true, true)
                    {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    for selector in &children[1..] {
                        if let Some(reason) =
                            self.closed_integer_source_error(file, selector, true, true)
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                    }
                    if let Domain::Array { indices, .. } =
                        &self.domains.declarations[array_id.0].domain
                        && indices.iter().zip(&children[1..]).any(|(axis, selector)| {
                            crate::domains::invariant_expression_integer(
                                self.context,
                                self.bindings,
                                file,
                                selector,
                            )
                            .is_ok_and(|value| {
                                value.is_some_and(|value| {
                                    crate::domains::index_domain_member(axis, value) == Some(false)
                                })
                            })
                        })
                    {
                        return DefinitionSafety::Unsupported(
                            "scalar selection is outside its declared index set".into(),
                        );
                    }
                    return DefinitionSafety::Unknown(
                        "numeric array index membership is unproved".into(),
                    );
                }
                if let Err(reason) = self.child_dependencies(file, &children, view, generators) {
                    return DefinitionSafety::Unsupported(reason);
                }
                DefinitionSafety::Unknown("numeric array index membership is unproved".into())
            }
            NodeKind::BinaryExpression => {
                let name =
                    operator(self.context, file, node).and_then(crate::bindings::symbolic_operator);
                if name == Some("*") && children.len() == 2
                    && children.iter().any(|n| ty(n).is_some_and(|t| t.kind == TypeKind::Bool))
                    && self.core(file, node, view, "*")
                    && crate::definitions::annotations_safe(self.context, file, written)
                    && children.iter().all(|n| ty(n).is_some_and(|t| t.known() && !optional(t)
                        && matches!(t.kind, TypeKind::Int | TypeKind::Bool)))
                    && self.operation_fact(self.view(file, node, view), file, node).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 2 && return_type.known() && !optional(return_type)
                                && return_type.kind == TypeKind::Int && Some(return_type) == ty(node)
                                && parameters.iter().zip(&children).all(|(formal, actual)| formal.known()
                                    && !optional(formal) && formal.kind == TypeKind::Int
                                    && ty(actual).is_some_and(|t| crate::types::coerces(t, formal))))
                    }) {
                    return match self.initialized_children_safety(file, &children, view, generators) {
                        unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                        _ => DefinitionSafety::Unknown("coerced multiplication value is unproved".into()),
                    };
                }
                if !matches!(name, Some("+" | "-" | "*" | "div" | "mod"))
                    || children.len() != 2
                    || !self.core(file, node, view, name.unwrap())
                    || children.iter().any(|n| {
                        ty(n).is_none_or(|t| t.kind != TypeKind::Int || !t.known() || optional(t))
                    })
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                if matches!(name, Some("div" | "mod")) {
                    let integer =
                        |t: &TypeInst| t.known() && !optional(t) && t.kind == TypeKind::Int;
                    if !crate::definitions::annotations_safe(self.context, file, written)
                        || self
                            .operation_fact(self.view(file, node, view), file, node)
                            .is_none_or(|call| {
                                !matches!(&call.outcome,
                                CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2 && parameters.iter().all(integer)
                                    && integer(return_type))
                            })
                    {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    if self
                        .inactive_integer_body
                        .as_ref()
                        .is_some_and(|(f, range)| {
                            *f == file
                                && range.start <= node.range().start
                                && node.range().end <= range.end
                        })
                    {
                        return match self
                            .initialized_children_safety(file, &children, view, generators)
                        {
                            unsupported @ DefinitionSafety::Unsupported(_) => unsupported,
                            _ => DefinitionSafety::Unknown(
                                "inactive parameter integer value is unproved".into(),
                            ),
                        };
                    }
                    let mut safety = DefinitionSafety::Supported;
                    let mut unsupported = None;
                    let mut values = Vec::new();
                    for child in &children {
                        let inspected = self.initialized_source_safety(
                            file,
                            child,
                            view,
                            generators,
                            &mut Vec::new(),
                        );
                        // The typed source inspector checks the array and every
                        // selector; membership remains unproved. The independent
                        // integer interpreter has no array-selection evaluator.
                        let symbolic_selection = unwrap(child).kind()
                            == NodeKind::ArrayAccessExpression
                            && !matches!(&inspected, DefinitionSafety::Unsupported(_))
                            && ty(child).is_some_and(|t| {
                                t.known()
                                    && !optional(t)
                                    && t.kind == TypeKind::Int
                                    && matches!(
                                        t.instantiation,
                                        Instantiation::Parameter | Instantiation::Decision
                                    )
                            });
                        let wrapper = unwrap(child);
                        let wrapper_name = operator(self.context, file, wrapper)
                            .and_then(crate::bindings::symbolic_operator);
                        let operands: Vec<_> = wrapper.child_nodes().collect();
                        // Only one inspected selection plus/minus a closed integer
                        // can retain an unproved value. A failing closed sibling is
                        // left to the independent evaluator, including overflow.
                        let wrapped_selection =
                            !matches!(&inspected, DefinitionSafety::Unsupported(_))
                                && wrapper.kind() == NodeKind::BinaryExpression
                                && matches!(wrapper_name, Some("+" | "-"))
                                && operands.len() == 2
                                && self.core(file, wrapper, view, wrapper_name.unwrap())
                                && ty(wrapper).is_some_and(integer)
                                && self
                                    .operation_fact(self.view(file, wrapper, view), file, wrapper)
                                    .is_some_and(|call| {
                                        matches!(&call.outcome,
                                    CallOutcome::Resolved { parameters, return_type, .. }
                                        if parameters.len() == 2 && parameters.iter().all(integer)
                                            && Some(return_type) == ty(wrapper))
                                    })
                                && operands.iter().enumerate().any(|(position, operand)| {
                                    unwrap(operand).kind() == NodeKind::ArrayAccessExpression
                                        && ty(operand).is_some_and(|t| {
                                            integer(t)
                                                && matches!(
                                                    t.instantiation,
                                                    Instantiation::Parameter
                                                        | Instantiation::Decision
                                                )
                                        })
                                        && crate::domains::invariant_expression_integer(
                                            self.context,
                                            self.bindings,
                                            file,
                                            operands[1 - position],
                                        )
                                        .is_ok_and(|value| value.is_some())
                                });
                        match inspected {
                            DefinitionSafety::Unsupported(reason) => unsupported = Some(reason),
                            unknown @ DefinitionSafety::Unknown(_) => safety = unknown,
                            DefinitionSafety::Supported => {}
                        }
                        // A selected length has already had its original source
                        // inspected above. The definitionless integer interpreter
                        // cannot evaluate this call; retain an unproved value.
                        if symbolic_selection
                            || wrapped_selection
                            || length_argument(
                                self.context,
                                self.bindings,
                                self.view(file, child, view),
                                file,
                                unwrap(child),
                            )
                            .is_some()
                        {
                            values.push(None);
                            continue;
                        }
                        // Inspect each operand independently: an unknown dividend
                        // cannot hide a literal zero divisor or invalid sibling.
                        match crate::domains::invariant_expression_integer(
                            self.context,
                            self.bindings,
                            file,
                            child,
                        ) {
                            Ok(value) => values.push(value),
                            Err(reason) => {
                                unsupported = Some(reason);
                                values.push(None);
                            }
                        }
                    }
                    if values[1] == Some(0) {
                        unsupported = Some("integer division by zero".into());
                    }
                    if let Some(reason) = unsupported {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    if values.iter().all(Option::is_some) {
                        match crate::domains::invariant_expression_integer(
                            self.context,
                            self.bindings,
                            file,
                            node,
                        ) {
                            Ok(Some(_)) => return safety,
                            Err(reason) => return DefinitionSafety::Unsupported(reason),
                            Ok(None) => {}
                        }
                    }
                    return DefinitionSafety::Unknown(
                        "integer division/remainder operands are unproved".into(),
                    );
                }
                self.direct_children_safety(file, &children, view, generators)
            }
            NodeKind::CallExpression => {
                if self.core(file, node, view, "pow") {
                    let base_int = |t: &TypeInst| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && matches!(
                                t.instantiation,
                                Instantiation::Parameter | Instantiation::Decision
                            )
                    };
                    let parameter_int = |t: &TypeInst| {
                        t.known()
                            && !optional(t)
                            && t.kind == TypeKind::Int
                            && t.instantiation == Instantiation::Parameter
                    };
                    if children.len() != 2
                        || children.iter().any(|child| child.kind() == NodeKind::NamedArgument)
                        || !crate::definitions::annotations_safe(self.context, file, written)
                        || ty(children[0]).is_none_or(|t| !base_int(t))
                        || ty(children[1]).is_none_or(|t| !parameter_int(t))
                        || ty(node).is_none_or(|t| !base_int(t) || ty(children[0]) != Some(t))
                        || self.operation_fact(self.view(file, node, view), file, node).is_none_or(|call| {
                            !matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 2 && base_int(&parameters[0])
                                    && parameter_int(&parameters[1]) && Some(&parameters[0]) == ty(children[0])
                                    && Some(&parameters[1]) == ty(children[1]) && Some(return_type) == ty(node))
                        })
                    {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    if let unsupported @ DefinitionSafety::Unsupported(_) =
                        self.initialized_children_safety(file, &children, view, generators)
                    {
                        return unsupported;
                    }
                    return match crate::domains::invariant_expression_integer(
                        self.context,
                        self.bindings,
                        file,
                        children[1],
                    ) {
                        Ok(Some(exponent)) if exponent >= 0 => {
                            DefinitionSafety::Unknown("integer power value is unproved".into())
                        }
                        Err(reason) => DefinitionSafety::Unsupported(reason),
                        _ => DefinitionSafety::Unsupported(
                            "integer power requires a closed nonnegative parameter exponent".into(),
                        ),
                    };
                }
                if parameter_index_extremum(
                    self.context,
                    self.bindings,
                    self.view(file, node, view),
                    file,
                    node,
                )
                .is_some()
                {
                    let source = node
                        .child_nodes()
                        .next()
                        .and_then(|set| unwrap(set).child_nodes().next());
                    let Some(source) = source else {
                        return DefinitionSafety::Unsupported(reason);
                    };
                    if let Err(reason) = self.dependencies(file, source, view, generators) {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    return DefinitionSafety::Unknown(
                        "integer index-set extremum may be undefined for an empty array".into(),
                    );
                }
                if self.core(file, node, view, "to_enum_internal") {
                    return match self.integer_conversion_fallback(file, node, view) {
                        Ok(fallback) => self.direct_safety(file, fallback, view, generators),
                        Err(reason) => DefinitionSafety::Unsupported(reason),
                    };
                }
                if self.core(file, node, view, "assert") {
                    let args = match self.integer_assertion_arguments(file, node, view) {
                        Ok(args) => args,
                        Err(reason) => return DefinitionSafety::Unsupported(reason),
                    };
                    for (file, argument) in &args[..2] {
                        if let Err(reason) = self.dependencies(*file, argument, view, generators) {
                            return DefinitionSafety::Unsupported(reason);
                        }
                    }
                    let literal = self.assertion_literal(args[0].0, args[0].1);
                    if literal == Some(TokenKind::False) {
                        return DefinitionSafety::Unsupported(
                            "false assertion condition aborts evaluation".into(),
                        );
                    }
                    let safety = self.direct_safety(args[2].0, args[2].1, view, generators);
                    if literal == Some(TokenKind::True)
                        || matches!(safety, DefinitionSafety::Unsupported(_))
                    {
                        return safety;
                    }
                    return DefinitionSafety::Unknown(
                        "returning assertion condition is unproved".into(),
                    );
                }
                if self.core(file, node, view, "abs") {
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty(children[0]).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
                        || self.operation_fact(self.view(file, node, view), file, node)
                            .is_none_or(|call| {
                                !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                        && parameters[0].kind == TypeKind::Int)
                            })
                    {
                        return DefinitionSafety::Unsupported(reason);
                    }
                    return self.direct_children_safety(file, &children, view, generators);
                }
                if self.is_scalar_integer_extremum(file, written, view, &children) {
                    return self.direct_children_safety(file, &children, view, generators);
                }
                let integer_array = |t: &TypeInst| {
                    t.known()
                        && !optional(t)
                        && matches!(&t.kind,
                        TypeKind::Array { indices, element } if indices.len() == 1
                            && indices[0].known() && !optional(&indices[0])
                            && indices[0].instantiation == Instantiation::Parameter
                            && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Int)
                };
                if !(self.core(file, node, view, "min") || self.core(file, node, view, "max"))
                    || children.len() != 1
                    || children[0].kind() == NodeKind::NamedArgument
                    || ty(children[0]).is_none_or(|t| !integer_array(t))
                    || self
                        .operation_fact(self.view(file, node, view), file, node)
                        .is_none_or(|c| {
                            !matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                            if parameters.len() == 1 && integer_array(&parameters[0]))
                        })
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let collection = unwrap(children[0]);
                let values: Vec<_> = collection.child_nodes().collect();
                let (safety, nonempty) = match collection.kind() {
                    NodeKind::Expression => {
                        let safety = match self.dependencies(file, collection, view, generators) {
                            Ok(_) => DefinitionSafety::Supported,
                            Err(reason) => DefinitionSafety::Unsupported(reason),
                        };
                        let nonempty = self.reference(file, collection).is_some_and(|array| {
                            self.reflection_array_nonempty(file, node, array, view)
                        });
                        (safety, nonempty)
                    }
                    NodeKind::ArrayLiteral => (
                        self.direct_children_safety(file, &values, view, generators),
                        !values.is_empty(),
                    ),
                    NodeKind::BinaryExpression => {
                        if !crate::definitions::annotations_safe(self.context, file, written)
                            || !crate::definitions::annotations_safe(self.context, file, children[0])
                            || self.operation_fact(self.view(file, node, view), file, node)
                                .is_none_or(|call| !matches!(&call.outcome,
                                    CallOutcome::Resolved { return_type, .. }
                                        if return_type.known() && !optional(return_type)
                                            && return_type.kind == TypeKind::Int && Some(return_type) == ty(node)))
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                        let Some(arguments) = array_concatenation(
                            self.context,
                            self.bindings,
                            self.view(file, collection, view),
                            file,
                            collection,
                        ) else {
                            return DefinitionSafety::Unsupported(reason);
                        };
                        let mut unsupported = None;
                        for argument in arguments {
                            if let DefinitionSafety::Unsupported(reason) = self
                                .initialized_source_safety(
                                    file,
                                    argument,
                                    view,
                                    generators,
                                    &mut Vec::new(),
                                )
                            {
                                unsupported = Some(reason);
                            }
                        }
                        // Source inspection gives neither extent nor extrema value proof.
                        return match unsupported {
                            Some(reason) => DefinitionSafety::Unsupported(reason),
                            None => DefinitionSafety::Unknown(
                                "integer extrema concatenation extent and value are unproved"
                                    .into(),
                            ),
                        };
                    }
                    NodeKind::ArrayComprehension => {
                        let Some(list) =
                            values.iter().find(|n| n.kind() == NodeKind::GeneratorList)
                        else {
                            return DefinitionSafety::Unsupported(reason);
                        };
                        let mut all = generators.to_vec();
                        all.extend(list.child_nodes());
                        if let Err(reason) =
                            self.relation_iterations(file, &all, generators.len(), view, false)
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                        let Some(body) =
                            values.iter().find(|n| n.kind() != NodeKind::GeneratorList)
                        else {
                            return DefinitionSafety::Unsupported(reason);
                        };
                        if ty(body)
                            .is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
                        {
                            return DefinitionSafety::Unsupported(reason);
                        }
                        let nonempty = list.child_nodes().all(|g| {
                            !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                                && self.nonempty_source(file, g)
                        });
                        (self.direct_safety(file, body, view, &all), nonempty)
                    }
                    _ => return DefinitionSafety::Unsupported(reason),
                };
                if safety == DefinitionSafety::Supported && !nonempty {
                    DefinitionSafety::Unknown("integer extrema collection may be empty".into())
                } else {
                    safety
                }
            }
            NodeKind::GeneratorCallExpression => {
                let extrema =
                    self.core(file, node, view, "min") || self.core(file, node, view, "max");
                if !(extrema || self.core(file, node, view, "sum"))
                    || self.operation_fact(self.view(file, node, view), file, node)
                        .is_none_or(|c| {
                            !matches!(&c.outcome, CallOutcome::Resolved { parameters, .. }
                                if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                    && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                        if indices.len() == 1 && element.kind == TypeKind::Int))
                        })
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let Some(list) = children
                    .iter()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                else {
                    return DefinitionSafety::Unsupported(reason);
                };
                let Some(body) = children
                    .iter()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                else {
                    return DefinitionSafety::Unsupported(reason);
                };
                if ty(body).is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int) {
                    return DefinitionSafety::Unsupported(reason);
                }
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                if let Err(reason) =
                    self.relation_iterations(file, &all, generators.len(), view, false)
                {
                    return DefinitionSafety::Unsupported(reason);
                }
                let safety = self.direct_safety(file, body, view, &all);
                if extrema
                    && safety == DefinitionSafety::Supported
                    && !list.child_nodes().all(|g| {
                        !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                            && self.nonempty_source(file, g)
                    })
                {
                    DefinitionSafety::Unknown("integer extrema iteration may be empty".into())
                } else {
                    safety
                }
            }
            _ => DefinitionSafety::Unsupported(reason),
        }
    }
    fn named_selector_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        axis: usize,
        selector: &SyntaxNode,
        view: &CallableFacts,
    ) -> bool {
        let selector = unwrap(selector);
        let range = self.context.files[file].location(selector.range()).range;
        let scalar = view
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range);
        if scalar.is_none_or(|e| {
            !e.ty.known()
                || optional(&e.ty)
                || e.ty.kind != TypeKind::Int
                || !matches!(
                    e.ty.instantiation,
                    Instantiation::Parameter | Instantiation::Decision
                )
        }) {
            return false;
        }
        let array_type = &view.declarations[array.0].ty;
        let TypeKind::Array {
            indices: types,
            element,
        } = &array_type.kind
        else {
            return false;
        };
        if !array_type.known()
            || optional(array_type)
            || !matches!(types.len(), 1 | 2)
            || !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
            || !types.iter().all(|index| {
                index.kind == TypeKind::Int
                    && index.instantiation == Instantiation::Parameter
                    && !optional(index)
            })
        {
            return false;
        }
        let mut outer = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain, .. } = outer {
            outer = domain;
        }
        let Domain::Array {
            indices,
            element: domain_element,
        } = outer
        else {
            return false;
        };
        if indices.len() != types.len()
            || indices
                .iter()
                .chain(std::iter::once(domain_element.as_ref()))
                .any(|domain| domain.numeric_minimum().is_err())
        {
            return false;
        }
        let Some(Domain::Named {
            declaration: expected,
            ..
        }) = indices.get(axis)
        else {
            return false;
        };
        let set_type = &view.declarations[expected.0].ty;
        if !set_type.known()
            || optional(set_type)
            || set_type.instantiation != Instantiation::Parameter
            || !matches!(&set_type.kind, TypeKind::Set(element)
                if element.kind == TypeKind::Int
                    && element.instantiation == Instantiation::Parameter)
        {
            return false;
        }
        let own = if selector.kind() == NodeKind::Expression {
            let Some(id) = self.reference(file, selector) else {
                return false;
            };
            &self.domains.declarations[id.0].domain
        } else if selector.kind() == NodeKind::ArrayAccessExpression {
            let parts: Vec<_> = selector.child_nodes().collect();
            let Some(subject) = parts.first().map(|node| unwrap(node)) else {
                return false;
            };
            let Some(id) = self
                .reference(file, subject)
                .filter(|_| subject.kind() == NodeKind::Expression)
            else {
                return false;
            };
            let array_type = &view.declarations[id.0].ty;
            if !array_type.known()
                || optional(array_type)
                || !matches!(&array_type.kind, TypeKind::Array { indices, element }
                    if matches!(indices.len(), 1 | 2) && parts.len() == indices.len() + 1
                        && element.kind == TypeKind::Int
                        && indices.iter().all(|index| index.kind == TypeKind::Int
                            && index.instantiation == Instantiation::Parameter))
            {
                return false;
            }
            let mut inner = &self.domains.declarations[id.0].domain;
            while let Domain::Named { domain, .. } = inner {
                inner = domain;
            }
            let Domain::Array { element, .. } = inner else {
                return false;
            };
            element.as_ref()
        } else {
            return false;
        };
        matches!(own, Domain::Named { declaration: actual, .. } if actual == expected)
    }
    fn decision_selector_membership(
        &self,
        file: FileId,
        array: DeclarationId,
        indices: &[&'a SyntaxNode],
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> bool {
        let selector = unwrap(indices[2]);
        let children: Vec<_> = selector.child_nodes().collect();
        if selector.kind() != NodeKind::ArrayAccessExpression || children.len() != 3 {
            return false;
        }
        let subject = unwrap(children[0]);
        let Some(selected) = self
            .reference(file, subject)
            .filter(|_| subject.kind() == NodeKind::Expression)
        else {
            return false;
        };
        let mut outer = &self.domains.declarations[array.0].domain;
        while let Domain::Named { domain, .. } = outer {
            outer = domain;
        }
        let mut inner = &self.domains.declarations[selected.0].domain;
        while let Domain::Named { domain, .. } = inner {
            inner = domain;
        }
        let (
            Domain::Array { indices: outer, .. },
            Domain::Array {
                indices: inner,
                element,
            },
        ) = (outer, inner)
        else {
            return false;
        };
        outer.len() == 3 && inner.len() == 2
            && matches!((&outer[2], element.as_ref()), (Domain::Named { declaration: expected, .. }, Domain::Named { declaration: actual, .. }) if expected == actual)
            && outer[..2].iter().zip(inner).all(|(expected, actual)| {
                matches!((expected, actual), (Domain::Named { declaration: expected, .. }, Domain::Named { declaration: actual, .. }) if expected == actual)
            })
            && indices[..2].iter().zip(&children[1..]).all(|(outer, inner)| {
                let (outer, inner) = (unwrap(outer), unwrap(inner));
                outer.kind() == NodeKind::Expression && inner.kind() == NodeKind::Expression
                    && self.reference(file, outer).is_some_and(|id| self.reference(file, inner) == Some(id))
            })
            && self.dependencies(file, selector, view, generators).is_ok()
    }
    fn direct_children_safety(
        &self,
        file: FileId,
        nodes: &[&'a SyntaxNode],
        view: &CallableFacts,
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
    fn literal_bool_index_membership(
        &self,
        file: FileId,
        access: &SyntaxNode,
        array: DeclarationId,
        view: &CallableFacts,
    ) -> bool {
        let children: Vec<_> = access.child_nodes().collect();
        let [subject, index] = children.as_slice() else {
            return false;
        };
        let parsed = &self.context.files[file].parsed;
        if subject.kind() != NodeKind::Expression
            || !matches!(crate::domains::tokens(parsed, subject).as_slice(),
                [token] if matches!(token.kind, TokenKind::Identifier | TokenKind::QuotedIdentifier))
            || index.kind() != NodeKind::Expression
            || index.child_nodes().next().is_some()
            || !matches!(crate::domains::tokens(parsed, index).as_slice(),
                [token] if token.kind == TokenKind::IntegerLiteral)
        {
            return false;
        }
        let declared = &view.declarations[array.0].ty;
        let type_of = |node: &SyntaxNode| {
            self.expression_type(self.view(file, node, view), file, node)
                .map(|e| &e.ty)
        };
        if !declared.known()
            || optional(declared)
            || type_of(subject) != Some(declared)
            || type_of(access)
                .is_none_or(|ty| !ty.known() || optional(ty) || ty.kind != TypeKind::Bool)
            || type_of(index).is_none_or(|ty| {
                !ty.known()
                    || optional(ty)
                    || ty.instantiation != Instantiation::Parameter
                    || ty.kind != TypeKind::Int
            })
            || !matches!(&declared.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                    && indices[0].instantiation == Instantiation::Parameter
                    && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Bool)
        {
            return false;
        }
        let Domain::Array { indices, .. } = &self.domains.declarations[array.0].domain else {
            return false;
        };
        let [Domain::Range { .. }] = indices.as_slice() else {
            return false;
        };
        let Some((lower, upper)) = crate::domains::index_domain_interval(&indices[0]) else {
            return false;
        };
        if !crate::domains::invariant_expression_integer(self.context, self.bindings, file, index)
            .is_ok_and(|value| value.is_some_and(|value| lower <= value && value <= upper))
        {
            return false;
        }
        let owner = &self.bindings.declarations[array.0];
        if owner.role != DeclarationRole::Value || !owner.top_level {
            return false;
        }
        let Some(written) = find_node(
            self.context.files[owner.file].parsed.tree(),
            &owner.syntax_range,
            owner.role,
        ) else {
            return false;
        };
        // Initial support is the original uninitialized array, not evaluation
        // of an arbitrary initializer or a callable/default-local array.
        if !crate::definitions::annotations_safe(self.context, owner.file, written)
            || written.child_nodes().any(|node| is_expression(node.kind()))
        {
            return false;
        }
        let Some(array_type) = written
            .child_nodes()
            .next()
            .filter(|node| node.kind() == NodeKind::ArrayType)
        else {
            return false;
        };
        let types: Vec<_> = array_type.child_nodes().collect();
        if types.len() != 2 || types[0].kind() != NodeKind::DomainType {
            return false;
        }
        let axes: Vec<_> = types[0].child_nodes().collect();
        if axes.len() != 1
            || axes[0].kind() != NodeKind::RangeExpression
            || !self.core(owner.file, axes[0], view, "..")
            || self
                .type_dependencies(owner.file, written, view, &[])
                .is_err()
        {
            return false;
        }
        true
    }
    fn dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let written = node;
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let ty = self.expression_type(facts, file, node).map(|e| &e.ty);
        if ty.is_none_or(|t| !t.known() || optional(t)) {
            return Err("output dependency type or optionality is unsupported".into());
        }
        if self.default_collection(file, node) {
            let (mut ids, fallback, all) =
                self.default_collection_arguments(file, node, view, generators)?;
            extend(&mut ids, self.dependencies(file, fallback, view, &all)?);
            return Ok(ids);
        }
        let children: Vec<_> = node.child_nodes().collect();
        match node.kind() {
            NodeKind::Expression => {
                let mut ids = Vec::new();
                if let Some(id) = self.reference(file, node)
                    && self.bindings.declarations[id.0].role != DeclarationRole::Generator
                {
                    let declaration = &self.bindings.declarations[id.0];
                    if declaration.role == DeclarationRole::Local
                        && declaration.syntax_range.end <= node.range().start
                        && facts.declarations[id.0].ty.instantiation == Instantiation::Parameter
                    {
                        let local = find_node(
                            self.context.files[file].parsed.tree(),
                            &declaration.syntax_range,
                            declaration.role,
                        )
                        .ok_or("local declaration unavailable")?;
                        if !crate::definitions::annotations_safe(self.context, file, local) {
                            return Err("local annotation evaluation is unsupported".into());
                        }
                        let value = local
                            .child_nodes()
                            .find(|n| is_expression(n.kind()))
                            .ok_or("local initializer unavailable")?;
                        // An initializer keeps its declaration's lexical binders,
                        // not a later generator that consumes this local value.
                        let lexical: Vec<_> = generators
                            .iter()
                            .copied()
                            .filter(|g| g.range().end <= declaration.syntax_range.start)
                            .collect();
                        return self.dependencies(file, value, view, &lexical);
                    }
                    ids.push(id);
                }
                Ok(ids)
            }
            NodeKind::ArrayAccessExpression => {
                let subject = unwrap(children.first().ok_or("array access subject unavailable")?);
                if subject.kind() == NodeKind::ArrayLiteral && children.len() == 2 {
                    let values: Vec<_> = subject.child_nodes().collect();
                    let index = unwrap(children[1]);
                    let parts: Vec<_> = index.child_nodes().map(unwrap).collect();
                    let type_of = |node: &SyntaxNode| {
                        self.expression_type(self.view(file, node, facts), file, node)
                            .map(|e| &e.ty)
                    };
                    let one = parts.iter().position(|part| {
                        part.kind() == NodeKind::Expression
                            && crate::domains::invariant_expression_integer(
                                self.context,
                                self.bindings,
                                file,
                                part,
                            )
                            .is_ok_and(|value| value == Some(1))
                    });
                    if values.len() != 2
                        || values
                            .iter()
                            .any(|value| value.kind() == NodeKind::IndexedArrayEntry)
                        || ty.is_none_or(|t| !matches!(t.kind, TypeKind::Bool | TypeKind::Int))
                        || type_of(subject).is_none_or(|t| {
                            !t.known()
                                || optional(t)
                                || !matches!(&t.kind, TypeKind::Array { indices, element }
                                if indices.len() == 1 && indices[0].kind == TypeKind::Int
                                    && indices[0].instantiation == Instantiation::Parameter
                                    && Some(&element.kind) == ty.map(|t| &t.kind))
                        })
                        || values.iter().any(|value| {
                            type_of(value).is_none_or(|t| {
                                !t.known() || optional(t) || Some(&t.kind) != ty.map(|t| &t.kind)
                            })
                        })
                        || index.kind() != NodeKind::BinaryExpression
                        || parts.len() != 2
                        || !self.core(file, index, facts, "+")
                        || type_of(index)
                            .is_none_or(|t| !t.known() || optional(t) || t.kind != TypeKind::Int)
                        || one.is_none_or(|position| {
                            let conversion = parts[1 - position];
                            conversion.kind() != NodeKind::CallExpression
                                || !self.core(file, conversion, facts, "bool2int")
                                || conversion.child_nodes().count() != 1
                        })
                    {
                        return Err(
                            "literal array selection is not a supported two-value Boolean choice"
                                .into(),
                        );
                    }
                    // The exact core conversion makes the index 1 or 2. Both
                    // values and the selector remain forward dependencies.
                    return self.child_dependencies(file, &children, view, generators);
                }
                if subject.kind() != NodeKind::Expression {
                    return Err(
                        "computed array access subject does not establish output dependencies"
                            .into(),
                    );
                }
                let id = self
                    .reference(file, subject)
                    .ok_or("array access identity unavailable")?;
                if self.literal_bool_index_membership(file, node, id, facts) {
                    // This exact closed index proves one scalar selection, not
                    // whole-array coverage. Keep source and selector dependencies.
                    return self.child_dependencies(file, &children, view, generators);
                }
                if matches!(&facts.declarations[id.0].ty.kind,
                    TypeKind::Array { indices, .. }
                        if matches!(indices.len(), 1 | 2) && indices.len() + 1 == children.len())
                    && children[1..].iter().enumerate().all(|(axis, selector)| {
                        self.named_selector_membership(file, id, axis, selector, facts)
                    })
                {
                    // Every selector must match its own exact declared axis.
                    // Evaluation and all forward selector dependencies remain required.
                    if children.len() == 3
                        && let DefinitionSafety::Unsupported(reason) =
                            self.initialized_children_safety(file, &children, view, generators)
                    {
                        return Err(reason);
                    }
                    let mut ids = vec![id];
                    for selector in &children[1..] {
                        extend(
                            &mut ids,
                            self.dependencies(file, selector, view, generators)?,
                        );
                    }
                    return Ok(ids);
                }
                let supported = children.len() == 2
                    && (self.member_index(file, id, children[1], generators, view)
                        || self.asserted_index_membership(
                            file,
                            node,
                            id,
                            children[1],
                            generators,
                            facts,
                        )
                        || self.array_bound_index(file, id, children[1], view)
                        || self.shifted_index_membership(
                            file,
                            node,
                            id,
                            children[1],
                            generators,
                            facts,
                        ))
                    || self.written_index_membership(file, id, &children[1..], generators, facts)
                    || complete_array_coverage(
                        self.context,
                        self.bindings,
                        (self.calls, facts),
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
            NodeKind::BinaryExpression | NodeKind::RangeExpression => {
                let name = crate::bindings::symbolic_operator(
                    operator(self.context, file, node).ok_or("operator unavailable")?,
                )
                .ok_or("operator unsupported")?;
                if !matches!(
                    name,
                    "=" | "!="
                        | "+"
                        | "-"
                        | "*"
                        | ".."
                        | "<"
                        | "<="
                        | ">"
                        | ">="
                        | "in"
                        | "diff"
                        | "/\\"
                        | "\\/"
                        | "++"
                ) || !self.core(file, node, view, name)
                {
                    return Err(
                        "output dependency operator identity or partiality is unsupported".into(),
                    );
                }
                if name == "++" {
                    let strings = ty.is_some_and(|t| {
                        t.instantiation == Instantiation::Parameter && t.kind == TypeKind::String
                    });
                    let integers = ty.is_some_and(parameter_integers)
                        && children.len() == 2
                        && children.iter().all(|child| {
                            self.expression_type(facts, file, child)
                                .is_some_and(|e| parameter_integers(&e.ty))
                        });
                    if !strings && !integers {
                        // Keep strict dependencies limited to the existing integer shape.
                        let Some(arguments) = ty
                            .filter(|t| {
                                matches!(&t.kind, TypeKind::Array { element, .. }
                                if element.kind == TypeKind::Int)
                            })
                            .and_then(|_| {
                                array_concatenation(self.context, self.bindings, facts, file, node)
                            })
                        else {
                            return Err("concatenation requires supported parameter strings or integer arrays".into());
                        };
                        for argument in arguments {
                            match self.initialized_source_safety(
                                file,
                                argument,
                                view,
                                generators,
                                &mut Vec::new(),
                            ) {
                                DefinitionSafety::Supported => {}
                                DefinitionSafety::Unknown(reason)
                                | DefinitionSafety::Unsupported(reason) => return Err(reason),
                            }
                        }
                    }
                }
                if name == "diff"
                    && ty.is_none_or(|t| {
                        t.instantiation != Instantiation::Parameter
                            || !matches!(t.kind, TypeKind::Set(_))
                    })
                {
                    return Err("set difference requires supported parameter sets".into());
                }
                let scalar_comparison = matches!(name, "=" | "!=" | "<" | "<=" | ">" | ">=")
                    && ty.is_some_and(|t| t.kind == TypeKind::Bool)
                    && children.len() == 2
                    && children.iter().all(|child| {
                        self.expression_type(facts, file, child).is_some_and(|e| {
                            e.ty.known()
                                && !optional(&e.ty)
                                && matches!(e.ty.kind, TypeKind::Int | TypeKind::Enum(_))
                        })
                    });
                let range_membership = name == "in" && ty.is_some_and(|t| t.kind == TypeKind::Bool) && children.len() == 2
                    && children.iter().enumerate().all(|(position, child)| {
                        self.expression_type(facts, file, child).is_some_and(|e| {
                            e.ty.known() && !optional(&e.ty) && if position == 0 { e.ty.kind == TypeKind::Int }
                            else { e.ty.instantiation == Instantiation::Parameter && matches!(&e.ty.kind, TypeKind::Set(element) if element.kind == TypeKind::Int) }
                        })
                    });
                let mut ids = Vec::new();
                for child in &children {
                    let dependencies = match self.dependencies(file, child, view, generators) {
                        Ok(ids) => ids,
                        Err(reason) if scalar_comparison || range_membership => self
                            .relational_operand_dependencies(file, child, view, generators)
                            .map_err(|_| reason)?,
                        Err(reason) => return Err(reason),
                    };
                    extend(&mut ids, dependencies);
                }
                Ok(ids)
            }
            NodeKind::UnaryExpression => {
                let name = crate::bindings::symbolic_operator(
                    operator(self.context, file, node).ok_or("unary operator unavailable")?,
                )
                .ok_or("unary operator unsupported")?;
                if !matches!(name, "not" | "+" | "-") || !self.core(file, node, view, name) {
                    return Err("unary operation identity is unsupported".into());
                }
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::CallExpression => {
                if self.core(file, node, view, "to_enum_internal") {
                    let fallback = self.integer_conversion_fallback(file, node, view)?;
                    return self.dependencies(file, fallback, view, generators);
                }
                if self.core(file, node, view, "assert") {
                    let args = self.integer_assertion_arguments(file, node, view)?;
                    let mut ids = Vec::new();
                    for (file, argument) in &args[..2] {
                        extend(
                            &mut ids,
                            self.dependencies(*file, argument, view, generators)?,
                        );
                    }
                    if self.assertion_literal(args[0].0, args[0].1) != Some(TokenKind::True) {
                        return Err("returning assertion condition is unproved".into());
                    }
                    extend(
                        &mut ids,
                        self.dependencies(args[2].0, args[2].1, view, generators)?,
                    );
                    return Ok(ids);
                }
                if self.core(file, node, view, "abs") {
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| t.kind != TypeKind::Int)
                        || self.expression_type(facts, file, children[0]).is_none_or(|e| !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Int)
                        || self.operation_fact(facts, file, node).is_none_or(|call| {
                            !matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                                if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                    && parameters[0].kind == TypeKind::Int)
                        })
                    {
                        return Err("abs requires a supported present integer value".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "bool2int") {
                    let argument = children
                        .first()
                        .and_then(|child| self.expression_type(facts, file, child));
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| t.kind != TypeKind::Int)
                        || argument.is_none_or(|e| {
                            !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool
                        })
                        || !self.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                                if parameters.len() == 1 && parameters[0].known()
                                    && !optional(&parameters[0]) && parameters[0].kind == TypeKind::Bool)
                        })
                    {
                        return Err("bool2int requires a supported present Boolean value".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "arrayXd") {
                    self.parameter_array_source(file, node, view)
                        .ok_or("parameter reindexing lacks a same-source one-to-one traversal")?;
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "arg_min") {
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| {
                            t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                        })
                        || !self.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                                if parameters.len() == 1 && parameter_integers(&parameters[0]))
                        })
                    {
                        return Err("arg_min requires a supported parameter integer array".into());
                    }
                    let source = self
                        .parameter_array_source(file, children[0], view)
                        .ok_or("arg_min lacks a same-source parameter traversal")?;
                    if !self.reflection_array_nonempty(file, node, source, view) {
                        return Err("arg_min lacks a scoped nonempty source proof".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "dom") {
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| t.instantiation != Instantiation::Parameter
                            || !matches!(&t.kind, TypeKind::Set(element) if element.known() && !optional(element) && element.kind == TypeKind::Int))
                        || !self.asserted_integer_bounds(file, node, children[0], view)
                    {
                        return Err("scalar domain reflection lacks a same-operand assertion bounds proof".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if ["lb", "ub", "fix", "is_fixed", "lb_array", "ub_array"]
                    .iter()
                    .any(|name| self.core(file, node, view, name))
                {
                    let argument = children
                        .first()
                        .and_then(|child| self.expression_type(facts, file, child));
                    let array = self.core(file, node, view, "lb_array")
                        || self.core(file, node, view, "ub_array");
                    let result = if self.core(file, node, view, "is_fixed") {
                        TypeKind::Bool
                    } else {
                        TypeKind::Int
                    };
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| {
                            t.kind != result || t.instantiation != Instantiation::Parameter
                        })
                        || argument.is_none_or(|e| {
                            if array {
                                !parameter_integers(&e.ty)
                            } else {
                                e.ty.kind != TypeKind::Int
                                    || e.ty.instantiation != Instantiation::Parameter
                                        && !(self.core(file, node, view, "lb")
                                            && self.asserted_integer_bounds(
                                                file,
                                                node,
                                                children[0],
                                                view,
                                            ))
                                    || !e.ty.known()
                                    || optional(&e.ty)
                            }
                        })
                    {
                        return Err(
                            "reflection requires a supported present parameter integer operand"
                                .into(),
                        );
                    }
                    if array && unwrap(children[0]).kind() != NodeKind::Expression {
                        return Err("array bounds require a guarded bare array identity".into());
                    }
                    if array
                        && !self
                            .reference(file, unwrap(children[0]))
                            .is_some_and(|id| self.reflection_array_nonempty(file, node, id, view))
                    {
                        return Err("array bounds lack a scoped nonempty proof".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "sum") {
                    let argument = children
                        .first()
                        .and_then(|child| self.expression_type(facts, file, child));
                    if children.len() != 1
                        || children[0].kind() == NodeKind::NamedArgument
                        || ty.is_none_or(|t| {
                            t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                        })
                        || argument.is_none_or(|e| !parameter_integers(&e.ty))
                        || !self.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                                if parameters.len() == 1 && parameter_integers(&parameters[0]))
                        })
                    {
                        return Err(
                            "sum requires a supported present parameter integer array".into()
                        );
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "set2array") {
                    let argument = children
                        .first()
                        .filter(|n| n.kind() != NodeKind::NamedArgument)
                        .and_then(|n| self.expression_type(facts, file, n));
                    if children.len() != 1
                        || argument.is_none_or(|e| {
                            !e.ty.known()
                                || optional(&e.ty)
                                || e.ty.instantiation != Instantiation::Parameter
                                || !matches!(&e.ty.kind, TypeKind::Set(element)
                                    if element.known() && !optional(element)
                                        && element.instantiation == Instantiation::Parameter
                                        && element.kind == TypeKind::Int)
                        })
                        || ty.is_none_or(|t| !parameter_integers(t))
                        || !self.operation_fact(facts, file, node).is_some_and(|call| {
                            matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                                if parameters.len() == 1
                                    && argument.is_some_and(|e| parameters[0] == e.ty)
                                    && ty == Some(return_type))
                        })
                    {
                        return Err("set2array requires an exact present parameter integer-set conversion".into());
                    }
                    if let DefinitionSafety::Unsupported(reason) = self.initialized_source_safety(
                        file,
                        children[0],
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        return Err(reason);
                    }
                    if let Some(reason) =
                        self.closed_integer_source_error(file, children[0], true, true)
                    {
                        return Err(reason);
                    }
                    // Inspect the written source without inventing converted values or extent.
                    return self.dependencies(file, children[0], view, generators);
                }
                if ["absent", "occurs"].iter().any(|name| {
                    crate::optional::core_optional_call(
                        self.context,
                        self.bindings,
                        facts,
                        file,
                        node,
                        name,
                    )
                }) {
                    if children.len() != 1 {
                        return Err("presence guard operand unavailable".into());
                    }
                    return self.presence_dependencies(file, children[0], view);
                }
                if self.core(file, node, view, "min") || self.core(file, node, view, "max") {
                    if children.len() == 1 {
                        let set = unwrap(children[0]);
                        let args: Vec<_> = set.child_nodes().collect();
                        if args.len() == 1
                            && unwrap(args[0]).kind() == NodeKind::Expression
                            && let Some(array) = self.reference(file, unwrap(args[0]))
                            && self.array_bound_index(file, array, node, view)
                        {
                            return self.dependencies(file, args[0], view, generators);
                        }
                    } else if self.is_scalar_integer_extremum(file, written, view, &children) {
                        return self.child_dependencies(file, &children, view, generators);
                    }
                    return Err(
                        "extrema lack a supported scalar or guarded nonempty index-set proof"
                            .into(),
                    );
                }
                if self.core(file, node, view, "length")
                    && let Some(argument) =
                        length_argument(self.context, self.bindings, facts, file, node)
                    && self
                        .expression_type(facts, file, argument)
                        .is_some_and(|e| matches!(e.ty.kind, TypeKind::Set(_)))
                {
                    // Inspect the original set; the selected array formal is
                    // only the compiler's set2array matching view.
                    match self.initialized_source_safety(
                        file,
                        argument,
                        view,
                        generators,
                        &mut Vec::new(),
                    ) {
                        DefinitionSafety::Supported => {
                            return self.dependencies(file, argument, view, generators);
                        }
                        DefinitionSafety::Unknown(reason)
                        | DefinitionSafety::Unsupported(reason) => return Err(reason),
                    }
                }
                let metadata = if self.core(file, node, view, "show_index_sets") {
                    Some((1, TypeKind::String))
                } else if self.core(file, node, view, "length") {
                    Some((1, TypeKind::Int))
                } else if crate::optional::core_optional_call(
                    self.context,
                    self.bindings,
                    facts,
                    file,
                    node,
                    "index_sets_agree",
                ) {
                    Some((2, TypeKind::Bool))
                } else {
                    None
                };
                if let Some((arity, result)) = metadata {
                    if children.len() != arity
                        || ty.is_none_or(|t| {
                            t.kind != result || t.instantiation != Instantiation::Parameter
                        })
                        || children.iter().any(|child| {
                            self.expression_type(facts, file, child)
                                .is_none_or(|e| !matches!(e.ty.kind, TypeKind::Array { .. }))
                        })
                    {
                        return Err("array metadata type or arity is unsupported".into());
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if self.core(file, node, view, "dom_array") {
                    let array = children
                        .first()
                        .filter(|n| unwrap(n).kind() == NodeKind::Expression)
                        .and_then(|n| self.reference(file, unwrap(n)))
                        .ok_or("reflected array identity is unsupported")?;
                    if children.len() != 1 || !self.finite_array_branch(file, node, array, view) {
                        return Err(
                            "reflected array domains lack a complete guarded finite-bounds proof"
                                .into(),
                        );
                    }
                    return self.child_dependencies(file, &children, view, generators);
                }
                if !(self.core(file, node, view, "array1d")
                    || self.core(file, node, view, "enum2int")
                    || self.array_conversion_argument(file, node, view).is_some()
                    || self.core(file, node, view, "index_set")
                    || self.core(file, node, view, "has_bounds"))
                {
                    return Err("arbitrary value calls do not prove output dependencies".into());
                }
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::GeneratorCallExpression
            | NodeKind::SetComprehension
            | NodeKind::ArrayComprehension => {
                let parameter_collection = (node.kind() == NodeKind::SetComprehension
                    && ty.is_some_and(|t| {
                        t.instantiation == Instantiation::Parameter
                            && matches!(t.kind, TypeKind::Set(_))
                    }))
                    || (node.kind() == NodeKind::ArrayComprehension
                        && ty.is_some_and(parameter_integers));
                let quantifier = (self.core(file, node, view, "forall")
                    || self.core(file, node, view, "exists")
                    || self.core(file, node, view, "xorall"))
                    && ty.is_some_and(|t| t.kind == TypeKind::Bool);
                let count = self.core(file, node, view, "count")
                    && ty.is_some_and(|t| t.kind == TypeKind::Int)
                    && self.operation_fact(facts, file, node).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                            if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                && matches!(&parameters[0].kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Bool))
                    });
                let extrema = (self.core(file, node, view, "min")
                    || self.core(file, node, view, "max"))
                    && ty.is_some_and(|t| t.kind == TypeKind::Int)
                    && self.operation_fact(facts, file, node).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                            if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0])
                                && matches!(&parameters[0].kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && element.kind == TypeKind::Int))
                    });
                if !parameter_collection
                    && !quantifier
                    && !count
                    && !extrema
                    && !self.core(file, node, view, "sum")
                {
                    return Err("output dependency iteration is unsupported".into());
                }
                let list = node
                    .child_nodes()
                    .find(|n| n.kind() == NodeKind::GeneratorList)
                    .ok_or("sum generators unavailable")?;
                let mut all = generators.to_vec();
                all.extend(list.child_nodes());
                if parameter_collection || quantifier || count || extrema {
                    let decision_quantifier = quantifier && self.core(file, node, view, "exists") && list.child_nodes().all(|generator| {
                        let Some(source) = generator.child_nodes().next() else { return false; };
                        self.expression_type(self.view(file, source, view), file, source).is_some_and(|e| {
                            e.ty.instantiation != Instantiation::Decision
                                || matches!(&e.ty.kind, TypeKind::Array { indices, element }
                                    if indices.len() == 1 && indices[0].known() && !optional(&indices[0])
                                        && indices[0].instantiation == Instantiation::Parameter
                                        && matches!(indices[0].kind, TypeKind::Int | TypeKind::Enum(_))
                                        && element.known() && !optional(element) && element.kind == TypeKind::Int)
                        })
                    });
                    self.relation_iterations(
                        file,
                        &all,
                        generators.len(),
                        view,
                        count || decision_quantifier,
                    )?;
                } else {
                    self.iterations(file, &all, view)?;
                }
                let body = node
                    .child_nodes()
                    .find(|n| n.kind() != NodeKind::GeneratorList)
                    .ok_or("sum body unavailable")?;
                if quantifier
                    && self.expression_type(facts, file, body).is_none_or(|e| {
                        !e.ty.known() || optional(&e.ty) || e.ty.kind != TypeKind::Bool
                    })
                {
                    return Err("quantifier requires a supported present Boolean body".into());
                }
                let selected_parameter_sum = node.kind() == NodeKind::GeneratorCallExpression
                    && self.core(file, node, view, "sum")
                    && self.operation_fact(facts, file, node).is_some_and(|call| {
                        matches!(&call.outcome, CallOutcome::Resolved { parameters, return_type, .. }
                            if parameters.len() == 1 && parameter_integers(&parameters[0])
                                && call.generator_argument.as_ref() == Some(&parameters[0])
                                && return_type.known() && !optional(return_type)
                                && return_type.instantiation == Instantiation::Parameter
                                && return_type.kind == TypeKind::Int && ty == Some(return_type))
                    });
                let mut ids = self.dependencies(file, body, view, &all)?;
                for (position, g) in list.child_nodes().enumerate() {
                    let position = generators.len() + position;
                    // Dependency identity does not inspect a selected set's initializer.
                    if selected_parameter_sum
                        && let Some(DefinitionSafety::Unsupported(reason)) =
                            self.selected_generator_source_safety(file, g, view, &all[..position])
                    {
                        return Err(reason);
                    }
                    extend(
                        &mut ids,
                        self.dependencies(
                            file,
                            g.child_nodes().next().ok_or("sum domain unavailable")?,
                            view,
                            &all[..position],
                        )?,
                    );
                    for filter in g
                        .child_nodes()
                        .filter(|n| n.kind() == NodeKind::WhereFilter)
                    {
                        let condition = filter
                            .child_nodes()
                            .next()
                            .ok_or("iteration filter unavailable")?;
                        extend(
                            &mut ids,
                            self.dependencies(file, condition, view, &all[..=position])?,
                        );
                    }
                }
                if extrema
                    && !list.child_nodes().all(|g| {
                        !g.child_nodes().any(|n| n.kind() == NodeKind::WhereFilter)
                            && self.nonempty_source(file, g)
                    })
                {
                    return Err("integer extrema iteration may be empty".into());
                }
                Ok(ids)
            }
            NodeKind::ArrayLiteral | NodeKind::SetLiteral => {
                self.child_dependencies(file, &children, view, generators)
            }
            NodeKind::ConditionalExpression => {
                if ty.is_none_or(|t| {
                    t.kind != TypeKind::Int || t.instantiation != Instantiation::Parameter
                }) {
                    return Err(
                        "value conditional requires a present parameter integer result".into(),
                    );
                }
                let values = self.integer_conditional_values(file, node, view)?;
                self.child_dependencies(file, &values, view, generators)
            }
            NodeKind::LetExpression => {
                if ty.is_none_or(|t| {
                    t.kind != TypeKind::Bool || t.instantiation != Instantiation::Parameter
                }) || children.len() != 2
                    || children[0].kind() != NodeKind::LetBlock
                {
                    return Err(
                        "value let requires a supported present parameter Boolean result".into(),
                    );
                }
                let mut ids = Vec::new();
                let mut locals = Vec::new();
                for local in children[0].child_nodes() {
                    let declaration = self.bindings.declarations.iter().find(|d| {
                        d.file == file
                            && d.syntax_range == local.range()
                            && d.role == DeclarationRole::Local
                    });
                    let Some(declaration) =
                        declaration.filter(|_| local.kind() == NodeKind::Declaration)
                    else {
                        return Err(
                            "value let supports initialized parameter declarations only".into()
                        );
                    };
                    let local_type = &facts.declarations[declaration.id.0].ty;
                    let initializer = local.child_nodes().find(|n| is_expression(n.kind()));
                    if !local_type.known()
                        || optional(local_type)
                        || local_type.instantiation != Instantiation::Parameter
                        || initializer.is_none()
                        || !crate::definitions::annotations_safe(self.context, file, local)
                    {
                        return Err(
                            "value let declaration, initializer or annotation is unsupported"
                                .into(),
                        );
                    }
                    locals.push(declaration.id);
                    extend(
                        &mut ids,
                        self.type_dependencies(file, local, view, generators)?,
                    );
                    extend(
                        &mut ids,
                        self.dependencies(file, initializer.unwrap(), view, generators)?,
                    );
                }
                extend(
                    &mut ids,
                    self.dependencies(file, children[1], view, generators)?,
                );
                if ids.iter().any(|id| locals.contains(id)) {
                    return Err(
                        "value let has an unsupported forward or cyclic local dependency".into(),
                    );
                }
                Ok(ids)
            }
            _ => Err("output dependency control, option or value safety is unsupported".into()),
        }
    }
    fn boolean_relation_dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        if let Ok(ids) = self.dependencies(file, node, view, generators) {
            return Ok(ids);
        }
        let node = unwrap(node);
        let facts = self.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            let range = self.context.files[file].location(value.range()).range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        if ty(node).is_none_or(|t| !t.known() || t.kind != TypeKind::Bool || optional(t)) {
            return Err("nondefining Boolean relation type is unsupported".into());
        }
        let children: Vec<_> = node.child_nodes().collect();
        if node.kind() == NodeKind::ArrayAccessExpression && matches!(children.len(), 2 | 3) {
            let subject = unwrap(children[0]);
            let array = ty(subject).ok_or("Boolean array type unavailable")?;
            if subject.kind() != NodeKind::Expression
                || self.reference(file, subject).is_none()
                || !array.known()
                || optional(array)
                || !matches!(&array.kind, TypeKind::Array { indices, element }
                    if indices.len() + 1 == children.len()
                        && element.known() && !optional(element) && element.kind == TypeKind::Bool
                        && indices.iter().zip(&children[1..]).all(|(axis, selector)|
                            axis.known() && !optional(axis) && axis.instantiation == Instantiation::Parameter
                                && matches!(axis.kind, TypeKind::Int | TypeKind::Enum(_))
                                && ty(selector).is_some_and(|index| index.known() && !optional(index)
                                    && index.instantiation == Instantiation::Parameter && index.kind == axis.kind)))
            {
                return Err("nondefining Boolean array or index is unsupported".into());
            }
            if children.len() == 3 {
                let array = self.reference(file, subject).unwrap();
                if let Domain::Array { indices, .. } = &self.domains.declarations[array.0].domain
                    && indices.iter().any(|axis| axis.numeric_minimum().is_err())
                {
                    return Err("nondefining Boolean source index domain is unsupported".into());
                }
                for selector in &children[1..] {
                    if let Some(reason) =
                        self.closed_integer_source_error(file, selector, false, false)
                    {
                        return Err(reason);
                    }
                }
                if let DefinitionSafety::Unsupported(reason) =
                    self.initialized_children_safety(file, &children, view, generators)
                {
                    return Err(reason);
                }
            }
            // Undefined Boolean selection becomes false in this relation. Index
            // evaluation stays strict; this proves no raw membership or output.
            return self.child_dependencies(file, &children, view, generators);
        }
        let name = operator(self.context, file, node)
            .and_then(crate::bindings::symbolic_operator)
            .ok_or("nondefining Boolean operation unavailable")?;
        let supported = match node.kind() {
            NodeKind::BinaryExpression => {
                children.len() == 2
                    && matches!(
                        name,
                        "=" | "!=" | "<" | "<=" | ">" | ">=" | "/\\" | "\\/" | "<->" | "->" | "<-"
                    )
            }
            NodeKind::UnaryExpression => children.len() == 1 && name == "not",
            _ => false,
        };
        if !supported || !self.core(file, node, view, name) {
            return Err("nondefining Boolean operation identity is unsupported".into());
        }
        let mut ids = Vec::new();
        for child in children {
            if ty(child).is_none_or(|t| !t.known() || t.kind != TypeKind::Bool || optional(t)) {
                return Err("nondefining Boolean operand is unsupported".into());
            }
            extend(
                &mut ids,
                self.boolean_relation_dependencies(file, child, view, generators)?,
            );
        }
        Ok(ids)
    }
    fn relational_operand_dependencies(
        &self,
        file: FileId,
        node: &'a SyntaxNode,
        view: &CallableFacts,
        generators: &[&'a SyntaxNode],
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(node);
        let children: Vec<_> = node.child_nodes().collect();
        let facts = self.view(file, node, view);
        let ty = |value: &SyntaxNode| {
            let range = self.context.files[file].location(value.range()).range;
            facts
                .expressions
                .iter()
                .find(|e| e.file == file && e.location.range == range)
                .map(|e| &e.ty)
        };
        if node.kind() == NodeKind::BinaryExpression
            && children.len() == 2
            && ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            && children.iter().all(|child| {
                ty(child).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            })
            && operator(self.context, file, node)
                .and_then(crate::bindings::symbolic_operator)
                .is_some_and(|name| matches!(name, "+" | "-") && self.core(file, node, view, name))
        {
            let mut ids = Vec::new();
            for child in &children {
                let dependencies =
                    self.dependencies(file, child, view, generators)
                        .or_else(|_| {
                            self.relational_operand_dependencies(file, child, view, generators)
                        })?;
                extend(&mut ids, dependencies);
            }
            return Ok(ids);
        }
        if node.kind() == NodeKind::CallExpression
            && children.len() == 1
            && self.core(file, node, view, "bool2int")
            && ty(node).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Int)
            && ty(children[0]).is_some_and(|t| t.known() && !optional(t) && t.kind == TypeKind::Bool)
            && self.operation_fact(facts, file, node).is_some_and(|call| {
                matches!(&call.outcome, CallOutcome::Resolved { parameters, .. }
                    if parameters.len() == 1 && parameters[0].known() && !optional(&parameters[0]) && parameters[0].kind == TypeKind::Bool)
            })
        {
            return self.boolean_relation_dependencies(file, children[0], view, generators);
        }
        if node.kind() == NodeKind::RangeExpression
            && children.len() == 2
            && self.core(file, node, view, "..")
        {
            let facts = self.view(file, node, view);
            let supported = children.iter().all(|child| {
                let range = self.context.files[file].location(child.range()).range;
                matches!(
                    unwrap(child).kind(),
                    NodeKind::Expression | NodeKind::ArrayAccessExpression
                ) && facts
                    .expressions
                    .iter()
                    .find(|e| e.file == file && e.location.range == range)
                    .is_some_and(|e| {
                        e.ty.kind == TypeKind::Int
                            && e.ty.instantiation == Instantiation::Parameter
                            && !optional(&e.ty)
                    })
            });
            if !supported {
                return Err("relational range endpoints are unsupported".into());
            }
            let mut ids = Vec::new();
            for child in &children {
                let dependencies = match self.dependencies(file, child, view, generators) {
                    Ok(ids) => ids,
                    Err(reason) => self
                        .relational_operand_dependencies(file, child, view, generators)
                        .map_err(|_| reason)?,
                };
                extend(&mut ids, dependencies);
            }
            return Ok(ids);
        }
        if node.kind() != NodeKind::ArrayAccessExpression
            || !matches!(children.len(), 2 | 3)
            || unwrap(children[0]).kind() != NodeKind::Expression
            || self.reference(file, unwrap(children[0])).is_none()
            || children[1..].iter().any(|index| {
                !matches!(
                    unwrap(index).kind(),
                    NodeKind::Expression
                        | NodeKind::ArrayAccessExpression
                        | NodeKind::BinaryExpression
                )
            })
        {
            return Err("relational selection shape is unsupported".into());
        }
        let array = ty(children[0]).ok_or("relational array type unavailable")?;
        let TypeKind::Array { indices, element } = &array.kind else {
            return Err("relational subject is not an array".into());
        };
        if !array.known()
            || optional(array)
            || indices.len() != children.len() - 1
            || indices
                .iter()
                .any(|index| index.instantiation != Instantiation::Parameter)
            || !matches!(element.kind, TypeKind::Int | TypeKind::Enum(_))
            || children[1..].iter().zip(indices).any(|(node, formal)| {
                ty(node).is_none_or(|index| {
                    !index.known()
                        || optional(index)
                        || index.instantiation != Instantiation::Parameter
                        || index.kind != formal.kind
                        || !matches!(index.kind, TypeKind::Int | TypeKind::Enum(_))
                })
            })
        {
            return Err("relational array or scalar index type is unsupported".into());
        }
        // The nearest core Boolean relation converts undefined numeric selections
        // to false. Nested parameter indices retain their own dependencies; this
        // supplies neither raw membership nor a directional output guarantee.
        let mut ids = self.dependencies(file, children[0], view, generators)?;
        for index in &children[1..] {
            let dependencies = self
                .dependencies(file, index, view, generators)
                .or_else(|_| self.relational_operand_dependencies(file, index, view, generators))?;
            extend(&mut ids, dependencies);
        }
        Ok(ids)
    }
    fn presence_dependencies(
        &self,
        file: FileId,
        node: &SyntaxNode,
        view: &CallableFacts,
    ) -> Result<Vec<DeclarationId>, String> {
        let node = unwrap(node);
        let range = self.context.files[file].location(node.range()).range;
        let ty = self
            .view(file, node, view)
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == range)
            .map(|e| &e.ty);
        if node.kind() != NodeKind::Expression
            || ty.is_none_or(|t| !t.known() || t.instantiation != Instantiation::Parameter)
        {
            return Err("presence guard requires a supported parameter reference".into());
        }
        // Presence is total with missing parameter data. Retain its identity,
        // without evaluating its optional value or substituting a default.
        Ok(self.reference(file, node).into_iter().collect())
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
fn decision_integer_set(set: &TypeInst) -> bool {
    set.known()
        && !optional(set)
        && set.instantiation == Instantiation::Decision
        && matches!(&set.kind, TypeKind::Set(element)
            if element.known() && !optional(element)
                && element.instantiation == Instantiation::Parameter && element.kind == TypeKind::Int)
}
fn parameter_integers(array: &TypeInst) -> bool {
    array.known()
        && !optional(array)
        && array.instantiation == Instantiation::Parameter
        && matches!(&array.kind, TypeKind::Array { indices, element }
            if indices.len() == 1 && indices[0].kind == TypeKind::Int
                && indices[0].instantiation == Instantiation::Parameter
                && !optional(&indices[0]) && element.kind == TypeKind::Int
                && element.instantiation == Instantiation::Parameter
                && !optional(element))
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
// Recover only the lexical prefix of this actual header inside the retained
// subtree. An unavailable or ambiguous owning path keeps ordinary strict safety.
fn generator_source_scope<'a>(
    mut node: &'a SyntaxNode,
    header: &'a SyntaxNode,
    ambient: &[&'a SyntaxNode],
) -> Option<Vec<&'a SyntaxNode>> {
    if std::ptr::eq(node, header) {
        return None;
    }
    let contains = |outer: &SyntaxNode| {
        outer.range().start <= header.range().start && header.range().end <= outer.range().end
    };
    let mut scope = ambient.to_vec();
    while !std::ptr::eq(node, header) {
        if let Some(list) = node
            .child_nodes()
            .find(|child| child.kind() == NodeKind::GeneratorList)
        {
            if !matches!(
                node.kind(),
                NodeKind::GeneratorCallExpression
                    | NodeKind::ArrayComprehension
                    | NodeKind::SetComprehension
            ) {
                return None;
            }
            if contains(list) {
                let owners: Vec<_> = list.child_nodes().filter(|child| contains(child)).collect();
                let [owner] = owners.as_slice() else {
                    return None;
                };
                scope.extend(
                    list.child_nodes()
                        .take_while(|child| !std::ptr::eq(*child, *owner)),
                );
                if owner
                    .child_nodes()
                    .any(|child| child.kind() == NodeKind::WhereFilter && contains(child))
                {
                    scope.push(owner);
                }
            } else {
                scope.extend(list.child_nodes());
            }
        }
        let children: Vec<_> = node.child_nodes().filter(|child| contains(child)).collect();
        let [child] = children.as_slice() else {
            return None;
        };
        if std::ptr::eq(*child, header) && node.kind() != NodeKind::GeneratorList {
            return None;
        }
        node = child;
    }
    Some(scope)
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
