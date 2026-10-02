use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int: '..'(int:a,int:b); function int: 'div'(int:a,int:b); function var int: 'div'(var int:a,var int:b);\n",
    "function var bool: '='(any $T:a,any $T:b); function bool: '>'(int:a,int:b);\n",
    "function var bool: '/\\'(var bool:a,var bool:b); function var bool: '\\/'(var bool:a,var bool:b); function var bool: 'not'(var bool:a);\n",
    "predicate forall(array[int] of var opt bool:a); predicate exists(array[int] of var opt bool:a);\n",
    "function $T: assert(bool:c,string:m,$T:v);\n",
);
fn model(name: &str, source: &str, selection: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!("zincite-vacuity-{name}-{}", std::process::id()));
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
    let result = analyze_model(&context, &LintOptions::from_selection(selection).unwrap());
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    (dir, context, result)
}
fn spelling<'a>(context: &'a ModelContext, finding: &FileFinding) -> &'a str {
    let source = context
        .files
        .iter()
        .find(|f| f.path == finding.location.path)
        .unwrap();
    source.parsed.source()[finding.location.range.start - source.byte_offset
        ..finding.location.range.end - source.byte_offset]
        .trim()
}
#[test]
fn empty_filters_and_constant_truth_report_the_exact_context() {
    let source = concat!(
        "\u{feff}% π\r\nvar bool:p; int:N=0;\r\n",
        "constraint forall(i in 1..0)(true); constraint exists([false | j in 1..0]);\r\n",
        "constraint forall(k in 1..3 where k>3)(true); constraint p=p;\r\n",
        "var int:v=if p /\\ not p then 1 else 2 endif; constraint forall(q in 1..N)(p); solve satisfy;\r\n",
    );
    let (dir, context, result) = model("categories", source, "vacuous-constraint");
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        [
            "forall(i in 1..0)(true)",
            "exists([false | j in 1..0])",
            "k>3",
            "p=p",
            "p /\\ not p"
        ]
    );
    assert_eq!(result.status(), 1);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{result:?}"
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    assert!(result.findings[0].message.contains("identity is true"));
    assert!(result.findings[1].message.contains("identity is false"));
    assert!(
        result.findings[2].message.contains("proved false")
            && result.findings[2].message.contains("not prove")
    );
    assert!(result.findings[3].message.contains("constraint is true"));
    assert!(!result.findings[3].message.contains("condition 'p=p'"));
    assert!(result.findings[4].message.contains("condition is false"));
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.location.path == dir.join("root.mzn"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn partiality_optional_presence_and_upstream_evaluation_remain_distinct() {
    let source = concat!(
        "var int:x; int:d; array[1..2] of var opt bool:unknown_values;\n",
        "constraint x=x; constraint (1 div d)=(1 div d); constraint (1 div 0)=(1 div 0);\n",
        "var bool:absent_values=let {array[int] of opt bool:a=[<> | i in 1..3];} in forall(a);\n",
        "var bool:unknown=forall(unknown_values);\n",
        "constraint forall(i in {2 div 0},j in 1..0)(true);\n",
        "var bool:abort=forall(i in 1..assert(false,\"abort\",0),j in 1..0)(true); solve satisfy;\n",
    );
    let (dir, context, result) = model("partial", source, "vacuous-constraint,partial-expression");
    let vac: Vec<_> = result
        .findings
        .iter()
        .filter(|f| f.rule == Rule::VacuousConstraint)
        .collect();
    assert_eq!(
        vac.iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        [
            "x=x",
            "(1 div 0)=(1 div 0)",
            "forall(a)",
            "forall(i in {2 div 0},j in 1..0)(true)"
        ]
    );
    assert!(
        vac[2].message.contains("present Boolean elements") && vac[2].message.contains("capacity")
    );
    assert!(vac[1].message.contains("undefinedness becomes false"));
    assert!(
        vac[3].message.contains("undefinedness becomes false")
            && !vac[3].message.contains("empty Boolean identity")
    );
    assert_eq!(
        result
            .findings
            .iter()
            .filter(|f| f.rule == Rule::PartialExpression)
            .count(),
        3
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("assertion evaluation can abort")),
        "{result:?}"
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn assumptions_inactivity_unknowns_and_item_suppression_preserve_boundaries() {
    let source = concat!(
        "var bool:p; var bool:q; var bool:r; bool:keep; int:N=3; function bool:opaque();\n",
        "constraint p=true; constraint q=true; var int:enforced=if not(p=true) then 1 else 2 endif;\n",
        "var int:local=if r then if not r then 1 else 2 endif else 0 endif;\n",
        "var int:inactive=if false then if q /\\ not q then 1 else 2 endif else 0 endif;\n",
        "constraint forall(i in 1..3 where i>3,j in 1..2 where keep)(true);\n",
        "constraint forall(k in 1..N where k>N)(keep);\n",
        "% zincite-lint: ignore vacuous-constraint\nconstraint true; constraint opaque(); solve satisfy;\n",
    );
    let (dir, context, result) = model("scopes", source, "vacuous-constraint");
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| spelling(&context, f))
            .collect::<Vec<_>>(),
        ["not(p=true)", "not r", "false", "i>3"]
    );
    let root = context
        .files
        .iter()
        .find(|f| f.path == dir.join("root.mzn"))
        .unwrap();
    for condition in ["p=true", "q=true"] {
        let start = source.find(condition).unwrap();
        let location = root.location(start..start + condition.len());
        assert!(
            result.findings[0].message.contains(&format!(
                "enforced condition '{condition}' = true at {}:{}:{}",
                location.path.display(),
                location.line,
                location.column
            )),
            "{}",
            result.findings[0].message
        );
    }
    assert!(
        !result.findings[0]
            .message
            .contains("condition 'not(p=true)'")
    );
    assert!(
        result.findings[1].message.contains("local condition")
            && result.findings[1].message.contains("conditional")
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("opaque")),
        "{result:?}"
    );
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
}
