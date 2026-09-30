use std::path::{Path, PathBuf};
use zincite_lint::{
    BindingFacts, CallOutcome, CallableFacts, DeclarationRole, Instantiation, LintOptions,
    ModelOptions, Rule, RuleOutcome, TypeKind, analyze_file, analyze_model, lint_with_options,
    load_model, resolve_bindings, resolve_callables,
};
use zincite_syntax::{FileMode, parse, parse_with_mode};

const ELEMENT: &str = concat!(
    "predicate element(var $$E: i, array[$$E] of var bool: x, var bool: y) = y = x[i];\n",
    "predicate element(var $$E: i, array[$$E] of var float: x, var float: y) = y = x[i];\n",
    "predicate element(var $$E: i, array[$$E] of var $$T: x, var $$T: y) = y = x[i];\n",
    "predicate element(var $$E: i, array[$$E] of var set of $$T: x, var set of $$T: y) = y = x[i];\n",
);
fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}
fn setup(name: &str) -> (PathBuf, ModelOptions) {
    let directory =
        std::env::temp_dir().join(format!("zincite-callables-{name}-{}", std::process::id()));
    let library = directory.join("library");
    write(
        &library.join("std/stdlib.mzn"),
        concat!(
            "function any $T: element(int: idx, array[int] of any $T: xs) = xs[idx];\n",
            "function any $T: element(int: row, int: col, array[int,int] of any $T: xs) = xs[row,col];\n",
        ),
    );
    write(&library.join("std/element.mzn"), ELEMENT);
    (
        directory,
        ModelOptions {
            stdlib_dir: Some(library),
            include_dirs: Vec::new(),
        },
    )
}
fn outcome<'a>(
    facts: &'a CallableFacts,
    path: &Path,
    source: &str,
    marker: &str,
) -> &'a CallOutcome {
    let start = source.find(marker).unwrap();
    &facts
        .calls
        .iter()
        .find(|c| c.location.path == path && c.location.range.start == start)
        .unwrap()
        .outcome
}
fn resolved<'a>(
    facts: &'a CallableFacts,
    bindings: &'a BindingFacts,
    path: &Path,
    source: &str,
    marker: &str,
) -> &'a zincite_lint::Declaration {
    let CallOutcome::Resolved { declaration, .. } = outcome(facts, path, source, marker) else {
        panic!("{marker}: {:?}", outcome(facts, path, source, marker));
    };
    &bindings.declarations[declaration.0]
}

