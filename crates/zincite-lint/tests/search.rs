use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, DefinitionCoverage, DefinitionSafety, LintOptions, ModelContext, ModelOptions,
    ModelRootState, RuleOutcome, SearchCoverage, SearchFacts, analyze_model, load_model,
    resolve_bindings, resolve_callable_definitions, resolve_callables, resolve_definitions,
    resolve_domains, resolve_instantiations, resolve_search_coverage,
};
const CORE: &str = concat!(
    "function var bool: '='(any $T: left,any $T: right); function bool: '='($T: left,$T: right);\n",
    "function var int: '+'(var int: left,var int: right); function set of int: '..'(int: left,int: right);\n",
    "function var bool: forall(array[int] of var opt bool: body);\n",
    "function var bool: '->'(var bool: left,var bool: right);\n",
    "function var int: sum(array[int] of var int: body); function set of int: index_set(array[int] of any $V: xs);\n",
    "function var int: enum2int(var $$E: x); function array[int] of var int: enum2int(array[int] of var $$E: x);\n",
    "function array[int] of any $V: index2int(array[$$E] of any $V: x);\n",
    "function var bool: '/\\'(var bool: left,var bool: right);\n",
    "function var bool: 'in'(var int: value,set of int: choices);\n",
    "function array[int] of var int: '++'(array[int] of var int: left,array[int] of var int: right);\n",
    "annotation input_order; annotation indomain_min; annotation complete;\n",
    "annotation seq_search(array[int] of ann: s);\n",
    "annotation int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore);\n",
    "function ann: int_search(array[$X] of var $$E: x,ann: select,ann: choice,ann: explore);\n",
    "annotation bool_search(array[int] of var bool: x,ann: select,ann: choice,ann: explore);\n",
    "annotation float_search(array[int] of var float: x,float: prec,ann: select,ann: choice,ann: explore);\n",
    "annotation set_search(array[int] of var set of int: x,ann: select,ann: choice,ann: explore);\n",
    "function array[int] of any $V: array1d(array[$U] of any $V: x);\n",
    "function array[int] of any $V: array1d(set of int: S,array[$U] of any $V: x);\n"
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
        "var 0..9: concatenated_seed; array[1..2,1..2] of var int: traversed_grid; array[1..2] of var int: traversed_filtered;\n",
        "set of int: T; array[S] of var int: mismatched; constraint forall(i in T)(mismatched[i]=seed);\n",
        "array[S] of var int: filtered; constraint forall(i in S where true)(filtered[i]=seed);\n",
        "ann: first_stage=int_search(array1d(0..3,grid),input_order,indomain_min,complete);\n",
        "function ann: custom_search(array[int] of var int: xs)=int_search(xs,input_order,indomain_min,complete);\n",
        "ann: nested=seq_search([first_stage,seq_search([bool_search([flag],input_order,indomain_min,complete)])]);\n",
        "solve :: seq_search([nested,custom_search([seed,anchored_a,partial[1]]),int_search(input,input_order,indomain_min,complete),float_search([amount],0.001,input_order,indomain_min,complete),set_search([chosen],input_order,indomain_min,complete),int_search([concatenated_seed]++[traversed_grid[i,j]|i in 1..2,j in 1..2],input_order,indomain_min,complete),int_search([traversed_filtered[i]|i in 1..2 where true],input_order,indomain_min,complete)]) satisfy;\n"
    );
    let included = "var int: derived; var int: tail; constraint derived=seed+1 /\\ tail=derived+1;\n% zincite-lint: ignore search-coverage\nvar int: suppressed;";
    let (dir, context) = model("closure", source, included);
    let (bindings, search) = facts(&context);
    assert_eq!(search.root_state, ModelRootState::Complete);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    for name in [
        "seed",
        "concatenated_seed",
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
    for name in ["grid", "input", "whole", "traversed_grid"] {
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
        "traversed_filtered",
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
        !definitions
            .bounded_or_defined_targets(&bindings, &domains)
            .contains(&id)
    );
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
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
            "partial",
            "traversed_filtered"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    // Searching a pure reshape retains its literal source values, not unrelated values.
    for (name, rows, expected) in [
        (
            "searched-reshape-alias",
            "set of int: Rows;",
            SearchCoverage::Scalar,
        ),
        (
            "searched-reshape-unsafe",
            "set of int: Rows=1..(1 div 0);",
            SearchCoverage::Unknown,
        ),
    ] {
        let source = format!(
            "{rows} set of int: Columns; var 0..9: first_cell; var 0..9: second_cell; var 0..9: unrelated; array[1..2] of var int: cells=[first_cell,second_cell]; array[int] of var int: alias=cells; array[int,int] of var int: view=array2d(Rows,Columns,alias); solve :: int_search(array1d(view),input_order,indomain_min,complete) satisfy;"
        );
        let (case_dir, _) = model(name, &source, "");
        std::fs::write(
            case_dir.join("library/std/stdlib.mzn"),
            format!("{CORE}\nfunction array[int,int] of any $V: array2d(set of int: rows,set of int: columns,array[int] of any $V: values); function int: 'div'(int: left,int: right);\n"),
        ).unwrap();
        let context = load_model(
            case_dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(case_dir.join("library")),
                ..Default::default()
            },
        );
        let (bindings, search) = facts(&context);
        for name in ["first_cell", "second_cell"] {
            assert_eq!(
                coverage(&bindings, &search, name),
                expected,
                "{name}: {:?}",
                search.limitations
            );
        }
        let result = analyze_model(&context, &selected());
        if expected == SearchCoverage::Scalar {
            assert_eq!(
                coverage(&bindings, &search, "unrelated"),
                SearchCoverage::Uncovered
            );
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{:?}",
                result.limitations
            );
            assert_eq!(result.findings.len(), 1);
            let calls = resolve_callables(&context, &bindings);
            let inst = resolve_instantiations(&context, &bindings, &calls);
            let domains = resolve_domains(&context, &bindings);
            let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
            let view = bindings
                .declarations
                .iter()
                .find(|d| d.name == "view" && d.top_level)
                .unwrap()
                .id;
            let definition = definitions
                .definitions
                .iter()
                .find(|d| d.target == view)
                .unwrap();
            assert!(matches!(definition.safety, DefinitionSafety::Unknown(_)));
        } else {
            assert!(matches!(
                result.rules[0].outcome,
                RuleOutcome::Limited { .. }
            ));
        }
        std::fs::remove_dir_all(case_dir).unwrap();
    }
    // An inspected reshape keeps unproved cardinality and search coverage unknown.
    for (name, declarations, inspected) in [
        (
            "reshape-symbolic",
            "set of int: Rows; set of int: Columns; array[1..4] of var 0..9: cells; array[int,int] of var int: value=array2d(Rows,Columns,cells);",
            true,
        ),
        (
            "reshape-unsafe-axis",
            "set of int: Rows=1..(1 div 0); set of int: Columns; array[1..4] of var 0..9: cells; array[int,int] of var int: value=array2d(Rows,Columns,cells);",
            false,
        ),
        (
            "reshape-cyclic-axis",
            "set of int: Rows=Other; set of int: Other=Rows; set of int: Columns; array[1..4] of var 0..9: cells; array[int,int] of var int: value=array2d(Rows,Columns,cells);",
            false,
        ),
        (
            "reshape-annotated",
            "set of int: Rows; set of int: Columns; array[1..4] of var 0..9: cells; annotation tag; array[int,int] of var int: value=(array2d(Rows,Columns,cells))::tag;",
            false,
        ),
        (
            "reshape-user",
            "function array[int,int] of var int: array2d(set of int: r,set of int: c,array[int] of var int: a)=[|a[1],a[2]|]; set of int: Rows; set of int: Columns; array[1..4] of var 0..9: cells; array[int,int] of var int: value=array2d(Rows,Columns,cells);",
            false,
        ),
        (
            "reshape-optional",
            "set of int: Rows; set of int: Columns; array[1..4] of var opt 0..9: cells; array[int,int] of var opt int: value=array2d(Rows,Columns,cells);",
            false,
        ),
    ] {
        let source = format!(
            "{declarations} solve :: int_search(cells,input_order,indomain_min,complete) satisfy;"
        );
        let (case_dir, _) = model(name, &source, "");
        std::fs::write(
            case_dir.join("library/std/stdlib.mzn"),
            format!("{CORE}\nfunction array[int,int] of any $V: array2d(set of int: rows,set of int: columns,array[int] of any $V: values); function int: 'div'(int: left,int: right);\n"),
        ).unwrap();
        let context = load_model(
            case_dir.join("root.mzn"),
            &ModelOptions {
                include_dirs: vec![],
                stdlib_dir: Some(case_dir.join("library")),
            },
        );
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "value"),
            SearchCoverage::Unknown,
            "{name}"
        );
        let result = analyze_model(&context, &selected());
        assert!(result.findings.is_empty(), "{name}: {:?}", result.findings);
        if inspected {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{name}: {:?}",
                result.limitations
            );
            assert!(result.limitations.is_empty(), "{name}");
        } else {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}"
            );
            assert!(!result.limitations.is_empty(), "{name}");
        }
        std::fs::remove_dir_all(case_dir).unwrap();
    }
    // A checked symbolic traversal can be inspected without seeding its whole source.
    let filtered_source = r#"include "globals.mzn";

% Data and selected values retain a shared named integer axis.
set of int: Pool;
set of Pool: Eligible;
set of Pool: Removed;
array[Pool] of set of int: first_tags;
array[Pool] of set of int: second_tags;
array[Pool] of var bool: chosen;

