//! Publish model definitions and unavailable siblings in source-item order.
use super::*;

pub(super) fn interpret_model_roots<'a>(
    source: &SourceInspector<'a>,
    roots: &[Clause<'a>],
    instances: &[Instance<'a>],
    outputs: Vec<CallableOutput>,
    boundaries: &[Boundary],
) -> CallableDefinitionFacts {
    let interpreter = BodyInterpreter::new(source, instances, boundaries);
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
        // Equalities retain their direct interpreter, and core implication/disjunction
        // supply no ordinary callable output. Their direct definitions retain
        // their separate producer.
        let inspection_only = |clause: &Clause<'_>| {
            matches!(clause.kind, ClauseKind::Equality)
                || operator(source.context, clause.file, clause.node)
                    .and_then(crate::bindings::symbolic_operator)
                    .is_some_and(|name| {
                        matches!(name, "->" | "<-" | "\\/")
                            && source.core(clause.file, clause.node, source.calls, name)
                    })
        };
        let mut definitions = Vec::new();
        let mut known_failure = false;
        for clause in clauses {
            if inspection_only(clause) {
                if let Some(reason) = opaque_source_error(
                    source.context,
                    source.bindings,
                    source.calls,
                    source.instantiations,
                    source.domains,
                    clause.file,
                    clause.node,
                ) {
                    known_failure = true;
                    facts.unavailable.push(UnavailableCallableDefinition {
                        location: source.context.files[clause.file].location(clause.node.range()),
                        targets: Vec::new(),
                        status: CallableDefinitionStatus::Unsupported,
                        operation: CallableDefinitionOperation::Source,
                        reason,
                    });
                }
                continue;
            }
            let mut found = Vec::new();
            let mut unavailable = Vec::new();
            interpreter.interpret_root(
                clause,
                source.calls,
                &facts.outputs,
                &mut found,
                &mut unavailable,
            );
            for output in found {
                definitions.push(Definition {
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
        if !known_failure {
            facts.definitions.extend(definitions);
        }
        cursor += count;
    }
    facts
}
