use std::path::PathBuf;
use zincite_lint::{
    Domain, LintOptions, ModelOptions, NumericBound, Rule, RuleOutcome, analyze_model, load_model,
    resolve_bindings, resolve_domains,
};

fn model(name: &str, source: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let directory =
        std::env::temp_dir().join(format!("zincite-domains-{name}-{}", std::process::id()));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(directory.join("library/std/stdlib.mzn"), "").unwrap();
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
}
