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
    for source in [
        "int: x = f([1]);",
        "int: x = a[1];",
        "int: x = if true then 1 else 2 endif;",
        "int: x = f(1)(2);",
    ] {
        let parsed = parse(source);
        assert!(
            parsed
                .diagnostics()
                .iter()
                .any(|diagnostic| { diagnostic.message.contains("unsupported") })
        );
    }
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
    assert_eq!(nested(nodes[0])[0].kind(), NodeKind::Annotation);
    let annotated_call = nested(nodes[0])[1];
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
