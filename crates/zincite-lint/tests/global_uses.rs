use std::path::PathBuf;
use zincite_lint::{
    GlobalUseOutcome, LintOptions, ModelOptions, RuleOutcome, analyze_model, load_model,
    resolve_bindings, resolve_callables, resolve_global_uses, resolve_instantiations,
};

#[test]
fn resolved_globals_keep_context_dependency_identity_ranges_and_suppression() {
    let directory =
        std::env::temp_dir().join(format!("zincite-global-uses-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "function var bool: forall(array[int] of var bool: xs)=true; predicate implicit_check(var int: x)=true; function var bool: '/\\'(var bool: x,var bool: y)=true; function int: '+'(int: x,int: y)=x; function bool: '='(int: x,int: y)=true; function int: numeric(int: x)=x;").unwrap();
    std::fs::write(
        library.join("std/global.mzn"),
        "predicate global_check(array[int] of var int: xs)=true;",
    )
    .unwrap();
    let source = concat!(
        "\u{feff}% é\r\ninclude \"global.mzn\"; array[1..3] of var 1..3: xs; var bool: b; bool: p;\n",
        "constraint global_check(xs); constraint global_check(xs)/\\global_check(xs);\n",
        "constraint forall(i in 1..3)(global_check(xs)); constraint forall(([global_check(xs)]));\n",
        "var bool: v=global_check(xs); constraint b\\/global_check(xs);\n",
        "constraint b->global_check(xs); constraint global_check(xs)->b;\n",
        "constraint b<->global_check(xs); constraint not global_check(xs);\n",
        "constraint if b then global_check(xs) else true endif;\n",
        "constraint if p then global_check(xs) else true endif;\n",
        "var bool: parameter_only=global_check([1,2,3]);\n",
        "predicate global_check(var int: x)=true; var bool: user=global_check(xs[1]);\n",
        "var bool: core=implicit_check(xs[1]);\n",
        "var bool: local_value=let { constraint global_check(xs); } in true;\n",
        "constraint b->(let { constraint global_check(xs); } in true);\n",
        "constraint let { int: n=3; } in global_check(xs);\n",
        "constraint let { var bool: value=global_check(xs); } in true;\n",
        "constraint let { int: n=let { constraint global_check(xs); } in 1; } in true;\n",
        "constraint (let { constraint global_check(xs); } in 1)+1=2;\n",
        "constraint numeric(let { constraint global_check(xs); } in 1)=1;\n",
        "constraint (if b then let { constraint global_check(xs); } in 1 else 2 endif)=2;\n",
        "constraint (if p then let { constraint global_check(xs); } in 1 else 2 endif)=2;\n",
        "% zincite-lint: ignore reified-global\nconstraint b->global_check(xs); solve satisfy;\n"
    );
    let root = directory.join("root.mzn");
    std::fs::write(&root, source).unwrap();
    let context = load_model(
        &root,
        &ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(library.clone()),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let facts = resolve_global_uses(&context, &bindings, &calls, &inst);
    assert_eq!(
        facts
            .uses
            .iter()
            .filter(|u| u.outcome == GlobalUseOutcome::Enforced)
            .count(),
        11
    );
    assert_eq!(
        facts
            .uses
            .iter()
            .filter(|u| u.outcome == GlobalUseOutcome::Parameter)
            .count(),
        1
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("reified-global").unwrap(),
    );
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?} {:?}",
        result.errors,
        result.limitations
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(result.findings.len(), 11);
    for finding in &result.findings {
        assert_eq!(finding.location.path, root);
        assert_eq!(&source[finding.location.range.clone()], "global_check(xs)");
    }
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "reified-global", "--stdlib-dir"])
        .arg(&library)
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)
            .unwrap()
            .matches("warning [reified-global]")
            .count(),
        11
    );
    let data = directory.join("data.dzn");
    std::fs::write(&data, "xs=[1,2,3];").unwrap();
    let clean = std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "reified-global"])
        .arg(data)
        .output()
        .unwrap();
    assert_eq!(clean.status.code(), Some(0));
    assert!(clean.stderr.is_empty());
    let stdin = std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--rules", "reified-global"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert_eq!(stdin.status.code(), Some(0));
    assert!(
        String::from_utf8(stdin.stderr)
            .unwrap()
            .contains("ModelContext")
    );
    // A user conjunction can return either operand rather than enforce both.
    // Its var overload must not borrow the implicit par conjunction's meaning.
    std::fs::write(
        library.join("std/stdlib.mzn"),
        "function bool: '/\\'(bool: x,bool: y)=true;",
    )
    .unwrap();
    let user_operator = directory.join("user-operator.mzn");
    std::fs::write(&user_operator, "include \"global.mzn\"; function var bool: '/\\'(var bool: x,var bool: y)=x; array[1..3] of var int: xs; constraint global_check(xs)/\\global_check(xs); solve satisfy;").unwrap();
    let context = load_model(
        &user_operator,
        &ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(library),
        },
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("reified-global").unwrap(),
    );
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?} {:?}",
        result.errors,
        result.limitations
    );
    assert_eq!(result.findings.len(), 2);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unsupported_actual_dependency_reports_a_limit_instead_of_a_warning() {
    let directory: PathBuf =
        std::env::temp_dir().join(format!("zincite-global-limits-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), "").unwrap();
    std::fs::write(
        library.join("std/global.mzn"),
        "predicate global_check(var int: x)=true; predicate opaque_check(var int: x=missing())=true;",
    )
    .unwrap();
    let root = directory.join("root.mzn");
    std::fs::write(
        &root,
        "include \"global.mzn\"; var bool: value=opaque_check(); var int: x; constraint if missing_guard then global_check(x) else true endif; constraint (if missing_guard then let { constraint global_check(x); } in 1 else 2 endif)=2; solve satisfy;",
    )
    .unwrap();
    let context = load_model(
        &root,
        &ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(library),
        },
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("reified-global").unwrap(),
    );
    assert_eq!(
        result
            .limitations
            .iter()
            .filter(|limitation| limitation
                .message
                .contains("Boolean enforcement context is unknown"))
            .count(),
        2
    );
    assert_eq!(result.status(), 0);
    assert!(result.findings.is_empty());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("reified-global"))
    );
    std::fs::remove_dir_all(directory).unwrap();
}
