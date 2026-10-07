use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, DeclarationId, Instantiation, IntegerBoundsOutcome, ModelContext, ModelOptions,
    NumericBound, NumericDeclaration, NumericDomainRelation, NumericFacts, NumericOutcome,
    load_model, resolve_bindings, resolve_callables, resolve_definitions, resolve_domains,
    resolve_instantiations, resolve_integer_bounds, resolve_numeric_facts,
};
const CORE: &str = concat!(
    "function var bool: '='(var int: left,var int: right);\n",
    "function var bool: '/\\'(var bool: left,var bool: right); function var bool: '->'(var bool: left,var bool: right);\n",
    "function int: '+'(int: left,int: right); function int: '-'(int: left,int: right); function int: '-'(int: value);\n",
    "function int: '*'(int: left,int: right); function int: 'div'(int: left,int: right); function int: 'mod'(int: left,int: right);\n",
    "function var int: '+'(var int: left,var int: right); function var int: '-'(var int: left,var int: right); function var int: '-'(var int: value);\n",
    "function var int: '*'(var int: left,var int: right); function var int: 'div'(var int: left,var int: right); function var int: 'mod'(var int: left,var int: right);\n",
);
fn model(name: &str, source: &str, included: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!("zincite-numeric-{name}-{}", std::process::id()));
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
fn facts(context: &ModelContext) -> (BindingFacts, NumericFacts) {
    let bindings = resolve_bindings(context);
    let calls = resolve_callables(context, &bindings);
    let inst = resolve_instantiations(context, &bindings, &calls);
    let domains = resolve_domains(context, &bindings);
    let definitions = resolve_definitions(context, &bindings, &calls, &inst, &domains);
    let facts = resolve_numeric_facts(context, &bindings, &calls, &inst, &domains, &definitions);
    (bindings, facts)
}
fn id(bindings: &BindingFacts, name: &str) -> DeclarationId {
    bindings
        .declarations
        .iter()
        .find(|d| d.name == name)
        .unwrap()
        .id
}
fn declaration<'a>(
    facts: &'a NumericFacts,
    bindings: &BindingFacts,
    name: &str,
) -> &'a NumericDeclaration {
    facts
        .declarations
        .iter()
        .find(|d| d.declaration == id(bindings, name))
        .unwrap()
}
fn text<'a>(
    context: &'a ModelContext,
    file: usize,
    location: &zincite_lint::SourceLocation,
) -> &'a str {
    let source = &context.files[file];
    &source.parsed.source()
        [location.range.start - source.byte_offset..location.range.end - source.byte_offset]
}

