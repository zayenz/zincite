use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function int: '+'(int:a,int:b); function set of int: '..'(int:a,int:b);\n",
    "function bool: '<'(int:a,int:b); function bool: '!='(int:a,int:b);\n",
    "function var bool: '='(any $T:a,any $T:b); function var bool: '>'(var int:a,var int:b);\n",
    "function var bool: '->'(var bool:a,var bool:b); function var bool: 'not'(var bool:a);\n",
    "function int: 'div'(int:a,int:b); function int: 'mod'(int:a,int:b);\n",
    "function var int: 'div'(var int:a,var int:b); function var int: 'mod'(var int:a,var int:b);\n",
    "function var bool: forall(array[int] of var bool:a); function var int: sum(array[int] of var int:a);\n",
    "function var int: min(array[int] of var int:a); function var int: max(array[int] of var int:a);\n",
    "function bool: assert(bool:c,string:m); function $T: assert(bool:c,string:m,$T:v);\n",
    "function float:ln(float:x); function float:log(float:b,float:x); function float:int2float(int:x);\n",
    "function $T: deopt(opt $T:x); function var $T: deopt(var opt $T:x);\n",
    "function $T: 'default'(opt $T:x,$T:y); function var $T: 'default'(var opt $T:x,var $T:y);\n",
);
fn model(name: &str, source: &str, selection: &str) -> (PathBuf, ModelContext, AnalysisResult) {
    let dir = std::env::temp_dir().join(format!("zincite-partial-{name}-{}", std::process::id()));
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
    let file = context
        .files
        .iter()
        .find(|f| f.path == finding.location.path)
        .unwrap();
    file.parsed.source()[finding.location.range.start - file.byte_offset
        ..finding.location.range.end - file.byte_offset]
        .trim()
}
fn spellings<'a>(context: &'a ModelContext, result: &AnalysisResult) -> Vec<&'a str> {
    result
        .findings
        .iter()
        .map(|f| spelling(context, f))
        .collect()
}
#[test]
fn exact_requirements_report_locations_and_keep_source_unchanged() {
    let source = concat!(
        "\u{feff}% π\r\narray[1..3] of int:a;\r\n",
        "int:d=1 div 0; int:r=4 mod 0; int:v=a[4]; float:logarithm=ln(0); float:positive=ln(int2float(1));\r\n",
        "var int:lo=min([]); var int:hi=max(i in 1..0)(i);\r\n",
        "int:p=let {opt int:missing=<>;} in deopt(missing); solve satisfy;\r\n",
    );
    let (dir, context, result) = model("exact", source, "partial-expression");
    assert_eq!(
        spellings(&context, &result),
        [
            "1 div 0",
            "4 mod 0",
            "a[4]",
            "ln(0)",
            "min([])",
            "max(i in 1..0)(i)",
            "deopt(missing)"
        ]
    );
    assert_eq!(result.status(), 1);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Completed),
        "{result:?}"
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    for f in &result.findings {
        assert_eq!(f.rule, Rule::PartialExpression);
        assert_eq!(f.location.path, dir.join("root.mzn"));
        assert!(f.location.line >= 3);
        assert!(
            f.message
                .contains("refuted when this operation is evaluated"),
            "{}",
            f.message
        );
        assert!(!f.message.contains("runtime error"));
    }
    assert!(
        result.findings[2].message.contains("dimension 1")
            && result.findings[2].message.contains("declared at")
    );
    assert!(result.findings[3].message.contains("strictly positive"));
    assert!(result.findings[4].message.contains("nonempty"));
    assert!(result.findings[6].message.contains("present value"));
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "logarithm-subset",
        "float:x; float:a=ln(x); float:b=log(2,3); solve satisfy;",
        "partial-expression",
    );
    assert!(result.findings.is_empty());
    assert_eq!(result.status(), 0);
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("log(base,value)")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn candidate_indices_are_distinct_from_unknowns_and_scoped_protection() {
    let source = concat!(
        "array[1..3] of int:a; bool:keep; int:N=0; var {-1,1}:d; var opt int:maybe;\n",
        "constraint forall(i in 1..3)(a[i+1]>0);\n",
        "constraint forall(j in 1..3)(if j<3 then a[j+1]>0 else true endif);\n",
        "constraint forall(k in 1..3 where keep)(a[k+1]>0);\n",
        "var int:sparse=6 div d; var int:parameter=8 div N; var int:optional=deopt(maybe);\n",
        "constraint forall(e in 1..0)(a[e+1]>0); int:inactive=if false then 7 div 0 else 0 endif; var int:source=sum(s in {11 div 0} where false)(s); solve satisfy;\n",
    );
    let (dir, context, result) = model("candidates", source, "partial-expression");
    assert_eq!(spellings(&context, &result), ["a[i+1]", "11 div 0"]);
    assert!(
        result.findings[0]
            .message
            .contains("exact interpreted candidate indices")
    );
    assert!(
        result.findings[0].message.contains("does not establish")
            && result.findings[0].message.contains("feasible")
    );
    assert!(
        result.findings[0]
            .message
            .contains("nearest Boolean expression")
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn guards_default_capture_and_boolean_context_keep_their_distinct_meanings() {
    let source = concat!(
        "int:N; int:M; bool:choice;\n",
        "int:safe=if N!=0 then 6 div N else 0 endif; int:asserted=assert(N!=0,\"nonzero\",8 div N);\n",
        "int:captured=(1 div 0) default 9; int:fallback=<> default (2 div 0);\n",
        "bool:relational=(3 div 0=1) default true; constraint true -> (4 mod 0=1); bool:negative=not(5 div 0=1);\n",
        "int:selected=if choice then 10 div 0 else 0 endif; int:unknown_guard=if M=0 then 12 div M else 0 endif; solve satisfy;\n",
    );
    let (dir, context, result) = model("contexts", source, "partial-expression");
    assert_eq!(
        spellings(&context, &result),
        ["2 div 0", "3 div 0", "4 mod 0", "5 div 0", "10 div 0"]
    );
    assert!(result.findings[0].message.contains("default fallback"));
    for f in &result.findings[1..4] {
        assert!(
            f.message.contains("nearest Boolean expression"),
            "{}",
            f.message
        );
    }
    assert!(
        result.findings[4].message.contains("conditional")
            && result.findings[4].message.contains("choice")
    );
    assert!(result.limitations.is_empty(), "{:?}", result.limitations);
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, result) = model(
        "abort",
        "int:x=assert(false,\"abort\",1 div 0) default 2; solve satisfy;",
        "partial-expression",
    );
    assert!(result.findings.is_empty());
    assert_eq!(result.status(), 0);
    assert!(
        matches!(result.rules[0].outcome, RuleOutcome::Limited { .. }),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("aborting assertion")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn joint_selection_deduplicates_emitted_accesses_and_preserves_suppression() {
    let source = "array[1..3] of int:a; int:bad=a[4]; int:nested=a[4 div 0]; solve satisfy;";
    for (name, selection, expected) in [
        (
            "alone",
            "partial-expression",
            vec![
                ("a[4]", Rule::PartialExpression),
                ("4 div 0", Rule::PartialExpression),
            ],
        ),
        (
            "index",
            "index-set-mismatch",
            vec![("a[4]", Rule::IndexSetMismatch)],
        ),
        (
            "joint",
            "index-set-mismatch,partial-expression",
            vec![
                ("a[4]", Rule::IndexSetMismatch),
                ("4 div 0", Rule::PartialExpression),
            ],
        ),
    ] {
        let (dir, context, result) = model(name, source, selection);
        assert_eq!(
            result
                .findings
                .iter()
                .map(|f| (spelling(&context, f), f.rule))
                .collect::<Vec<_>>(),
            expected,
            "{result:?}"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    let source = "array[1..3] of int:a;\n% zincite-lint: ignore index-set-mismatch\nint:first=a[4];\n% zincite-lint: ignore partial-expression\nint:second=a[5]; solve satisfy;";
    let (dir, context, result) = model(
        "suppression",
        source,
        "index-set-mismatch,partial-expression",
    );
    assert_eq!(
        result
            .findings
            .iter()
            .map(|f| (spelling(&context, f), f.rule))
            .collect::<Vec<_>>(),
        [
            ("a[5]", Rule::IndexSetMismatch),
            ("a[4]", Rule::PartialExpression)
        ]
    );
    std::fs::remove_dir_all(dir).unwrap();
    let result = analyze_file(
        &zincite_syntax::parse("int:x=1 div 0; solve satisfy;"),
        std::path::Path::new("root.mzn"),
        0,
        &LintOptions::from_selection("partial-expression").unwrap(),
    );
    assert!(result.findings.is_empty());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(Rule::DEFAULT.len(), 2);
    assert_eq!(Rule::THESIS.len(), 14);
}
