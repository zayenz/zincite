use zincite_fmt::format;
use zincite_syntax::{TokenKind, parse};

const MODEL: &str = include_str!("../../../tests/fixtures/scalar.mzn");

#[test]
fn include_groups_sort_decoded_paths_with_attached_comments_and_stable_duplicates() {
    let source = r#"% Z  attachment
include "z.mzn"; % Z  tail
/* A  attachment */
include "a.mzn"; % First  a
include "\x67lobals.mzn";
include "A.mzn";
include "\141.mzn"; % Second  a
include "\xc3\xa9.mzn"; % First  unicode
include "é.mzn"; % Second  unicode

% Section  stays

include "q.mzn";
include "b.mzn";
int: barrier=1;
include "d.mzn";
include "c.mzn";
"#;
    let expected = r#"include "\x67lobals.mzn";
include "A.mzn";
/* A  attachment */
include "a.mzn"; % First  a
include "\141.mzn"; % Second  a
% Z  attachment
include "z.mzn"; % Z  tail
include "\xc3\xa9.mzn"; % First  unicode
include "é.mzn"; % Second  unicode

% Section  stays

include "b.mzn";
include "q.mzn";
int: barrier = 1;
include "c.mzn";
include "d.mzn";
"#;
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert_eq!(formatted, expected);
    let reparsed = parse(formatted.clone());
    assert!(reparsed.diagnostics().is_empty());
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn include_sorting_respects_directive_and_protected_barriers() {
    let skipped = "\r\n% zincite-fmt: skip\r\n include  \"globals.mzn\" ; \r\n";
    let off = "\r\n% zincite-fmt: off\r\ninclude  \"z.mzn\" ;\r\ninclude \"a.mzn\";\r\n% zincite-fmt: on\r\n";
    let source = format!(
        "include \"z.mzn\";\ninclude \"a.mzn\";{skipped}include \"d.mzn\";\ninclude \"c.mzn\";{off}include \"f.mzn\";\ninclude \"e.mzn\";\n% zincite-lint: ignore naming\ninclude \"q.mzn\";\ninclude \"b.mzn\";\ninclude \"a.mzn\";\ninclude \"\\(path).mzn\";\ninclude \"d.mzn\";\ninclude \"c.mzn\";"
    );
    let expected = format!(
        "include \"a.mzn\";\ninclude \"z.mzn\";{skipped}include \"c.mzn\";\ninclude \"d.mzn\";{off}include \"e.mzn\";\ninclude \"f.mzn\";\n% zincite-lint: ignore naming\ninclude \"q.mzn\";\ninclude \"a.mzn\";\ninclude \"b.mzn\";\ninclude \"\\(path).mzn\";\ninclude \"c.mzn\";\ninclude \"d.mzn\";\n"
    );
    let formatted = format(&parse(source)).unwrap();
    assert_eq!(formatted, expected);
    assert_eq!(format(&parse(formatted.clone())).unwrap(), formatted);
}

#[test]
fn directives_preserve_items_regions_and_boundary_bytes() {
    use zincite_fmt::protected_ranges;

    let skip = " \r\n\t% zincite-fmt: skip  \r\n\r\n  array [1..2] of int: kept = [ 1,2 ]; % keep  tail\r\n \t\r\n";
    let off = "\r\n\t% zincite-fmt: off\r\n  string: kept_text=\"keep  spacing\";\r\n\r\n int: kept_number =  7 ; \r\n % zincite-fmt: on\r\n  ";
    let source = format!("int: before=0;{skip}int: middle=1;{off}int: after=3;");
    let parsed = parse(source.clone());
    let ranges = protected_ranges(&parsed).unwrap();
    assert_eq!(
        ranges
            .iter()
            .map(|range| &source[range.clone()])
            .collect::<Vec<_>>(),
        [skip, off]
    );
    let formatted = format(&parsed).unwrap();
    assert_eq!(
        formatted,
        format!("int: before = 0;{skip}int: middle = 1;{off}int: after = 3;\n")
    );
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);

    // Protected EOF layout overrides the ordinary final-newline default, even
    // for a final item without a written semicolon.
    let tail = "\r\n% zincite-fmt: skip\r\n\tint: kept=2   \r\n \t";
    let source = format!("int: before=0;{tail}");
    let formatted = format(&parse(source)).unwrap();
    assert_eq!(formatted, format!("int: before = 0;{tail}"));
    assert_eq!(format(&parse(formatted.clone())).unwrap(), formatted);

    for source in [
        "% zincite-fmt: skip\r\n int:a=1;\r\n% zincite-fmt: skip\r\n int:b=2;  ",
        "\t% zincite-fmt: off\r\n \r\n% zincite-fmt: on\r\n \t",
    ] {
        let parsed = parse(source);
        assert_eq!(format(&parsed).unwrap(), source);
        let ranges = protected_ranges(&parsed).unwrap();
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0], 0..source.len());
    }
    // Directive spellings inside literals and block comments are ordinary text.
    let source = "string:x=\"% zincite-fmt: off\"; /* % zincite-fmt: on */";
    assert!(protected_ranges(&parse(source)).unwrap().is_empty());
    let formatted = format(&parse(source)).unwrap();
    assert_eq!(format(&parse(formatted.clone())).unwrap(), formatted);
}

