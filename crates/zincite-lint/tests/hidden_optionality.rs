use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function int: length(array[$I] of any $T:x); function var int: card(var set of int:x);\n",
    "function var bool: '='(any $T:a,any $T:b); function var bool: '<='(var int:a,var int:b); function var bool: '>='(var int:a,var int:b); function var bool: '<'(var int:a,var int:b);\n",
    "function bool:'>'($T:x,$T:y); function var bool:'>'(var $T:x,var $T:y); function var bool:'>'($T:x,var $T:y); function var bool:'>'(var $T:x,$T:y);\n",
    "function bool:'>'(opt $$E:x,opt $$E:y); function var bool:'>'(var opt $$E:x,var opt $$E:y);\n",
    "function set of int: '..'(int:a,int:b); function var int: sum(array[int] of var opt int:x); function var bool: forall(array[int] of var opt bool:x);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-hidden-optionality-{name}-{}",
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
        &LintOptions::from_selection("hidden-optionality").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    (dir, context, result)
}
#[test]
fn direct_length_comparisons_explain_slots_presence_and_supported_aliases() {
    let source = concat!(
        "\u{feff}% é\r\nvar set of 1..9:x; var bool:keep; int:N; var set of 1..N:s;\r\n",
        "constraint card(x)<=4; constraint length([i | i in x])>5;\r\n",
        "constraint (length(x:[v | v in 1..3 where keep]))=2;\r\n",
        "array[int] of var opt int:xs=[i | i in 1..3 where keep]; array[int] of var opt int:alias=xs; constraint length(alias)>=1;\r\n",
        "constraint length([j | j in s])=N; array[1..3] of int:a; int:outside=a[4]; solve satisfy;\r\n",
    );
    let (dir, context, result) = model("counts", source);
    assert_eq!(result.status(), 1);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{result:?}"
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| &source[f.location.range.clone()])
            .collect::<Vec<_>>(),
        [
            "length([i | i in x])",
            "length(x:[v | v in 1..3 where keep])",
            "length(alias)",
            "length([j | j in s])"
        ]
    );
    for f in &result.findings {
        assert!(
            f.message
                .contains("if this comparison intends to count selected or present elements")
                && f.message.contains("capacity")
                && f.message.contains("when the collection is defined")
        );
        assert!(
            !f.message.contains("replace")
                && !f.message.contains("card(")
                && !f.message.contains("wrong")
        );
    }
    assert!(
        result.findings[0]
            .message
            .contains("decision-set membership")
            && result.findings[0].message.contains("9 slots")
    );
    assert!(
        result.findings[1].message.contains("decision where filter")
            && result.findings[1].message.contains("3 slots")
    );
    assert!(
        result.findings[3].message.contains("not known")
            && result.findings[3]
                .message
                .contains("present count remains unknown")
    );
    let joint = analyze_model(
        &context,
        &LintOptions::from_selection("hidden-optionality,index-set-mismatch").unwrap(),
    );
    assert_eq!(joint.findings.len(), 5, "{joint:?}");
    assert!(
        joint
            .rules
            .iter()
            .all(|r| matches!(r.outcome, RuleOutcome::Completed))
    );
    assert_eq!(
        joint
            .findings
            .iter()
            .filter(|f| f.rule == Rule::HiddenOptionality)
            .count(),
        4
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn capacity_uses_aggregates_safe_contexts_and_unknown_data_stay_quiet() {
    let source = concat!(
        "var set of 1..3:x; var bool:keep; bool:p; array[1..3] of opt int:data;\n",
        "constraint length([i | i in 1..3 where p])=2; constraint sum([i | i in x])>=0; constraint forall(i in x)(i>=0);\n",
        "int:display=length([i | i in x]); array[1..length([i | i in x])] of var int:allocated;\n",
        "constraint length([i | i in 1..0 where keep])=0; constraint length([i | i in 1..3])=3;\n",
        "constraint if keep then length([i | i in 1..3 where keep])=1 else true endif;\n",
        "constraint if false then length([i | i in 1..3 where keep])=1 else true endif; constraint length(data)=1;\n",
        "var bool:always; array[int] of var opt int:all_present=[i | i in 1..3 where always=always]; array[int] of var opt int:all_alias=all_present; constraint length(all_alias)=1;\n",
        "array[int] of opt int:replaceable=[<> | i in 1..3]; constraint length(replaceable)=2; solve satisfy;\n",
    );
    let (dir, _, result) = model("quiet", source);
    assert_eq!(result.status(), 0, "{result:?}");
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "lookalike",
        "function int:length(array[int] of var opt int:x)=1; var bool:keep; constraint length([i | i in 1..3 where keep])=1; solve satisfy;",
    );
    assert_eq!(result.status(), 0, "{result:?}");
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "comparison-lookalike",
        "function bool:'='(int:a,int:b)=true; var bool:keep; constraint length([i | i in 1..3 where keep])=1; solve satisfy;",
    );
    assert_eq!(result.status(), 0, "{result:?}");
    assert!(
        result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "unsupported",
        "var bool:keep; constraint length([i | i,j in 1..9223372036854775807 where keep])=1; solve satisfy;",
    );
    assert_eq!(result.status(), 0);
    assert!(result.findings.is_empty());
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.location.path == dir.join("root.mzn")
                && l.message
                    .contains("hidden-optionality: collection count overflow")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
