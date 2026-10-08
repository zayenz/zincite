use std::path::{Path, PathBuf};
use zincite_lint::{
    BindingFacts, DefinitionCoverage, DefinitionEnforcement, DefinitionFacts, DefinitionSafety,
    LintOptions, ModelOptions, RuleOutcome, analyze_model, load_model, resolve_bindings,
    resolve_callables, resolve_definitions, resolve_domains, resolve_instantiations,
};
fn write(path: &Path, source: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}
fn model(name: &str, source: &str, included: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let directory =
        std::env::temp_dir().join(format!("zincite-definitions-{name}-{}", std::process::id()));
    write(
        &directory.join("library/std/stdlib.mzn"),
        concat!(
            "function var bool: forall(array[int] of var opt bool: body);\n",
            "function var bool: symmetry_breaking_constraint(var bool: body)=body;\n",
            "function array[int] of int: reverse(array[int] of int: values);\n",
        ),
    );
    write(&directory.join("included.mzn"), included);
    write(&directory.join("root.mzn"), source);
    let context = load_model(
        directory.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(directory.join("library")),
            include_dirs: Vec::new(),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (directory, context)
}
fn facts(context: &zincite_lint::ModelContext) -> (BindingFacts, DefinitionFacts) {
    let bindings = resolve_bindings(context);
    let calls = resolve_callables(context, &bindings);
    let instantiations = resolve_instantiations(context, &bindings, &calls);
    let domains = resolve_domains(context, &bindings);
    let definitions = resolve_definitions(context, &bindings, &calls, &instantiations, &domains);
    (bindings, definitions)
}
fn selected() -> LintOptions {
    LintOptions::from_selection("constant-variable").unwrap()
}

#[test]
fn whole_definitions_retain_identity_ranges_dependencies_and_item_suppression() {
    let source = concat!(
        "\u{feff}% é\r\ninclude \"included.mzn\"; int: symbolic; int: N; type Decision=var int;\n",
        "Decision: initialized=symbolic; array[int] of var int: whole=[1,2]; var int: arithmetic=6 div 2;\n",
        "var int: scalar; var int: reverse; var int: wrapped;\n",
        "function var bool: forward(var bool: body,int: ignored)=body;\n",
        "constraint (scalar=symbolic) /\\ (2=reverse);\n",
        "constraint forward(ignored: 2,body: (wrapped=symbolic));\n",
        "var int: default_target; predicate default_forward(var bool: body=(default_target=symbolic))=body; constraint default_forward();\n",
        "set of int: Sparse={1,3}; array[Sparse,1..N] of var int: grid;\n",
        "constraint forall(j in 1..N,i in Sparse)(grid[i,j]=j);\n",
        "array[1..3] of var int: reordered; constraint forall(i in {3,1,2,2})(reordered[i]=i);\n",
        "array[1..100000000000] of var int: large; constraint forall(i in 1..99999999999+1)(large[i]=1);\n",
        "enum Color={red,blue}; array[Color] of var int: colors; constraint forall(c in Color)(colors[c]=1);\n",
        "constraint included_target=symbolic; constraint scalar=3;\n",
        "% zincite-lint: ignore constant-variable\nvar int: suppressed=2; solve satisfy;\n",
    );
    let included = "var int: included_target;\n% zincite-lint: ignore constant-variable\nvar int: locally_suppressed=1;";
    let (directory, context) = model("positive", source, included);
    let (bindings, definitions) = facts(&context);
    let grid = bindings
        .declarations
        .iter()
        .find(|d| d.name == "grid")
        .unwrap()
        .id;
    let grid_definition = definitions
        .definitions
        .iter()
        .find(|d| d.target == grid)
        .unwrap();
    assert_eq!(grid_definition.coverage, DefinitionCoverage::WholeArray);
    assert!(
        grid_definition.enforcement == DefinitionEnforcement::Enforced && !grid_definition.cyclic
    );
    assert_eq!(grid_definition.safety, DefinitionSafety::Supported);
    let suppressed = bindings
        .declarations
        .iter()
        .find(|d| d.name == "suppressed")
        .unwrap()
        .id;
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| d.target == suppressed && d.enforcement == DefinitionEnforcement::Enforced)
    );
    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    let targets: Vec<_> = result
        .findings
        .iter()
        .map(|f| f.message.split('\'').nth(1).unwrap())
        .collect();
    assert_eq!(
        targets,
        [
            "initialized",
            "whole",
            "arithmetic",
            "scalar",
            "reverse",
            "wrapped",
            "default_target",
            "grid",
            "reordered",
            "large",
            "colors",
            "included_target"
        ]
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.location.path == directory.join("root.mzn"))
    );
    let initializer = definitions
        .definitions
        .iter()
        .find(|d| bindings.declarations[d.target.0].name == "initialized")
        .unwrap();
    assert_eq!(&source[initializer.value.range.clone()], "symbolic");
    assert_eq!(
        bindings.declarations[initializer.dependencies[0].0].name,
        "symbolic"
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(directory).unwrap();

    let source = concat!(
        "var int: cycle_seed=1; var int: cycle_second; var int: cycle_third;\n",
        "constraint cycle_seed=cycle_second /\\ cycle_second=cycle_third /\\ cycle_third=cycle_seed;\n",
        "var int: diamond_left=cycle_seed; var int: diamond_right=cycle_seed;\n",
        "var int: diamond_top=diamond_left+diamond_right;\n",
        "function var bool: conditional()=cycle_third=diamond_top; solve satisfy;\n",
    );
    let (directory, context) = model("cycle-rows", source, "");
    let (bindings, definitions) = facts(&context);
    let seed_flags: Vec<_> = definitions
        .definitions
        .iter()
        .filter(|d| bindings.declarations[d.target.0].name == "cycle_seed")
        .map(|d| d.cyclic)
        .collect();
    assert_eq!(seed_flags, [false, true, true]);
    for name in ["cycle_second", "cycle_third"] {
        assert!(
            definitions
                .definitions
                .iter()
                .filter(|d| bindings.declarations[d.target.0].name == name)
                .all(|d| d.cyclic)
        );
    }
    assert!(definitions.definitions.iter().any(|d| {
        bindings.declarations[d.target.0].name == "cycle_third"
            && d.enforcement == DefinitionEnforcement::Conditional
            && d.cyclic
    }));
    for name in ["diamond_left", "diamond_right", "diamond_top"] {
        assert!(
            definitions
                .definitions
                .iter()
                .any(|d| bindings.declarations[d.target.0].name == name && !d.cyclic)
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn conditional_partial_symbolic_and_cyclic_definitions_do_not_prove_whole_values() {
    let source = concat!(
        "int: N; int: K; var int: disjoined; var int: implied; var int: opaque_target; var int: switchable; var int: user_target;\n",
        "function var bool: opaque(var bool: body)=true; function var bool: forall(var bool: body,int: ignored)=true;\n",
        "constraint disjoined=1 \\/ true; constraint true -> implied=1; constraint opaque(opaque_target=1);\n",
        "constraint symmetry_breaking_constraint(switchable=1); constraint forall(user_target=1,2);\n",
        "array[1..N] of var int: mismatched; constraint forall(i in 1..K)(mismatched[i]=1);\n",
        "array[1..3] of var int: filtered; constraint forall(i in 1..3 where i>1)(filtered[i]=1);\n",
        "array[1..3] of var int: partial; constraint forall(i in 1..3)(partial[1]=1);\n",
        "array[1..3] of var int: extra; constraint forall(i in 1..3,j in {})(extra[i]=1);\n",
        "var int: vacuous; constraint forall(i in {})(vacuous=1);\n",
        "constraint let {int: disjoined=1;} in disjoined=1;\n",
        "var int: cycle_a; var int: cycle_b; constraint cycle_a=cycle_b /\\ cycle_b=cycle_a;\n",
        "array[1..3] of var int: self; constraint forall(i in 1..3)(self[i]=self[i]); solve satisfy;\n",
    );
    let (directory, context) = model("negative", source, "");
    let (bindings, definitions) = facts(&context);
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty(), "{:?}", result.findings);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    for name in ["cycle_a", "cycle_b", "self"] {
        assert!(
            definitions
                .definitions
                .iter()
                .any(|d| bindings.declarations[d.target.0].name == name && d.cyclic)
        );
    }
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| bindings.declarations[d.target.0].name == "mismatched"
                && matches!(d.coverage, DefinitionCoverage::Unproved(_)))
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unsupported_value_safety_limits_only_constant_variable() {
    let source = "var opt int: optional=<>; function int: value()=1; var int: opaque=value(); var int: partial=1 div 0; var int: known=2; var MissingType: unsupported_target=1; int: divisor; var int: symbolic_partial=1 div divisor; var int: unknown_enforcement; constraint missing(unknown_enforcement=1); array[1..3] of var int: unknown_array; constraint forall(i in missing_domain())(unknown_array[i]=1); solve satisfy;";
    let (directory, context) = model("unsupported", source, "");
    let options =
        LintOptions::from_selection("constant-variable,global-variable-in-function").unwrap();
    let result = analyze_model(&context, &options);
    assert_eq!(result.findings.len(), 1, "{:?}", result.findings);
    assert_eq!(result.limitations.len(), 6, "{:?}", result.limitations);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(result.rules[1].outcome, RuleOutcome::Completed);
    assert_eq!(result.status(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn boundary_union_comprehensions_do_not_prove_whole_arrays() {
    let source = r#"% Symbolic parameters remain unassigned during lint analysis.
int: row_count;
int: column_count;
int: step_count;

set of int: Rows = 0..row_count - 1;
set of int: Columns = 0..column_count - 1;
set of int: Cells = 0..row_count * column_count - 1;
set of int: Outside = -2..-1;
set of int: Places = Cells union Outside;
set of int: Steps = 0..step_count - 1;

array[Rows, Columns] of Cells: grid = array2d(Rows, Columns,
    [r * column_count + c | r in Rows, c in Columns]
);
set of Cells: Border = {grid[r, 0] | r in Rows}
    union {grid[r, column_count - 1] | r in Rows};

array[Steps, Places] of var 0..1: levels;
array[Steps, Places] of var 0..1: fixed_levels;

% This selects a boundary subset, not every place in the declared array.
constraint :: "Boundary cells remain empty"
forall (t in Steps, p in Border) (
    levels[t, p] = 0
);

% A same-axis control must keep its whole-array constant-variable advice.
constraint :: "Every fixed level is empty"
forall (t in Steps, p in Places) (
    fixed_levels[t, p] = 0
);

solve satisfy;
"#;
    let (directory, _) = model("boundary-union", source, "");
    write(
        &directory.join("library/std/stdlib.mzn"),
        concat!(
            "function set of $T: 'union'(set of $T: x,set of $T: y);\n",
            "function set of $$E: '..'($$E: x,$$E: y);\n",
            "function int: '-'(int: x,int: y); function int: '-'(int: x);\n",
            "function int: '+'(int: x,int: y); function int: '*'(int: x,int: y);\n",
            "function var bool: '='(any $T: x,any $T: y);\n",
            "function var bool: forall(array[$T] of var bool: body);\n",
            "function array[$$E,$$F] of any $V: array2d(set of $$E: rows,set of $$F: columns,array[$U] of any $V: values);\n",
        ),
    );
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        include_dirs: Vec::new(),
    };
    let root = directory.join("root.mzn");
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|finding| &source[finding.location.range.clone()])
            .collect::<Vec<_>>(),
        ["fixed_levels[t, p] = 0"],
    );
    assert_eq!(result.findings[0].location.path, root);
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);

    // Equal element types do not prove equality of named index domains.
    for (written, replacement) in [
        (
            "array[Steps, Places] of var 0..1: fixed_levels;",
            "array[Columns, Places] of var 0..1: fixed_levels;",
        ),
        (
            "p in Places) (\n    fixed_levels",
            "p in Rows) (\n    fixed_levels",
        ),
    ] {
        let partial = source.replace(written, replacement);
        assert_ne!(partial, source);
        write(&root, &partial);
        let context = load_model(&root, &options);
        assert!(context.errors.is_empty(), "{:?}", context.errors);
        let result = analyze_model(&context, &selected());
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert!(result.limitations.is_empty(), "{:?}", result.limitations);
        assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    }

    // A symbolic generator must not conceal closed overflow in its head.
    let hazardous = source.replace("grid[r, 0]", "9223372036854775807 + 1");
    write(&root, &hazardous);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(&context, &selected());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result.limitations.iter().any(|limitation| {
            limitation.location.path == root
                && &hazardous[limitation.location.range.clone()] == "levels[t, p] = 0"
                && limitation.message.contains("integer arithmetic overflow")
        }),
        "{:?}",
        result.limitations,
    );
    assert_eq!(
        result
            .findings
            .iter()
            .map(|finding| &hazardous[finding.location.range.clone()])
            .collect::<Vec<_>>(),
        ["fixed_levels[t, p] = 0"],
    );
    std::fs::remove_dir_all(directory).unwrap();
}