#[test]
fn invalid_directives_fail_before_any_output_and_skips_still_check_syntax() {
    for (source, marker) in [
        ("% zincite-fmt: on\nint:x=1;", "% zincite-fmt: on"),
        ("% zincite-fmt: off\nint:x=1;", "% zincite-fmt: off"),
        ("int:x=1;\n% zincite-fmt: skip\n", "% zincite-fmt: skip"),
        (
            "% zincite-fmt: off\n% zincite-fmt: off\n% zincite-fmt: on\n",
            "% zincite-fmt: off",
        ),
        (
            "% zincite-fmt: skip\n% zincite-fmt: off\nint:x=1;\n% zincite-fmt: on",
            "% zincite-fmt: off",
        ),
        (
            "% zincite-fmt: off\n% zincite-fmt: skip\nint:x=1;\n% zincite-fmt: on",
            "% zincite-fmt: skip",
        ),
        ("any:x=f(\n% zincite-fmt: skip\n1);", "% zincite-fmt: skip"),
        (
            "int:x=1; % zincite-fmt: skip\nint:y=2;",
            "% zincite-fmt: skip",
        ),
        ("% zincite-fmt: unknown\nint:x=1;", "% zincite-fmt: unknown"),
    ] {
        let parsed = parse(source);
        assert!(
            parsed.diagnostics().is_empty(),
            "{source}: {:?}",
            parsed.diagnostics()
        );
        let diagnostics = format(&parsed).unwrap_err();
        assert_eq!(diagnostics.len(), 1, "{source}: {diagnostics:?}");
        assert_eq!(&source[diagnostics[0].range.clone()], marker);
        assert!(!diagnostics[0].message.is_empty());
    }
    for source in [
        "% zincite-fmt: skip\nint:bad=;",
        "% zincite-fmt: off\nint:bad=;\n% zincite-fmt: on",
    ] {
        let parsed = parse(source);
        assert!(!parsed.diagnostics().is_empty());
        assert_eq!(format(&parsed).unwrap_err(), parsed.diagnostics());
    }
}

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
    assert!(formatted.contains(r#"first \(count + 1 :: doc_comment("hello")) nested \("inner \(extra * 2)") last \(count) end""#));
    let string = |parsed: &zincite_syntax::ParsedFile| {
        structure(
            parsed
                .tree()
                .child_nodes()
                .nth(7)
                .unwrap()
                .child_nodes()
                .nth(1)
                .unwrap(),
            parsed,
        )
    };
    assert_eq!(string(&parsed), string(&reparsed));
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn errors_produce_no_formatted_source() {
    for source in [
        "int: bad = ; int: after = 7;",
        "constraint f(1 + );",
        "annotation section(string: name); output :: section(name: \"x\") [\"x\"]; solve satisfy;",
        r#"string: bad="value \(1+[; int: hidden; 2) end"; int: after=7;"#,
        r#"string: bad="\()";"#,
        r#"string: bad="\(1"#,
    ] {
        let parsed = parse(source);
        assert_eq!(format(&parsed).unwrap_err(), parsed.diagnostics());
    }
}

#[test]
fn interpolated_labels_and_expression_layout_preserve_chunks_and_structure() {
    let source = "constraint::\"Count \\(1+2)\" true; string:s=\"value \\(if true then let {int:n=1; constraint::\"n \\(n)\" n>0;} in n+1 else 0 endif) after \\(sum(i in {1,2})(i)) done \\(1.. /* Keep  delimiter comment */)\"; string:t=\"line \\(1 % Keep  line comment\n) literal\"; any: calls=f(\"\\(let {int:n=1;} in n)\"); any: keyed=[(\"\\(1)\", 2):3];";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("constraint :: \"Count \\(1 + 2)\"\ntrue;"));
    assert!(formatted.contains("constraint :: \"n \\(n)\"\n        n > 0;"));
    assert!(formatted.contains("% Keep  line comment\n) literal\";"));
    assert!(formatted.contains("/* Keep  delimiter comment */)\";"));
    let chunks = |parsed: &zincite_syntax::ParsedFile| {
        parsed
            .tokens()
            .iter()
            .filter(|token| {
                matches!(
                    token.kind,
                    TokenKind::StringHead | TokenKind::StringMiddle | TokenKind::StringTail
                )
            })
            .map(|token| parsed.source()[token.range.clone()].to_owned())
            .collect::<Vec<_>>()
    };
    let reparsed = parse(formatted.clone());
    assert!(
        reparsed.diagnostics().is_empty(),
        "{:?}",
        reparsed.diagnostics()
    );
    assert_eq!(chunks(&parsed), chunks(&reparsed));
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
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
    assert!(formatted.contains("keyed = [3: 7, 4: 8];"));
    assert!(formatted.contains("stepped = [0: 5, 6, 7];"));
    assert!(formatted.contains("tupled = [(1, 2): 9, (1, 3): 10];"));
    assert!(formatted.contains("plain = [|\n      1, 2\n    | 3, 4\n|];"));
    assert!(formatted.contains("rows = [|\n      3: 1, 2\n    | 4: 3, 4\n|];"));
    assert!(formatted.contains("cols = [|\n      5: 6:\n    | 1, 2\n    | 3, 4\n|];"));
    assert!(formatted.contains("both = [|\n         5:   6: % Keep  column indices\n    | 3: 0x1, 2 % Keep  row comment\n    | 4: 3,   0o4\n|];"));
    assert!(formatted.contains("array[c in 1 .. 3] of var set(c) of 1 .. 5: dependent_sets;"));
    assert!(formatted.contains("array[i in 1 .. 2, j in 1 .. 3] of var 1 .. (i + j): dependent;"));
    assert!(formatted.contains(
        "array[i in 1 .. 2] of array[j in 1 .. 2] of var 1 .. (i + j): dependent_nested;"
    ));
    assert!(formatted.contains("var set(1 .. 3) of 1 .. 5: cardinality;"));
    assert!(
        formatted
            .contains("generated = {i + j | i, j in Indices where i < j, k = i + j where k > 0};")
    );
    assert!(formatted.contains(
        "filtered = [sum (j in {k | k in Indices where k <= i}) (j) | i in Indices where i > 0];"
    ));
    assert!(formatted.contains("indexed_comp = [i: i * 2 | i in Indices];"));
    assert!(formatted.contains("tuple_comp = [(i, j): i + j | i, j in Indices];"));
    assert!(formatted.contains("short_sum = sum (i in Indices) (i);"));
    assert!(
        formatted.contains("expanded_sum = sum (i in Indices) (\n    i % Keep  body comment\n);")
    );
    assert!(formatted.contains("constraint forall (i in Indices) (\n    i > 0\n);"));
    assert!(formatted.contains("constraint 'forall' (i in Indices /* Keep  binding comment */ where i > 0) (\n    /* Keep  body start */ i > 0\n);"));
    assert!(formatted.contains("long_sum = sum (\n    first_index in Indices\n        where first_index > 0,\n    second_index in Indices\n        where second_index > first_index,\n    combined_index = first_index + second_index\n        where combined_index > 0,\n) (combined_index);"));
    assert!(formatted.contains("constraint forall (\n    first_index in Indices\n        where first_index > 0,\n    second_index in Indices\n        where second_index > first_index,\n) (\n    first_index < second_index\n);"));
    let comments = |parsed: &zincite_syntax::ParsedFile| {
        parsed
            .tokens()
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment))
            .map(|token| parsed.source()[token.range.clone()].to_owned())
            .collect::<Vec<_>>()
    };
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
    assert_eq!(comments(&parsed), comments(&reparsed));
    assert_eq!(format(&reparsed).unwrap(), formatted);

    let source = "array[\n 1..2,\n 1..3\n] of int: a; any: x=f([\n1,\n2\n],{3,4}); any: y=a[\n1, % index\n2\n]; any: keyed=[(\n1, % component\n2\n):3]; any: empty=[||]; any: empty_rows=[| | |]; any: empty_cols=[|1:2:|]; any: empty_key=[():3]; any: row=[|f(\n1,\n2\n),3,|4,5,||]; array[i in 1..2, 1..3] of int: mixed;";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("array[\n    1 .. 2,\n    1 .. 3,\n] of int"));
    assert!(formatted.contains("f([\n    1,\n    2,\n], {3, 4})"));
    assert!(formatted.contains("a[\n    1, % index\n    2,\n]"));
    assert!(formatted.contains("keyed = [(\n    1, % component\n    2,\n): 3];"));
    assert!(formatted.contains("empty = [||];"));
    assert!(formatted.contains("empty_key = [(): 3];"));
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn matrix_columns_align_and_share_width_breaks_without_changing_rows() {
    use std::num::NonZeroUsize;
    use zincite_fmt::{FormatOptions, format_with_options};

    let source = include_str!("../../../tests/fixtures/collections.mzn")
        .lines()
        .skip_while(|line| !line.starts_with("any: uneven="))
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse(source);
    let default = format(&parsed).unwrap();
    assert!(
        default.contains(
            "uneven = [|\n      1,  222,                3\n    | 55, 6 /* Keep  gap */ , 777 % Keep"
        ),
        "{default}"
    );
    let narrow = FormatOptions {
        max_line_length: NonZeroUsize::new(32),
        ..FormatOptions::default()
    };
    let wrapped = format_with_options(&parsed, &narrow).unwrap();
    assert!(
        wrapped.contains(
            "uneven = [|\n      1,  222,\n      3\n    | 55, 6 /* Keep  gap */ ,\n      777 % Keep"
        ),
        "{wrapped}"
    );
    assert!(
        wrapped
            .lines()
            .filter(|line| !line.contains("% Keep"))
            .all(|line| line.chars().count() <= 32),
        "{wrapped}"
    );
    assert!(
        wrapped
            .lines()
            .any(|line| line.contains("% Keep") && line.chars().count() > 32)
    );
    let comments = |file: &zincite_syntax::ParsedFile| {
        file.tokens()
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment))
            .map(|token| file.source()[token.range.clone()].to_owned())
            .collect::<Vec<_>>()
    };
    for (formatted, options) in [(default, FormatOptions::default()), (wrapped, narrow)] {
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
        assert_eq!(comments(&parsed), comments(&reparsed));
        assert_eq!(format_with_options(&reparsed, &options).unwrap(), formatted);
    }

    let parsed = parse("any: x=[|1,2|/* lead */3,4|]; any: y=[|1:1,2|/* lead */2:3,4|];");
    for max_line_length in [NonZeroUsize::new(120), NonZeroUsize::new(24)] {
        let options = FormatOptions {
            max_line_length,
            ..FormatOptions::default()
        };
        let formatted = format_with_options(&parsed, &options).unwrap();
        assert!(
            formatted.contains("x = [|\n      1, 2\n    | /* lead */\n      3, 4\n|];"),
            "{formatted}"
        );
        assert!(
            formatted.contains("y = [|\n      1: 1, 2\n    | /* lead */\n      2: 3, 4\n|];"),
            "{formatted}"
        );
        let reparsed = parse(formatted.clone());
        assert!(reparsed.diagnostics().is_empty());
        assert_eq!(
            structure(parsed.tree(), &parsed),
            structure(reparsed.tree(), &reparsed)
        );
        assert_eq!(comments(&parsed), comments(&reparsed));
        assert_eq!(format_with_options(&reparsed, &options).unwrap(), formatted);
    }
}

