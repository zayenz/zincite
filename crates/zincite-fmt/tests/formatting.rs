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

// Child boundaries retain list entries; formatting may add trailing commas.
// Compare nesting and significant spellings rather than only flat tokens.
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
                    TokenKind::Whitespace
                        | TokenKind::LineComment
                        | TokenKind::BlockComment
                        | TokenKind::Comma
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

#[test]
fn formats_collection_lists_and_preserves_comments_and_structure() {
    let source = include_str!("../../../tests/fixtures/collections.mzn");
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("set of int: Indices = {1, 2};"));
    assert!(formatted.contains("array[Indices] of opt int: optional = [0x2a, <>];"));
    assert!(formatted.contains(
        "list of int: values = [\n    0o7, % Keep  spelling\n    /* Next  entry */ 2,\n];"
    ));
    assert!(formatted.contains("nested[1][2] + (values)[1] + array1d(1 .. 2, [5, 6])[1]"));
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

    let source = "array[\n 1..2,\n 1..3\n] of int: a; any: x=f([\n1,\n2\n],{3,4}); any: y=a[\n1, % index\n2\n];";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("array[\n    1 .. 2,\n    1 .. 3,\n] of int"));
    assert!(formatted.contains("f([\n    1,\n    2,\n], {3, 4})"));
    assert!(formatted.contains("a[\n    1, % index\n    2,\n]"));
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
}
