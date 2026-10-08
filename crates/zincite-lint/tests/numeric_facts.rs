use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, DeclarationId, Instantiation, IntegerBoundsOutcome, ModelContext, ModelOptions,
    NumericBound, NumericDeclaration, NumericDomainRelation, NumericFacts, NumericOutcome,
    load_model, resolve_bindings, resolve_callables, resolve_definitions, resolve_domains,
    resolve_instantiations, resolve_integer_bounds, resolve_numeric_facts,
};
const CORE: &str = concat!(
    "function int:bool2int(bool:x); function var int:bool2int(var bool:x); function opt int:bool2int(opt bool:x);\n",
    "function int: min(int:a,int:b); function int: max(int:a,int:b);\n",
    "function var int: min(var int:a,var int:b); function var int: max(var int:a,var int:b);\n",
    "function int: length(array[int] of int:a); function int: length(array[int] of var int:a);\n",
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
    std::fs::write(dir.join("included.mzn"), "").unwrap();
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}function set of int: '..'(int: left,int: right); function var bool: 'not'(var bool: value);\n"),
    )
    .unwrap();
    std::fs::write(
        dir.join("root.mzn"),
        concat!(
            "array[1..4] of var bool: flags; int: N; array[1..N] of var bool: symbolic;\n",
            "var int: selected=1*flags[1]; var int: outside=1*flags[5]; var int: unproved=1*symbolic[1];\n",
            "var int: nested_unknown=1*(not symbolic[1]); solve satisfy;\n",
        ),
    )
    .unwrap();
    let selections = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    assert!(selections.errors.is_empty());
    let (bindings, numeric) = self::facts(&selections);
    let definition = |name: &str| {
        &numeric
            .definitions
            .iter()
            .find(|d| d.target == id(&bindings, name))
            .unwrap()
            .outcome
    };
    assert!(matches!(definition("selected"), NumericOutcome::Unknown(_)));
    for name in ["nested_unknown", "outside", "unproved"] {
        assert!(
            matches!(definition(name), NumericOutcome::Unsupported(_)),
            "{name}: {:?}",
            definition(name)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cycles_and_conditional_or_unsafe_definitions_cannot_establish_values() {
    let (dir, context) = model(
        "bool2int-numeric",
        concat!(
            "bool:p; var bool:q; bool:safe=p; function bool:opaque(); bool:bad=opaque(); bool:cycle=cycle;\n",
            "annotation unsafe; bool:annotated::unsafe=p; opt bool:o; int:a=bool2int(p); var int:b=bool2int(q); int:c=bool2int(safe); int:d=bool2int(true);\n",
            "int:e=bool2int(opaque()); int:f=bool2int(bad); int:g=(bool2int(p)::unsafe); int:h=bool2int(annotated); int:i=bool2int(cycle); opt int:j=bool2int(o); solve satisfy;\n",
        ),
        "",
    );
    let (_, values) = self::facts(&context);
    let outcome = |source| {
        &values
            .expressions
            .iter()
            .find(|value| text(&context, value.file, &value.location) == source)
            .unwrap_or_else(|| panic!("missing {source}"))
            .outcome
    };
    for source in [
        "bool2int(p)",
        "bool2int(q)",
        "bool2int(safe)",
        "bool2int(true)",
    ] {
        assert!(
            matches!(outcome(source), NumericOutcome::Unknown(_)),
            "{source}: {:?}",
            outcome(source)
        );
    }
    for source in [
        "bool2int(opaque())",
        "bool2int(bad)",
        "bool2int(p)::unsafe",
        "bool2int(annotated)",
        "bool2int(cycle)",
        "bool2int(o)",
    ] {
        assert!(
            matches!(outcome(source), NumericOutcome::Unsupported(_)),
            "{source}: {:?}",
            outcome(source)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context) = model(
        "user-bool2int-numeric",
        "function int:bool2int(bool:x)=1; int:a=bool2int(true); solve satisfy;",
        "",
    );
    let (_, values) = self::facts(&context);
    assert!(
        values
            .expressions
            .iter()
            .any(
                |value| text(&context, value.file, &value.location) == "bool2int(true)"
                    && matches!(value.outcome, NumericOutcome::Unsupported(_))
            )
    );
    std::fs::remove_dir_all(dir).unwrap();
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
    let (dir, _) = model(
        "length-extrema",
        concat!(
            "int: N; set of int: S; set of int: R=1..N; array[1..3] of var int: values;\n",
            "var int:x; int:a=length(S); int:b=length(R); int:c=length(values);\n",
            "int:d=min(N,2); var int:e=max(x,N); int:f=max(N,9 div 0);\n",
            "set of int:bad={1 div 0}; int:g=length(bad); annotation unsafe;\n",
            "int:h=(max(N,2)::unsafe); int:i=(length(S)::unsafe); solve satisfy;\n",
        ),
        "",
    );
    std::fs::write(
        dir.join("library/std/stdlib.mzn"),
        format!("{CORE}function set of int: '..'(int:a,int:b);\n"),
    )
    .unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..Default::default()
        },
    );
    assert!(context.errors.is_empty());
    let (_, values) = self::facts(&context);
    let outcome = |source| {
        &values
            .expressions
            .iter()
            .find(|e| text(&context, e.file, &e.location) == source)
            .unwrap_or_else(|| panic!("missing {source}"))
            .outcome
    };
    for source in [
        "length(S)",
        "length(R)",
        "length(values)",
        "min(N,2)",
        "max(x,N)",
    ] {
        assert!(
            matches!(outcome(source), NumericOutcome::Unknown(_)),
            "{source}: {:?}",
            outcome(source)
        );
    }
    for source in [
        "max(N,9 div 0)",
        "length(bad)",
        "max(N,2)::unsafe",
        "length(S)::unsafe",
    ] {
        assert!(
            matches!(outcome(source), NumericOutcome::Unsupported(_)),
            "{source}: {:?}",
            outcome(source)
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context) = model(
        "user-length",
        "array[1..2] of int: a; function int:length(array[int] of int:v)=1; int:n=length(a); solve satisfy;",
        "",
    );
    let (_, values) = self::facts(&context);
    assert!(
        values
            .expressions
            .iter()
            .any(|e| text(&context, e.file, &e.location) == "length(a)"
                && matches!(e.outcome, NumericOutcome::Unsupported(_)))
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
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
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
    let included_file = context
        .files
        .iter()
        .position(|f| f.path.ends_with("included.mzn"))
        .unwrap();
    let mut wrong_file = target.clone();
    wrong_file.file = included_file;
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
        calls.expressions.push(eligible.clone());
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
        let values =
            resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
        let expected: Vec<_> = calls
            .expressions
            .iter()
            .filter(|e| {
                context.files[e.file].warnings_enabled()
                    && matches!(
                        e.ty.kind,
                        zincite_lint::TypeKind::Int
                            | zincite_lint::TypeKind::Float
                            | zincite_lint::TypeKind::Unknown(_)
                    )
                    && !(e.file == included_file && e.location.range == target.location.range)
            })
            .map(|e| (e.file, e.location.clone()))
            .collect();
        assert_eq!(
            values
                .expressions
                .iter()
                .map(|e| (e.file, e.location.clone()))
                .collect::<Vec<_>>(),
            expected
        );
        let duplicates: Vec<_> = values
            .expressions
            .iter()
            .filter(|e| e.file == file && e.location.range == target.location.range)
            .collect();
        assert_eq!(duplicates.len(), 2);
        assert!(
            duplicates
                .iter()
                .all(|e| e.outcome == NumericOutcome::Exact(7))
        );
        assert!(values.expressions.iter().any(|e| {
            e.file == included_file
                && text(&context, e.file, &e.location) == "7"
                && e.outcome == NumericOutcome::Exact(7)
        }));
        calls.expressions.pop();
        calls.expressions.pop();
    }
    calls.expressions.push(target.clone());
    calls.expressions[0].ty.optional = true;
    let values = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    assert!(values.expressions.iter().any(|e| {
        e.file == file
            && e.location.range == target.location.range
            && matches!(&e.outcome, NumericOutcome::Unsupported(reason) if reason.contains("presence"))
    }));
    calls.expressions[0].ty.optional = false;
    calls.expressions[0].ty.kind = zincite_lint::TypeKind::Float;
    let values = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let duplicates: Vec<_> = values
        .expressions
        .iter()
        .filter(|e| e.file == file && e.location.range == target.location.range)
        .collect();
    assert_eq!(duplicates.len(), 2);
    assert!(duplicates.iter().all(|e| {
        matches!(&e.outcome, NumericOutcome::Unsupported(reason) if reason.contains("floating-point"))
    }));
    std::fs::remove_dir_all(dir).unwrap();
}