#[test]
fn generator_layout_keeps_nested_expansion_and_comments_attached() {
    let source = "any: a=sum(i in [\n1,\n2\n])(f(\n1,\n2\n)); any: b=[\ni | i in {1,2}\n]; any: c=sum(\n/* Before  binding */ i, % Binding  comment\nj in {1,2} % Source  comment\nwhere i<j, % Filter  comment\nk=i+j % Final  comment\n)(/* Body  comment */ k); any: d=[first_index+second_index | first_index in VeryLongDomainNameWithRepeatedCharacters where first_index>0, second_index in VeryLongDomainNameWithRepeatedCharacters where second_index>0]; any: e=sum(i in {1})(sum(\nj in {1,2} where j>0\n)(i+j));";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("a = sum (i in [\n    1,\n    2,\n]) (f(\n    1,\n    2,\n));"));
    assert!(formatted.contains("b = [\n    i |\n    i in {1, 2},\n];"));
    assert!(formatted.contains("/* Before  binding */ i, % Binding  comment\n    j in {1, 2} % Source  comment\n        where i < j, % Filter  comment\n    k = i + j, % Final  comment\n) ( /* Body  comment */ k);"));
    assert!(formatted.contains("d = [first_index + second_index |\n    first_index in VeryLongDomainNameWithRepeatedCharacters\n        where first_index > 0,\n    second_index in VeryLongDomainNameWithRepeatedCharacters\n        where second_index > 0,\n];"));
    assert!(
        formatted.contains(
            "e = sum (i in {1}) (sum (\n    j in {1, 2}\n        where j > 0,\n) (i + j));"
        )
    );
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
    let domain = "é".repeat(65);
    let body = "i+".repeat(45) + "i";
    let source =
        format!("any: unicode=sum(i in '{domain}')(i); any: wide=sum(i in {{1}})({body});");
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains(&format!("unicode = sum (i in '{domain}') (i);")));
    assert!(formatted.contains("wide = sum (i in {1}) (\n    i +\n        i +"));
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn nested_control_layout_preserves_scope_comments_annotations_and_stability() {
    let source = include_str!("../../../tests/fixtures/control.mzn");
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
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
    assert!(formatted.contains("int: local_value = count + 1; % Keep  local spelling\n"));
    assert!(formatted.contains("constraint :: \"Local bound\"\n        local_value > 0;"));
    assert!(formatted.contains("elseif count = 0 then\n    0\nelse\n    -1\nendif)"));
    let source = "any: r=if true then 1.. elseif false then ..3 else 2..< endif; any: x=(let {int: n::tag=1, constraint::\"label\" n>0,} in (if true then % Branch  comment\n n::a else 0 endif)::b)::c;";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
    assert!(formatted.contains("then % Branch  comment\n"));
    assert!(
        format(&parse(
            "any: x=let {int: bad=; constraint true;} in 1; int: after=7;"
        ))
        .is_err()
    );
}

