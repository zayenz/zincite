use zincite_syntax::{NodeKind, ParsedFile, SyntaxElement, SyntaxNode, parse};

const MODEL: &str = include_str!("../../../tests/fixtures/scalar.mzn");

fn collect_leaves(node: &SyntaxNode, parsed: &ParsedFile, leaves: &mut Vec<usize>) {
    for child in node.children() {
        let range = match child {
            SyntaxElement::Node(child) => {
                collect_leaves(child, parsed, leaves);
                child.range()
            }
            SyntaxElement::Token(index) => {
                leaves.push(*index);
                parsed.tokens()[*index].range.clone()
            }
        };
        assert!(node.range().start <= range.start && range.end <= node.range().end);
    }
}

fn items(parsed: &ParsedFile) -> Vec<&SyntaxNode> {
    parsed
        .tree()
        .children()
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node),
            SyntaxElement::Token(_) => None,
        })
        .collect()
}

#[test]
fn scalar_tree_retains_every_token_and_exposes_items_and_atoms() {
    let parsed = parse(MODEL);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(parsed.tree().kind(), NodeKind::Root);
    assert_eq!(parsed.tree().range(), 0..MODEL.len());
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    let mut end = 0;
    let mut reconstructed = String::new();
    for index in leaves {
        let token = &parsed.tokens()[index];
        assert_eq!(token.range.start, end);
        reconstructed.push_str(&parsed.source()[token.range.clone()]);
        end = token.range.end;
    }
    assert_eq!(end, MODEL.len());
    assert_eq!(reconstructed, MODEL);
    let nodes = items(&parsed);
    assert_eq!(nodes.len(), 12);
    assert_eq!(nodes[0].kind(), NodeKind::Declaration);
    assert_eq!(nodes[2].kind(), NodeKind::Assignment);
    assert_eq!(nodes[9].kind(), NodeKind::Constraint);
    assert_eq!(nodes[11].kind(), NodeKind::Solve);
    let atom = nodes[0]
        .children()
        .iter()
        .find_map(|child| match child {
            SyntaxElement::Node(node) if node.kind() == NodeKind::Expression => Some(node),
            _ => None,
        })
        .unwrap();
    let start = MODEL.find("0x2a").unwrap();
    assert_eq!(atom.range(), start..start + 4);
    let string = nodes[7].child_nodes().nth(1).unwrap();
    assert_eq!(string.kind(), NodeKind::InterpolatedString);
    let embedded = string.child_nodes().collect::<Vec<_>>();
    assert_eq!(
        embedded.iter().map(|node| node.kind()).collect::<Vec<_>>(),
        [
            NodeKind::BinaryExpression,
            NodeKind::InterpolatedString,
            NodeKind::Expression,
        ]
    );
    assert_eq!(
        &MODEL[embedded[0].range()],
        "count+1::doc_comment(\"hello\")"
    );
    let annotated = embedded[0].child_nodes().nth(1).unwrap();
    assert_eq!(annotated.kind(), NodeKind::AnnotatedExpression);
    assert_eq!(
        annotated.child_nodes().nth(1).unwrap().kind(),
        NodeKind::Annotation
    );
    let inner = embedded[1].child_nodes().next().unwrap();
    let start = MODEL.find("extra*2").unwrap();
    assert_eq!(inner.kind(), NodeKind::BinaryExpression);
    assert_eq!(inner.range(), start..start + 7);
}

#[test]
fn malformed_interpolation_retains_coverage_and_recovers_after_its_closing_delimiter() {
    let source = r#"string: bad="value \(1+[; int: hidden; 2) end"; int: after=7;"#;
    let parsed = parse(source);
    assert_eq!(parsed.diagnostics().len(), 1);
    let bad = source.find(';').unwrap();
    assert_eq!(parsed.diagnostics()[0].range, bad..bad + 1);
    assert_eq!(parsed.diagnostics()[0].message, "expected an expression");
    let nodes = items(&parsed);
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].kind(), NodeKind::Error);
    assert_eq!(&source[nodes[1].range()], "int: after=7;");
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    assert_eq!(
        leaves
            .iter()
            .map(|&index| &source[parsed.tokens()[index].range.clone()])
            .collect::<String>(),
        source
    );
    for source in [r#"string: bad="\()"; int: after=7;"#, r#"string: bad="\(1"#] {
        assert!(!parse(source).diagnostics().is_empty());
    }
}

