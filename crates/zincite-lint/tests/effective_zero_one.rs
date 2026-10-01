use std::path::PathBuf;
use zincite_lint::{
    BindingResolution, CallOutcome, DefinitionSafety, EffectiveZeroOneOutcome,
    IntegerBoundsOutcome, LintOptions, ModelOptions, RuleOutcome, analyze_model, expression_safety,
    load_model, resolve_bindings, resolve_callables, resolve_domains, resolve_effective_zero_one,
    resolve_instantiations, resolve_integer_bounds,
};
const CORE: &str = concat!(
    "function bool: '='($T: left,$T: right); function bool: '='(opt $T: left,opt $T: right);\n",
    "function var bool: '='(any $T: left,any $T: right);\n",
    "function var bool: '='(var int: left,float: right); function var bool: '='(float: left,var int: right);\n",
    "function var bool: '->'(var bool: left,var bool: right); function var bool: '<-'(var bool: left,var bool: right);\n",
    "function int: '+'(int: left,int: right); function int: '-'(int: left,int: right); function int: '-'(int: value);\n",
    "function int: '*'(int: left,int: right); function int: 'div'(int: left,int: right); function int: 'mod'(int: left,int: right);\n",
    "function var int: '+'(var int: left,var int: right); function var int: '-'(var int: left,var int: right); function var int: '-'(var int: value);\n",
    "function var int: '*'(var int: left,var int: right); function var int: 'div'(var int: left,var int: right); function var int: 'mod'(var int: left,var int: right);\n",
    "function var int: sum(array[int] of var int: values);\n"
);
fn model(name: &str, source: &str, included: &str) -> (PathBuf, zincite_lint::ModelContext) {
    let dir = std::env::temp_dir().join(format!("zincite-zero-one-{name}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("included.mzn"), included).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(dir.join("library")),
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (dir, context)
}
fn selected() -> LintOptions {
    LintOptions::from_selection("effective-zero-one").unwrap()
}
#[test]
fn invariant_bounds_both_polarities_and_whole_sums_have_independent_source_facts() {
    let source = concat!(
        "\u{feff}% é\r\ninclude \"included.mzn\"; type Bit=var 0..1; Bit: a; Bit: b; var 1..2: shifted;\n",
        "constraint a=1 -> b=1; constraint 0=a <- 0=b; constraint 1==shifted-1 -> b=2 div 2;\n",
        "constraint a=+1 → b=1; constraint -(-a)=1 -> b=1; constraint a*1=0 -> b=0;\n",
        "set of int: S; set of int: Alias=S; array[S] of Bit: flags; var int: total=sum(i in Alias)(flags[i]=1);\n",
        "array[Alias] of Bit: reversed_alias; var int: alias_total=sum(i in S)(reversed_alias[i]=1);\n",
        "array[{2,3}] of Bit: literal_flags; var int: literal_total=sum(i in (1+1)..3)(literal_flags[i]=1);\n",
        "constraint a=1 -> 1=b; solve satisfy;\n"
    );
    let included = "var 0..1: left; var 0..1: right;\n% zincite-lint: ignore effective-zero-one\nconstraint left=0 -> right=0;";
    let (dir, context) = model("positive", source, included);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let bounds = resolve_integer_bounds(&context, &bindings, &calls, &domains);
    let facts = resolve_effective_zero_one(&context, &bindings, &calls, &inst, &domains, &bounds);
    let forms: Vec<_> = facts
        .expressions
        .iter()
        .filter_map(|f| {
            if let EffectiveZeroOneOutcome::Eligible { formulation } = &f.outcome {
                Some(formulation.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        forms,
        [
            "a <= b",
            "b >= a",
            "shifted-1 <= b",
            "a <= b",
            "-(-a) <= b",
            "a*1 >= b",
            "sum(flags)",
            "sum(reversed_alias)",
            "sum(literal_flags)",
            "a <= b",
            "left >= right"
        ],
        "{:?}",
        facts.expressions
    );
    assert!(
        bounds
            .expressions
            .iter()
            .any(
                |f| &context.files[f.file].parsed.source()[f.location.range.start
                    - context.files[f.file].byte_offset
                    ..f.location.range.end - context.files[f.file].byte_offset]
                    == "shifted-1"
                    && f.outcome == IntegerBoundsOutcome::Known { lower: 0, upper: 1 }
            )
    );
    let result = analyze_model(&context, &selected());
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?}",
        result.limitations
    );
    assert_eq!(result.findings.len(), 10);
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(
        &source[result.findings[0].location.range.clone()],
        "a=1 -> b=1"
    );
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.location.path == dir.join("root.mzn"))
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn symbolic_numeric_defaults_partial_traversals_and_user_operations_do_not_prove_forms() {
    let source = concat!(
        "var 0..1: a; var 0..1: b; var 0..2: extra; int: one=1; int: offset=0; int: upper=1; var 0..upper: dependent;\n",
        "constraint extra=1 -> b=1; constraint a=1 -> b=0; constraint a=one -> b=1; constraint a+offset=1 -> b=1; constraint dependent=1 -> b=1;\n",
        "int: N=2; int: K=2; array[1..N] of var 0..1: flags;\n",
        "var int: different=sum(i in 1..K)(flags[i]=1); var int: filtered=sum(i in 1..N where i>1)(flags[i]=1);\n",
        "var int: partial=sum(i in 1..N)(flags[1]=1); var int: repeated=sum(i in 1..N,j in {})(flags[i]=1);\n",
        "var int: projection=sum(i in 1..N)(flags[i+0]=1);\n",
        "set of int: DefaultSet={1,2}; array[DefaultSet] of var 0..1: default_flags; var int: same_default=sum(i in 1..2)(default_flags[i]=1);\n",
        "int: divisor=1; array[1 div divisor..2] of var 0..1: divisor_flags; var int: default_division=sum(i in 1 div divisor..2)(divisor_flags[i]=1);\n",
        "var {}: empty; var 2..1: reversed; constraint empty=1 -> b=1; constraint reversed=0 -> b=0;\n",
        "solve satisfy;\n"
    );
    let (dir, context) = model("negative", source, "");
    let b = resolve_bindings(&context);
    let c = resolve_callables(&context, &b);
    let i = resolve_instantiations(&context, &b, &c);
    let d = resolve_domains(&context, &b);
    let n = resolve_integer_bounds(&context, &b, &c, &d);
    let f = resolve_effective_zero_one(&context, &b, &c, &i, &d, &n);
    assert!(
        !f.expressions
            .iter()
            .any(|f| matches!(f.outcome, EffectiveZeroOneOutcome::Eligible { .. })),
        "{:?}",
        f.expressions
    );
    assert!(
        f.expressions
            .iter()
            .any(|f| matches!(f.outcome, EffectiveZeroOneOutcome::Unknown(_)))
    );
    let r = analyze_model(&context, &selected());
    assert!(
        r.findings.is_empty() && r.errors.is_empty() && r.limitations.is_empty(),
        "{:?}",
        r.limitations
    );
    assert_eq!(r.rules[0].outcome, RuleOutcome::Completed);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn unsupported_safety_and_ambiguous_operators_remain_local_without_blanket_access_proof() {
    let source = concat!(
        "var 0..1: a; var 0..1: b; array[1..2] of var 0..1: flags;\n",
        "constraint flags[1]=1 -> b=1; constraint a=1 div 0 -> b=1; constraint a=(9223372036854775807+1) -> b=1;\n",
        "function int: opaque()=1; constraint a=opaque() -> b=1;\n",
        "var opt 0..1: option; constraint option=1 -> b=1;\n",
        "solve satisfy;\n"
    );
    let (dir, context) = model("unsupported", source, "");
    let b = resolve_bindings(&context);
    let c = resolve_callables(&context, &b);
    let r = analyze_model(
        &context,
        &LintOptions::from_selection("effective-zero-one,global-variable-in-function").unwrap(),
    );
    assert!(r.findings.is_empty() && r.errors.is_empty());
    assert!(!r.limitations.is_empty());
    assert!(matches!(r.rules[0].outcome, RuleOutcome::Limited { .. }));
    assert_eq!(r.rules[1].outcome, RuleOutcome::Completed);
    fn access(n: &zincite_syntax::SyntaxNode) -> Option<&zincite_syntax::SyntaxNode> {
        if n.kind() == zincite_syntax::NodeKind::ArrayAccessExpression {
            Some(n)
        } else {
            n.child_nodes().find_map(access)
        }
    }
    assert!(matches!(
        expression_safety(
            &context,
            &b,
            &c,
            0,
            access(context.files[0].parsed.tree()).unwrap()
        ),
        DefinitionSafety::Unsupported(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
    let overloaded = concat!(
        "var 0..1: a; var 0..1: b; array[1..2] of var 0..1: flags;\n",
        "function var bool: '='(var int: left,var int: right)=true; constraint a=1 -> b=1;\n",
        "function var bool: '<-'(var bool: left,var bool: right)=true; constraint a=0 <- b=0;\n",
        "function var int: sum(array[int] of var bool: values)=0; var int: total=sum(i in 1..2)(flags[i]=1); solve satisfy;\n"
    );
    let (dir, context) = model("overloads", overloaded, "");
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    assert!(
        bindings
            .references
            .iter()
            .any(|r| r.name == "=" && matches!(r.resolution, BindingResolution::Overloads(_)))
    );
    assert!(calls.calls.iter().any(|c| c.symbolic_operator
        && c.name == "<-"
        && matches!(c.outcome, CallOutcome::Ambiguous { .. })));
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty());
    assert_eq!(result.limitations.len(), 2);
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context) = model(
        "generator-operation",
        "function int: '+'(int: left,int: right)=2; array[1..2] of var 0..1: flags; var int: total=sum(i in (1+0)..2)(flags[i]=1); solve satisfy;",
        "",
    );
    let result = analyze_model(&context, &selected());
    assert!(result.findings.is_empty() && result.errors.is_empty());
    assert_eq!(result.limitations.len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
