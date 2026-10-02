//! Formal-input requirements in general user callable bodies, independent of lint.
//! A missing contract is advice about a named boundary, not a failing input witness.
use crate::callables::{find_node, is_expression};
use crate::partial_expression::{captured_by_default, collect_nodes, empty_iteration};
use crate::{
    BindingFacts, BindingResolution, CallableFacts, DeclarationId, DeclarationRole, FileFinding,
    FileId, GuardActivation, GuardObligation, GuardObligationKind, GuardedFacts, GuardedOutcome,
    IterationFacts, ModelContext, ReferenceKind, Rule, Severity, SourceDiagnostic,
};
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputRequirement {
    Nonzero,
    Positive,
    Nonempty,
    Index {
        array: DeclarationId,
        dimension: usize,
    },
}
/// Deduplication describes one emitted operation/operand/kind/dimension, not a
/// whole syntax node. Suppressed findings never provide one of these keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InputObligationKey {
    pub file: FileId,
    pub operation: Range<usize>,
    pub operand: Range<usize>,
    pub requirement: InputRequirement,
}
#[derive(Clone, Debug)]
pub struct CallableInputFact {
    pub callable: DeclarationId,
    pub inputs: Vec<DeclarationId>,
    pub obligation: GuardObligation,
    /// Only the supported core default can capture ordinary undefinedness;
    /// a Boolean ancestor or aborting assertion does not supply this contract.
    pub captured: bool,
}
pub(super) fn obligation_key(o: &GuardObligation) -> Option<InputObligationKey> {
    let requirement = match o.kind {
        GuardObligationKind::Nonzero => InputRequirement::Nonzero,
        GuardObligationKind::Positive => InputRequirement::Positive,
        GuardObligationKind::Nonempty { .. } => InputRequirement::Nonempty,
        GuardObligationKind::Index {
            array, dimension, ..
        } => InputRequirement::Index { array, dimension },
        _ => return None,
    };
    Some(InputObligationKey {
        file: o.file,
        operation: o.operation.range.clone(),
        operand: o.operand.range.clone(),
        requirement,
    })
}
/// Supply same-context bindings, callables, guarded and iteration facts. Bodies
/// use general formals, not selected call instances or replaceable defaults.
/// Formal dependencies include supported immutable local aliases and generator
/// sources. Opaque operation meaning produces no guessed input requirement.
pub fn resolve_callable_input_facts(
    context: &ModelContext,
    bindings: &BindingFacts,
    calls: &CallableFacts,
    guarded: &GuardedFacts,
    iteration: &IterationFacts,
) -> Vec<CallableInputFact> {
    let mut facts = Vec::new();
    for callable in &bindings.declarations {
        if !matches!(
            callable.role,
            DeclarationRole::Function | DeclarationRole::Predicate | DeclarationRole::Test
        ) || !context.files[callable.file].warnings_enabled()
        {
            continue;
        }
        let Some(node) = find_node(
            context.files[callable.file].parsed.tree(),
            &callable.syntax_range,
            callable.role,
        ) else {
            continue;
        };
        let Some(body) = node.child_nodes().find(|n| is_expression(n.kind())) else {
            continue;
        };
        let source = &context.files[callable.file];
        let body_location = source.location(body.range());
        let mut nodes = Vec::new();
        collect_nodes(body, &mut nodes);
        for o in &guarded.obligations {
            if o.file != callable.file
                || !contains(&body_location.range, &o.operation.range)
                || obligation_key(o).is_none()
            {
                continue;
            }
            let mut inputs = BTreeSet::new();
            let mut active = BTreeSet::new();
            let mut supported = true;
            // Unsupported arity/type can involve any argument; preserve a
            // located limitation when a formal participates, never a contract.
            let dependency_range = if matches!(o.outcome, GuardedOutcome::Unsupported(_)) {
                &o.operation.range
            } else {
                &o.operand.range
            };
            for r in bindings.references.iter().filter(|r| {
                r.file == o.file
                    && contains(dependency_range, &r.location.range)
                    && r.kind == ReferenceKind::Value
            }) {
                match r.resolution {
                    BindingResolution::Resolved(id) => {
                        supported &= input_dependencies(
                            context,
                            bindings,
                            iteration,
                            guarded,
                            callable.id,
                            id,
                            &mut inputs,
                            &mut active,
                        )
                    }
                    _ => supported = false,
                }
            }
            if let GuardObligationKind::Index { array, .. } = o.kind {
                supported &= input_dependencies(
                    context,
                    bindings,
                    iteration,
                    guarded,
                    callable.id,
                    array,
                    &mut inputs,
                    &mut active,
                );
            }
            if inputs.is_empty() {
                continue;
            }
            let mut obligation = o.clone();
            if let Some(expression) = guarded.expression(o.file, &o.operand)
                && let GuardedOutcome::Unsupported(reason) = &expression.raw_definedness
            {
                obligation.outcome = GuardedOutcome::Unsupported(reason.clone());
            }
            if let crate::DefinitionEnforcement::Unsupported(reason) = &o.context.enforcement {
                obligation.outcome = GuardedOutcome::Unsupported(reason.clone());
            }
            if !supported {
                obligation.outcome = GuardedOutcome::Unsupported(
                    "formal dependency mapping is outside the supported alias/generator subset"
                        .into(),
                );
            }
            let captured =
                captured_by_default(context, bindings, calls, guarded, o.file, &nodes, o)
                    || empty_iteration(iteration, o.file, &o.operation);
            facts.push(CallableInputFact {
                callable: callable.id,
                inputs: inputs.into_iter().map(DeclarationId).collect(),
                obligation,
                captured,
            });
        }
    }
    facts
}
fn contains(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.start && b.end <= a.end
}
#[allow(clippy::too_many_arguments)]
fn input_dependencies(
    context: &ModelContext,
    bindings: &BindingFacts,
    iteration: &IterationFacts,
    guarded: &GuardedFacts,
    owner: DeclarationId,
    id: DeclarationId,
    inputs: &mut BTreeSet<usize>,
    active: &mut BTreeSet<usize>,
) -> bool {
    let d = &bindings.declarations[id.0];
    let callable = &bindings.declarations[owner.0];
    if d.file == callable.file
        && d.role == DeclarationRole::Parameter
        && contains(&callable.syntax_range, &d.syntax_range)
    {
        inputs.insert(id.0);
        return true;
    }
    if !matches!(d.role, DeclarationRole::Local | DeclarationRole::Generator) {
        return true;
    }
    if !active.insert(id.0) {
        return false;
    }
    let node = if d.role == DeclarationRole::Generator {
        iteration
            .iterations
            .iter()
            .flat_map(|i| &i.generators)
            .find(|g| g.bindings.contains(&id))
            .and_then(|g| crate::guarded::node_at(context, d.file, &g.source))
    } else if d.instantiation == crate::Instantiation::Parameter {
        find_node(context.files[d.file].parsed.tree(), &d.syntax_range, d.role)
            .and_then(|n| n.child_nodes().find(|n| is_expression(n.kind())))
    } else {
        None
    };
    let result = node.is_some_and(|node| {
        let location = context.files[d.file].location(node.range());
        let supported_evaluation = guarded
            .expression(d.file, &location)
            .is_some_and(|e| !matches!(e.raw_definedness, GuardedOutcome::Unsupported(_)));
        let dependencies = bindings
            .references
            .iter()
            .filter(|r| {
                r.file == d.file
                    && contains(&location.range, &r.location.range)
                    && r.kind == ReferenceKind::Value
            })
            .all(|r| match r.resolution {
                BindingResolution::Resolved(other) => input_dependencies(
                    context, bindings, iteration, guarded, owner, other, inputs, active,
                ),
                _ => false,
            });
        dependencies && supported_evaluation
    });
    active.remove(&id.0);
    result
}
pub(super) fn check_input_preconditions(
    context: &ModelContext,
    bindings: &BindingFacts,
    facts: &[CallableInputFact],
) -> (
    Vec<FileFinding>,
    Vec<SourceDiagnostic>,
    Vec<InputObligationKey>,
) {
    let mut findings = Vec::new();
    let mut limits = Vec::new();
    let mut keys = Vec::new();
    for fact in facts {
        let o = &fact.obligation;
        let callable = &bindings.declarations[fact.callable.0];
        let source = &context.files[o.file];
        if !source.warnings_enabled()
            || source
                .suppressions
                .as_ref()
                .is_none_or(|s| s[callable.item].contains(&Rule::MissingInputPrecondition))
            || fact.captured
            || o.context.activation == GuardActivation::Inactive
            || o.outcome == GuardedOutcome::Proven
        {
            continue;
        }
        if let GuardedOutcome::Unsupported(reason) = &o.outcome {
            limits.push(SourceDiagnostic {
                location: o.operation.clone(),
                message: format!("missing-input-precondition: {reason}"),
            });
            continue;
        }
        let names = fact
            .inputs
            .iter()
            .map(|id| bindings.declarations[id.0].name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let operand = source.parsed.source()
            [o.operand.range.start - source.byte_offset..o.operand.range.end - source.byte_offset]
            .trim();
        let condition = match &o.kind {
            GuardObligationKind::Nonzero => format!("'{operand}' must be nonzero"),
            GuardObligationKind::Positive => format!("'{operand}' must be strictly positive"),
            GuardObligationKind::Nonempty { aggregate } => format!(
                "{aggregate} requires a nonempty collection of present values in '{operand}'"
            ),
            GuardObligationKind::Index {
                array, dimension, ..
            } => format!(
                "'{operand}' must belong to array '{}' dimension {dimension}'s index set",
                bindings.declarations[array.0].name
            ),
            _ => continue,
        };
        findings.push(FileFinding { fix: None,
            location: o.operation.clone(),
            rule: Rule::MissingInputPrecondition,
            severity: Severity::Warning,
            message: format!(
                "callable '{}' (boundary {}:{}:{}) uses inputs [{names}] without an established condition: {condition}; this supported body requirement is neither declared nor protected here by a guard/assertion; evaluation may be conditional, and this advice does not establish a failing input, model feasibility or runtime error",
                callable.name,
                callable.location.path.display(),
                callable.location.line,
                callable.location.column
            ),
        });
        keys.push(obligation_key(o).unwrap());
    }
    (findings, limits, keys)
}
