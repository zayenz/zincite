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
    let library = options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn");
    let mut standard = std::fs::read_to_string(&library).unwrap();
    standard.push_str(concat!(
        "function var int: sum(array[$T] of var int: values);\n",
        "function var int: bool2int(var bool: value);\n"
    ));
    write(&library, &standard);
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
        "var int: matrix_sum=sum(matrix); array[int,int] of var opt int: optional_matrix; var opt int: optional_sum=sum(optional_matrix);\n",
        "function var int: sum(array[int,int] of var bool: flags)=bool2int(flags[2,4]); array[int,int] of var bool: matrix_flags; var int: user_sum=sum(matrix_flags);\n",
        "array[int] of var opt int: optional; var opt int: ov; constraint element(1,optional,ov);\n",
        "any: inferred_parameter=1; any: inferred_decision=iv; any: inferred_values=values; any: inferred_optional=ov;\n",
        "any: inferred_unknown=unavailable_value; any: inferred_cycle=inferred_cycle;\n",
        "array[int] of set of int: ordinal_sets; int: ordinal;\n",
        "opt set of int: optional_set; var set of int: decision_set;\n",
        "any: inferred_ordinal=ordinal_sets[1][ordinal];\n",
        "any: inferred_optional_set=optional_set[ordinal]; any: inferred_decision_set=decision_set[ordinal];\n",
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
    let declaration = resolved(&facts, &bindings, &root, source, "sum(matrix);");
    assert_eq!(declaration.name, "sum");
    assert_eq!(
        context.files[declaration.file].canonical_path,
        library.canonicalize().unwrap()
    );
    let matrix = bindings
        .declarations
        .iter()
        .find(|d| d.name == "matrix")
        .unwrap();
    assert!(matches!(
        &facts.declarations[matrix.id.0].ty.kind,
        TypeKind::Array { indices, .. } if indices.len() == 2
    ));
    assert_eq!(
        context.files[resolved(&facts, &bindings, &root, source, "sum(matrix_flags)").file]
            .canonical_path,
        root.canonicalize().unwrap()
    );
    assert!(matches!(
        outcome(&facts, &root, source, "sum(optional_matrix)"),
        CallOutcome::NoMatch { .. }
    ));
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
    let declaration_type = |name: &str| {
        let declaration = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        &facts.declarations[declaration.id.0].ty
    };
    assert_eq!(declaration_type("inferred_parameter").kind, TypeKind::Int);
    let ordinal_type = declaration_type("inferred_ordinal");
    assert_eq!(ordinal_type.kind, TypeKind::Int);
    assert_eq!(ordinal_type.instantiation, Instantiation::Parameter);
    assert!(!ordinal_type.optional);
    assert_eq!(
        declaration_type("inferred_parameter").instantiation,
        Instantiation::Parameter
    );
    for (alias, original) in [
        ("inferred_decision", "iv"),
        ("inferred_values", "values"),
        ("inferred_optional", "ov"),
    ] {
        assert_eq!(
            declaration_type(alias),
            declaration_type(original),
            "{alias}"
        );
    }
    for name in [
        "inferred_unknown",
        "inferred_cycle",
        "inferred_optional_set",
        "inferred_decision_set",
    ] {
        assert!(
            matches!(declaration_type(name).kind, TypeKind::Unknown(_)),
            "{name}: {:?}",
            declaration_type(name)
        );
    }
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
    let source = concat!(
        "predicate set_cross(array[int] of int:x,var int:y)=true; predicate set_cross(array[int] of var int:x,int:y)=true; constraint set_cross({1,2},1);\n",
        "predicate set_uncertain(array[int] of int:x)=true; predicate set_uncertain(MissingType:x)=true; constraint set_uncertain({1,2});\n",
        "function int: choose(set of int:values)=1; function int: choose(array[int] of int:values)=2; int: chosen=choose({1,2});\n",
        "solve satisfy;\n",
    );
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let bindings = resolve_bindings(&context);
    let facts = resolve_callables(&context, &bindings);
    assert!(
        matches!(outcome(&facts,&root,source,"set_cross({1,2},1)"),CallOutcome::Ambiguous {candidates} if candidates.len()==2)
    );
    assert!(
        matches!(outcome(&facts,&root,source,"set_uncertain({1,2})"),CallOutcome::Unsupported {candidates,..} if candidates.len()==2)
    );
    assert!(
        matches!(outcome(&facts, &root, source, "choose({1,2})"), CallOutcome::Resolved { parameters, .. } if matches!(parameters[0].kind, zincite_lint::TypeKind::Set(_)))
    );
    let source = concat!(
        "array[1..1] of int: f = [1];\n",
        "function int: f(int: i) = f[i];\n",
        "function float: f(float: i) = i;\n",
        "int: selected = f(1);\n",
        "array[1..1] of int: crossing = [0];\n",
        "function int: crossing(int: x, var int: y) = 1;\n",
        "function int: crossing(var int: x, int: y) = 2;\n",
        "int: ambiguous = crossing(1,1);\n",
        "array[1..1] of int: uncertain = [0];\n",
        "function int: uncertain(int: x) = 1;\n",
        "function int: uncertain(MissingType: x) = 2;\n",
        "int: unavailable = uncertain(1);\n",
        "int: shadowed = let { int: f = 0; } in 'f'(1);\n",
        "solve satisfy;\n",
    );
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let bindings = resolve_bindings(&context);
    let facts = resolve_callables(&context, &bindings);
    let selected = resolved(&facts, &bindings, &root, source, "f(1)");
    assert_eq!(selected.role, DeclarationRole::Function);
    assert!(matches!(
        outcome(&facts, &root, source, "f(1)"),
        CallOutcome::Resolved { parameters, return_type, .. }
            if parameters.len() == 1
                && parameters[0].kind == TypeKind::Int
                && parameters[0].instantiation == Instantiation::Parameter
                && !parameters[0].optional
                && return_type.kind == TypeKind::Int
                && return_type.instantiation == Instantiation::Parameter
                && !return_type.optional
    ));
    let array_reference = bindings
        .references
        .iter()
        .find(|reference| {
            reference.location.path == root
                && reference.location.range.start == source.find("f[i]").unwrap()
        })
        .unwrap();
    assert_eq!(array_reference.kind, zincite_lint::ReferenceKind::Value);
    let zincite_lint::BindingResolution::Resolved(array) = &array_reference.resolution else {
        panic!("{:?}", array_reference.resolution);
    };
    assert_eq!(bindings.declarations[array.0].role, DeclarationRole::Value);
    assert!(bindings.declarations[array.0].top_level);
    assert!(matches!(
        outcome(&facts, &root, source, "crossing(1,1)"),
        CallOutcome::Ambiguous { candidates }
            if candidates.len() == 2
                && candidates.iter().all(|id| bindings.declarations[id.0].role == DeclarationRole::Function)
    ));
    assert!(matches!(
        outcome(&facts, &root, source, "uncertain(1)"),
        CallOutcome::Unsupported { candidates, .. }
            if candidates.len() == 2
                && candidates.iter().all(|id| bindings.declarations[id.0].role == DeclarationRole::Function)
    ));
    assert!(matches!(
        outcome(&facts, &root, source, "'f'(1)"),
        CallOutcome::NoMatch { .. }
    ));
    let local_reference = bindings
        .references
        .iter()
        .find(|reference| {
            reference.location.path == root
                && reference.location.range.start == source.find("'f'(1)").unwrap()
        })
        .unwrap();
    assert_eq!(local_reference.kind, zincite_lint::ReferenceKind::Callable);
    assert!(matches!(
        &local_reference.resolution,
        zincite_lint::BindingResolution::Resolved(id)
            if bindings.declarations[id.0].role == DeclarationRole::Local
    ));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn element_fix_facts_require_actual_indices_total_operands_and_core_equality() {
    use zincite_lint::*;
    let (directory, options) = setup("fix");
    write(
        &options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn"),
        concat!(
            "function set of int: '..'(int:a,int:b);\n",
            "function var bool: '='(var $T:a,var $T:b);\n",
            "function int: 'div'(int:a,int:b); function var bool: 'in'(var int:a,set of int:b);\n",
        ),
    );
    let root = directory.join("root.mzn");
    let source = concat!(
        "\u{feff}include \"element.mzn\"; % é\r\n",
        "array[0..2] of var 0..5:xs; var 0..2:index; var int:value; var bool:flag;\r\n",
        "constraint :: \"kept label\" flag <-> element /* head */ (index /* i */, xs /* a */, value /* y */);\r\n",
        "int:unknown; var int:choice; function int:opaque(int:x);\r\n",
        "constraint if choice in 0..2 then element(choice,xs,value) else true endif;\r\n",
        "constraint element(unknown,xs,value); constraint element(3,xs,value);\r\n",
        "constraint element(1 div 0,xs,value); constraint element(opaque(1),xs,value);\r\n",
        "constraint element(i:index,x:xs,y:value);\r\n",
        "array[0..2] of var opt int:optional; var opt int:optional_value; constraint element(index,optional,optional_value);\r\n",
        "enum X={A,B}; enum Y={C,D}; array[X] of var int:typed; Y:wrong; constraint element(wrong,typed,value);\r\n",
        "solve satisfy;\r\n",
    );
    write(&root, source);
    let context = load_model(&root, &options);
    let lint = LintOptions::from_selection("element-predicate").unwrap();
    let result = analyze_model(&context, &lint);
    assert!(result.errors.is_empty(), "{result:?}");
    let fixes: Vec<_> = result
        .findings
        .iter()
        .filter_map(|f| f.fix.as_ref())
        .collect();
    assert_eq!(fixes.len(), 2, "{result:?}");
    assert_eq!(fixes[0].safety, FixSafety::Safe);
    let snapshot = context.files[context.root_file.unwrap()].source_snapshot();
    let prepared = prepare_fixes(
        &snapshot,
        source.as_bytes(),
        &result.findings,
        &FixOptions::default(),
    )
    .unwrap();
    assert!(
        std::str::from_utf8(&prepared.candidate)
            .unwrap()
            .contains("flag <-> ( /* head */ ( value /* y */) = ( xs /* a */)[(index /* i */)])"),
        "{:?}",
        prepared.candidate
    );
    for comment in [
        "/* head */",
        "/* i */",
        "/* a */",
        "/* y */",
        "\"kept label\"",
        "% é",
    ] {
        assert_eq!(
            std::str::from_utf8(&prepared.candidate)
                .unwrap()
                .matches(comment)
                .count(),
            1
        );
    }
    assert!(
        prepared.candidate.starts_with(b"\xef\xbb\xbf")
            && std::str::from_utf8(&prepared.candidate)
                .unwrap()
                .contains("\r\n")
    );
    replace_fixed_file(&snapshot, &prepared.candidate).unwrap();
    let final_context = load_model(&root, &options);
    assert!(
        analyze_model(&final_context, &lint)
            .findings
            .iter()
            .all(|f| f.fix.is_none())
    );
    // A selected user equality and a user element lookalike retain no Safe fix.
    write(
        &root,
        "include \"element.mzn\"; function var bool:'='(var int:a,var int:b)=true; array[0..2] of var int:xs; var 0..2:index; var int:value; constraint element(index,xs,value); solve satisfy;",
    );
    assert!(
        analyze_model(&load_model(&root, &options), &lint)
            .findings
            .iter()
            .all(|f| f.fix.is_none())
    );
    write(
        &root,
        "predicate element(var int:i,array[int] of var int:a,var int:y)=true; array[0..2] of var int:xs; var 0..2:index; var int:value; constraint element(index,xs,value); solve satisfy;",
    );
    assert!(
        analyze_model(&load_model(&root, &options), &lint)
            .findings
            .is_empty()
    );
    // Unknown formal annotations on a configured standard implementation with
    // the same written relation must still withhold the Safe rewrite.
    write(
        &options.stdlib_dir.as_ref().unwrap().join("std/element.mzn"),
        &format!(
            "annotation semantic;\n{}",
            ELEMENT.replace("var $$T: y)", "var $$T: y :: semantic)")
        ),
    );
    write(
        &root,
        "include \"element.mzn\"; array[0..2] of var int:xs; var 0..2:index; var int:value; constraint element(index,xs,value); solve satisfy;",
    );
    let annotated = analyze_model(&load_model(&root, &options), &lint);
    assert_eq!(annotated.findings.len(), 1, "{annotated:?}");
    assert!(annotated.findings[0].fix.is_none(), "{annotated:?}");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn supplied_argument_ranking_retains_defaults_scope_and_parameter_guards() {
    use zincite_lint::*;
    let (directory, options) = setup("default-ranking");
    let root = directory.join("root.mzn");
    let included = directory.join("included.mzn");
    write(
        &options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn"),
        "function var bool: '='(var int:a,var int:b);",
    );
    let declarations = concat!(
        "predicate choose_present(array[int] of var int:xs,int:tag=default_tag)=true;\n",
        "predicate choose_present(array[int] of var opt int:xs)=true;\n",
        "predicate mapped(var int:result,int:needle=default_tag,array[int] of var int:xs)=result=needle;\n",
        "predicate mapped(var opt int:result,array[int] of var opt int:xs)=true;\n",
        "predicate competing(int:x=1)=true; predicate competing(int:x=2)=true;\n",
        "predicate inferred_copy($T:input,var int:result)=let{any:alias=input;}in result=alias;\n",
        "function bool: keep(opt $T:x)=true; function var bool: keep(var opt $$E:x)=true;\n",
        "function bool: '<='($T:a,$T:b); function var bool: '<='(var opt $$E:a,var opt $$E:b);\n",
    );
    let source = concat!(
        "include \"included.mzn\"; int:default_tag; int:limit; opt int:maybe;\n",
        "array[1..2] of var 1..3:values; var int:result; var int:inferred_result; constraint inferred_copy(1,inferred_result);\n",
        "constraint choose_present(values); constraint choose_present(xs:values);\n",
        "constraint choose_present(tag:1,xs:values);\n",
        "constraint let {var int:default_tag;} in choose_present(values);\n",
        "constraint mapped(xs:values,result:result); constraint competing();\n",
        "constraint if keep(maybe) then true else false endif;\n",
        "constraint if 1<=limit then true else false endif; solve satisfy;\n",
    );
    write(&root, source);
    write(&included, declarations);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    for marker in [
        "choose_present(values)",
        "choose_present(xs:values)",
        "choose_present(tag:1,xs:values)",
        "mapped(xs:values,result:result)",
    ] {
        let CallOutcome::Resolved {
            declaration,
            parameters,
            ..
        } = outcome(&calls, &root, source, marker)
        else {
            panic!("{marker}: {:?}", outcome(&calls, &root, source, marker));
        };
        let selected = &bindings.declarations[declaration.0];
        assert_eq!(selected.location.path, included);
        let signature = calls
            .signatures
            .iter()
            .find(|s| s.declaration == *declaration)
            .unwrap();
        assert_eq!(parameters.len(), signature.parameters.len());
        assert_eq!(
            parameters.len(),
            if marker.starts_with("mapped") { 3 } else { 2 }
        );
        assert!(signature.parameters[1].has_default);
        assert_eq!(parameters[1].kind, TypeKind::Int);
        assert_eq!(parameters[1].instantiation, Instantiation::Parameter);
        let array_slot = if marker.starts_with("mapped") { 2 } else { 0 };
        let TypeKind::Array { element, .. } = &parameters[array_slot].kind else {
            panic!("missing array parameter");
        };
        assert!(!element.optional);
    }
    for call in calls
        .calls
        .iter()
        .filter(|c| c.location.path == root && c.name == "choose_present")
    {
        assert!(matches!(call.outcome, CallOutcome::Resolved { .. }));
        assert_eq!(&source[call.location.range.clone()], "choose_present");
    }
    assert!(matches!(
        outcome(&calls, &root, source, "competing()"),
        CallOutcome::Ambiguous { candidates } if candidates.len()==2
    ));
    let global = bindings
        .declarations
        .iter()
        .find(|d| d.name == "default_tag" && d.top_level)
        .unwrap();
    for reference in bindings
        .references
        .iter()
        .filter(|r| r.location.path == included && r.name == "default_tag")
    {
        assert_eq!(reference.resolution, BindingResolution::Resolved(global.id));
    }
    let inst = resolve_instantiations(&context, &bindings, &calls);
    for marker in ["keep(maybe)", "1<=limit"] {
        let start = source.find(marker).unwrap();
        let fact = inst
            .expressions
            .iter()
            .filter(|f| {
                f.location.path == root
                    && f.location.range.start <= start
                    && f.location.range.end >= start + marker.len()
            })
            .min_by_key(|f| f.location.range.len())
            .unwrap();
        assert_eq!(fact.instantiation, Instantiation::Parameter);
    }
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_callable_definitions(&context, &bindings, &calls, &inst, &domains);
    let result = bindings
        .declarations
        .iter()
        .find(|d| d.name == "result" && d.top_level)
        .unwrap();
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| d.target == result.id && d.dependencies.contains(&global.id)),
        "{definitions:?}"
    );
    let inferred_result = bindings
        .declarations
        .iter()
        .find(|d| d.name == "inferred_result")
        .unwrap();
    assert!(
        definitions
            .definitions
            .iter()
            .any(|d| d.target == inferred_result.id),
        "{definitions:?}"
    );
    let analysis = analyze_model(
        &context,
        &LintOptions::from_selection("decision-variable-condition").unwrap(),
    );
    assert!(
        analysis.errors.is_empty() && analysis.limitations.is_empty(),
        "{analysis:?}"
    );
    assert!(analysis.findings.is_empty());
    assert_eq!(analysis.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn mixed_boolean_enum_conditionals_use_a_numeric_common_type() {
    let (directory, options) = setup("mixed-enum-bool");
    let library = options.stdlib_dir.as_ref().unwrap().join("std/stdlib.mzn");
    let mut standard = std::fs::read_to_string(&library).unwrap();
    standard.push_str("annotation input_order; annotation indomain_min; function ann: int_search(array[$X] of var $$E: x, ann: select, ann: choice);\n");
    write(&library, &standard);
    let root = directory.join("root.mzn");
    let source = "enum E={A,B};\narray[1..2] of var E: features;\narray[1..2] of var bool: flags;\narray[int] of var int: reversed=[if j=1 then flags[i] else features[i] endif|i in 1..2,j in 1..2];\nsolve :: int_search([if j=1 then features[i] else flags[i] endif|i in 1..2,j in 1..2],input_order,indomain_min) satisfy;\n";
    write(&root, source);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    let bindings = resolve_bindings(&context);
    let facts = resolve_callables(&context, &bindings);
    assert!(matches!(
        outcome(&facts, &root, source, "int_search("),
        CallOutcome::Resolved { .. }
    ));
    let enumeration = bindings
        .declarations
        .iter()
        .find(|d| d.name == "E")
        .unwrap()
        .id;
    let expression = |marker: &str| {
        let start = source.find(marker).unwrap();
        &facts
            .expressions
            .iter()
            .find(|e| {
                e.location.path == root
                    && e.location.range.start == start
                    && e.location.range.end == start + marker.len()
            })
            .unwrap()
            .ty
    };
    assert_eq!(expression("features[i]").kind, TypeKind::Enum(enumeration));
    assert_eq!(expression("flags[i]").kind, TypeKind::Bool);
    for branch in [
        "if j=1 then features[i] else flags[i] endif",
        "if j=1 then flags[i] else features[i] endif",
    ] {
        let ty = expression(branch);
        assert_eq!(ty.kind, TypeKind::Int);
        assert_eq!(ty.instantiation, Instantiation::Decision);
        assert!(!ty.optional);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
