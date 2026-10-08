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
