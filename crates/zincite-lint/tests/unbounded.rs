use std::path::PathBuf;
use zincite_lint::{
    DefinitionCoverage, DefinitionFacts, LintOptions, ModelOptions, Rule, RuleOutcome,
    analyze_model, load_model, resolve_bindings, resolve_callables, resolve_definitions,
    resolve_domains, resolve_instantiations,
};
fn model(name: &str, source: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let directory =
        std::env::temp_dir().join(format!("zincite-unbounded-{name}-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        "function var bool: forall(array[int] of var opt bool: body);",
    )
    .unwrap();
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
    LintOptions {
        rules: vec![Rule::UnboundedVariable],
    }
}
#[test]
fn numeric_aliases_and_complete_anchored_definitions_keep_policy_separate() {
    let source = concat!(
        "\u{feff}% é\r\n",
        "type Integer=var int; type Real=var float; type Bounded=var 0..5; type RealBound=var 0.0..5.0;\n",
        "Integer: scalar; Real: floating; array[1..2] of Integer: integers; array[1..2] of Real: floats;\n",
        "Bounded: bounded; RealBound: real_bound; var bool: flag; var set of int: choices; var string: text;\n",
        "Integer: initialized=bounded; Real: initialized_real=1.0; array[1..2] of Integer: initialized_array=[1,2];\n",
        "Integer: anchored; Integer: chained; constraint anchored=bounded; constraint chained=anchored;\n",
        "array[1..2] of var int: whole; constraint forall(i in 1..2)(whole[i]=i);\n",
        "% zincite-lint: ignore unbounded-variable\nInteger: suppressed; solve satisfy;\n"
    );
    let (directory, context) = model("positive", source);
    let bindings = resolve_bindings(&context);
    let domains = resolve_domains(&context, &bindings);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let definitions: DefinitionFacts =
        resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let anchored = bindings
        .declarations
        .iter()
        .find(|d| d.name == "anchored")
        .unwrap()
        .id;
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| d.target == anchored && d.cyclic)
    );
    assert!(
        definitions
            .bounded_or_defined_targets(&bindings, &domains)
            .contains(&anchored)
    );
    let floating = bindings
        .declarations
        .iter()
        .find(|d| d.name == "floating")
        .unwrap()
        .id;
    assert_eq!(
        domains.declarations[floating.0]
            .domain
            .has_unbounded_numeric_elements(),
        Ok(true)
    );
    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    let names: Vec<_> = result
        .findings
        .iter()
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(names, ["scalar", "floating", "integers", "floats"]);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(
        std::fs::read_to_string(directory.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(directory).unwrap();
}
#[test]
fn conditional_partial_and_unanchored_cycles_still_receive_domain_advice() {
    let source = concat!(
        "var bool: gate; var int: conditional; constraint gate -> conditional=1;\n",
        "var int: self=self; var int: first; var int: second; constraint first=second;\n",
        "array[1..3] of var int: partial; constraint partial[1]=1;\n",
        "array[1..3] of var float: filtered; constraint forall(i in 1..3 where i>1)(filtered[i]=1.0);\n",
        "array[1..2] of var 0..5: bounded_array; array[1..2] of var int: limited_array; constraint forall(i in 1..2)(limited_array[i]=bounded_array[i]);\n",
        "solve satisfy;\n"
    );
    let (directory, context) = model("negative", source);
    let result = analyze_model(&context, &selected());
    assert_eq!(result.limitations.len(), 1, "{:?}", result.limitations);
    assert_eq!(
        &source[result.limitations[0].location.range.clone()],
        "limited_array"
    );
    let names: Vec<_> = result
        .findings
        .iter()
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(
        names,
        [
            "conditional",
            "self",
            "first",
            "second",
            "partial",
            "filtered"
        ]
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let bindings = resolve_bindings(&context);
    let domains = resolve_domains(&context, &bindings);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let defs = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    assert!(
        defs.definitions
            .iter()
            .any(|d| d.coverage == DefinitionCoverage::ArrayElement)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
