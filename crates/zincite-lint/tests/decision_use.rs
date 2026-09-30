use std::path::{Path, PathBuf};
use zincite_lint::{
    Instantiation, InstantiationFacts, LintOptions, ModelOptions, Rule, RuleOutcome, analyze_model,
    load_model, resolve_bindings, resolve_callables, resolve_instantiations,
};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
fn setup(name: &str) -> (PathBuf, ModelOptions) {
    let directory =
        std::env::temp_dir().join(format!("zincite-decision-{name}-{}", std::process::id()));
    let library = directory.join("library");
    write(
        &library.join("std/stdlib.mzn"),
        concat!(
            "function var bool: forall(array[int] of var opt bool: x);\n",
            "function int: 'div'(int: x,int: y); function var int: 'div'(var int: x,var int: y);\n",
            "function bool: 'not'(bool: x); function var bool: 'not'(var bool: x);\n",
        ),
    );
    (
        directory,
        ModelOptions {
            stdlib_dir: Some(library),
            include_dirs: Vec::new(),
        },
    )
}
fn selected() -> LintOptions {
    LintOptions::from_selection(
        "decision-variable-operator,decision-variable-generator,decision-variable-condition",
    )
    .unwrap()
}
fn fact<'a>(
    facts: &'a InstantiationFacts,
    path: &Path,
    source: &str,
    text: &str,
) -> &'a zincite_lint::ExpressionInstantiation {
    facts
        .expressions
        .iter()
        .find(|f| f.location.path == path && &source[f.location.range.clone()] == text)
        .unwrap()
}