#[test]
fn chains_intermediates_and_independent_requirements_support_only_conservative_proofs() {
    let source = concat!(
        "\u{feff}% é\r\ninclude \"included.mzn\"; type Positive=var 1..3; Positive: a;\n",
        "var int: alias=a; var int: chain=(alias+2)*2; var int: exact=-(6 div 2)+7 mod 3;\n",
        "1..3: bounded; var int: shifted=bounded+2; var 0..1: conflict=3+2;\n",
        "var 1..3: overlap=input; var {1,3}: sparse; var int: hull=sparse-sparse;\n",
        "var int: hole=2; var int: equality_alias; constraint equality_alias=chain;\n",
        "var int: exact_alias; constraint exact_alias=exact; 0..1: bad_default=5; solve satisfy;\n",
    );
    let included = "\u{feff}% å\r\nvar 2..4: input; var int: included_chain=input+1;";
    let (dir, context) = model("chains", source, included);
    let (bindings, facts) = facts(&context);
    assert!(facts.limitations.is_empty(), "{:?}", facts.limitations);
    assert_eq!(
        declaration(&facts, &bindings, "chain").value,
        NumericOutcome::Interval {
            lower: 6,
            upper: 10
        }
    );
    assert_eq!(
        declaration(&facts, &bindings, "exact").value,
        NumericOutcome::Exact(-2)
    );
    assert_eq!(
        declaration(&facts, &bindings, "shifted").value,
        NumericOutcome::Interval { lower: 3, upper: 5 }
    );
    assert_eq!(
        declaration(&facts, &bindings, "equality_alias").value,
        NumericOutcome::Interval {
            lower: 6,
            upper: 10
        }
    );
    assert_eq!(
        declaration(&facts, &bindings, "exact_alias").value,
        NumericOutcome::Exact(-2)
    );
    let bad_default = declaration(&facts, &bindings, "bad_default");
    assert_eq!(
        bad_default.value,
        NumericOutcome::Interval { lower: 0, upper: 1 }
    );
    let written_default = facts
        .definitions
        .iter()
        .find(|d| d.target == bad_default.declaration)
        .unwrap();
    assert!(written_default.parameter_default);
    assert_eq!(written_default.outcome, NumericOutcome::Exact(5));
    assert_eq!(
        written_default.domain_relation(bad_default),
        NumericDomainRelation::Inconclusive
    );
    let conflict = declaration(&facts, &bindings, "conflict");
    assert_eq!(
        conflict.required,
        NumericOutcome::Interval { lower: 0, upper: 1 }
    );
    assert_eq!(conflict.value, NumericOutcome::Exact(5));
    assert_eq!(
        facts
            .definitions
            .iter()
            .find(|d| d.target == conflict.declaration)
            .unwrap()
            .domain_relation(conflict),
        NumericDomainRelation::ExactOutside
    );
    assert_eq!(
        conflict.value.domain_relation(conflict),
        NumericDomainRelation::ExactOutside
    );
    assert_eq!(
        declaration(&facts, &bindings, "chain")
            .value
            .domain_relation(conflict),
        NumericDomainRelation::Disjoint
    );
    let overlap = declaration(&facts, &bindings, "overlap");
    assert_eq!(
        overlap.value.domain_relation(overlap),
        NumericDomainRelation::Inconclusive
    );
    assert_eq!(overlap.value.excludes(0), Some(true));
    assert_eq!(overlap.value.excludes(3), None);
    let sparse = declaration(&facts, &bindings, "sparse");
    assert_eq!(sparse.value.excludes(2), None);
    assert_eq!(
        declaration(&facts, &bindings, "hole")
            .value
            .domain_relation(sparse),
        NumericDomainRelation::ExactOutside
    );
    assert_eq!(
        declaration(&facts, &bindings, "hull").value.excludes(0),
        None
    );
    assert!(
        facts
            .expressions
            .iter()
            .any(|e| text(&context, e.file, &e.location) == "alias+2"
                && e.outcome == NumericOutcome::Interval { lower: 3, upper: 5 }
                && e.instantiation == Instantiation::Decision)
    );
    let included_expression = facts
        .expressions
        .iter()
        .find(|e| text(&context, e.file, &e.location) == "input+1")
        .unwrap();
    assert_eq!(included_expression.location.path, dir.join("included.mzn"));
    assert_eq!(
        &included[included_expression.location.range.clone()],
        "input+1"
    );
    assert_eq!(
        included_expression.outcome,
        NumericOutcome::Interval { lower: 3, upper: 5 }
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("root.mzn")).unwrap(),
        source
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("included.mzn")).unwrap(),
        included
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn missing_data_distinct_defaults_and_unsupported_math_retain_identity_and_locations() {
    let source = concat!(
        "include \"included.mzn\"; int: N=2; int: K=2; int: missing; var int: n_value=N+1; var int: k_value=K+1;\n",
        "var int: overflow=9223372036854775807+1; var int: invalid=missing div 0;\n",
        "var 9223372036854775807..N: high; var int: symbolic_overflow=high+1;\n",
        "var int: opaque=custom(1); function int: custom(int: value)=value; float: decimal=1.5;\n",
        "set of int: DefaultSet={1,3}; var DefaultSet: dependent; solve satisfy;\n",
    );
    let included = "var int: user_value=1*2; function int: '*'(int: a,int: b)=0;";
    let (dir, context) = model("uncertainty", source, included);
    let (bindings, facts) = facts(&context);
    let n = declaration(&facts, &bindings, "N");
    let k = declaration(&facts, &bindings, "K");
    assert_eq!(
        n.value,
        NumericOutcome::Symbolic {
            lower: NumericBound::Symbol(n.declaration),
            upper: NumericBound::Symbol(n.declaration)
        }
    );
    assert_ne!(n.value, k.value);
    assert_ne!(
        declaration(&facts, &bindings, "n_value").value,
        declaration(&facts, &bindings, "k_value").value
    );
    assert!(matches!(
        declaration(&facts, &bindings, "missing").value,
        NumericOutcome::Symbolic { .. }
    ));
    assert_eq!(
        NumericOutcome::Exact(2).domain_relation(declaration(&facts, &bindings, "dependent")),
        NumericDomainRelation::Inconclusive
    );
    for fragment in [
        "9223372036854775807+1",
        "missing div 0",
        "high+1",
        "custom(1)",
        "1*2",
        "1.5",
    ] {
        assert!(
            facts
                .limitations
                .iter()
                .any(|l| text(&context, l.file, &l.location) == fragment),
            "missing {fragment}: {:?}",
            facts.limitations
        );
        assert!(
            facts
                .expressions
                .iter()
                .any(|e| text(&context, e.file, &e.location) == fragment
                    && matches!(e.outcome, NumericOutcome::Unsupported(_)))
        );
    }
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "1*2"
                && l.location.path == dir.join("included.mzn"))
    );
    assert!(
        !facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "missing")
    );
    let calls = resolve_callables(&context, &bindings);
    let domains = resolve_domains(&context, &bindings);
    let invariant = resolve_integer_bounds(&context, &bindings, &calls, &domains);
    assert!(
        invariant
            .expressions
            .iter()
            .any(|e| text(&context, e.file, &e.location) == "missing div 0"
                && matches!(e.outcome, IntegerBoundsOutcome::Unknown(_)))
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cycles_and_conditional_or_unsafe_definitions_cannot_establish_values() {
    let source = concat!(
        "var int: self; constraint self=self; var int: left; var int: right; constraint left=right /\\ right=left;\n",
        "var 1..3: anchored; constraint anchored=anchored+1; var int: uses_anchor=anchored+1;\n",
        "var int: conditional; constraint true -> conditional=5; var int: unsafe=if true then 2 else 3 endif;\n",
        "var 2..4: unsafe_bounded=custom(); function int: custom()=9; solve satisfy;\n",
    );
    let (dir, context) = model("cycles", source, "");
    let (bindings, facts) = facts(&context);
    for name in ["self", "left", "right", "conditional", "unsafe"] {
        assert!(
            matches!(
                declaration(&facts, &bindings, name).value,
                NumericOutcome::Unknown(_)
            ),
            "{name}: {:?}",
            declaration(&facts, &bindings, name).value
        );
    }
    assert_eq!(
        declaration(&facts, &bindings, "anchored").value,
        NumericOutcome::Interval { lower: 1, upper: 3 }
    );
    assert_eq!(
        declaration(&facts, &bindings, "uses_anchor").value,
        NumericOutcome::Interval { lower: 2, upper: 4 }
    );
    assert_eq!(
        declaration(&facts, &bindings, "unsafe_bounded").value,
        NumericOutcome::Interval { lower: 2, upper: 4 }
    );
    assert!(
        facts
            .definitions
            .iter()
            .any(|d| d.target == id(&bindings, "conditional")
                && matches!(d.outcome, NumericOutcome::Unknown(_)))
    );
    assert!(
        facts
            .definitions
            .iter()
            .any(|d| d.target == id(&bindings, "self")
                && matches!(d.outcome, NumericOutcome::Unknown(_)))
    );
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| text(&context, l.file, &l.location) == "if true then 2 else 3 endif")
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn supplied_expression_order_and_duplicate_types_keep_any_match_bounds() {
    let (dir, context) = model(
        "expression-order",
        "\u{feff}include \"included.mzn\"; int: a=7; int: n; solve satisfy;",
        "int: b=7;",
    );
    let bindings = resolve_bindings(&context);
    let mut calls = resolve_callables(&context, &bindings);
    let domains = resolve_domains(&context, &bindings);
    let file = context.root_file.unwrap();
    let target = calls
        .expressions
        .iter()
        .find(|e| e.file == file && text(&context, file, &e.location) == "7")
        .unwrap()
        .clone();
    assert!(target.location.range.start >= 3);
    calls
        .expressions
        .retain(|e| e.file != file || e.location.range != target.location.range);
    calls.expressions.reverse();
    let mut wrong_file = target.clone();
    wrong_file.file = context
        .files
        .iter()
        .position(|f| f.path.ends_with("included.mzn"))
        .unwrap();
    let mut wrong_type = target.clone();
    wrong_type.ty.kind = zincite_lint::TypeKind::Bool;
    calls.expressions.insert(0, wrong_file);
    calls.expressions.insert(0, wrong_type);
    let bounds = resolve_integer_bounds(&context, &bindings, &calls, &domains);
    assert!(
        !bounds
            .expressions
            .iter()
            .any(|e| e.file == file && e.location.range == target.location.range)
    );
    for kind in [
        zincite_lint::TypeKind::Int,
        zincite_lint::TypeKind::Unknown("supplied uncertainty".into()),
    ] {
        let mut eligible = target.clone();
        eligible.ty.kind = kind;
        calls.expressions.push(eligible);
        let bounds = resolve_integer_bounds(&context, &bindings, &calls, &domains);
        let value = bounds
            .expressions
            .iter()
            .find(|e| e.file == file && e.location.range == target.location.range)
            .unwrap();
        assert_eq!(
            value.outcome,
            IntegerBoundsOutcome::Known { lower: 7, upper: 7 }
        );
        calls.expressions.pop();
    }
    std::fs::remove_dir_all(dir).unwrap();
}
