use std::path::PathBuf;
use zincite_lint::{
    DefinitionEnforcement, GuardActivation, GuardEvaluation, GuardObligationKind,
    GuardedExpression, GuardedFacts, GuardedOutcome, ModelContext, ModelOptions, NumericOutcome,
    Presence, load_model, resolve_bindings, resolve_callables, resolve_definitions,
    resolve_domains, resolve_guarded_facts, resolve_guarded_facts_with_options,
    resolve_instantiations, resolve_numeric_facts, resolve_optional_facts,
};

const CORE: &str = concat!(
    "function string:show(bool:x); function string:show(var bool:x); function string:show(opt bool:x);\n",
    "function int: min(int:a,int:b); function int: max(int:a,int:b);\n",
    "function var int: min(var int:a,var int:b); function var int: max(var int:a,var int:b);\n",
    "function int: bool2int(bool:x); function var int: bool2int(var bool:x);\n",
    "function opt int: bool2int(opt bool:x); function array[$I] of int: bool2int(array[$I] of bool:x); function set of int: bool2int(set of bool:x);\n",
    "function bool: '='(int: a,int: b); function bool: '!='(int: a,int: b);\n",
    "function bool: '<'(int: a,int: b); function bool: '<='(int: a,int: b);\n",
    "function bool: '>'(int: a,int: b); function bool: '>='(int: a,int: b);\n",
    "function int: '-'(int: a,int: b);\n",
    "function set of int: '..'(int: left,int: right);\n",
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
    "function int: lb_array(array[int] of var int: a); function int: ub_array(array[int] of var int: a);\n",
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
    std::fs::write(dir.join("library/std/stdlib.mzn"), format!("{CORE}\nfunction array[int] of var int: '++'(array[int] of var int: left,array[int] of var int: right); function var bool: exists(array[int] of var bool: body); function set of int: index_set(array[int] of var int: a);\n")).unwrap();
    std::fs::write(
        dir.join("library/std/all_different.mzn"),
        "predicate alldifferent(array[$X] of var int: values,set of int: except={});\n",
    )
    .unwrap();
    let source = concat!(
        "include \"all_different.mzn\"; array[1..2] of var int: left; array[1..2] of var int: right;\n",
        "constraint exists(Y in left++right)(Y=1);\n",
        "constraint alldifferent([left[i]|i in index_set(left)]);\n",
        "constraint alldifferent([left[0]]); solve satisfy;\n"
    );
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let facts = resolve_guarded_facts(&context, &bindings, &calls, &inst, &domains, &numeric);
    let joined = expression(&context, &facts, "left++right");
    assert_eq!(joined.definedness, GuardedOutcome::Proven);
    assert_eq!(joined.numeric, None);
    let different = expression(
        &context,
        &facts,
        "alldifferent([left[i]|i in index_set(left)])",
    );
    assert!(!matches!(
        different.raw_definedness,
        GuardedOutcome::Unsupported(_)
    ));
    assert_eq!(different.truth, Some(GuardedOutcome::Unknown));
    let partial = expression(&context, &facts, "alldifferent([left[0]])");
    assert_eq!(partial.raw_definedness, GuardedOutcome::Refuted);
    assert_eq!(
        obligation(&context, &facts, "left[0]").outcome,
        GuardedOutcome::Refuted
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
    let (dir, context, facts) = model(
        "array-reflection",
        "var 0..9: x; var 0..9: y; array[int] of var int: values=[x,y]; array[1..0] of var int: empty; int: low=lb_array(values); int: high=ub_array(values); int: bad=lb_array(empty); function int: overridable(array[int] of var int: choice=[x])=lb_array(choice); int: overridden=overridable(empty); solve satisfy;",
    );
    for source in ["lb_array(values)", "ub_array(values)"] {
        assert_eq!(
            expression(&context, &facts, source).definedness,
            GuardedOutcome::Unknown
        );
        assert_eq!(
            obligation(&context, &facts, source).outcome,
            GuardedOutcome::Proven
        );
        assert_eq!(expression(&context, &facts, source).numeric, None);
    }
    assert_eq!(
        obligation(&context, &facts, "lb_array(empty)").outcome,
        GuardedOutcome::Refuted
    );
    assert_eq!(
        expression(&context, &facts, "lb_array(empty)").definedness,
        GuardedOutcome::Refuted
    );
    assert_ne!(
        obligation(&context, &facts, "lb_array(choice)").outcome,
        GuardedOutcome::Proven
    );
    std::fs::remove_dir_all(dir).unwrap();
    for options in [false, true] {
        let (dir, context, facts) = model_using_options(
            "scalar-extrema",
            "int:N; var int:x; int:a=min(N,2); var int:b=max(x,N); int:c=max(9 div 0,1); annotation unsafe; int:d=(max(N,3)::unsafe); solve satisfy;",
            options,
        );
        for source in ["min(N,2)", "max(x,N)"] {
            assert_eq!(
                expression(&context, &facts, source).definedness,
                GuardedOutcome::Proven
            );
            assert!(
                !facts
                    .obligations
                    .iter()
                    .any(|o| text(&context, o.file, &o.operation) == source
                        && matches!(o.kind, GuardObligationKind::Nonempty { .. }))
            );
        }
        assert_eq!(
            expression(&context, &facts, "max(9 div 0,1)").definedness,
            GuardedOutcome::Refuted
        );
        assert_eq!(
            obligation(&context, &facts, "9 div 0").outcome,
            GuardedOutcome::Refuted
        );
        assert!(matches!(
            expression(&context, &facts, "max(N,3)::unsafe").definedness,
            GuardedOutcome::Unsupported(_)
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }
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

#[test]
fn scalar_bool2int_retains_boolean_and_raw_partiality_boundaries() {
    for options in [false, true] {
        let (dir, context, facts) = model_using_options(
            if options { "show-options" } else { "show" },
            "bool:p; var bool:q; opt bool:o; function bool:opaque(); annotation unsafe; string:a=show(p); string:b=show(q); string:c=show(assert(false,\"abort\",true)); string:d=show(opaque()); string:e=show(o); string:f=(show(p)::unsafe); solve satisfy;",
            options,
        );
        for shown in ["show(p)", "show(q)"] {
            let value = expression(&context, &facts, shown);
            assert_eq!(value.definedness, GuardedOutcome::Proven);
            assert_eq!(value.truth, None);
            assert_eq!(value.numeric, None);
        }
        assert_ne!(
            expression(&context, &facts, "show(assert(false,\"abort\",true))").definedness,
            GuardedOutcome::Proven
        );
        for shown in ["show(opaque())", "show(o)", "show(p)::unsafe"] {
            assert!(matches!(
                expression(&context, &facts, shown).definedness,
                GuardedOutcome::Unsupported(_)
            ));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    let (dir, context, facts) = model(
        "user-show",
        "function string:show(bool:x)=\"yes\"; string:s=show(true); solve satisfy;",
    );
    assert!(matches!(
        expression(&context, &facts, "show(true)").definedness,
        GuardedOutcome::Unsupported(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let source = concat!(
        "bool:p; int:d; function bool:opaque();\n",
        "int:yes=bool2int(true); int:no=bool2int(false); int:unknown=bool2int(p);\n",
        "int:relational=bool2int(1 div 0=0); int:abort=bool2int(assert(false,\"abort\",true));\n",
        "int:opaque_value=bool2int(opaque()); opt bool:q; opt int:o=bool2int(q); array[int] of int:a=bool2int([true]); set of int:s=bool2int({true}); solve satisfy;\n",
    );
    let (dir, context, facts) = model("bool2int", source);
    assert_eq!(
        expression(&context, &facts, "bool2int(true)").numeric,
        Some(NumericOutcome::Exact(1))
    );
    assert_eq!(
        expression(&context, &facts, "bool2int(false)").numeric,
        Some(NumericOutcome::Exact(0))
    );
    assert_eq!(
        expression(&context, &facts, "bool2int(p)").numeric,
        Some(NumericOutcome::Interval { lower: 0, upper: 1 })
    );
    assert_eq!(
        expression(&context, &facts, "bool2int(1 div 0=0)").numeric,
        Some(NumericOutcome::Exact(0))
    );
    assert_eq!(
        expression(&context, &facts, "1 div 0=0").raw_definedness,
        GuardedOutcome::Refuted
    );
    assert!(
        facts
            .obligations
            .iter()
            .any(|o| matches!(o.kind, GuardObligationKind::Nonzero)
                && o.outcome == GuardedOutcome::Refuted)
    );
    assert_ne!(
        expression(&context, &facts, "bool2int(assert(false,\"abort\",true))").definedness,
        GuardedOutcome::Proven
    );
    assert!(matches!(
        expression(&context, &facts, "bool2int(opaque())").definedness,
        GuardedOutcome::Unsupported(_)
    ));
    for conversion in ["bool2int(q)", "bool2int([true])", "bool2int({true})"] {
        assert!(matches!(
            expression(&context, &facts, conversion).definedness,
            GuardedOutcome::Unsupported(_)
        ));
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context, facts) = model(
        "user-bool2int",
        "function int:bool2int(bool:x)=7; int:v=bool2int(true); solve satisfy;",
    );
    assert!(matches!(
        expression(&context, &facts, "bool2int(true)").definedness,
        GuardedOutcome::Unsupported(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

const BOOLEAN_CONCAT: &str = concat!(
    "annotation mzn_internal_representation;\n",
    "function array[int] of any $T: '++'(array[$$X] of any $T:x,array[$$Y] of any $T:y)",
    " :: mzn_internal_representation;\n",
);

fn boolean_concat_model(
    name: &str,
    source: &str,
    primitive: &str,
    options: bool,
) -> (PathBuf, ModelContext, GuardedFacts) {
    let dir =
        std::env::temp_dir().join(format!("zincite-bool-concat-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}{primitive}"),
    )
    .unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            include_dirs: Vec::new(),
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

#[test]
fn boolean_array_concatenation_inspects_scoped_sources_without_proving_values() {
    let source = concat!(
        "int:N; var bool:x;\n",
        "constraint forall(i in 1..N,j in 1..2 where j>0)(\n",
        "let {array[int] of var bool:b=[x | k in 1..i];} in forall(b ++ b));\n",
        "solve satisfy;\n",
    );
    for options in [false, true] {
        let (dir, context, facts) = boolean_concat_model(
            if options { "scoped-options" } else { "scoped" },
            source,
            BOOLEAN_CONCAT,
            options,
        );
        let value = expression(&context, &facts, "b ++ b");
        assert_eq!(
            value.raw_definedness,
            GuardedOutcome::Unknown,
            "BOOLEAN_CONCAT_SOURCE_RED"
        );
        assert_eq!(value.definedness, GuardedOutcome::Unknown);
        assert_eq!(value.truth, None);
        assert_eq!(value.numeric, None);
        let bindings = resolve_bindings(&context);
        let names: Vec<_> = value
            .context
            .assumptions
            .iter()
            .filter_map(|assumption| match &assumption.kind {
                zincite_lint::GuardAssumptionKind::GeneratorMembership { declaration, .. } => {
                    Some(bindings.declarations[declaration.0].name.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(names, ["i", "j"]);
        assert!(value.context.assumptions.iter().any(|assumption| {
            matches!(
                assumption.kind,
                zincite_lint::GuardAssumptionKind::Condition { expected: true }
            ) && text(&context, assumption.file, &assumption.location) == "j>0"
        }));
        let result = zincite_lint::analyze_model(
            &context,
            &zincite_lint::LintOptions::from_selection("vacuous-constraint").unwrap(),
        );
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert!(result.limitations.is_empty(), "{:?}", result.limitations);
        assert!(result.findings.is_empty(), "{:?}", result.findings);
        assert_eq!(
            result.rules[0].outcome,
            zincite_lint::RuleOutcome::Completed
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn boolean_array_concatenation_keeps_operand_and_written_primitive_refusals() {
    let source = "array[1..2] of var bool:a; constraint forall(a ++ a); solve satisfy;";
    for (name, source, primitive, concatenation) in [
        (
            "opaque",
            "array[1..2] of var bool:a; function array[int] of var bool:opaque(); constraint forall(opaque() ++ a); solve satisfy;",
            BOOLEAN_CONCAT.to_owned(),
            "opaque() ++ a",
        ),
        (
            "cycle",
            "array[int] of var bool:a=a; constraint forall(a ++ a); solve satisfy;",
            BOOLEAN_CONCAT.to_owned(),
            "a ++ a",
        ),
        (
            "optional",
            "array[1..2] of var opt bool:a; constraint forall(a ++ a); solve satisfy;",
            BOOLEAN_CONCAT.to_owned(),
            "a ++ a",
        ),
        (
            "type-source",
            "array[1..1 div 0] of var bool:a; constraint forall(a ++ a); solve satisfy;",
            BOOLEAN_CONCAT.to_owned(),
            "a ++ a",
        ),
        (
            "body",
            source,
            BOOLEAN_CONCAT.replace(" :: mzn_internal_representation;", "=x;"),
            "a ++ a",
        ),
        (
            "default",
            source,
            BOOLEAN_CONCAT.replace("any $T:x,", "any $T:x=[],"),
            "a ++ a",
        ),
        (
            "metadata",
            source,
            format!(
                "annotation unsupported_hint;\n{}",
                BOOLEAN_CONCAT.replace(" :: mzn_internal_representation;", ":: unsupported_hint;")
            ),
            "a ++ a",
        ),
    ] {
        let (dir, context, facts) = boolean_concat_model(name, source, &primitive, true);
        assert!(
            matches!(
                expression(&context, &facts, concatenation).definedness,
                GuardedOutcome::Unsupported(_)
            ),
            "{name}: {:?}",
            expression(&context, &facts, concatenation)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn optional_parameter_matrix_initializers_inspect_sources_without_proving_values() {
    use zincite_lint::{CallOutcome, DeclarationRole, Instantiation, SourceKind, TypeKind};
    let primitive = "function array[$$E,$$F] of any $V: array2d(set of $$E:S1,set of $$F:S2,array[$U] of any $V:x);\n";
    let changed_body = primitive.replace(
        " any $V:x);",
        " any $V:x) = [(i,j):x[1] | i in S1,j in S2];",
    );
    assert_ne!(changed_body, primitive);
    for (name, cell, standard, failure) in [
        ("symbolic", "value - 1", primitive, None),
        (
            "closed-cell",
            "(1 div 0)",
            primitive,
            Some("division by zero"),
        ),
        (
            "written-body",
            "value - 1",
            changed_body.as_str(),
            Some("body, default or type expression"),
        ),
    ] {
        let input = if name == "written-body" {
            format!("[1,<>,<>,{cell}]")
        } else {
            format!("[|1, <>|<>, {cell}|]")
        };
        let initializer = format!("array2d(States, Axis, {input})");
        let source = format!(
            "int:limit; int:value; enum Axis = {{A,B}};\n\
             constraint let {{set of int:States=1..limit;\n\
             array[States,Axis] of opt States:transitions={initializer};}} in true;\n\
             solve satisfy;\n"
        );
        let dir = std::env::temp_dir().join(format!(
            "zincite-optional-matrix-guarded-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        std::fs::write(
            dir.join("library/std/stdlib.mzn"),
            format!("{CORE}{standard}"),
        )
        .unwrap();
        std::fs::write(dir.join("root.mzn"), &source).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(context.includes.iter().all(|edge| edge.target.is_some()));
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let constructor = calls
            .calls
            .iter()
            .find(|call| context.files[call.file].path == context.root && call.name == "array2d")
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &constructor.outcome
        else {
            panic!("{name}: {:?}", constructor.outcome);
        };
        let owner = &bindings.declarations[declaration.0];
        assert_eq!(owner.role, DeclarationRole::Function);
        assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
        assert!(context.files[owner.file].implicit);
        assert_eq!(parameters.len(), 3);
        assert!(
            matches!(&parameters[2].kind, TypeKind::Array { indices, element }
            if indices.len() == 1 && element.optional && element.kind == TypeKind::Int)
        );
        assert!(
            matches!(&return_type.kind, TypeKind::Array { indices, element }
            if indices.len() == 2 && element.optional && element.kind == TypeKind::Int)
        );
        assert_eq!(return_type.instantiation, Instantiation::Parameter);
        assert!(!return_type.optional);
        let local = bindings
            .declarations
            .iter()
            .find(|owner| owner.role == DeclarationRole::Local && owner.name == "transitions")
            .unwrap()
            .id;
        assert_eq!(calls.declarations[local.0].ty, *return_type);
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        let facts = resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        );
        let value = expression(&context, &facts, &initializer);
        if let Some(reason) = failure {
            assert!(
                facts.limitations.iter().any(|limit| {
                    limit.file == value.file
                        && limit.location.range == value.location.range
                        && limit.reason.contains(reason)
                }),
                "{name}: {:?}",
                facts.limitations
            );
            if name == "closed-cell" {
                assert_eq!(value.raw_definedness, GuardedOutcome::Refuted);
                assert_eq!(value.definedness, GuardedOutcome::Refuted);
            } else {
                assert!(
                    matches!(&value.raw_definedness, GuardedOutcome::Unsupported(message)
                    if message.contains(reason)),
                    "{name}: {value:?}"
                );
                assert_eq!(value.definedness, value.raw_definedness);
            }
        } else {
            assert_eq!(
                value.raw_definedness,
                GuardedOutcome::Unknown,
                "OPTIONAL_MATRIX_SOURCE_RED {name}: {value:?}"
            );
            assert_eq!(value.definedness, GuardedOutcome::Unknown);
            assert!(
                !facts
                    .limitations
                    .iter()
                    .any(|limit| limit.file == value.file
                        && limit.location.range == value.location.range)
            );
        }
        assert_eq!(value.truth, None);
        assert_eq!(value.numeric, None);
        let callable = zincite_lint::resolve_callable_definitions(
            &context, &bindings, &calls, &inst, &domains,
        );
        assert!(callable.outputs.is_empty());
        assert!(!callable.inspected_locals.contains(&local));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn regular_four_relations_inspect_optional_sources_and_both_bodies_without_totality_proof() {
    use zincite_lint::{CallOutcome, DeclarationRole, Instantiation, SourceKind, TypeKind};
    let positive = concat!(
        "include \"regular.mzn\";\n",
        "enum Symbol = {A, B}; int: count; int: state_count;\n",
        "array[1..count] of var Symbol: schedule;\n",
        "constraint let {\n",
        "  set of int: States = 1..state_count; int: initial = 1;\n",
        "  array[States,Symbol] of opt States: transitions =\n",
        "    array2d(States,Symbol,[|1,<>|<>,2|]);\n",
        "} in regular(schedule,transitions,initial,States);\n",
        "solve satisfy;\n",
    );
    let regular = concat!(
        "include \"fzn_regular.mzn\"; include \"fzn_regular_set.mzn\";\n",
        "predicate regular(\n",
        "  array [$$X] of var $$Val: xs,\n",
        "  array [$$State, $$Val] of opt $$State: d,\n",
        "  $$State: q0, set of $$State: F,\n",
        ") = let {\n",
        "  any: State = enum2int(index_set_1of2(d));\n",
        "  any: Val = enum2int(index_set_2of2(d));\n",
        "  any: q_off = enum2int(min(State)) - 1;\n",
        "  any: dd = array2d(State,Val,\n",
        "    [if occurs(d_sv) then enum2int(deopt(d_sv))-q_off else 0 endif | d_sv in d],\n",
        "  );\n",
        "  any: qq0 = enum2int(q0)-q_off;\n",
        "  any: FF = {enum2int(i)-q_off | i in F};\n",
        "} in if min(Val)=1 then\n",
        "  fzn_regular(index2int(enum2int(xs)),card(State),max(Val),dd,qq0,FF)\n",
        "else fzn_regular_set(index2int(enum2int(xs)),card(State),Val,dd,qq0,FF) endif;\n",
    );
    let integer_body = concat!(
        "predicate fzn_regular(\n",
        " array[int] of var int: xs, int: Q, int: S,\n",
        " array[int,int] of int: d, int: q0, set of int: F,\n",
        ") = if length(xs)=0 then q0 in F else let {\n",
        " int: m=min(index_set(xs)); int: n=max(index_set(xs))+1;\n",
        " array[m..n] of var 1..Q: a;\n",
        "} in a[m]=q0 /\\ forall(i in index_set(xs),)(\n",
        " xs[i] in 1..S /\\ a[i+1]=d[a[i],xs[i]]\n",
        ") /\\ a[n] in F endif;\n",
    );
    let set_body = concat!(
        "predicate fzn_regular_set(\n",
        " array[int] of var int: xs, int: Q, set of int: S,\n",
        " array[int,int] of int: d, int: q0, set of int: F,\n",
        ") = let {\n",
        " int: m=min(index_set(xs)); int: n=max(index_set(xs))+1;\n",
        " array[m..n] of var 1..Q: a;\n",
        "} in a[m]=q0 /\\ forall(i in index_set(xs),)(\n",
        " xs[i] in S /\\ a[i+1]=d[a[i],xs[i]]\n",
        ") /\\ a[n] in F;\n",
    );
    // Reuse the portable regular recurrence fixture; each selected primitive
    // below retains its generic qualifiers and bodyless declaration.
    let core = concat!(
        "function var bool: '='(any $T:left,any $T:right); function bool: '='($T:left,$T:right);\n",
        "function int: '+'(int:left,int:right); function int: '-'(int:left,int:right);\n",
        "function int: 'div'(int:left,int:right); function set of int: '..'(int:left,int:right);\n",
        "function var bool: '/\\'(var bool:left,var bool:right);\n",
        "function bool: 'in'(int:value,set of int:choices); function var bool: 'in'(var int:value,set of int:choices);\n",
        "function var bool: forall(array[int] of var bool:body);\n",
        "function int: enum2int($$E:value); function set of int: enum2int(set of $$E:values);\n",
        "function array[int] of var int: enum2int(array[int] of var $$E:values);\n",
        "function array[int] of any $V: index2int(array[$$E] of any $V:values);\n",
        "function $$E: min(set of $$E:values); function $$E: max(set of $$E:values);\n",
        "function int: card(set of $$E:values); function int: length(array[$$X] of any $V:values);\n",
        "function set of int: index_set(array[int] of any $V:values);\n",
        "function set of $$E: index_set_1of2(array[$$E,$$F] of any $V:values);\n",
        "function set of $$F: index_set_2of2(array[$$E,$$F] of any $V:values);\n",
        "function array[$$E,$$F] of any $V: array2d(set of $$E:S1,set of $$F:S2,array[$U] of any $V:x);\n",
        "test occurs(opt $T:x); function $$T: deopt(opt $$T:x);\n",
    );
    let source_zero = positive.replace("<>,2|]", "<>,(1 div 0)|]");
    let body_zero = set_body.replace("max(index_set(xs))+1", "max(index_set(xs))+(1 div 0)");
    assert_ne!(source_zero, positive);
    assert_ne!(body_zero, set_body);
    for (name, source, second_body, failure) in [
        ("symbolic", positive, set_body, false),
        ("closed-actual", source_zero.as_str(), set_body, true),
        ("second-body", positive, body_zero.as_str(), true),
    ] {
        let dir = std::env::temp_dir().join(format!(
            "zincite-regular-four-guarded-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        for (file, contents) in [
            ("stdlib.mzn", core),
            ("regular.mzn", regular),
            ("fzn_regular.mzn", integer_body),
            ("fzn_regular_set.mzn", second_body),
        ] {
            std::fs::write(dir.join("library/std").join(file), contents).unwrap();
        }
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(context.includes.iter().all(|edge| edge.target.is_some()));
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let root_call = calls
            .calls
            .iter()
            .find(|call| context.files[call.file].path == context.root && call.name == "regular")
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &root_call.outcome
        else {
            panic!("{name}: {:?}", root_call.outcome);
        };
        let owner = &bindings.declarations[declaration.0];
        assert_eq!(owner.role, DeclarationRole::Predicate);
        assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
        assert_eq!(
            context.files[owner.file].path,
            dir.join("library/std/regular.mzn")
        );
        assert!(!context.files[owner.file].implicit);
        assert_eq!(parameters.len(), 4);
        let symbol = bindings
            .declarations
            .iter()
            .find(|owner| owner.top_level && owner.name == "Symbol")
            .unwrap()
            .id;
        assert!(
            matches!(&parameters[0].kind, TypeKind::Array { indices, element }
            if indices.len() == 1 && element.kind == TypeKind::Enum(symbol))
        );
        assert!(
            matches!(&parameters[1].kind, TypeKind::Array { indices, element }
            if indices.len() == 2 && indices[0].kind == TypeKind::Int
                && indices[1].kind == TypeKind::Enum(symbol) && element.optional
                && element.kind == TypeKind::Int && element.instantiation == Instantiation::Parameter)
        );
        assert_eq!(return_type.kind, TypeKind::Bool);
        assert_eq!(return_type.instantiation, Instantiation::Decision);
        assert!(!return_type.optional);
        let local = bindings
            .declarations
            .iter()
            .find(|owner| owner.role == DeclarationRole::Local && owner.name == "transitions")
            .unwrap()
            .id;
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        let facts = resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        );
        let value = expression(
            &context,
            &facts,
            "regular(schedule,transitions,initial,States)",
        );
        if failure {
            assert!(
                matches!(
                    &value.raw_definedness,
                    GuardedOutcome::Unsupported(_) | GuardedOutcome::Refuted
                ),
                "{name}: {value:?}"
            );
            assert_eq!(value.definedness, value.raw_definedness);
            assert!(
                facts.limitations.iter().any(|limit| {
                    limit.file == value.file
                        && limit.location.range == value.location.range
                        && limit.reason.contains("integer division by zero")
                }),
                "{name}: {:?}",
                facts.limitations
            );
        } else {
            assert_eq!(
                value.raw_definedness,
                GuardedOutcome::Unknown,
                "REGULAR_FOUR_SOURCE_RED {name}: {value:?}"
            );
            assert_eq!(value.definedness, GuardedOutcome::Unknown);
            assert_eq!(value.truth, Some(GuardedOutcome::Unknown));
            assert!(!facts.limitations.iter().any(|limit| {
                limit.file == value.file && limit.location.range == value.location.range
            }));
        }
        assert_eq!(value.numeric, None);
        let callable = zincite_lint::resolve_callable_definitions(
            &context, &bindings, &calls, &inst, &domains,
        );
        assert!(callable.outputs.is_empty());
        assert!(!callable.inspected_locals.contains(&local));
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn literal_regular_relations_inspect_sources_without_totality_proof() {
    use zincite_lint::{
        CallOutcome, DeclarationRole, Instantiation, SearchCoverage, SourceKind, TypeKind,
    };
    let positive = concat!(
        "include \"regular_regexp.mzn\";\n",
        "enum Day = {First, Last}; enum Shift = {Work, Rest};\n",
        "int: workers; set of int: Workers = 1..workers;\n",
        "array[Day,Workers] of var Shift: schedule;\n",
        "constraint forall(worker in Workers)(regular(schedule[..,worker], \".* Rest Rest .*\"));\n",
        "solve satisfy;\n",
    );
    let regular = concat!(
        "include \"fzn_regular_regexp.mzn\";\n",
        "predicate regular(array[int] of var $$E: xs,string: regexp) = fzn_regular(enum2int(xs),regexp);\n",
    );
    let native = "predicate fzn_regular(array[int] of var int: xs,string: regexp);\n";
    let core = CORE.replace("array[int] of var opt bool: a", "array[int] of var bool: a");
    let core =
        format!("{core}\nfunction array[$X] of var int: enum2int(array[$X] of var $$E: x);\n");
    let source_zero = positive.replace("schedule[..,worker]", "schedule[..,(worker div 0)]");
    let written_native = native.replace(");", ") = true;");
    assert_ne!(source_zero, positive);
    assert_ne!(written_native, native);
    for (name, source, native, failure) in [
        ("symbolic", positive, native, None),
        (
            "closed-selector",
            source_zero.as_str(),
            native,
            Some("division by zero"),
        ),
        (
            "written-native",
            positive,
            written_native.as_str(),
            Some("native declaration"),
        ),
    ] {
        let dir = std::env::temp_dir().join(format!(
            "zincite-literal-regular-guarded-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        for (file, contents) in [
            ("stdlib.mzn", core.as_str()),
            ("regular_regexp.mzn", regular),
            ("fzn_regular_regexp.mzn", native),
        ] {
            std::fs::write(dir.join("library/std").join(file), contents).unwrap();
        }
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(context.includes.iter().all(|edge| edge.target.is_some()));
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let file = context.root_file.unwrap();
        assert!(
            calls
                .calls
                .iter()
                .filter(|call| call.file == file)
                .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
            "{name}: {:?}",
            calls.calls
        );
        let root_call = calls
            .calls
            .iter()
            .find(|call| call.file == file && call.name == "regular")
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &root_call.outcome
        else {
            unreachable!()
        };
        let owner = &bindings.declarations[declaration.0];
        assert_eq!(owner.role, DeclarationRole::Predicate);
        assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
        assert!(!context.files[owner.file].implicit);
        let symbol = bindings
            .declarations
            .iter()
            .find(|owner| owner.top_level && owner.name == "Shift")
            .unwrap()
            .id;
        assert!(matches!(parameters.as_slice(), [array, pattern]
            if !array.optional && array.instantiation == Instantiation::Decision
                && matches!(&array.kind, TypeKind::Array { indices, element }
                    if indices.len() == 1 && indices[0].kind == TypeKind::Int
                        && element.kind == TypeKind::Enum(symbol) && element.instantiation == Instantiation::Decision && !element.optional)
                && pattern.kind == TypeKind::String && !pattern.optional && pattern.instantiation == Instantiation::Parameter));
        assert_eq!(return_type.kind, TypeKind::Bool);
        assert_eq!(return_type.instantiation, Instantiation::Decision);
        assert!(!return_type.optional);
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        let facts = resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        );
        let call_start = source.find("regular(").unwrap();
        let value = facts
            .expressions
            .iter()
            .find(|value| {
                value.file == file
                    && value.location.range.start == call_start
                    && text(&context, file, &value.location).starts_with("regular(")
            })
            .unwrap();
        if let Some(reason) = failure {
            assert!(
                matches!(
                    &value.raw_definedness,
                    GuardedOutcome::Unsupported(_) | GuardedOutcome::Refuted
                ),
                "{name}: {value:?}"
            );
            assert_eq!(value.definedness, value.raw_definedness);
            assert!(
                facts.limitations.iter().any(|limit| limit.file == file
                    && limit.location.range == value.location.range
                    && limit.reason.contains(reason)),
                "{name}: {:?}",
                facts.limitations
            );
        } else {
            assert_eq!(
                value.raw_definedness,
                GuardedOutcome::Unknown,
                "REGULAR_TWO_SOURCE_RED {name}: {value:?}"
            );
            assert_eq!(value.definedness, GuardedOutcome::Unknown);
            assert_eq!(value.truth, Some(GuardedOutcome::Unknown));
            assert!(!facts.limitations.iter().any(|limit| limit.file == file && limit.location.range == value.location.range));
        }
        assert_eq!(value.numeric, None);
        let callable = zincite_lint::resolve_callable_definitions(
            &context, &bindings, &calls, &inst, &domains,
        );
        assert!(callable.outputs.is_empty());
        assert!(callable.inspected_locals.is_empty());
        let search = zincite_lint::resolve_search_coverage(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &definitions,
        );
        let schedule = bindings
            .declarations
            .iter()
            .find(|owner| owner.top_level && owner.name == "schedule")
            .unwrap()
            .id;
        let coverage = search
            .declarations
            .iter()
            .find(|value| value.declaration == schedule)
            .unwrap()
            .coverage;
        if failure.is_none() {
            assert_eq!(coverage, SearchCoverage::Uncovered, "{name}: {search:?}");
        } else {
            assert!(
                matches!(
                    coverage,
                    SearchCoverage::Unknown | SearchCoverage::Uncovered
                ),
                "{name}: {search:?}"
            );
        }
        assert!(search.searched.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn sliding_sum_relations_inspect_sources_without_proving_windows_or_totality() {
    use zincite_lint::{CallOutcome, DeclarationRole, Instantiation, SourceKind, TypeKind};
    let core = concat!(
        "annotation mzn_internal_representation;\n",
        "function var bool: '='(any $T:left,any $T:right);\n",
        "function int: '+'(int:left,int:right); function var int: '+'(var int:left,var int:right);\n",
        "function int: '-'(int:left,int:right); function var int: '-'(var int:left,var int:right);\n",
        "function int: 'div'(int:left,int:right); function set of int: '..'(int:left,int:right);\n",
        "function var bool: '<='(var int:left,var int:right); function var bool: '/\\'(var bool:left,var bool:right);\n",
        "function var bool: forall(array[int] of var bool:body);\n",
        "function $$E: min(set of $$E:values); function $$E: max(set of $$E:values);\n",
        "function set of $$E: index_set(array[$$E] of any $V:values);\n",
        "function array[int] of any $V: index2int(array[$$E] of any $V:values) :: mzn_internal_representation;\n",
        "function array[int] of any $T: '++'(array[$$X] of any $T:x,array[$$Y] of any $T:y) :: mzn_internal_representation;\n",
    );
    let wrapper = concat!(
        "include \"fzn_sliding_sum.mzn\";\n",
        "predicate sliding_sum(int:low,int:up,int:window_size,array[$$E] of var int:xs)=\n",
        " fzn_sliding_sum(low,up,window_size,index2int(xs));\n",
    );
    let body = concat!(
        "predicate fzn_sliding_sum(int:low,int:up,int:window_size,array[int] of var int:xs)=\n",
        " let { int:lx=min(index_set(xs)); int:ux=max(index_set(xs));\n",
        " array[lx-1..ux] of var int:S; } in\n",
        " S[lx-1]=0 /\\ forall(i in lx..ux)(S[i]=xs[i]+S[i-1]) /\\\n",
        " forall(i in lx-1..ux-window_size)(S[i]<=S[i+window_size]-low /\\ S[i+window_size]<=S[i]+up);\n",
    );
    let plain = concat!(
        "include \"sliding_sum.mzn\"; int:N; var bool:x;\n",
        "constraint forall(k in 1..N)(let { array[int] of var bool:b=[x | j in 1..k]; }\n",
        " in sliding_sum(1,6,6,b)); solve satisfy;\n",
    );
    let concatenated = plain.replace("sliding_sum(1,6,6,b)", "sliding_sum(1,6,6,b ++ b)");
    let source_zero = plain.replace("j in 1..k", "j in 1..(k+(1 div 0))");
    let body_zero = body.replace("max(index_set(xs))", "max(index_set(xs))+(1 div 0)");
    let written_native = core.replace(
        "index2int(array[$$E] of any $V:values) :: mzn_internal_representation;",
        "index2int(array[$$E] of any $V:values)=values;",
    );
    let empty = "include \"sliding_sum.mzn\"; array[1..0] of var bool:b; constraint sliding_sum(1,6,6,b); solve satisfy;";
    assert_ne!(concatenated, plain);
    assert_ne!(source_zero, plain);
    assert_ne!(body_zero, body);
    assert_ne!(written_native, core);
    for (name, source, selected_body, primitives, failure) in [
        ("plain", plain, body, core, None),
        ("concat", concatenated.as_str(), body, core, None),
        (
            "source-zero",
            source_zero.as_str(),
            body,
            core,
            Some("integer division by zero"),
        ),
        (
            "body-zero",
            plain,
            body_zero.as_str(),
            core,
            Some("integer division by zero"),
        ),
        (
            "known-empty",
            empty,
            body,
            core,
            Some("known empty actual array"),
        ),
        (
            "written-native",
            plain,
            body,
            written_native.as_str(),
            Some("written primitive"),
        ),
    ] {
        let dir = std::env::temp_dir().join(format!(
            "zincite-sliding-source-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        for (file, source) in [
            ("stdlib.mzn", primitives),
            ("sliding_sum.mzn", wrapper),
            ("fzn_sliding_sum.mzn", selected_body),
        ] {
            std::fs::write(dir.join("library/std").join(file), source).unwrap();
        }
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(context.includes.iter().all(|edge| edge.target.is_some()));
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let call = calls
            .calls
            .iter()
            .find(|call| {
                context.files[call.file].path == context.root && call.name == "sliding_sum"
            })
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &call.outcome
        else {
            panic!("{name}: {:?}", call.outcome);
        };
        let owner = &bindings.declarations[declaration.0];
        assert_eq!(owner.role, DeclarationRole::Predicate);
        assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
        assert_eq!(
            context.files[owner.file].path,
            dir.join("library/std/sliding_sum.mzn")
        );
        assert!(!context.files[owner.file].implicit);
        assert_eq!(parameters.len(), 4);
        assert!(
            matches!(&parameters[3].kind, TypeKind::Array { indices, element }
            if parameters[3].instantiation == Instantiation::Decision && !parameters[3].optional
                && indices.len() == 1 && indices[0].kind == TypeKind::Int
                && element.kind == TypeKind::Int && element.instantiation == Instantiation::Decision && !element.optional)
        );
        assert_eq!(return_type.kind, TypeKind::Bool);
        assert_eq!(return_type.instantiation, Instantiation::Decision);
        assert!(!return_type.optional);
        let source_array = bindings
            .declarations
            .iter()
            .find(|declaration| {
                declaration.name == "b" && context.files[declaration.file].path == context.root
            })
            .unwrap()
            .id;
        assert!(
            matches!(&calls.declarations[source_array.0].ty.kind, TypeKind::Array { element, .. } if element.kind == TypeKind::Bool)
        );
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        let facts = resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        );
        let selected = if name == "concat" {
            "sliding_sum(1,6,6,b ++ b)"
        } else {
            "sliding_sum(1,6,6,b)"
        };
        let value = expression(&context, &facts, selected);
        if let Some(reason) = failure {
            assert!(
                matches!(
                    &value.raw_definedness,
                    GuardedOutcome::Unsupported(_) | GuardedOutcome::Refuted
                ),
                "{name}: {value:?}"
            );
            assert!(
                facts
                    .limitations
                    .iter()
                    .any(|limit| limit.file == value.file
                        && limit.location.range == value.location.range
                        && limit.reason.contains(reason)),
                "{name}: {:?}",
                facts.limitations
            );
        } else {
            assert_eq!(
                value.raw_definedness,
                GuardedOutcome::Unknown,
                "SLIDING_SUM_SOURCE_RED {name}: {value:?}"
            );
            assert_eq!(
                value.definedness,
                GuardedOutcome::Unknown,
                "{name}: {value:?}"
            );
            assert_eq!(
                value.truth,
                Some(GuardedOutcome::Unknown),
                "{name}: {value:?}"
            );
            assert!(
                !facts
                    .limitations
                    .iter()
                    .any(|limit| limit.file == value.file
                        && limit.location.range == value.location.range),
                "{name}: {:?}",
                facts.limitations
            );
        }
        assert_eq!(value.numeric, None, "{name}: {value:?}");
        let callable = zincite_lint::resolve_callable_definitions(
            &context, &bindings, &calls, &inst, &domains,
        );
        assert!(
            !callable
                .outputs
                .iter()
                .any(|output| output.target == source_array),
            "{name}: source array gained a callable output"
        );
        assert!(
            !callable
                .definitions
                .iter()
                .any(|definition| definition.target == source_array),
            "{name}: source array gained a callable definition"
        );
        let result = zincite_lint::analyze_model(
            &context,
            &zincite_lint::LintOptions::from_selection("vacuous-constraint").unwrap(),
        );
        assert!(result.errors.is_empty(), "{name}: {:?}", result.errors);
        if failure.is_some() {
            assert!(
                matches!(
                    result.rules[0].outcome,
                    zincite_lint::RuleOutcome::Limited { .. }
                ),
                "{name}: {:?}",
                result.rules
            );
        } else {
            assert_eq!(
                result.rules[0].outcome,
                zincite_lint::RuleOutcome::Completed,
                "{name}: {:?}",
                result.limitations
            );
            assert!(
                result.limitations.is_empty(),
                "{name}: {:?}",
                result.limitations
            );
            assert!(result.findings.is_empty(), "{name}: {:?}", result.findings);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[test]
fn bound_gcc_relations_inspect_count_bodies_without_totality_proof() {
    use zincite_lint::{CallOutcome, DeclarationRole, Instantiation, SourceKind, TypeKind};
    let positive = concat!(
        "include \"global_cardinality.mzn\";\n",
        "enum Day = {First, Last}; enum Shift = {Work, Rest};\n",
        "int: workers; set of int: Workers = 1..workers;\n",
        "array[Day,Workers] of var Shift: schedule; array[Shift] of int: requirements;\n",
        "constraint forall(worker in Workers)(global_cardinality(schedule[..,worker],Shift,requirements,requirements));\n",
        "solve satisfy;\n",
    );
    let gcc = r#"include "fzn_global_cardinality_low_up.mzn";
predicate global_cardinality(
  array [$X] of var $$E: xs,
  array [$Y] of $$E: cover,
  array [$Y] of int: lower_bound,
  array [$Y] of int: upper_bound,
) =
  assert(
    index_sets_agree(cover, lower_bound) /\ index_sets_agree(cover, upper_bound),
    "global_cardinality: " ++
      "cover has index sets " ++
      show_index_sets(cover) ++
      ", lower_bound has index sets " ++
      show_index_sets(lower_bound) ++
      ", and upper_bound has index sets " ++
      show_index_sets(lower_bound) ++
      ", but they must have identical index sets",
    if length(xs) == 0 then
      assert(
        forall (l in array1d(lower_bound)) (l <= 0) /\ forall (u in array1d(upper_bound)) (u >= 0) \/
          length(cover) == 0,
        "global_cardinality_low_up: " ++
          "lower_bound and upper_bound must allow a count of 0 when xs is empty, or also be empty",
        true,
      )
    else
      fzn_global_cardinality_low_up(
        enum2int(array1d(xs)),
        enum2int(array1d(cover)),
        array1d(lower_bound),
        array1d(upper_bound),
      )
    endif,
  );
"#;
    let native = r#"predicate fzn_global_cardinality_low_up(
  array [int] of var int: xs,
  array [int] of int: cover,
  array [int] of int: lower_bound,
  array [int] of int: upper_bound,
) =
  forall (i in index_set(cover)) (
    if upper_bound[i] >= length(xs) then
      count (xi in xs) (xi = cover[i]) >= lower_bound[i]
    elseif lower_bound[i] <= 0 then
      count (xi in xs) (xi = cover[i]) <= upper_bound[i]
    else
      count (xi in xs) (xi = cover[i]) in lower_bound[i]..upper_bound[i]
    endif
  );
"#;
    let count = r#"function var int: count(array [$T] of var bool: xs :: promise_ctx_monotone) :: promise_commutative =
  let {
    array [int] of var bool: xx :: promise_ctx_monotone = array1d(xs);
  } in sum([bool2int(y) | y in xx]);
"#;
    let core = CORE
        .replace("array[int] of var opt bool: a", "array[int] of var bool: a")
        .replace(
            "function int: length(array[$I] of $T: a);",
            "function int: length(array[$I] of any $T: a);",
        )
        .replace(
            "function $T: assert(bool: condition,string: message,$T: value);",
            "function any $T: assert(bool: condition,string: message,any $T: value);",
        );
    let core = format!(
        "{core}\n{}",
        concat!(
            "annotation promise_ctx_monotone; annotation promise_commutative;\n",
            "function bool: '/\\'(bool:a,bool:b); function bool: '\\/'(bool:a,bool:b);\n",
            "function bool: forall(array[int] of bool:a);\n",
            "function string: '++'(string:a,string:b);\n",
            "test index_sets_agree(array[$T] of any $U:x,array[$T] of any $W:y);\n",
            "function string: show_index_sets(array[$T] of any $U:x);\n",
            "function array[int] of any $V: array1d(array[$U] of any $V:x);\n",
            "function array[$X] of int: enum2int(array[$X] of $$E:x);\n",
            "function array[$X] of var int: enum2int(array[$X] of var $$E:x);\n",
        )
    );
    let integer_positive = positive
        .replace("enum Day =", "enum Days =")
        .replace("array[Day,Workers]", "array[Days,Workers]")
        .replace("array[Shift] of int: requirements", "array[Days,Shift] of int: requirements")
        .replace(
            "forall(worker in Workers)(global_cardinality(schedule[..,worker],Shift,requirements,requirements))",
            "forall(day in Days)(global_cardinality(schedule[day,..],Shift,requirements[day,..],requirements[day,..]))",
        );
    assert_ne!(integer_positive, positive);
    let closed = positive.replace("schedule[..,worker]", "schedule[..,(worker div 0)]");
    let changed_count = count.replace(
        "sum([bool2int(y) | y in xx])",
        "sum([bool2int(y) | y in xx]) + 1",
    );
    assert_ne!(closed, positive);
    assert_ne!(changed_count, count);
    for (name, source, count, failure, enum_axis) in [
        ("enum-axis", positive, count, None, true),
        (
            "integer-axis",
            integer_positive.as_str(),
            count,
            None,
            false,
        ),
        (
            "closed-selector",
            closed.as_str(),
            count,
            Some("division by zero"),
            true,
        ),
        (
            "changed-count",
            positive,
            changed_count.as_str(),
            Some("complete written body"),
            true,
        ),
    ] {
        let dir = std::env::temp_dir().join(format!(
            "zincite-bound-gcc-guarded-{name}-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(dir.join("library/std")).unwrap();
        let core = format!("{core}\n{count}");
        for (file, source) in [
            ("stdlib.mzn", core.as_str()),
            ("global_cardinality.mzn", gcc),
            ("fzn_global_cardinality_low_up.mzn", native),
        ] {
            std::fs::write(dir.join("library/std").join(file), source).unwrap();
        }
        std::fs::write(dir.join("root.mzn"), source).unwrap();
        let context = load_model(
            dir.join("root.mzn"),
            &ModelOptions {
                stdlib_dir: Some(dir.join("library")),
                ..Default::default()
            },
        );
        assert!(context.errors.is_empty(), "{name}: {:?}", context.errors);
        assert!(
            context.limitations.is_empty(),
            "{name}: {:?}",
            context.limitations
        );
        assert!(context.includes.iter().all(|edge| edge.target.is_some()));
        assert!(
            context
                .files
                .iter()
                .all(|file| file.parsed.diagnostics().is_empty())
        );
        let bindings = resolve_bindings(&context);
        let calls = resolve_callables(&context, &bindings);
        let file = context.root_file.unwrap();
        assert!(
            calls
                .calls
                .iter()
                .filter(|call| call.file == file)
                .all(|call| matches!(call.outcome, CallOutcome::Resolved { .. })),
            "{name}: {:?}",
            calls.calls
        );
        let root = calls
            .calls
            .iter()
            .find(|call| call.file == file && call.name == "global_cardinality")
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            parameters,
            return_type,
        } = &root.outcome
        else {
            unreachable!()
        };
        let owner = &bindings.declarations[declaration.0];
        assert_eq!(owner.role, DeclarationRole::Predicate);
        assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
        assert!(!context.files[owner.file].implicit);
        let symbol = bindings
            .declarations
            .iter()
            .find(|owner| owner.top_level && owner.name == "Shift")
            .unwrap()
            .id;
        let day = bindings
            .declarations
            .iter()
            .find(|owner| owner.top_level && matches!(owner.name.as_str(), "Day" | "Days"))
            .unwrap()
            .id;
        let axis = if enum_axis {
            TypeKind::Enum(day)
        } else {
            TypeKind::Int
        };
        assert!(
            matches!(parameters.as_slice(), [xs, cover, lower, upper]
            if !xs.optional && xs.instantiation == Instantiation::Decision
                && matches!(&xs.kind, TypeKind::Array { indices, element } if indices.len() == 1 && indices[0].kind == axis
                    && indices[0].instantiation == Instantiation::Parameter && !indices[0].optional
                    && element.kind == TypeKind::Enum(symbol) && element.instantiation == Instantiation::Decision && !element.optional)
                && !cover.optional && cover.instantiation == Instantiation::Parameter
                && matches!(&cover.kind, TypeKind::Array { indices, element } if indices.len() == 1 && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Enum(symbol) && element.instantiation == Instantiation::Parameter && !element.optional)
                && lower == upper && !lower.optional && lower.instantiation == Instantiation::Parameter
                && matches!(&lower.kind, TypeKind::Array { indices, element } if indices.len() == 1 && indices[0].kind == TypeKind::Int && element.kind == TypeKind::Int && element.instantiation == Instantiation::Parameter && !element.optional)),
            "GCC_SELECTED_TUPLE {name}: {parameters:?}; root={root:?}; symbol={symbol:?}"
        );
        assert_eq!(return_type.kind, TypeKind::Bool);
        assert_eq!(return_type.instantiation, Instantiation::Decision);
        assert!(!return_type.optional);
        let inst = resolve_instantiations(&context, &bindings, &calls);
        let domains = resolve_domains(&context, &bindings);
        let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
        let numeric =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let optional = resolve_optional_facts(
            &context,
            &bindings,
            &calls,
            &inst,
            &domains,
            &numeric,
            &definitions,
        );
        let facts = resolve_guarded_facts_with_options(
            &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
        );
        let start = source.find("global_cardinality(").unwrap();
        let value = facts
            .expressions
            .iter()
            .find(|value| {
                value.file == file
                    && value.location.range.start == start
                    && text(&context, file, &value.location).starts_with("global_cardinality(")
            })
            .unwrap();
        if let Some(reason) = failure {
            assert!(
                matches!(
                    value.raw_definedness,
                    GuardedOutcome::Unsupported(_) | GuardedOutcome::Refuted
                ),
                "{name}: {value:?}"
            );
            assert_eq!(value.definedness, value.raw_definedness);
            assert!(
                facts.limitations.iter().any(|limit| limit.file == file
                    && limit.location.range == value.location.range
                    && limit.reason.contains(reason)),
                "{name}: {:?}",
                facts.limitations
            );
        } else {
            assert_eq!(
                value.raw_definedness,
                GuardedOutcome::Unknown,
                "GCC_SOURCE_RED {name}: {value:?}"
            );
            assert_eq!(value.definedness, GuardedOutcome::Unknown);
            assert_eq!(value.truth, Some(GuardedOutcome::Unknown));
            assert!(!facts.limitations.iter().any(|limit| limit.file == file && limit.location.range == value.location.range));
        }
        assert_eq!(value.numeric, None);
        let callable = zincite_lint::resolve_callable_definitions(
            &context, &bindings, &calls, &inst, &domains,
        );
        assert!(callable.outputs.is_empty());
        assert!(callable.inspected_locals.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
