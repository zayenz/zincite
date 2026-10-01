use std::path::PathBuf;
use zincite_lint::{
    BindingFacts, CallableFacts, Cardinality, Instantiation, ModelContext, ModelOptions,
    OptionalFacts, Presence, TypeKind, load_model, resolve_bindings, resolve_callables,
    resolve_definitions, resolve_domains, resolve_instantiations, resolve_numeric_facts,
    resolve_optional_facts,
};
const CORE: &str = concat!(
    "function int: '+'(int:a,int:b); function int: '*'(int:a,int:b); function int: 'mod'(int:a,int:b);\n",
    "function bool: '='(int:a,int:b);\n",
    "function int: '+'(opt int:a,opt int:b); function int: '*'(opt int:a,opt int:b);\n",
    "test occurs(opt $T:x); test absent(opt $T:x); function $T: deopt(opt $T:x);\n",
    "function var bool: occurs(var opt $T:x); function var bool: absent(var opt $T:x); function var $T: deopt(var opt $T:x);\n",
    "function $T: 'default'(opt $T:x,$T:y);\n",
);
fn model(
    name: &str,
    source: &str,
) -> (
    PathBuf,
    ModelContext,
    BindingFacts,
    CallableFacts,
    OptionalFacts,
) {
    let dir = std::env::temp_dir().join(format!("zincite-optional-{name}-{}", std::process::id()));
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
    let facts = resolve_optional_facts(
        &context,
        &bindings,
        &calls,
        &inst,
        &domains,
        &numeric,
        &definitions,
    );
    (dir, context, bindings, calls, facts)
}
fn expression<'a>(
    context: &ModelContext,
    facts: &'a OptionalFacts,
    text: &str,
) -> &'a zincite_lint::OptionalExpression {
    facts
        .expressions
        .iter()
        .find(|e| {
            let source = &context.files[e.file];
            source.parsed.source()[e.location.range.start - source.byte_offset
                ..e.location.range.end - source.byte_offset]
                .trim()
                == text
        })
        .unwrap_or_else(|| panic!("missing {text}"))
}
fn count<'a>(
    context: &ModelContext,
    facts: &'a OptionalFacts,
    text: &str,
) -> &'a zincite_lint::CollectionCardinality {
    let e = expression(context, facts, text);
    facts.collection(e.file, &e.location).unwrap()
}
#[test]
fn capacity_and_presence_follow_generator_kind_and_keep_parameter_identity() {
    let source = concat!(
        "var set of 1..4: S; var bool: keep; int:N=3; int:K=3; set of int:P={1,2};\n",
        "array[1..3] of var int: values;\n",
        "array[int] of var opt int: a=[i | i in S];\n",
        "array[int] of var opt int: b=[i | i in 1..3 where keep];\n",
        "array[int] of int: c=[i | i in 1..4 where i mod 2=0];\n",
        "array[int] of var int: d=[x | x in values];\n",
        "array[int] of int: e=[i | i,j in 1..3];\n",
        "array[int] of int: n=[i | i in 1..N]; array[int] of int: k=[i | i in 1..K];\n",
        "array[int] of int: p=[i | i in P]; array[int] of int: dep=[j | i in 1..3,j in 1..i];\n",
        "array[int] of opt int: absent_values=[<> | i in 1..3];\n",
        "array[int] of int: huge=[i | i,j in 1..9223372036854775807]; solve satisfy;\n",
    );
    let (dir, context, bindings, calls, facts) = model("counts", source);
    for (text, size) in [("[i | i in S]", 4), ("[i | i in 1..3 where keep]", 3)] {
        let c = count(&context, &facts, text);
        assert_eq!(c.capacity, Cardinality::Exact(size));
        assert_eq!(
            c.present,
            Cardinality::Bounds {
                lower: 0,
                upper: size
            }
        );
        assert_eq!(c.element_presence, Presence::Conditional);
        assert!(!c.conditions.is_empty());
    }
    let c = count(&context, &facts, "[i | i in 1..4 where i mod 2=0]");
    assert_eq!(c.candidates, Cardinality::Exact(4));
    assert_eq!(c.capacity, Cardinality::Bounds { lower: 0, upper: 4 });
    assert_eq!(c.present, c.capacity);
    assert_eq!(c.element_presence, Presence::Present);
    let d = count(&context, &facts, "[x | x in values]");
    assert_eq!(d.present, Cardinality::Exact(3));
    let e = expression(&context, &facts, "[x | x in values]");
    let ty = &calls
        .expressions
        .iter()
        .find(|t| t.file == e.file && t.location.range == e.location.range)
        .unwrap()
        .ty;
    assert_eq!(ty.instantiation, Instantiation::Decision);
    assert!(matches!(&ty.kind, TypeKind::Array { element, .. } if !element.optional));
    assert_eq!(
        count(&context, &facts, "[i | i,j in 1..3]").capacity,
        Cardinality::Exact(9)
    );
    for (text, name) in [
        ("[i | i in 1..N]", "N"),
        ("[i | i in 1..K]", "K"),
        ("[i | i in P]", "P"),
    ] {
        let id = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .id;
        assert_eq!(
            count(&context, &facts, text).capacity,
            Cardinality::Symbolic {
                dependencies: vec![id]
            }
        );
    }
    assert_eq!(
        count(&context, &facts, "[j | i in 1..3,j in 1..i]").capacity,
        Cardinality::Unknown
    );
    let absent = count(&context, &facts, "[<> | i in 1..3]");
    assert_eq!(absent.capacity, Cardinality::Exact(3));
    assert_eq!(absent.present, Cardinality::Exact(0));
    assert!(matches!(
        count(&context, &facts, "[i | i,j in 1..9223372036854775807]").capacity,
        Cardinality::Unsupported(_)
    ));
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| l.reason == "collection count overflow")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn intrinsic_presence_follows_enforced_aliases_without_substituting_defaults() {
    let source = concat!(
        "\u{feff}% é\r\nopt int: missing; opt int: default_absent=<>; opt int: default_present=7;\n",
        "var opt int: none=<>; var opt int: some=3; var opt int: alias=none;\n",
        "var bool: select; var opt int: conditional=if select then 4 else <> endif;\n",
        "var opt int: cycle_a=cycle_b; var opt int: cycle_b=cycle_a;\n",
        "array[1..3] of opt int: xs=[<>,<>,<>];\n",
        "array[int] of opt int: locals=let {opt int:local_absent=<>; opt int:local_present=3; opt int:local_alias=local_absent;} in [local_absent,local_present,local_alias];\n",
        "function opt int: with_default(opt int:formal_default=<> )=formal_default; solve satisfy;\n",
    );
    let (dir, context, bindings, _, facts) = model("presence", source);
    for (name, expected) in [
        ("missing", Presence::Unknown),
        ("default_absent", Presence::Unknown),
        ("default_present", Presence::Unknown),
        ("none", Presence::Absent),
        ("some", Presence::Present),
        ("alias", Presence::Absent),
        ("conditional", Presence::Conditional),
        ("cycle_a", Presence::Unknown),
        ("local_absent", Presence::Absent),
        ("local_present", Presence::Present),
        ("local_alias", Presence::Absent),
        ("formal_default", Presence::Unknown),
    ] {
        let id = bindings
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .id;
        assert_eq!(facts.declaration(id).unwrap().presence, expected, "{name}");
    }
    let xs = bindings
        .declarations
        .iter()
        .find(|d| d.name == "xs")
        .unwrap()
        .id;
    assert_eq!(
        facts
            .declaration(xs)
            .unwrap()
            .collection
            .as_ref()
            .unwrap()
            .present,
        Cardinality::Bounds { lower: 0, upper: 3 }
    );
    assert_eq!(
        count(&context, &facts, "[<>,<>,<>]").present,
        Cardinality::Exact(0)
    );
    for (name, expected) in [
        ("local_absent", Presence::Absent),
        ("local_present", Presence::Present),
        ("local_alias", Presence::Absent),
    ] {
        assert_eq!(
            expression(&context, &facts, name).presence,
            expected,
            "local reference {name}"
        );
    }
    assert_eq!(
        count(&context, &facts, "[local_absent,local_present,local_alias]").present,
        Cardinality::Exact(1)
    );
    std::fs::remove_dir_all(dir).unwrap();
}