#[test]
fn callable_layout_preserves_signatures_comments_and_body_structure() {
    let parsed = parse(include_str!("../../../tests/fixtures/callables.mzn"));
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("function any $T: identity(\n    any $T: value, % Keep  parameter comment\n    int: count = 1,\n) :: doc_comment(\"Identity  docs\") = let"), "{formatted}");
    assert!(formatted.contains(
        "function array[$$Index] of var opt $T: opaque(array[$$Index] of var opt $T: values);"
    ));
    assert!(formatted.contains(
        "annotation marker;\nannotation note(string: text = \"Keep  literal\") = doc_comment(text);"
    ));
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
    assert_eq!(
        format(&parse("predicate bad(set(2) of int: values);"))
            .unwrap_err()
            .len(),
        1
    );
}

#[test]
fn model_item_layout_keeps_include_comments_and_annotation_attachment_stable() {
    let model = include_str!("../../../tests/fixtures/model-items.mzn");
    for mode in ["minimize", "maximize", "satisfy"] {
        let source = if mode == "satisfy" {
            model.replace("minimize (choice+0)::output_var", "satisfy")
        } else {
            model.replace("minimize", mode)
        };
        let parsed = parse(source);
        let formatted = format(&parsed).unwrap();
        assert!(formatted.contains(
            "include \"globals.mzn\";\n% Keep  include order\ninclude \"alldifferent.mzn\"; % Keep  include comment"
        ));
        assert!(formatted.contains("output :: \"result\" [\"choice = \", show(choice), \"\\n\"];"));
        assert!(formatted.contains("output :: json_section(\"details\") (choice + 1);"));
        assert!(formatted.contains("output :: (\"extra\") [\"Keep  literal\\n\"];"));
        assert!(formatted.contains(&format!("solve :: int_search([choice], input_order, indomain_min, complete) :: restart_none {mode}")));
        let reparsed = parse(formatted.clone());
        assert!(
            reparsed.diagnostics().is_empty(),
            "{:?}",
            reparsed.diagnostics()
        );
        let ordinary_items = |file: &zincite_syntax::ParsedFile| {
            file.tree()
                .child_nodes()
                .filter(|item| item.kind() != zincite_syntax::NodeKind::Include)
                .map(|item| structure(item, file))
                .collect::<Vec<_>>()
        };
        assert_eq!(ordinary_items(&parsed), ordinary_items(&reparsed));
        assert_eq!(format(&reparsed).unwrap(), formatted);
    }
    assert!(format(&parse("output :: name [\"x\"]; ")).is_err());
}

