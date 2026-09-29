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
        ("int: bad = f(1)\nint: after = 7;", 12..13),
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
        "int: x = f(1);",
        "int: x :: named = 1;",
        "int: x = 1 + 2;",
        "int: x = -1;",
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