#[test]
fn syntax_errors_retain_ranges_and_a_following_declaration() {
    for (source, bad_range) in [
        ("int: bad = ;\nint: after = 7;", 11..12),
        ("int: bad = 1\nint: after = 7;", 13..16),
        ("int: bad = f(1 + )\nint: after = 7;", 17..18),
    ] {
        let parsed = parse(source);
        assert_eq!(parsed.diagnostics().len(), 1);
        assert_eq!(parsed.diagnostics()[0].range, bad_range);
        let nodes = items(&parsed);
        assert_eq!(nodes[0].kind(), NodeKind::Error);
        assert_eq!(nodes[1].kind(), NodeKind::Declaration);
        assert_eq!(&source[nodes[1].range()], "int: after = 7;");
        let mut leaves = Vec::new();
        collect_leaves(parsed.tree(), &parsed, &mut leaves);
        assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    }
}

#[test]
fn unsupported_expressions_are_diagnosed_and_anonymous_atoms_parse() {
    let parsed = parse("int: x = case x of 1: 2 endcase;");
    assert!(
        parsed
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("unsupported"))
    );
    let parsed = parse("var int: x = _;");
    assert!(parsed.diagnostics().is_empty());
    let node = items(&parsed)[0];
    assert!(node.children().iter().any(|child| {
        matches!(child, SyntaxElement::Node(atom)
            if atom.kind() == NodeKind::Expression && &parsed.source()[atom.range()] == "_")
    }));
}

fn nested(node: &SyntaxNode) -> Vec<&SyntaxNode> {
    node.children()
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(node),
            _ => None,
        })
        .collect()
}

fn operator_text<'a>(node: &SyntaxNode, parsed: &'a ParsedFile) -> Vec<&'a str> {
    node.children()
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Token(index)
                if !matches!(
                    parsed.tokens()[*index].kind,
                    zincite_syntax::TokenKind::Whitespace
                        | zincite_syntax::TokenKind::LineComment
                        | zincite_syntax::TokenKind::BlockComment
                ) =>
            {
                Some(&parsed.source()[parsed.tokens()[*index].range.clone()])
            }
            _ => None,
        })
        .collect()
}

#[test]
fn precedence_and_associativity_are_visible_in_the_tree() {
    let parsed = parse("x=1+2*3; y=2^3^2; z=-2^2; s=a++b++c; b=not x::a -> y <- z;");
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let nodes = items(&parsed);
    let addition = nested(nodes[0])[0];
    assert_eq!(operator_text(addition, &parsed), ["+"]);
    assert_eq!(operator_text(nested(addition)[1], &parsed), ["*"]);
    let power = nested(nodes[1])[0];
    assert_eq!(operator_text(power, &parsed), ["^"]);
    assert_eq!(operator_text(nested(power)[0], &parsed), ["^"]);
    let negative = nested(nodes[2])[0];
    assert_eq!(negative.kind(), NodeKind::UnaryExpression);
    assert_eq!(operator_text(nested(negative)[0], &parsed), ["^"]);
    let concat = nested(nodes[3])[0];
    assert_eq!(operator_text(concat, &parsed), ["++"]);
    assert_eq!(operator_text(nested(concat)[1], &parsed), ["++"]);
    let implication = nested(nodes[4])[0];
    assert_eq!(operator_text(implication, &parsed), ["<-"]);
    let forward = nested(implication)[0];
    assert_eq!(operator_text(forward, &parsed), ["->"]);
    let negation = nested(forward)[0];
    assert_eq!(negation.kind(), NodeKind::UnaryExpression);
    assert_eq!(nested(negation)[0].kind(), NodeKind::AnnotatedExpression);
    for source in [
        "x=1<2<=3;",
        "x=a=b!=c;",
        "x=a in b subset c;",
        "x=1..2..<3;",
    ] {
        assert!(
            parse(source)
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.message.contains("non-associative"))
        );
    }
    assert!(parse("x=(1<2)<3;").diagnostics().is_empty());
}

