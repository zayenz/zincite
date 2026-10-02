use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int:'..'(int:a,int:b); function int:'+'(int:a,int:b); function int:'div'(int:a,int:b);\n",
    "function bool:'!='(int:a,int:b); function bool:'>'(int:a,int:b); function bool:'<'(int:a,int:b); function bool:'='(int:a,int:b);\n",
    "function var bool:'>'(var int:a,var int:b); function var bool:'!='(var int:a,var int:b);\n",
    "function var int:sum(array[int] of var int:a); function var int:product(array[int] of var int:a); function var int:min(array[int] of var int:a);\n",
    "predicate forall(array[int] of var opt bool:a); function var bool:exists(array[int] of var opt bool:a);\n",
    "function $T:assert(bool:c,string:m,$T:v); function $T:trace(string:m,$T:v); function $T:deopt(opt $T:x);\n",
    "function set of $I:index_set(array[$I] of any $T:a); annotation semantic;\n"
);
fn model(
    name: &str,
    source: &str,
    options: &LintOptions,
) -> (
    PathBuf,
    ModelContext,
    BindingFacts,
    Vec<ComprehensionStructureFact>,
    AnalysisResult,
) {
    let dir = std::env::temp_dir().join(format!("zincite-expansion-{name}-{}", std::process::id()));
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
    let structure =
        resolve_comprehension_structure(&context, &b, &c, &i, &d, &n, &o, &g, &iteration);
    let result = analyze_model(&context, options);
    assert!(result.errors.is_empty(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    (dir, context, b, structure, result)
}
fn options() -> LintOptions {
    LintOptions::from_selection("expensive-comprehension").unwrap()
}
fn text<'a>(context: &'a ModelContext, file: FileId, location: &SourceLocation) -> &'a str {
    let s = &context.files[file];
    s.parsed.source()[location.range.start - s.byte_offset..location.range.end - s.byte_offset]
        .trim()
}
#[test]
fn checked_thresholds_symbolic_dimensions_and_optional_capacity_remain_distinct() {
    let source = concat!(
        "int:N; int:K; var set of 1..5:s; var bool:choose;\n",
        "array[int] of int:equal=[1 | _,_ in 1..1000]; array[int] of int:above=[1 | _,_ in 1..1001];\n",
        "array[int] of int:symbolic=[1 | i in 1..N,j in 1..K]; array[int] of int:single=[1 | i in 1..N];\n",
        "array[int] of var opt int:universe=[1 | i in s,j in 1..4]; array[int] of var opt int:filtered=[i | i in 1..4,j in 1..5 where choose]; array[int] of int:empty=[1 | _,_ in 1..0];\n",
        "array[int] of int:overflow=[1 | _,_ in 1..5000000000]; solve satisfy;\n"
    );
    let (dir, context, b, facts, result) = model("counts", source, &options());
    assert!(
        facts
            .iter()
            .any(|f| matches!(f.candidates, CandidateCount::Exact(1_000_000)))
    );
    assert!(
        facts
            .iter()
            .any(|f| matches!(f.candidates, CandidateCount::Exact(1_002_001)))
    );
    assert!(
        facts
            .iter()
            .any(|f| matches!(f.candidates, CandidateCount::UpperBound(20)))
    );
    assert!(
        facts
            .iter()
            .any(|f| matches!(f.candidates, CandidateCount::Exact(0)))
    );
    let product = facts
        .iter()
        .find(|f| text(&context, f.file, &f.location).contains("i in 1..N,j in 1..K"))
        .unwrap();
    let CandidateCount::Symbolic { factors, .. } = &product.candidates else {
        panic!("{product:?}")
    };
    assert_eq!(factors.len(), 2);
    assert_eq!(
        text(&context, factors[0].file, &factors[0].location),
        "1..N"
    );
    assert_eq!(
        text(&context, factors[1].file, &factors[1].location),
        "1..K"
    );
    assert!(
        factors[0]
            .dependencies
            .iter()
            .any(|id| b.declarations[id.0].name == "N")
    );
    assert_eq!(result.findings.len(), 2, "{result:?}");
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.message.contains("1002001 exceeds max-candidates 1000000"))
    );
    assert!(result.findings.iter().any(|f| {
        f.message.contains("symbolic Cartesian")
            && f.message
                .contains("no numeric count or threshold exceedance")
    }));
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("overflow")),
        "{result:?}"
    );
    let mut report = vec![];
    assert_eq!(write_analysis(&result, &mut report).unwrap(), 1);
    assert!(
        String::from_utf8(report)
            .unwrap()
            .contains("potential expansion")
    );
    std::fs::remove_dir_all(dir).unwrap();
    let mut bounded = options();
    bounded.parameters.comprehension_max_candidates = std::num::NonZeroU64::new(19).unwrap();
    let (dir, _, _, _, result) = model(
        "upper-bound",
        "var set of 1..5:s; var bool:choose; array[int] of var opt int:universe=[1|i in s,j in 1..4]; array[int] of var opt int:filtered=[i|i in 1..4,j in 1..5 where choose]; solve satisfy;",
        &bounded,
    );
    assert_eq!(result.findings.len(), 2, "{result:?}");
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.message.contains("upper bound 20 exceeds")
                && f.message.contains("not selected/present"))
    );
    std::fs::remove_dir_all(dir).unwrap();
    let settings=zincite_lint::settings::LintSettings::from_toml("[lint]\nselect=['preset:small']\n[lint.presets.small]\nselect=['expensive-comprehension']\n[lint.presets.small.options.expensive-comprehension]\nmax-candidates=20\n[lint.options.expensive-comprehension]\nmax-candidates=9").unwrap();
    let resolved = settings.resolve().unwrap();
    assert_eq!(resolved.parameters.comprehension_max_candidates.get(), 9);
    let (dir, _, _, _, result) = model(
        "override",
        "array[int] of int:equal=[1|_,_ in 1..3];\n% zincite-lint: ignore expensive-comprehension\narray[int] of int:suppressed=[1|_,_ in 1..4]; array[int] of int:above=[1|i in 1..3,j in 1..4]; solve satisfy;",
        &resolved,
    );
    assert_eq!(result.findings.len(), 1, "{result:?}");
    assert!(
        result.findings[0]
            .message
            .contains("count 12 exceeds max-candidates 9")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn earlier_filter_aggregate_and_enclosing_alias_retain_prefix_evidence() {
    let source = concat!(
        "int:d;\n",
        "array[int] of int:filter=[i+j | i in 1..3,j in 1..5 where i>1];\n",
        "constraint :: \"label\" forall(i in 1..3,j in 1..5 where i>1)(true);\n",
        "array[int] of var int:repeated=[sum(k in 1..3)(k) | j in 1..5];\n",
        "array[int] of int:guarded=if d!=0 then [1 | i in 1..3,j in 1..5 where 6 div d>0] else [] endif;\n",
        "array[int] of var int:alias=let {int:constant=3;} in [sum(k in 1..3)(constant+k) | j in 1..5];\n",
        "array[int] of var int:nested=[if d!=0 then sum(j in 1..4)(sum(k in 1..3)(6 div d)) else 0 endif | i in 1..5];\n",
        "array[int] of var int:body_local=[let {int:local=3;} in sum(k in 1..3)(local+k) | j in 1..5]; solve satisfy;\n"
    );
    let (dir, context, b, facts, result) = model("placement", source, &options());
    let placements: Vec<_> = facts
        .iter()
        .flat_map(|f| f.placements.iter().map(move |p| (f.file, p)))
        .collect();
    let (_, filter) = placements
        .iter()
        .find(|(file, p)| text(&context, *file, &p.location) == "i>1")
        .unwrap();
    assert_eq!(filter.earliest_scope, 1);
    assert_eq!(filter.safety, GuardedOutcome::Proven, "{filter:?}");
    assert_eq!(filter.crossed_bindings.len(), 1);
    assert_eq!(b.declarations[filter.crossed_bindings[0].0].name, "j");
    let prefix = &filter.prospective.as_ref().unwrap().context;
    assert!(prefix.assumptions.iter().any(|a|matches!(&a.kind,GuardAssumptionKind::GeneratorMembership{declaration,..} if b.declarations[declaration.0].name=="i")));
    assert!(!prefix.assumptions.iter().any(|a|matches!(&a.kind,GuardAssumptionKind::GeneratorMembership{declaration,..} if b.declarations[declaration.0].name=="j")));
    let (_, guard) = placements
        .iter()
        .find(|(file, p)| text(&context, *file, &p.location) == "6 div d>0")
        .unwrap();
    assert_eq!(guard.safety, GuardedOutcome::Proven, "{guard:?}");
    assert!(
        guard
            .obligations
            .iter()
            .any(|o| matches!(o.kind, GuardObligationKind::Nonzero)
                && o.outcome == GuardedOutcome::Proven)
    );
    assert!(
        placements
            .iter()
            .any(
                |(file, p)| text(&context, *file, &p.location) == "sum(k in 1..3)(constant+k)"
                    && p.safety == GuardedOutcome::Proven
            ),
        "{placements:?}"
    );
    assert!(
        !placements
            .iter()
            .any(
                |(file, p)| text(&context, *file, &p.location) == "sum(k in 1..3)(local+k)"
                    && p.safety == GuardedOutcome::Proven
            )
    );
    assert_eq!(result.findings.len(), 6, "{result:?}");
    assert!(result.limitations.is_empty(), "{result:?}");
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("compiler may already optimize"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn lost_guards_skipped_work_effects_and_unknown_domains_never_prove_movement() {
    let source = concat!(
        "int:d; int:N; opt int:o; array[1..3] of int:a;\n",
        "array[int] of int:lost=[1 | i in 1..3 where d!=0,j in 1..5 where 6 div d>0];\n",
        "array[int] of var int:branch=[if d!=0 then sum(k in 1..3)(6 div d) else 0 endif | j in 1..5];\n",
        "array[int] of var int:double_branch=if d!=0 then [if N>0 then sum(k in 1..3)(k) else 0 endif | j in 1..5] else [] endif;\n",
        "array[int] of var int:skipped=[if false then sum(k in 1..3)(1 div 0) else 0 endif | j in 1..5];\n",
        "array[int] of var int:empty=[sum(k in 1..3)(k) | j in 1..0]; array[int] of var int:unknown=[sum(k in 1..3)(k) | j in 1..N];\n",
        "array[int] of int:abort=[1 | i in 1..3,j in 1..5 where assert(true,\"stop\",true),k in 1..4 where i>1];\n",
        "array[int] of var int:effect=[sum(k in 1..3)(trace(\"seen\",k)) | j in 1..5];\n",
        "array[int] of var int:semantic=[sum(k in 1..3)(k) :: semantic | j in 1..5];\n",
        "constraint forall(i in 1..3,j in 1..5 where i>1)(true) :: semantic;\n",
        "array[int] of int:index=[1 | i in 1..3,j in 1..5 where a[d]>0];\n",
        "array[int] of int:presence=[1 | i in 1..3,j in 1..5 where deopt(o)>0];\n",
        "array[int] of var int:minimum=[min(k in 1..0)(k) | j in 1..5]; solve satisfy;\n"
    );
    let (dir, context, _, facts, result) = model("unsafe", source, &options());
    for f in &facts {
        for p in &f.placements {
            if text(&context, f.file, &p.location) != "d!=0" {
                assert_ne!(p.safety, GuardedOutcome::Proven, "{p:?}");
            }
        }
    }
    assert_eq!(result.findings.len(), 1, "{result:?}");
    assert!(
        result.findings[0]
            .message
            .contains("whole parameter filter")
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("assertion")),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("annotation")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