#[test]
fn standalone_facts_keep_component_call_let_and_generator_boundaries() {
    let (directory, options) = setup("facts");
    let root = directory.join("root.mzn");
    let source = concat!(
        "\u{feff}% é\r\nint: symbolic; type Decision = var 1..3; Decision: value; var bool: choice; var opt int: optional;\r\n",
        "array[1..3] of int: fixed; tuple(int,var int): pair; record(int: fixed,var int: decision): rec;\n",
        "function var int: decision_value()=value; function bool: parameter_result(var bool: ignored)=true;\n",
        "float: parameter_infinity=1/infinity;\n",
        "int: one=let {var int: unused; constraint unused>0;} in symbolic;\n",
        "var int: indexed=fixed[value]; int: projected=pair.1; var int: second=pair.2;\n",
        "int: field=rec.fixed; var int: other_field=rec.decision; var int: called=decision_value();\n",
        "bool: returned=parameter_result(choice); var int: branch=if true then value else symbolic endif;\n",
        "var set of 1..3: chosen; array[int] of var opt int: hidden=[i div 2|i in chosen];\n",
        "array[int] of var opt int: assigned=[j div 2|i=optional+1,j=i where j>0];\n",
        "array[int] of var int: nested=[let {int: value=i;} in value|i in 1..3];\n",
        "constraint forall(i in 1..decision_value() where choice)(i>0); solve satisfy;\n",
    );
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let callables = resolve_callables(&context, &bindings);
    let facts = resolve_instantiations(&context, &bindings, &callables);
    for text in [
        "symbolic;\n",
        "infinity",
        "1/infinity",
        "pair.1",
        "rec.fixed",
        "parameter_result(choice)",
        "let {var int: unused; constraint unused>0;} in symbolic",
        "i div 2",
    ] {
        let text = text.trim_end_matches([';', '\n']);
        assert_eq!(
            fact(&facts, &root, source, text).instantiation,
            Instantiation::Parameter,
            "{text}"
        );
    }
    for text in [
        "fixed[value]",
        "pair.2",
        "rec.decision",
        "decision_value();\n",
        "if true then value else symbolic endif",
        "[i div 2|i in chosen]",
        "optional+1",
        "j div 2",
        "j>0",
    ] {
        let text = text.trim_end_matches([';', '\n']);
        assert_eq!(
            fact(&facts, &root, source, text).instantiation,
            Instantiation::Decision,
            "{text}"
        );
    }
    assert!(
        facts
            .expressions
            .iter()
            .all(|f| (f.instantiation == Instantiation::Unknown) == f.reason.is_some())
    );
    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    let ranges: Vec<_> = result
        .findings
        .iter()
        .map(|f| (f.rule, &source[f.location.range.clone()]))
        .collect();
    assert_eq!(
        ranges,
        vec![
            (Rule::DecisionVariableOperator, "div"),
            (Rule::DecisionVariableGenerator, "chosen"),
            (Rule::DecisionVariableGenerator, "optional+1"),
            (Rule::DecisionVariableGenerator, "i"),
            (Rule::DecisionVariableGenerator, "1..decision_value()"),
            (Rule::DecisionVariableCondition, "j>0"),
            (Rule::DecisionVariableCondition, "choice")
        ]
    );
    assert!(
        result
            .rules
            .iter()
            .all(|r| r.outcome == RuleOutcome::Completed)
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn operator_families_conditions_and_sources_use_separate_facts_and_suppressions() {
    let (directory, options) = setup("policy");
    let root = directory.join("root.mzn");
    let included = directory.join("included.mzn");
    let source = concat!(
        "include \"included.mzn\"; int: parameter; var 1..3: value; bool: fixed; var bool: choice;\n",
        "var int: power=value^2; var int: quotient=value div 2; var int: remainder=value mod 2; var float: division=value/2;\n",
        "var bool: a=choice xor fixed; var bool: b=choice \\/ fixed; var bool: c=choice -> fixed; var bool: d=choice <- fixed; var bool: e=choice <-> fixed; var bool: f=not choice;\n",
        "var bool: g=choice ∨ fixed; var bool: h=choice → fixed; var bool: k=choice ← fixed; var bool: l=choice ↔ fixed; var bool: m=¬choice;\n",
        "int: quiet=parameter^2+parameter div 2+parameter mod 2; float: quiet_division=parameter/2; bool: quiet_logic=not fixed \\/ (fixed xor true) -> true;\n",
        "var int: quoted='div'(value,2); var bool: quoted_not='not'(choice);\n",
        "function var int: 'mod'(var int: x,var int: y)=x; var int: user='mod'(value,2);\n",
        "var int: branches=if fixed then value elseif choice then 2 else 3 endif;\n",
        "array[int] of var int: body=[value|i in 1..3 where fixed]; constraint forall(i in 1..3)(choice);\n",
        "solve satisfy;\n",
    );
    let shared = "% zincite-lint: ignore decision-variable-operator\n% zincite-lint: ignore decision-variable-condition\nvar int: ignored=if choice then value^2 else 1 endif;\nvar int: visible=if choice then value^2 else 1 endif;\n";
    write(&root, source);
    write(&included, shared);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    let operators: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == Rule::DecisionVariableOperator && f.location.path == root)
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(
        operators,
        vec![
            "^", "div", "mod", "/", "xor", "\\/", "->", "<-", "<->", "not", "∨", "→", "←", "↔",
            "¬", "'div'", "'not'"
        ]
    );
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.rule == Rule::DecisionVariableGenerator)
            .count(),
        0
    );
    let conditions: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == Rule::DecisionVariableCondition && f.location.path == root)
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(conditions, ["choice"]);
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.location.path == included)
            .count(),
        2
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| !f.message.contains("faster") && !f.message.contains("guarantee"))
    );
    let system = load_model(
        options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn"),
        &options,
    );
    assert!(analyze_model(&system, &selected()).findings.is_empty());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unknown_relevant_facts_limit_only_their_policy_and_preserve_proved_findings() {
    let (directory, options) = setup("unknown");
    let root = directory.join("root.mzn");
    let source = "var bool: decision; bool: fixed; function bool: known()=true; function MissingType: uncertain(bool: x,bool: y); constraint if known() `uncertain` fixed then decision else true endif; solve satisfy;";
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let bindings = resolve_bindings(&context);
    let callables = resolve_callables(&context, &bindings);
    let facts = resolve_instantiations(&context, &bindings, &callables);
    assert_eq!(
        fact(&facts, &root, source, "known() `uncertain` fixed").instantiation,
        Instantiation::Unknown
    );
    let selections=LintOptions::from_selection("decision-variable-operator,decision-variable-generator,decision-variable-condition,global-variable-in-function").unwrap();
    let result = analyze_model(&context, &selections);
    assert_eq!(result.status(), 0);
    assert_eq!(result.limitations.len(), 1);
    assert!(matches!(
        result.rules[2].outcome,
        RuleOutcome::Limited { .. }
    ));
    for i in [0, 1, 3] {
        assert_eq!(result.rules[i].outcome, RuleOutcome::Completed);
    }
    write(
        &root,
        "var bool: decision; constraint if decision then true else false endif; constraint if missing() then true else false endif; constraint 2 ^ unresolved=1; array[int] of int: a=[i|i in unavailable()]; solve satisfy;",
    );
    let result = analyze_model(&load_model(&root, &options), &selected());
    assert_eq!(result.findings.len(), 1);
    assert_eq!(result.limitations.len(), 3);
    write(
        &root,
        "constraint if missing() then true else false endif; solve satisfy;",
    );
    let result = analyze_model(&load_model(&root, &options), &selected());
    assert_eq!(result.status(), 0);
    assert_eq!(result.limitations.len(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}
