use zincite_lint::*;

#[test]
fn parameter_only_constraints_suggest_assert() {
    let dir = std::env::temp_dir().join(format!("zincite-prefer-assertion-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        concat!(
            "function bool:'>'(int:a,int:b); function var bool:'>'(var int:a,var int:b);\n",
            "function bool: assert(bool: b, string: msg);\n",
        ),
    )
    .unwrap();
    let source = concat!(
        "int: n; var 1..10: x;\n",
        "constraint n > 0;\n",
        "constraint assert(n > 0, \"n must be positive\");\n",
        "constraint x > n;\n",
        "constraint (assert(n > 0, \"positive\"));\n",
        "constraint assert(n > 0, \"positive\") :: \"check\";\n",
        "constraint 'assert'(n > 0, \"positive\");\n",
        "constraint (n > 0) `assert` \"positive\";\n",
        "constraint (((n > 0) `assert` \"positive\")) :: \"check\";\n",
        "var bool: b;\n",
        "constraint let { constraint b; } in true;\n",
        "solve satisfy;\n",
    );
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("prefer-assertion").unwrap(),
    );
    std::fs::remove_dir_all(dir).unwrap();
    assert!(context.errors.is_empty() && result.errors.is_empty());
    assert!(result.limitations.is_empty(), "{result:?}");
    let [finding] = result.findings.as_slice() else {
        panic!("{result:?}");
    };
    let start = finding.location.range.start - context.files[0].byte_offset;
    assert_eq!(&source[start..finding.location.range.end], "n > 0");
    assert_eq!(finding.location.line, 2);
    assert_eq!(finding.severity, Severity::Warning);
}
