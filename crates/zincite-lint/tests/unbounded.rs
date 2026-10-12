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
        concat!(
            "function var bool: forall(array[int] of var opt bool: body);\n",
            "function var bool: '/\\'(var bool: a, var bool: b);\n",
            "function var bool: 'in'(var int: a, set of int: b);\n",
            "function set of int: '..'(int: a, int: b);\n",
            "function var bool: '>='(var int: a, var int: b);\n",
        ),
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
        ..LintOptions::default()
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

#[test]
fn symbolic_enum_subset_domains_complete_without_inventing_bounds() {
    use zincite_lint::Domain;
    let positive = concat!(
        "enum Participant; set of Participant: Leads;\n",
        "array[Participant] of var Leads: assignment;\n",
        "var Leads: chosen; var Participant: direct; solve satisfy;\n",
    );
    let optional = positive.replace("of var Leads: assignment", "of var opt Leads: assignment");
    for (name, source, completed) in [
        ("symbolic-enum-subset", positive, true),
        ("optional-enum-subset", optional.as_str(), false),
    ] {
        let (directory, context) = model(name, source);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let domains = resolve_domains(&context, &bindings);
        let leads = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == "Leads")
            .unwrap()
            .id;
        let assignment = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == "assignment")
            .unwrap();
        assert!(matches!(&domains.declarations[assignment.id.0].domain,
            Domain::Array { element, .. } if matches!(element.as_ref(),
                Domain::Named { declaration, domain } if *declaration == leads && domain.as_ref() == &Domain::Unknown)));
        let result = analyze_model(&context, &selected());
        assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
        assert!(result.findings.is_empty(), "{name}: {:?}", result.findings);
        if completed {
            assert_eq!(
                result.rules[0].outcome,
                RuleOutcome::Completed,
                "{name}: {:?}",
                result.limitations
            );
            assert!(
                result.limitations.is_empty(),
                "{name}: {:?}",
                result.limitations
            );
        } else {
            assert!(matches!(
                result.rules[0].outcome,
                RuleOutcome::Limited { .. }
            ));
            assert_eq!(
                result.limitations.len(),
                1,
                "{name}: {:?}",
                result.limitations
            );
            assert_eq!(result.limitations[0].location, assignment.location);
            assert!(
                result.limitations[0]
                    .message
                    .contains("declared domain is unknown")
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn full_index_range_membership_under_let_and_conjunction_supplies_bounds() {
    let source = concat!(
        "enum Objects = {first, second};\n",
        "array[Objects] of int: object_bound;\n",
        "array[Objects] of var int: object_size;\n",
        "constraint forall(object in Objects)(let { int: lower = 0; } in\n",
        "  object_size[object] in 0..object_bound[object] /\\ object_size[object] >= lower);\n",
        "solve satisfy;\n",
    );
    let (directory, context) = model("element-ranges", source);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result.rules[0].outcome,
        RuleOutcome::Completed,
        "{:?}",
        result.limitations
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn partial_and_conditional_ranges_keep_domain_advice() {
    for (name, constraint) in [
        (
            "filter",
            "forall(object in Objects where object != first)(size[object] in 0..10)",
        ),
        ("subset", "forall(object in {first})(size[object] in 0..10)"),
        ("single", "size[first] in 0..10"),
        (
            "implication",
            "forall(object in Objects)(gate -> size[object] in 0..10)",
        ),
        (
            "branch",
            "forall(object in Objects)(if gate then size[object] in 0..10 else true endif)",
        ),
    ] {
        let source = format!(
            "enum Objects = {{first, second}}; var bool: gate;\n\
             array[Objects] of var int: size; constraint {constraint}; solve satisfy;\n"
        );
        let (directory, context) = model(name, &source);
        let result = analyze_model(&context, &selected());
        assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
        assert_eq!(
            result.rules[0].outcome,
            RuleOutcome::Completed,
            "{name}: {:?}",
            result.limitations
        );
        assert!(
            result.limitations.is_empty(),
            "{name}: {:?}",
            result.limitations
        );
        let names: Vec<_> = result
            .findings
            .iter()
            .map(|f| &source[f.location.range.clone()])
            .collect();
        assert_eq!(names, ["size"], "{name}");
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn unsupported_range_source_limits_only_its_array() {
    let source = concat!(
        "enum Objects = {first, second};\n",
        "array[Objects] of record(int: cap): object_bound;\n",
        "array[Objects] of var int: size; var int: unrelated;\n",
        "constraint forall(object in Objects)(let { int: upper = object_bound[object].cap; } in size[object] in 0..upper);\n",
        "solve satisfy;\n",
    );
    let (directory, context) = model("limited-range-source", source);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{:?}",
        result
    );
    assert_eq!(result.limitations.len(), 1, "{:?}", result.limitations);
    assert_eq!(
        &source[result.limitations[0].location.range.clone()],
        "upper"
    );
    let names: Vec<_> = result
        .findings
        .iter()
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(names, ["unrelated"]);
    std::fs::remove_dir_all(directory).unwrap();
}
