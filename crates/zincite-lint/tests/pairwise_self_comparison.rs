use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int:'..'(int:a,int:b); function int:'+'(int:a,int:b);\n",
    "function bool:'!='($T:a,$T:b); function var bool:'!='(any $T:a,any $T:b); function var bool:'='(any $T:a,any $T:b);\n",
    "function bool:'<'(int:a,int:b); function var bool:'<'(var int:a,var int:b); function var bool:'>'(var int:a,var int:b);\n",
    "function var bool:'/\\'(var bool:a,var bool:b); function var bool:'\\/'(var bool:a,var bool:b);\n",
    "function var bool:'->'(var bool:a,var bool:b); predicate forall(array[int] of var opt bool:a);\n",
    "function set of $$E:index_set(array[$$E] of any $T:x);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-self-comparison-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
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
        &LintOptions::from_selection("pairwise-self-comparison").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (dir, context, result)
}
fn spelling<'a>(context: &'a ModelContext, finding: &FileFinding) -> &'a str {
    let source = &context.files[0];
    &source.parsed.source()[finding.location.range.start - source.byte_offset
        ..finding.location.range.end - source.byte_offset]
}
#[test]
fn unexcluded_equal_pairs_report_the_comparison() {
    let source = concat!(
        "int:n; array[1..3] of var 1..3:x; array[1..n] of var int:y;\n",
        "constraint forall(i,j in 1..3)(x[i]!=x[j]);\n",
        "constraint forall(i in 1..n,j in 1..n)(y[i]<y[j]); solve satisfy;\n",
    );
    let (dir, context, result) = model("reported", source);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["x[i]!=x[j]", "y[i]<y[j]"],
        "{result:?}"
    );
    let [known, symbolic] = [&result.findings[0], &result.findings[1]];
    assert_eq!(known.severity, Severity::Warning);
    assert!(known.fix.is_none());
    for finding in [known, symbolic] {
        assert!(
            finding.message.contains("'i' and 'j'"),
            "{}",
            finding.message
        );
    }
    assert!(
        known.message.contains("cannot be satisfied."),
        "{}",
        known.message
    );
    assert!(!known.message.contains("whenever"));
    assert!(
        symbolic.message.contains("whenever"),
        "{}",
        symbolic.message
    );
    assert_eq!(result.status(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn excluded_pairs_and_unproved_identities_stay_quiet() {
    let source = concat!(
        "int:n; array[1..n] of var int:x; array[1..n] of var int:y;\n",
        "constraint forall(i,j in 1..n where i<j)(x[i]!=x[j]);\n",
        "constraint forall(i,j in 1..n where i!=j)(x[i]!=x[j]);\n",
        "constraint forall(i in 1..n,j in i+1..n)(x[i]!=x[j]);\n",
        "constraint forall(i,j in 1..n)(i=j \\/ x[i]!=x[j]);\n",
        "constraint forall(i,j in 1..n)(i!=j -> x[i]!=x[j]);\n",
        "constraint forall(i,j in 1..n)(x[i]!=y[j]);\n",
        "constraint forall(i,j in 1..0)(x[i]!=x[j]);\n",
        "enum E={}; array[E] of var int:z;\n",
        "constraint forall(i,j in E)(z[i]!=z[j]);\n",
        "constraint forall(i,j in index_set(z))(z[i]!=z[j]); solve satisfy;\n",
    );
    let (dir, _, result) = model("quiet", source);
    assert!(result.findings.is_empty(), "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
}
