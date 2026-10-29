use zincite_lint::*;

fn analyze(name: &str, source: &str) -> (ModelContext, AnalysisResult) {
    let dir =
        std::env::temp_dir().join(format!("zincite-deprecated-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), "").unwrap();
    std::fs::write(
        dir.join("library/std/legacy.mzn"),
        "predicate legacy(var int: x) = true;\npredicate legacy(var bool: b) = b;\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("library/std/legacy.mzn.deprecated.mzn"),
        "predicate legacy(var int: x) :: mzn_deprecated(\"2.6.0\", \"use modern\");\n",
    )
    .unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("deprecated-call").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    std::fs::remove_dir_all(dir).unwrap();
    (context, result)
}

#[test]
fn deprecated_standard_overloads_report_their_annotation() {
    let source = concat!(
        "include \"legacy.mzn\";\n",
        "var int: x; var bool: b; var float: f;\n",
        "predicate legacy(var float: y) = true;\n",
        "constraint legacy(x);\n",
        "constraint legacy(b) /\\ legacy(f);\nsolve satisfy;\n",
    );
    let (context, result) = analyze("calls", source);
    let [finding] = result.findings.as_slice() else {
        panic!("{result:?}");
    };
    let start = finding.location.range.start - context.files[0].byte_offset;
    assert_eq!(&source[start..finding.location.range.end], "legacy");
    assert_eq!(finding.location.line, 4);
    assert_eq!(finding.severity, Severity::Warning);
    assert_eq!(
        finding.message,
        "'legacy' is deprecated since MiniZinc 2.6.0: use modern"
    );
    assert!(result.limitations.is_empty(), "{result:?}");
}
