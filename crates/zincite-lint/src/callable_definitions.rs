//! Bounded callable output guarantees, independent of search advice.
//!
//! The entry point retains one set of lookups, collects root clauses, discovers
//! concrete body instances, stabilizes outputs, stabilizes unavailable boundaries,
//! and publishes model facts in source-item order before marking definition cycles.
//!
//! - `discovery` owns the growing instance list. Traversal remains child-first;
//!   concrete parameter tuples, ancestry and recursive flags retain their order.
//! - `inference` owns each fixed-point result. Outputs stabilize first. Boundary
//!   propagation reads a complete previous-round snapshot and retains full equality.
//! - `interpretation` borrows instances and the applicable boundary snapshot.
//!   Its coordinating path checks forwarding cycles and invocation sources before
//!   dispatching named clause handlers, then applies output eligibility separately.
//! - `model_roots` publishes successful definitions alongside unavailable siblings.
//!   Inspected locals require a complete owning source item and passing siblings.
//! - `source` borrows immutable facts and optional lookup views. Its source families
//!   keep owning scopes, written bodies, metadata, primitives and error checks together.
//!   Body-aware source coordination stays on `BodyInterpreter`: Float lets reuse the
//!   ordered Local interpreter with the current instances and boundaries.
//! - `dependencies` returns strict ordered declaration dependencies or a refusal.
//!   Checked Unknown inspection cannot satisfy a proof-sensitive dependency path.
//! - `adapters` preserves the consumer entry points and their Some/None lookup choices.
//!   Selected regular and sliding bodies use isolated discovery without inference.
//!
//! Invocation actuals and the active forwarding stack exist only during their
//! original inspection. Source inspection supplies no value, nonemptiness,
//! membership, output or search-coverage proof. Lookup indexes retain original-row
//! selection and physical ranges; substituted callable views use the linear fallback.
//! No source checker rebuilds indexes or caches semantic results.
use std::collections::HashMap;

use crate::callables::{
    array_concatenation, call_argument, core_operation, find_node, formal_parameter,
    instantiated_body, is_expression, operation_fact,
};
use crate::definitions::complete_array_coverage;
use crate::domains::{NumericBound, expression_domain};
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
    let lookups = source::LookupIndexes::new(context, bindings, calls);
    let source = SourceInspector::new(
        context,
        bindings,
        calls,
        instantiations,
        domains,
        Some(lookups.borrowed()),
    );
    let roots = discovery::collect_root_clauses(&source);
    let instances = discovery::discover_instances(&source, &roots);
    let outputs = inference::infer_outputs(&source, &instances);
    let boundaries = inference::infer_boundaries(&source, &instances, &outputs);
    let mut facts =
        model_roots::interpret_model_roots(&source, &roots, &instances, outputs, &boundaries);
    crate::definitions::mark_cycles(&mut facts.definitions);
    facts
}

mod adapters;
mod dependencies;
mod discovery;
mod inference;
mod interpretation;
mod model_roots;
mod source;

use interpretation::BodyInterpreter;
use source::SourceInspector;

pub(super) use adapters::{
    DirectSafetyLookups, closed_integer_type_source_error, decision_prefix_source_safety,
    direct_expression_safety, gcc_source_safety, indexed_expression_safety,
    indexed_model_value_fresh_locals, indexed_set_axis_integer_reshape,
    initialized_conditional_bound_safety, initialized_expression_safety,
    initialized_integer_set_source_safety, initialized_parameter_set_extremum_safety,
    length_argument, literal_regular_source_safety, native_operation_source_safe,
    optional_parameter_matrix_initializer_safety, parameter_index_extremum,
    regular_four_source_safety, selected_integer_set_source_safety,
    set_axis_integer_reshape_source, sliding_sum_source_safety,
};

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
// Recover the Local declaration's actual lexical headers for source inspection.
// Later consumers contribute no headers, even when they use this local value.
fn local_source_scope<'a>(
    mut node: &'a SyntaxNode,
    local: &'a SyntaxNode,
) -> Option<Vec<&'a SyntaxNode>> {
    let contains = |outer: &SyntaxNode| {
        outer.range().start <= local.range().start && local.range().end <= outer.range().end
    };
    if local.kind() != NodeKind::Declaration || !contains(node) {
        return None;
    }
    let mut scope = Vec::new();
    while !std::ptr::eq(node, local) {
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
        node = child;
    }
    Some(scope)
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