#[test]
fn calls_annotations_and_ranges_retain_all_tokens_and_bounds() {
    let source = "any: value :: tag(1) = f(1, 'arg name': '+'(2, 3,), z: empty(),) :: (a::b);\nann: a=cons(_, <>); x=C^-1(v)+C⁻¹(v)+v⁻¹; y=..3 union 1..< union 1<..2 union 1<..<2; var -(n+1)..max(true, 9): bounded;";
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let nodes = items(&parsed);
    assert_eq!(nested(nodes[0])[1].kind(), NodeKind::Annotation);
    let annotated_call = nested(nodes[0])[2];
    assert_eq!(annotated_call.kind(), NodeKind::AnnotatedExpression);
    let call = nested(annotated_call)[0];
    assert_eq!(call.kind(), NodeKind::CallExpression);
    assert_eq!(nested(call)[1].kind(), NodeKind::NamedArgument);
    let range_union = nested(nodes[3])[0];
    let last_range = nested(range_union)[1];
    assert_eq!(last_range.kind(), NodeKind::RangeExpression);
    assert_eq!(nested(last_range).len(), 2);
    let previous_union = nested(range_union)[0];
    let first_union = nested(previous_union)[0];
    assert_eq!(nested(nested(first_union)[0]).len(), 1);
    assert_eq!(nested(nested(first_union)[1]).len(), 1);
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
}

