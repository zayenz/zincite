use std::path::PathBuf;
use zincite_lint::{
    Domain, LintOptions, ModelOptions, NumericBound, Rule, RuleOutcome, analyze_model, load_model,
    resolve_bindings, resolve_domains,
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
