use zincite_syntax::{LexedSource, TokenKind, lex};

const COVERAGE: &str = concat!(
    "% café λ 🦀\r\n",
    "\tvar int: 'odd\\name' = -0x2a; /* 雪 */\n",
    r#"string: text = "quote \" slash \\ \n \t \1012 \x41F";"#,
    "\n",
    "constraint text = \"bad\\q\"; 💥 int: after = 0o7;\n",
    "'unterminated\n",
    "solve satisfy;\n",
    "/* unfinished",
);

fn assert_coverage(lexed: &LexedSource) {
    let source = lexed.source();
    let mut next = 0;
    let mut reconstructed = String::new();
    for token in lexed.tokens() {
        assert_eq!(token.range.start, next);
        assert!(token.range.end > token.range.start);
        assert!(source.is_char_boundary(token.range.start));
        assert!(source.is_char_boundary(token.range.end));
        reconstructed.push_str(&source[token.range.clone()]);
        next = token.range.end;
    }
    assert_eq!(next, source.len());
    assert_eq!(reconstructed, source);
    for diagnostic in lexed.diagnostics() {
        assert!(diagnostic.range.start < diagnostic.range.end);
        assert!(diagnostic.range.end <= source.len());
        assert!(source.is_char_boundary(diagnostic.range.start));
        assert!(source.is_char_boundary(diagnostic.range.end));
        assert!(!diagnostic.message.is_empty());
    }
}

#[test]
fn retains_all_bytes_and_recovers_after_lexical_errors() {
    let lexed = lex(COVERAGE.to_owned());
    assert_coverage(&lexed);
    assert_eq!(lexed.tokens()[0].kind, TokenKind::LineComment);
    assert_eq!(lexed.tokens()[0].range, 0..COVERAGE.find('\r').unwrap());
    for (spelling, kind) in [
        ("'odd\\name'", TokenKind::QuotedIdentifier),
        ("/* 雪 */", TokenKind::BlockComment),
        ("0x2a", TokenKind::IntegerLiteral),
        ("\"bad\\q\"", TokenKind::Error),
        ("💥", TokenKind::Error),
        ("after", TokenKind::Identifier),
        ("solve", TokenKind::Solve),
    ] {
        let start = COVERAGE.find(spelling).unwrap();
        assert!(
            lexed.tokens().iter().any(|token| {
                token.kind == kind && token.range == (start..start + spelling.len())
            })
        );
    }
    let escape = COVERAGE.find("\\q").unwrap();
    assert!(lexed.diagnostics().iter().any(|diagnostic| {
        diagnostic.range == (escape..escape + 2) && diagnostic.message == "invalid string escape"
    }));
    assert_eq!(lexed.diagnostics().len(), 4);
}

#[test]
fn distinguishes_numeral_and_operator_boundaries() {
    use TokenKind::*;
    let lexed = lex(
        "-1..2 0x2..3 0x2.field 0o7 1.5e-2 0x.8p-1 0X1.p2 <..< <= ~!= x~divy ∈ ⁻¹ $T $$U `'weird name'`",
    );
    assert_coverage(&lexed);
    assert!(lexed.diagnostics().is_empty());
    let significant: Vec<_> = lexed
        .tokens()
        .iter()
        .filter(|token| token.kind != Whitespace)
        .map(|token| (token.kind, &lexed.source()[token.range.clone()]))
        .collect();
    assert_eq!(
        significant,
        [
            (Minus, "-"),
            (IntegerLiteral, "1"),
            (RangeInclusive, ".."),
            (IntegerLiteral, "2"),
            (IntegerLiteral, "0x2"),
            (RangeInclusive, ".."),
            (IntegerLiteral, "3"),
            (IntegerLiteral, "0x2"),
            (Dot, "."),
            (Identifier, "field"),
            (IntegerLiteral, "0o7"),
            (FloatLiteral, "1.5e-2"),
            (FloatLiteral, "0x.8p-1"),
            (FloatLiteral, "0X1.p2"),
            (RangeExclusive, "<..<"),
            (LessEqual, "<="),
            (WeakNotEqual, "~!="),
            (Identifier, "x"),
            (WeakDiv, "~div"),
            (Identifier, "y"),
            (In, "∈"),
            (Inverse, "⁻¹"),
            (TypeInstVariable, "$T"),
            (TypeInstVariable, "$$U"),
            (InfixIdentifier, "`'weird name'`"),
        ]
    );

    let malformed = lex("0x; 0o8; 1e+; 0X1; int: after;");
    assert_coverage(&malformed);
    assert_eq!(malformed.diagnostics().len(), 4);
    assert!(malformed.tokens().iter().any(|token| token.kind == Int));
    assert!(malformed.tokens().iter().any(|token| {
        token.kind == Identifier && &malformed.source()[token.range.clone()] == "after"
    }));
}

#[test]
fn tokenizes_interpolation_through_nested_delimiters() {
    let source = r#""value \(f("inner \(g(1))", 'odd)name', /* ) " */ (2))) end"; int: next = 1;"#;
    let lexed = lex(source);
    assert_coverage(&lexed);
    assert_eq!(lexed.tokens()[0].kind, TokenKind::StringHead);
    assert_eq!(lexed.tokens()[0].range, 0..source.find("f(").unwrap());
    assert!(lexed.diagnostics().is_empty());
    let chunks = lexed
        .tokens()
        .iter()
        .filter(|token| {
            matches!(
                token.kind,
                TokenKind::StringHead | TokenKind::StringMiddle | TokenKind::StringTail
            )
        })
        .map(|token| (token.kind, &source[token.range.clone()]))
        .collect::<Vec<_>>();
    assert_eq!(
        chunks,
        [
            (TokenKind::StringHead, r#""value \("#),
            (TokenKind::StringHead, r#""inner \("#),
            (TokenKind::StringTail, r#")""#),
            (TokenKind::StringTail, r#") end""#),
        ]
    );
    assert!(
        lexed
            .tokens()
            .iter()
            .any(|token| token.kind == TokenKind::Int)
    );

    let plain = lex(r#""escaped \\(x) and \1234 \x123""#);
    assert_coverage(&plain);
    assert_eq!(plain.tokens().len(), 1);
    assert_eq!(plain.tokens()[0].kind, TokenKind::StringLiteral);
    assert!(plain.diagnostics().is_empty());
}