#[test]
fn public_callable_facts_preserve_types_and_select_standard_identity_before_advice() {
    let (directory, options) = setup("types");
    let root = directory.join("root.mzn");
    let included = directory.join("included.mzn");
    let source = concat!(
        "include \"element.mzn\"; include \"included.mzn\";\n",
        "enum Index = {I1,I2}; enum Value = {V1,V2}; type IX = Index; type VX = var Value;\n",
        "array[IX] of VX: values; var IX: index; VX: value;\n",
        "array[int] of var int: ints; var int: iv; array[int] of var bool: bools; var bool: bv;\n",
        "array[int] of var float: floats; var float: fv; array[int] of var set of int: sets; var set of int: sv;\n",
        "constraint element(i:index,x:values,y:value); constraint element(1,ints,iv);\n",
        "constraint element(1,bools,bv); constraint element(1,floats,fv); constraint element(1,sets,sv);\n",
        "function any $T: identity(any $T: x) = x;\n",
        "function $T: defaulted($T: x=1)=x; int: defaulted_value=defaulted(); type CycleA=CycleB; type CycleB=CycleA;\n",
        "predicate consume(var int: x, int: count=1) = x>count;\n",
        "predicate fields(record(int: a,var int: b): x)=true; constraint fields((b:iv,a:1));\n",
        "array[int] of int: fixed_values; predicate fixed(int: x)=true; predicate fixed(var int: x)=true; constraint fixed(fixed_values[iv]);\n",
        "predicate numeric(int: x)=true; constraint numeric(-V1); constraint numeric(V1+V2);\n",
        "record(var int: value, int: count): rec; tuple(int,var int): pair;\n",
        "constraint consume(x:identity(rec.value)); constraint consume(pair.2,2);\n",
        "constraint let {var int: value;} in consume(value);\n",
        "var int: function_result = element(1,ints);\n",
        "array[int,int] of var int: matrix; var int: matrix_result = element(1,1,matrix);\n",
        "array[int] of var opt int: optional; var opt int: ov; constraint element(1,optional,ov);\n",
        "array[int] of tuple(var int,var int): structured; tuple(var int,var int): tv; constraint element(1,structured,tv);\n",
        "var set of int: domain; constraint element(1,[i|i in domain],iv);\n",
        "constraint element(1,[i|i in 1..2 where bv],iv); solve satisfy;\n",
    );
    let shared = "\u{feff}% é\r\nconstraint element(1,ints,iv);\r\n% zincite-lint: ignore element-predicate\r\nconstraint element(1,ints,iv);\r\n";
    write(&root, source);
    write(&included, shared);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let facts = resolve_callables(&context, &bindings);
    for marker in [
        "element(i:index",
        "element(1,ints,iv)",
        "element(1,bools",
        "element(1,floats",
        "element(1,sets",
    ] {
        let declaration = resolved(&facts, &bindings, &root, source, marker);
        assert_eq!(declaration.role, DeclarationRole::Predicate);
        assert_eq!(
            context.files[declaration.file].canonical_path,
            options
                .stdlib_dir
                .as_ref()
                .unwrap()
                .join("std/element.mzn")
                .canonicalize()
                .unwrap()
        );
    }
    for marker in ["element(1,ints);", "element(1,1,matrix)"] {
        assert_eq!(
            resolved(&facts, &bindings, &root, source, marker).role,
            DeclarationRole::Function
        );
    }
    for marker in [
        "element(1,optional",
        "element(1,structured",
        "element(1,[i|i in domain]",
        "element(1,[i|i in 1..2 where bv]",
    ] {
        assert!(
            matches!(
                outcome(&facts, &root, source, marker),
                CallOutcome::NoMatch { .. }
            ),
            "{marker}: {:?}",
            outcome(&facts, &root, source, marker)
        );
    }
    let index = bindings
        .declarations
        .iter()
        .find(|d| d.name == "Index")
        .unwrap();
    let values = bindings
        .declarations
        .iter()
        .find(|d| d.name == "values")
        .unwrap();
    let TypeKind::Array { indices, element } = &facts.declarations[values.id.0].ty.kind else {
        panic!("array fact missing");
    };
    assert_eq!(indices[0].kind, TypeKind::Enum(index.id));
    assert_eq!(element.instantiation, Instantiation::Decision);
    for marker in ["consume(x:identity", "consume(pair.2", "consume(value)"] {
        assert_eq!(
            resolved(&facts, &bindings, &root, source, marker).name,
            "consume"
        );
    }
    assert_eq!(
        resolved(&facts, &bindings, &root, source, "fields((b:iv,a:1))").name,
        "fields"
    );
    let fixed = resolved(&facts, &bindings, &root, source, "fixed(fixed_values[iv])");
    assert_eq!(
        facts
            .signatures
            .iter()
            .find(|s| s.declaration == fixed.id)
            .unwrap()
            .parameters[0]
            .ty
            .instantiation,
        Instantiation::Decision
    );
    for marker in ["numeric(-V1)", "numeric(V1+V2)"] {
        assert_eq!(
            resolved(&facts, &bindings, &root, source, marker).name,
            "numeric"
        );
    }
    let CallOutcome::Resolved { return_type, .. } =
        outcome(&facts, &root, source, "identity(rec.value)")
    else {
        panic!("identity unresolved");
    };
    assert_eq!(return_type.kind, TypeKind::Int);
    assert_eq!(return_type.instantiation, Instantiation::Decision);
    let CallOutcome::Resolved { return_type, .. } = outcome(&facts, &root, source, "defaulted()")
    else {
        panic!("default substitution failed");
    };
    assert_eq!(return_type.kind, TypeKind::Int);
    for name in ["CycleA", "CycleB"] {
        let declaration = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        assert!(matches!(
            facts.declarations[declaration.id.0].ty.kind,
            TypeKind::Unknown(_)
        ));
    }
    let rule = LintOptions::from_selection("element-predicate").unwrap();
    let result = analyze_model(&context, &rule);
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(result.findings.len(), 6);
    let shared_finding = result
        .findings
        .iter()
        .find(|f| f.location.path == included)
        .unwrap();
    assert_eq!(shared_finding.location.line, 2);
    assert_eq!(&shared[shared_finding.location.range.clone()], "element");
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("easier to read") && !f.message.contains("faster"))
    );
    let system = load_model(
        options.stdlib_dir.as_ref().unwrap().join("std/element.mzn"),
        &options,
    );
    assert!(analyze_model(&system, &rule).findings.is_empty());
    std::fs::remove_file(options.stdlib_dir.as_ref().unwrap().join("std/element.mzn")).unwrap();
    assert_eq!(analyze_model(&context, &rule).findings.len(), 6);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn lattice_minima_unknown_candidates_and_rule_local_limits_do_not_guess() {
    let (directory, options) = setup("ordering");
    let root = directory.join("root.mzn");
    let source = concat!(
        "include \"element.mzn\"; array[int] of var int: xs; var int: value; var int: index;\n",
        "predicate element(int: i,array[int] of var int: x,var int: y)=true;\n",
        "constraint element(1,xs,value); constraint element(index,xs,value);\n",
        "function int: number(int: x)=x; function float: number(float: x)=x; int: n=number(1);\n",
        "predicate cross(int: x,var int: y)=true; predicate cross(var int: x,int: y)=true; constraint cross(1,1);\n",
        "predicate uncertain(var int: x)=true; predicate uncertain(MissingType: x)=true; constraint uncertain(1);\n",
        "constraint missing(1); constraint let {int: element=1;} in 'element'(1,xs,value);\n",
        "predicate consume(var int: x)=true; array[int] of int: fixed; var opt int: optional; constraint consume(fixed[1..2]); constraint consume(optional+1);\n",
        "function var int: capture()=value; solve satisfy;\n",
    );
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let bindings = resolve_bindings(&context);
    let facts = resolve_callables(&context, &bindings);
    assert_eq!(
        resolved(&facts, &bindings, &root, source, "element(1,xs,value)").file,
        context.root_file.unwrap()
    );
    assert_ne!(
        resolved(&facts, &bindings, &root, source, "element(index,xs,value)").file,
        context.root_file.unwrap()
    );
    let number = resolved(&facts, &bindings, &root, source, "number(1)");
    assert_eq!(facts.declarations[number.id.0].ty.kind, TypeKind::Int);
    assert!(
        matches!(outcome(&facts,&root,source,"cross(1,1)"),CallOutcome::Ambiguous {candidates} if candidates.len()==2)
    );
    assert!(
        matches!(outcome(&facts,&root,source,"uncertain(1)"),CallOutcome::Unsupported {candidates,..} if candidates.len()==2)
    );
    assert!(matches!(
        outcome(&facts, &root, source, "missing(1)"),
        CallOutcome::Unresolved { .. }
    ));
    assert!(matches!(
        outcome(&facts, &root, source, "'element'(1,xs,value)"),
        CallOutcome::NoMatch { .. }
    ));
    let both =
        LintOptions::from_selection("element-predicate,global-variable-in-function").unwrap();
    let result = analyze_model(&context, &both);
    assert_eq!(result.status(), 1);
    assert_eq!(result.findings.len(), 2);
    assert_eq!(result.limitations.len(), 5);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(result.rules[1].outcome, RuleOutcome::Completed);
    write(
        &root,
        "predicate element(int: x)=true; constraint element(1);",
    );
    let clean = analyze_model(&load_model(&root, &options), &both);
    assert_eq!(clean.status(), 0);
    assert!(clean.limitations.is_empty());
    let parsed = parse("constraint element(1,[],1);");
    assert!(
        lint_with_options(&parsed, &both).unwrap_err()[0]
            .message
            .contains("ModelContext")
    );
    assert!(matches!(
        analyze_file(&parsed, "buffer.mzn", 0, &both).rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    let data = analyze_file(
        &parse_with_mode("x=1;", FileMode::Data),
        "data.dzn",
        0,
        &both,
    );
    assert!(data.limitations.is_empty());
    assert!(matches!(
        data.rules[0].outcome,
        RuleOutcome::Inapplicable { .. }
    ));
    assert!(Rule::ElementPredicate.is_available());
    assert_eq!(Rule::DEFAULT.len(), 2);
    std::fs::remove_dir_all(directory).unwrap();
}
