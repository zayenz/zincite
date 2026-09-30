use std::path::PathBuf;
use zincite_lint::{
    CompactIfOutcome, DefinitionSafety, LintOptions, ModelOptions, RuleOutcome, analyze_model,
    expression_safety, load_model, resolve_bindings, resolve_callables, resolve_compact_ifs,
    resolve_instantiations,
};
fn model(name: &str, source: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let directory =
        std::env::temp_dir().join(format!("zincite-compact-{name}-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(directory.join("library/std/stdlib.mzn"), "").unwrap();
    let root = directory.join("root.mzn");
    std::fs::write(&root, source).unwrap();
    let context = load_model(
        &root,
        &ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(directory.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (directory, context)
}
fn selected() -> LintOptions {
    LintOptions::from_selection("compact-if").unwrap()
}
#[test]
fn both_zero_orders_have_independent_facts_original_ranges_and_suppression() {
    let source = concat!(
        "\u{feff}% é\r\ntype Flag=var bool; Flag: choice; var int: value; int: zero=2-2;\n",
        "var int: first=if choice then value+1 else 0 endif;\n",
        "var int: second=if choice then zero else value endif;\n",
        "% zincite-lint: ignore compact-if\nvar int: suppressed=if choice then 0 else value endif; solve satisfy;\n"
    );
    let (directory, context) = model("positive", source);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let facts = resolve_compact_ifs(&context, &bindings, &calls, &inst);
    assert_eq!(
        facts
            .conditionals
            .iter()
            .map(|f| &f.outcome)
            .collect::<Vec<_>>(),
        [
            &CompactIfOutcome::Eligible {
                zero_in_then: false
            },
            &CompactIfOutcome::Eligible { zero_in_then: true },
            &CompactIfOutcome::Eligible { zero_in_then: true }
        ]
    );
    let root_file = context
        .files
        .iter()
        .position(|f| f.path == directory.join("root.mzn"))
        .unwrap();
    fn atom(node: &zincite_syntax::SyntaxNode) -> Option<&zincite_syntax::SyntaxNode> {
        if node.kind() == zincite_syntax::NodeKind::Expression {
            Some(node)
        } else {
            node.child_nodes().find_map(atom)
        }
    }
    let expression = atom(context.files[root_file].parsed.tree()).unwrap();
    assert_eq!(
        expression_safety(&context, &bindings, &calls, root_file, expression),
        DefinitionSafety::Supported
    );
    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| &source[f.location.range.clone()])
            .collect::<Vec<_>>(),
        [
            "if choice then value+1 else 0 endif",
            "if choice then zero else value endif"
        ]
    );
    assert!(result.findings[0].message.contains("bool2int(condition)"));
    assert!(
        result.findings[1]
            .message
            .contains("bool2int(not condition)")
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn parameter_noninteger_optional_and_partial_forms_receive_no_replacement_advice() {
    let source = concat!(
        "bool: parameter=true; var bool: choice; var int: value; var opt int: optional; int: divisor;\n",
        "var int: par_case=if parameter then value else 0 endif; var float: float_case=if choice then 1.0 else 0 endif;\n",
        "var opt int: opt_case=if choice then optional else 0 endif; var int: no_zero=if choice then 1 else 2 endif;\n",
        "var int: partial=if choice then 1 div 0 else 0 endif; var int: symbolic=if choice then value div divisor else 0 endif;\n",
        "var Missing: unknown; var int: missing=if choice then unknown else 0 endif; solve satisfy;\n"
    );
    let (directory, context) = model("negative", source);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let facts = resolve_compact_ifs(&context, &bindings, &calls, &inst);
    assert!(
        facts.conditionals[..4]
            .iter()
            .all(|f| f.outcome == CompactIfOutcome::NotApplicable)
    );
    assert!(matches!(
        facts.conditionals[4].outcome,
        CompactIfOutcome::Unsupported(_)
    ));
    assert!(matches!(
        facts.conditionals[5].outcome,
        CompactIfOutcome::Unknown(_)
    ));
    assert!(matches!(
        facts.conditionals[6].outcome,
        CompactIfOutcome::Unsupported(_)
    ));
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty());
    assert_eq!(result.limitations.len(), 3, "{:?}", result.limitations);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(directory).unwrap();
}
