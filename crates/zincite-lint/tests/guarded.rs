use std::path::PathBuf;
use zincite_lint::{
    DefinitionEnforcement, GuardActivation, GuardEvaluation, GuardObligationKind,
    GuardedExpression, GuardedFacts, GuardedOutcome, ModelContext, ModelOptions, NumericOutcome,
    Presence, load_model, resolve_bindings, resolve_callables, resolve_definitions,
    resolve_domains, resolve_guarded_facts, resolve_guarded_facts_with_options,
    resolve_instantiations, resolve_numeric_facts, resolve_optional_facts,
};

const CORE: &str = concat!(
    "function bool: '='(int: a,int: b); function bool: '!='(int: a,int: b);\n",
    "function bool: '<'(int: a,int: b); function bool: '<='(int: a,int: b);\n",
    "function bool: '>'(int: a,int: b); function bool: '>='(int: a,int: b);\n",
    "function int: '-'(int: a,int: b);\n",
    "function var bool: '='(var int: a,var int: b); function var bool: '!='(var int: a,var int: b);\n",
    "function var bool: '<'(var int: a,var int: b); function var bool: '<='(var int: a,var int: b);\n",
    "function var bool: '>'(var int: a,var int: b); function var bool: '>='(var int: a,var int: b);\n",
    "function var bool: 'in'(var int: a,set of int: b);\n",
    "function var bool: '/\\'(var bool: a,var bool: b); function var bool: '\\/'(var bool: a,var bool: b);\n",
    "function var bool: '->'(var bool: a,var bool: b); function var bool: 'not'(var bool: a);\n",
    "function int: 'div'(int: a,int: b); function int: 'mod'(int: a,int: b);\n",
    "function int: '-'(int: a); function var int: 'div'(var int: a,var int: b);\n",
    "function var int: 'mod'(var int: a,var int: b);\n",
    "function var bool: forall(array[int] of var opt bool: a);\n",
    "function var bool: exists(array[int] of var opt bool: a);\n",
    "function var int: min(array[int] of var opt int: a);\n",
    "function int: min(set of int: a);\n",
    "function var int: min(array[int] of var int: a); function var int: max(array[int] of var int: a);\n",
    "function var int: sum(array[int] of var int: a); function var int: product(array[int] of var int: a);\n",
    "function int: length(array[$I] of $T: a);\n",
    "function set of $I: index_set(array[$I] of $T: a);\n",
    "function bool: assert(bool: condition,string: message);\n",
    "function $T: assert(bool: condition,string: message,$T: value);\n",
    "test occurs(opt $T:x); test absent(opt $T:x); function $T: deopt(opt $T:x);\n",
    "function var bool: occurs(var opt $T:x); function var bool: absent(var opt $T:x); function var $T: deopt(var opt $T:x);\n",
    "function $T: 'default'(opt $T:x,$T:y); function var $T: 'default'(var opt $T:x,var $T:y);\n",
    "function int: '+'(opt int:a,opt int:b); function int: '*'(opt int:a,opt int:b);\n",
    "function var int: '+'(var opt int:a,var opt int:b); function var int: '*'(var opt int:a,var opt int:b);\n",
    "function var int: sum(array[int] of var opt int:a);\n",
    "function opt int: '~+'(opt int:a,opt int:b);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, GuardedFacts) {
    model_using_options(name, source, false)
}
fn model_using_options(
    name: &str,
    source: &str,
    options: bool,
) -> (PathBuf, ModelContext, GuardedFacts) {
    let dir = std::env::temp_dir().join(format!("zincite-guarded-{name}-{}", std::process::id()));
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
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let facts = if options {
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        )
    } else {
        resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric)
    };
    (dir, context, facts)
}
fn text<'a>(
    context: &'a ModelContext,
    file: usize,
    location: &zincite_lint::SourceLocation,
) -> &'a str {
    let source = &context.files[file];
    source.parsed.source()
        [location.range.start - source.byte_offset..location.range.end - source.byte_offset]
        .trim()
}
fn expression<'a>(
    context: &ModelContext,
    facts: &'a GuardedFacts,
    source: &str,
) -> &'a GuardedExpression {
    facts
        .expressions
        .iter()
        .find(|e| text(context, e.file, &e.location) == source)
        .unwrap_or_else(|| panic!("missing {source}: {:?}", facts.expressions))
}
fn obligation<'a>(
    context: &ModelContext,
    facts: &'a GuardedFacts,
    source: &str,
) -> &'a zincite_lint::GuardObligation {
    facts
        .obligations
        .iter()
        .find(|o| text(context, o.file, &o.operation) == source)
        .unwrap_or_else(|| panic!("missing obligation {source}: {:?}", facts.obligations))
}