% Filtered searches do not promise whole-Pool coverage.
solve :: seq_search([
    bool_search([chosen[m] | m in Eligible diff Removed
        where card(first_tags[m]) + card(second_tags[m]) > 2],
        input_order, indomain_max, complete),
    bool_search([chosen[m] | m in Eligible diff Removed
        where card(first_tags[m]) + card(second_tags[m]) = 2],
        input_order, indomain_max, complete),
    bool_search([chosen[m] | m in Eligible diff Removed
        where card(first_tags[m]) + card(second_tags[m]) = 1],
        input_order, indomain_max, complete)
]) satisfy;
"#;
    let (filtered_dir, _) = model("filtered-parameter-search", "solve satisfy;", "");
    std::fs::write(filtered_dir.join("root.mzn"), filtered_source).unwrap();
    std::fs::write(filtered_dir.join("library/std/globals.mzn"), "").unwrap();
    std::fs::write(
        filtered_dir.join("library/std/stdlib.mzn"),
        format!("{CORE}function set of int: 'diff'(set of int: left,set of int: right); function int: card(set of $T: values); function int: '+'(int: left,int: right); function bool: '>'(int: left,int: right); annotation indomain_max;\n"),
    ).unwrap();
    let filtered = load_model(
        filtered_dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(filtered_dir.join("library")),
            ..Default::default()
        },
    );
    assert!(filtered.errors.is_empty(), "{:?}", filtered.errors);
    let bindings = resolve_bindings(&filtered);
    let calls = resolve_callables(&filtered, &bindings);
    assert!(calls.calls.iter().any(|call| call.name == "diff"));
    assert!(calls.calls.iter().any(|call| call.name == "card"));
    for call in calls
        .calls
        .iter()
        .filter(|call| filtered.files[call.file].path == filtered_dir.join("root.mzn"))
    {
        assert!(
            matches!(&call.outcome, zincite_lint::CallOutcome::Resolved { .. }),
            "{:?}",
            call
        );
    }
    let result = analyze_model(&filtered, &selected());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{:?}",
        result.limitations
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    let (bindings, search) = facts(&filtered);
    assert_eq!(
        coverage(&bindings, &search, "chosen"),
        SearchCoverage::Uncovered
    );
    assert!(
        !search
            .searched
            .iter()
            .any(|value| value.coverage == SearchCoverage::WholeArray)
    );

    // An independent closed failure must survive the neighboring symbolic count.
    let overflow = filtered_source.replace(
        "card(first_tags[m]) + card(second_tags[m]) > 2",
        "card(first_tags[m]) + (9223372036854775807 + 1) > 2",
    );
    std::fs::write(filtered_dir.join("root.mzn"), overflow).unwrap();
    let negative = load_model(
        filtered_dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(filtered_dir.join("library")),
            ..Default::default()
        },
    );
    assert!(negative.errors.is_empty(), "{:?}", negative.errors);
    let result = analyze_model(&negative, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(!result.limitations.is_empty());
    let (_, search) = facts(&negative);
    assert!(!search.limitations.is_empty());
    assert!(
        !search
            .searched
            .iter()
            .any(|value| value.coverage == SearchCoverage::WholeArray)
    );
    std::fs::remove_dir_all(filtered_dir).unwrap();
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
            "assigned-annotation",
            "var 0..9: value; var 0..9: unrelated; annotation strategy; strategy=seq_search([int_search([value],input_order,indomain_min,complete)]); solve :: strategy satisfy;",
            SearchCoverage::Scalar,
            1,
        ),
        (
            "multiple-annotation-assignments",
            "var 0..9: value; annotation strategy; strategy=int_search([value],input_order,indomain_min,complete); strategy=int_search([value],input_order,indomain_min,complete); solve :: strategy satisfy;",
            SearchCoverage::Unknown,
            0,
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
            "opaque-reindex",
            "array[1..2] of var int: value; function set of int: opaque_indices()=0..1; function ann: reindexed(set of int: S,array[int] of var int: xs)=int_search(array1d(S,xs),input_order,indomain_min,complete); solve :: reindexed(opaque_indices(),value) satisfy;",
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
        if name == "assigned-annotation" {
            assert_eq!(
                coverage(&bindings, &search, "unrelated"),
                SearchCoverage::Uncovered
            );
        }
        let result = analyze_model(&context, &selected());
        assert_eq!(
            result.findings.len(),
            count,
            "{name}: {:?}",
            result.findings
        );
        if expected == SearchCoverage::Unknown {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}"
            );
            assert!(!result.limitations.is_empty(), "{name}");
        } else {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{name}: {:?}",
                result.limitations
            );
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
    let source = concat!(
        "int: task_count; set of int: Tasks=0..task_count; array[Tasks] of var int: starts; array[Tasks] of var int: iterations;\n",
        "var int: span=max([starts[i]-iterations[i]*starts[task_count]|i in Tasks where i>0 /\\ i<task_count])-min([starts[i]-iterations[i]*starts[task_count]|i in Tasks where i>0 /\\ i<task_count])+1;\n",
        "var int: finish=starts[task_count]+span; solve :: seq_search([int_search(starts,input_order,indomain_min,complete),int_search(iterations,input_order,indomain_min,complete)]) satisfy;\n"
    );
    let (dir, _) = model("symbolic-partial", source, "");
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\nfunction var int: max(array[int] of var int: values); function var int: min(array[int] of var int: values); function var int: '-'(var int: left,var int: right); function var int: '*'(var int: left,var int: right); function bool: '<'(int: left,int: right); function bool: '>'(int: left,int: right); function bool: '/\\'(bool: left,bool: right);\n")).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&context);
    for name in ["span", "finish"] {
        assert_eq!(coverage(&bindings, &search, name), SearchCoverage::Unknown);
    }
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    let result = analyze_model(&context, &selected());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{:?}",
        result
    );
    assert!(result.findings.is_empty());
    std::fs::write(dir.join("root.mzn"), format!("{source}\nfunction var int: opaque(var int: x)=x; var int: opaque_span=max([starts[task_count],opaque(starts[0])]);\n")).unwrap();
    let opaque = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let result = analyze_model(&opaque, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "int: K; int: N; array[1..K,1..N] of var bool: value; array[1..N] of set of int: labels;\n",
        "array[1..K,1..N] of var bool: relation; var bool: existential; var bool: universal;\n",
        "constraint forall(d in 1..K,t in 1..N)(relation[d,t]<->sum(i in 1..N)(bool2int(not(i in labels[t]))*bool2int(value[d,i]))<=0);\n",
        "constraint forall(t in 1..N)(existential<->exists(d in 1..K)(relation[d,t]));\n",
        "constraint universal<->forall(d in 1..K,t in 1..N)(value[d,t]);\n",
        "solve :: bool_search([value[d,i]|d in 1..K,i in 1..N],input_order,indomain_min,complete) satisfy;\n"
    );
    let (dir, _) = model("quantified-reads", source, "");
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\nfunction var bool: exists(array[int] of var bool: body); function var bool: '<->'(var bool: left,var bool: right); function var bool: '<='(var int: left,var int: right); function var int: bool2int(var bool: value); function var bool: 'not'(var bool: value); function bool: 'in'(int: left,set of int: right); function var int: '*'(var int: left,var int: right);\n")).unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    assert_eq!(
        coverage(&bindings, &search, "value"),
        SearchCoverage::WholeArray
    );
    for name in ["relation", "existential", "universal"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Uncovered
        );
    }
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    for negative in [
        source.replace("sum(i in 1..N)", "sum(i in 0..N)"),
        format!(
            "{source}\nfunction var bool: opaque(var bool: b)=b; constraint existential<->exists(i in 1..0)(opaque(true));\n"
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), negative).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let result = analyze_model(&context, &selected());
        assert!(matches!(
            result.rules[0].outcome,
            RuleOutcome::Limited { .. }
        ));
    }
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "int: N; array[1..N] of var 1..N: left; array[1..N] of var 1..N: right; var int: result;\n",
        "constraint forall(C in 1..N)(exists(Y in left++right)(Y=C));\n",
        "constraint exists(Y in left++right)(result=Y);\n",
        "solve :: seq_search([int_search(left,input_order,indomain_min,complete),int_search(right,input_order,indomain_min,complete)]) satisfy;\n"
    );
    let (dir, _) = model("decision-array-exists", source, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var bool: exists(array[int] of var bool: body);\n"),
    )
    .unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&context);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    assert_eq!(
        coverage(&bindings, &search, "left"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "right"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "result"),
        SearchCoverage::Uncovered
    );
    let result = analyze_model(&context, &selected());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{:?}",
        result
    );
    for rule in ["expensive-comprehension", "vacuous-constraint"] {
        let result = analyze_model(&context, &LintOptions::from_selection(rule).unwrap());
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Completed),
            "{:?}",
            result
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "array[1..2] of int: values=[2,4]; var int: result; constraint result=0;\n",
        "constraint forall(i in 1..2,j in index_set([values[i]|k in 1..1]) where (j in {k|k in 1..1}) /\\ forall(k in 1..1)(values[i]>=k))(true); solve satisfy;\n"
    );
    let (dir, _) = model("generator-scope", source, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction bool: forall(array[int] of bool: body); function bool: 'in'(int: left,set of int: right); function bool: '>='(int: left,int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    assert_eq!(
        coverage(&bindings, &search, "result"),
        SearchCoverage::Scalar
    );
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    for negative in [
        format!(
            "function int: opaque(int: value)=value; {}",
            source.replace("[values[i]|", "[opaque(values[i])|")
        ),
        format!(
            "function bool: opaque(bool: value)=value; {}",
            source.replace(
                "forall(k in 1..1)(values[i]>=k)",
                "forall(k in 1..0)(opaque(true))"
            )
        ),
    ] {
        let negative = negative
            .replace("constraint forall", "predicate checked(var int: result_value)=if forall")
            .replace("(true); solve satisfy;", "(true) then result_value=0 else result_value=0 endif; var int: guarded_result; constraint checked(guarded_result); solve satisfy;");
        std::fs::write(dir.join("root.mzn"), negative).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "guarded_result"),
            SearchCoverage::Unknown
        );
        let result = analyze_model(&context, &selected());
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            "{:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let source = "int: n; array[0..n] of var int: values; constraint forall(i in 2..n)(values[i-1]-values[i]<=0); solve satisfy;\n";
    let (dir, _) = model("relational-index", source, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: '-'(int: left,int: right); function var int: '-'(var int: left,var int: right); function var bool: '<='(var int: left,var int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    assert_eq!(
        coverage(&bindings, &search, "values"),
        SearchCoverage::Uncovered
    );
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    std::fs::write(
        dir.join("root.mzn"),
        format!("{source}\nvar int: raw_result; constraint raw_result=values[n-1];"),
    )
    .unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "raw_result"),
        SearchCoverage::Unknown
    );
    std::fs::write(
        dir.join("root.mzn"),
        format!(
            "function int: opaque_index(int: i)=i; {}",
            source.replace("values[i-1]", "values[opaque_index(i)]")
        ),
    )
    .unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let result = analyze_model(&context, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "array[2..4] of var bool: bits; var bool: value;\n",
        "constraint value=xorall(i in index_set(bits))(bits[i]);\n",
        "solve :: bool_search(bits,input_order,indomain_min,complete) satisfy;\n"
    );
    let (dir, _) = model("quantified-parity", source, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var bool: xorall(array[$T] of var bool: body);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "value"),
        SearchCoverage::Scalar
    );
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    let result = analyze_model(&context, &selected());
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    let conditional = source
        .replace("var bool: value;", "var bool: value; var bool: enabled;")
        .replace("constraint value=", "constraint enabled -> value=")
        .replace(
            "bool_search(bits,input_order,indomain_min,complete)",
            "seq_search([bool_search(bits,input_order,indomain_min,complete),bool_search([enabled],input_order,indomain_min,complete)])"
        );
    std::fs::write(dir.join("root.mzn"), conditional).unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "value"),
        SearchCoverage::Uncovered
    );
    std::fs::remove_dir_all(dir).unwrap();
    let shifted = concat!(
        "int: upper; array[1..upper] of var int: input_values; array[1..upper,1..upper] of var int: value;\n",
        "constraint forall(i,j in 1..upper)(if i<j then value[i,j]=input_values[j]-input_values[j-i] else value[i,j]=0 endif);\n",
        "solve :: int_search(input_values,input_order,indomain_min,complete) satisfy;\n"
    );
    let nonstrict_shifted = shifted.replace("if i<j", "if i<=j");
    let wrong_branch_shifted = shifted.replace(
        "then value[i,j]=input_values[j]-input_values[j-i] else value[i,j]=0",
        "then value[i,j]=0 else value[i,j]=input_values[j]-input_values[j-i]",
    );
    let different_upper_shifted = shifted
        .replace("int: upper;", "int: upper; int: other_upper;")
        .replace(
            "array[1..upper] of var int: input_values",
            "array[1..other_upper] of var int: input_values",
        );
    let user_shifted = format!("function int: '-'(int: left,int: right)=left+upper; {shifted}");
    let (dir, _) = model("shifted-membership", shifted, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: '+'(int: left,int: right); function int: '-'(int: left,int: right); function var int: '-'(var int: left,var int: right); function bool: '<'(int: left,int: right); function bool: '<='(int: left,int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (name, source, expected) in [
        ("strict-true-branch", shifted, SearchCoverage::WholeArray),
        (
            "nonstrict",
            nonstrict_shifted.as_str(),
            SearchCoverage::Unknown,
        ),
        (
            "else-branch",
            wrong_branch_shifted.as_str(),
            SearchCoverage::Unknown,
        ),
        (
            "different-upper",
            different_upper_shifted.as_str(),
            SearchCoverage::Unknown,
        ),
        (
            "user-subtract",
            user_shifted.as_str(),
            SearchCoverage::Unknown,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "value"),
            expected,
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        if expected == SearchCoverage::WholeArray {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{name}: {:?}",
                result.limitations
            );
        } else {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}: {:?}",
                result.limitations
            );
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    let forward = concat!(
        "predicate update_int(var int: result_value,var bool: flag,var int: old_value,var int: new_value)=result_value=[old_value,new_value][1+bool2int(flag)];\n",
        "predicate update_bool(var bool: result_value,var bool: flag,var bool: old_value,var bool: new_value)=result_value=[old_value,new_value][bool2int(flag)+1];\n",
        "var int: old_int; var int: new_int; var bool: old_bool; var bool: new_bool; var bool: flag; var int: result_int; var bool: result_bool;\n",
        "constraint update_int(result_int,flag,old_int,new_int); constraint update_bool(result_bool,flag,old_bool,new_bool);\n",
        "solve :: seq_search([int_search([old_int,new_int],input_order,indomain_min,complete),bool_search([old_bool,new_bool,flag],input_order,indomain_min,complete)]) satisfy;\n"
    );
    let unsearched_forward = forward.replace("[old_bool,new_bool,flag]", "[old_bool,new_bool]");
    let unsafe_forward = forward.replace("[1+bool2int(flag)]", "[2+bool2int(flag)]");
    let (dir, _) = model("forward-choice", forward, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var int: bool2int(var bool: value);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (name, source, expected_int, expected_bool, limited) in [
        (
            "both orders and scalar kinds",
            forward,
            SearchCoverage::Scalar,
            SearchCoverage::Scalar,
            false,
        ),
        (
            "unsearched selector",
            unsearched_forward.as_str(),
            SearchCoverage::Uncovered,
            SearchCoverage::Uncovered,
            false,
        ),
        (
            "wrong offset",
            unsafe_forward.as_str(),
            SearchCoverage::Unknown,
            SearchCoverage::Scalar,
            true,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "result_int"),
            expected_int,
            "{name}: {:?}",
            search.limitations
        );
        assert_eq!(
            coverage(&bindings, &search, "result_bool"),
            expected_bool,
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{name}: {:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let literal_calls = concat!(
        "predicate update_cells(var int: assigned,var int: index,array[int] of var int: old_cells,array[int] of var int: new_cells,var bool: active)=forall(cell in index_set(old_cells))(new_cells[cell]=[old_cells[cell],assigned][bool2int(active /\\ cell=index)+1]);\n",
        "predicate lookup_cell(var int: index,array[int] of var int: cells,var int: value)=cells[index]=value;\n",
        "predicate self_indexed(var int: value,array[int] of var int: cells)=value=cells[value];\n",
        "var 1..2: selected_index; var 1..6: assigned_value; var bool: active; var int: first_cell; var int: second_cell; var int: selected_value; var int: mapped_value; var 1..2: self_lookup;\n",
        "constraint update_cells(assigned_value,selected_index,[1,2],[first_cell,second_cell],active); constraint lookup_cell(selected_index,[first_cell,second_cell],selected_value); constraint element(selected_index,[5,6],mapped_value); constraint lookup_cell(self_lookup,[1,2],self_lookup); constraint self_indexed(self_lookup,[1,2]);\n",
        "solve :: seq_search([int_search([selected_index,assigned_value],input_order,indomain_min,complete),bool_search([active],input_order,indomain_min,complete)]) satisfy;\n"
    );
    let (dir, _) = model("literal-array-calls", literal_calls, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var int: bool2int(var bool: value); predicate element(var $$E: index,array[$$E] of var $$T: cells,var $$T: value)=value=cells[index];\n"),
    ).unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (name, source, cells, scalar, limited) in [
        (
            "forward cells and scalar lookup",
            literal_calls.to_owned(),
            SearchCoverage::Scalar,
            SearchCoverage::Scalar,
            false,
        ),
        (
            "unsearched selector",
            literal_calls.replace("[selected_index,assigned_value]", "[assigned_value]"),
            SearchCoverage::Uncovered,
            SearchCoverage::Uncovered,
            false,
        ),
        (
            "unproved selector membership",
            literal_calls.replace("var 1..2: selected_index", "var 0..2: selected_index"),
            SearchCoverage::Scalar,
            SearchCoverage::Unknown,
            true,
        ),
        (
            "no cyclic seed",
            literal_calls.replace(
                "selected_index,[1,2],",
                "selected_index,[first_cell,second_cell],",
            ),
            SearchCoverage::Uncovered,
            SearchCoverage::Uncovered,
            false,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        for value in ["first_cell", "second_cell"] {
            assert_eq!(
                coverage(&bindings, &search, value),
                cells,
                "{name}: {:?}",
                search.limitations
            );
        }
        assert_eq!(
            coverage(&bindings, &search, "selected_value"),
            scalar,
            "{name}: {:?}",
            search.limitations
        );
        assert_eq!(
            coverage(&bindings, &search, "mapped_value"),
            if name == "no cyclic seed" {
                SearchCoverage::Scalar
            } else {
                scalar
            },
            "{name}: {:?}",
            search.limitations
        );
        assert_eq!(
            coverage(&bindings, &search, "self_lookup"),
            SearchCoverage::Uncovered,
            "a selector cannot define itself: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{name}: {:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let row_array = concat!(
        "int: bound; array[1..2,1..2] of var 0..bound: board;\n",
        "constraint forall(row in 1..2)(let {array[1..2] of var 0..bound: cells=[board[row,1],board[row,2]];} in cells[1]<=cells[2]);\n",
        "solve :: int_search(array1d(board),input_order,indomain_min,complete) satisfy;\n"
    );
    let local_array = row_array
        .replace("forall(row in 1..2)", "forall(row,column in 1..2)")
        .replace(
            "[board[row,1],board[row,2]]",
            "[board[row,column],board[row,column]]",
        );
    let unknown_array = row_array
        .replace("int: bound;", "int: bound; int: row;")
        .replace("forall(row in 1..2)(let", "(let")
        .replace("board[row,", "board[row+1,");
    let partial_array = unknown_array.replace("board[row+1,2]", "1 div 0");
    let (dir, _) = model("initialized-local-array", &local_array, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: '+'(int: left,int: right); function var bool: '<='(var int: left,var int: right); function var int: 'div'(var int: left,var int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (name, source, expected, inspected, output, limited) in [
        (
            "strict searched initializer",
            local_array.to_owned(),
            SearchCoverage::WholeArray,
            false,
            true,
            false,
        ),
        (
            "unsearched initializer dependencies",
            local_array.replace(
                "solve :: int_search(array1d(board),input_order,indomain_min,complete) satisfy;",
                "solve satisfy;",
            ),
            SearchCoverage::Uncovered,
            false,
            true,
            false,
        ),
        (
            "unproved parameter membership",
            unknown_array,
            SearchCoverage::Unknown,
            true,
            false,
            false,
        ),
        (
            "unsupported child wins over uncertainty",
            partial_array,
            SearchCoverage::Unknown,
            false,
            false,
            true,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        let calls = resolve_callables(&context, &bindings);
        let instantiations = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let callable =
            resolve_callable_definitions(&context, &bindings, &calls, &instantiations, &domains);
        let cells = bindings
            .declarations
            .iter()
            .find(|d| d.name == "cells")
            .unwrap()
            .id;
        assert_eq!(
            search.declarations[cells.0].coverage, expected,
            "{name}: {:?}; callable: {callable:?}",
            search.limitations
        );
        assert_eq!(
            callable.inspected_locals.contains(&cells),
            inspected,
            "{name}: {callable:?}"
        );
        assert_eq!(
            callable
                .definitions
                .iter()
                .any(|d| d.target == cells && d.coverage == DefinitionCoverage::WholeArray),
            output,
            "{name}: {:?}",
            callable.unavailable
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{name}: {:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let local_choice = concat!(
        "bool: enabled=true; int: duration=1; var int: start_a; var int: start_b;\n",
        "predicate unused_choice()=let {var bool: private_before;} in private_before \\/ not private_before;\n",
        "constraint if enabled then let {var bool: before;} in (before->start_a+duration<=start_b) /\\ ((start_b+duration<=start_a)<-not before) /\\ (before \\/ not before) else true endif;\n",
        "solve :: int_search([start_a,start_b],input_order,indomain_min,complete) satisfy;\n"
    );
    let failed_choice = local_choice
        .replace(
            "int: duration=1;",
            "int: duration=1; int: parameter_index=1;",
        )
        .replace("constraint if enabled", "constraint (if enabled")
        .replace(
            "else true endif;",
            "else true endif) /\\ forall(k in {[1,2][parameter_index]} where true)(true);",
        );
    let (dir, _) = model("nondefining-local-choice", local_choice, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var bool: '<-'(var bool: left,var bool: right); function var bool: '\\/'(var bool: left,var bool: right); function var bool: 'not'(var bool: value); function var bool: '<='(var int: left,var int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (source, limited) in [(local_choice, false), (failed_choice.as_str(), true)] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        let before = bindings
            .declarations
            .iter()
            .find(|d| d.name == "before")
            .unwrap()
            .id;
        assert_eq!(
            search.declarations[before.0].coverage,
            SearchCoverage::Unknown
        );
        let calls = resolve_callables(&context, &bindings);
        let instantiations = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let callable =
            resolve_callable_definitions(&context, &bindings, &calls, &instantiations, &domains);
        assert!(callable.definitions.iter().all(|d| d.target != before));
        assert!(callable.outputs.iter().all(|d| d.target != before));
        assert_eq!(callable.inspected_locals.contains(&before), !limited);
        let private = bindings
            .declarations
            .iter()
            .find(|d| d.name == "private_before")
            .unwrap()
            .id;
        assert!(!callable.inspected_locals.contains(&private));
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let literal_extrema = concat!(
        "array[1..2] of var int: positions; var int: value=sum(r in 1..2)(min(d in 1..2)(abs(positions[d]-r)));\n",
        "solve :: int_search(positions,input_order,indomain_min,complete) satisfy;\n"
    );
    let symbolic_extrema = concat!(
        "int: depot_count=2; set of int: Depot=1..depot_count; array[Depot] of var int: positions;\n",
        "var int: value=sum(r in 1..2)(min(d in Depot)(abs(positions[d]-r)));\n",
        "solve :: int_search(positions,input_order,indomain_min,complete) satisfy;\n"
    );
    let partial_extrema = symbolic_extrema
        .replace(
            "int: depot_count=2;",
            "int: depot_count=2; int: denominator=2;",
        )
        .replace(
            "abs(positions[d]-r)",
            "abs(positions[d]-r)+(1 div denominator)",
        );
    let literal_max = literal_extrema.replace("min(d in", "max(d in");
    let symbolic_max = symbolic_extrema.replace("min(d in", "max(d in");
    let partial_max = partial_extrema.replace("min(d in", "max(d in");
    let literal_equality = literal_max
        .replace("var int: value=", "var int: value; constraint (value=")
        .replace(")));\nsolve", "))));\nsolve");
    let symbolic_equality =
        symbolic_max.replace("var int: value=", "var int: value; constraint value=");
    let partial_equality =
        partial_max.replace("var int: value=", "var int: value; constraint value=");
    let shadowed_equality =
        format!("function var bool: '='(var int: left,var int: right)=true;\n{literal_equality}");
    let symbolic_abs = concat!(
        "int: parameter_index=1; array[1..2] of var int: positions; var int: value=abs(positions[parameter_index]);\n",
        "solve :: int_search(positions,input_order,indomain_min,complete) satisfy;\n"
    );
    let selector = concat!(
        "set of int: Rows; set of int: Cols; set of int: Configs;\n",
        "array[Rows,Cols,Configs] of int: widths; array[Rows,Cols] of var Configs: config; array[Rows,Cols] of var int: value;\n",
        "constraint forall(r in Rows,c in Cols)(value[r,c]=widths[r,c,config[r,c]]);\n",
        "solve :: int_search(array1d(config),input_order,indomain_min,complete) satisfy;\n"
    );
    let unsearched_selector = selector.replace(
        "solve :: int_search(array1d(config),input_order,indomain_min,complete) satisfy;",
        "solve satisfy;",
    );
    let other_selector = selector.replace(
        "array[Rows,Cols] of var Configs: config;",
        "set of int: OtherConfigs; array[Rows,Cols] of var OtherConfigs: config;",
    );
    let (dir, _) = model("bounds-and-selector", literal_extrema, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction var int: min(array[int] of var int: body); function var int: max(array[int] of var int: body); function var int: abs(var int: value); function var int: '-'(var int: left,var int: right); function int: 'div'(int: left,int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let mut failures = Vec::new();
    for (name, source, expected, limited) in [
        (
            "literal extrema",
            literal_extrema,
            SearchCoverage::Scalar,
            false,
        ),
        (
            "symbolic extrema",
            symbolic_extrema,
            SearchCoverage::Unknown,
            false,
        ),
        (
            "partial extrema",
            partial_extrema.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
        (
            "literal max extrema",
            literal_max.as_str(),
            SearchCoverage::Scalar,
            false,
        ),
        (
            "symbolic max extrema",
            symbolic_max.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
        (
            "partial max extrema",
            partial_max.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
        (
            "literal equality extrema",
            literal_equality.as_str(),
            SearchCoverage::Scalar,
            false,
        ),
        (
            "symbolic equality extrema",
            symbolic_equality.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
        (
            "partial equality extrema",
            partial_equality.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
        (
            "shadowed equality extrema",
            shadowed_equality.as_str(),
            SearchCoverage::Unknown,
            true,
        ),
        ("symbolic abs", symbolic_abs, SearchCoverage::Unknown, false),
        (
            "searched selector",
            selector,
            SearchCoverage::WholeArray,
            false,
        ),
        (
            "unsearched selector",
            unsearched_selector.as_str(),
            SearchCoverage::Uncovered,
            false,
        ),
        (
            "other selector domain",
            other_selector.as_str(),
            SearchCoverage::Unknown,
            false,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        let result = analyze_model(&context, &selected());
        let actual = coverage(&bindings, &search, "value");
        if name.contains("extrema") || name == "symbolic abs" {
            let unbounded = analyze_model(
                &context,
                &LintOptions::from_selection("unbounded-variable").unwrap(),
            );
            let value = &bindings
                .declarations
                .iter()
                .find(|d| d.name == "value")
                .unwrap()
                .location;
            if matches!(unbounded.rules[0].outcome, RuleOutcome::Limited { .. }) != limited
                || unbounded.findings.iter().any(|f| &f.location == value)
                || (!limited && !unbounded.limitations.is_empty())
                || (limited && !unbounded.limitations.iter().any(|l| &l.location == value))
            {
                failures.push(format!(
                    "{name} unbounded: {:?}, {:?}, {:?}",
                    unbounded.rules[0].outcome, unbounded.findings, unbounded.limitations
                ));
            }
        }
        if actual != expected
            || matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }) != limited
            || (!limited && !result.limitations.is_empty())
        {
            failures.push(format!(
                "{name}: {actual:?}, {:?}, {:?}",
                result.rules[0].outcome, result.limitations
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    std::fs::remove_dir_all(dir).unwrap();
    let assertions = concat!(
        "bool: gate; var 0..2: input_value; var int: proved_result; var int: guarded_result; var int: sibling_result; var int: reflected_result;\n",
        "constraint let { int: checked=assert(true,\"proved\",2); } in proved_result=checked;\n",
        "constraint let { int: checked=assert(gate,\"enabled\",2); } in guarded_result=checked /\\ sibling_result=0;\n",
        "constraint let { int: checked=assert(has_bounds(input_value),\"finite\",if 0 in dom(enum2int(input_value)) then enum2int(lb(input_value))-1 else 0 endif); } in reflected_result=checked;\n",
        "solve :: int_search([input_value],input_order,indomain_min,complete) satisfy;\n",
    );
    let (dir, _) = model("returning-assertions", assertions, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: assert(bool: guard,string: message,int: value); function bool: has_bounds(var int: value); function set of int: dom(var int: value); function int: lb(var int: value); function int: enum2int(int: value); function int: '-'(int: left,int: right); function bool: 'in'(int: value,set of int: domain); function int: 'div'(int: left,int: right);\n"),
    ).unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    let result = analyze_model(&context, &selected());
    assert_eq!(
        coverage(&bindings, &search, "proved_result"),
        SearchCoverage::Scalar,
        "{:?}",
        result.limitations
    );
    for name in ["guarded_result", "sibling_result", "reflected_result"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    std::fs::write(
        dir.join("root.mzn"),
        "bool: gate; var int: partial_result; constraint let { int: checked=assert(gate,\"enabled\",1 div 0); } in partial_result=checked; solve satisfy;",
    ).unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "partial_result"),
        SearchCoverage::Unknown
    );
    assert!(matches!(
        analyze_model(&context, &selected()).rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let defaults = concat!(
        "array[1..2] of opt int: xs; bool: gate; var 0..2: value;\n",
        "var int: strict_result; var int: guarded_result; var int: sibling_result; var int: partial_result;\n",
        "predicate consume(array[int] of var int: values,var int: result_value)=result_value=0;\n",
        "constraint consume([i default to_enum_internal(enum_of(value),2)|i in xs],strict_result);\n",
        "constraint let {int: fallback=assert(gate,\"enabled\",2);} in consume([i default to_enum_internal(enum_of(value),fallback)|i in xs],guarded_result) /\\ sibling_result=0;\n",
        "constraint consume([i default to_enum_internal(enum_of(value),1 div 0)|i in xs],partial_result);\n",
        "solve satisfy;\n",
    );
    let (dir, _) = model("integer-defaults", defaults, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: assert(bool: guard,string: message,int: value); function int: 'div'(int: left,int: right); function set of int: enum_of(var opt int: value); function int: to_enum_internal(set of int: witness,int: value); function var int: 'default'(var opt int: value,var int: fallback);\n"),
    ).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&context);
    let actual: Vec<_> = [
        "strict_result",
        "guarded_result",
        "sibling_result",
        "partial_result",
    ]
    .into_iter()
    .map(|name| coverage(&bindings, &search, name))
    .collect();
    let result = analyze_model(&context, &selected());
    assert_eq!(
        actual,
        vec![
            SearchCoverage::Scalar,
            SearchCoverage::Uncovered,
            SearchCoverage::Uncovered,
            SearchCoverage::Unknown
        ],
        "{:?}",
        result.limitations
    );
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    let partial = bindings
        .declarations
        .iter()
        .find(|d| d.name == "partial_result")
        .unwrap()
        .id;
    assert!(!callable.definitions.iter().any(|d| d.target == partial));
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let partial_start = defaults
        .find("constraint consume([i default to_enum_internal(enum_of(value),1 div 0)")
        .unwrap();
    assert!(
        result
            .limitations
            .iter()
            .all(|l| l.location.range.start >= partial_start),
        "{:?}",
        result.limitations
    );
    std::fs::remove_dir_all(dir).unwrap();
    let named_selector = concat!(
        "set of int: Rows; set of int: Cols; set of int: Ids;\n",
        "array[Rows,Cols] of var Ids: selector; array[Ids] of var int: table; array[Rows,Cols] of var int: value;\n",
        "constraint forall(r in Rows,c in Cols)(value[r,c]=table[selector[r,c]]);\n",
        "solve :: seq_search([int_search(array1d(selector),input_order,indomain_min,complete),int_search(table,input_order,indomain_min,complete)]) satisfy;\n"
    );
    let (dir, _) = model("named-selector", named_selector, "");
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (name, source, expected, limited) in [
        (
            "both dependencies searched",
            named_selector.to_owned(),
            SearchCoverage::WholeArray,
            false,
        ),
        (
            "selector unsearched",
            named_selector.replace(
                "int_search(array1d(selector),input_order,indomain_min,complete),",
                "",
            ),
            SearchCoverage::Uncovered,
            false,
        ),
        (
            "different declaration",
            named_selector
                .replace("set of int: Ids;", "set of int: Ids; set of int: OtherIds;")
                .replace("of var Ids: selector", "of var OtherIds: selector"),
            SearchCoverage::Unknown,
            true,
        ),
        (
            "optional selector",
            named_selector.replace("of var Ids: selector", "of var opt Ids: selector"),
            SearchCoverage::Unknown,
            true,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "value"),
            expected,
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{name}: {:?}",
            result.limitations
        );
        let unbounded = analyze_model(
            &context,
            &LintOptions::from_selection("unbounded-variable").unwrap(),
        );
        assert_eq!(
            matches!(unbounded.rules[0].outcome, RuleOutcome::Limited { .. }),
            name == "optional selector",
            "{name}: {:?}",
            unbounded.limitations
        );
        if name == "different declaration" {
            assert!(unbounded.findings.iter().any(|finding| {
                &context.files[0].parsed.source()[finding.location.range.clone()] == "value"
            }));
        }
    }
    let neighbour = concat!(
        "set of int: Rows; set of int: Cols; set of int: Ids;\n",
        "array[Rows,Cols] of var Ids: selector; array[Ids] of int: table; var int: sibling;\n",
        "constraint forall(r in Rows,c in Cols where c>1)(let { var Ids: a=selector[r,c-1]; var Ids: b=selector[r,c]; } in sibling=1 /\\ (a=b)=(table[a]=table[b]));\n",
        "solve :: int_search(array1d(selector),input_order,indomain_min,complete) satisfy;\n"
    );
    std::fs::write(dir.join("root.mzn"), neighbour).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\nfunction int: '-'(int: left,int: right); function bool: '>'(int: left,int: right);\n")).unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    let calls = resolve_callables(&context, &bindings);
    let instantiations = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let callable =
        resolve_callable_definitions(&context, &bindings, &calls, &instantiations, &domains);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    for name in ["a", "b"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(
            search.declarations[local.id.0].coverage,
            SearchCoverage::Unknown
        );
        assert!(callable.inspected_locals.contains(&local.id));
        assert!(!callable.definitions.iter().any(|d| d.target == local.id));
    }
    assert_eq!(
        coverage(&bindings, &search, "sibling"),
        SearchCoverage::Uncovered
    );
    assert!(
        !callable
            .definitions
            .iter()
            .any(|d| bindings.declarations[d.target.0].name == "sibling")
    );
    let combined = analyze_model(
        &context,
        &LintOptions::from_selection("search-coverage,unbounded-variable").unwrap(),
    );
    assert!(
        combined.limitations.is_empty(),
        "{:?}",
        combined.limitations
    );
    assert!(
        combined
            .rules
            .iter()
            .all(|r| matches!(r.outcome, RuleOutcome::Completed))
    );
    for name in ["selector", "a", "b"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert!(combined.findings.iter().all(
            |f| f.rule != zincite_lint::Rule::UnboundedVariable || f.location != local.location
        ));
    }
    let sibling = bindings
        .declarations
        .iter()
        .find(|d| d.name == "sibling")
        .unwrap();
    assert!(
        combined
            .findings
            .iter()
            .any(|f| f.rule == zincite_lint::Rule::UnboundedVariable
                && f.location == sibling.location)
    );
    std::fs::remove_dir_all(dir).unwrap();
    let comprehension = concat!(
        "set of int: Input; int: bound; bool: enabled; int: low; int: high; array[1..bound] of var int: input; var int: sibling; var int: maximum_value;\n",
        "array[1..bound] of var 1..10: root_constant=[if enabled then low else high endif|i in 1..bound];\n",
        "array[1..card(Input)] of var int: root_indexed=[input[i]|i in Input];\n",
        "constraint let {array[1..card(Input)] of var int: constants=[1|i in Input]; array[1..card(Input)] of var int: indexed=[input[i]|i in Input];} in sibling=1 /\\ constants[1]=indexed[1] /\\ maximum(maximum_value,indexed);\n",
        "solve :: int_search(input,input_order,indomain_min,complete) satisfy;\n"
    );
    let (dir, _) = model("uncertain-local-comprehension", comprehension, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: card(set of int: value); predicate array_int_maximum(var int: m,array[int] of var int: xs); predicate maximum(var int: m,array[int] of var int: xs)=array_int_maximum(m,xs);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (source, limited) in [
        (comprehension.to_owned(), false),
        (
            comprehension.replace(";} in sibling=1", "; var opt int: unused=<>;} in sibling=1"),
            true,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        let result = analyze_model(
            &context,
            &LintOptions::from_selection("search-coverage,unbounded-variable,constant-variable")
                .unwrap(),
        );
        assert_eq!(
            !result.limitations.is_empty(),
            limited,
            "{:?}",
            result.limitations
        );
        if !limited {
            for name in ["constants", "indexed"] {
                let local = bindings
                    .declarations
                    .iter()
                    .find(|d| d.name == name)
                    .unwrap();
                assert_eq!(
                    search.declarations[local.id.0].coverage,
                    SearchCoverage::Unknown
                );
                assert!(result.findings.iter().all(|f| f.location != local.location));
            }
            assert_eq!(
                coverage(&bindings, &search, "root_indexed"),
                SearchCoverage::Unknown
            );
            assert_eq!(
                coverage(&bindings, &search, "maximum_value"),
                SearchCoverage::Uncovered
            );
        }
        assert_eq!(
            coverage(&bindings, &search, "sibling"),
            if limited {
                SearchCoverage::Unknown
            } else {
                SearchCoverage::Uncovered
            }
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let summed = concat!(
        "set of int: Input; int: bound; array[1..bound] of var int: input;\n",
        "array[1..bound] of var int: values=[input[i]|i in Input]; var int: total=sum(values);\n",
        "solve :: int_search(input,input_order,indomain_min,complete) satisfy;\n"
    );
    let (dir, _) = model("initialized-decision-sum", summed, "");
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}\nfunction int: opaque_count(int: value); function int: length(array[int] of bool: values); function int: length(array[int] of int: values); function int: card(set of int: values); function bool: exists(array[int] of bool: values); function bool: '<'(int: left,int: right); function bool: '!='(int: left,int: right); function bool: '!='(set of int: left,set of int: right); function int: 'div'(int: left,int: right); function int: 'mod'(int: left,int: right);\n"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    for (source, limited) in [
        (summed.to_owned(), false),
        (
            summed.replace("int: bound;", "int: bound; int: divisor;")
                .replace("[input[i]|i in Input]", "[input[i]+(i div divisor)+(i mod divisor)|i in Input]"),
            false,
        ),
        (
            summed.replace("[input[i]|i in Input]", "[input[i]+(i div 0)|i in Input]"),
            true,
        ),
        (
            summed.replace("int: bound;", "array[Input] of set of int: neighbours=[{b|b in Input where b!=a}|a in Input]; int: bound=length([true|a in Input,b in neighbours[a] where a<b]); set of int: selected={a|a in Input where exists(b in neighbours[a])(a!=b)}; int: counted=card(selected);")
                .replace("[input[i]|i in Input]", "[input[i]|i in 1..counted]"),
            false,
        ),
        (
            summed.replace("int: bound;", "int: selector; set of int: Target; array[Input] of set of int: neighbours=[{b|b in Input where b!=a}|a in Input]; int: bound=length([true|a in Input,b in neighbours[a] where a<b]); set of int: selected={a|a in Input where exists(b in neighbours[a])(neighbours[selector]!=Target)}; int: counted=card(selected);")
                .replace("[input[i]|i in Input]", "[input[i]|i in 1..counted]"),
            false,
        ),

        (
            summed
                .replace("int: bound;", "int: bound; int: extra=opaque_count(bound);")
                .replace("[input[i]|", "[input[i]+extra|"),
            true,
        ),
        (
            summed
                .replace("int: bound;", "int: bound; int: extra=extra;")
                .replace("[input[i]|", "[input[i]+extra|"),
            true,
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        let (bindings, search) = facts(&context);
        assert_eq!(
            coverage(&bindings, &search, "total"),
            SearchCoverage::Unknown
        );
        let result = analyze_model(
            &context,
            &LintOptions::from_selection("search-coverage,unbounded-variable").unwrap(),
        );
        assert_eq!(
            !result.limitations.is_empty(),
            limited,
            "{:?}",
            result.limitations
        );
        if !limited {
            let indices = analyze_model(
                &context,
                &LintOptions::from_selection("array-index-start").unwrap(),
            );
            assert!(indices.limitations.is_empty(), "{:?}", indices.limitations);
            assert_eq!(indices.rules[0].outcome, RuleOutcome::Completed);
            let total = bindings
                .declarations
                .iter()
                .find(|d| d.name == "total")
                .unwrap();
            assert!(result.findings.iter().all(|f| f.location != total.location));
        }
    }
    // A named parameter collection still requires inspection of its initializer.
    std::fs::write(
        dir.join("root.mzn"),
        concat!(
            "int: bound; array[1..bound] of int: counts=[opaque_count(i)|i in 1..bound];\n",
            "int: counted=length(counts); array[1..counted] of int: unsafe_extent;\n",
            "var int: total=length(counts); solve satisfy;\n"
        ),
    )
    .unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unbounded-variable").unwrap(),
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|limit| limit.message.contains("for 'total'")),
        "{:?}",
        result
    );
    let indices = analyze_model(
        &context,
        &LintOptions::from_selection("array-index-start").unwrap(),
    );
    assert!(matches!(
        indices.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(!indices.limitations.is_empty());
    std::fs::write(
        dir.join("root.mzn"),
        "annotation count_hint; set of int: Input; int: counted :: count_hint=length([true|i in Input]); array[1..counted] of int: values; solve satisfy;",
    ).unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let indices = analyze_model(
        &context,
        &LintOptions::from_selection("array-index-start").unwrap(),
    );
    assert!(matches!(
        indices.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(!indices.limitations.is_empty());
    std::fs::write(
        dir.join("root.mzn"),
        concat!(
            "array[1..2,1..2] of var int: matrix; var int: total=sum(matrix);\n",
            "solve :: int_search(matrix,input_order,indomain_min,complete) satisfy;\n"
        ),
    )
    .unwrap();
    let context = load_model(dir.join("root.mzn"), &options);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "total"),
        SearchCoverage::Scalar
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("search-coverage,unbounded-variable").unwrap(),
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    std::fs::remove_dir_all(dir).unwrap();
    // Keep the complete Routing336 producer chain in one symbolic public case.
    let routing = concat!(
        "int: count; set of int: Cols=1..2; var 0..9: coord_a; var 0..9: coord_b; array[int] of var int: coords=[coord_a,coord_b,coord_a,coord_b];\n",
        "set of int: Rows=1..length(coords) div length(Cols);\n",
        "array[int] of int: kinds=[5,3]; set of int: Boxes=index_set(kinds);\n",
        "set of int: Pref={z|z in Boxes where 5==kinds[z]};\n",
        "array[int] of int: pipe=[1,1]; array[int] of int: diam=[4];\n",
        "array[int] of int: node=[1,2]; array[int] of int: direction=[3,4];\n",
        "array[int] of int: radius=[diam[p] div 2|p in index_set(diam)];\n",
        "array[int] of int: base=[0,0,0,0]; array[int] of int: sizes=[1,1,1,1];\n",
        "array[int,int] of int: corners=array2d(Boxes,Cols,[base[k]+sizes[k]|k in index_set(base)]);\n",
        "array[int,int] of var int: positions=array2d(Rows,Cols,coords);\n",
        "var 0..9: first_coord; var 0..9: last_coord; array[int] of var int: bound_coords=[first_coord,last_coord];\n",
        "array[Rows,Cols] of var lb_array(bound_coords)..ub_array(bound_coords): legs;\n",
        "test first(Rows: i)=1==i;\n",
        "test leg(Rows: i)=i<length(Rows) /\\ pipe[i+1]==pipe[i];\n",
        "array[int,int,int] of var bool: disjoint=array3d(Boxes,Rows,Cols,[if kinds[z]<3 \\/ not leg(i) then true else corners[z,k]<=legs[i,k] endif|z in Boxes,i in Rows,k in Cols]);\n",
        "array[int,int] of var bool: inside=array2d(Rows,Boxes,[if kinds[z]<3 then false else forall(k in Cols)(if 1==k then positions[i,k]-radius[pipe[i]]>=corners[z,k] /\\ positions[i,k]-radius[pipe[i]]<=corners[z,k] else positions[i,k]<=corners[z,k] endif) endif|i in Rows,z in Boxes]);\n",
        "array[int] of var 0..1: not_pref=[if first(i) \\/ not leg(i) then 0 else 1-sum(z in Pref)(inside[i,z]) endif|i in Rows];\n",
        "array[Rows] of var 0..1: exists_leg; array[int] of int: price=[3,4];\n",
        "array[int] of var int: cost=[if first(i) \\/ not leg(i) then 0 else exists_leg[i]*(not_pref[i]+sum(z in Pref)(inside[i,z] * (price[z] div 2+1))) endif|i in Rows];\n",
        "array[int] of set of int: legs_by_pipe=[{i|i in Rows where p==node[i]}|p in Boxes]; array[int] of float: factor=[1.0,0.0];\n",
        "array[int,int] of var int: orthogonal=array2d(Boxes,Cols,[sum(j in legs_by_pipe[p],q in Cols diff{k})(if factor[p]<=0.0 then 0 else pow(legs[j,q],3) endif)|p in Boxes,k in Cols]);\n",
        "constraint forall(i in Rows,k in Cols)(if not leg(i) then legs[i,k]==0 else legs[i,k]>=lb(positions[i,k]) /\\ legs[i,k]<=ub(positions[i,k]) endif);\n",
        "constraint forall(i in Rows)(if first(i) then exists_leg[i]==0 else exists_leg[i]==exists_leg[i-1]+1 endif);\n",
        "constraint forall(i in Rows,k in Cols)(if k==(direction[node[i]]-1) mod 3+1 then legs[i,k]>=0 else true endif);\n",
        "array[Rows] of var 0..6: selected_direction;\n",
        "constraint forall(i in Rows)(if 1==i then selected_direction[i]==direction[node[i]] else selected_direction[i]==0 endif);\n",
        "array[Boxes] of int: first_leg=[min(legs_by_pipe[p])|p in Boxes]; array[Boxes] of int: last_leg=[max(legs_by_pipe[p])|p in Boxes];\n",
        "int: units; int: reduction=1; var int: float_let_output;\n",
        "constraint let { float: kappa=int2float(units)/4.0; float: unused_kappa=int2float(units)/8.0; } in\n",
        "forall(p in Boxes,k in Cols)(if factor[p]<=0.0 then true else\n",
        "let { int: delta=abs(fix(legs[last_leg[p],k])-fix(legs[first_leg[p],k])); int: coefficient=ceil(factor[p]*int2float(delta)/kappa); } in\n",
        "forall(j in legs_by_pipe[p],q in Cols diff{k})(float_let_output=coefficient /\\ ((coefficient+reduction-1) div reduction)*legs[j,q]<=orthogonal[p,k]) endif);\n",
        "var 0..9: unrelated; solve :: int_search(coords,input_order,indomain_min,complete) satisfy;\n"
    );
    let (dir, _) = model("routing-symbolic-producer-chain", routing, "");
    // This case uses the present standard signatures; other cases retain CORE.
    let routing_core = CORE
        .replace(
            "function var bool: forall(array[int] of var opt bool: body);\n",
            "",
        )
        .replace(
            "function var int: sum(array[int] of var int: body);",
            "function var int: sum(array[$T] of var int: body);",
        );
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!(
            "{routing_core}{}",
            concat!(
                "function int: '+'(int: left,int: right); function int: '-'(int: left,int: right);\n",
                "function var int: '-'(var int: left,var int: right);\n",
                "function int: '*'(int: left,int: right); function var int: '*'(var int: left,var int: right);\n",
                "function int: 'div'(int: left,int: right); function var int: 'div'(var int: left,var int: right); function int: 'mod'(int: left,int: right); function int: length(array[$T] of any $U: values);\n",
                "function bool: '<'(int: left,int: right); function bool: '<='(int: left,int: right);\n",
                "function var bool: '<='(var int: left,var int: right); function var bool: '>='(var int: left,var int: right);\n",
                "function bool: '/\\'(bool: left,bool: right); function bool: '\\/'(bool: left,bool: right); function bool: 'not'(bool: value);\n",
                "function bool: forall(array[$T] of bool: body); function var bool: forall(array[$T] of var bool: body);\n",
                "function int: sum(array[$T] of int: body);\n",
                "function array[$$R,$$C] of any $V: array2d(set of $$R: rows,set of $$C: columns,array[$U] of any $V: values);\n",
                "function array[$$R,$$C,$$D] of any $V: array3d(set of $$R: rows,set of $$C: columns,set of $$D: depths,array[$U] of any $V: values);\n",
                "function int: lb_array(array[$U] of var int: values); function int: ub_array(array[$U] of var int: values);\n",
                "function int: lb(var int: value); function int: ub(var int: value);\n",
                "function bool: '<='(float: left,float: right); function var int: pow(var int: value,int: exponent);\n",
                "function set of int: 'diff'(set of int: left,set of int: right);\n",
                "function float: int2float(int: value); function int: ceil(float: value); function int: fix(var int: value);\n",
                "function float: '*'(float: left,float: right); function float: '/'(float: left,float: right);\n",
                "function int: abs(int: value); function int: min(set of int: values); function int: max(set of int: values);\n",
            )
        ),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let parameter_int = |ty: &zincite_lint::TypeInst| {
        ty.kind == zincite_lint::TypeKind::Int
            && ty.instantiation == zincite_lint::Instantiation::Parameter
            && !ty.optional
    };
    for written in ["length(Cols)", "length(Rows)"] {
        let start = routing.find(written).unwrap();
        let call = calls
            .calls
            .iter()
            .find(|call| call.file == 0 && call.location.range.start == start)
            .unwrap();
        assert!(
            matches!(&call.outcome,
                zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                    if context.files[bindings.declarations[declaration.0].file].kind
                        == zincite_lint::SourceKind::StandardLibrary
                        && bindings.declarations[declaration.0].name == "length"
                        && parameter_int(return_type)
                        && parameters.len() == 1
                        && parameters[0].instantiation == zincite_lint::Instantiation::Parameter
                        && !parameters[0].optional
                        && matches!(&parameters[0].kind,
                            zincite_lint::TypeKind::Array { indices, element }
                                if indices.len() == 1 && parameter_int(&indices[0])
                                    && parameter_int(element))
            ),
            "{written}: {:?}",
            call.outcome
        );
        let argument_start = start + "length(".len();
        let argument_end = start + written.len() - 1;
        assert!(
            calls.expressions.iter().any(|expression| {
                expression.file == 0
                    && expression.location.range == (argument_start..argument_end)
                    && expression.ty.instantiation == zincite_lint::Instantiation::Parameter
                    && !expression.ty.optional
                    && matches!(&expression.ty.kind, zincite_lint::TypeKind::Set(element)
                    if parameter_int(element))
            }),
            "{written}: written set facts must remain sets"
        );
    }
    // Compiler matching must retain the written Bool instead of relabelling it Int.
    for (start, written) in routing.match_indices("inside[i,z]") {
        assert!(calls.expressions.iter().any(|expression| {
            expression.file == 0
                && expression.location.range == (start..start + written.len())
                && expression.ty.kind == zincite_lint::TypeKind::Bool
                && expression.ty.instantiation == zincite_lint::Instantiation::Decision
                && !expression.ty.optional
        }));
    }
    let product_start = routing.rfind("inside[i,z] *").unwrap() + "inside[i,z] ".len();
    let product = calls
        .calls
        .iter()
        .find(|call| call.file == 0 && call.location.range.start == product_start)
        .unwrap();
    assert!(
        matches!(&product.outcome,
            zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                if context.files[bindings.declarations[declaration.0].file].kind
                    == zincite_lint::SourceKind::StandardLibrary
                    && parameters.len() == 2
                    && parameters.iter().chain(std::iter::once(return_type)).all(|ty|
                        ty.kind == zincite_lint::TypeKind::Int
                            && ty.instantiation == zincite_lint::Instantiation::Decision
                            && !ty.optional)
        ),
        "{:?}",
        product.outcome
    );
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    let search =
        resolve_search_coverage(&context, &bindings, &calls, &inst, &domains, &definitions);
    for name in ["inside", "not_pref", "cost", "disjoint", "orthogonal"] {
        let id = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == name)
            .unwrap()
            .id;
        assert_eq!(search.declarations[id.0].coverage, SearchCoverage::Unknown);
        assert!(
            !definitions
                .definitions
                .iter()
                .chain(&callable.definitions)
                .any(|definition| definition.target == id
                    && definition.safety == DefinitionSafety::Supported
                    && definition.coverage == DefinitionCoverage::WholeArray)
        );
    }
    let orthogonal = bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "orthogonal")
        .unwrap()
        .id;
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| d.target == orthogonal && matches!(d.safety, DefinitionSafety::Unknown(_)))
    );
    assert_eq!(
        coverage(&bindings, &search, "coords"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "unrelated"),
        SearchCoverage::Uncovered
    );
    let float_output = bindings
        .declarations
        .iter()
        .find(|declaration| declaration.top_level && declaration.name == "float_let_output")
        .unwrap()
        .id;
    assert_eq!(
        search.declarations[float_output.0].coverage,
        SearchCoverage::Uncovered
    );
    assert!(
        !callable
            .definitions
            .iter()
            .any(|definition| definition.target == float_output)
    );
    let selected_direction = bindings
        .declarations
        .iter()
        .find(|declaration| declaration.top_level && declaration.name == "selected_direction")
        .unwrap()
        .id;
    assert!(
        !callable
            .definitions
            .iter()
            .any(|definition| definition.target == selected_direction),
        "Unproved selected-direction membership must supply no callable output"
    );
    let let_start = routing.find("constraint let {").unwrap();
    let let_end = routing[let_start..].find(" endif);").unwrap() + let_start + " endif);".len();
    assert!(!callable.inspected_locals.iter().any(|id| {
        let local = &bindings.declarations[id.0];
        local.file == 0
            && let_start <= local.syntax_range.start
            && local.syntax_range.end <= let_end
    }));
    let result = analyze_model(&context, &selected());
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    let unrelated = bindings
        .declarations
        .iter()
        .find(|declaration| declaration.name == "unrelated")
        .unwrap();
    assert!(
        result
            .findings
            .iter()
            .any(|finding| finding.location == unrelated.location)
    );
    let conditional_target = bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "exists_leg")
        .unwrap()
        .id;
    assert!(matches!(
        search.declarations[conditional_target.0].coverage,
        SearchCoverage::Uncovered | SearchCoverage::Unknown
    ));
    assert!(
        !definitions
            .definitions
            .iter()
            .chain(&callable.definitions)
            .any(|d| d.target == conditional_target
                && d.enforcement == zincite_lint::DefinitionEnforcement::Enforced
                && d.safety == DefinitionSafety::Supported
                && d.coverage == DefinitionCoverage::WholeArray)
    );
    // Equality inspection cannot hide a hazardous selector or literal source.
    for (before, after) in [
        (
            "selected_direction[i]==direction[node[i]]",
            "selected_direction[i]==direction[node[i]-(1 div 0)]",
        ),
        (
            "selected_direction[i]==direction[node[i]]",
            "selected_direction[i]==direction[node[i]-(9223372036854775807+1)]",
        ),
        (
            "array[int] of int: direction=[3,4]",
            "array[int] of int: direction=[9223372036854775807+1,4]",
        ),
        (
            "set of int: Rows=1..length(coords) div length(Cols)",
            "set of int: Rows=1..(9223372036854775807+1)",
        ),
    ] {
        assert!(routing.contains(before));
        std::fs::write(dir.join("root.mzn"), routing.replace(before, after)).unwrap();
        let equality_context = load_model(dir.join("root.mzn"), &options);
        assert!(
            equality_context.errors.is_empty(),
            "{after}: {:?}",
            equality_context.errors
        );
        let equality_result = analyze_model(&equality_context, &selected());
        assert!(
            matches!(
                equality_result.rules[0].outcome,
                RuleOutcome::Limited { .. }
            ),
            "{after}: {:?}",
            equality_result.rules[0].outcome
        );
        assert!(
            !equality_result.limitations.is_empty(),
            "{after}: a hazardous equality must retain a search limitation"
        );
    }
    // A closed hazardous sibling cannot become an unknown wrapped selection.
    for wrapped in [
        "direction[node[i]]-(1 div 0)",
        "direction[node[i]]-(9223372036854775807+1)",
    ] {
        std::fs::write(
            dir.join("root.mzn"),
            routing.replace("direction[node[i]]-1", wrapped),
        )
        .unwrap();
        let wrapped_context = load_model(dir.join("root.mzn"), &options);
        assert!(
            wrapped_context.errors.is_empty(),
            "{wrapped}: {:?}",
            wrapped_context.errors
        );
        let wrapped_result = analyze_model(&wrapped_context, &selected());
        assert!(
            matches!(wrapped_result.rules[0].outcome, RuleOutcome::Limited { .. }),
            "{wrapped}: {:?}",
            wrapped_result.rules[0].outcome
        );
        assert!(
            !wrapped_result.limitations.is_empty(),
            "{wrapped}: a hazardous sibling must retain a search limitation"
        );
    }
    // Inspection must retain a partial child in the otherwise unproved else.
    std::fs::write(
        dir.join("root.mzn"),
        routing.replace(
            "legs[i,k]>=lb(positions[i,k])",
            "legs[i,k]>=lb(positions[i,k])+(1 div 0)",
        ),
    )
    .unwrap();
    let partial_context = load_model(dir.join("root.mzn"), &options);
    assert!(
        partial_context.errors.is_empty(),
        "{:?}",
        partial_context.errors
    );
    let partial_result = analyze_model(&partial_context, &selected());
    assert!(matches!(
        partial_result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(!partial_result.limitations.is_empty());
    // A partial child remains unsupported even when its Float branch may be unused.
    std::fs::write(
        dir.join("root.mzn"),
        routing.replace("pow(legs[j,q],3)", "pow(legs[j,q]+(1 div 0),3)"),
    )
    .unwrap();
    let unsafe_context = load_model(dir.join("root.mzn"), &options);
    assert!(
        unsafe_context.errors.is_empty(),
        "{:?}",
        unsafe_context.errors
    );
    let unsafe_bindings = resolve_bindings(&unsafe_context);
    let unsafe_calls = resolve_callables(&unsafe_context, &unsafe_bindings);
    let unsafe_inst = resolve_instantiations(&unsafe_context, &unsafe_bindings, &unsafe_calls);
    let unsafe_domains = resolve_domains(&unsafe_context, &unsafe_bindings);
    let unsafe_definitions = resolve_definitions(
        &unsafe_context,
        &unsafe_bindings,
        &unsafe_calls,
        &unsafe_inst,
        &unsafe_domains,
    );
    let unsafe_target = unsafe_bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "orthogonal")
        .unwrap()
        .id;
    assert!(
        unsafe_definitions
            .definitions
            .iter()
            .any(|d| d.target == unsafe_target
                && matches!(d.safety, DefinitionSafety::Unsupported(_)))
    );
    let unsafe_result = analyze_model(&unsafe_context, &selected());
    assert!(!unsafe_result.limitations.is_empty());
    // The unused Float local is still inspected with an unknown numerator.
    std::fs::write(
        dir.join("root.mzn"),
        routing.replace(
            "unused_kappa=int2float(units)/8.0",
            "unused_kappa=int2float(units)/0.0",
        ),
    )
    .unwrap();
    let zero_context = load_model(dir.join("root.mzn"), &options);
    assert!(zero_context.errors.is_empty(), "{:?}", zero_context.errors);
    let zero_result = analyze_model(&zero_context, &selected());
    assert!(!zero_result.limitations.is_empty());
    assert!(matches!(
        zero_result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let zero_start = routing.find("unused_kappa=int2float(units)/8.0").unwrap();
    assert!(
        zero_result
            .limitations
            .iter()
            .any(|limit| limit.location.range.start <= zero_start
                && zero_start < limit.location.range.end),
        "{:?}",
        zero_result.limitations
    );
    // A named global source must also be inspected for an unused local.
    // The declaration initializer takes the public direct-safety/RHS path.
    let transitive = format!(
        "{routing}\nfloat: bad=int2float(units)/0.0;\nvar bool: transitive_let=let {{ float: unused=bad; }} in true;\n"
    );
    std::fs::write(dir.join("root.mzn"), &transitive).unwrap();
    let transitive_context = load_model(dir.join("root.mzn"), &options);
    assert!(
        transitive_context.errors.is_empty(),
        "{:?}",
        transitive_context.errors
    );
    let (transitive_bindings, transitive_search) = facts(&transitive_context);
    assert_eq!(
        coverage(&transitive_bindings, &transitive_search, "transitive_let"),
        SearchCoverage::Unknown
    );
    let transitive_result = analyze_model(&transitive_context, &selected());
    assert!(matches!(
        transitive_result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let transitive_start = transitive.find("var bool: transitive_let").unwrap();
    assert!(
        transitive_result
            .limitations
            .iter()
            .any(|limit| limit.location.range.start <= transitive_start
                && transitive_start < limit.location.range.end),
        "{:?}",
        transitive_result.limitations
    );
    let membership = concat!(
        "set of int: Index;\n",
        "array[Index] of int: selected;\n",
        "array[int] of int: direction;\n",
        "array[Index] of var 0..6: result;\n",
        "test active(int: index) = index > 0;\n",
        "constraint forall(index in Index)(\n",
        "  if active(index) then result[index] in {0, direction[selected[index]]}\n",
        "  else result[index] = 0 endif\n",
        ");\n",
        "solve satisfy;\n",
    );
    let library = dir.join("library/std/stdlib.mzn");
    let mut standard = std::fs::read_to_string(&library).unwrap();
    standard.push_str("function bool: '>'(int: left,int: right);\n");
    std::fs::write(&library, standard).unwrap();
    std::fs::write(dir.join("root.mzn"), membership).unwrap();
    let membership_context = load_model(dir.join("root.mzn"), &options);
    assert!(membership_context.errors.is_empty());
    let (membership_bindings, membership_search) = facts(&membership_context);
    assert_eq!(membership_search.root_state, ModelRootState::Complete);
    assert!(
        membership_search.limitations.is_empty(),
        "{:?}",
        membership_search.limitations
    );
    assert_eq!(
        coverage(&membership_bindings, &membership_search, "result"),
        SearchCoverage::Uncovered
    );
    let membership_calls = resolve_callables(&membership_context, &membership_bindings);
    let membership_inst =
        resolve_instantiations(&membership_context, &membership_bindings, &membership_calls);
    let membership_domains = resolve_domains(&membership_context, &membership_bindings);
    let membership_callable = resolve_callable_definitions(
        &membership_context,
        &membership_bindings,
        &membership_calls,
        &membership_inst,
        &membership_domains,
    );
    let membership_result = membership_bindings
        .declarations
        .iter()
        .find(|declaration| declaration.top_level && declaration.name == "result")
        .unwrap()
        .id;
    assert!(
        !membership_callable
            .definitions
            .iter()
            .any(|definition| definition.target == membership_result)
    );
    assert!(matches!(
        analyze_model(&membership_context, &selected()).rules[0].outcome,
        RuleOutcome::Completed
    ));
    let outside_membership = membership
        .replace(
            "array[int] of int: direction;",
            "array[1..1] of int: direction;",
        )
        .replace("direction[selected[index]]", "direction[2]");
    std::fs::write(dir.join("root.mzn"), &outside_membership).unwrap();
    let outside_context = load_model(dir.join("root.mzn"), &options);
    assert!(outside_context.errors.is_empty());
    let outside_result = analyze_model(&outside_context, &selected());
    assert!(matches!(
        outside_result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let membership_start = outside_membership.find("result[index] in").unwrap();
    assert!(
        outside_result.limitations.iter().any(|limit| {
            limit.location.range.start <= membership_start
                && membership_start < limit.location.range.end
        }),
        "{:?}",
        outside_result.limitations
    );
    // Existing unsafe-source, annotation, optional, opaque and cycle cases stay above.
    std::fs::remove_dir_all(dir).unwrap();
    // Keep both filtered symmetry relations and the concatenated maximum together.
    {
        use zincite_lint::{CallOutcome, Instantiation, TypeInst, TypeKind};
        let pillars = r#"include "globals.mzn";

int: plank_count;
int: pillar_count;
int: width_limit;
int: height_limit;
set of int: Planks = 1..plank_count;
set of int: Pillars = 1..pillar_count;
set of int: Horizontal = 0..width_limit-1;
set of int: Vertical = 0..height_limit-1;
array[Planks] of int: plank_width;
array[Pillars] of int: pillar_width;
array[Pillars] of int: pillar_height;
array[Planks] of var Horizontal: plank_x;
array[Planks] of var Vertical: plank_y;
array[Pillars] of var Horizontal: pillar_x;
array[Pillars] of var Vertical: pillar_y;

% Interchangeable pieces use the same vertical, then horizontal, ordering.
constraint :: "Order equal planks" symmetry_breaking_constraint(
  forall(a,b in Planks where a < b /\ plank_width[a] = plank_width[b])(
    plank_y[a] <= plank_y[b] /\ (plank_y[a] = plank_y[b] -> plank_x[a] < plank_x[b])
  )
);
constraint :: "Order equal pillars" symmetry_breaking_constraint(
  forall(a,b in Pillars where a < b /\ pillar_width[a] = pillar_width[b]
      /\ pillar_height[a] = pillar_height[b])(
    pillar_y[a] <= pillar_y[b] /\ (pillar_y[a] = pillar_y[b] -> pillar_x[a] < pillar_x[b])
  )
);

% Ignoring a symmetry wrapper must not create a definition certificate.
var int: auxiliary;
constraint :: "Ignored equality control" symmetry_breaking_constraint(auxiliary = 0);
var int: objective = max([plank_y[p] + 1 | p in Planks]
  ++ [pillar_y[p] + pillar_height[p] | p in Pillars]);
solve :: seq_search([
  int_search([if j = 1 then plank_x[p] else plank_y[p] endif
              | p in Planks, j in 1..2], input_order, indomain_min, complete),
  int_search([if j = 1 then pillar_x[p] else pillar_y[p] endif
              | p in Pillars, j in 1..2], input_order, indomain_min, complete)
]) minimize objective;
"#;
        let dir = std::env::temp_dir().join(format!(
            "zincite-search-pillars-symmetry-extrema-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        std::fs::write(dir.join("root.mzn"), pillars).unwrap();
        let core = CORE
            .replace(
                "function var bool: forall(array[int] of var opt bool: body);",
                "function var bool: forall(array[$T] of var bool: body);",
            )
            .replace(
                "function array[int] of var int: '++'(array[int] of var int: left,array[int] of var int: right);",
                "function array[int] of any $T: '++'(array[$$X] of any $T: x,array[$$Y] of any $T: y);",
            );
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!(
                "{core}{}",
                concat!(
                    "predicate symmetry_breaking_constraint(var bool: b);\n",
                    "function var $$T: max(array[$U] of var $$T: x);\n",
                    "function bool: '<'($T: x,$T: y); function var bool: '<'(var $T: x,var $T: y);\n",
                    "function var bool: '<='(var $T: x,var $T: y); function bool: '/\\'(bool: x,bool: y);\n",
                    "function int: '-'(int: x,int: y); function int: 'div'(int: x,int: y);\n",
                ),
            ),
        )
        .unwrap();
        std::fs::write(dir.join("library/std/globals.mzn"), "").unwrap();
        // The adjacent instance is compiler input, never invariant analysis data.
        std::fs::write(dir.join("instance.dzn"), "plank_count = 2;\npillar_count = 2;\nwidth_limit = 4;\nheight_limit = 4;\nplank_width = [2, 2];\npillar_width = [1, 1];\npillar_height = [1, 1];\n").unwrap();
        let options = ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        };
        let context = load_model(dir.join("root.mzn"), &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let integer = |ty: &TypeInst, instantiation| {
            !ty.optional && ty.kind == TypeKind::Int && ty.instantiation == instantiation
        };
        let integer_array = |ty: &TypeInst| {
            !ty.optional
                && ty.instantiation == Instantiation::Decision
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && integer(&indices[0], Instantiation::Parameter)
                        && integer(element, Instantiation::Decision))
        };
        let boolean = |ty: &TypeInst| {
            !ty.optional && ty.kind == TypeKind::Bool && ty.instantiation == Instantiation::Decision
        };
        let standard = |id: zincite_lint::DeclarationId, name: &str| {
            let declaration = &bindings.declarations[id.0];
            let source = &context.files[declaration.file];
            declaration.name == name
                && source.kind == zincite_lint::SourceKind::StandardLibrary
                && source.implicit
        };
        let at = |start| {
            calls
                .calls
                .iter()
                .find(|call| call.file == 0 && call.location.range.start == start)
                .unwrap()
        };
        for written in [
            "symmetry_breaking_constraint(\n  forall(a,b in Planks",
            "symmetry_breaking_constraint(\n  forall(a,b in Pillars",
            "symmetry_breaking_constraint(auxiliary = 0)",
        ] {
            let call = at(pillars.find(written).unwrap());
            assert!(
                matches!(&call.outcome,
                CallOutcome::Resolved { declaration, parameters, return_type }
                    if standard(*declaration, "symmetry_breaking_constraint") && parameters.len() == 1
                        && boolean(&parameters[0]) && boolean(return_type)),
                "{written}: {:?}",
                call.outcome
            );
        }
        let maximum = at(pillars.find("max([").unwrap());
        assert!(
            matches!(&maximum.outcome,
            CallOutcome::Resolved { declaration, parameters, return_type }
                if standard(*declaration, "max") && parameters.len() == 1
                    && integer_array(&parameters[0]) && integer(return_type, Instantiation::Decision)),
            "{:?}",
            maximum.outcome
        );
        // Binary call facts identify the written operator, not its left operand.
        let concatenation = at(pillars.find("++").unwrap());
        assert!(
            matches!(&concatenation.outcome,
            CallOutcome::Resolved { declaration, parameters, return_type }
                if standard(*declaration, "++") && parameters.len() == 2
                    && parameters.iter().chain(std::iter::once(return_type)).all(integer_array)),
            "{:?}",
            concatenation.outcome
        );
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
        let wrapper_starts: Vec<_> = [
            "symmetry_breaking_constraint(\n  forall(a,b in Planks",
            "symmetry_breaking_constraint(\n  forall(a,b in Pillars",
            "symmetry_breaking_constraint(auxiliary = 0)",
        ]
        .iter()
        .map(|written| pillars.find(written).unwrap())
        .collect();
        // Inspect raw failures before Search can hide them behind covered targets.
        let wrapper_unavailable: Vec<_> = callable
            .unavailable
            .iter()
            .filter(|missing| {
                missing.location.path == context.files[0].path
                    && wrapper_starts.iter().any(|start| {
                        missing.location.range.start <= *start
                            && *start < missing.location.range.end
                    })
            })
            .collect();
        assert!(wrapper_unavailable.is_empty(), "{:?}", wrapper_unavailable);
        let result = analyze_model(&context, &selected());
        // A completed inspection may retain uncovered-source warnings.
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Completed),
            "{:?}",
            result.limitations
        );
        assert!(result.limitations.is_empty(), "{:?}", result.limitations);
        let search =
            resolve_search_coverage(&context, &bindings, &calls, &inst, &domains, &definitions);
        assert_eq!(search.root_state, ModelRootState::Complete);
        assert_eq!(
            coverage(&bindings, &search, "objective"),
            SearchCoverage::Unknown
        );
        assert_eq!(
            coverage(&bindings, &search, "auxiliary"),
            SearchCoverage::Uncovered
        );
        let objective = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == "objective")
            .unwrap()
            .id;
        assert!(
            definitions
                .definitions
                .iter()
                .any(|definition| definition.target == objective
                    && matches!(definition.safety, DefinitionSafety::Unknown(_)))
        );
        assert!(
            !callable
                .definitions
                .iter()
                .any(|definition| definition.target == objective
                    && definition.safety == DefinitionSafety::Supported)
        );
        assert!(
            !definitions
                .bounded_or_defined_targets(&bindings, &domains)
                .contains(&objective)
        );
        for name in ["plank_x", "plank_y", "pillar_x", "pillar_y"] {
            let target = bindings
                .declarations
                .iter()
                .find(|declaration| declaration.top_level && declaration.name == name)
                .unwrap()
                .id;
            assert!(
                matches!(
                    coverage(&bindings, &search, name),
                    SearchCoverage::Uncovered | SearchCoverage::Unknown
                ),
                "{name}"
            );
            assert!(
                !callable
                    .definitions
                    .iter()
                    .any(|definition| definition.target == target
                        && definition.safety == DefinitionSafety::Supported),
                "{name}"
            );
            assert!(
                !search
                    .searched
                    .iter()
                    .any(|value| value.declaration == Some(target)
                        && value.coverage == SearchCoverage::WholeArray),
                "{name}"
            );
        }
        // Full operand inspection must reject a closed partial child despite symbolic headers.
        let partial = pillars.replace("plank_y[p] + 1", "plank_y[p] + (1 div 0)");
        std::fs::write(dir.join("root.mzn"), &partial).unwrap();
        let partial_context = load_model(dir.join("root.mzn"), &options);
        assert!(
            partial_context.errors.is_empty(),
            "{:?}",
            partial_context.errors
        );
        let partial_bindings = resolve_bindings(&partial_context);
        let partial_calls = resolve_callables(&partial_context, &partial_bindings);
        assert!(partial_calls.calls.iter().any(|call| call.file == 0 && matches!(&call.outcome,
            CallOutcome::Resolved { declaration, parameters, return_type }
                if partial_bindings.declarations[declaration.0].name == "div"
                    && partial_context.files[partial_bindings.declarations[declaration.0].file].kind
                        == zincite_lint::SourceKind::StandardLibrary
                    && parameters.len() == 2 && parameters.iter().chain(std::iter::once(return_type))
                        .all(|ty| integer(ty, Instantiation::Parameter)))));
        let partial_result = analyze_model(&partial_context, &selected());
        assert!(matches!(
            partial_result.rules[0].outcome,
            RuleOutcome::Limited { .. }
        ));
        let maximum_start = partial.find("max([").unwrap();
        assert!(
            partial_result
                .limitations
                .iter()
                .any(|limit| limit.location.range.start <= maximum_start
                    && maximum_start < limit.location.range.end
                    && limit
                        .message
                        .contains("direct definition safety or enforcement is unsupported")),
            "{:?}",
            partial_result.limitations
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    // Keep the compiler-positive CTW header, nested selectors and all three calls.
    let ctw = r#"include "globals.mzn";

int: size;
set of int: Positions = 1..size;
array[int, int] of Positions: preference_pairs;
array[Positions] of var Positions: positions;

% Choose one position per item; preferences only contribute to the objective.
constraint :: "Distinct positions" all_different(positions);
var int: penalty = sum(row in index_set_1of2(preference_pairs))(
  positions[preference_pairs[row, 1]] > positions[preference_pairs[row, 2]]
);
var int: objective = penalty * pow(size, 3) + penalty * pow(size, 2)
  + penalty * pow(size, 1) + penalty;

% This ignored-capable wrapper supplies no search coverage for these variables.
array[Positions] of var Positions: auxiliary;
constraint :: "Redundant distinct positions"
  redundant_constraint(all_different(auxiliary));
solve :: int_search(positions, first_fail, indomain_split) minimize objective;
"#;
    let dir = std::env::temp_dir().join(format!(
        "zincite-search-ctw-generator-power-wrapper-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("root.mzn"), ctw).unwrap();
    // Data is retained for the compiler precheck, not loaded as invariant facts.
    std::fs::write(
        dir.join("instance.dzn"),
        "size = 3;\npreference_pairs = [|1, 2|2, 3|];\n",
    )
    .unwrap();
    let ctw_core = CORE
        .replace(
            "function var bool: forall(array[int] of var opt bool: body);",
            "function var bool: forall(array[$T] of var bool: body);",
        )
        .replace(
            "function var int: sum(array[int] of var int: body);",
            "function var int: sum(array[$T] of var int: body);",
        )
        .replace(
            "annotation int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore);",
            "annotation int_search(array[int] of var int: x,ann: select,ann: choice,ann: explore=complete);",
        );
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!(
            "{ctw_core}{}",
            concat!(
                "function set of $$E: index_set_1of2(array[$$E,$$F] of any $U: x);\n",
                "function int: pow(int: x,int: y); function var int: pow(var int: x,int: y);\n",
                "function var int: '*'(var int: x,var int: y); function var bool: '>'(var int: x,var int: y);\n",
                "function var bool: '!='(var int: x,var int: y); function bool: '<'(int: x,int: y);\n",
                "function var bool: '\\/'(var bool: x,var bool: y);\n",
                "predicate redundant_constraint(var bool: b); annotation first_fail; annotation indomain_split;\n",
                "function int: 'div'(int: x,int: y);\n",
            ),
        ),
    )
    .unwrap();
    // Retain the standard generic/defaulted signature and pairwise semantics.
    std::fs::write(
        dir.join("library/std/globals.mzn"),
        "predicate all_different(array[$X] of var $$E: xs,set of $$E: except={})=forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j] \\/ (xs[i] in except /\\ xs[j] in except));\n",
    )
    .unwrap();
    let options = ModelOptions {
        include_dirs: vec![],
        stdlib_dir: Some(dir.join("library")),
    };
    let context = load_model(dir.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let integer = |ty: &zincite_lint::TypeInst, instantiation| {
        !ty.optional && ty.kind == zincite_lint::TypeKind::Int && ty.instantiation == instantiation
    };
    let standard = |id: zincite_lint::DeclarationId, name: &str| {
        let declaration = &bindings.declarations[id.0];
        let source = &context.files[declaration.file];
        declaration.name == name
            && source.kind == zincite_lint::SourceKind::StandardLibrary
            && source.implicit
    };
    let at = |written: &str| {
        let start = ctw.find(written).unwrap();
        calls
            .calls
            .iter()
            .find(|call| call.file == 0 && call.location.range.start == start)
            .unwrap()
    };
    let header = at("index_set_1of2(preference_pairs)");
    assert!(
        matches!(&header.outcome,
            zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                if standard(*declaration, "index_set_1of2") && parameters.len() == 1
                    && !parameters[0].optional
                    && parameters[0].instantiation == zincite_lint::Instantiation::Parameter
                    && matches!(&parameters[0].kind, zincite_lint::TypeKind::Array { indices, element }
                        if indices.len() == 2 && indices.iter().all(|ty| integer(ty, zincite_lint::Instantiation::Parameter))
                            && integer(element, zincite_lint::Instantiation::Parameter))
                    && !return_type.optional
                    && return_type.instantiation == zincite_lint::Instantiation::Parameter
                    && matches!(&return_type.kind, zincite_lint::TypeKind::Set(element)
                        if integer(element, zincite_lint::Instantiation::Parameter))
        ),
        "{:?}",
        header.outcome
    );
    let sum = at("sum(row in");
    assert!(
        matches!(&sum.outcome,
            zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                if standard(*declaration, "sum") && parameters.len() == 1
                    && integer(return_type, zincite_lint::Instantiation::Decision)
                    && !parameters[0].optional
                    && parameters[0].instantiation == zincite_lint::Instantiation::Decision
                    && matches!(&parameters[0].kind, zincite_lint::TypeKind::Array { indices, element }
                        if indices.len() == 1 && integer(&indices[0], zincite_lint::Instantiation::Parameter)
                            && integer(element, zincite_lint::Instantiation::Decision))
        ),
        "{:?}",
        sum.outcome
    );
    let body = "positions[preference_pairs[row, 1]] > positions[preference_pairs[row, 2]]";
    let body_start = ctw.find(body).unwrap();
    assert!(
        calls
            .expressions
            .iter()
            .any(|expression| expression.file == 0
                && expression.location.range == (body_start..body_start + body.len())
                && !expression.ty.optional
                && expression.ty.kind == zincite_lint::TypeKind::Bool
                && expression.ty.instantiation == zincite_lint::Instantiation::Decision)
    );
    for written in ["pow(size, 3)", "pow(size, 2)", "pow(size, 1)"] {
        let call = at(written);
        assert!(
            matches!(&call.outcome,
                zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                    if standard(*declaration, "pow") && parameters.len() == 2
                        && parameters.iter().chain(std::iter::once(return_type))
                            .all(|ty| integer(ty, zincite_lint::Instantiation::Parameter))
            ),
            "{written}: {:?}",
            call.outcome
        );
    }
    let wrapper = at("redundant_constraint(all_different(auxiliary))");
    assert!(
        matches!(&wrapper.outcome,
            zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                if standard(*declaration, "redundant_constraint") && parameters.len() == 1
                    && parameters.iter().chain(std::iter::once(return_type)).all(|ty|
                        !ty.optional && ty.kind == zincite_lint::TypeKind::Bool
                            && ty.instantiation == zincite_lint::Instantiation::Decision)
        ),
        "{:?}",
        wrapper.outcome
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("search-coverage,unbounded-variable").unwrap(),
    );
    // This is the first expected RED: the same located compiler-positive gaps.
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert!(
        result
            .rules
            .iter()
            .all(|rule| matches!(rule.outcome, RuleOutcome::Completed))
    );
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    let search =
        resolve_search_coverage(&context, &bindings, &calls, &inst, &domains, &definitions);
    assert_eq!(search.root_state, ModelRootState::Complete);
    for name in ["penalty", "objective"] {
        let target = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == name)
            .unwrap()
            .id;
        assert!(
            definitions
                .definitions
                .iter()
                .any(|definition| definition.target == target
                    && matches!(definition.safety, DefinitionSafety::Unknown(_))),
            "{name}"
        );
        assert!(
            !callable
                .definitions
                .iter()
                .any(|definition| definition.target == target
                    && definition.safety == DefinitionSafety::Supported),
            "{name}"
        );
        assert!(
            !definitions
                .bounded_or_defined_targets(&bindings, &domains)
                .contains(&target)
        );
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Unknown,
            "{name}"
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "positions"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "auxiliary"),
        SearchCoverage::Uncovered
    );
    // An ignored-capable wrapper must still evaluate its partial argument.
    let partial = ctw.replace(
        "redundant_constraint(all_different(auxiliary))",
        "redundant_constraint((1 div 0)=0)",
    );
    std::fs::write(dir.join("root.mzn"), &partial).unwrap();
    let partial_context = load_model(dir.join("root.mzn"), &options);
    assert!(
        partial_context.errors.is_empty(),
        "{:?}",
        partial_context.errors
    );
    let partial_bindings = resolve_bindings(&partial_context);
    let partial_calls = resolve_callables(&partial_context, &partial_bindings);
    assert!(partial_calls.calls.iter().any(|call| call.file == 0 && matches!(&call.outcome,
        zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
            if partial_bindings.declarations[declaration.0].name == "div"
                && partial_context.files[partial_bindings.declarations[declaration.0].file].kind
                    == zincite_lint::SourceKind::StandardLibrary
                && parameters.len() == 2 && parameters.iter().chain(std::iter::once(return_type))
                    .all(|ty| integer(ty, zincite_lint::Instantiation::Parameter))
    )));
    let partial_result = analyze_model(&partial_context, &selected());
    assert!(matches!(
        partial_result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let wrapper_start = partial.find("redundant_constraint((1 div 0)=0)").unwrap();
    assert!(
        partial_result
            .limitations
            .iter()
            .any(|limit| limit.location.range.start <= wrapper_start
                && wrapper_start < limit.location.range.end
                && limit
                    .message
                    .contains("output dependency operator identity or partiality is unsupported")),
        "{:?}",
        partial_result.limitations
    );
    std::fs::remove_dir_all(dir).unwrap();
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
    // Retain the compiler-checked Bool/Enum source chain instead of reducing it to Int.
    let sparse = concat!(
        "enum Feature = {F, C};\n",
        "enum FeatureX = anon_enum(card(Feature) + 1);\n",
        "Feature: class = C;\n",
        "FeatureX: dummy = max(FeatureX);\n",
        "FeatureX: classx = to_enum(FeatureX, class);\n",
        "int: n = 2;\n",
        "int: rows = 2;\n",
        "set of int: Nodes = 1..n;\n",
        "set of int: Nodes0 = 0..n;\n",
        "set of int: Rows = 1..rows;\n",
        "array[Nodes] of var bool: sign;\n",
        "array[Nodes] of var FeatureX: feature;\n",
        "array[Nodes, Rows] of var bool: valid;\n",
        "array[Rows] of var bool: mistakes;\n",
        "array[Nodes0] of var bool: leaf = array1d(Nodes0,\n",
        "  [if j in Nodes then feature[j] = classx else true endif | j in Nodes0]);\n",
        "array[Nodes] of var bool: unused = [feature[j] = dummy | j in Nodes];\n",
        "var 1..n: used = n - sum(unused);\n",
        "var 0..rows: misclassified = sum(mistakes);\n",
        "constraint forall(i in Rows)(valid[1,i]);\n",
        "solve :: int_search([if j = 1 then feature[i] else sign[i] endif |\n",
        "                    i in Nodes, j in 1..2], input_order, indomain_min)\n",
        "  minimize used + misclassified;\n",
    );
    let (dir, _) = model("sparse-bool-enum-source-chain", sparse, "");
    let sparse_core = CORE
        .replace(
            "function var bool: forall(array[int] of var opt bool: body);\n",
            "function var bool: forall(array[$T] of var bool: body);\n",
        )
        .replace(
            "function var int: sum(array[int] of var int: body);",
            "function var int: sum(array[$T] of var int: body);",
        );
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!(
            "{sparse_core}{}",
            concat!(
                "function int: '+'(int: left,int: right);\n",
                "function var int: '-'(var int: left,var int: right);\n",
                "function int: card(set of $T: values);\n",
                "function $$E: max(set of $$E: values);\n",
                "function $$E: to_enum(set of $$E: values,int: value);\n",
                "function bool: 'in'(int: value,set of int: choices);\n",
                "function ann: int_search(array[$X] of var $$E: x,ann: select,ann: choice)=int_search(x,select,choice,complete);\n",
            )
        ),
    )
    .unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    for written in ["sum(unused)", "sum(mistakes)"] {
        let start = sparse.find(written).unwrap();
        let call = calls
            .calls
            .iter()
            .find(|call| call.file == 0 && call.location.range.start == start)
            .unwrap();
        assert!(
            matches!(&call.outcome,
                zincite_lint::CallOutcome::Resolved { declaration, parameters, return_type }
                    if context.files[bindings.declarations[declaration.0].file].kind
                        == zincite_lint::SourceKind::StandardLibrary
                        && bindings.declarations[declaration.0].name == "sum"
                        && parameters.len() == 1
                        && !parameters[0].optional
                        && return_type.kind == zincite_lint::TypeKind::Int
                        && return_type.instantiation == zincite_lint::Instantiation::Decision
                        && !return_type.optional
                        && matches!(&parameters[0].kind,
                            zincite_lint::TypeKind::Array { indices, element }
                                if indices.len() == 1 && !element.optional
                                    && element.kind == zincite_lint::TypeKind::Int
                                    && element.instantiation == zincite_lint::Instantiation::Decision)
            ),
            "{written}: {:?}",
            call.outcome
        );
        let argument = start + "sum(".len()..start + written.len() - 1;
        assert!(
            calls.expressions.iter().any(|expression| {
                expression.file == 0
                    && expression.location.range == argument
                    && !expression.ty.optional
                    && matches!(&expression.ty.kind,
                    zincite_lint::TypeKind::Array { indices, element }
                        if indices.len() == 1 && !element.optional
                            && element.kind == zincite_lint::TypeKind::Bool
                            && element.instantiation == zincite_lint::Instantiation::Decision)
            }),
            "{written}: the actual Bool elements must remain Bool"
        );
    }
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    let search =
        resolve_search_coverage(&context, &bindings, &calls, &inst, &domains, &definitions);
    assert_eq!(search.root_state, ModelRootState::Complete);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    for name in ["leaf", "unused", "used", "misclassified"] {
        let target = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == name)
            .unwrap()
            .id;
        assert_eq!(
            search.declarations[target.0].coverage,
            SearchCoverage::Unknown,
            "{name}"
        );
        assert!(
            !definitions
                .definitions
                .iter()
                .chain(&callable.definitions)
                .any(|definition| definition.target == target
                    && definition.safety == DefinitionSafety::Supported)
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "valid"),
        SearchCoverage::Uncovered
    );
    assert!(matches!(
        analyze_model(&context, &selected()).rules[0].outcome,
        RuleOutcome::Completed
    ));
    // Each new inspection path retains a concrete partiality/source veto.
    for (name, outside, anchor) in [
        (
            "enum-ordinal",
            sparse.replace("to_enum(FeatureX, class)", "to_enum(FeatureX, 0)"),
            "array[Nodes0] of var bool: leaf",
        ),
        (
            "enum-count-overflow",
            sparse.replace(
                "card(Feature) + 1",
                "card(Feature) + (9223372036854775807 + 1)",
            ),
            "array[Nodes0] of var bool: leaf",
        ),
        (
            "enum-selection",
            sparse.replace(
                "array[Nodes] of var FeatureX: feature;",
                "array[1..1] of var FeatureX: feature;\nvar FeatureX: selected = feature[2];",
            ),
            "var FeatureX: selected",
        ),
        (
            "Boolean-selector-overflow",
            sparse.replace("valid[1,i]", "valid[1,(9223372036854775807 + 1)]"),
            "valid[1,(9223372036854775807 + 1)]",
        ),
    ] {
        std::fs::write(dir.join("root.mzn"), &outside).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        let result = analyze_model(&context, &selected());
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            "{name}: {:?}",
            result.limitations
        );
        let located = outside.find(anchor).unwrap();
        assert!(
            result
                .limitations
                .iter()
                .any(|limit| limit.location.range.start <= located
                    && located < limit.location.range.end),
            "{name}: {:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    // Inspect all three DecisionBool row lets without defining their arrays.
    {
        use zincite_lint::{CallOutcome, Instantiation, SourceKind, TypeInst, TypeKind};
        let row_relations = r#"include "globals.mzn";

% Data rows supply symbolic integer selectors; their values are not lint proofs.
set of int: Rows;
set of int: Items;
array[Rows, 1..3] of int: triples;
array[Rows, 1..4] of int: ranges;
array[Items] of var bool: enabled;
array[Items] of var 0..3: position;
array[Items] of var Items: successor;

% An enabled item requires its two positions to coincide.
constraint :: "Enabled positions coincide"
    forall(r in index_set_1of2(triples))(
        let { int: item = triples[r, 1],
              int: left = triples[r, 2],
              int: right = triples[r, 3] } in
        enabled[item] -> position[left] = position[right]
    );

% Enablement constrains a successor without functionally defining the arrays.
constraint :: "Enabled successor relation"
    forall(r in index_set_1of2(triples))(
        let { int: item = triples[r, 1],
              int: left = triples[r, 2],
              int: right = triples[r, 3] } in
        position[item] in {left, 0} /\
        (enabled[item] -> successor[left] = right)
    );

% Enabled positions obey the supplied row bounds.
constraint :: "Enabled position interval"
    forall(r in index_set_1of2(ranges))(
        let { int: item = ranges[r, 1], int: left = ranges[r, 2] } in
        enabled[item] -> position[left] in ranges[r, 3]..ranges[r, 4]
    );

solve :: bool_search(enabled, input_order, indomain_min, complete) satisfy;
"#;
        // Establish the include tree before loading a source that includes globals.
        let (dir, _) = model("parameter-row-local-relations", "solve satisfy;", "");
        std::fs::write(dir.join("root.mzn"), row_relations).unwrap();
        std::fs::write(dir.join("library/std/globals.mzn"), "").unwrap();
        let core = CORE
            .replace(
                "function var bool: '='(any $T: left,any $T: right); function bool: '='($T: left,$T: right);",
                "function var bool: '='(var int: left,var int: right); function bool: '='(int: left,int: right);",
            )
            .replace(
                "function var bool: forall(array[int] of var opt bool: body);",
                "function var bool: forall(array[$T] of var bool: body);",
            );
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!("{core}function set of $$E: index_set_1of2(array[$$E,$$F] of any $U: x); function int: '+'(int: left,int: right);\n"),
        ).unwrap();
        let options = ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        };
        let context = load_model(dir.join("root.mzn"), &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let scalar = |ty: &TypeInst, kind: TypeKind, instantiation| {
            !ty.optional && ty.kind == kind && ty.instantiation == instantiation
        };
        let integer_set = |ty: &TypeInst| {
            !ty.optional
                && ty.instantiation == Instantiation::Parameter
                && matches!(&ty.kind, TypeKind::Set(element)
                    if scalar(element, TypeKind::Int, Instantiation::Parameter))
        };
        let array = |ty: &TypeInst, rank, kind, instantiation| {
            !ty.optional
                && ty.instantiation == instantiation
                && matches!(&ty.kind, TypeKind::Array { indices, element }
                    if indices.len() == rank
                        && indices.iter().all(|axis| scalar(axis, TypeKind::Int, Instantiation::Parameter))
                        && scalar(element, kind, instantiation))
        };
        // A tuple failure reports the actually selected declaration and concrete types.
        for call in calls.calls.iter().filter(|call| call.file == 0) {
            let CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &call.outcome
            else {
                panic!("{:?}", call);
            };
            let owner = &bindings.declarations[declaration.0];
            let standard = &context.files[owner.file];
            assert!(
                standard.kind == SourceKind::StandardLibrary
                    && standard.implicit
                    && owner.name == call.name,
                "{:?}",
                call
            );
            let tuple = match call.name.as_str() {
                ".." => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Int, Instantiation::Parameter))
                        && integer_set(return_type)
                }
                "index_set_1of2" => {
                    parameters.len() == 1
                        && array(&parameters[0], 2, TypeKind::Int, Instantiation::Parameter)
                        && integer_set(return_type)
                }
                "forall" => {
                    parameters.len() == 1
                        && array(&parameters[0], 1, TypeKind::Bool, Instantiation::Decision)
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "=" => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Int, Instantiation::Decision))
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "->" | "/\\" => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Bool, Instantiation::Decision))
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "in" => {
                    parameters.len() == 2
                        && scalar(&parameters[0], TypeKind::Int, Instantiation::Decision)
                        && integer_set(&parameters[1])
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "bool_search" => {
                    parameters.len() == 4
                        && array(&parameters[0], 1, TypeKind::Bool, Instantiation::Decision)
                        && parameters[1..]
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Annotation, Instantiation::Parameter))
                        && scalar(return_type, TypeKind::Annotation, Instantiation::Parameter)
                }
                _ => panic!("unexpected row relation call: {:?}", call),
            };
            assert!(tuple, "{:?}", call);
        }
        for (name, count) in [("index_set_1of2", 3), ("forall", 3), ("->", 3), ("in", 2)] {
            assert_eq!(
                calls
                    .calls
                    .iter()
                    .filter(|call| call.file == 0 && call.name == name)
                    .count(),
                count,
                "{name}"
            );
        }
        for name in ["triples", "ranges"] {
            let id = bindings
                .declarations
                .iter()
                .find(|declaration| declaration.top_level && declaration.name == name)
                .unwrap()
                .id;
            assert!(
                array(
                    &calls.declarations[id.0].ty,
                    2,
                    TypeKind::Int,
                    Instantiation::Parameter
                ),
                "{name}"
            );
        }
        for declaration in bindings.declarations.iter().filter(|declaration| {
            declaration.file == 0 && declaration.role == zincite_lint::DeclarationRole::Local
        }) {
            assert!(
                scalar(
                    &calls.declarations[declaration.id.0].ty,
                    TypeKind::Int,
                    Instantiation::Parameter
                ),
                "{:?}",
                declaration
            );
        }
        // Type facts use each retained CST range, including owned trivia.
        let parsed = &context.files[0].parsed;
        assert_eq!(parsed.source(), row_relations);
        let mut pending = vec![parsed.tree()];
        let mut nodes = Vec::new();
        while let Some(node) = pending.pop() {
            nodes.push(node);
            pending.extend(node.child_nodes());
        }
        for (written, node_kind, count, kind, instantiation) in [
            (
                "triples[r, 1]",
                zincite_syntax::NodeKind::ArrayAccessExpression,
                2,
                TypeKind::Int,
                Instantiation::Parameter,
            ),
            (
                "ranges[r, 3]..ranges[r, 4]",
                zincite_syntax::NodeKind::RangeExpression,
                1,
                TypeKind::Set(Box::new(TypeInst {
                    instantiation: Instantiation::Parameter,
                    optional: false,
                    kind: TypeKind::Int,
                })),
                Instantiation::Parameter,
            ),
            (
                "enabled[item]",
                zincite_syntax::NodeKind::ArrayAccessExpression,
                3,
                TypeKind::Bool,
                Instantiation::Decision,
            ),
        ] {
            assert_eq!(
                row_relations.match_indices(written).count(),
                count,
                "{written}"
            );
            let matching: Vec<_> = nodes
                .iter()
                .copied()
                .filter(|node| {
                    node.kind() == node_kind && parsed.source()[node.range()].trim() == written
                })
                .collect();
            assert_eq!(matching.len(), count, "{written}: retained node count");
            for node in matching {
                assert_eq!(parsed.source()[node.range()].trim(), written);
                let range = context.files[0].location(node.range()).range;
                let Some(expression) = calls
                    .expressions
                    .iter()
                    .find(|expression| expression.file == 0 && expression.location.range == range)
                else {
                    let nearby: Vec<_> = calls
                        .expressions
                        .iter()
                        .filter(|expression| {
                            expression.file == 0
                                && expression.location.range.start <= range.end
                                && range.start <= expression.location.range.end
                        })
                        .collect();
                    panic!(
                        "missing type for {written}, {:?} {:?}: {:?}",
                        node.kind(),
                        range,
                        nearby
                    );
                };
                assert!(
                    scalar(&expression.ty, kind.clone(), instantiation),
                    "{written}: {:?}",
                    expression
                );
            }
        }
        // The first behavior assertion reaches the full owning lets after all tuple checks.
        let result = analyze_model(&context, &selected());
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Completed),
            "{:?}",
            result.limitations
        );
        assert!(result.limitations.is_empty(), "{:?}", result.limitations);
        let (_, search) = facts(&context);
        assert_eq!(search.root_state, ModelRootState::Complete);
        assert_eq!(
            coverage(&bindings, &search, "enabled"),
            SearchCoverage::WholeArray
        );
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
        for name in ["position", "successor"] {
            let target = bindings
                .declarations
                .iter()
                .find(|declaration| declaration.top_level && declaration.name == name)
                .unwrap()
                .id;
            assert!(
                matches!(
                    coverage(&bindings, &search, name),
                    SearchCoverage::Uncovered | SearchCoverage::Unknown
                ),
                "{name}"
            );
            assert!(
                !search
                    .searched
                    .iter()
                    .any(|value| value.declaration == Some(target)
                        && matches!(
                            value.coverage,
                            SearchCoverage::Scalar | SearchCoverage::WholeArray
                        )),
                "{name}"
            );
            assert!(
                !callable
                    .definitions
                    .iter()
                    .any(|definition| definition.target == target),
                "{name}: {:?}",
                callable.definitions
            );
        }
        let enabled = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == "enabled")
            .unwrap()
            .id;
        assert_eq!(search.searched.len(), 1);
        assert_eq!(search.searched[0].declaration, Some(enabled));
        assert_eq!(search.searched[0].coverage, SearchCoverage::WholeArray);

        // A closed source-domain error must survive symbolic row selectors.
        let overflow = row_relations.replace(
            "array[Rows, 1..3] of int: triples;",
            "array[Rows, 1..3] of 0..(9223372036854775807 + 1): triples;",
        );
        std::fs::write(dir.join("root.mzn"), &overflow).unwrap();
        let context = load_model(dir.join("root.mzn"), &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        let bindings = resolve_bindings(&context);
        let domains = resolve_domains(&context, &bindings);
        let triples = bindings
            .declarations
            .iter()
            .find(|declaration| declaration.top_level && declaration.name == "triples")
            .unwrap()
            .id;
        assert!(matches!(&domains.declarations[triples.0].domain,
            zincite_lint::Domain::Array { element, .. } if element.numeric_minimum().is_err()));
        let result = analyze_model(&context, &selected());
        assert!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            "{:?}",
            result.limitations
        );
        let selected_source = overflow.find("triples[r, 1]").unwrap();
        assert!(
            result
                .limitations
                .iter()
                .any(|limit| limit.location.path == dir.join("root.mzn")
                    && limit.location.range.start <= selected_source
                    && selected_source < limit.location.range.end),
            "{:?}",
            result.limitations
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn callable_outputs_keep_selected_direction_defaults_captures_and_array_extent() {
    let included = concat!(
        "predicate count_like(array[int] of var int: xs,var int: needle,var int: n);\n",
        "predicate count_like(array[int] of var int: xs,var int: needle,var int: n)=n=sum(i in index_set(xs))(xs[i]==needle);\n",
        "predicate wrapper(array[int] of var int: xs,var int: n,var int: needle=global_needle)=count_like(n:n,xs:xs,needle:needle);\n",
        "predicate copy_value(var int: source,var int: result)=result=source; predicate enforce(var bool: condition)=condition;\n",
        "predicate recursive_a(var int: source,var int: result)=result=source /\\ recursive_b(source,result);\n",
        "predicate recursive_b(var int: source,var int: result)=recursive_a(source,result);\n",
        "predicate loop_a(var int: source,var int: result)=loop_b(source,result);\n",
        "predicate loop_b(var int: source,var int: result)=loop_a(source,result);\n",
        "predicate named_output(var int: result,var int: input)=true;\n",
        "predicate competing(var int: source,var int: result)=result=source; predicate competing(var int: source,var int: result)=true;\n",
        "predicate copy_array(array[S] of var int: xs,array[S] of var int: ys)=forall(i in S)(ys[i]=xs[i]);\n",
        "predicate filtered_array(array[S] of var int: xs,array[S] of var int: ys)=forall(i in S where true)(ys[i]=xs[i]);\n",
        "function var int: opaque(var int: x)=x; predicate partly_supported(var int: source,var int: good,var int: bad)=good=source /\\ bad=opaque(source);\n",
        "predicate optional_output(var opt int: source,var opt int: result)=result=source;\n",
        "predicate reciprocal(array[int] of var int: a,array[int] of var int: b)=forall(i in index_set(a))(a[i] in index_set(b) /\\ b[a[i]]=i) /\\ forall(j in index_set(b))(b[j] in index_set(a) /\\ a[b[j]]=j);\n",
        "predicate half_reciprocal(array[int] of var int: a,array[int] of var int: b)=forall(i in index_set(a))(a[i] in index_set(b) /\\ b[a[i]]=i);\n",
        "predicate filtered_reciprocal(array[int] of var int: a,array[int] of var int: b)=forall(i in index_set(a) where true)(a[i] in index_set(b) /\\ b[a[i]]=i) /\\ forall(j in index_set(b))(b[j] in index_set(a) /\\ a[b[j]]=j);\n",
    );
    let source = concat!(
        "\u{feff}% é\r\ninclude \"included.mzn\"; int: global_needle; array[1..2,1..2] of var 0..3: values;\n",
        "var 0..4: counted; var int: need_capture; var int: unsearched;\n",
        "set of int: S; array[S] of var int: array_input; array[S] of var int: array_output; array[S] of var int: filtered_output;\n",
        "array[S] of var int: converted_output; array[S] of var int: converted_filtered;\n",
        "array[S] of var opt int: converted_optional;\n",
        "array[S] of var int: inverse_output; array[S] of var int: half_inverse; array[S] of var int: filtered_inverse; array[S] of var int: cycle_left; array[S] of var int: cycle_right;\n",
        "var int: misleading; var int: competing_output; var int: good; var int: bad; var int: forwarded; var opt int: optional_input; var opt int: optional_result;\n",
        "predicate capture_output(var int: out_value)=out_value=unsearched;\n",
        "var int: anchored; var int: unanchored; var int: conditional; array[1..2] of var int: partial; var bool: gate;\n",
        "constraint :: \"Count \\(1)\" wrapper(array1d(values),n:counted); constraint recursive_b(counted,anchored);\n",
        "constraint named_output(misleading,counted); constraint competing(counted,competing_output);\n",
        "constraint enforce(copy_value(counted,forwarded)); constraint partly_supported(counted,good,bad); constraint optional_output(optional_input,optional_result);\n",
        "constraint copy_array(array_input,array_output); constraint filtered_array(array_input,filtered_output);\n",
        "constraint copy_array(index2int(enum2int(array_input)),index2int(enum2int(converted_output)));\n",
        "constraint filtered_array(array_input,index2int(enum2int(converted_filtered)));\n",
        "constraint copy_array(array_input,index2int(enum2int(converted_optional)));\n",
        "constraint reciprocal(index2int(enum2int(array_input)),index2int(enum2int(inverse_output)));\n",
        "constraint half_reciprocal(array_input,index2int(enum2int(half_inverse))); constraint filtered_reciprocal(array_input,index2int(enum2int(filtered_inverse)));\n",
        "constraint reciprocal(cycle_left,cycle_right);\n",
        "constraint capture_output(need_capture); constraint loop_a(counted,unanchored);\n",
        "constraint gate -> copy_value(counted,conditional); constraint copy_value(counted,partial[1]);\n",
        "solve :: seq_search([int_search(array1d(values),input_order,indomain_min,complete),int_search(array_input,input_order,indomain_min,complete),bool_search([gate],input_order,indomain_min,complete)]) satisfy;\n"
    );
    let (dir, context) = model("callable", source, included);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let outputs =
        zincite_lint::resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    assert!(
        outputs
            .outputs
            .iter()
            .any(|g| bindings.declarations[g.callable.0].name == "count_like"
                && bindings.declarations[g.target.0].name == "n"
                && g.dependencies
                    .iter()
                    .all(|id| bindings.declarations[id.0].name != "i")),
        "{:?}",
        outputs
    );
    let counted = bindings
        .declarations
        .iter()
        .find(|d| d.name == "counted")
        .unwrap()
        .id;
    let definition = outputs
        .definitions
        .iter()
        .find(|d| d.target == counted)
        .unwrap();
    assert_eq!(
        &source[definition.location.range.clone()],
        "wrapper(array1d(values),n:counted)"
    );
    let (_, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "counted"),
        SearchCoverage::Scalar,
        "{:?}",
        search.limitations
    );
    assert_eq!(
        coverage(&bindings, &search, "anchored"),
        SearchCoverage::Scalar
    );
    assert_eq!(
        coverage(&bindings, &search, "unanchored"),
        SearchCoverage::Unknown
    );
    assert_eq!(
        coverage(&bindings, &search, "need_capture"),
        SearchCoverage::Uncovered
    );
    assert_eq!(
        coverage(&bindings, &search, "conditional"),
        SearchCoverage::Uncovered
    );
    assert_ne!(
        coverage(&bindings, &search, "partial"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "array_output"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "converted_output"),
        SearchCoverage::WholeArray,
        "{:?}",
        search.limitations
    );
    assert_ne!(
        coverage(&bindings, &search, "converted_filtered"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "inverse_output"),
        SearchCoverage::WholeArray,
        "{:?}",
        search.limitations
    );
    for name in [
        "half_inverse",
        "filtered_inverse",
        "cycle_left",
        "cycle_right",
    ] {
        assert_ne!(
            coverage(&bindings, &search, name),
            SearchCoverage::WholeArray
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "misleading"),
        SearchCoverage::Uncovered
    );
    assert_eq!(coverage(&bindings, &search, "good"), SearchCoverage::Scalar);
    assert_eq!(
        coverage(&bindings, &search, "forwarded"),
        SearchCoverage::Scalar
    );
    assert_eq!(
        coverage(&bindings, &search, "filtered_output"),
        SearchCoverage::Uncovered
    );
    assert!(
        outputs
            .outputs
            .iter()
            .filter(|g| { bindings.declarations[g.callable.0].name == "filtered_array" })
            .all(|g| g.coverage != DefinitionCoverage::WholeArray)
    );
    for name in [
        "bad",
        "competing_output",
        "optional_result",
        "converted_optional",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Unknown,
            "{name}: {:?}",
            search.limitations
        );
    }
    assert!(
        search
            .limitations
            .iter()
            .all(|l| !l.message.contains("direct definitions only"))
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();

    let shadowed = format!(
        "{source}\nfunction array[int] of var int: index2int(array[int] of var int: xs)=[xs[1],xs[1]];\n"
    );
    let (dir, context) = model("shadowed-conversion", &shadowed, included);
    let (bindings, search) = facts(&context);
    assert_eq!(
        coverage(&bindings, &search, "converted_output"),
        SearchCoverage::Unknown
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn total_controls_and_filtered_relations_separate_outputs_from_unavailable_bodies() {
    let included = concat!(
        "predicate branch(bool: take,int: input,var int: result)=if take then result=input else result=input+1 endif;\n",
        "predicate specialized(var bool: take,int: input,var int: result)=if take then result=input else result=input+1 endif;\n",
        "predicate specialized_default(int: input,var int: result,var bool: take=gate)=specialized(take,input,result);\n",
        "predicate specialized_relation(var int: input,var int: result)=input>=0;\n",
        "predicate scalar_bounds(var int: input,var int: result)=if is_fixed(input) then result=fix(input)+lb(input)+ub(input) else result=0 endif;\n",
        "predicate array_bounds(array[int] of var int: values,var int: result)=if length(values)=0 then result=0 else result=lb_array(values)+ub_array(values) endif;\n",
        "predicate raw_bounds(array[int] of var int: values,var int: result)=result=lb_array(values);\n",
        "predicate minimum_index(array[int] of var int: values,var int: result)=if length(values)=0 then result=0 else let { int: chosen=arg_min(arrayXd(values,[lb(v)|v in values])); } in result=chosen endif;\n",
        "predicate raw_minimum_index(array[int] of int: values,var int: result)=let { int: chosen=arg_min(arrayXd(values,[lb(v)|v in values])); } in result=chosen;\n",
        "predicate filtered_minimum_index(array[int] of int: values,var int: result)=if length(values)=0 then result=0 else let { int: chosen=arg_min(arrayXd(values,[lb(v)|v in values where v>0])); } in result=chosen endif;\n",
        "predicate other_minimum_index(array[int] of int: values,array[int] of int: other,var int: result)=if length(values)=0 then result=0 else let { int: chosen=arg_min(arrayXd(values,[lb(v)|v in other])); } in result=chosen endif;\n",
        "predicate parameter_value_guard(var int: input,var int: result)=if let { int: lower_bound=lb(input); int: fixed_value=fix(input); 0..3: domain_value=0; } in is_fixed(input) /\\ fixed_value>=lower_bound then result=input else result=input endif;\n",
        "predicate partial_value_domain(array[int] of int: bounds,int: position,var int: result)=if let { 0..bounds[position]: local_value=0; } in true then result=0 else result=0 endif;\n",
        "predicate partial_local_domain(array[int] of int: bounds,int: position,var int: result)=let { 0..bounds[position]: local_value=0; } in result=0;\n",
        "function int: opaque_parameter(int: input)=input; predicate unused_value_guard(int: input,var int: result)=if let { int: unused_value=opaque_parameter(input); } in true then result=input else result=input endif;\n",
        "predicate decision_value_guard(var int: result)=if let { var bool: private_value; } in true then result=0 else result=0 endif;\n",
        "predicate aligned_bounds(array[int] of var int: start,array[int] of var int: values,var int: result)=assert(index_set(start)=index_set(values),\"aligned indices\") /\\ if length(start)>=1 then result=lb_array(values)+ub_array(values) else result=0 endif;\n",
        "predicate nonasserted_alignment(array[int] of var int: start,array[int] of var int: values,var int: result)=index_set(start)=index_set(values) /\\ if length(start)>=1 then result=lb_array(values) else result=0 endif;\n",
        "predicate reified_alignment(bool: gate,array[int] of var int: start,array[int] of var int: values,var int: result)=(gate -> assert(index_set(start)=index_set(values),\"aligned indices\")) /\\ if length(start)>=1 then result=lb_array(values) else result=0 endif;\n",
        "predicate wrong_alignment(array[int] of var int: start,array[int] of var int: values,array[int] of var int: other,var int: result)=assert(index_set(start)=index_set(other),\"other indices\") /\\ if length(start)>=1 then result=lb_array(values) else result=0 endif;\n",
        "predicate local(int: input,var int: result)=let { int: copy=input; } in result=copy;\n",
        "predicate relation(var int: left,var int: right)=left!=right;\n",
        "predicate private_constant(var int: result)=let { var bool: private_value; constraint private_value; } in result=0;\n",
        "predicate private_opaque(var bool: result)=let { var bool: private_value; constraint opaque_relation(private_value); } in result=true;\n",
        "predicate opaque_relation(var bool: value);\n",
        "predicate nested_relation(array[int,int] of int: positions,array[int] of var int: values,int: row,var bool: flag)=(flag<->values[positions[row,1]]<=values[positions[row,2]]) /\\ (values[positions[row,1]]+bool2int(not flag)<=values[positions[row,2]]+positions[row,3]);\n",
        "predicate nested_raw(array[int,int] of int: positions,array[int] of var int: values,int: row,var int: result)=result=values[positions[row,1]];\n",
        "predicate opaque_equivalence(var bool: result)=result<->opaque_relation(result);\n",
        "predicate boolean_member(array[int] of var bool: values,int: position)=values[position];\n",
        "predicate boolean_value_relation(array[int] of var bool: values,int: position,var bool: result)=result=values[position];\n",
        "predicate private_array_chain(int: last,var bool: left,var bool: right)=let { array[0..last+1] of var bool: private_values; constraint forall(i in 0..last)(private_values[i]=(left<right \\/ private_values[i+1])); } in private_values[0];\n",
        "predicate boolean_guard(array[int] of bool: values,int: position,var int: result)=if values[position] then result=0 else result=1 endif;\n",
        "predicate boolean_opaque_index(array[int] of var bool: values,int: position,var bool: result)=result=values[opaque_count(position)];\n",
        "predicate private_array_relation(int: last,var bool: left,var bool: right)=let { array[0..last] of var bool: private_values; constraint forall(i in index_set(private_values))(private_values[i]=(left<right)); } in true;\n",
        "predicate private_array_opaque(int: last,var int: result)=let { array[0..opaque_count(last)] of var bool: private_values; } in result=0;\n",
        "function int: opaque_count(int: last);\n",
        "predicate private_relation(var bool: left,var bool: right)=let { var bool: private_value; constraint private_value=(left<right); } in private_value;\n",
        "predicate pattern_inspection(array[int] of var bool: values,var bool: result)=let { int: pattern_size=max(index_set(values)); array[0..pattern_size+1] of var bool: pattern_choices; } in (pattern_size>=0) /\\ (result=true) /\\ forall(i in 0..pattern_size)(pattern_choices[i]=values[i]);\n",
        "predicate pattern_direct(array[int] of var bool: x,array[int] of var bool: y,var bool: result)=let { array[1..max(index_set(x))+1] of var bool: direct_left; array[1..max(index_set(y))+1] of var bool: direct_right; } in (result=true) /\\ forall(i in index_set(direct_left))(direct_left[i]=direct_right[i]);\n",
        "predicate table_inspection(array[int] of var int: xs,array[int,int] of int: tuples,var bool: result)=let { int: l=min(index_set(xs)); int: u=max(index_set(xs)); int: lt=min(index_set_1of2(tuples)); int: ut=max(index_set_1of2(tuples)); var lt..ut: i; array[l..u,lt..ut] of int: t_transposed=array2d(l..u,lt..ut,[tuples[k,j]|j in l..u,k in lt..ut]); } in (result=true) /\\ forall(j in l..u)(t_transposed[j,i]=xs[j]);\n",
        "predicate table_filtered(array[int] of var int: xs,array[int,int] of int: tuples,var bool: result)=let { int: l=min(index_set(xs)); int: u=max(index_set(xs)); int: lt=min(index_set_1of2(tuples)); int: ut=max(index_set_1of2(tuples)); var lt..ut: i; array[l..u,lt..ut] of int: t_transposed=array2d(l..u,lt..ut,[tuples[k,j]|j in l..u,k in lt..ut where j>l]); } in (result=true) /\\ forall(j in l..u)(t_transposed[j,i]=xs[j]);\n",
        "predicate pattern_optional(array[int] of var bool: values,var opt bool: maybe,var bool: result)=let { int: pattern_optional_size=max(index_set(values)); } in result=maybe;\n",
        "predicate promised_relation(var int: left,var int: right) :: promise_total = left!=right;\n",
        "predicate promised_partial(var int: result) :: promise_total = result=1 div 0;\n",
        "predicate presence(opt int: cap,var int: result)=if absent(cap) then result=0 else result=1 endif;\n",
        "predicate value_guard(array[int] of int: lower,var int: result)=if forall(l in array1d(lower))(l<=0) then result=0 else result=1 endif;\n",
        "predicate existential_guard(array[int] of int: values,int: input,var int: result)=if exists(i in index_set(values))(values[i]>0) then result=input else result=input endif;\n",
        "predicate collected(array[int] of int: values,int: input,var int: result)=result=input /\\ forall(i in index_set(values))(let { set of int: selected={values[i]|j in 1..1 where j>0}; int: total=sum([0]++[j|j in selected]); } in if total>=0 /\\ exists(j in selected)(j>0) then true else true endif);\n",
        "predicate collected_raw(array[int] of int: values,set of int: other,var int: result)=let { int: total=sum([0]++[values[i]|i in other]); } in result=0;\n",
        "predicate collected_opaque(set of int: values,var int: result)=let { set of int: selected={i|i in values where opaque_relation(true)}; } in result=0;\n",
        "set of int: Rows; set of int: Columns; set of int: OtherRows; array[Rows,Columns] of int: matrix;\n",
        "predicate subset_values(int: input,var int: result)=result=input /\\ forall(column in Columns)(let { set of int: selected={row|row in Rows where row>0}; int: total=sum([0]++[matrix[row,column]|row in selected]); } in if total>=0 then true else true endif);\n",
        "predicate subset_wrong(var int: result)=forall(column in Columns)(let { set of int: selected={row|row in OtherRows where row>0}; int: total=sum([0]++[matrix[row,column]|row in selected]); } in result=total);\n",
        "predicate subset_shift(var int: result)=forall(column in Columns)(let { set of int: selected={row+1|row in Rows where row>0}; int: total=sum([0]++[matrix[row,column]|row in selected]); } in result=total);\n",
        "predicate index_guard(array[int] of int: bounds,int: position,var int: result)=if bounds[position]>=0 then result=0 else result=1 endif;\n",
        "predicate counted(array[int] of var int: xs,var int: result)=result=count(x in xs)(x=0);\n",
        "predicate range_guard(array[int] of int: bounds,int: position,var int: result)=if position in bounds[position]..bounds[position] then result=0 else result=1 endif;\n",
        "predicate raw_range(array[int] of int: bounds,int: position,var int: result)=let { set of int: values=bounds[position]..bounds[position]; } in result=0;\n",
        "predicate raw_index(array[int] of int: bounds,int: position,var int: result)=result=bounds[position];\n",
        "predicate singleton_pair(array[int] of var bool: xs,array[int] of var bool: ys)=if length(xs)=1 /\\ length(ys)=1 then xs[min(index_set(xs))]<ys[min(index_set(ys))] else true endif;\n",
        "predicate raw_min(array[int] of var bool: xs,var bool: result)=result=xs[min(index_set(xs))];\n",
        "predicate extrema(array[int] of var int: xs,array[int] of var int: ys,var int: result)=if length(xs)=0 then result=0 elseif length(ys)=0 then result=0 else let { int: lx=min(index_set(xs)); int: ux=max(index_set(xs)); int: ly=min(index_set(ys)); int: uy=max(index_set(ys)); int: size=min(ux-lx,uy-ly); } in result=size endif;\n",
        "predicate compound_bounds(bool: gate,array[int] of var bool: xs,var bool: result)=if length(xs)=0 /\\ gate then result=false else result=xs[min(index_set(xs))] endif;\n",
        "predicate wrong_bounds(array[int] of var bool: xs,array[int] of var bool: ys,var bool: result)=if length(xs)=0 then result=false else result=ys[min(index_set(ys))] endif;\n",
        "predicate else_min(array[int] of var bool: xs,var bool: result)=if length(xs)=1 then result=false else result=xs[min(index_set(xs))] endif;\n",
        "function array[int] of var int: selected_values(array[int] of var int: first_values,array[int] of var int: second_values)=second_values;\n",
        "predicate asserted(bool: gate,array[int] of var int: xs,var int: result)=assert(gate,\"indexes \"++show_index_sets(xs),result=0);\n",
        "predicate asserted_true(int: input,var int: result)=assert(true,\"guard\",result=input);\n",
        "predicate asserted_partial(bool: gate,var int: result)=assert(gate,\"guard\",result=1 div 0);\n",
        "predicate asserted_false(var int: result)=assert(false,\"abort\",result=0);\n",
        "predicate filtered(int: input,var int: result)=forall(i in 1..1 where false)(result=input);\n",
        "predicate always(int: input,var int: result)=result=input;\n",
        "predicate filtered_call(int: input,var int: result)=forall(i in 1..1 where false)(always(input,result));\n",
        "predicate one_side(bool: take,int: input,var int: result)=if take then result=input else true endif;\n",
        "predicate alternate(bool: take,int: input,var int: left,var int: right)=if take then left=input else right=input endif;\n",
        "predicate decision(var bool: take,int: input,var int: result)=if take then result=input else result=input+1 endif;\n",
        "predicate partial_indices(array[int] of var int: ys,int: input)=forall(i in index_set(ys))(ys[i div 2]=input);\n",
        "predicate partial(bool: take,int: input,var int: result)=if take then result=input else result=1 div 0 endif;\n",
        "predicate pairs(array[int] of var int: xs)=forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]);\n",
        "predicate reflected(array[int] of var int: xs,set of int: excluded)=if forall(i in index_set(xs))(has_bounds(xs[i])) then let { set of int: D=dom_array(xs) diff excluded; } in pairs(xs) else pairs(xs) endif;\n",
        "predicate unguarded_bounds(array[int] of var int: xs,var int: result)=let { set of int: D=dom_array(xs); } in result=0;\n",
        "predicate filtered_bounds(bool: gate,array[int] of var int: xs,var int: result)=if forall(i in index_set(xs) where gate)(has_bounds(xs[i])) then let { set of int: D=dom_array(xs); } in result=0 else result=0 endif;\n",
        "predicate filtered_guard(bool: gate,array[int] of var int: xs,var int: result)=if forall(i in index_set(xs) where gate)(has_bounds(xs[i])) then if length(xs)=0 then result=0 else result=1 endif else result=1 endif;\n",
    );
    let source = concat!(
        "include \"included.mzn\"; bool: gate; int: input; var int: branched; var int: local_result;\n",
        "set of int: excluded; array[int] of int: lower; int: position; var int: index_guard_result; var int: range_guard_result; var int: counted_result; var int: value_guard_result; opt int: cap; var int: presence_result; var int: asserted_result; var int: asserted_true_result; var int: guarded_result; var int: filtered_result; var int: filtered_call_result; var int: one_sided; var int: alternate_left; var int: alternate_right; var int: left; var int: right; array[0..1] of var int: xs;\n",
        "constraint branch(gate,input,branched); constraint local(input,local_result) :: domain; constraint presence(cap,presence_result); constraint value_guard(lower,value_guard_result); constraint index_guard(lower,position,index_guard_result); constraint range_guard(lower,position,range_guard_result); constraint counted(xs,counted_result); constraint asserted(gate,xs,asserted_result); constraint asserted_true(input,asserted_true_result);\n",
        "var int: specialized_result; var int: specialized_default_result; constraint specialized(result:specialized_result,input:input,take:gate); constraint specialized_default(result:specialized_default_result,input:input);\n",
        "var int: scalar_bounds_result; var int: array_bounds_result; constraint scalar_bounds(input,scalar_bounds_result); constraint array_bounds(lower,array_bounds_result);\n",
        "var int: aligned_bounds_result; constraint aligned_bounds(xs,lower,aligned_bounds_result);\n",
        "var int: minimum_index_result; constraint minimum_index(lower,minimum_index_result);\n",
        "set of int: ScopedTasks; array[ScopedTasks] of var int: scoped_values; constraint forall(i in ScopedTasks)(let { var bool: scoped_flag; } in (scoped_flag<->scoped_values[i]<=0) /\\ bool2int(not scoped_flag)<=scoped_values[i]);\n",
        "var bool: scoped_result; constraint let { var bool: scoped_dependency; } in (scoped_dependency<->true) /\\ (scoped_result=scoped_dependency);\n",
        "array[ScopedTasks] of int: scoped_weights; constraint forall(p in 0..1)(let { var int: scoped_weighted=sum(c in ScopedTasks)(bool2int(scoped_values[c]>p)*scoped_weights[c]); } in scoped_weighted>=0);\n",
        "set of int: ScopedPeriods; array[ScopedPeriods,ScopedTasks] of var int: direct_flags; array[ScopedPeriods] of var int: direct_load; constraint forall(p in ScopedPeriods)(forall(c in ScopedTasks)(direct_flags[p,c]=bool2int(scoped_values[c]>p)) /\\ direct_load[p]=sum(c in ScopedTasks)(direct_flags[p,c]*scoped_weights[c]));\n",
        "int: weighted_size; set of int: DefinedTasks=1..weighted_size; array[DefinedTasks] of var int: defined_values; array[1..weighted_size] of int: defined_weights; constraint forall(p in 0..1)(let { var int: defined_weighted=sum(c in DefinedTasks)(bool2int(defined_values[c]>p)*defined_weights[c]); } in defined_weighted>=0);\n",
        "var int: parameter_value_guard_result; constraint parameter_value_guard(input,parameter_value_guard_result);\n",
        "array[1..1] of int: default_values; predicate default_array(var int: result,array[int] of var int: values=[default_values[k]|k in 1..1])=result=0; var int: array_default_result; constraint forall(i in 1..1)(default_array(array_default_result));\n",
        "array[int,int] of int: positions; constraint nested_relation(positions,xs,position,private_left);\n",
        "var int: existential_result; constraint existential_guard(lower,input,existential_result);\n",
        "var int: collected_result; constraint collected(lower,input,collected_result);\n",
        "var int: subset_result; constraint subset_values(input,subset_result);\n",
        "var bool: boolean_value_result; constraint boolean_value_relation(flags,position,boolean_value_result); constraint boolean_member(flags,position); constraint private_array_chain(input,private_left,private_right); var int: private_constant_result; constraint private_constant(private_constant_result); var bool: private_left; var bool: private_right; constraint private_relation(private_left,private_right); constraint private_array_relation(input,private_left,private_right); array[int] of var bool: flags; constraint singleton_pair(flags,flags); var int: extrema_result; constraint extrema(xs,xs,extrema_result);\n",
        "var bool: pattern_result; constraint pattern_inspection(flags,pattern_result);\n",
        "var bool: pattern_direct_result; constraint pattern_direct(flags,flags,pattern_direct_result);\n",
        "var bool: table_result; constraint table_inspection(xs,positions,table_result);\n",
        "constraint filtered(input,filtered_result); constraint filtered_call(input,filtered_call_result); constraint one_side(gate,input,one_sided); constraint alternate(gate,input,alternate_left,alternate_right); constraint relation(left,right); constraint promised_relation(left,right); constraint pairs(xs); constraint reflected(xs,excluded); constraint filtered_guard(gate,xs,guarded_result);\n",
        "solve :: seq_search([int_search(xs,input_order,indomain_min,complete),int_search(scoped_values,input_order,indomain_min,complete),int_search(defined_values,input_order,indomain_min,complete),bool_search(flags,input_order,indomain_min,complete),bool_search([private_left,private_right],input_order,indomain_min,complete)]) satisfy;\n",
    );
    let (dir, _) = model("total-controls", source, included);
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\nfunction int: '+'(int: left,int: right); function var bool: '!='(var int: left,var int: right); function bool: '<'(int: left,int: right); function bool: '<='(int: left,int: right); function bool: '>='(int: left,int: right); function bool: 'in'(int: left,set of int: right); function int: 'div'(int: left,int: right); test absent(opt $T: x); test occurs(opt $T: x); function bool: has_bounds(var int: x); function set of int: dom_array(array[int] of var int: xs); function set of int: 'diff'(set of int: left,set of int: right); function bool: forall(array[int] of bool: body); function bool: exists(array[int] of bool: body); function int: sum(array[int] of int: body); function array[int] of int: '++'(array[int] of int: left,array[int] of int: right); function bool: '>'(int: left,int: right); function var bool: assert(bool: b,string: msg,var bool: x); function bool: assert(bool: b,string: msg); function string: '++'(string: left,string: right); function string: show_index_sets(array[int] of var int: xs); function int: length(array[$U] of any $V: xs); annotation promise_total; annotation domain; function int: min(set of int: s); function int: max(set of int: s); function set of int: index_set_1of2(array[int,int] of any $V: values); function array[int,int] of int: array2d(set of int: rows,set of int: columns,array[int] of int: values); function int: min(int: left,int: right); function int: '-'(int: left,int: right); function bool: '/\\'(bool: left,bool: right); function var bool: '<'(var bool: left,var bool: right); function var int: count(array[int] of var bool: body); function var bool: '\\/'(var bool: left,var bool: right); function var bool: '<->'(var bool: left,var bool: right); function var bool: '<='(var int: left,var int: right); function var int: bool2int(var bool: value); function var bool: 'not'(var bool: value); function int: lb(var int: value); function int: ub(var int: value); function int: fix(var int: value); function bool: is_fixed(var int: value); function int: lb_array(array[int] of var int: values); function int: ub_array(array[int] of var int: values); function array[$T] of any $V: arrayXd(array[$T] of any $X: shape,array[$U] of any $V: values); function $$E: arg_min(array[$$E] of $$T: values); function var bool: '>'(var int: left,var int: right); function var bool: '>='(var int: left,var int: right); function var int: '*'(var int: left,var int: right);\n")).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&context);
    let calls = resolve_callables(&context, &bindings);
    let instantiations = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let outputs =
        resolve_callable_definitions(&context, &bindings, &calls, &instantiations, &domains);
    assert!(
        calls
            .calls
            .iter()
            .filter(|c| c.name == "specialized")
            .all(|c| {
                matches!(&c.outcome, zincite_lint::CallOutcome::Resolved { parameters, .. }
            if parameters[0].instantiation == zincite_lint::Instantiation::Decision)
            })
    );
    assert!(outputs.unavailable.is_empty(), "{:?}", outputs.unavailable);
    let scoped_flag = bindings
        .declarations
        .iter()
        .find(|d| d.name == "scoped_flag")
        .unwrap();
    assert_eq!(
        search.declarations[scoped_flag.id.0].coverage,
        SearchCoverage::Scalar
    );
    assert!(
        outputs
            .definitions
            .iter()
            .any(|d| d.target == scoped_flag.id
                && d.coverage == DefinitionCoverage::Scalar
                && d.safety == DefinitionSafety::Supported)
    );
    let scoped_dependency = bindings
        .declarations
        .iter()
        .find(|d| d.name == "scoped_dependency")
        .unwrap();
    assert_eq!(
        coverage(&bindings, &search, "scoped_result"),
        SearchCoverage::Scalar
    );
    assert!(outputs.definitions.iter().any(|d| {
        bindings.declarations[d.target.0].name == "scoped_result"
            && d.dependencies.contains(&scoped_dependency.id)
    }));
    for name in ["scoped_weighted", "defined_weighted"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(
            search.declarations[local.id.0].coverage,
            SearchCoverage::Scalar,
            "{name}"
        );
    }
    for name in ["direct_flags", "direct_load"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::WholeArray,
            "{name}: {:?}",
            search.limitations
        );
    }
    let unbounded = analyze_model(
        &context,
        &LintOptions::from_selection("unbounded-variable").unwrap(),
    );
    for name in ["scoped_weighted", "defined_weighted"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert!(
            unbounded
                .findings
                .iter()
                .all(|f| f.location != local.location)
                && unbounded
                    .limitations
                    .iter()
                    .all(|l| l.location != local.location),
            "{name}: {:?}",
            unbounded
        );
    }
    let combined = analyze_model(
        &context,
        &LintOptions::from_selection("search-coverage,unbounded-variable").unwrap(),
    );
    assert_eq!(
        unbounded.rules[0].outcome,
        combined
            .rules
            .iter()
            .find(|r| r.rule == zincite_lint::Rule::UnboundedVariable)
            .unwrap()
            .outcome
    );
    assert_eq!(
        unbounded
            .findings
            .iter()
            .map(|f| (&f.location, &f.message))
            .collect::<Vec<_>>(),
        combined
            .findings
            .iter()
            .filter(|f| f.rule == zincite_lint::Rule::UnboundedVariable)
            .map(|f| (&f.location, &f.message))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        unbounded
            .limitations
            .iter()
            .map(|l| (&l.location, &l.message))
            .collect::<Vec<_>>(),
        combined
            .limitations
            .iter()
            .filter(|l| l.message.starts_with("unbounded-variable:"))
            .map(|l| (&l.location, &l.message))
            .collect::<Vec<_>>()
    );
    let private_relation = bindings
        .declarations
        .iter()
        .find(|d| d.name == "private_relation")
        .unwrap();
    assert!(
        outputs
            .outputs
            .iter()
            .all(|o| o.callable != private_relation.id)
    );
    let private_array = bindings
        .declarations
        .iter()
        .find(|d| d.name == "private_array_relation")
        .unwrap();
    assert!(
        outputs
            .outputs
            .iter()
            .all(|o| o.callable != private_array.id)
    );
    for name in [
        "boolean_member",
        "boolean_value_relation",
        "private_array_chain",
        "nested_relation",
    ] {
        let callable = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert!(
            outputs.outputs.iter().all(|o| o.callable != callable.id),
            "{name}"
        );
    }
    for name in ["pattern_inspection", "pattern_direct", "table_inspection"] {
        let pattern = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        let private_ids: Vec<_> = bindings
            .declarations
            .iter()
            .filter(|d| d.file == pattern.file && d.item == pattern.item)
            .map(|d| d.id)
            .collect();
        assert!(outputs.outputs.iter().all(|o| o.callable != pattern.id));
        assert!(
            outputs
                .definitions
                .iter()
                .all(|d| !private_ids.contains(&d.target))
        );
        assert!(
            outputs
                .inspected_locals
                .iter()
                .all(|id| !private_ids.contains(id))
        );
    }
    let filtered_guard = bindings
        .declarations
        .iter()
        .find(|d| d.name == "filtered_guard")
        .unwrap();
    let guard = bindings
        .declarations
        .iter()
        .find(|d| {
            d.name == "gate" && d.file == filtered_guard.file && d.item == filtered_guard.item
        })
        .unwrap();
    assert!(
        outputs
            .outputs
            .iter()
            .filter(|o| o.callable == filtered_guard.id)
            .all(|o| o.dependencies.contains(&guard.id))
    );
    assert!(
        outputs
            .outputs
            .iter()
            .any(|o| o.callable == filtered_guard.id)
    );
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    for name in [
        "branched",
        "specialized_result",
        "specialized_default_result",
        "scalar_bounds_result",
        "array_bounds_result",
        "minimum_index_result",
        "aligned_bounds_result",
        "parameter_value_guard_result",
        "array_default_result",
        "local_result",
        "presence_result",
        "value_guard_result",
        "existential_result",
        "collected_result",
        "subset_result",
        "index_guard_result",
        "range_guard_result",
        "counted_result",
        "guarded_result",
        "extrema_result",
        "private_constant_result",
        "asserted_true_result",
        "asserted_result",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Scalar,
            "{name}"
        );
    }
    for name in [
        "pattern_result",
        "pattern_direct_result",
        "table_result",
        "boolean_value_result",
        "filtered_result",
        "filtered_call_result",
        "left",
        "right",
        "one_sided",
        "alternate_left",
        "alternate_right",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "xs"),
        SearchCoverage::WholeArray
    );
    assert!(matches!(
        analyze_model(&context, &selected()).rules[0].outcome,
        RuleOutcome::Completed
    ));
    std::fs::write(dir.join("root.mzn"), "include \"included.mzn\"; bool: gate; int: input; array[1..1] of var 0..1: first_values; array[1..1] of var 0..1: second_values; array[int] of bool: boolean_parameters; var int: boolean_guard_result; constraint boolean_guard(boolean_parameters,position,boolean_guard_result); var bool: boolean_opaque_result; constraint boolean_opaque_index(flags,position,boolean_opaque_result); var int: private_array_opaque_result; constraint private_array_opaque(input,private_array_opaque_result); var bool: private_opaque_result; constraint private_opaque(private_opaque_result); annotation user_hint; predicate user_promised(var int: result) :: user_hint = result=0; var int: user_promised_result; constraint user_promised(user_promised_result); var int: promised_partial_result; constraint promised_partial(promised_partial_result) :: domain; array[int,int] of int: positions; array[int] of int: parameter_values; set of int: other; var int: subset_wrong_result; constraint subset_wrong(subset_wrong_result); var int: subset_shift_result; constraint subset_shift(subset_shift_result); var int: collected_raw_result; constraint collected_raw(parameter_values,other,collected_raw_result); var int: collected_opaque_result; constraint collected_opaque(other,collected_opaque_result); var int: nested_raw_result; constraint nested_raw(positions,first_values,position,nested_raw_result); var bool: opaque_equivalence_result; constraint opaque_equivalence(opaque_equivalence_result); var int: computed_result; predicate computed_subject(var int: result)=forall(i in 1..1)(result=selected_values(first_values,second_values)[i]); constraint computed_subject(computed_result); array[int] of var bool: flags; array[int] of var bool: other_flags; var bool: compound_bounds_result; constraint compound_bounds(gate,flags,compound_bounds_result); var bool: wrong_bounds_result; constraint wrong_bounds(flags,other_flags,wrong_bounds_result); var bool: raw_min_result; var bool: else_min_result; constraint raw_min(flags,raw_min_result); constraint else_min(flags,else_min_result); var bool: decision_gate; var int: specialized_parameter_result; var int: specialized_decision_result; constraint specialized(gate,input,specialized_parameter_result); constraint specialized(decision_gate,input,specialized_decision_result); var int: decision_result; var int: partial_result; var int: asserted_partial_result; var int: asserted_false_result; var int: named_assert_result; array[int] of int: bounds; int: position; var int: specialized_partial_result; constraint specialized_relation(bounds[position],specialized_partial_result); var int: raw_index_result; var int: raw_range_result; var int: unguarded_result; var int: filtered_bounds_result; array[0..3] of var int: partial_array; constraint partial_indices(partial_array,input); constraint raw_index(bounds,position,raw_index_result); constraint raw_range(bounds,position,raw_range_result); constraint asserted_partial(gate,asserted_partial_result); constraint asserted_false(asserted_false_result); constraint assert(x:named_assert_result=0,msg:\"named\",b:true); constraint unguarded_bounds(partial_array,unguarded_result); constraint filtered_bounds(gate,partial_array,filtered_bounds_result); constraint decision(decision_gate,input,decision_result); constraint partial(gate,input,partial_result); solve :: seq_search([bool_search([decision_gate],input_order,indomain_min,complete),int_search(first_values,input_order,indomain_min,complete)]) satisfy;").unwrap();
    let mut negative_source = std::fs::read_to_string(dir.join("root.mzn")).unwrap();
    negative_source.push_str("var opt bool: pattern_optional_value; var bool: pattern_optional_result; constraint pattern_optional(flags,pattern_optional_value,pattern_optional_result);\n");
    negative_source.push_str("\nvar int: reflection_decision; var int: reflection_decision_result; var int: reflection_raw_result; constraint scalar_bounds(reflection_decision,reflection_decision_result); constraint raw_bounds(bounds,reflection_raw_result);\n");
    negative_source.push_str("var int: nonasserted_alignment_result; var int: reified_alignment_result; var int: wrong_alignment_result; constraint nonasserted_alignment(first_values,bounds,nonasserted_alignment_result); constraint reified_alignment(gate,first_values,bounds,reified_alignment_result); constraint wrong_alignment(first_values,bounds,parameter_values,wrong_alignment_result);\n");
    negative_source.push_str("var int: unused_value_guard_result; var int: decision_value_guard_result; constraint unused_value_guard(input,unused_value_guard_result); constraint decision_value_guard(decision_value_guard_result);\n");
    negative_source.push_str("var int: partial_value_domain_result; var int: partial_local_domain_result; constraint partial_value_domain(bounds,position,partial_value_domain_result); constraint partial_local_domain(bounds,position,partial_local_domain_result);\n");
    negative_source.push_str("var int: decision_minimum_index_result; var int: raw_minimum_index_result; var int: filtered_minimum_index_result; var int: other_minimum_index_result; constraint minimum_index(first_values,decision_minimum_index_result); constraint raw_minimum_index(bounds,raw_minimum_index_result); constraint filtered_minimum_index(bounds,filtered_minimum_index_result); constraint other_minimum_index(bounds,parameter_values,other_minimum_index_result);\n");
    negative_source.push_str("constraint let { var bool: cycle_first; var bool: cycle_second; } in (cycle_first<->cycle_second) /\\ (cycle_second<->cycle_first); constraint let { var bool: outside_flag; } in forall(i in other)(outside_flag<->bounds[i]>=0); var int: missing_seed; constraint let { var bool: unanchored_flag; } in unanchored_flag<->missing_seed<=0;\n");
    negative_source.push_str("constraint let { var bool: poisoned_flag; } in (poisoned_flag<->first_values[1]<=0) /\\ opaque_relation(poisoned_flag); constraint gate -> let { var bool: reified_flag; } in reified_flag<->first_values[1]<=0; constraint let { var bool: partial_boolean_flag; } in partial_boolean_flag<->flags[position]; constraint let { var opt bool: optional_flag; } in optional_flag<->true; predicate hidden_iff(var bool: value)=let { var bool: hidden_flag; } in hidden_flag<->value; constraint hidden_iff(decision_gate);\n");
    negative_source.push_str("set of int: WeightedTasks; array[WeightedTasks] of var int: weighted_values; array[1..input] of int: shifted_weights; constraint forall(p in 0..1)(let { var int: shifted_weighted=sum(c in WeightedTasks)(bool2int(weighted_values[c]>p)*shifted_weights[c]); } in shifted_weighted>=0); constraint let { var int: partial_converted=bool2int(flags[position]); } in partial_converted>=0; constraint let { var 1..(1 div input): partial_integer=1; } in partial_integer>=0; constraint gate -> let { var int: reified_weighted=bool2int(decision_gate); } in reified_weighted>=0; predicate hidden_integer(var int: value)=let {var int: hidden_weighted=value;} in hidden_weighted>=0; constraint hidden_integer(position);\n");
    negative_source.push_str("var bool: table_filtered_result; constraint table_filtered(first_values,positions,table_filtered_result);\n");
    std::fs::write(dir.join("root.mzn"), negative_source).unwrap();
    let unsupported = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    assert!(unsupported.errors.is_empty(), "{:?}", unsupported.errors);
    assert!(
        unsupported
            .files
            .iter()
            .all(|file| file.parsed.diagnostics().is_empty())
    );
    let (bindings, search) = facts(&unsupported);
    for name in [
        "outside_flag",
        "poisoned_flag",
        "reified_flag",
        "partial_boolean_flag",
        "optional_flag",
        "hidden_flag",
        "shifted_weighted",
        "partial_converted",
        "partial_integer",
        "reified_weighted",
        "hidden_weighted",
    ] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(
            search.declarations[local.id.0].coverage,
            SearchCoverage::Unknown,
            "{name}"
        );
    }
    for name in ["cycle_first", "cycle_second", "unanchored_flag"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(
            search.declarations[local.id.0].coverage,
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "missing_seed"),
        SearchCoverage::Uncovered
    );
    assert_eq!(
        coverage(&bindings, &search, "specialized_parameter_result"),
        SearchCoverage::Scalar
    );
    for name in [
        "specialized_decision_result",
        "specialized_partial_result",
        "reflection_decision_result",
        "reflection_raw_result",
        "nonasserted_alignment_result",
        "reified_alignment_result",
        "wrong_alignment_result",
        "unused_value_guard_result",
        "decision_value_guard_result",
        "partial_value_domain_result",
        "partial_local_domain_result",
        "decision_minimum_index_result",
        "raw_minimum_index_result",
        "filtered_minimum_index_result",
        "other_minimum_index_result",
        "nested_raw_result",
        "collected_raw_result",
        "subset_wrong_result",
        "subset_shift_result",
        "collected_opaque_result",
        "opaque_equivalence_result",
        "computed_result",
        "promised_partial_result",
        "user_promised_result",
        "pattern_optional_result",
        "table_filtered_result",
        "private_opaque_result",
        "private_array_opaque_result",
        "boolean_guard_result",
        "boolean_opaque_result",
        "raw_min_result",
        "else_min_result",
        "wrong_bounds_result",
        "compound_bounds_result",
        "decision_result",
        "partial_result",
        "unguarded_result",
        "filtered_bounds_result",
        "asserted_partial_result",
        "asserted_false_result",
        "named_assert_result",
        "raw_index_result",
    ] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Unknown,
            "{name}: {:?}",
            search.limitations
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "raw_range_result"),
        SearchCoverage::Uncovered,
    );
    assert_ne!(
        coverage(&bindings, &search, "partial_array"),
        SearchCoverage::WholeArray
    );
    assert!(matches!(
        analyze_model(&unsupported, &selected()).rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let unbounded = analyze_model(
        &unsupported,
        &LintOptions::from_selection("unbounded-variable").unwrap(),
    );
    for name in ["shifted_weighted", "partial_converted", "reified_weighted"] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert!(
            unbounded
                .limitations
                .iter()
                .any(|l| l.location == local.location),
            "{name}: {:?}",
            unbounded
        );
    }
    std::fs::write(
        dir.join("root.mzn"),
        concat!(
            "include \"included.mzn\"; int: input; annotation domain; ",
            "var int: shadowed_domain_result; constraint local(input,shadowed_domain_result) :: domain; ",
            "var int: unknown_hint_result; constraint local(input,unknown_hint_result) :: missing_hint; solve satisfy;\n",
        ),
    )
    .unwrap();
    let annotated = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&annotated);
    for name in ["shadowed_domain_result", "unknown_hint_result"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Unknown,
            "{name}: {:?}",
            search.limitations
        );
    }
    std::fs::write(dir.join("root.mzn"),
        "constraint let { var bool: cycle_first; var bool: cycle_second; } in (cycle_first<->cycle_second) /\\ (cycle_second<->cycle_first); var int: missing_seed; constraint let { var bool: unanchored_flag; } in unanchored_flag<->missing_seed<=0; constraint let { var int: unanchored_integer=missing_seed*2; } in unanchored_integer>=0; var bool: cycle_result; constraint let { var bool: cycle_dependency; } in (cycle_dependency<->cycle_result) /\\ (cycle_result=cycle_dependency); set of int: EmptyOrMore; var bool: quantified_result; constraint forall(i in EmptyOrMore)(let { var bool: quantified_dependency; } in (quantified_dependency<->true) /\\ (quantified_result=quantified_dependency)); solve satisfy;\n").unwrap();
    let supported = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let (bindings, search) = facts(&supported);
    assert!(search.limitations.is_empty(), "{:?}", search.limitations);
    for name in [
        "cycle_first",
        "cycle_second",
        "unanchored_flag",
        "unanchored_integer",
        "cycle_dependency",
    ] {
        let local = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert_eq!(
            search.declarations[local.id.0].coverage,
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    for name in ["cycle_result", "quantified_result"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Uncovered,
            "{name}"
        );
    }
    assert!(matches!(
        analyze_model(&supported, &selected()).rules[0].outcome,
        RuleOutcome::Completed
    ));
    std::fs::write(dir.join("included.mzn"), "").unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}function int: min(int: a,int: b); function int: max(int: a,int: b); function var int: min(var int: a,var int: b); function var int: max(var int: a,var int: b); function var int: '*'(var int: a,var int: b); function int: 'div'(int: a,int: b);\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("root.mzn"),
        concat!(
            "int: input; var int: seed; var int: parameter_extrema=max(min(input,3),1); var int: decision_extrema=max(min(seed,3),1);\n",
            "var int: partial_extrema=min(seed,1 div 0); array[1..4] of var bool: flags; array[1..4] of var bool: unsearched_flags;\n",
            "var int: selected=1*flags[1]; var int: outside=1*flags[5]; var int: unseeded=1*unsearched_flags[1];\n",
            "solve :: seq_search([int_search([seed],input_order,indomain_min,complete),bool_search(flags,input_order,indomain_min,complete)]) satisfy;\n",
        ),
    )
    .unwrap();
    let extrema_and_selections = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    assert!(extrema_and_selections.errors.is_empty());
    let (bindings, search) = facts(&extrema_and_selections);
    for name in ["parameter_extrema", "decision_extrema", "selected"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Scalar,
            "{name}: {:?}",
            search.limitations
        );
    }
    for name in ["partial_extrema", "outside"] {
        assert_eq!(
            coverage(&bindings, &search, name),
            SearchCoverage::Unknown,
            "{name}: {:?}",
            search.limitations
        );
    }
    assert_eq!(
        coverage(&bindings, &search, "flags"),
        SearchCoverage::WholeArray
    );
    assert_eq!(
        coverage(&bindings, &search, "unsearched_flags"),
        SearchCoverage::Uncovered
    );
    assert_eq!(
        coverage(&bindings, &search, "unseeded"),
        SearchCoverage::Uncovered
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bin_weight_guards_require_ordered_parameter_or_and_supported_assertion_bodies() {
    let builtins = concat!(
        "function bool: '\\/'(bool: left,bool: right); function var bool: '\\/'(var bool: left,var bool: right);\n",
        "function int: length(array[$U] of any $V: xs); function int: lb_array(array[int] of var int: xs);\n",
        "function bool: '>='(int: left,int: right); function var bool: assert(bool: b,string: msg,var bool: x);\n",
        "function int: 'div'(int: left,int: right);\n",
    );
    for (name, values, condition, body, extra, supported) in [
        (
            "left-singleton",
            "[1]",
            "length(weight)=0 \\/ lb_array(weight)>=0",
            "result=0",
            "",
            true,
        ),
        (
            "left-empty",
            "[]",
            "length(weight)=0 \\/ lb_array(weight)>=0",
            "result=0",
            "",
            true,
        ),
        (
            "raw-empty",
            "[]",
            "lb_array(weight)>=0",
            "result=0",
            "",
            false,
        ),
        (
            "reversed-empty",
            "[]",
            "lb_array(weight)>=0 \\/ length(weight)=0",
            "result=0",
            "",
            false,
        ),
        (
            "other-array",
            "[]",
            "length(other)=0 \\/ lb_array(weight)>=0",
            "result=0",
            "",
            false,
        ),
        (
            "shadowed-or",
            "[]",
            "length(weight)=0 \\/ lb_array(weight)>=0",
            "result=0",
            "function bool: '\\/'(bool: left,bool: right)=true;",
            false,
        ),
        (
            "partial-body",
            "[]",
            "length(weight)=0 \\/ lb_array(weight)>=0",
            "result=0 /\\ result=1 div 0",
            "",
            false,
        ),
    ] {
        let axis = if values == "[]" { "1..0" } else { "1..1" };
        let source = format!(
            "include \"included.mzn\"; array[{axis}] of int: item_weight={values}; array[1..1] of int: other=[1]; var int: result; constraint public_weight_guard(item_weight,result); solve satisfy;\n"
        );
        let included = format!(
            "{extra}\npredicate public_weight_guard(array[int] of int: weight,var int: result)=assert({condition},\"Weights must be nonnegative\",{body});\n"
        );
        let (dir, _) = model(&format!("bin-weight-{name}"), &source, &included);
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!("{CORE}{builtins}"),
        )
        .unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context
                .files
                .iter()
                .all(|f| f.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        if name != "raw-empty" {
            let call = calls.calls.iter().find(|c| c.name == "\\/").unwrap();
            if name == "shadowed-or" {
                let zincite_lint::CallOutcome::Ambiguous { candidates } = &call.outcome else {
                    panic!("{name}: {:?}", call.outcome);
                };
                assert!(
                    candidates.iter().any(|id| {
                        let declaration = &bindings.declarations[id.0];
                        let owner = &context.files[declaration.file];
                        declaration.name == "\\/"
                            && declaration.role == zincite_lint::DeclarationRole::Function
                            && declaration.top_level
                            && owner.kind == zincite_lint::SourceKind::User
                            && owner.path == dir.join("included.mzn")
                            && !owner.implicit
                    }),
                    "{name}: {:?}",
                    call.outcome
                );
            } else {
                let zincite_lint::CallOutcome::Resolved {
                    declaration,
                    parameters,
                    return_type,
                } = &call.outcome
                else {
                    panic!("{name}: {:?}", call.outcome);
                };
                let parameter_bool = |ty: &zincite_lint::TypeInst| {
                    ty.instantiation == zincite_lint::Instantiation::Parameter
                        && ty.kind == zincite_lint::TypeKind::Bool
                        && !ty.optional
                };
                assert_eq!(parameters.len(), 2);
                assert!(parameters.iter().all(parameter_bool) && parameter_bool(return_type));
                let owner = &context.files[bindings.declarations[declaration.0].file];
                assert_eq!(owner.kind, zincite_lint::SourceKind::StandardLibrary);
                assert!(owner.implicit);
            }
        }
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
        let (_, search) = facts(&context);
        let result_id = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == "result")
            .unwrap()
            .id;
        let supported_definition = callable.definitions.iter().any(|d| {
            d.target == result_id
                && d.coverage == DefinitionCoverage::Scalar
                && d.safety == DefinitionSafety::Supported
        });
        assert_eq!(supported_definition, supported, "{name}: {callable:?}");
        assert_eq!(
            coverage(&bindings, &search, "result"),
            if supported {
                SearchCoverage::Scalar
            } else {
                SearchCoverage::Unknown
            },
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        if supported {
            assert!(
                callable.unavailable.is_empty(),
                "{name}: {:?}",
                callable.unavailable
            );
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{name}: {:?}",
                result.limitations
            );
            assert!(result.findings.is_empty());
        } else {
            assert!(!callable.unavailable.is_empty(), "{name}");
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}: {:?}",
                result.limitations
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn enforced_boolean_assertions_keep_outer_branch_and_abort_boundaries() {
    for (name, body, expected, limited) in [
        (
            "enforced",
            "assert(gate,\"enabled\",result=0)",
            SearchCoverage::Scalar,
            false,
        ),
        (
            "outer-branch",
            "if take then assert(gate,\"enabled\",result=0) else true endif",
            SearchCoverage::Uncovered,
            false,
        ),
        (
            "false-abort",
            "assert(false,\"abort\",result=0)",
            SearchCoverage::Unknown,
            true,
        ),
    ] {
        let source = "include \"included.mzn\"; bool: gate; bool: take; var int: result; constraint checked(gate,take,result); solve satisfy;";
        let included =
            format!("predicate checked(bool: gate,bool: take,var int: result)={body};\n");
        let (dir, _) = model(&format!("boolean-assert-{name}"), source, &included);
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!("{CORE}function var bool: assert(bool: b,string: msg,var bool: x);\n"),
        )
        .unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        let (bindings, search) = facts(&context);
        let calls = resolve_callables(&context, &bindings);
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
        let result_id = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == "result")
            .unwrap()
            .id;
        assert_eq!(
            callable
                .definitions
                .iter()
                .any(|d| d.target == result_id && d.safety == DefinitionSafety::Supported),
            expected == SearchCoverage::Scalar,
            "{name}: {callable:?}"
        );
        assert_eq!(
            coverage(&bindings, &search, "result"),
            expected,
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        assert_eq!(
            matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
            limited,
            "{name}: {:?}",
            result.limitations
        );
        if name == "false-abort" {
            assert!(
                callable
                    .unavailable
                    .iter()
                    .any(|u| u.reason == "false assertion condition aborts evaluation")
            );
        } else {
            assert!(
                callable.unavailable.is_empty(),
                "{name}: {:?}",
                callable.unavailable
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn asserted_array_membership_requires_exact_scope_and_successful_actual_axes() {
    fn present_type(ty: &zincite_lint::TypeInst) -> bool {
        !ty.optional
            && matches!(
                ty.instantiation,
                zincite_lint::Instantiation::Parameter | zincite_lint::Instantiation::Decision
            )
            && match &ty.kind {
                zincite_lint::TypeKind::Bool
                | zincite_lint::TypeKind::Int
                | zincite_lint::TypeKind::String => true,
                zincite_lint::TypeKind::Set(element) => present_type(element),
                zincite_lint::TypeKind::Array { indices, element } => {
                    indices.iter().all(present_type) && present_type(element)
                }
                _ => false,
            }
    }

    // These core tuples match the captured public direct producer. Compiler
    // acceptance of the partial controls supplies no selector safety proof.
    let builtins = concat!(
        "function var bool: assert(bool: condition,string: message,var bool: value);\n",
        "function var int: '*'(var int: left,var int: right); annotation first_fail;\n",
        "function var bool: forall(array[int] of var bool: body);\n",
    );
    for (name, source, supported) in [
        (
            "same-callee-unguarded-mismatched-axis",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_weighted_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)));

predicate public_aligned_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    assert(
        index_set(bin) == index_set(weight),
        "Bin and weight index sets must agree",
        public_weighted_load(load, bin, weight)
    );

% A guarded use must not certify another invocation of the same declaration.
array[1..1] of int: guarded_weight = [1];
array[1..1] of var 1..1: guarded_placement;
array[1..1] of var 0..1: guarded_load;
array[1..0] of int: item_weight = [];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "The first invocation has its own index guard"
    public_aligned_load(guarded_load, guarded_placement, guarded_weight);
constraint :: "The second invocation has no index guard"
    public_weighted_load(bin_load, placement, item_weight);

solve :: seq_search([
    int_search(placement, first_fail, indomain_min, complete),
    int_search(guarded_placement, first_fail, indomain_min, complete)
]) satisfy;
"#,
            false,
        ),
        (
            "aligned-direct-singleton",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_direct_aligned_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    assert(
        index_set(bin) == index_set(weight),
        "Bin and weight index sets must agree",
        forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)))
    );

array[1..1] of int: item_weight = [1];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "weighted_load_indices"
    public_direct_aligned_load(bin_load, placement, item_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            true,
        ),
        (
            "aligned-direct-abort",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_direct_aligned_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    assert(
        index_set(bin) == index_set(weight),
        "Bin and weight index sets must agree",
        forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)))
    );

array[1..0] of int: item_weight = [];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "weighted_load_indices"
    public_direct_aligned_load(bin_load, placement, item_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            false,
        ),
        (
            "other-assert-empty-weight",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_other_aligned_load(
    array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight, array[int] of int: other
) =
    assert(
        index_set(bin) == index_set(other),
        "Bin and other index sets must agree",
        forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)))
    );

array[1..0] of int: item_weight = [];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;
array[1..1] of int: other_weight = [1];

constraint :: "weighted_load_indices"
    public_other_aligned_load(bin_load, placement, item_weight, other_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            false,
        ),
        (
            "unaligned-empty-weight",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_weighted_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)));

array[1..0] of int: item_weight = [];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "weighted_load_indices"
    public_weighted_load(bin_load, placement, item_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            false,
        ),
        (
            "unaligned-shifted-exact-weight",
            r#"% V2: explicit array1d fixes the initializer axis before testing the selector.
% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_weighted_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)));

array[2..2] of int: item_weight = array1d(2..2, [1]);
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "weighted_load_indices"
    public_weighted_load(bin_load, placement, item_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            false,
        ),
        (
            "aligned-nested-singleton",
            r#"% Public reduction of fzn_bin_packing_load.mzn:10's weighted selector.
% This preserves weight[i] under i in index_set(bin), with an unsearched load.
predicate public_weighted_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    forall(b in index_set(load))(load[b] = sum(i in index_set(bin))(weight[i] * (bin[i] = b)));

predicate public_aligned_load(array[int] of var int: load, array[int] of var int: bin, array[int] of int: weight) =
    assert(
        index_set(bin) == index_set(weight),
        "Bin and weight index sets must agree",
        public_weighted_load(load, bin, weight)
    );

array[1..1] of int: item_weight = [1];
array[1..1] of var 1..1: placement;
array[1..1] of var 0..1: bin_load;

constraint :: "weighted_load_indices"
    public_aligned_load(bin_load, placement, item_weight);

solve :: int_search(placement, first_fail, indomain_min, complete) satisfy;
"#,
            true,
        ),
    ] {
        let (dir, _) = model(&format!("asserted-array-membership-{name}"), source, "");
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!("{CORE}{builtins}"),
        )
        .unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context
                .files
                .iter()
                .all(|f| f.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let parameter = |ty: &zincite_lint::TypeInst, kind| {
            !ty.optional
                && ty.instantiation == zincite_lint::Instantiation::Parameter
                && ty.kind == kind
        };
        // Fail fixture preflight before asserting semantic behavior if these
        // faithful public operations do not retain the observed core tuples.
        for call in calls.calls.iter().filter(|call| {
            context.files[call.file].kind == zincite_lint::SourceKind::User
                && matches!(
                    call.name.as_str(),
                    "assert" | "index_set" | "forall" | "sum" | "*" | "="
                )
        }) {
            let zincite_lint::CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &call.outcome
            else {
                panic!("{name}: {} fixture tuple {:?}", call.name, call.outcome);
            };
            let selected = &bindings.declarations[declaration.0];
            let owner = &context.files[selected.file];
            assert_eq!(selected.name, call.name, "{name}");
            assert_eq!(
                owner.kind,
                zincite_lint::SourceKind::StandardLibrary,
                "{name}"
            );
            assert!(owner.implicit, "{name}");
            assert!(parameters.iter().all(present_type), "{name}");
            assert!(present_type(return_type), "{name}");
            if call.name == "assert" {
                assert_eq!(parameters.len(), 3, "{name}");
                assert!(
                    parameter(&parameters[0], zincite_lint::TypeKind::Bool),
                    "{name}"
                );
                assert!(
                    parameter(&parameters[1], zincite_lint::TypeKind::String),
                    "{name}"
                );
                assert_eq!(parameters[2].kind, zincite_lint::TypeKind::Bool, "{name}");
                assert_eq!(
                    parameters[2].instantiation,
                    zincite_lint::Instantiation::Decision,
                    "{name}"
                );
                assert_eq!(return_type, &parameters[2], "{name}");
            } else if call.name == "index_set" {
                assert_eq!(parameters.len(), 1, "{name}");
                assert!(
                    matches!(&parameters[0].kind,
                    zincite_lint::TypeKind::Array { indices, element }
                    if indices.len() == 1 && parameter(&indices[0], zincite_lint::TypeKind::Int)
                        && element.instantiation != zincite_lint::Instantiation::Unknown && !element.optional && element.kind == zincite_lint::TypeKind::Int),
                    "{name}"
                );
                assert!(
                    matches!(&return_type.kind, zincite_lint::TypeKind::Set(element)
                    if parameter(element, zincite_lint::TypeKind::Int)),
                    "{name}"
                );
                assert_eq!(
                    return_type.instantiation,
                    zincite_lint::Instantiation::Parameter,
                    "{name}"
                );
            } else if matches!(call.name.as_str(), "forall" | "sum") {
                assert_eq!(parameters.len(), 1, "{name}");
                let kind = if call.name == "forall" {
                    zincite_lint::TypeKind::Bool
                } else {
                    zincite_lint::TypeKind::Int
                };
                assert!(
                    matches!(&parameters[0].kind,
                    zincite_lint::TypeKind::Array { indices, element }
                    if indices.len() == 1 && parameter(&indices[0], zincite_lint::TypeKind::Int)
                        && !element.optional && element.kind == kind
                        && element.instantiation == zincite_lint::Instantiation::Decision),
                    "{name}"
                );
                assert_eq!(
                    parameters[0].instantiation,
                    zincite_lint::Instantiation::Decision,
                    "{name}"
                );
                assert_eq!(return_type.kind, kind, "{name}");
                assert_eq!(
                    return_type.instantiation,
                    zincite_lint::Instantiation::Decision,
                    "{name}"
                );
            } else if call.name == "*" {
                assert_eq!(parameters.len(), 2, "{name}");
                assert!(
                    parameters
                        .iter()
                        .all(|ty| ty.kind == zincite_lint::TypeKind::Int
                            && ty.instantiation == zincite_lint::Instantiation::Decision),
                    "{name}"
                );
                assert_eq!(return_type.kind, zincite_lint::TypeKind::Int, "{name}");
                assert_eq!(
                    return_type.instantiation,
                    zincite_lint::Instantiation::Decision,
                    "{name}"
                );
            } else if call.name == "="
                && matches!(&parameters[0].kind, zincite_lint::TypeKind::Set(_))
            {
                assert_eq!(parameters.len(), 2, "{name}");
                assert!(
                    parameters.iter().all(|ty| {
                        ty.instantiation == zincite_lint::Instantiation::Parameter
                            && matches!(&ty.kind, zincite_lint::TypeKind::Set(element)
                            if parameter(element, zincite_lint::TypeKind::Int))
                    }),
                    "{name}"
                );
                assert!(
                    parameter(return_type, zincite_lint::TypeKind::Bool),
                    "{name}"
                );
            }
        }
        for call in calls.calls.iter().filter(|call| {
            context.files[call.file].kind == zincite_lint::SourceKind::User
                && matches!(
                    call.name.as_str(),
                    "public_weighted_load" | "public_aligned_load"
                )
        }) {
            let zincite_lint::CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &call.outcome
            else {
                panic!("{name}: nested selection {:?}", call.outcome);
            };
            let selected = &bindings.declarations[declaration.0];
            assert_eq!(selected.name, call.name, "{name}");
            assert_eq!(
                selected.role,
                zincite_lint::DeclarationRole::Predicate,
                "{name}"
            );
            assert_eq!(
                context.files[selected.file].kind,
                zincite_lint::SourceKind::User,
                "{name}"
            );
            let integer_array = |ty: &zincite_lint::TypeInst, instantiation| {
                present_type(ty)
                    && ty.instantiation == instantiation
                    && matches!(&ty.kind, zincite_lint::TypeKind::Array { indices, element }
                        if indices.len() == 1 && parameter(&indices[0], zincite_lint::TypeKind::Int)
                            && element.kind == zincite_lint::TypeKind::Int
                            && element.instantiation == instantiation)
            };
            assert_eq!(parameters.len(), 3, "{name}");
            assert!(
                integer_array(&parameters[0], zincite_lint::Instantiation::Decision),
                "{name}"
            );
            assert!(
                integer_array(&parameters[1], zincite_lint::Instantiation::Decision),
                "{name}"
            );
            assert!(
                integer_array(&parameters[2], zincite_lint::Instantiation::Parameter),
                "{name}"
            );
            assert!(present_type(return_type), "{name}");
            assert_eq!(return_type.kind, zincite_lint::TypeKind::Bool, "{name}");
            assert_eq!(
                return_type.instantiation,
                zincite_lint::Instantiation::Decision,
                "{name}"
            );
        }
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        if matches!(
            name,
            "aligned-nested-singleton" | "same-callee-unguarded-mismatched-axis"
        ) {
            let axis = |name| {
                let id = bindings
                    .declarations
                    .iter()
                    .find(|d| d.top_level && d.name == name)
                    .unwrap()
                    .id;
                let zincite_lint::Domain::Array { indices, .. } =
                    &domains.declarations[id.0].domain
                else {
                    panic!("{name}: written array domain unavailable");
                };
                indices.clone()
            };
            let singleton = vec![zincite_lint::Domain::Range {
                lower: zincite_lint::NumericBound::Integer(1),
                upper: zincite_lint::NumericBound::Integer(1),
            }];
            assert_eq!(axis("placement"), singleton, "{name}");
            assert_eq!(
                axis("item_weight"),
                if name == "aligned-nested-singleton" {
                    singleton.clone()
                } else {
                    vec![zincite_lint::Domain::Range {
                        lower: zincite_lint::NumericBound::Integer(1),
                        upper: zincite_lint::NumericBound::Integer(0),
                    }]
                },
                "{name}"
            );
            if name == "same-callee-unguarded-mismatched-axis" {
                assert_eq!(axis("guarded_placement"), singleton, "{name}");
                assert_eq!(axis("guarded_weight"), singleton, "{name}");
            }
        }
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let callable = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
        if name == "same-callee-unguarded-mismatched-axis" {
            let callee = bindings
                .declarations
                .iter()
                .find(|d| d.top_level && d.name == "public_weighted_load")
                .unwrap()
                .id;
            assert!(
                !callable.outputs.iter().any(|output| {
                    output.callable == callee && output.coverage == DefinitionCoverage::WholeArray
                }),
                "{name}: guarded invocation leaked into an unconditional callee summary: {:?}",
                callable.outputs
            );
        }
        let search =
            resolve_search_coverage(&context, &bindings, &calls, &inst, &domains, &definitions);
        let load = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == "bin_load")
            .unwrap()
            .id;
        let supported_definition = callable.definitions.iter().any(|d| {
            d.target == load
                && d.safety == DefinitionSafety::Supported
                && d.coverage == DefinitionCoverage::WholeArray
        });
        assert_eq!(supported_definition, supported, "{name}: {callable:?}");
        assert_eq!(
            coverage(&bindings, &search, "placement"),
            SearchCoverage::WholeArray,
            "{name}"
        );
        assert_eq!(
            coverage(&bindings, &search, "bin_load"),
            if supported {
                SearchCoverage::WholeArray
            } else {
                SearchCoverage::Unknown
            },
            "{name}: {:?}",
            search.limitations
        );
        let result = analyze_model(&context, &selected());
        if supported {
            assert!(
                callable.unavailable.is_empty(),
                "{name}: {:?}",
                callable.unavailable
            );
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Completed),
                "{name}: {:?}",
                result.limitations
            );
            assert!(result.findings.is_empty(), "{name}: {:?}", result.findings);
        } else {
            assert!(!callable.unavailable.is_empty(), "{name}");
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}: {:?}",
                result.limitations
            );
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn set_valued_headers_and_parameter_conditionals_inspect_relations_without_outputs() {
    use zincite_lint::{
        BindingResolution, CallOutcome, DeclarationRole, Domain, Instantiation, ReferenceKind,
        TypeInst, TypeKind,
    };
    use zincite_syntax::NodeKind;
    let source = r#"include "globals.mzn";

% Data retain separate named axes and symbolic set contents.
set of int: Matches;
set of int: DataAxis;
set of int: Blocks;
set of int: BlocksPlusNull;
set of int: Removed;
array[Matches] of set of int: data_for;
array[Matches] of set of int: spans;

% Only gate is an explicit search seed.
array[Matches] of var bool: chosen;
array[DataAxis] of var Blocks: defined;
array[Matches] of var BlocksPlusNull: placed;
var bool: gate;

% A selected match relates each covered datum to its spanned or placed block.
constraint :: "Selected match relates its data to blocks"
    forall (m in Matches diff Removed) (
        forall (e in data_for[m]) (
            if card(spans[m]) > 0 then
                chosen[m] -> defined[e] in spans[m]
            else
                chosen[m] -> defined[e] = placed[m]
            endif
        )
    );

solve :: bool_search([gate], input_order, indomain_min, complete) satisfy;
"#;
    let extra = concat!(
        "function set of int: 'diff'(set of int: left,set of int: right);\n",
        "function int: card(set of int: values); function bool: '>'(int: left,int: right);\n",
        "function int: '+'(int: left,int: right);\n",
    );
    // Establish the directory before loading the source's globals include.
    let (dir, _) = model("set-valued-header-conditional", "solve satisfy;", "");
    let standard = CORE.replace(
        "function var bool: forall(array[int] of var opt bool: body);",
        "function var bool: forall(array[int] of var bool: body);",
    );
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    std::fs::write(dir.join("library/std/globals.mzn"), "").unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{standard}{extra}"),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(dir.join("library")),
        ..Default::default()
    };
    let context = load_model(dir.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let scalar = |ty: &TypeInst, kind: TypeKind, instantiation| {
        !ty.optional && ty.kind == kind && ty.instantiation == instantiation
    };
    let integer_set = |ty: &TypeInst| {
        !ty.optional
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element)
                if scalar(element, TypeKind::Int, Instantiation::Parameter))
    };
    let boolean_array = |ty: &TypeInst| {
        !ty.optional
            && ty.instantiation == Instantiation::Decision
            && matches!(&ty.kind, TypeKind::Array { indices, element }
                if indices.len() == 1 && scalar(&indices[0], TypeKind::Int, Instantiation::Parameter)
                    && scalar(element, TypeKind::Bool, Instantiation::Decision))
    };
    for (name, count) in [
        ("diff", 1),
        ("card", 1),
        (">", 1),
        ("forall", 2),
        ("->", 2),
        ("in", 1),
        ("=", 1),
    ] {
        let selected: Vec<_> = calls
            .calls
            .iter()
            .filter(|call| call.file == 0 && call.name == name)
            .collect();
        assert_eq!(selected.len(), count, "{name}: {:?}", calls.calls);
        for call in selected {
            let CallOutcome::Resolved {
                declaration,
                parameters,
                return_type,
            } = &call.outcome
            else {
                panic!("{name}: {:?}", call);
            };
            let owner = &bindings.declarations[declaration.0];
            assert_eq!(owner.role, DeclarationRole::Function, "{:?}", call);
            assert!(context.files[owner.file].implicit, "{:?}", call);
            assert_eq!(
                context.files[owner.file].path,
                dir.join("library/std/stdlib.mzn")
            );
            let signature = match name {
                "diff" => {
                    parameters.len() == 2
                        && parameters.iter().all(integer_set)
                        && integer_set(return_type)
                }
                "card" => {
                    parameters.len() == 1
                        && integer_set(&parameters[0])
                        && scalar(return_type, TypeKind::Int, Instantiation::Parameter)
                }
                ">" => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Int, Instantiation::Parameter))
                        && scalar(return_type, TypeKind::Bool, Instantiation::Parameter)
                }
                "forall" => {
                    parameters.len() == 1
                        && boolean_array(&parameters[0])
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "->" => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Bool, Instantiation::Decision))
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "in" => {
                    parameters.len() == 2
                        && scalar(&parameters[0], TypeKind::Int, Instantiation::Decision)
                        && integer_set(&parameters[1])
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                "=" => {
                    parameters.len() == 2
                        && parameters
                            .iter()
                            .all(|ty| scalar(ty, TypeKind::Int, Instantiation::Decision))
                        && scalar(return_type, TypeKind::Bool, Instantiation::Decision)
                }
                _ => false,
            };
            assert!(signature, "{name}: {:?}", call);
        }
    }
    assert!(
        calls
            .calls
            .iter()
            .filter(|call| call.file == 0)
            .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
        "{:?}",
        calls.calls
    );
    let declaration = |name: &str| {
        bindings
            .declarations
            .iter()
            .find(|d| d.name == name && d.top_level)
            .unwrap()
            .id
    };
    let domains = resolve_domains(&context, &bindings);
    for (array, axis) in [
        ("data_for", "Matches"),
        ("spans", "Matches"),
        ("chosen", "Matches"),
        ("defined", "DataAxis"),
        ("placed", "Matches"),
    ] {
        assert!(
            matches!(&domains.declarations[declaration(array).0].domain, Domain::Array { indices, .. }
            if matches!(indices.as_slice(), [Domain::Named { declaration: id, .. }] if *id == declaration(axis))),
            "{array}: {:?}",
            domains.declarations[declaration(array).0].domain
        );
    }
    let parsed = &context.files[0].parsed;
    let mut pending = vec![parsed.tree()];
    let mut nodes = Vec::new();
    while let Some(node) = pending.pop() {
        nodes.push(node);
        pending.extend(node.child_nodes());
    }
    for (written, count, kind, instantiation) in [
        (
            "data_for[m]",
            1,
            TypeKind::Set(Box::new(TypeInst {
                kind: TypeKind::Int,
                instantiation: Instantiation::Parameter,
                optional: false,
            })),
            Instantiation::Parameter,
        ),
        (
            "spans[m]",
            2,
            TypeKind::Set(Box::new(TypeInst {
                kind: TypeKind::Int,
                instantiation: Instantiation::Parameter,
                optional: false,
            })),
            Instantiation::Parameter,
        ),
        ("chosen[m]", 2, TypeKind::Bool, Instantiation::Decision),
        ("defined[e]", 2, TypeKind::Int, Instantiation::Decision),
        ("placed[m]", 1, TypeKind::Int, Instantiation::Decision),
    ] {
        let accesses: Vec<_> = nodes
            .iter()
            .copied()
            .filter(|node| {
                node.kind() == NodeKind::ArrayAccessExpression
                    && parsed.source()[node.range()].trim() == written
            })
            .collect();
        assert_eq!(accesses.len(), count, "{written}");
        for node in accesses {
            let range = context.files[0].location(node.range()).range;
            let actual = calls
                .expressions
                .iter()
                .find(|fact| fact.file == 0 && fact.location.range == range);
            assert!(
                actual.is_some_and(|fact| scalar(&fact.ty, kind.clone(), instantiation)),
                "{written}: {:?}",
                actual
            );
            let selector = node.child_nodes().nth(1).unwrap();
            let range = context.files[0].location(selector.range()).range;
            let actual = calls
                .expressions
                .iter()
                .find(|fact| fact.file == 0 && fact.location.range == range);
            assert!(
                actual.is_some_and(|fact| scalar(
                    &fact.ty,
                    TypeKind::Int,
                    Instantiation::Parameter
                )),
                "{written}: {:?}",
                actual
            );
            let reference = bindings
                .references
                .iter()
                .find(|reference| {
                    reference.file == 0
                        && reference.kind == ReferenceKind::Value
                        && reference.location.range.start >= range.start
                        && reference.location.range.end <= range.end
                })
                .unwrap();
            let BindingResolution::Resolved(binder) = reference.resolution else {
                panic!("{:?}", reference);
            };
            let owner = &bindings.declarations[binder.0];
            assert_eq!(owner.role, DeclarationRole::Generator);
            assert_eq!(owner.name, if written.contains("[e]") { "e" } else { "m" });
            assert!(nodes.iter().any(
                |node| node.kind() == NodeKind::Generator && node.range() == owner.syntax_range
            ));
        }
    }
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let outputs = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    for name in ["chosen", "defined", "placed"] {
        assert!(
            !outputs
                .definitions
                .iter()
                .any(|definition| definition.target == declaration(name)),
            "{name}: {:?}",
            outputs.definitions
        );
    }
    let (_, search) = facts(&context);
    assert!(
        !search
            .searched
            .iter()
            .any(|value| value.coverage == SearchCoverage::WholeArray)
    );
    for name in ["chosen", "defined", "placed"] {
        assert!(
            !search
                .searched
                .iter()
                .any(|value| value.declaration == Some(declaration(name))),
            "{name}"
        );
    }
    let result = analyze_model(&context, &selected());
    // Only this reached assertion establishes the intended behavioral RED.
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{:?}",
        result.limitations
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(coverage(&bindings, &search, "gate"), SearchCoverage::Scalar);
    for name in ["chosen", "defined", "placed"] {
        assert!(
            matches!(
                coverage(&bindings, &search, name),
                SearchCoverage::Uncovered | SearchCoverage::Unknown
            ),
            "{name}: {:?}",
            search
        );
    }
    // A different symbolic axis is uncertainty, not an independently closed failure.
    // The negative instead retains a failing written source axis for the same header.
    let negative_source = source.replace(
        "array[Matches] of set of int: data_for;",
        "array[1..(9223372036854775807 + 1)] of set of int: data_for;",
    );
    std::fs::write(dir.join("root.mzn"), &negative_source).unwrap();
    let negative = load_model(dir.join("root.mzn"), &options);
    assert!(negative.errors.is_empty(), "{:?}", negative.errors);
    let negative_bindings = resolve_bindings(&negative);
    let negative_calls = resolve_callables(&negative, &negative_bindings);
    assert!(
        negative_calls
            .calls
            .iter()
            .filter(|call| call.file == 0)
            .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
        "{:?}",
        negative_calls.calls
    );
    let negative_domains = resolve_domains(&negative, &negative_bindings);
    let source_array = negative_bindings
        .declarations
        .iter()
        .find(|d| d.name == "data_for" && d.top_level)
        .unwrap()
        .id;
    assert!(
        matches!(&negative_domains.declarations[source_array.0].domain,
        Domain::Array { indices, .. } if indices.len() == 1 && indices[0].numeric_minimum().is_err()),
        "{:?}",
        negative_domains.declarations[source_array.0].domain
    );
    let result = analyze_model(&negative, &selected());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{:?}",
        result.limitations
    );
    assert!(!result.limitations.is_empty());
    let (_, search) = facts(&negative);
    assert!(
        !search
            .searched
            .iter()
            .any(|value| value.coverage == SearchCoverage::WholeArray)
    );
    // The where filter owns a separate source error, independent of data_for.
    let filter_source = source
        .replace(
            "array[Matches] of set of int: spans;",
            "array[Matches] of set of int: spans;\narray[1..(9223372036854775807 + 1)] of set of int: filter_sets;",
        )
        .replace(
            "forall (e in data_for[m])",
            "forall (e in data_for[m] where card(filter_sets[m]) > 0)",
        );
    std::fs::write(dir.join("root.mzn"), &filter_source).unwrap();
    let filtered = load_model(dir.join("root.mzn"), &options);
    assert!(filtered.errors.is_empty(), "{:?}", filtered.errors);
    let filter_bindings = resolve_bindings(&filtered);
    let filter_calls = resolve_callables(&filtered, &filter_bindings);
    assert!(
        filter_calls
            .calls
            .iter()
            .filter(|call| call.file == 0)
            .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
        "{:?}",
        filter_calls.calls
    );
    let mut pending = vec![filtered.files[0].parsed.tree()];
    let mut conditions = Vec::new();
    while let Some(node) = pending.pop() {
        if node.kind() == NodeKind::WhereFilter {
            conditions.extend(node.child_nodes());
        }
        pending.extend(node.child_nodes());
    }
    assert_eq!(conditions.len(), 1);
    let range = filtered.files[0].location(conditions[0].range()).range;
    let condition = filter_calls
        .expressions
        .iter()
        .find(|fact| fact.file == 0 && fact.location.range == range);
    assert!(
        condition.is_some_and(|fact| scalar(&fact.ty, TypeKind::Bool, Instantiation::Parameter)),
        "{:?}",
        condition
    );
    let filter_domains = resolve_domains(&filtered, &filter_bindings);
    let filter_array = filter_bindings
        .declarations
        .iter()
        .find(|d| d.name == "filter_sets" && d.top_level)
        .unwrap()
        .id;
    assert!(
        matches!(&filter_domains.declarations[filter_array.0].domain,
        Domain::Array { indices, .. } if indices.len() == 1 && indices[0].numeric_minimum().is_err()),
        "{:?}",
        filter_domains.declarations[filter_array.0].domain
    );
    let result = analyze_model(&filtered, &selected());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{:?}",
        result.limitations
    );
    assert!(!result.limitations.is_empty());
    let (_, search) = facts(&filtered);
    assert!(
        !search
            .searched
            .iter()
            .any(|value| value.coverage == SearchCoverage::WholeArray)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