#[test]
fn enum_and_alias_layout_preserves_names_comments_and_type_structure() {
    let source = include_str!("../../../tests/fixtures/enums-aliases.mzn");
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(
        formatted
            .contains("enum Colour :: doc_comment(\"Keep  enum docs\") = {Red, 'Ocean blue'};")
    );
    assert!(formatted.contains("    None, % Keep  member comment\n    /* Next  member */ Extra,\n} ++ FromColour(Colour) ++ _(1 .. 2);"));
    assert!(formatted.contains("type ColourAlias :: doc_comment(\"Keep  alias docs\") = Colour;"));
    assert!(formatted.contains("FromColour⁻¹(entry)"));
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
    let source = "type Joined=Left++var opt Right++set of int; function Left++Right: identity(Left++Right: value)=value; enum 'Written Enum'= 'Written Constructor'(\n{1,2} % Keep  argument comment\n);";
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("type Joined = Left ++ var opt Right ++ set of int;"));
    assert!(formatted.contains("'Written Constructor'({1, 2} % Keep  argument comment\n"));
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
fn structured_layout_preserves_field_order_comments_and_access_chains() {
    let source = include_str!("../../../tests/fixtures/structured.mzn");
    let parsed = parse(source);
    let formatted = format(&parsed).unwrap();
    assert!(formatted.contains("tuple(int, int): coordinates, array[1 .. 2] of int: samples"));
    assert!(formatted.contains("nested.1.2 + entry.details.samples[1] + nested.2.coordinates.1"));
    assert!(formatted.contains("% Keep  field comment\n"));
    assert!(formatted.contains("quoted.'quoted field'"));
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format(&reparsed).unwrap(), formatted);
}

