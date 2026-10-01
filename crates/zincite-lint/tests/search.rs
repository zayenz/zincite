use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, DefinitionSafety, LintOptions, ModelContext, ModelOptions, ModelRootState,
    RuleOutcome, SearchCoverage, SearchFacts, analyze_model, load_model, resolve_bindings,
    resolve_callables, resolve_definitions, resolve_domains, resolve_instantiations,
    resolve_search_coverage,
};
const CORE: &str = concat!(
    "function var bool: '='(any $T: left,any $T: right); function bool: '='($T: left,$T: right);\n",
    "function var int: '+'(var int: left,var int: right); function set of int: '..'(int: left,int: right);\n",
    "function var bool: forall(array[int] of var opt bool: body);\n",
    "function var bool: '/\\'(var bool: left,var bool: right);\n",
    "annotation input_order; annotation indomain_min; annotation complete;\n",
    "annotation seq_search(array[int] of ann: s);\n",
    "annotation int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore);\n",
    "function ann: int_search(array[$X] of var $$E: x,ann: select,ann: choice,ann: explore);\n",
    "annotation bool_search(array[int] of var bool: x,ann: select,ann: choice,ann: explore);\n",
    "annotation float_search(array[int] of var float: x,float: prec,ann: select,ann: choice,ann: explore);\n",
    "annotation set_search(array[int] of var set of int: x,ann: select,ann: choice,ann: explore);\n",
    "function array[int] of any $V: array1d(array[$U] of any $V: x);\n"
);
fn model(name: &str, source: &str, included: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!("zincite-search-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    std::fs::write(dir.join("included.mzn"), included).unwrap();
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
fn facts(context: &ModelContext) -> (BindingFacts, SearchFacts) {
    let bindings = resolve_bindings(context);
    let calls = resolve_callables(context, &bindings);
    let inst = resolve_instantiations(context, &bindings, &calls);
    let domains = resolve_domains(context, &bindings);
    let definitions = resolve_definitions(context, &bindings, &calls, &inst, &domains);
    let search = resolve_search_coverage(context, &bindings, &calls, &inst, &domains, &definitions);
    (bindings, search)
}
fn coverage(bindings: &BindingFacts, search: &SearchFacts, name: &str) -> SearchCoverage {
    let id = bindings
        .declarations
        .iter()
        .find(|d| d.name == name && d.top_level)
        .unwrap()
        .id;
    search.declarations[id.0].coverage
}
fn selected() -> LintOptions {
    LintOptions::from_selection("search-coverage").unwrap()
}
#[test]
fn typed_searches_aliases_and_direct_closure_preserve_whole_array_and_cycle_boundaries() {
    let source = concat!(
        "\u{feff}% é\r\ninclude \"included.mzn\"; int: parameter_value; var 0..9: seed; var int: constant_value=parameter_value;\n",
        "array[1..2,1..2] of var 0..9: grid; var bool: flag; var 0.0..9.0: amount; var set of 1..3: chosen;\n",
        "var int: absent_dependency; var int: both=seed+absent_dependency;\n",
        "var int: cycle_a; var int: cycle_b; constraint cycle_a=cycle_b;\n",
        "var int: anchored_a; var int: anchored_b; constraint anchored_a=anchored_b;\n",
        "set of int: S; array[S] of var int: input; array[S] of var int: whole; constraint forall(i in S)(whole[i]=input[i]);\n",
        "array[1..2] of var int: partial; constraint partial[1]=seed;\n",
        "set of int: T; array[S] of var int: mismatched; constraint forall(i in T)(mismatched[i]=seed);\n",
        "array[S] of var int: filtered; constraint forall(i in S where true)(filtered[i]=seed);\n",
        "ann: first_stage=int_search(array1d(grid),input_order,indomain_min,complete);\n",
        "function ann: custom_search(array[int] of var int: xs)=int_search(xs,input_order,indomain_min,complete);\n",
        "ann: nested=seq_search([first_stage,seq_search([bool_search([flag],input_order,indomain_min,complete)])]);\n",
        "solve :: seq_search([nested,custom_search([seed,anchored_a,partial[1]]),int_search(input,input_order,indomain_min,complete),float_search([amount],0.001,input_order,indomain_min,complete),set_search([chosen],input_order,indomain_min,complete)]) satisfy;\n"
    );
    let included = "var int: derived; var int: tail; constraint derived=seed+1 /\\ tail=derived+1;\n% zincite-lint: ignore search-coverage\nvar int: suppressed;";
    let (dir, context) = model("closure", source, included);
    let (bindings, search) = facts(&context);
    assert_eq!(search.root_state, ModelRootState::Complete);
    assert_eq!(search.limitations.len(), 1, "{:?}", search.limitations);
    for name in [
        "seed",
        "constant_value",
        "derived",
        "tail",
        "anchored_a",
        "anchored_b",
        "flag",
        "amount",
        "chosen",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Scalar,
            "{name}"
        );
    }
    for name in ["grid", "input", "whole"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::WholeArray,
            "{name}"
        );
    }
    for name in [
        "absent_dependency",
        "both",
        "cycle_a",
        "cycle_b",
        "suppressed",
        "mismatched",
        "filtered",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "partial"),
        SearchCoverage::PartialArray
    );
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let id = bindings
        .declarations
        .iter()
        .find(|d| d.name == "whole")
        .unwrap()
        .id;
    assert!(
        definitions
            .definitions
            .iter()
            .filter(|d| d.target == id)
            .all(|d| matches!(d.safety, DefinitionSafety::Unsupported(_)))
    );
    assert!(
        !definitions
            .bounded_or_defined_targets(&bindings, &domains)
            .contains(&id)
    );
    let result = analyze_model(&context, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let mut names: Vec<_> = result
        .findings
        .iter()
        .map(|f| {
            let bytes = std::fs::read(&f.location.path).unwrap();
            String::from_utf8(bytes[f.location.range.clone()].to_vec()).unwrap()
        })
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "absent_dependency",
            "both",
            "cycle_a",
            "cycle_b",
            "filtered",
            "mismatched",
            "partial"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unknown_annotations_computed_values_and_fragments_remain_explicit() {
    let cases = [
        (
            "none",
            "var 0..9: value; solve satisfy;",
            SearchCoverage::Uncovered,
            1,
        ),
        (
            "computed",
            "var 0..9: value; solve :: int_search([value+1],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Uncovered,
            1,
        ),
        (
            "partial",
            "array[1..2] of var int: value; solve :: int_search([value[1]],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::PartialArray,
            1,
        ),
        (
            "partial-formal",
            "array[1..2] of var int: value; function ann: first_only(array[int] of var int: xs)=int_search([xs[1]],input_order,indomain_min,complete); solve :: first_only(value) satisfy;",
            SearchCoverage::PartialArray,
            1,
        ),
        (
            "cyclic",
            "var int: value; ann: a=b; ann: b=a; solve :: a satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "opaque",
            "var int: value; annotation strategy; solve :: strategy satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "optional",
            "var opt int: value; solve :: int_search([value],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "user-view",
            "array[1..2] of var int: value; function array[int] of var int: array1d(array[int] of var int: x)=x; solve :: int_search(array1d(value),input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "user-search",
            "var int: value; function ann: int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore)=complete; solve :: int_search([value],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "user-conjunction",
            "var int: seed; var int: value; function var bool: '/\\'(var bool: left,var bool: right)=true; constraint (value=seed) /\\ true; solve :: int_search([seed],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "local-decision",
            "var int: value=let { var int: scoped; } in scoped; solve satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
        (
            "callable-output",
            "var int: input; function var int: identity(var int: x)=x; var int: value=identity(input); var int: dependent=value; solve :: int_search([input],input_order,indomain_min,complete) satisfy;",
            SearchCoverage::Unknown,
            0,
        ),
    ];
    for (name, source, expected, count) in cases {
        let (dir, context) = model(name, source, "");
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "value"),
            expected,
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            result.findings.len(),
            count,
            "{name}: {:?}",
            result.findings
        );
        assert!(matches!(
            result.rules[0].outcome,
            RuleOutcome::Limited { .. }
        ));
        if expected == SearchCoverage::Unknown {
            assert!(result.limitations.len() > 1, "{name}");
        }
        if name == "callable-output" {
            assert_eq!(
                coverage(&bindings, &search, "dependent"),
                SearchCoverage::Unknown
            );
        }
        if name == "computed" {
            assert!(search.searched.iter().all(|s| s.declaration.is_none()));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    let (dir, context) = model("fragment", "var int: exported;", "");
    let (_, search) = facts(&context);
    assert_eq!(search.root_state, ModelRootState::Fragment);
    let result = analyze_model(&context, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Inapplicable { .. }
    ));
    assert!(result.findings.is_empty() && result.limitations.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