#[test]
fn local_guards_preserve_obligations_identity_and_boolean_ranges() {
    let source = concat!(
        "\u{feff}% é\r\nint: missing; int: N=2; int: K=2; var -1..1: d;\n",
        "array[1..N, {1,3}] of int: grid; array[1..2] of int: a;\n",
        "var int: unguarded=6 div d; var int: guarded=if d!=0 then 7 div d else 0 endif;\n",
        "var int: unknown=8 div missing; var int: same_index=grid[N,1]; var int: other_index=grid[K,1];\n",
        "var int: sparse_bad=grid[N,2]; var 0..3: i;\n",
        "var int: protected=if i in index_set(a) then a[i] else 0 endif;\n",
        "var bool: reified=(a[0]=a[0]); constraint not (a[3]=1);\n",
        "constraint false -> d!=0; constraint d!=0 \\/ true;\n",
        "constraint forall(j in 1..N)(grid[j,1]=1); constraint forall(q in 1..K)(grid[q,1]=1); solve satisfy;\n",
    );
    let (dir, context, facts) = model("guards", source);
    assert_eq!(
        obligation(&context, &facts, "6 div d").outcome,
        GuardedOutcome::Unknown
    );
    let protected = obligation(&context, &facts, "7 div d");
    assert_eq!(protected.invariant, GuardedOutcome::Unknown);
    assert_eq!(protected.outcome, GuardedOutcome::Proven);
    assert_eq!(text(&context, protected.file, &protected.operand), "d");
    assert_eq!(
        protected.context.evaluation,
        GuardEvaluation::DecisionBranch
    );
    assert_eq!(
        obligation(&context, &facts, "8 div missing").outcome,
        GuardedOutcome::Unknown
    );
    let indices: Vec<_> = facts
        .obligations_at(
            expression(&context, &facts, "grid[N,1]").file,
            &expression(&context, &facts, "grid[N,1]").location,
        )
        .collect();
    assert_eq!(indices.len(), 2);
    assert!(matches!(
        indices[1].kind,
        GuardObligationKind::Index { dimension: 2, .. }
    ));
    assert_eq!(indices[0].outcome, GuardedOutcome::Unknown);
    assert_eq!(
        obligation(&context, &facts, "grid[j,1]").outcome,
        GuardedOutcome::Proven
    );
    assert_eq!(
        obligation(&context, &facts, "grid[q,1]").outcome,
        GuardedOutcome::Unknown
    );
    assert_eq!(
        obligation(&context, &facts, "grid[K,1]").outcome,
        GuardedOutcome::Unknown
    );
    assert!(
        facts
            .obligations
            .iter()
            .any(|o| text(&context, o.file, &o.operation) == "grid[N,2]"
                && o.outcome == GuardedOutcome::Refuted)
    );
    assert_eq!(
        obligation(&context, &facts, "a[i]").outcome,
        GuardedOutcome::Proven
    );
    let partial = expression(&context, &facts, "a[0]=a[0]");
    assert_eq!(partial.truth, Some(GuardedOutcome::Refuted));
    assert_eq!(partial.raw_definedness, GuardedOutcome::Refuted);
    assert_eq!(partial.definedness, GuardedOutcome::Proven);
    assert_eq!(
        text(
            &context,
            partial.file,
            partial.context.nearest_boolean.as_ref().unwrap()
        ),
        "a[0]=a[0]"
    );
    assert_eq!(
        expression(&context, &facts, "not (a[3]=1)").truth,
        Some(GuardedOutcome::Proven)
    );
    assert_eq!(partial.location.path, dir.join("root.mzn"));
    assert_eq!(&source[partial.location.range.clone()], "a[0]=a[0]");
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn lazy_branches_assertions_and_generators_keep_assumptions_scoped() {
    let source = concat!(
        "int: d; int: e; annotation mystery; var int: skipped=if false then 1 div 0 else 7 endif;\n",
        "var int: elseif_skipped=if true then 9 elseif assert(false,\"later\") then 2 else 3 endif;\n",
        "int: checked=assert((d!=0),\"nonzero\",6 div d); constraint assert((d!=0),\"root\");\n",
        "int: locally_checked=if d!=0 then assert(d!=0,\"local\",1) else 0 endif;\n",
        "int: local_only=if e!=0 then assert(e!=0,\"conditional\",1) else 0 endif;\n",
        "constraint (e!=0)::mystery; int: outside_e=9 div e;\n",
        "int: failed=assert(false,\"failed\",2 div 0);\n",
        "array[1..3] of int: a; constraint forall(i in index_set(a) where i!=0)(a[i]=8 div i);\n",
        "constraint forall([a[j]=7 div j | j in index_set(a) where j!=0 /\\ j>0]);\n",
        "set of int: S; var int: filtered_sum=sum(i in 1..1 where false,j in S)(11 div 0);\n",
        "constraint forall(k in {})(a[k]=1); var int: outside=5 div 0; solve satisfy;\n",
    );
    let (dir, context, facts) = model("contexts", source);
    let skipped = obligation(&context, &facts, "1 div 0");
    assert_eq!(skipped.outcome, GuardedOutcome::Refuted);
    assert_eq!(skipped.context.activation, GuardActivation::Inactive);
    assert_eq!(skipped.context.evaluation, GuardEvaluation::ParameterBranch);
    assert_eq!(
        expression(&context, &facts, "if false then 1 div 0 else 7 endif").definedness,
        GuardedOutcome::Proven
    );
    assert_eq!(
        expression(
            &context,
            &facts,
            "if true then 9 elseif assert(false,\"later\") then 2 else 3 endif"
        )
        .definedness,
        GuardedOutcome::Proven
    );
    assert_eq!(
        obligation(&context, &facts, "assert((d!=0),\"root\")").outcome,
        GuardedOutcome::Unknown
    );
    let local_assertion = obligation(&context, &facts, "assert(d!=0,\"local\",1)");
    assert_eq!(local_assertion.invariant, GuardedOutcome::Unknown);
    assert_eq!(local_assertion.outcome, GuardedOutcome::Proven);
    assert_eq!(
        obligation(&context, &facts, "9 div e").outcome,
        GuardedOutcome::Unknown
    );
    assert!(matches!(
        expression(&context, &facts, "e!=0").context.enforcement,
        DefinitionEnforcement::Conditional | DefinitionEnforcement::Unsupported(_)
    ));
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location).contains("::mystery"))
    );
    let returned = obligation(&context, &facts, "6 div d");
    assert_eq!(
        returned.context.evaluation,
        GuardEvaluation::AssertionReturn
    );
    assert_eq!(returned.outcome, GuardedOutcome::Proven);
    assert_eq!(
        obligation(&context, &facts, "assert(false,\"failed\",2 div 0)").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        obligation(&context, &facts, "2 div 0").context.activation,
        GuardActivation::Inactive
    );
    for access in ["a[i]", "a[j]"] {
        let o = obligation(&context, &facts, access);
        assert_eq!(o.outcome, GuardedOutcome::Proven);
        assert_eq!(o.context.enforcement, DefinitionEnforcement::Conditional);
        assert!(o.context.assumptions.iter().any(|a| !a.global));
    }
    for comparison in ["a[i]=8 div i", "a[j]=7 div j"] {
        assert_eq!(
            expression(&context, &facts, comparison).context.enforcement,
            DefinitionEnforcement::Enforced
        );
    }
    for division in ["8 div i", "7 div j"] {
        assert_eq!(
            obligation(&context, &facts, division).outcome,
            GuardedOutcome::Proven
        );
    }
    assert_eq!(
        obligation(&context, &facts, "a[k]").context.activation,
        GuardActivation::Inactive
    );
    assert_eq!(
        obligation(&context, &facts, "5 div 0").outcome,
        GuardedOutcome::Refuted
    );
    let filtered_sum = expression(
        &context,
        &facts,
        "sum(i in 1..1 where false,j in S)(11 div 0)",
    );
    assert_eq!(filtered_sum.definedness, GuardedOutcome::Proven);
    assert_eq!(filtered_sum.numeric, Some(NumericOutcome::Exact(0)));
    let skipped_body = obligation(&context, &facts, "11 div 0");
    assert_eq!(skipped_body.outcome, GuardedOutcome::Refuted);
    assert_eq!(skipped_body.context.activation, GuardActivation::Inactive);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn aggregate_preconditions_and_missing_interpretation_remain_explicit() {
    let source = concat!(
        "array[1..0] of int: empty; array[1..2] of int: a; array[1..3] of int: b;\n",
        "var int: low=min(empty); var int: high=max(a); var int: zero=sum(empty); var int: one=product(empty);\n",
        "set of int: S={1,2}; array[S] of int: symbolic_array; var int: uncertain=min(symbolic_array);\n",
        "array[1..1] of opt int: optional_array; var opt int: optional_min=min(optional_array);\n",
        "int: empty_set_min=min({});\n",
        "var int: later_source=sum(i in 1..0,j in {1 div 0})(j);\n",
        "var int: generated_min=min(i in 1..0)(i); var int: generated_sum=sum(i in 1..0)(i);\n",
        "var bool: different=length(a)=length(b); function int: opaque(int: x)=x; int: value=opaque(1);\n",
        "var int: overflow=(-9223372036854775807-1) div -1;\n",
        "var int: remainder=(-9223372036854775807-1) mod -1;\n",
        "function int: min(int: x,int: y)=x; int: user_min=min(1,2); solve satisfy;\n",
    );
    let (dir, context, facts) = model("aggregates", source);
    assert_eq!(
        obligation(&context, &facts, "min(empty)").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        expression(&context, &facts, "min(empty)").definedness,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        obligation(&context, &facts, "max(a)").outcome,
        GuardedOutcome::Proven
    );
    assert_eq!(
        obligation(&context, &facts, "min(symbolic_array)").outcome,
        GuardedOutcome::Unknown
    );
    assert_eq!(
        obligation(&context, &facts, "min({})").outcome,
        GuardedOutcome::Refuted
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "min(optional_array)")
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "min(1,2)")
    );
    assert_eq!(
        expression(&context, &facts, "sum(empty)").numeric,
        Some(NumericOutcome::Exact(0))
    );
    assert_eq!(
        expression(&context, &facts, "product(empty)").numeric,
        Some(NumericOutcome::Exact(1))
    );
    assert_eq!(
        obligation(&context, &facts, "min(i in 1..0)(i)").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        expression(&context, &facts, "sum(i in 1..0,j in {1 div 0})(j)").definedness,
        GuardedOutcome::Proven
    );
    assert_eq!(
        obligation(&context, &facts, "1 div 0").context.activation,
        GuardActivation::Inactive
    );
    assert_eq!(
        expression(&context, &facts, "sum(i in 1..0)(i)").numeric,
        Some(NumericOutcome::Exact(0))
    );
    assert_eq!(
        expression(&context, &facts, "length(a)=length(b)").truth,
        Some(GuardedOutcome::Unknown)
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "opaque(1)")
    );
    for overflow in [
        "(-9223372036854775807-1) div -1",
        "(-9223372036854775807-1) mod -1",
    ] {
        assert_eq!(
            obligation(&context, &facts, overflow).outcome,
            GuardedOutcome::Proven
        );
        assert!(matches!(
            expression(&context, &facts, overflow).definedness,
            GuardedOutcome::Unsupported(_)
        ));
        assert!(
            facts
                .limitations
                .iter()
                .any(|l| text(&context, l.file, &l.location) == overflow)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context, facts) = model(
        "user-domain",
        concat!(
            "function int: '*'(int: x,int: y)=0; type Unsafe=1..(2*2);\n",
            "array[Unsafe] of int: unsafe_array; var Unsafe: i;\n",
            "var int: unsafe_access=unsafe_array[i]; var int: unsafe_min=min(unsafe_array); solve satisfy;\n",
        ),
    );
    assert!(matches!(
        obligation(&context, &facts, "unsafe_array[i]").outcome,
        GuardedOutcome::Unsupported(_)
    ));
    assert!(matches!(
        obligation(&context, &facts, "min(unsafe_array)").outcome,
        GuardedOutcome::Unsupported(_)
    ));
    assert!(facts.limitations.iter().any(|l| l.reason.contains("user")));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn optional_guards_default_and_aggregates_keep_presence_separate_from_totality() {
    let source = concat!(
        "var opt int:x; opt int:q=<>; var opt int:none=<>;\n",
        "var int:unsafe=deopt(x); var int:safe=if occurs(x) then deopt(x) else 0 endif;\n",
        "var int:negative=if not absent(x) then deopt(x) else 0 endif;\n",
        "var int:inactive=if occurs(none) then deopt(none) else 0 endif;\n",
        "int:a=(<> default 7); int:b=(1 div 0) default 9; int:c=<>+3; int:d=<>*4;\n",
        "var int:fallback=x default 0; bool:known=absent(<>); bool:present=occurs(3);\n",
        "array[int] of opt int:empty=[<> | i in 1..3]; array[1..2] of var opt int:unknown_values;\n",
        "var int:capacity_only=if length(unknown_values)>0 then min(unknown_values) else 0 endif;\n",
        "opt int:weak=<> ~+ 3;\n",
        "var int:low=min([<> | i in 1..3]); var int:zero=sum([<> | i in 1..3]);\n",
        "constraint forall([<> | i in 1..3]);\n",
        "constraint false -> occurs(x); constraint occurs(x) \\/ true;\n",
        "constraint assert(occurs(q),\"need presence\",deopt(q)>0);\n",
        "function opt int:opaque(opt int:v)=v; opt int:u=opaque(q);\n",
        "function bool:opaque_bool()=true; int:opaque_default=(if opaque_bool() then <> else <> endif) default 5;\n",
        "function bool:occurs(string:v)=true; bool:user=occurs(\"shadow\"); solve satisfy;\n",
    );
    let (dir, context, facts) = model_using_options("options", source, true);
    let unguarded = facts
        .obligations
        .iter()
        .filter(|o| text(&context, o.file, &o.operation) == "deopt(x)")
        .collect::<Vec<_>>();
    assert_eq!(unguarded.len(), 3);
    assert_eq!(unguarded[0].invariant, GuardedOutcome::Unknown);
    assert_eq!(unguarded[0].outcome, GuardedOutcome::Unknown);
    assert!(
        unguarded[1..]
            .iter()
            .all(|o| o.invariant == GuardedOutcome::Unknown && o.outcome == GuardedOutcome::Proven)
    );
    assert!(matches!(unguarded[0].kind, GuardObligationKind::Presence));
    let inactive = obligation(&context, &facts, "deopt(none)");
    assert_eq!(inactive.invariant, GuardedOutcome::Refuted);
    assert_eq!(inactive.outcome, GuardedOutcome::Refuted);
    assert_eq!(inactive.context.activation, GuardActivation::Inactive);
    assert_eq!(
        expression(
            &context,
            &facts,
            "if occurs(none) then deopt(none) else 0 endif"
        )
        .definedness,
        GuardedOutcome::Proven
    );
    for (code, n) in [
        ("(<> default 7)", 7),
        ("(1 div 0) default 9", 9),
        ("<>+3", 3),
        ("<>*4", 4),
    ] {
        let e = expression(&context, &facts, code);
        assert_eq!(e.definedness, GuardedOutcome::Proven, "{code}");
        assert_eq!(e.numeric, Some(NumericOutcome::Exact(n)), "{code}");
        assert_eq!(e.presence, Some(Presence::Present));
    }
    assert_eq!(
        obligation(&context, &facts, "1 div 0").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        expression(&context, &facts, "x default 0").definedness,
        GuardedOutcome::Proven
    );
    assert_eq!(
        expression(&context, &facts, "absent(<>)").truth,
        Some(GuardedOutcome::Proven)
    );
    assert_eq!(
        expression(&context, &facts, "occurs(3)").truth,
        Some(GuardedOutcome::Proven)
    );
    assert_eq!(
        obligation(&context, &facts, "min([<> | i in 1..3])").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        expression(&context, &facts, "sum([<> | i in 1..3])").numeric,
        Some(NumericOutcome::Exact(0))
    );
    assert_eq!(
        expression(&context, &facts, "forall([<> | i in 1..3])").truth,
        Some(GuardedOutcome::Proven)
    );
    assert_eq!(
        obligation(&context, &facts, "min(unknown_values)").outcome,
        GuardedOutcome::Unknown
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "<> ~+ 3")
    );
    let assertion = obligation(
        &context,
        &facts,
        "assert(occurs(q),\"need presence\",deopt(q)>0)",
    );
    assert_eq!(assertion.invariant, GuardedOutcome::Unknown);
    assert_eq!(assertion.outcome, GuardedOutcome::Unknown);
    assert_eq!(
        obligation(&context, &facts, "deopt(q)").outcome,
        GuardedOutcome::Proven
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "opaque(q)")
    );
    assert!(matches!(
        expression(
            &context,
            &facts,
            "(if opaque_bool() then <> else <> endif) default 5"
        )
        .definedness,
        GuardedOutcome::Unsupported(_)
    ));
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "occurs(\"shadow\")")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn boolean_complements_require_total_operands_and_keep_raw_partiality() {
    let source = concat!(
        "var bool:p; int:d; function bool:opaque();\n",
        "bool:no=p /\\ not p; bool:yes=p \\/ not p;\n",
        "bool:relational=(1 div d=0) /\\ not(1 div d=0);\n",
        "bool:abort=assert(false,\"abort\",p) /\\ not assert(false,\"abort\",p);\n",
        "bool:unknown=opaque() \\/ not opaque(); solve satisfy;\n",
    );
    let (dir, context, facts) = model("complements", source);
    assert_eq!(
        expression(&context, &facts, "p /\\ not p").truth,
        Some(GuardedOutcome::Refuted)
    );
    assert_eq!(
        expression(&context, &facts, "p \\/ not p").truth,
        Some(GuardedOutcome::Proven)
    );
    assert_eq!(
        expression(&context, &facts, "(1 div d=0) /\\ not(1 div d=0)").truth,
        Some(GuardedOutcome::Refuted)
    );
    assert!(
        facts
            .obligations
            .iter()
            .any(|o| matches!(o.kind, GuardObligationKind::Nonzero)
                && o.outcome == GuardedOutcome::Unknown)
    );
    assert_ne!(
        expression(
            &context,
            &facts,
            "assert(false,\"abort\",p) /\\ not assert(false,\"abort\",p)"
        )
        .definedness,
        GuardedOutcome::Proven
    );
    assert_ne!(
        expression(&context, &facts, "opaque() \\/ not opaque()").truth,
        Some(GuardedOutcome::Proven)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