#[test]
fn model_data_pair_and_cross_family_model_keep_structure_and_stable_layout() {
    use zincite_syntax::{FileMode, SyntaxElement, parse_with_mode};
    for (source, mode) in [
        (
            include_str!("../../../tests/fixtures/data-model.mzn"),
            FileMode::Model,
        ),
        (
            include_str!("../../../tests/fixtures/data.dzn"),
            FileMode::Data,
        ),
        (
            include_str!("../../../tests/fixtures/integration.mzn"),
            FileMode::Model,
        ),
    ] {
        let parsed = parse_with_mode(source, mode);
        let formatted = format(&parsed).unwrap();
        let reparsed = parse_with_mode(formatted.clone(), mode);
        assert!(
            reparsed.diagnostics().is_empty(),
            "{:?}",
            reparsed.diagnostics()
        );
        assert_eq!(
            structure(parsed.tree(), &parsed),
            structure(reparsed.tree(), &reparsed)
        );
        let comments = |parsed: &zincite_syntax::ParsedFile| {
            parsed
                .tokens()
                .iter()
                .filter(|token| {
                    matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
                })
                .map(|token| &parsed.source()[token.range.clone()])
                .collect::<Vec<_>>()
                .join("\n")
        };
        assert_eq!(comments(&parsed), comments(&reparsed));
        fn leaves(node: &zincite_syntax::SyntaxNode, indices: &mut Vec<usize>) {
            for child in node.children() {
                match child {
                    SyntaxElement::Node(node) => leaves(node, indices),
                    SyntaxElement::Token(index) => indices.push(*index),
                }
            }
        }
        for parsed in [&parsed, &reparsed] {
            let mut indices = Vec::new();
            leaves(parsed.tree(), &mut indices);
            assert_eq!(indices, (0..parsed.tokens().len()).collect::<Vec<_>>());
            let reconstructed: String = indices
                .iter()
                .map(|index| &parsed.source()[parsed.tokens()[*index].range.clone()])
                .collect();
            assert_eq!(reconstructed, parsed.source());
        }
        assert_eq!(format(&reparsed).unwrap(), formatted);
    }
    assert!(format(&parse_with_mode("constraint true;", FileMode::Data)).is_err());
}

