//! Publish model definitions in source-item order; certify locals only after every sibling passes.
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
        // supply no ordinary output. Inspect these siblings only when a local
        // choice requires the entire constraint item to be understood.
        let inspection_only = |clause: &Clause<'_>| {
            matches!(clause.kind, ClauseKind::Equality)
                || operator(source.context, clause.file, clause.node)
                    .and_then(crate::bindings::symbolic_operator)
                    .is_some_and(|name| {
                        matches!(name, "->" | "<-" | "\\/")
                            && source.core(clause.file, clause.node, source.calls, name)
                    })
        };
        let mut inspected = Vec::new();
        let mut complete = true;
        for clause in clauses.iter().filter(|clause| !inspection_only(clause)) {
            let mut found = Vec::new();
            let mut unavailable = Vec::new();
            interpreter.interpret_root(
                clause,
                source.calls,
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
            && interpreter.inspected_item_iterations(first.file, first.item, source.calls)
            && clauses
                .iter()
                .filter(|clause| inspection_only(clause))
                .all(|clause| {
                    interpreter
                        .boolean_relation_dependencies(
                            clause.file,
                            clause.node,
                            source.calls,
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
    facts
}
