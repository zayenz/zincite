use std::path::PathBuf;
use zincite_lint::*;
const CORE: &str = concat!(
    "function int: '+'(int:a,int:b); function int: '-'(int:a,int:b); function int: 'div'(int:a,int:b);\n",
    "function bool: '>'(int:a,int:b); function bool: '<'(int:a,int:b); function bool: '='(int:a,int:b); function bool: 'in'(int:a,set of int:b);\n",
    "function set of int: '..'(int:a,int:b);\n",
    "function int: sum(array[int] of int:a); function int: product(array[int] of int:a); function bool: forall(array[int] of bool:a); function bool: exists(array[int] of bool:a);\n",
    "function set of $I: index_set(array[$I] of $T:a);\n",
    "function set of $I: index_set_1of2(array[$I,$J] of $T:a); function set of $J: index_set_2of2(array[$I,$J] of $T:a);\n",
    "annotation mark(int:x);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext, BindingFacts, IterationFacts) {
    let dir = std::env::temp_dir().join(format!("zincite-iteration-{name}-{}", std::process::id()));
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
    let optional = resolve_optional_facts(
        &context,
        &bindings,
        &calls,
        &inst,
        &domains,
        &numeric,
        &definitions,
    );
    let guarded = resolve_guarded_facts_with_options(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
    );
    let facts = resolve_iteration_facts(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional, &guarded,
    );
    (dir, context, bindings, facts)
}
fn text<'a>(context: &'a ModelContext, file: FileId, location: &SourceLocation) -> &'a str {
    let source = &context.files[file];
    &source.parsed.source()
        [location.range.start - source.byte_offset..location.range.end - source.byte_offset]
}
fn iteration<'a>(
    context: &ModelContext,
    facts: &'a IterationFacts,
    spelling: &str,
) -> &'a IterationFact {
    facts
        .iterations
        .iter()
        .find(|i| text(context, i.file, &i.location) == spelling)
        .unwrap_or_else(|| panic!("missing {spelling}"))
}
fn set<'a>(
    context: &ModelContext,
    facts: &'a IterationFacts,
    spelling: &str,
) -> &'a IterationIndexSet {
    facts
        .index_sets
        .iter()
        .find(|i| text(context, i.file, &i.location) == spelling)
        .unwrap_or_else(|| panic!("missing set {spelling}"))
}
#[test]
fn membership_relations_keep_order_offsets_enums_and_replaceable_identity() {
    let source = concat!(
        "enum X={A,B,C}; enum Y={D,E,F}; int:N=3; int:K=3; set of int:P={1,2,3}; set of int:Q={1,2,3};\n",
        "array[0..2] of int:a; array[1..3] of int:b; array[X,1..3] of int:grid;\n",
        "array[int] of int: p=[i | i in P]; array[int] of int:q=[i | i in Q];\n",
        "array[int] of int:n=[i | i in 1..N]; array[int] of int:k=[i | i in 1..K];\n",
        "array[int] of int:x=[1 | i in X]; array[int] of int:y=[1 | i in Y];\n",
        "array[int] of int:ax=[i+1 | i in index_set(a)]; array[int] of int:bx=[i | i in index_set(b)]; array[int] of int:commute=[1+i | i in index_set(a)]; array[int] of int:overflow=[i+1 | i in 9223372036854775807..9223372036854775807];\n",
        "array[int] of int:g1=[1 | i in index_set_1of2(grid)]; array[int] of int:g2=[i | i in index_set_2of2(grid)];\n",
        "array[int] of int:smaller=[i | i in {1,3}]; array[int] of int:apart=[i | i in 8..9]; array[int] of int:empty=[i | i in 1..0];\n",
        "array[int] of int:local=let {set of int:L={1,3};} in [i | i in L]; solve satisfy;\n",
    );
    let (dir, context, bindings, facts) = model("sets", source);
    let a = set(&context, &facts, "index_set(a)");
    let b = set(&context, &facts, "index_set(b)");
    assert_eq!(a.cardinality, Cardinality::Exact(3));
    assert_eq!(b.cardinality, a.cardinality);
    assert_eq!(a.equal_members(b), GuardedOutcome::Refuted);
    assert_eq!(a.coverage_of(b), IterationCoverage::ProperPartial);
    assert_eq!(
        set(&context, &facts, "i+1").equal_members(b),
        GuardedOutcome::Proven
    );
    assert_eq!(
        set(&context, &facts, "1+i").equal_members(b),
        GuardedOutcome::Proven
    );
    assert!(matches!(
        set(&context, &facts, "i+1").cardinality,
        Cardinality::Exact(_)
    ));
    assert!(
        facts
            .limitations
            .iter()
            .any(|l| l.reason == "index offset overflow")
    );
    let x = set(&context, &facts, "X");
    let y = set(&context, &facts, "Y");
    assert_eq!(x.cardinality, Cardinality::Exact(3));
    assert_eq!(x.equal_members(y), GuardedOutcome::Refuted);
    assert_eq!(x.disjoint_from(y), GuardedOutcome::Proven);
    assert_eq!(
        x.equal_members(set(&context, &facts, "index_set_1of2(grid)")),
        GuardedOutcome::Proven
    );
    assert_eq!(
        b.equal_members(set(&context, &facts, "index_set_2of2(grid)")),
        GuardedOutcome::Proven
    );
    let grid = bindings
        .declarations
        .iter()
        .find(|d| d.name == "grid")
        .unwrap()
        .id;
    let dimensions = &facts
        .arrays
        .iter()
        .find(|a| a.declaration == grid)
        .unwrap()
        .dimensions;
    assert_eq!(dimensions.len(), 2);
    assert_eq!(dimensions[0].equal_members(x), GuardedOutcome::Proven);
    assert_eq!(dimensions[1].equal_members(b), GuardedOutcome::Proven);
    let subset = set(&context, &facts, "{1,3}");
    assert_eq!(subset.subset_of(b), GuardedOutcome::Proven);
    assert_eq!(subset.coverage_of(b), IterationCoverage::ProperPartial);
    assert_eq!(
        subset.disjoint_from(set(&context, &facts, "8..9")),
        GuardedOutcome::Proven
    );
    assert_eq!(
        set(&context, &facts, "1..0").coverage_of(b),
        IterationCoverage::Empty
    );
    assert_eq!(
        set(&context, &facts, "P").equal_members(set(&context, &facts, "P")),
        GuardedOutcome::Proven
    );
    assert_eq!(
        set(&context, &facts, "P").equal_members(set(&context, &facts, "Q")),
        GuardedOutcome::Unknown
    );
    assert_eq!(
        set(&context, &facts, "1..N").equal_members(set(&context, &facts, "1..K")),
        GuardedOutcome::Unknown
    );
    let mut anonymous_left = set(&context, &facts, "P").clone();
    let mut anonymous_right = set(&context, &facts, "Q").clone();
    for index in [&mut anonymous_left, &mut anonymous_right] {
        index.domain = Domain::Range {
            lower: NumericBound::Integer(1),
            upper: NumericBound::Arithmetic {
                operator: zincite_syntax::TokenKind::Plus,
                operands: vec![NumericBound::Unknown, NumericBound::Integer(1)],
            },
        };
    }
    assert_eq!(
        anonymous_left.equal_members(&anonymous_right),
        GuardedOutcome::Unknown
    );
    assert_eq!(
        set(&context, &facts, "1..N").equal_members(set(&context, &facts, "1..N")),
        GuardedOutcome::Proven
    );
    assert_eq!(
        set(&context, &facts, "L").equal_members(subset),
        GuardedOutcome::Proven
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context, _, facts) = model(
        "opaque-target",
        "function int: '+'(int:a,int:b)=9; array[int] of int:xs=[i | i in 1..3 where i in {1+1}]; solve satisfy;",
    );
    let i = iteration(&context, &facts, "[i | i in 1..3 where i in {1+1}]");
    assert_eq!(i.coverage, IterationCoverage::Unknown);
    assert!(matches!(i.selected, CandidateCount::UpperBound(3)));
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn lexical_dependencies_include_later_sources_annotations_and_partial_operations() {
    let source = "int: result=sum(i in 1..3 where i>0,k=i+1,j in 1..(6 div k) where j>i)(let {int:t=k;} in (t+j+sum(i in 1..2 where k>0)(i))::mark(i)); solve satisfy;";
    let (dir, context, bindings, facts) = model("uses", source);
    let outer = facts
        .iterations
        .iter()
        .find(|i| i.generators.len() == 3)
        .unwrap();
    let i = outer.generators[0].bindings[0];
    let k = outer.generators[1].bindings[0];
    let j = outer.generators[2].bindings[0];
    assert_eq!(outer.generators[1].slots, 1);
    assert!(!outer.generators[1].membership);
    assert!(outer.generators[2].dependencies.contains(&k));
    assert!(outer.generators[2].dependencies.contains(&i));
    assert!(matches!(outer.candidates, CandidateCount::Unknown));
    for (spelling, scope) in [("i>0", 1), ("i+1", 1), ("1..(6 div k)", 2), ("j>i", 3)] {
        let e = outer
            .expressions
            .iter()
            .find(|e| text(&context, e.file, &e.location) == spelling)
            .unwrap();
        assert_eq!(e.earliest_scope, Some(scope), "{spelling}: {e:?}");
    }
    let partial = outer
        .expressions
        .iter()
        .find(|e| text(&context, e.file, &e.location) == "1..(6 div k)")
        .unwrap();
    assert!(
        partial
            .obligations
            .iter()
            .any(|o| matches!(o.kind, GuardObligationKind::Nonzero))
    );
    assert!(
        outer
            .uses
            .iter()
            .any(|u| u.binding == k && u.region == IterationUseRegion::Source(2))
    );
    assert!(
        outer
            .uses
            .iter()
            .any(|u| u.binding == i && u.region == IterationUseRegion::Filter(2))
    );
    assert!(outer.uses.iter().any(|u| u.binding == i && u.annotation));
    let body = outer
        .expressions
        .iter()
        .find(|e| e.location.range == outer.body.range)
        .unwrap();
    assert!(body.iteration_dependencies.contains(&k));
    assert!(body.iteration_dependencies.contains(&j));
    let inner = iteration(&context, &facts, "sum(i in 1..2 where k>0)(i)");
    let inner_i = inner.generators[0].bindings[0];
    let nested_filter = inner
        .expressions
        .iter()
        .find(|e| text(&context, e.file, &e.location) == "k>0")
        .unwrap();
    assert_eq!(nested_filter.earliest_scope, Some(0));
    assert!(nested_filter.iteration_dependencies.contains(&k));
    assert_ne!(inner_i, i);
    assert!(inner.uses.iter().all(|u| u.binding == inner_i));
    assert!(outer.uses.iter().all(|u| u.binding != inner_i));
    let literal = outer
        .expressions
        .iter()
        .find(|e| {
            text(&context, e.file, &e.location) == "1"
                && outer.body.range.start <= e.location.range.start
                && e.location.range.end <= outer.body.range.end
        })
        .unwrap();
    assert!(literal.invariant_with_respect_to.contains(&i));
    assert_eq!(bindings.declarations[k.0].role, DeclarationRole::Generator);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn cartesian_products_coverage_and_multiplicity_remain_separate_from_presence() {
    let source = concat!(
        "int:N; int:K; set of int:S; var set of 1..4:V; var set of 1..N:VN; var bool:keep;\n",
        "array[int] of int:anon=[1 | _,_ in 1..3]; array[int] of int:named=[1 | i,j in 1..3];\n",
        "array[int] of int:symbolic=[1 | _,_ in 1..N,j in 1..K,k in 1..2];\n",
        "array[int] of int:huge=[1 | i,j in 1..9223372036854775807];\n",
        "array[int] of int:empty=[1 | i in 1..0]; array[int] of int:unknown=[1 | i in S];\n",
        "array[int] of int:false_filter=[1 | i in 1..3 where false]; array[int] of var opt int:maybe=[1 | i in 1..3 where keep];\n",
        "array[int] of var opt int:variable=[1 | i in V]; array[int] of var opt int:variable_n=[1 | i in VN]; array[int] of int:part=[i | i in 1..4 where i>2];\n",
        "array[int] of int:dependent=[j | i in 1..3,j in 1..i];\n",
        "bool:q=forall(i in 1..3)(true); int:s=sum(i in 1..3)(1); int:p=product(i in 1..3)(2); solve satisfy;\n",
    );
    let (dir, context, bindings, facts) = model("counts", source);
    for spelling in ["[1 | _,_ in 1..3]", "[1 | i,j in 1..3]"] {
        let i = iteration(&context, &facts, spelling);
        assert!(matches!(i.candidates, CandidateCount::Exact(9)));
        assert_eq!(
            i.optional_collection.as_ref().unwrap().capacity,
            Cardinality::Exact(9)
        );
        assert_eq!(i.coverage, IterationCoverage::Full);
    }
    let symbolic = iteration(&context, &facts, "[1 | _,_ in 1..N,j in 1..K,k in 1..2]");
    let CandidateCount::Symbolic {
        coefficient,
        factors,
        upper_bound,
    } = &symbolic.candidates
    else {
        panic!("{:?}", symbolic.candidates)
    };
    assert_eq!(*coefficient, 2);
    assert_eq!(factors.len(), 3);
    assert!(!upper_bound);
    let n = bindings
        .declarations
        .iter()
        .find(|d| d.name == "N")
        .unwrap()
        .id;
    let k = bindings
        .declarations
        .iter()
        .find(|d| d.name == "K")
        .unwrap()
        .id;
    assert!(factors[0].dependencies.contains(&n));
    assert!(factors[1].dependencies.contains(&n));
    assert!(factors[2].dependencies.contains(&k));
    assert_eq!((factors[0].slot, factors[1].slot), (0, 1));
    assert!(matches!(
        iteration(&context, &facts, "[1 | i,j in 1..9223372036854775807]").candidates,
        CandidateCount::Unsupported(_)
    ));
    assert!(!facts.limitations.is_empty());
    for spelling in ["[1 | i in 1..0]", "[1 | i in 1..3 where false]"] {
        let i = iteration(&context, &facts, spelling);
        assert_eq!(i.coverage, IterationCoverage::Empty);
        assert!(matches!(i.selected, CandidateCount::Exact(0)));
    }
    let maybe = iteration(&context, &facts, "[1 | i in 1..3 where keep]");
    assert_eq!(maybe.coverage, IterationCoverage::Unknown);
    assert!(matches!(maybe.candidates, CandidateCount::Exact(3)));
    assert!(matches!(maybe.selected, CandidateCount::UpperBound(3)));
    assert_eq!(
        maybe.optional_collection.as_ref().unwrap().present,
        Cardinality::Bounds { lower: 0, upper: 3 }
    );
    let variable = iteration(&context, &facts, "[1 | i in V]");
    assert!(matches!(variable.candidates, CandidateCount::UpperBound(4)));
    assert!(variable.generators[0].index_set.universe.is_some());
    assert_eq!(
        variable.optional_collection.as_ref().unwrap().capacity,
        Cardinality::Exact(4)
    );
    let variable_n = iteration(&context, &facts, "[1 | i in VN]");
    assert!(
        matches!(&variable_n.candidates,CandidateCount::Symbolic{upper_bound:true,factors,..} if matches!(factors[0].domain,Domain::Range{..}))
    );
    let part = iteration(&context, &facts, "[i | i in 1..4 where i>2]");
    assert_eq!(part.coverage, IterationCoverage::ProperPartial);
    assert!(matches!(part.selected, CandidateCount::Exact(2)));
    let dependent = iteration(&context, &facts, "[j | i in 1..3,j in 1..i]");
    assert!(matches!(
        dependent.candidates,
        CandidateCount::UpperBound(9)
    ));
    assert_eq!(dependent.generators[1].uniform_upper_bound, Some(3));
    assert_eq!(
        iteration(&context, &facts, "forall(i in 1..3)(true)").multiplicity,
        IterationMultiplicity::IdempotentQuantifier
    );
    for spelling in ["sum(i in 1..3)(1)", "product(i in 1..3)(2)"] {
        assert_eq!(
            iteration(&context, &facts, spelling).multiplicity,
            IterationMultiplicity::Arithmetic
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn rejecting_filters_do_not_relabel_inherited_emptiness() {
    let source = "int:N=3; bool:keep; function bool:opaque(int:i); array[int] of int:a=[i | i in 1..3 where i>3,j in 1..2 where keep]; array[int] of int:b=[i | i in 1..N where i>N]; array[int] of int:c=[i | i in 1..3 where opaque(i)]; array[int] of int:d=[i | i in 1..3 where i in {1,4}]; solve satisfy;";
    let (dir, context, _, facts) = model("filters", source);
    let first = iteration(
        &context,
        &facts,
        "[i | i in 1..3 where i>3,j in 1..2 where keep]",
    );
    assert_eq!(first.filters[0].coverage, IterationCoverage::Empty);
    assert!(!first.filters[0].incoming_empty);
    assert!(first.filters[0].prior_domain.is_some());
    assert_eq!(first.filters[0].truth, GuardedOutcome::Refuted);
    let intersection = &iteration(&context, &facts, "[i | i in 1..3 where i in {1,4}]").filters[0];
    assert!(intersection.prior_domain.is_some() && intersection.selected_domain.is_some());
    assert_eq!(intersection.coverage, IterationCoverage::ProperPartial);
    assert!(first.filters[1].incoming_empty);
    assert_eq!(first.filters[1].truth, GuardedOutcome::Unknown);
    assert_eq!(first.filters[1].coverage, IterationCoverage::Unknown);
    assert_eq!(
        iteration(&context, &facts, "[i | i in 1..N where i>N]").filters[0].coverage,
        IterationCoverage::Unknown
    );
    assert!(matches!(
        iteration(&context, &facts, "[i | i in 1..3 where opaque(i)]").filters[0].definedness,
        GuardedOutcome::Unsupported(_)
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn supplied_expression_order_keeps_first_exact_file_range_kind() {
    let (dir, context, bindings, _) = model(
        "expression-order",
        "\u{feff}function set of int: opaque(int: x); set of int: result=opaque(1); solve satisfy;",
    );
    let mut calls = resolve_callables(&context, &bindings);
    let inst = resolve_instantiations(&context, &bindings, &calls);
    let domains = resolve_domains(&context, &bindings);
    let definitions = resolve_definitions(&context, &bindings, &calls, &inst, &domains);
    let numeric = resolve_numeric_facts(&context, &bindings, &calls, &inst, &domains, &definitions);
    let optional = resolve_optional_facts(
        &context,
        &bindings,
        &calls,
        &inst,
        &domains,
        &numeric,
        &definitions,
    );
    let guarded = resolve_guarded_facts_with_options(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional,
    );
    let file = context.root_file.unwrap();
    let target = calls
        .expressions
        .iter()
        .find(|e| e.file == file && text(&context, file, &e.location) == "opaque(1)")
        .unwrap()
        .clone();
    calls
        .expressions
        .retain(|e| e.file != file || e.location.range != target.location.range);
    calls.expressions.reverse();
    let mut unknown = target.clone();
    unknown.ty.kind = TypeKind::Unknown("supplied uncertainty".into());
    let mut other_file = target.clone();
    other_file.file = context.implicit_core.unwrap();
    calls.expressions.insert(0, other_file);
    calls.expressions.insert(0, target.clone());
    calls.expressions.insert(0, unknown);
    let facts = resolve_iteration_facts(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional, &guarded,
    );
    assert!(
        !facts
            .index_sets
            .iter()
            .any(|e| e.file == file && e.location.range == target.location.range)
    );
    calls.expressions.swap(0, 1);
    let facts = resolve_iteration_facts(
        &context, &bindings, &calls, &inst, &domains, &numeric, &optional, &guarded,
    );
    let result = facts
        .index_sets
        .iter()
        .find(|e| e.file == file && e.location.range == target.location.range)
        .unwrap();
    assert_eq!(result.location.range, target.location.range);
    std::fs::remove_dir_all(dir).unwrap();
}
