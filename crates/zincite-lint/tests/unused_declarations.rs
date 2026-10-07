use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, DeclarationRole, LintOptions, ModelContext, ModelOptions, ModelRootState, Rule,
    RuleOutcome, UsageFacts, UsageOutcome, analyze_model, load_model, resolve_bindings,
    resolve_callables, resolve_unused_declarations,
};
const CORE: &str = concat!(
    "function bool: '='($T: left,$T: right); function var bool: '='(any $T: left,any $T: right);\n",
    "function int: '+'(int: left,int: right); function int: '-'(int: left,int: right); function set of int: '..'(int: left,int: right);\n",
    "function string: show(any $T: value); annotation no_output; annotation add_to_output;\n",
    "annotation output_var; annotation output_only; annotation annotated_expression;\n",
    "function ann: 'output'(any $T: value :: annotated_expression);\n",
    "function ann: search_marker(int: value);\n",
    "function int: standard_bridge(int: value)=used_global+value; constraint 1=1;\n"
);
fn model(name: &str, source: &str, included: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!("zincite-unused-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("included.mzn"), included).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (dir, context)
}
fn selected() -> LintOptions {
    LintOptions::from_selection("unused-declaration").unwrap()
}
fn facts(context: &ModelContext) -> (BindingFacts, UsageFacts) {
    let bindings = resolve_bindings(context);
    let calls = resolve_callables(context, &bindings);
    let usage = resolve_unused_declarations(context, &bindings, &calls);
    (bindings, usage)
}
fn outcome(bindings: &BindingFacts, usage: &UsageFacts, name: &str) -> UsageOutcome {
    let d = bindings
        .declarations
        .iter()
        .find(|d| d.name == name && d.top_level)
        .unwrap();
    usage.declarations[d.id.0].outcome
}
fn warnings(context: &ModelContext) -> Vec<String> {
    let result = analyze_model(context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    result
        .findings
        .iter()
        .map(|f| {
            let source = &context
                .files
                .iter()
                .find(|s| s.path == f.location.path)
                .unwrap();
            source.parsed.source()[f.location.range.start - source.byte_offset
                ..f.location.range.end - source.byte_offset]
                .to_owned()
        })
        .collect()
}
#[test]
fn complete_roots_follow_selected_dependencies_without_conflating_containment() {
    let source = concat!(
        "include \"included.mzn\"; int: seed=2; type Bounded=0..seed; var Bounded: live_value;\n",
        "var 0..3: unused_decision; int: unused_parameter=5; int: unused_nested_global=1;\n",
        "int: unused_outer=let { int: nested=3; constraint unused_nested_global=1; } in nested;\n",
        "int: _discarded=1; int: search_seed=2;\n",
        "function int: chosen(int: value)=live_cycle_a(value)+standard_bridge(value);\n",
        "function float: chosen(float: value)=value;\n",
        "constraint live_value=chosen(1); solve :: search_marker(search_seed) satisfy; output [show(live_value)];\n"
    );
    let included = concat!(
        "\u{feff}% é\r\nint: used_global=1; int: unused_global=9;\r\n",
        "function int: live_cycle_a(int: depth)=let { int: unused_local=3; } in if depth=0 then used_global else live_cycle_b(depth-1) endif;\n",
        "function int: live_cycle_b(int: depth)=if depth=0 then used_global else live_cycle_a(depth-1) endif;\n",
        "function int: dead_cycle_a(int: depth)=dead_cycle_b(depth);\n",
        "function int: dead_cycle_b(int: depth)=dead_cycle_a(depth);\n",
        "% zincite-lint: ignore unused-declaration\nint: suppressed=let { int: nested_suppressed=0; } in nested_suppressed;\n"
    );
    let (dir, context) = model("dependencies", source, included);
    let (bindings, usage) = facts(&context);
    assert_eq!(usage.root_state, ModelRootState::Complete);
    assert!(usage.limitations.is_empty(), "{:?}", usage.limitations);
    for name in [
        "seed",
        "Bounded",
        "live_value",
        "used_global",
        "live_cycle_a",
        "live_cycle_b",
        "search_seed",
        "standard_bridge",
    ] {
        assert_eq!(
            outcome(&bindings, &usage, name),
            UsageOutcome::Reachable,
            "{name}"
        );
    }
    assert_eq!(
        outcome(&bindings, &usage, "suppressed"),
        UsageOutcome::Unreachable
    );
    let nested = bindings
        .declarations
        .iter()
        .find(|d| d.name == "nested")
        .unwrap();
    let enclosing = usage.declarations[nested.id.0].enclosing.unwrap();
    assert_eq!(bindings.declarations[enclosing.0].name, "unused_outer");
    let mut names = warnings(&context);
    names.sort();
    assert_eq!(
        names,
        [
            "chosen",
            "dead_cycle_a",
            "dead_cycle_b",
            "unused_decision",
            "unused_global",
            "unused_local",
            "unused_nested_global",
            "unused_outer",
            "unused_parameter"
        ]
    );
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    let included_warning = result
        .findings
        .iter()
        .find(|f| f.message.contains("'unused_global'"))
        .unwrap();
    assert_eq!(
        &std::fs::read(&included_warning.location.path).unwrap()
            [included_warning.location.range.clone()],
        b"unused_global"
    );
    assert!(
        !result
            .findings
            .iter()
            .any(|f| f.location.path.starts_with(dir.join("library")))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn default_output_roots_follow_confirmed_markers_and_initializer_shapes() {
    let cases = [
        (
            "ordinary",
            "type Bit=var 0..1; Bit: alias_bit; var int: plain; var int: fixed=1; var int: copied=plain; var int: assigned; assigned=plain; array[1..2] of var int: fixed_array=[0,1]; array[1..2] of var int: anonymous_array=[0,_]; var int: hidden :: no_output; int: parameter_value=2; solve satisfy;",
            vec!["alias_bit", "plain", "assigned", "anonymous_array"],
            vec![
                "fixed",
                "copied",
                "fixed_array",
                "hidden",
                "parameter_value",
            ],
        ),
        (
            "add",
            "var int: ordinary; var int: forced :: add_to_output=1; var int: marked :: output_var; solve satisfy;",
            vec!["forced"],
            vec!["ordinary", "marked"],
        ),
        (
            "supplement",
            "var int: ordinary; var int: forced :: output=1; solve satisfy;",
            vec!["ordinary", "forced"],
            vec![],
        ),
        (
            "explicit",
            "var int: ordinary; int: used_global=1; output [show(used_global)]; solve satisfy;",
            vec!["used_global"],
            vec!["ordinary"],
        ),
    ];
    for (name, source, roots, unused) in cases {
        let (dir, context) = model(name, source, "");
        let (bindings, usage) = facts(&context);
        assert!(
            usage.limitations.is_empty(),
            "{name}: {:?}",
            usage.limitations
        );
        for name in roots {
            assert_eq!(
                outcome(&bindings, &usage, name),
                UsageOutcome::Reachable,
                "{name}"
            );
        }
        for name in unused {
            assert_eq!(
                outcome(&bindings, &usage, name),
                UsageOutcome::Unreachable,
                "{name}"
            );
        }
        assert!(matches!(
            analyze_model(&context, &selected()).rules[0].outcome,
            RuleOutcome::Completed
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
    let (dir, context) = model(
        "unknown-output",
        "ann: marker=no_output; var int: ordinary :: marker; Missing: unknown; solve satisfy;",
        "",
    );
    let (bindings, usage) = facts(&context);
    assert!(!usage.limitations.is_empty());
    assert_ne!(
        outcome(&bindings, &usage, "ordinary"),
        UsageOutcome::Unreachable
    );
    assert_ne!(
        outcome(&bindings, &usage, "unknown"),
        UsageOutcome::Unreachable
    );
    assert!(matches!(
        analyze_model(&context, &selected()).rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn uncertainty_stays_local_and_fragments_do_not_claim_unused_exports() {
    let source = "int: used_global=1; int: dead=1; function bool: '!='(int: left,int: right)=left=right; function int: dead_call()=missing(); constraint used_global!=0; solve satisfy; output [];";
    let (dir, context) = model("symbolic", source, "");
    let (bindings, usage) = facts(&context);
    assert!(usage.limitations.is_empty(), "{:?}", usage.limitations);
    let op = bindings
        .declarations
        .iter()
        .find(|d| d.name == "!=" && d.role == DeclarationRole::Function)
        .unwrap();
    assert_eq!(usage.declarations[op.id.0].outcome, UsageOutcome::Reachable);
    assert_eq!(
        outcome(&bindings, &usage, "dead_call"),
        UsageOutcome::Unreachable
    );
    assert_eq!(warnings(&context), ["dead", "dead_call"]);
    std::fs::write(dir.join("root.mzn"),"int: used_global=1; int: dead=1; function int: possible(int: value)=value; function int: possible(float: value)=1; constraint possible(missing)=1; solve satisfy; output [];").unwrap();
    let options = ModelOptions {
        include_dirs: vec![],
        stdlib_dir: Some(dir.join("library")),
    };
    let uncertain = load_model(dir.join("root.mzn"), &options);
    let options_rules =
        LintOptions::from_selection("unused-declaration,global-variable-in-function").unwrap();
    let result = analyze_model(&uncertain, &options_rules);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(matches!(result.rules[1].outcome, RuleOutcome::Completed));
    assert!(!result.limitations.is_empty());
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.rule != Rule::UnusedDeclaration)
    );
    std::fs::write(
        dir.join("root.mzn"),
        "int: exported=1; function int: exposed(int: value)=value;",
    )
    .unwrap();
    let fragment = load_model(dir.join("root.mzn"), &options);
    let (bindings, usage) = facts(&fragment);
    assert_eq!(usage.root_state, ModelRootState::Fragment);
    assert_eq!(
        outcome(&bindings, &usage, "exported"),
        UsageOutcome::Uncertain
    );
    let result = analyze_model(&fragment, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Inapplicable { .. }
    ));
    assert!(result.findings.is_empty() && result.limitations.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn selected_generic_bodies_keep_concrete_optional_dependencies_and_raw_unknowns() {
    let source = "opt int: value=<>; constraint check_absence(value); solve satisfy; output [];";
    let (dir, _) = model("concrete-body", source, "");
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\ntest occurs(opt $T: value); function bool: 'not'(bool: value); test check_absence(opt $T: value)=not occurs(value);\n")).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    assert!(calls.calls.iter().any(|call| call.name == "occurs"
        && matches!(call.outcome, zincite_lint::CallOutcome::Unsupported { .. })));
    let usage = resolve_unused_declarations(&context, &bindings, &calls);
    assert!(usage.limitations.is_empty(), "{:?}", usage.limitations);
    assert_eq!(outcome(&bindings, &usage, "value"), UsageOutcome::Reachable);
    assert!(matches!(
        analyze_model(&context, &selected()).rules[0].outcome,
        RuleOutcome::Completed
    ));
    // The concrete consumer must not rewrite the context-free generic facts.
    assert!(calls.calls.iter().any(|call| call.name == "occurs"
        && matches!(call.outcome, zincite_lint::CallOutcome::Unsupported { .. })));
    std::fs::write(dir.join("root.mzn"), "opt int: value=<>; constraint check_absence(value); constraint check_absence(missing); solve satisfy; output [];").unwrap();
    let uncertain = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, usage) = facts(&uncertain);
    assert!(!usage.limitations.is_empty());
    assert_eq!(outcome(&bindings, &usage, "value"), UsageOutcome::Reachable);
    assert!(matches!(
        analyze_model(&uncertain, &selected()).rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
