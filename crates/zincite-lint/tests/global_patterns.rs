use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int:'..'(int:a,int:b); function set of $I:index_set(array[$I] of any $T:a);\n",
    "function bool:'!='($T:a,$T:b); function var bool:'!='(any $T:a,any $T:b); function var bool:'='(any $T:a,any $T:b);\n",
    "function bool:'<'(int:a,int:b); function var bool:'<='(var int:a,var int:b); function var bool:'>='(var int:a,var int:b);\n",
    "function var bool:'/\\'(var bool:a,var bool:b); predicate forall(array[int] of var opt bool:a);\n",
    "function var int:sum(array[int] of var int:a); function int:bool2int(bool:a); function var int:bool2int(var bool:a);\n",
    "function int:'+'(int:a,int:b); function int:'-'(int:a); annotation semantic;\n",
);
fn model(
    name: &str,
    source: &str,
) -> (
    PathBuf,
    ModelContext,
    BindingFacts,
    Vec<GlobalPatternFact>,
    AnalysisResult,
) {
    let dir = std::env::temp_dir().join(format!("zincite-patterns-{name}-{}", std::process::id()));
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
    let b = resolve_bindings(&context);
    let c = resolve_callables(&context, &b);
    let i = resolve_instantiations(&context, &b, &c);
    let d = resolve_domains(&context, &b);
    let definitions = resolve_definitions(&context, &b, &c, &i, &d);
    let n = resolve_numeric_facts(&context, &b, &c, &i, &d, &definitions);
    let o = resolve_optional_facts(&context, &b, &c, &i, &d, &n, &definitions);
    let g = resolve_guarded_facts_with_options(&context, &b, &c, &i, &d, &n, &o);
    let iteration = resolve_iteration_facts(&context, &b, &c, &i, &d, &n, &o, &g);
    let facts = resolve_global_patterns(&context, &b, &c, &o, &g, &iteration);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("global-constraint-opportunity").unwrap(),
    );
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    (dir, context, b, facts, result)
}
#[test]
fn both_complete_patterns_preserve_actual_indices_and_source_identities() {
    let source = concat!(
        "\u{feff}% π\r\narray[2..4] of var int:xs; array[-1..0] of int:cover; array[-1..0] of int:lower; array[-1..0] of int:upper;\r\n",
        "constraint forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]) :: \"pairs\";\r\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=lower[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=upper[k]); solve satisfy;\r\n",
    );
    let (dir, context, b, facts, result) = model("complete", source);
    assert_eq!(facts.len(), 2, "{facts:?}");
    let xs = b.declarations.iter().find(|d| d.name == "xs").unwrap();
    assert!(
        matches!(&facts[0].outcome,GlobalPatternOutcome::Complete(GlobalPattern::PairwiseDisequality{array,..}) if *array==xs.id)
    );
    assert!(
        matches!(
            facts[1].outcome,
            GlobalPatternOutcome::Complete(GlobalPattern::OccurrenceBounds { .. })
        ),
        "{facts:?}"
    );
    assert_eq!(result.findings.len(), 2, "{result:?}");
    assert_eq!(result.status(), 1);
    assert!(result.limitations.is_empty(), "{result:?}");
    assert!(result.findings[0].message.contains("all_different(xs)"));
    assert!(
        result.findings[1]
            .message
            .contains("open global_cardinality(xs, cover, lower, upper)")
    );
    assert!(
        result.findings[1]
            .message
            .contains("outside cover unrestricted")
    );
    for f in &result.findings {
        assert_eq!(f.location.path, dir.join("root.mzn"));
        assert!(
            f.message
                .contains("no automatic edit or solving-speed claim")
        );
        let file = context
            .files
            .iter()
            .find(|s| s.path == f.location.path)
            .unwrap();
        let range =
            f.location.range.start - file.byte_offset..f.location.range.end - file.byte_offset;
        assert!(file.parsed.source()[range].trim().starts_with("forall("));
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "enums",
        "enum I={I1,I2,I3}; enum V={A,B,C}; array[I] of var V:xs; constraint forall(i,j in index_set(xs) where i!=j)(xs[i]!=xs[j]); solve satisfy;",
    );
    assert!(
        matches!(
            facts[0].outcome,
            GlobalPatternOutcome::Complete(GlobalPattern::PairwiseDisequality { .. })
        ),
        "{facts:?}"
    );
    assert_eq!(result.findings.len(), 1, "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn incomplete_coverage_shifts_types_and_count_bounds_withhold_opportunities() {
    let source = concat!(
        "int:N; array[2..4] of var int:xs; array[2..4] of var int:ys; array[-1..0] of int:cover; array[-1..0] of int:other_cover; array[0..1] of int:wrong; array[-1..0] of var int:decision; array[1..0] of var int:empty; array[-1..0] of 1..1:positive_lower;\n",
        "constraint forall(i,j in 2..3 where i<j)(xs[i]!=xs[j]);\n",
        "constraint forall(i,j in index_set(xs) where i<j /\\ i<4)(xs[i]!=xs[j]);\n",
        "constraint forall(i,j in index_set(xs) where i<j)(xs[i+1]!=xs[j]);\n",
        "constraint forall(i,j in index_set(xs) where i<j)(xs[i]!=ys[j]);\n",
        "constraint forall(i in 1..3,j in 2..4 where i<j)(xs[i]!=xs[j]);\n",
        "constraint forall(i,j in 1..N where i<j)(xs[i]!=xs[j]);\n",
        "constraint forall(k in index_set(cover))(sum(i in 2..3)(bool2int(xs[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=cover[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs) where i<4)(bool2int(xs[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=cover[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=wrong[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=wrong[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=decision[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=decision[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i,j in index_set(xs))(bool2int(xs[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=cover[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=other_cover[k]))<=cover[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(empty))(bool2int(empty[i]=cover[k]))>=positive_lower[k] /\\ sum(j in index_set(empty))(bool2int(empty[j]=cover[k]))<=cover[k]); solve satisfy;\n",
    );
    let (dir, _, _, facts, result) = model("excluded", source);
    assert!(
        facts
            .iter()
            .all(|f| !matches!(f.outcome, GlobalPatternOutcome::Complete(_))),
        "{facts:?}"
    );
    assert!(result.findings.is_empty(), "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn user_identity_optional_partiality_annotations_and_suppression_are_preserved() {
    let source = concat!(
        "array[1..3] of var int:xs; array[1..3] of var opt int:optional; array[1..2] of int:cover;\n",
        "constraint forall(i,j in index_set(optional) where i<j)(optional[i]!=optional[j]);\n",
        "% zincite-lint: ignore global-constraint-opportunity\nconstraint forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]);\n",
        "constraint forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]) :: semantic;\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(optional))(bool2int(optional[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(optional))(bool2int(optional[j]=cover[k]))<=cover[k]);\n",
        "constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=cover[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k+1]))<=cover[k]); solve satisfy;\n",
    );
    let (dir, _, _, _, result) = model("boundaries", source);
    assert!(result.findings.is_empty(), "{result:?}");
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("annotation")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "overload",
        "function var bool:'!='(var int:a,var int:b)=true; array[1..3] of var int:xs; constraint forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]); solve satisfy;",
    );
    assert!(
        facts
            .iter()
            .all(|f| !matches!(f.outcome, GlobalPatternOutcome::Complete(_)))
    );
    assert!(result.findings.is_empty(), "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "user-forall",
        "predicate forall(array[int] of var opt bool:a)=true; array[1..3] of var int:xs; constraint forall(i,j in index_set(xs) where i<j)(xs[i]!=xs[j]); solve satisfy;",
    );
    assert!(
        facts
            .iter()
            .all(|f| !matches!(f.outcome, GlobalPatternOutcome::Complete(_)))
            && result.findings.is_empty(),
        "{facts:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "unrelated",
        "function var bool:opaque(); constraint forall(i in 1..3)(opaque() /\\ opaque()); solve satisfy;",
    );
    assert!(
        facts.is_empty() && result.findings.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    assert!(matches!(result.rules[0].outcome, RuleOutcome::Completed));
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "enum-mismatch",
        "enum X={A,B,C}; enum Y={D,E,F}; array[1..3] of var X:xs; array[1..2] of Y:cover; array[1..2] of int:lower; array[1..2] of int:upper; constraint forall(k in index_set(cover))(sum(i in index_set(xs))(bool2int(xs[i]=cover[k]))>=lower[k] /\\ sum(j in index_set(xs))(bool2int(xs[j]=cover[k]))<=upper[k]); solve satisfy;",
    );
    assert!(
        facts
            .iter()
            .all(|f| !matches!(f.outcome, GlobalPatternOutcome::Complete(_)))
            && result.findings.is_empty(),
        "{facts:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
