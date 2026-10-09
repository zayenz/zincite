use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function set of int:'..'(int:a,int:b); function int:'+'(int:a,int:b); function int:'div'(int:a,int:b); function int:'mod'(int:a,int:b);\n",
    "function bool:'!='(int:a,int:b); function bool:'>'(int:a,int:b); function bool:'='(any $T:a,any $T:b); function bool:'in'(int:a,set of int:b);\n",
    "function bool:'/\\'(bool:a,bool:b); function bool:'->'(bool:a,bool:b);\n",
    "function int:sum(array[int] of int:a); function int:product(array[int] of int:a); function int:min(array[int] of int:a); function int:max(array[int] of int:a); function int:min(set of int:a); function int:min(array[int] of opt int:a);\n",
    "function int:length(array[$I] of any $T:a); function int:card(set of int:a);\n",
    "function set of $I:index_set(array[$I] of any $T:a); function set of $I:index_set_1of2(array[$I,$J] of any $T:a); function set of $J:index_set_2of2(array[$I,$J] of any $T:a);\n",
    "function bool:index_sets_agree(array[$I] of any $T:a,array[$J] of any $U:b);\n",
    "function bool:assert(bool:c,string:m); function $T:assert(bool:c,string:m,$T:v);\n",
    "function $T:'default'(opt $T:x,$T:y); function float:int2float(int:x); function float:ln(float:x); function float:log2(float:x); function float:log10(float:x); function float:log(float:b,float:x);\n"
);
fn model(
    name: &str,
    source: &str,
    selection: &str,
) -> (
    PathBuf,
    ModelContext,
    BindingFacts,
    Vec<CallableInputFact>,
    AnalysisResult,
) {
    let dir = std::env::temp_dir().join(format!("zincite-input-{name}-{}", std::process::id()));
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
    let defs = resolve_definitions(&context, &b, &c, &i, &d);
    let n = resolve_numeric_facts(&context, &b, &c, &i, &d, &defs);
    let o = resolve_optional_facts(&context, &b, &c, &i, &d, &n, &defs);
    let g = resolve_guarded_facts_with_options(&context, &b, &c, &i, &d, &n, &o);
    let it = resolve_iteration_facts(&context, &b, &c, &i, &d, &n, &o, &g);
    let facts = resolve_callable_input_facts(&context, &b, &c, &g, &it);
    let result = analyze_model(&context, &LintOptions::from_selection(selection).unwrap());
    assert!(result.errors.is_empty(), "{result:?}");
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    (dir, context, b, facts, result)
}
fn warned(b: &BindingFacts, f: &[CallableInputFact]) -> Vec<String> {
    f.iter()
        .filter(|f| {
            !f.captured_or_skipped
                && f.obligation.context.activation != GuardActivation::Inactive
                && matches!(
                    f.obligation.outcome,
                    GuardedOutcome::Unknown | GuardedOutcome::Refuted
                )
        })
        .map(|f| b.declarations[f.callable.0].name.clone())
        .collect()
}
#[test]
fn arrays_keep_source_dimension_identity_and_explicit_matching_guards() {
    let (dir, _, _, facts, result) = model(
        "unsupported-alias",
        "function array[int,int] of int:opaque(array[int,int] of int:a); function int:alias(array[int,int] of int:a,int:i,int:j)=let {array[int,int] of int:b=opaque(a);} in b[i,j]; solve satisfy;",
        "missing-input-precondition",
    );
    assert_eq!(facts.len(), 2, "{facts:?}");
    assert!(
        facts
            .iter()
            .all(|fact| matches!(fact.obligation.outcome, GuardedOutcome::Unsupported(_)))
    );
    assert_eq!(
        facts
            .iter()
            .map(|fact| match fact.obligation.kind {
                GuardObligationKind::Index { dimension, .. } => dimension,
                _ => panic!("{fact:?}"),
            })
            .collect::<Vec<_>>(),
        [1, 2]
    );
    assert_eq!(result.limitations.len(), 1, "{result:?}");
    assert!(result.findings.is_empty(), "{result:?}");
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "function int:cross(array[int] of int:a,array[int] of int:b)=sum(i in index_set(a))(b[i]);\n",
        "function int:own(array[int] of int:a)=sum(i in index_set(a))(a[i]);\n",
        "function int:own_zero(array[int] of int:a)=sum(i in index_set(a))(a[i+0]); function int:own_shift(array[int] of int:a)=sum(i in index_set(a))(a[i+1]);\n",
        "function int:declared(array[1..3] of int:a,array[1..3] of int:b)=sum(i in index_set(a))(b[i]);\n",
        "function int:offset(array[1..3] of int:a,array[2..4] of int:b)=sum(i in index_set(a))(b[i]);\n",
        "function int:two(array[int,int] of int:a,array[int,int] of int:b)=sum(i in index_set_1of2(a),j in index_set_2of2(a))(b[i,j]);\n",
        "function int:own_two(array[int,int] of int:a)=sum(i in index_set_1of2(a),j in index_set_2of2(a))(a[i,j]);\n",
        "function int:equal(array[int] of int:a,array[int] of int:b)=if index_set(a)=index_set(b) then sum(i in index_set(a))(b[i]) else 0 endif;\n",
        "function int:second_equal(array[1..1,int] of int:a,array[1..1,int] of int:b)=if index_set_2of2(a)=index_set_2of2(b) then sum(j in index_set_2of2(a))(b[1,j]) else 0 endif;\n",
        "function int:same_length(array[1..3] of int:a,array[2..4] of int:b)=if length(a)=length(b) then sum(i in index_set(a))(b[i]) else 0 endif;\n",
        "function int:member(array[int] of int:a,int:i)=if i in index_set(a) then a[i] else 0 endif;\n",
        "function int:asserted(array[int] of int:a,array[int] of int:b)=assert(index_sets_agree(a,b),\"same\",sum(i in index_set(a))(b[i]));\n",
        "function int:local_assert(array[int] of int:a,array[int] of int:b)=let {constraint assert(index_sets_agree(a,b),\"same\");} in sum(i in index_set(a))(b[i]); solve satisfy;\n"
    );
    let (dir, _, b, facts, result) = model("arrays", source, "missing-input-precondition");
    assert_eq!(
        warned(&b, &facts),
        ["cross", "own_shift", "offset", "two", "two", "same_length"]
    );
    assert_eq!(result.findings.len(), 6, "{result:?}");
    assert!(result.limitations.is_empty(), "{result:?}");
    let two: Vec<_> = facts
        .iter()
        .filter(|f| b.declarations[f.callable.0].name == "two")
        .collect();
    assert_eq!(two.len(), 2);
    for (j, f) in two.iter().enumerate() {
        let GuardObligationKind::Index {
            dimension,
            selection: Some(s),
            ..
        } = &f.obligation.kind
        else {
            panic!("{f:?}")
        };
        assert_eq!(*dimension, j + 1);
        assert_eq!(s.array_dimension.unwrap().1, j + 1);
        assert_eq!(b.declarations[s.array_dimension.unwrap().0.0].name, "a");
    }
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.message.contains("does not establish a failing input"))
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn numeric_and_nonempty_contracts_are_local_general_and_operation_specific() {
    let source = concat!(
        "int:d=1; constraint d!=0;\n",
        "function int:divide(int:d=1)=10 div d; function int:modulo(int:d)=10 mod d; function int:nonzero(1..3:d)=10 div d;\n",
        "function float:plain(int:x)=ln(x); function float:positive(1..3:x)=ln(x); function float:nonnegative(0..3:x)=log2(x); function float:both({-1,1}:x)=log10(x);\n",
        "function float:guarded(int:x)=if x>0 then ln(int2float(x)) else 0.0 endif; function float:lazy(int:x)=assert(x>0,\"positive\",ln(x));\n",
        "function int:minimum(array[int] of int:a)=min(a); function int:nonempty(array[1..3] of int:a)=min(a);\n",
        "function int:length_guard(array[int] of int:a)=if length(a)>0 then min(a) else 0 endif; function int:optional(array[1..3] of opt int:a)=if length(a)>0 then min(a) else 0 endif;\n",
        "function int:set_guard(set of int:s)=if card(s)>0 then min(s) else 0 endif; function int:lazy_min(array[int] of int:a)=assert(length(a)>0,\"nonempty\",max(a)); function int:let_min(array[int] of int:a)=let {constraint assert(length(a)>0,\"nonempty\");} in min(a);\n",
        "function int:let_guard(int:d)=let {constraint assert(d!=0,\"nz\"); int:alias=d;} in 10 div alias;\n",
        "function int:conditional(bool:flag,int:d)=let {constraint if flag then assert(d!=0,\"nz\") else true endif;} in 10 div d;\n",
        "function bool:self(int:d)=assert(10 div d>0,\"positive\"); function bool:strict(int:d)=d!=0 /\\ 10 div d>0;\n",
        "function int:fallback(int:d)=(10 div d) default 0; function int:empty_sum(array[int] of int:a)=sum(a);\n",
        "function float:floating(float:x)=ln(x); function float:general(int:x)=log(2,x); function int:opaque(int:x); function int:unsupported(int:x)=1 div opaque(x);\n",
        "function int:hidden(int:x)=let {int:y=opaque(x);} in 1 div y; function float:overflow(int:x)=ln((9223372036854775807+1)+x); function int:inactive(int:d)=if false then 10 div d else 0 endif; solve satisfy;\n"
    );
    let (dir, _, b, facts, result) = model("numeric", source, "missing-input-precondition");
    assert_eq!(
        warned(&b, &facts),
        [
            "divide",
            "modulo",
            "plain",
            "nonnegative",
            "both",
            "minimum",
            "optional",
            "conditional",
            "self",
            "strict"
        ],
        "{facts:?}"
    );
    assert_eq!(result.findings.len(), 10, "{result:?}");
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("logarithm"))
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("opaque"))
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("overflow")),
        "{result:?}"
    );
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("log(base,value)")),
        "{result:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, _, _, facts, result) = model(
        "overloads",
        "function int:'div'(int:a,int:b)=1; function float:ln(int:x)=1.0; function int:user_div(int:x)=10 div x; function float:user_log(int:x)=ln(x); solve satisfy;",
        "missing-input-precondition",
    );
    assert!(facts.is_empty());
    assert!(result.findings.is_empty(), "{result:?}");
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn joint_selection_keys_suppress_only_emitted_matching_obligations_and_preserve_bytes() {
    let source = "\u{feff}% π\r\narray[1..3,1..3] of int:captured;\r\nfunction int:mixed(int:i)=captured[i,4];\r\nfunction int:bad(array[1..3,1..3] of int:a)=a[4,5]+(1 div 0);\r\n% zincite-lint: ignore missing-input-precondition\r\nfunction int:suppressed(array[1..3] of int:a)=a[4]; solve satisfy;\r\n";
    for (selection, boundary, partial) in [
        ("missing-input-precondition", 3, 0),
        ("partial-expression", 0, 5),
        ("missing-input-precondition,partial-expression", 3, 3),
    ] {
        let (dir, context, _, _, result) = model("joint", source, selection);
        assert_eq!(
            result
                .findings
                .iter()
                .filter(|f| f.rule == Rule::MissingInputPrecondition)
                .count(),
            boundary,
            "{result:?}"
        );
        assert_eq!(
            result
                .findings
                .iter()
                .filter(|f| f.rule == Rule::PartialExpression)
                .count(),
            partial,
            "{result:?}"
        );
        assert!(
            result
                .findings
                .iter()
                .all(|f| f.location.range.start >= 3 && f.location.line >= 2)
        );
        assert_eq!(context.files[0].byte_offset, 3);
        assert_eq!(result.status(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