#[test]
fn ordinary_width_breaks_keep_structure_comments_and_explicit_lists_stable() {
    use std::num::NonZeroUsize;
    use zincite_fmt::{FormatOptions, format_with_options};

    let source = r#"any: value = first_operand + second_operand * third_operand + fourth_operand :: tag(17) :: annotation_call(first_operand, second_operand);


constraint :: "Keep  label"
(first_operand /\ second_operand /\ third_operand) :: tag(1);
any: nested = f(g(1, 2, 3), [
1, % Keep  entry comment
2
], (name: nested_call(first_operand, second_operand),));
any: local = let {constraint :: "Local  label" first_operand /\ second_operand /\ third_operand; int: x = 1;} in x + first_operand + second_operand;
any: condition = if first_operand > first_threshold /\ second_operand > second_threshold then f(first_operand, second_operand) else 0 endif;
any: comments = first_operand % Keep  operand comment
+ second_operand + third_operand;
any: short = 1 + % Keep  short comment
2;
any: groups = [1,


2];
any: call_groups = f(1,

2);
any: operand_groups = 1+

2;
any: annotation_groups = 1

::tag;
any: annotation_value_groups = 1::

tag;
enum Cases = {First, Second, Third} ++ Additional(first_operand);
any: unavoidable = f("This literal is intentionally longer than the selected width", 1); % Keep this long comment exactly as written, including  spacing
"#;
    let options = FormatOptions {
        max_line_length: NonZeroUsize::new(48),
        ..FormatOptions::default()
    };
    let parsed = parse(source);
    let formatted = format_with_options(&parsed, &options).unwrap();
    assert!(
        formatted.contains("first_operand +\n    second_operand *\n    third_operand +"),
        "{formatted}"
    );
    assert!(
        formatted.contains("constraint :: \"Local  label\"\n    first_operand"),
        "{formatted}"
    );
    assert!(
        formatted.contains("[\n        1, % Keep  entry comment\n        2,\n    ]"),
        "{formatted}"
    );
    assert!(
        formatted.contains("first_operand + % Keep  operand comment\n"),
        "{formatted}"
    );
    assert!(
        formatted.contains("short = 1 + % Keep  short comment\n    2;"),
        "{formatted}"
    );
    assert!(
        formatted.contains("groups = [\n    1,\n\n    2,\n];"),
        "{formatted}"
    );
    assert!(
        formatted.contains("call_groups = f(\n    1,\n\n    2,\n);"),
        "{formatted}"
    );
    assert!(
        formatted.contains("operand_groups = 1 +\n\n    2;"),
        "{formatted}"
    );
    assert!(
        formatted.contains("annotation_groups = 1\n\n:: tag;"),
        "{formatted}"
    );
    assert!(
        formatted.contains("annotation_value_groups = 1 ::\n\ntag;"),
        "{formatted}"
    );
    assert!(!formatted.contains("\n\n\n"));
    let reparsed = parse(formatted.clone());
    assert!(
        reparsed.diagnostics().is_empty(),
        "{:?}\n{formatted}",
        reparsed.diagnostics()
    );
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    let comments = |file: &zincite_syntax::ParsedFile| {
        file.tokens()
            .iter()
            .filter(|token| matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment))
            .map(|token| file.source()[token.range.clone()].to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(comments(&parsed), comments(&reparsed));
    assert_eq!(format_with_options(&reparsed, &options).unwrap(), formatted);
    let default = format(&parsed).unwrap();
    assert!(
        default.contains("operand_groups = 1 +\n\n    2;"),
        "{default}"
    );
    assert!(
        default.contains("annotation_groups = 1\n\n:: tag;"),
        "{default}"
    );
    assert!(
        default.contains("annotation_value_groups = 1 ::\n\ntag;"),
        "{default}"
    );
    assert_eq!(format(&parse(default.clone())).unwrap(), default);
}

#[test]
fn options_use_scalar_columns_tab_stops_and_unlimited_width() {
    use std::num::NonZeroUsize;
    use zincite_fmt::{FormatOptions, IndentStyle, format_with_options};

    let source =
        "any: x=f('ééé',2); constraint forall(i in {1})('ééé'+1); any: nested=f([\n1,\n2\n],3);";
    let options = FormatOptions {
        indent_style: IndentStyle::Tab,
        indent_size: NonZeroUsize::new(6).unwrap(),
        tab_width: NonZeroUsize::new(4).unwrap(),
        max_line_length: NonZeroUsize::new(21),
        ..FormatOptions::default()
    };
    let parsed = parse(source);
    let formatted = format_with_options(&parsed, &options).unwrap();
    assert!(formatted.contains("any: x = f('ééé', 2);"), "{formatted}");
    assert!(formatted.contains("\n\t  'ééé' + 1\n"), "{formatted}");
    assert!(
        formatted.contains("nested = f([\n\t  1,\n\t  2,\n], 3);"),
        "{formatted}"
    );
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(format_with_options(&reparsed, &options).unwrap(), formatted);

    let narrow = FormatOptions {
        max_line_length: NonZeroUsize::new(20),
        ..options
    };
    assert!(
        format_with_options(&parsed, &narrow)
            .unwrap()
            .contains("x = f(\n")
    );
    assert!(
        format_with_options(&parse("any: x=f('ééé',2)"), &narrow)
            .unwrap()
            .contains("x = f(\n")
    );
    let tab_boundary = FormatOptions {
        max_line_length: NonZeroUsize::new(14),
        ..options
    };
    let boundary_parsed = parse("constraint :: \"t\" forall(i in {1})('ééé'+1);");
    let formatted = format_with_options(&boundary_parsed, &tab_boundary).unwrap();
    assert!(
        formatted.contains("\n\t  'ééé' +\n\t\t\t1\n"),
        "{formatted}"
    );
    assert_eq!(
        format_with_options(&parse(formatted.clone()), &tab_boundary).unwrap(),
        formatted
    );
    let unlimited = FormatOptions {
        max_line_length: None,
        ..options
    };
    let formatted = format_with_options(&parsed, &unlimited).unwrap();
    assert!(formatted.contains("constraint forall (i in {1}) (\n\t  'ééé' + 1\n);"));
    assert!(formatted.contains("nested = f([\n\t  1,\n\t  2,\n], 3);"));
    assert_eq!(
        format_with_options(&parse(formatted.clone()), &unlimited).unwrap(),
        formatted
    );

    // Bodies retain punctuation reserved by their enclosing expression/item.
    let body_options = FormatOptions {
        max_line_length: NonZeroUsize::new(15),
        ..FormatOptions::default()
    };
    for source in [
        "any:x=let{int:y=1;}in f(123,456);",
        "any:x=let{int:y=1;}in f(123,456)",
        "any:x=f(\nlet{int:y=1;}in f(1,2)\n);",
    ] {
        let parsed = parse(source);
        let formatted = format_with_options(&parsed, &body_options).unwrap();
        assert!(
            formatted.lines().all(|line| line.chars().count() <= 15),
            "{formatted}"
        );
        let reparsed = parse(formatted.clone());
        let terminated = parse(format!("{};", source.trim_end_matches(';')));
        assert_eq!(
            structure(terminated.tree(), &terminated),
            structure(reparsed.tree(), &reparsed)
        );
        assert_eq!(
            format_with_options(&reparsed, &body_options).unwrap(),
            formatted
        );
    }
    let header_options = FormatOptions {
        max_line_length: NonZeroUsize::new(24),
        ..FormatOptions::default()
    };
    let parsed = parse(
        "function VeryLongReturnType:f(int:x)=x; function int:very_long_callable_name(int:x,int:y)=x+y;",
    );
    let formatted = format_with_options(&parsed, &header_options).unwrap();
    assert!(
        formatted.contains("function\n    VeryLongReturnType:"),
        "{formatted}"
    );
    assert!(
        formatted.contains("very_long_callable_name\n"),
        "{formatted}"
    );
    assert!(
        formatted
            .lines()
            .filter(|line| !line.contains("very_long_callable_name"))
            .all(|line| line.chars().count() <= 24),
        "{formatted}"
    );
    let reparsed = parse(formatted.clone());
    assert_eq!(
        structure(parsed.tree(), &parsed),
        structure(reparsed.tree(), &reparsed)
    );
    assert_eq!(
        format_with_options(&reparsed, &header_options).unwrap(),
        formatted
    );
}
