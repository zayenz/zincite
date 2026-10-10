use std::path::PathBuf;
use zincite_lint::{
    Domain, LintOptions, ModelOptions, NumericBound, Rule, RuleOutcome, analyze_model, load_model,
    resolve_bindings, resolve_callables, resolve_definitions, resolve_domains,
    resolve_instantiations,
};

fn model(name: &str, source: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let directory =
        std::env::temp_dir().join(format!("zincite-domains-{name}-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        "function int: card(set of int: values); function int: '+'(int: left,int: right);\n",
    )
    .unwrap();
    let root = directory.join("root.mzn");
    std::fs::write(&root, source).unwrap();
    let context = load_model(
        &root,
        &ModelOptions {
            stdlib_dir: Some(directory.join("library")),
            include_dirs: Vec::new(),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (directory, context)
}
fn selected() -> LintOptions {
    LintOptions {
        rules: vec![Rule::ArrayIndexStart],
        ..LintOptions::default()
    }
}

#[test]
fn symbolic_local_conditional_axes_retain_source_errors() {
    let source = concat!(
        "predicate symbolic_axis(array[int] of int: cons) =\n",
        "    let {\n",
        "        int: n = if cons[1] = -1 then 0 else max(index_set(cons)) endif,\n",
        "        array[1..n+1] of int: values = [1 | i in 1..n+1]\n",
        "    } in true;\n",
        "constraint symbolic_axis([0,1]); solve satisfy;\n",
    );
    let (directory, _) = model("local-conditional-axis", source);
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function int: '+'(int: left,int: right);\n",
            "function int: '-'(int: value);\n",
            "function bool: '='(int: left,int: right);\n",
            "function set of int: '..'(int: left,int: right);\n",
            "function set of int: index_set(array[int] of int: values);\n",
            "function int: max(set of int: values);\n",
        ),
    )
    .unwrap();
    let root = directory.join("root.mzn");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let domains = resolve_domains(&context, &bindings);
    let axis = domains
        .array_indices
        .iter()
        .find(|axis| {
            axis.location.path == root && source[axis.location.range.clone()].trim() == "1..n+1"
        })
        .unwrap();
    assert!(axis.domain.numeric_minimum().is_err(), "{:?}", axis);
    let result = analyze_model(&context, &selected());

    for (before, after) in [
        ("then 0", "then (9223372036854775807+1)"),
        ("cons[1]", "cons[9223372036854775807+1]"),
    ] {
        assert!(source.contains(before));
        let hazardous = source.replace(before, after);
        std::fs::write(&root, &hazardous).unwrap();
        let context = load_model(&root, &options);
        let unsafe_result = analyze_model(&context, &selected());
        assert!(
            unsafe_result.errors.is_empty(),
            "{:?}",
            unsafe_result.errors
        );
        assert!(
            unsafe_result.findings.is_empty(),
            "{:?}",
            unsafe_result.findings
        );
        assert!(matches!(
            unsafe_result.rules[0].outcome,
            RuleOutcome::Limited { .. }
        ));
        assert!(
            unsafe_result.limitations.iter().any(|limit| {
                limit.location.path == root
                    && hazardous[limit.location.range.clone()].trim() == "1..n+1"
            }),
            "{:?}",
            unsafe_result.limitations
        );
    }
    std::fs::write(&root, source).unwrap();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn index_set_axes_retain_declared_bounds_and_symbolic_uncertainty() {
    let source = r#"include "all_different.mzn";
predicate written_circuit(array[int] of var int: xs) =
    if length(xs) = 0 then true
    else let {
        set of int: S = index_set(xs);
        int: l = min(S);
        int: n = card(S);
        array[S] of var 1..n: order;
    } in all_different(xs) /\
        all_different(order) /\
        forall(i in S)(xs[i] != i) /\
        order[l] = 1 /\
        forall(i in S)(order[xs[i]] = if order[i] = n then 1 else order[i] + 1 endif)
    endif;
int: node_count;
set of int: Nodes = 1..node_count;
array[Nodes] of var Nodes: successor;
constraint written_circuit(successor);
solve :: int_search(successor, input_order, indomain_min, complete) satisfy;
"#;
    let (directory, _) = model("index-set-axis", "solve satisfy;");
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function set of int: index_set(array[int] of var int: values);\n",
            "function int: length(array[int] of var int: values);\n",
            "function int: min(set of int: values); function int: card(set of int: values);\n",
            "function set of int: '..'(int: left,int: right);\n",
            "function int: '+'(int: left,int: right);\n",
        ),
    )
    .unwrap();
    std::fs::write(
        directory.join("library/std/all_different.mzn"),
        "predicate all_different(array[int] of var int: values);\n",
    )
    .unwrap();
    let root = directory.join("root.mzn");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    std::fs::write(&root, source).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let raw = resolve_domains(&context, &bindings);
    assert!(raw.array_indices.iter().any(|axis| {
        axis.location.path == root
            && &source[axis.location.range.clone()] == "S"
            && axis.domain.numeric_minimum().is_err()
    }));
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);

    let closed = concat!(
        "array[0..3] of var int: input; set of int: Axis=index_set(input);\n",
        "array[Axis] of var int: copy; solve satisfy;\n",
    );
    std::fs::write(&root, closed).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(
        result.findings.iter().any(|finding| {
            &closed[finding.location.range.clone()] == "Axis"
                && finding.message.contains("starts at 0")
        }),
        "{:?}",
        result.findings
    );

    let unsafe_source = closed.replace("0..3", "0..(9223372036854775807 + 1)");
    std::fs::write(&root, &unsafe_source).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result.limitations.iter().any(|limit| {
            &unsafe_source[limit.location.range.clone()] == "Axis"
                && limit.message.contains("overflow")
        }),
        "{:?}",
        result.limitations
    );
    assert_eq!(std::fs::read_to_string(&root).unwrap(), unsafe_source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn index_set_declared_axes_inspect_known_element_types_without_extent_proof() {
    const NATIVE: &str = concat!(
        "function set of $$E: index_set(array[$$E] of any $U: x);\n",
        "function set of int: '..'(int: left, int: right);\n",
        "function int: 'div'(int: left, int: right);\n",
    );
    let symbolic = concat!(
        "array[int] of string: input = [0: \"one\", 1: \"two\"];\n",
        "set of int: Axis = index_set(input);\n",
        "array[Axis] of var int: copy; solve satisfy;\n",
    );
    let composite = symbolic.replace(
        "array[int] of string: input = [0: \"one\", 1: \"two\"]",
        "array[0..3] of tuple(0..5, string): input",
    );
    let closed_error = composite.replace("tuple(0..5,", "tuple(0..(1 div 0),");
    let changed_body = NATIVE.replace("of any $U: x);", "of any $U: x) = 0..3;");
    let user = format!("function set of int: index_set(array[int] of string: x);\n{symbolic}");
    let alias = symbolic.replace("of string: input", "of Contents: input");
    let alias = format!("type Contents = string;\n{alias}");
    let unknown = concat!(
        "function bool: generic_axis(array[int] of any $U: input) =\n",
        "  let { set of int: Axis = index_set(input);\n",
        "        array[Axis] of int: copy; } in true; solve satisfy;\n",
    );
    let (directory, _) = model("index-set-element-types", "solve satisfy;");
    let root = directory.join("root.mzn");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    for (name, source, library, completed, expected_minimum) in [
        ("symbolic-string", symbolic, NATIVE, true, None),
        ("written-tuple", composite.as_str(), NATIVE, true, Some(0)),
        (
            "closed-element-error",
            closed_error.as_str(),
            NATIVE,
            false,
            None,
        ),
        (
            "changed-native-body",
            symbolic,
            changed_body.as_str(),
            false,
            None,
        ),
        ("user-overload", user.as_str(), NATIVE, false, None),
        (
            "uninspected-type-alias",
            alias.as_str(),
            NATIVE,
            false,
            None,
        ),
        ("unknown-element", unknown, NATIVE, false, None),
    ] {
        std::fs::write(directory.join("library/std/stdlib.mzn"), library).unwrap();
        std::fs::write(&root, source).unwrap();
        let context = load_model(&root, &options);
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let call = calls
            .calls
            .iter()
            .find(|call| Some(call.file) == context.root_file && call.name == "index_set")
            .unwrap();
        if name == "unknown-element" {
            assert!(
                matches!(call.outcome, zincite_lint::CallOutcome::Unsupported { .. }),
                "{name}: {:?}",
                call.outcome
            );
        } else {
            assert!(
                matches!(&call.outcome, zincite_lint::CallOutcome::Resolved {
                parameters, return_type, ..
            } if parameters.len() == 1
                && return_type.instantiation == zincite_lint::Instantiation::Parameter
                && matches!(&return_type.kind, zincite_lint::TypeKind::Set(element)
                    if element.kind == zincite_lint::TypeKind::Int)),
                "{name}: {:?}",
                call.outcome
            );
        }
        let result = analyze_model(&context, &selected());
        assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
        if completed {
            assert_eq!(
                result.rules[0].outcome,
                RuleOutcome::Completed,
                "INDEX_SET_ELEMENT_RED {name}: {:?}",
                result.limitations
            );
            assert!(
                result.limitations.is_empty(),
                "{name}: {:?}",
                result.limitations
            );
            let axis_findings: Vec<_> = result
                .findings
                .iter()
                .filter(|finding| {
                    finding.location.path == root
                        && &source[finding.location.range.clone()] == "Axis"
                })
                .collect();
            if let Some(minimum) = expected_minimum {
                assert_eq!(axis_findings.len(), 1, "{name}: {:?}", result.findings);
                assert!(
                    axis_findings[0]
                        .message
                        .contains(&format!("starts at {minimum}"))
                );
            } else {
                assert!(result.findings.is_empty(), "{name}: {:?}", result.findings);
            }
        } else {
            assert!(
                matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
                "{name}: {:?}",
                result
            );
            assert!(
                result.limitations.iter().any(|limit| {
                    limit.location.path == root
                        && &source[limit.location.range.clone()] == "Axis"
                        && (name != "closed-element-error"
                            || limit.message.contains("division by zero"))
                }),
                "{name}: {:?}",
                result.limitations
            );
            assert!(
                !result.findings.iter().any(|finding| {
                    finding.location.path == root
                        && &source[finding.location.range.clone()] == "Axis"
                }),
                "{name}: {:?}",
                result.findings
            );
        }
        assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn named_domains_and_symbolic_bounds_retain_identity_and_precise_advice() {
    let source = concat!(
        "\u{feff}% é\r\n",
        "type Offset = 0..3; type Alias = Offset; set of int: sparse={7,2,4};\n",
        "int: N; int: K; enum Color={red,blue};\n",
        "array[Alias,sparse,1..3,Color,int,0..N,N+1..9,K+1..9,N<..9] of int: a;\n",
        "array[(2+3)..(4*2),3..<6,0<..<4] of int: b;\n",
        "annotation tag; type Annotated :: tag = 0..2; type Indices = set of int; type MoreIndices = Indices; MoreIndices: off={0,2}; array[Annotated,off] of var int: aliases;\n",
        "int: low=2; int: high=4; set of int: Known={low,high}; array[low..high,Known,low+1..high] of int: closed;\n",
        "% zincite-lint: ignore array-index-start\narray[0..3] of int: suppressed;\nsolve satisfy;\n"
    );
    let (directory, context) = model("identity", source);
    let bindings = resolve_bindings(&context);
    let facts = resolve_domains(&context, &bindings);
    let root = directory.join("root.mzn");
    let indices: Vec<_> = facts
        .array_indices
        .iter()
        .filter(|f| f.location.path == root)
        .collect();
    assert!(
        matches!(&indices[0].domain,Domain::Named{domain,..} if matches!(domain.as_ref(),Domain::Named{..}))
    );
    assert!(matches!(
        &indices[5].domain,
        Domain::Range {
            upper: NumericBound::Symbol(_),
            ..
        }
    ));
    let symbol = |domain: &Domain| match domain {
        Domain::Range {
            lower: NumericBound::Arithmetic { operands, .. },
            ..
        } => match operands.first().unwrap() {
            NumericBound::Symbol(id) => *id,
            _ => panic!("missing identity"),
        },
        _ => panic!("missing arithmetic"),
    };
    assert_ne!(symbol(&indices[6].domain), symbol(&indices[7].domain));
    assert_eq!(symbol(&indices[6].domain), symbol(&indices[8].domain));
    let mut domain = &indices[13].domain;
    let mut aliases = Vec::new();
    while let Domain::Named {
        declaration,
        domain: target,
    } = domain
    {
        aliases.push(bindings.declarations[declaration.0].name.as_str());
        domain = target;
    }
    assert_eq!(aliases, ["off", "MoreIndices", "Indices"]);
    assert_eq!(indices[13].domain.numeric_minimum(), Ok(Some(0)));
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty());
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    let actual: Vec<_> = result
        .findings
        .iter()
        .map(|f| &source[f.location.range.clone()])
        .collect();
    assert_eq!(
        actual,
        vec![
            "Alias",
            "sparse",
            "(2+3)..(4*2)",
            "3..<6",
            "Annotated",
            "off",
            "low..high",
            "Known",
            "low+1..high"
        ]
    );
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn empty_unknown_and_unsupported_domains_have_distinct_outcomes() {
    let (directory, context) = model(
        "empty",
        concat!(
            "set of int: empty={}; int: N; type Decisions = var set of int; Decisions: chosen={0,2};\n",
            "array[3..2,1..<1,empty,0..N] of int: quiet;\n",
            "set of int: Input; array[card(Input)..9,card(Input)+1..9] of int: cardinality_unknown;\n",
            "array[1 div 0..3,9223372036854775807+1..9,1.5..3.5] of int: unsupported; solve satisfy;\n"
        ),
    );
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty());
    let bindings = resolve_bindings(&context);
    let facts = resolve_domains(&context, &bindings);
    let chosen = bindings
        .declarations
        .iter()
        .find(|d| d.name == "chosen")
        .unwrap();
    assert_eq!(
        facts.declarations[chosen.id.0].domain.numeric_minimum(),
        Ok(None)
    );
    assert_eq!(result.limitations.len(), 3, "{:?}", result.limitations);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(directory).unwrap();

    let source = concat!(
        "array[1..3] of var int: choices; set of int: Coords;\n",
        "set of int: Nodes=1..(length(choices) div length(Coords));\n",
        "set of int: Alias=Nodes; array[Alias] of int: positions; solve satisfy;\n",
    );
    let (directory, _) = model("length-quotient", source);
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function int: length(array[int] of int: values);\n",
            "function int: length(array[int] of var int: values);\n",
            "function int: 'div'(int: left,int: right);\n",
            "function set of int: '..'(int: left,int: right);\n",
        ),
    )
    .unwrap();
    let context = load_model(
        directory.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(directory.join("library")),
            include_dirs: Vec::new(),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let domains = resolve_domains(&context, &bindings);
    let alias_domain = domains
        .array_indices
        .iter()
        .find(|index| {
            index.location.path == directory.join("root.mzn")
                && &source[index.location.range.clone()] == "Alias"
        })
        .unwrap();
    assert!(
        alias_domain.domain.numeric_minimum().is_err(),
        "{:?}",
        alias_domain
    );
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    let unsafe_source = concat!(
        "array[1..3] of var int: choices; set of int: Coords; annotation tag;\n",
        "function set of int: opaque(); set of int: UnsafeCoords=opaque();\n",
        "array[1..3] of var opt int: optional_choices;\n",
        "set of int: Tagged :: tag =1..(length(choices) div length(Coords));\n",
        "set of int: Opaque=1..(length(choices) div length(UnsafeCoords));\n",
        "set of int: Optional=1..(length(optional_choices) div length(Coords));\n",
        "int: bad=1 div 0; set of int: BadCoords={bad};\n",
        "set of int: Partial=1..(length(choices) div length(BadCoords));\n",
        "array[Tagged,Opaque,Optional,Partial] of int: positions; solve satisfy;\n",
    );
    std::fs::write(directory.join("root.mzn"), unsafe_source).unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let context = load_model(directory.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        result
            .limitations
            .iter()
            .map(|limit| &unsafe_source[limit.location.range.clone()])
            .collect::<Vec<_>>(),
        ["Tagged", "Opaque", "Optional", "Partial"]
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));

    std::fs::write(
        directory.join("root.mzn"),
        format!("function int: length(set of int: values)=0;\n{source}"),
    )
    .unwrap();
    let context = load_model(directory.join("root.mzn"), &options);
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty());
    assert_eq!(result.limitations.len(), 1, "{:?}", result.limitations);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(directory).unwrap();

    let source = concat!(
        "int: ntiles; array[1..ntiles,1..5] of int: tiles;\n",
        "int: Q=1; int: S=2;\n",
        "constraint forall(t in 1..ntiles)(let {\n",
        "  int: q=tiles[t,Q]; int: s=tiles[t,S];\n",
        "  array[1..q,1..s] of int: d=array2d(1..q,1..s,[0 | i in 1..q,j in 1..s]);\n",
        "} in true); solve satisfy;\n",
    );
    let (directory, _) = model("local-selection-axis", source);
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "predicate forall(array[int] of var bool: values);\n",
            "function set of int: '..'(int: left,int: right);\n",
            "function int: '+'(int: left,int: right);\n",
            "function array[$A,$B] of int: array2d(set of $A: rows,set of $B: columns,array[int] of int: values);\n",
        ),
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let context = load_model(directory.join("root.mzn"), &options);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);

    let out_of_bounds = source.replace("q=tiles[t,Q]", "q=tiles[t,6]");
    std::fs::write(directory.join("root.mzn"), &out_of_bounds).unwrap();
    let context = load_model(directory.join("root.mzn"), &options);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert_eq!(
        result
            .limitations
            .iter()
            .map(|limit| &out_of_bounds[limit.location.range.clone()])
            .collect::<Vec<_>>(),
        ["1..q"]
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    for (before, after) in [
        ("of int: tiles", "of 0..(9223372036854775807+1): tiles"),
        ("1..ntiles,1..5", "1..ntiles,1..(9223372036854775807+1)"),
        (
            "forall(t in 1..ntiles)",
            "forall(t in 1..(9223372036854775807+1))",
        ),
    ] {
        assert!(source.contains(before));
        let hazardous = source
            .replace(before, after)
            .replace("q=tiles[t,Q]", "q=tiles[t,1]");
        std::fs::write(directory.join("root.mzn"), &hazardous).unwrap();
        let context = load_model(directory.join("root.mzn"), &options);
        let result = analyze_model(&context, &selected());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(
            result
                .limitations
                .iter()
                .any(|limit| &hazardous[limit.location.range.clone()] == "1..q"),
            "A closed source failure must keep the local axis Limited: {after}: {:?}",
            result.limitations
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn symbolic_cardinality_sum_axes_require_inspected_sources() {
    use zincite_lint::{
        CallOutcome, DefinitionCoverage, DefinitionEnforcement, DefinitionSafety, Instantiation,
        SourceKind, TypeKind,
    };

    let source = concat!(
        "% Data.\n",
        "int: n;\n",
        "set of int: Nodes = 0..n-1;\n",
        "array[Nodes] of set of int: edges;\n\n",
        "% A symbolic cardinality sum determines the parameter table's first axis.\n",
        "int: extent = sum(i in Nodes)(card(edges[i])) + n;\n",
        "array[1..extent,1..2] of int: rows;\n",
        "solve satisfy;\n",
    );
    let (directory, _) = model("cardinality-sum-axis", source);
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function int: card(set of int: values);\n",
            "function int: sum(array[$T] of int: values);\n",
            "function int: '+'(int: left,int: right);\n",
            "function int: '-'(int: left,int: right);\n",
            "function set of int: '..'(int: left,int: right);\n",
        ),
    )
    .unwrap();
    let root = directory.join("root.mzn");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let instantiations = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &instantiations, &domains);
    let id = |name: &str| {
        bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == name)
            .unwrap()
            .id
    };
    let parameter_int = |ty: &zincite_lint::TypeInst| {
        !ty.optional && ty.instantiation == Instantiation::Parameter && ty.kind == TypeKind::Int
    };
    for name in ["sum", "card"] {
        let start = source.find(&format!("{name}(")).unwrap();
        let call = calls
            .calls
            .iter()
            .find(|call| call.location.path == root && call.location.range.start == start)
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &call.outcome
        else {
            panic!(
                "the source prerequisite must select core {name}: {:?}",
                call.outcome
            );
        };
        let selected = &bindings.declarations[declaration.0];
        assert_eq!(selected.name, name);
        assert_eq!(
            context.files[selected.file].kind,
            SourceKind::StandardLibrary
        );
        assert!(context.files[selected.file].implicit);
        assert_eq!(parameters.len(), 1);
        assert!(parameter_int(return_type));
        assert!(!parameters[0].optional);
        assert_eq!(parameters[0].instantiation, Instantiation::Parameter);
        assert!(match (&parameters[0].kind, name) {
            (TypeKind::Array { indices, element }, "sum") => {
                indices.len() == 1 && parameter_int(&indices[0]) && parameter_int(element)
            }
            (TypeKind::Set(element), "card") => parameter_int(element),
            _ => false,
        });
    }
    assert!(matches!(&domains.declarations[id("edges").0].domain,
        Domain::Array { indices, .. } if matches!(&indices[0], Domain::Named { declaration, .. }
            if *declaration == id("Nodes"))));
    let axis = domains
        .array_indices
        .iter()
        .find(|axis| {
            axis.location.path == root && &source[axis.location.range.clone()] == "1..extent"
        })
        .unwrap();
    assert!(
        matches!(&axis.domain, Domain::Range { upper: NumericBound::Defined { declaration, value }, .. }
        if *declaration == id("extent") && matches!(value.as_ref(), NumericBound::Unsupported(_)))
    );
    assert!(axis.domain.numeric_minimum().is_err());
    let extent = definitions
        .definitions
        .iter()
        .find(|definition| definition.target == id("extent"))
        .unwrap();
    assert_eq!(extent.instantiation, Instantiation::Parameter);
    assert_eq!(extent.coverage, DefinitionCoverage::Scalar);
    assert_eq!(extent.enforcement, DefinitionEnforcement::Enforced);
    assert!(!extent.cyclic);
    // Check the source prerequisite before the owning rule: a consumer cannot
    // make this axis Completed by ignoring an Unsupported initializer.
    assert!(
        matches!(
            extent.safety,
            DefinitionSafety::Supported | DefinitionSafety::Unknown(_)
        ),
        "the complete symbolic source must be inspected first: {extent:?}"
    );
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);

    // A symbolic sum must not hide a closed overflow in a referenced source.
    let hazardous = source
        .replace("int: n;", "int: n; int: invalid = 9223372036854775807+1;")
        .replace("card(edges[i])) + n;", "card(edges[i])) + n + invalid;");
    std::fs::write(&root, &hazardous).unwrap();
    let context = load_model(&root, &options);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert!(
        result
            .limitations
            .iter()
            .any(|limit| &hazardous[limit.location.range.clone()] == "1..extent"),
        "{:?}",
        result.limitations
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn symbolic_union_axes_preserve_closed_minima_and_partial_traversal() {
    use zincite_lint::{
        BindingResolution, CallOutcome, DeclarationRole, DefinitionCoverage, DefinitionEnforcement,
        DefinitionSafety, Instantiation, ReferenceKind, SourceKind, TypeInst, TypeKind,
        resolve_callables, resolve_definitions, resolve_instantiations,
    };
    let source = r#"% Public reduction of a symbolic union used as an array axis and element domain.

% Data
int: time_horizon;
int: inner_size;

% Validation
constraint :: "Positive dimensions"
    assert(time_horizon >= 1 /\ inner_size >= 1, "Dimensions must be positive");

% Derived domains
set of int: Time = 0..time_horizon - 1;
set of int: Inner = 0..inner_size - 1;
set of int: Dummy = -2..-1;
set of int: Places = Inner union Dummy;

% Decision variables
array[Time, Places] of var 0..1: height;
array[Time, Places] of var Places: next_position;

% Dummy positions have no height and retain their own position.
constraint :: "Dummy positions stay fixed"
    forall (t in Time, p in Dummy) (
        height[t,p] = 0 /\ next_position[t,p] = p
    );

solve :: seq_search([
    int_search(next_position, first_fail, indomain_min, complete),
    int_search(height, first_fail, indomain_min, complete)
]) satisfy;
"#;
    let (directory, _) = model("symbolic-union-axis", source);
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function set of $T: 'union'(set of $T: x,set of $T: y);\n",
            "function set of $$E: '..'($$E: a,$$E: b);\n",
            "function int: '-'(int: x,int: y); function int: '-'(int: x);\n",
            "function int: '+'(int: x,int: y); function bool: '>='($T: x,$T: y);\n",
            "function bool: '/\\'(bool: x,bool: y); function var bool: '/\\'(var bool: x,var bool: y);\n",
            "function bool: '='($T: x,$T: y); function var bool: '='(any $T: x,any $T: y);\n",
            "function var bool: forall(array[$T] of var bool: x);\n",
            "function bool: assert(bool: b,string: msg);\n",
            "function ann: int_search(array[$X] of var $$E: x,ann: select,ann: choice,ann: explore);\n",
            "annotation seq_search(array[int] of ann: s);\n",
            "annotation first_fail; annotation indomain_min; annotation complete;\n",
        ),
    )
    .unwrap();
    // This data supports the compiler precheck; Zincite loads only the model.
    std::fs::write(
        directory.join("instance.dzn"),
        "time_horizon = 2;\ninner_size = 3;\n",
    )
    .unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let context = load_model(directory.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let integer = |ty: &TypeInst| {
        !ty.optional && ty.instantiation == Instantiation::Parameter && ty.kind == TypeKind::Int
    };
    let integer_set = |ty: &TypeInst| {
        !ty.optional
            && ty.instantiation == Instantiation::Parameter
            && matches!(&ty.kind, TypeKind::Set(element) if integer(element))
    };
    let union_start = source.find("Inner union Dummy").unwrap();
    let union = calls
        .calls
        .iter()
        .find(|call| call.file == 0 && call.location.range.start == union_start + "Inner ".len())
        .unwrap();
    assert!(
        matches!(&union.outcome,
            CallOutcome::Resolved { declaration, parameters, return_type }
                if bindings.declarations[declaration.0].name == "union"
                    && context.files[bindings.declarations[declaration.0].file].kind
                        == SourceKind::StandardLibrary
                    && context.files[bindings.declarations[declaration.0].file].implicit
                    && parameters.len() == 2 && parameters.iter().all(integer_set)
                    && integer_set(return_type)
        ),
        "{:?}",
        union.outcome
    );
    for (name, range) in [
        ("Inner", union_start..union_start + 5),
        ("Dummy", union_start + 12..union_start + 17),
    ] {
        let reference = bindings
            .references
            .iter()
            .find(|reference| {
                reference.file == 0
                    && reference.location.range == range
                    && reference.kind == ReferenceKind::Value
            })
            .unwrap();
        let target = bindings
            .declarations
            .iter()
            .find(|declaration| {
                declaration.file == 0
                    && declaration.top_level
                    && declaration.role == DeclarationRole::Value
                    && declaration.name == name
            })
            .unwrap()
            .id;
        assert_eq!(
            reference.resolution,
            BindingResolution::Resolved(target),
            "written union operand: {name} at {range:?}"
        );
        let declared = &calls.declarations[target.0];
        assert_eq!(declared.declaration, target);
        assert!(
            integer_set(&declared.ty),
            "written union operand: {name}: {:?}",
            declared.ty
        );
    }
    // The generator has its real present Boolean body, not an opaque predicate.
    let body_start = source.find("height[t,p] = 0").unwrap();
    let body_end = source.find("next_position[t,p] = p").unwrap() + "next_position[t,p] = p".len();
    assert!(calls.expressions.iter().any(|expression| {
        expression.file == 0
            && expression.location.range == (body_start..body_end)
            && !expression.ty.optional
            && expression.ty.kind == TypeKind::Bool
            && expression.ty.instantiation == Instantiation::Decision
    }));
    assert!(
        calls.calls.iter().all(|call| matches!(
            call.outcome,
            CallOutcome::Resolved { .. } | CallOutcome::Intrinsic { .. }
        )),
        "{:?}",
        calls.calls
    );
    let rules = LintOptions {
        rules: vec![
            Rule::ArrayIndexStart,
            Rule::UnboundedVariable,
            Rule::ConstantVariable,
        ],
        ..LintOptions::default()
    };
    let result = analyze_model(&context, &rules);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    // First expected RED: the same three consumers limited by the accepted shape.
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules.len(), 3);
    assert!(
        result
            .rules
            .iter()
            .all(|rule| rule.outcome == RuleOutcome::Completed)
    );
    let places_axes: Vec<_> = source
        .match_indices("array[Time, Places]")
        .map(|(start, _)| {
            let start = start + "array[Time, ".len();
            start..start + "Places".len()
        })
        .collect();
    assert_eq!(result.findings.len(), 2, "{:?}", result.findings);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|finding| finding.location.range.clone())
            .collect::<Vec<_>>(),
        places_axes
    );
    assert!(
        result
            .findings
            .iter()
            .all(|finding| finding.rule == Rule::ArrayIndexStart
                && finding.location.path == directory.join("root.mzn")
                && finding.message.contains("starts at -2"))
    );
    let domains = resolve_domains(&context, &bindings);
    let time = bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "Time")
        .unwrap();
    assert_eq!(
        domains.declarations[time.id.0].domain.numeric_minimum(),
        Ok(None)
    );
    let positions = bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "next_position")
        .unwrap();
    let places = bindings
        .declarations
        .iter()
        .find(|d| d.top_level && d.name == "Places")
        .unwrap();
    assert!(matches!(&domains.declarations[positions.id.0].domain,
        Domain::Array { element, .. } if matches!(element.as_ref(),
            Domain::Named { declaration, .. } if *declaration == places.id)));
    let instantiations = resolve_instantiations(&context, &bindings, &calls);
    let definitions = resolve_definitions(&context, &bindings, &calls, &instantiations, &domains);
    for name in ["height", "next_position"] {
        let target = bindings
            .declarations
            .iter()
            .find(|d| d.top_level && d.name == name)
            .unwrap()
            .id;
        assert!(
            !definitions.definitions.iter().any(|definition| {
                definition.target == target
                    && definition.enforcement == DefinitionEnforcement::Enforced
                    && definition.coverage == DefinitionCoverage::WholeArray
                    && definition.safety == DefinitionSafety::Supported
            }),
            "Dummy traversal cannot certify all Places: {name}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(directory.join("root.mzn")).unwrap(),
        source
    );

    // A symbolic sibling must not hide a closed overflow in the union source.
    let hazardous = source.replace(
        "set of int: Dummy = -2..-1;",
        "set of int: Dummy = -2..(9223372036854775807 + 1);",
    );
    assert_ne!(hazardous, source);
    std::fs::write(directory.join("root.mzn"), &hazardous).unwrap();
    let context = load_model(directory.join("root.mzn"), &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &rules);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(
        result
            .rules
            .iter()
            .any(|rule| rule.rule == Rule::ArrayIndexStart
                && matches!(rule.outcome, RuleOutcome::Limited { .. })),
        "{:?}",
        result.rules
    );
    assert!(
        result.limitations.iter().any(|limit| {
            limit.location.path == directory.join("root.mzn")
                && &hazardous[limit.location.range.clone()] == "Places"
                && limit.message.starts_with("array-index-start:")
        }),
        "{:?}",
        result.limitations
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn grouping_floor_sum_bounds_inspect_unknown_inputs_and_closed_divisor_errors() {
    use zincite_lint::CallOutcome;

    let source = concat!(
        "array[int] of int: values;\n",
        "int: denominator = 335;\n",
        "int: extent = floor(sum(values) / denominator);\n",
        "set of int: Slots = 1..extent;\n",
        "set of int: Choices = {0} union Slots;\n",
        "array[Slots] of var 0..1: slots;\n",
        "array[Choices] of var Choices: choices;\n",
        "solve satisfy;\n",
    );
    let (directory, _) = model("grouping-floor-sum-bound", "solve satisfy;\n");
    std::fs::write(
        directory.join("library/std/stdlib.mzn"),
        concat!(
            "function int: '+'(int: left,int: right);\n",
            "function set of int: '..'(int: left,int: right);\n",
            "function set of int: 'union'(set of int: left,set of int: right);\n",
            "annotation promise_commutative;\n",
            "function int: sum(array[$T] of int: values) :: promise_commutative;\n",
            "function float: '/'(float: left,float: right);\n",
            "function int: floor(float: value);\n",
        ),
    )
    .unwrap();
    let root = directory.join("root.mzn");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let rules = LintOptions {
        rules: vec![Rule::ArrayIndexStart, Rule::UnboundedVariable],
        ..LintOptions::default()
    };
    for (divisor, error) in [
        ("335", None),
        ("0", Some("zero")),
        ("(9223372036854775807+1)", Some("overflow")),
    ] {
        let candidate = source.replace("denominator = 335", &format!("denominator = {divisor}"));
        std::fs::write(&root, &candidate).unwrap();
        let context = load_model(&root, &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        assert!(context.limitations.is_empty(), "{:?}", context.limitations);
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty()),
            "{:?}",
            context.files
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        assert!(
            calls
                .calls
                .iter()
                .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
            "{:?}",
            calls.calls
        );
        let result = analyze_model(&context, &rules);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(result.rules.len(), 2);
        if let Some(error) = error {
            assert!(
                result
                    .rules
                    .iter()
                    .all(|rule| matches!(rule.outcome, RuleOutcome::Limited { .. }))
            );
            let slots_axis = candidate.find("array[Slots]").unwrap() + "array[".len();
            let choices = bindings
                .declarations
                .iter()
                .find(|declaration| {
                    declaration.location.path == root
                        && declaration.top_level
                        && declaration.name == "choices"
                })
                .unwrap();
            for (prefix, range) in [
                ("array-index-start:", slots_axis..slots_axis + "Slots".len()),
                ("unbounded-variable:", choices.location.range.clone()),
            ] {
                assert!(
                    result.limitations.iter().any(|limit| {
                        limit.location.path == root
                            && limit.location.range == range
                            && limit.message.starts_with(prefix)
                            && limit.message.contains(error)
                    }),
                    "{divisor}: {:?}",
                    result.limitations
                );
            }
            assert!(result.findings.is_empty(), "{:?}", result.findings);
        } else {
            assert!(
                result
                    .rules
                    .iter()
                    .all(|rule| rule.outcome == RuleOutcome::Completed),
                "{:?}",
                result.rules
            );
            assert!(result.limitations.is_empty(), "{:?}", result.limitations);
            // Slots can be empty: its unobserved upper bound cannot prove its minimum.
            // Choices always contains 0, independently of the unknown array contents.
            let choices_axis = candidate.find("array[Choices]").unwrap() + "array[".len();
            assert_eq!(result.findings.len(), 1, "{:?}", result.findings);
            assert_eq!(result.findings[0].rule, Rule::ArrayIndexStart);
            assert_eq!(result.findings[0].location.path, root);
            assert_eq!(
                result.findings[0].location.range,
                choices_axis..choices_axis + "Choices".len()
            );
            assert!(result.findings[0].message.contains("starts at 0"));
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