#[test]
fn numeric_type_bounds_check_grammar_without_inference() {
    for source in [
        "var true..3: x;",
        "var (a=1)..3: x;",
        "var 1..not x: y;",
        "var 1..(a union b): x;",
        "var 1.._: x;",
        "var 1..<>: x;",
        "var 1..\"2\": x;",
        "var 1..infinity: x;",
        "int: x=..;",
        "int: x=f(1,,2);",
        "int: x=1+;",
    ] {
        let parsed = parse(format!("{source} int: after=7;"));
        assert!(!parsed.diagnostics().is_empty(), "{source}");
        assert_eq!(
            items(&parsed).last().unwrap().kind(),
            NodeKind::Declaration,
            "{source}"
        );
    }
    let parsed = parse("var f(true)::tag(<>)+n `combine` m..'+'(1,2): x; any: y=true..false;");
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn collection_types_entries_and_indices_share_the_lossless_tree() {
    let source = include_str!("../../../tests/fixtures/collections.mzn");
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let nodes = items(&parsed);
    let set_type = nodes[0].child_nodes().next().unwrap();
    assert_eq!(set_type.kind(), NodeKind::SetType);
    assert_eq!(
        set_type.child_nodes().next().unwrap().kind(),
        NodeKind::ScalarType
    );
    let array_type = nodes[2].child_nodes().next().unwrap();
    assert_eq!(array_type.kind(), NodeKind::ArrayType);
    assert_eq!(
        array_type
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [NodeKind::DomainType, NodeKind::ScalarType]
    );
    let literal = nodes[2].child_nodes().nth(1).unwrap();
    assert_eq!(literal.kind(), NodeKind::ArrayLiteral);
    assert_eq!(
        literal
            .child_nodes()
            .map(|entry| &source[entry.range()])
            .collect::<Vec<_>>(),
        ["0x2a", "<>"]
    );
    assert_eq!(
        nodes[3].child_nodes().next().unwrap().kind(),
        NodeKind::ListType
    );
    let access = nodes[6].child_nodes().nth(1).unwrap();
    assert_eq!(access.kind(), NodeKind::ArrayAccessExpression);
    assert_eq!(
        access
            .child_nodes()
            .map(|entry| &source[entry.range()])
            .collect::<Vec<_>>(),
        ["grid", "..", "2"]
    );
    let keyed = nodes[10].child_nodes().nth(1).unwrap();
    assert_eq!(
        keyed
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [NodeKind::IndexedArrayEntry, NodeKind::IndexedArrayEntry]
    );
    assert_eq!(
        keyed
            .child_nodes()
            .next()
            .unwrap()
            .child_nodes()
            .map(|child| &source[child.range()])
            .collect::<Vec<_>>(),
        ["3", "7"]
    );
    let stepped = nodes[11].child_nodes().nth(1).unwrap();
    assert_eq!(
        stepped
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [
            NodeKind::IndexedArrayEntry,
            NodeKind::Expression,
            NodeKind::Expression
        ]
    );
    let tuple = nodes[12]
        .child_nodes()
        .nth(1)
        .unwrap()
        .child_nodes()
        .next()
        .unwrap()
        .child_nodes()
        .next()
        .unwrap();
    assert_eq!(tuple.kind(), NodeKind::IndexTuple);
    assert_eq!(
        tuple
            .child_nodes()
            .map(|child| &source[child.range()])
            .collect::<Vec<_>>(),
        ["1", "2"]
    );
    let matrix = nodes[16].child_nodes().nth(1).unwrap();
    assert_eq!(matrix.kind(), NodeKind::MatrixLiteral);
    assert_eq!(
        matrix
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [
            NodeKind::MatrixColumnIndices,
            NodeKind::MatrixRow,
            NodeKind::MatrixRow
        ]
    );
    assert_eq!(
        matrix
            .child_nodes()
            .map(|row| row
                .child_nodes()
                .map(|child| &source[child.range()])
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [vec!["5", "6"], vec!["3", "0x1", "2"], vec!["4", "3", "0o4"]]
    );
    let dependent = nodes[17].child_nodes().next().unwrap();
    let binding = dependent.child_nodes().next().unwrap();
    assert_eq!(binding.kind(), NodeKind::ArrayIndexBinding);
    assert_eq!(operator_text(binding, &parsed), ["c", "in"]);
    let name = binding
        .children()
        .iter()
        .find_map(|child| match child {
            SyntaxElement::Token(index)
                if parsed.tokens()[*index].kind == zincite_syntax::TokenKind::Identifier =>
            {
                Some(parsed.tokens()[*index].range.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(&source[name], "c");
    let set = dependent.child_nodes().nth(1).unwrap();
    assert_eq!(set.kind(), NodeKind::SetType);
    let cardinality = set.child_nodes().next().unwrap();
    assert_eq!(cardinality.kind(), NodeKind::SetCardinality);
    assert_eq!(&source[cardinality.range()], "(c)");
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    let reconstructed = leaves
        .iter()
        .map(|&index| &source[parsed.tokens()[index].range.clone()])
        .collect::<String>();
    assert_eq!(reconstructed, source);
}

#[test]
fn comprehension_heads_and_generator_filters_are_direct_ordered_children() {
    let parsed = parse(include_str!("../../../tests/fixtures/collections.mzn"));
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let nodes = items(&parsed);
    let set = nodes[21].child_nodes().nth(1).unwrap();
    assert_eq!(set.kind(), NodeKind::SetComprehension);
    let list = set.child_nodes().nth(1).unwrap();
    assert_eq!(list.kind(), NodeKind::GeneratorList);
    let generators = list.child_nodes().collect::<Vec<_>>();
    assert_eq!(operator_text(generators[0], &parsed), ["i", ",", "j", "in"]);
    assert_eq!(operator_text(generators[1], &parsed), ["k", "="]);
    assert_eq!(
        generators
            .iter()
            .map(|generator| generator
                .child_nodes()
                .map(|child| (child.kind(), &parsed.source()[child.range()]))
                .collect::<Vec<_>>())
            .collect::<Vec<_>>(),
        [
            vec![
                (NodeKind::Expression, "Indices"),
                (NodeKind::WhereFilter, "where i<j")
            ],
            vec![
                (NodeKind::BinaryExpression, "i+j"),
                (NodeKind::WhereFilter, "where k>0")
            ],
        ]
    );
    let nested = nodes[22].child_nodes().nth(1).unwrap();
    assert_eq!(nested.kind(), NodeKind::ArrayComprehension);
    let call = nested.child_nodes().next().unwrap();
    assert_eq!(call.kind(), NodeKind::GeneratorCallExpression);
    assert_eq!(
        call.child_nodes().map(SyntaxNode::kind).collect::<Vec<_>>(),
        [NodeKind::GeneratorList, NodeKind::ParenthesizedExpression]
    );
    let source = call
        .child_nodes()
        .next()
        .unwrap()
        .child_nodes()
        .next()
        .unwrap()
        .child_nodes()
        .next()
        .unwrap();
    assert_eq!(source.kind(), NodeKind::SetComprehension);
    for (item, key) in [(23, NodeKind::Expression), (24, NodeKind::IndexTuple)] {
        let indexed = nodes[item].child_nodes().nth(1).unwrap();
        assert_eq!(indexed.kind(), NodeKind::IndexedArrayComprehension);
        let entry = indexed.child_nodes().next().unwrap();
        assert_eq!(entry.kind(), NodeKind::IndexedArrayEntry);
        assert_eq!(entry.child_nodes().next().unwrap().kind(), key);
    }
    let written = nodes[25]
        .child_nodes()
        .nth(1)
        .unwrap()
        .child_nodes()
        .nth(1)
        .unwrap()
        .child_nodes()
        .next()
        .unwrap();
    assert_eq!(
        operator_text(written, &parsed),
        ["_", ",", "'written name'", "in"]
    );
    let parsed = parse(
        "var sum(i in 1..3)(i)..sum(j=2)(j+1): bounded; any: x=sum(i in 1..3)([i])[1]::tag; any: open=[i | i in 1.. where true,]; any: trailing=[i | i, in {1},]; any: ordinary=f(i in A, i=2, named:3);",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
}

#[test]
fn malformed_and_excluded_collections_recover_to_the_next_item() {
    for source in [
        "array[] of int: bad;",
        "array[i in ] of int: bad;",
        "array[i in 1..3 where true] of int: bad;",
        "set() of int: bad;",
        "set(2 of int: bad;",
        "any: bad=[1:];",
        "any: bad=[1,2:3];",
        "any: bad=[1:2,3,4:5];",
        "any: bad=[1:2,3:4,5];",
        "any: bad=[(1,,2):3];",
        "any: bad=[(1,2)];",
        "any: bad=[1:2, 3:4 | i in 1..3];",
        "any: bad=[|1,,2|];",
        "any: bad=[|1,2];",
        "any: bad=[|1|2:|];",
        "any: bad=[|1:2:3|];",
        "any: bad={i | };",
        "any: bad=[i | i in ];",
        "any: bad=[i | i,j=1];",
        "any: bad=[i | i in 1..3 where ];",
        "any: bad=[i | i in 1..3 where true where false];",
        "any: bad=[i,j | i in 1..3];",
        "any: bad=forall(i in 1..3)();",
        "any: bad=sum(i in 1..3)(i,);",
        "any: bad=f(1)(2);",
        "any: bad=C^-1(i in 1..3)(i);",
        "any: bad=a[];",
        "any: bad=[1,,2];",
        "any: bad=a.field;",
    ] {
        let parsed = parse(format!("{source} int: after=7;"));
        assert!(!parsed.diagnostics().is_empty(), "{source}");
        assert_eq!(
            items(&parsed).last().unwrap().kind(),
            NodeKind::Declaration,
            "{source}"
        );
        assert_eq!(
            &parsed.source()[items(&parsed).last().unwrap().range()],
            "int: after=7;"
        );
        let mut leaves = Vec::new();
        collect_leaves(parsed.tree(), &parsed, &mut leaves);
        assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    }
    let parsed = parse(
        "any: empty_sets={}; any: empty_arrays=[]; array[1..2, 1..3,] of var opt 1..9: a; any: sliced=a[..2, 1..]; any: whole=a[..,<..,..<,<..<]; any: tagged=x::tag[1]; any: empty_matrix=[||]; any: empty_rows=[| | |]; any: empty_cols=[|1:2:|]; any: single_key=[(1,):2]; any: empty_key=[():2]; any: tuple_key=[(\n1,\n2,\n):3]; array[i in 1..2, 1..3] of int: mixed; array[1..2, j in 1..3] of int: reverse;",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
}

#[test]
fn control_expressions_retain_branches_local_bindings_and_greedy_bodies() {
    let source = include_str!("../../../tests/fixtures/control.mzn");
    let parsed = parse(source);
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut leaves = Vec::new();
    collect_leaves(parsed.tree(), &parsed, &mut leaves);
    assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    let reconstructed: String = leaves
        .iter()
        .map(|&i| &source[parsed.tokens()[i].range.clone()])
        .collect();
    assert_eq!(reconstructed, source);
    let parenthesized = items(&parsed)[1].child_nodes().nth(1).unwrap();
    assert_eq!(parenthesized.kind(), NodeKind::ParenthesizedExpression);
    let conditional = parenthesized.child_nodes().next().unwrap();
    assert_eq!(conditional.kind(), NodeKind::ConditionalExpression);
    assert_eq!(
        conditional
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [
            NodeKind::ConditionalBranch,
            NodeKind::ConditionalBranch,
            NodeKind::ElseBranch
        ]
    );
    let local = conditional
        .child_nodes()
        .next()
        .unwrap()
        .child_nodes()
        .nth(1)
        .unwrap();
    assert_eq!(local.kind(), NodeKind::LetExpression);
    let block = local.child_nodes().next().unwrap();
    assert_eq!(block.kind(), NodeKind::LetBlock);
    assert_eq!(
        block
            .child_nodes()
            .map(SyntaxNode::kind)
            .collect::<Vec<_>>(),
        [
            NodeKind::Declaration,
            NodeKind::Declaration,
            NodeKind::Constraint
        ]
    );
    let declaration = block.child_nodes().next().unwrap();
    let name = declaration
        .children()
        .iter()
        .find_map(|child| match child {
            SyntaxElement::Token(i)
                if parsed.tokens()[*i].kind == zincite_syntax::TokenKind::Identifier =>
            {
                Some(&parsed.tokens()[*i])
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(&source[name.range.clone()], "local_value");
    assert_eq!(name.range.start, source.find("local_value=count").unwrap());
    let greedy = parse(
        "any: x=2*let {any: y=3,} in y+4::tag; var (if true then 1 else 2 endif)..(let {int: n=5;} in n+1): v;",
    );
    assert!(
        greedy.diagnostics().is_empty(),
        "{:?}",
        greedy.diagnostics()
    );
    let product = items(&greedy)[0].child_nodes().nth(1).unwrap();
    assert_eq!(product.kind(), NodeKind::BinaryExpression);
    let local = product.child_nodes().nth(1).unwrap();
    assert_eq!(local.kind(), NodeKind::LetExpression);
    assert_eq!(
        local.child_nodes().nth(1).unwrap().kind(),
        NodeKind::BinaryExpression
    );
}

#[test]
fn malformed_local_blocks_recover_to_following_items() {
    for source in [
        "any: x=let {int: bad=; int: later=2; constraint true;} in later;",
        "any: x=let {int: bad=1; constraint ;} in bad;",
        "any: x=let {int: bad=1; constraint true; in bad;",
        "any: x=let {int: bad=1;} bad;",
        "any: x=let {bad=1;} in bad;",
        "any: x=let {solve satisfy;} in 1;",
        "any: x=let {} in 1;",
        "any: x=let {tuple(int): t=(1,);} in t;",
        "any: x=if true 1 else 2 endif;",
        "any: x=if true then 1 elseif false then endif;",
    ] {
        let source = format!("{source}\nint: after=7;");
        let parsed = parse(source.clone());
        assert!(!parsed.diagnostics().is_empty(), "{source}");
        let last = items(&parsed).last().copied().unwrap();
        assert_eq!(last.kind(), NodeKind::Declaration, "{source}");
        assert_eq!(&source[last.range()], "int: after=7;");
        let mut leaves = Vec::new();
        collect_leaves(parsed.tree(), &parsed, &mut leaves);
        assert_eq!(leaves, (0..parsed.tokens().len()).collect::<Vec<_>>());
    }
}
