use zincite_lint::*;

#[test]
fn parameter_float_equalities_warn_and_decisions_stay_quiet() {
    let dir = std::env::temp_dir().join(format!("zincite-float-equality-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        concat!(
            "function bool:'='(int:a,int:b); function var bool:'='(var int:a,var int:b);\n",
            "function bool:'='(float:a,float:b); function var bool:'='(var float:a,var float:b);\n",
            "function bool:'!='(float:a,float:b); function var bool:'!='(var float:a,var float:b);\n",
            "function float:'/'(float:a,float:b);\n",
        ),
    )
    .unwrap();
    let source = concat!(
        "float: total = 1.0; int: n = 2;\n",
        "bool: half = total / n = 0.5;\n",
        "bool: other = total != 0.3;\n",
        "var float: x;\n",
        "constraint x = 1.5;\n",
        "bool: same = n = 2;\n",
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
        &LintOptions::from_selection("float-equality").unwrap(),
    );
    std::fs::remove_dir_all(dir).unwrap();
    assert!(context.errors.is_empty() && result.errors.is_empty());
    let found: Vec<_> = result
        .findings
        .iter()
        .map(|f| {
            let start = f.location.range.start - context.files[0].byte_offset;
            assert_eq!(f.severity, Severity::Warning);
            &source[start..f.location.range.end - context.files[0].byte_offset]
        })
        .collect();
    assert_eq!(found, ["total / n = 0.5", "total != 0.3"], "{result:?}");
}
