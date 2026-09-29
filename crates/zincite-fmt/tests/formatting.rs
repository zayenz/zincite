use zincite_fmt::format;
use zincite_syntax::{TokenKind, parse};

const MODEL: &str = include_str!("../../../tests/fixtures/scalar.mzn");

#[test]
fn formats_a_complete_model_and_preserves_spelling_on_a_stable_second_pass() {
    let parsed = parse(MODEL);
    let formatted = format(&parsed).unwrap();
    assert!(
        formatted.contains("par /* type  comment */ int: count = 0x2a; % hexadecimal spelling\n")
    );
    assert!(formatted.contains("int: extra;\nextra = 0o7;\nvar int: choice = count;"));
    assert!(
        formatted.contains("\n\nconstraint :: \"Scalar  label\" % keep  label comment\ntrue;\n")
    );
    assert!(formatted.ends_with("solve satisfy;\n"));
    let reparsed = parse(formatted.clone());
    assert!(reparsed.diagnostics().is_empty());
    let protected = |source: &zincite_syntax::ParsedFile| {
        source
            .tokens()
            .iter()
            .filter(|token| {
                !matches!(
                    token.kind,
                    TokenKind::Whitespace
                        | TokenKind::Colon
                        | TokenKind::Equal
                        | TokenKind::Semicolon
                )
            })
            .map(|token| source.source()[token.range.clone()].to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(protected(&parsed), protected(&reparsed));
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn errors_produce_no_formatted_source() {
    for source in ["int: bad = ; int: after = 7;", "constraint f(1 + );"] {
        let parsed = parse(source);
        assert_eq!(format(&parsed).unwrap_err(), parsed.diagnostics());
    }
}

// Retain nesting and significant spellings, rather than only comparing flat tokens.
fn structure(node: &zincite_syntax::SyntaxNode, parsed: &zincite_syntax::ParsedFile) -> String {
    use zincite_syntax::SyntaxElement;
    let children = node
        .children()
        .iter()
        .filter_map(|child| match child {
            SyntaxElement::Node(node) => Some(structure(node, parsed)),
            SyntaxElement::Token(index) => {
                let token = &parsed.tokens()[*index];
                (!matches!(
                    token.kind,
                    TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
                ))
                .then(|| parsed.source()[token.range.clone()].to_owned())
            }
        })
        .collect::<Vec<_>>();
    format!("{:?}({})", node.kind(), children.join(","))
}

#[test]
fn formats_nested_expressions_with_stable_structure_and_multiline_calls() {
    let source = "int:x=-2^2+2^3^2; string:s=\"a\"++\"b\"++\"c\";\nany:v::tag(1)=f(1,'arg name': '+'(2,3,),z:empty(),)::(a::b);\nconstraint::\"label\" not(x)::a -> 'not'(x)::b;\nany:r=..3 union 1..<; any:i=C^-1(v)+C⁻¹(v)+v⁻¹;\nany:n=+ +2; any:m=f(\n1, % Keep  comment\n nested(2,3),\n); solve satisfy;";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("int: x = -2 ^ 2 + 2 ^ 3 ^ 2;"));
    assert!(formatted.contains("f(1, 'arg name': '+'(2, 3,), z: empty(),) :: (a :: b)"));
    assert!(formatted.contains("constraint :: \"label\"\nnot (x) :: a -> 'not'(x) :: b;"));
    assert!(formatted.contains("C^-1(v) + C⁻¹(v) + v⁻¹"));
    assert!(formatted.contains("+ +2"));
    assert!(formatted.contains("f(\n    1, % Keep  comment\n    nested(2, 3),\n)"));
    let reparsed = parse(formatted.clone());
    assert!(
        reparsed.diagnostics().is_empty(),
        "{:?}",
        reparsed.diagnostics()
    );
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
}
