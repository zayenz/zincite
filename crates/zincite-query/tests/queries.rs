use zincite_query::{ErrorLocation, Input, ItemKind, Limits, Query, QueryResult};
use zincite_syntax::{FileMode, SyntaxElement, SyntaxNode};

fn query<'a>(input: &'a Input, expression: &str) -> QueryResult<'a> {
    Query::parse(expression, Limits::default())
        .unwrap()
        .evaluate(input, Limits::default())
        .unwrap()
}

fn token_indices(node: &SyntaxNode, indices: &mut Vec<usize>) {
    for child in node.children() {
        match child {
            SyntaxElement::Node(node) => token_indices(node, indices),
            SyntaxElement::Token(index) => indices.push(*index),
        }
    }
}

#[test]
fn model_selection_exposes_original_nodes_tokens_and_attached_comment_bytes() {
    let bytes = b"\xef\xbb\xbf% Section\r\n\r\n% attached \xff\r\nint: 'capacity' = 2; % trailing\r\n\r\nconstraint true;\r\nsolve satisfy;\r\n";
    let input = Input::parse(bytes.to_vec(), FileMode::Model).unwrap();
    for expression in ["", "items", "items | emit"] {
        let result = query(&input, expression);
        assert_eq!(result.render(), bytes);
        let QueryResult::Selection(selection) = result else {
            panic!()
        };
        assert!(selection.is_document());
        assert_eq!(selection.items().len(), 3);
        let mut indices = Vec::new();
        token_indices(selection.input().syntax().tree(), &mut indices);
        let reconstructed: Vec<_> = indices
            .iter()
            .flat_map(|&index| input.token_bytes(index))
            .copied()
            .collect();
        assert_eq!(reconstructed, bytes[3..]);
    }
    let result = query(&input, "filter(name(\"capacity\"))");
    assert_eq!(
        result.render(),
        b"% attached \xff\r\nint: 'capacity' = 2; % trailing\r\n"
    );
    let QueryResult::Selection(selection) = result else {
        panic!()
    };
    assert!(!selection.is_document());
    let item = &selection.items()[0];
    assert_eq!(item.kind, ItemKind::Declaration);
    assert_eq!(item.name, Some("capacity"));
    assert_eq!(
        &input.source_bytes()[item.range.clone()],
        b"int: 'capacity' = 2;"
    );
    assert_eq!(input.line_column(item.range.start), (4, 1));
    let mut indices = Vec::new();
    token_indices(item.node, &mut indices);
    let reconstructed: Vec<_> = indices
        .iter()
        .flat_map(|&index| input.token_bytes(index))
        .copied()
        .collect();
    assert_eq!(reconstructed, &input.source_bytes()[item.range.clone()]);
    for index in indices {
        assert_eq!(
            input.token_bytes(index),
            &input.source_bytes()[input.token_range(index)]
        );
    }
    assert_eq!(
        query(&input, "filter(name(\"'capacity'\"))").render(),
        selection
            .items()
            .iter()
            .flat_map(|item| &input.source_bytes()[item.source_range.clone()])
            .copied()
            .collect::<Vec<_>>()
    );
    assert!(
        !query(&input, "head(3)")
            .render()
            .starts_with(b"\xef\xbb\xbf% Section")
    );
}

#[test]
fn data_order_boolean_precedence_head_count_and_empty_selections() {
    let input = Input::parse(
        b"z = 3;\n% belongs to capacity\n'capacity' = 2;\na = 1;\n".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = query(
        &input,
        "items | filter(not name(\"a\") and kind(\"assignment\") or name(\"a\")) | head(2)",
    );
    assert_eq!(
        result.render(),
        b"z = 3;\n% belongs to capacity\n'capacity' = 2;\n"
    );
    let QueryResult::Selection(selection) = result else {
        panic!()
    };
    assert_eq!(
        selection
            .items()
            .iter()
            .map(|item| item.name.unwrap())
            .collect::<Vec<_>>(),
        ["z", "capacity"]
    );
    assert_eq!(
        query(
            &input,
            "filter(name(\"z\") or name(\"a\") and name(\"absent\")) | count"
        )
        .render(),
        b"1\n"
    );
    assert_eq!(
        query(
            &input,
            "filter((name(\"z\") or name(\"a\")) and not name(\"absent\")) | count"
        )
        .render(),
        b"2\n"
    );
    assert_eq!(
        query(&input, "filter(name(\"absent\")) | count").render(),
        b"0\n"
    );
    assert!(query(&input, "head(0)").render().is_empty());
    assert_eq!(
        query(&input, "head(0) | items").render(),
        input.source_bytes()
    );
    assert!(
        Input::parse(b"constraint true;".to_vec(), FileMode::Data)
            .unwrap_err()
            .diagnostics[0]
            .message
            .contains("only assignments")
    );
}

#[test]
fn query_errors_and_limits_are_located_without_silent_truncation() {
    for expression in [
        "items |",
        "count | emit",
        "emit | count",
        "filter(kind(\"missing\"))",
        "filter(name(\"bad\\x\"))",
        "head(-1)",
    ] {
        let error = Query::parse(expression, Limits::default()).unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert!(error.range.end <= expression.len());
    }
    let nested = format!("filter({}name(\"x\"){})", "(".repeat(65), ")".repeat(65));
    assert!(
        Query::parse(&nested, Limits::default())
            .unwrap_err()
            .message
            .contains("nesting limit")
    );
    let flat = format!(
        "filter({}) | count",
        vec!["name(\"missing\")"; 1000].join(" or ")
    );
    let input = Input::parse(b"x = 1; y = 2;".to_vec(), FileMode::Data).unwrap();
    assert_eq!(query(&input, &flat).render(), b"0\n");
    let parsed = Query::parse(&flat, Limits::default()).unwrap();
    let error = parsed
        .evaluate(
            &input,
            Limits {
                work: 20,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work limit"));
    assert!(error.range.start > 0);
    let error = Query::parse("head(1)", Limits::default())
        .unwrap()
        .evaluate(
            &input,
            Limits {
                collection: 1,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("collection limit"));
    let error = Input::parse(b"\xef\xbb\xbfint: bad = ;".to_vec(), FileMode::Model).unwrap_err();
    assert!(
        error
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.range.start >= 3)
    );
}

#[test]
fn fragments_keep_next_item_directives_and_reject_unbalanced_formatter_pairs() {
    let source = b"% zincite-fmt: skip\n\nx = 1;\ny = 2;\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    assert_eq!(query(&input, "filter(name(\"y\"))").render(), b"y = 2;\n");
    assert_eq!(
        query(&input, "filter(name(\"x\"))").render(),
        b"% zincite-fmt: skip\n\nx = 1;\n"
    );
    let source = b"% zincite-fmt: off\nx = 1;\ny = 2;\n% zincite-fmt: on\nz = 3;\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    assert_eq!(
        query(&input, "filter(not name(\"z\"))").render(),
        &source[..source.len() - 7]
    );
    assert_eq!(query(&input, "filter(name(\"z\"))").render(), b"z = 3;\n");
    let error = Query::parse("head(1)", Limits::default())
        .unwrap()
        .evaluate(&input, Limits::default())
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert_eq!(error.range, 0..18);
    assert!(error.message.contains("unbalance"));
    assert_eq!(query(&input, "head(1) | count").render(), b"1\n");
}

#[test]
fn assignment_edits_emit_complete_documents_and_preserve_surviving_bytes() {
    let source = b"\xef\xbb\xbf% Section\r\n\r\n% attached \xff\r\n'capacity' = /* before \xff */ [1, 2] /* after */; % tail\r\n\r\n% zincite-lint: ignore made-up-rule\r\n\r\nused = 1;\r\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    let expression = "filter(name(\"capacity\")) | set_value(\"[3, 4]\")";
    let expected = b"\xef\xbb\xbf% Section\r\n\r\n% attached \xff\r\n'capacity' = /* before \xff */ [3, 4] /* after */; % tail\r\n\r\n% zincite-lint: ignore made-up-rule\r\n\r\nused = 1;\r\n";
    for expression in [
        expression.to_owned(),
        format!("{expression} | emit_document"),
    ] {
        let result = query(&input, &expression);
        assert!(matches!(result, QueryResult::Document(_)));
        assert_eq!(result.render(), expected);
    }
    assert_eq!(
        query(&input, "filter(name(\"used\")) | remove").render(),
        b"\xef\xbb\xbf% Section\r\n\r\n% attached \xff\r\n'capacity' = /* before \xff */ [1, 2] /* after */; % tail\r\n\r\n"
    );
    assert_eq!(
        query(&input, "filter(name(\"capacity\")) | remove").render(),
        b"\xef\xbb\xbf% Section\r\n\r\n\r\n% zincite-lint: ignore made-up-rule\r\n\r\nused = 1;\r\n"
    );
    assert_eq!(query(&input, "head(0) | remove").render(), source);
    assert_eq!(input.source_bytes(), source);
}

#[test]
fn removals_keep_next_item_targets_and_require_balanced_formatter_regions() {
    let source = b"% zincite-fmt: skip\n\nx = 1;\ny = 2;\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    assert_eq!(query(&input, "head(1) | remove").render(), b"y = 2;\n");
    let section = Input::parse(
        b"% zincite-fmt: skip\n\n% Section\n\nx = 1;\ny = 2;\n".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&section, "head(1) | remove").render(),
        b"% Section\n\ny = 2;\n"
    );
    let source = b"% zincite-fmt: off\nx = 1;\ny = 2;\n% zincite-fmt: on\nz = 3;\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    assert_eq!(query(&input, "head(2) | remove").render(), b"z = 3;\n");
    let error = Query::parse("head(1) | remove", Limits::default())
        .unwrap()
        .evaluate(&input, Limits::default())
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert!(error.message.contains("unbalance"));
    assert_eq!(&input.source_bytes()[error.range], b"% zincite-fmt: on");
}

#[test]
fn edits_refuse_invalid_replacements_internal_comments_and_stale_stage_evaluation() {
    for expression in [
        "remove | count",
        "remove | filter(name(\"x\"))",
        "set_value(\"1\") | remove",
        "set_value(\"1\") | emit",
        "emit_document",
    ] {
        assert!(Query::parse(expression, Limits::default()).is_err());
    }
    let input = Input::parse(b"x = 1; y = 2;".to_vec(), FileMode::Data).unwrap();
    for value in ["", "[1,", "1; injected = 2"] {
        let expression = format!("head(0) | set_value(\"{value}\")");
        let error = Query::parse(&expression, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert!(error.message.contains("replacement"));
    }
    // A final item can parse without a separator; replacement text must still
    // leave the original separator and same-line surviving assignment visible.
    let error = Query::parse("head(1) | set_value(\"3 % comment\")", Limits::default())
        .unwrap()
        .evaluate(&input, Limits::default())
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Query);
    assert!(error.message.contains("assignment separator"));
    let model = Input::parse(b"x = 1;".to_vec(), FileMode::Model).unwrap();
    assert!(
        Query::parse("remove", Limits::default())
            .unwrap()
            .evaluate(&model, Limits::default())
            .unwrap_err()
            .message
            .contains("data-file mode")
    );
    let comments = Input::parse(b"x = [1, % internal\n2];".to_vec(), FileMode::Data).unwrap();
    let error = Query::parse("set_value(\"[3]\")", Limits::default())
        .unwrap()
        .evaluate(&comments, Limits::default())
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert_eq!(&comments.source_bytes()[error.range], b"% internal");
    assert!(error.message.contains("placement"));
    let error = Query::parse("set_value(\"1234567890\")", Limits::default())
        .unwrap()
        .evaluate(
            &input,
            Limits {
                work: 20,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work limit"));
}

#[test]
fn nested_constraint_and_solve_inspection_keep_occurrences_and_original_ranges() {
    let source = b"\xef\xbb\xbfconstraint f(g(1), g(2));\nsolve :: int_search([x], input_order, indomain_min) satisfy;\n";
    let input = Input::parse_named(source.to_vec(), FileMode::Model, "nested.mzn").unwrap();
    let calls = query(
        &input,
        "filter(kind(\"constraint\")) | expressions | call_names",
    );
    let QueryResult::Strings(names) = calls else {
        panic!()
    };
    assert_eq!(names, ["f", "g", "g"]);
    assert_eq!(
        query(&input, "expressions | call_names | tally | json").render(),
        b"{\"f\":1,\"g\":2,\"int_search\":1}\n"
    );
    assert_eq!(
        query(
            &input,
            "filter(kind(\"solve\")) | subtree | annotation_names | json"
        )
        .render(),
        b"[\"int_search\"]\n"
    );
    let roots = "filter(kind(\"constraint\")) | expressions | filter(kind(\"call_expression\"))";
    let QueryResult::Nodes(nodes) = query(&input, roots) else {
        panic!()
    };
    assert_eq!(nodes.nodes().len(), 3);
    assert!(nodes.nodes().iter().all(|node| node.file == "nested.mzn"));
    for node in nodes.nodes() {
        assert!(node.range.start >= 3);
        assert_eq!(node.range.start, node.node.range().start + 3);
    }
    let overlapping = format!("{roots} | subtree");
    assert_eq!(
        query(&input, &format!("{overlapping} | call_names | tally")).render(),
        b"{\"f\":1,\"g\":4}\n"
    );
    assert_eq!(
        query(
            &input,
            &format!("{overlapping} | unique | call_names | tally")
        )
        .render(),
        b"{\"f\":1,\"g\":2}\n"
    );
    assert_eq!(
        query(
            &input,
            "filter(kind(\"constraint\")) | children | count | json"
        )
        .render(),
        b"1\n"
    );
    let selected = &nodes.nodes()[1];
    let range_query = format!(
        "{roots} | range({},{}) | text | json",
        selected.range.start, selected.range.end
    );
    assert_eq!(query(&input, &range_query).render(), b"[\"g(1)\"]\n");
    assert_eq!(
        query(&input, "expressions | range(0,0) | json").render(),
        b"[]\n"
    );
    assert_eq!(
        query(&input, "expressions | call_names | unique | json").render(),
        b"[\"f\",\"g\",\"int_search\"]\n"
    );
}

#[test]
fn json_inspects_exact_text_names_and_file_identity() {
    let source = "'capacity' = \"quote: \\\" and slash: \\\\\";\r\n";
    let input =
        Input::parse_named(source.as_bytes().to_vec(), FileMode::Data, "a\"\\b.dzn").unwrap();
    let result = query(&input, "json");
    let QueryResult::Json(json_result) = &result else {
        panic!()
    };
    assert!(matches!(json_result.result(), QueryResult::Selection(_)));
    let value: serde_json::Value = serde_json::from_slice(&result.render()).unwrap();
    assert_eq!(value[0]["kind"], "assignment");
    assert_eq!(value[0]["names"], serde_json::json!(["capacity"]));
    assert_eq!(value[0]["file"], "a\"\\b.dzn");
    let start = value[0]["range"]["start"].as_u64().unwrap() as usize;
    let end = value[0]["range"]["end"].as_u64().unwrap() as usize;
    assert_eq!(value[0]["text"], &source[start..end]);
    assert_eq!(query(&input, "names | json").render(), b"[\"capacity\"]\n");
    assert_eq!(
        query(&input, "head(0) | names | tally | json").render(),
        b"{}\n"
    );
}

#[test]
fn inspection_errors_and_expansion_limits_are_located() {
    let input = Input::parse(
        "constraint f('é', g(1));".as_bytes().to_vec(),
        FileMode::Model,
    )
    .unwrap();
    for expression in [
        "range(4,2)",
        "range(-1,2)",
        "range(0,99999999999999999999999999)",
        "count | subtree",
        "remove | json",
    ] {
        assert!(
            Query::parse(expression, Limits::default()).is_err(),
            "{expression}"
        );
    }
    for expression in [
        "range(0,999)",
        "expressions | range(15,16)",
        "names | children",
        "names | filter(name(\"f\"))",
        "tally",
        "expressions | remove",
    ] {
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query, "{expression}");
        assert!(error.range.end <= expression.len());
    }
    let expansion = Query::parse("subtree | subtree", Limits::default()).unwrap();
    for limits in [
        Limits {
            work: 5,
            ..Limits::default()
        },
        Limits {
            collection: 3,
            ..Limits::default()
        },
    ] {
        assert!(
            expansion
                .evaluate(&input, limits)
                .unwrap_err()
                .message
                .contains("limit")
        );
    }
    let opaque = Input::parse(b"x = [1, /* \xff */ 2];".to_vec(), FileMode::Data).unwrap();
    assert_eq!(query(&opaque, "emit").render(), opaque.source_bytes());
    for expression in ["text", "json"] {
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&opaque, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.message.contains("UTF-8"));
    }
}

#[test]
fn literal_views_keep_key_field_and_nested_value_identity() {
    use zincite_query::{ArrayIndexing, LiteralKind};
    let source =
        b"\xef\xbb\xbfr = ['A': ('label': \"A\", member: A, nested: ([1, 2], {A, B}))];\r\n";
    let input = Input::parse_named(source.to_vec(), FileMode::Data, "records.dzn").unwrap();
    let QueryResult::Literals(selection) = query(&input, "values") else {
        panic!()
    };
    let value = &selection.values()[0];
    assert_eq!(value.file, "records.dzn");
    assert_eq!(value.spelling, &source[value.range.clone()]);
    let LiteralKind::Array { indexing, entries } = &value.kind else {
        panic!()
    };
    assert_eq!(*indexing, ArrayIndexing::Explicit);
    let key = entries[0].key.as_ref().unwrap();
    assert!(matches!(key.kind, LiteralKind::Member("A")));
    assert_eq!(key.spelling, b"'A'");
    let LiteralKind::Record(fields) = &entries[0].value.kind else {
        panic!()
    };
    assert_eq!(fields[0].label.name, "label");
    assert_eq!(fields[0].label.spelling, b"'label'");
    assert_eq!(&source[fields[0].label.range.clone()], b"'label'");
    assert!(matches!(fields[0].value.kind, LiteralKind::String));
    assert!(matches!(fields[1].value.kind, LiteralKind::Member("A")));
    let LiteralKind::Tuple(nested) = &fields[2].value.kind else {
        panic!()
    };
    assert!(matches!(nested[0].kind, LiteralKind::Array { .. }));
    assert!(matches!(nested[1].kind, LiteralKind::Set(_)));
    let json: serde_json::Value =
        serde_json::from_slice(&query(&input, "values | json").render()).unwrap();
    assert_eq!(json[0]["entries"][0]["key"]["name"], "A");
    assert_eq!(
        json[0]["entries"][0]["value"]["fields"][0]["label"]["name"],
        "label"
    );
    for (expression, expected) in [
        ("keys | names | json", b"[\"A\"]\n".as_slice()),
        (
            "elements | fields | names | json",
            b"[\"label\",\"member\",\"nested\"]\n".as_slice(),
        ),
        (
            "elements | fields | head(1) | values | json",
            b"".as_slice(),
        ),
    ] {
        let result = query(&input, expression);
        if expected.is_empty() {
            let json: serde_json::Value = serde_json::from_slice(&result.render()).unwrap();
            assert_eq!(json[0]["kind"], "string");
            assert_eq!(json[0]["text"], "\"A\"");
        } else {
            assert_eq!(result.render(), expected);
        }
    }
    assert_eq!(query(&input, "values | count | json").render(), b"1\n");
    assert!(query(&input, "values | head(0) | emit").render().is_empty());
}

#[test]
fn explicit_element_filters_preserve_scalar_order_and_checked_numbers() {
    let input = Input::parse(
        b"x = [0, -2, +0x10, (3), 0o2]; keep = true;".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&input, "head(1) | filter_elements(gt(0))").render(),
        b"x = [  +0x10, (3), 0o2]; keep = true;"
    );
    assert_eq!(
        query(&input, "head(1) | filter_elements(eq(2))").render(),
        b"x = [    0o2]; keep = true;"
    );
    let input = Input::parse(
        b"x = [-9223372036854775808, 9223372036854775807];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&input, "filter_elements(eq(-9223372036854775808))").render(),
        b"x = [-9223372036854775808 ];"
    );
    assert_eq!(
        query(&input, "filter_elements(eq(9223372036854775807))").render(),
        b"x = [ 9223372036854775807];"
    );
    for source in ["x = [1, 2];", "x = {1, 2};", "x = [];", "x = {};"] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        let candidate = query(&input, "filter_elements(lt(0))").render();
        let candidate = Input::parse(candidate, FileMode::Data).unwrap();
        assert_eq!(query(&candidate, "elements | count").render(), b"0\n");
    }
    for (source, expression, expected) in [
        (
            "x = [true, false, true];",
            "filter_elements(eq(true))",
            "x = [true,  true];",
        ),
        (
            "x = [\"a\\n\", \"b\"];",
            "filter_elements(eq(\"a\\n\"))",
            "x = [\"a\\n\" ];",
        ),
        (
            "x = [A, 'B', A];",
            "filter_elements(eq(member(\"A\")))",
            "x = [A,  A];",
        ),
        (
            "x = [-9223372036854775808];",
            "filter_elements(eq(-9.223372036854776e18))",
            "x = [-9223372036854775808];",
        ),
    ] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        assert_eq!(query(&input, expression).render(), expected.as_bytes());
    }
}

#[test]
fn collection_edits_keep_written_keys_comments_sections_and_rectangular_axes() {
    let source = b"\xef\xbb\xbfx = [\r\n    % keep A \xff\r\n    'A': 1, % tail A\r\n    % remove B\r\n    B: -1, % tail B\r\n\r\n    % Section\r\n\r\n    C: 2 % tail C\r\n];\r\ny = 4;\r\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    let expected = b"\xef\xbb\xbfx = [\r\n    % keep A \xff\r\n    'A': 1, % tail A\r\n\r\n    % Section\r\n\r\n    C: 2 % tail C\r\n];\r\ny = 4;\r\n";
    assert_eq!(
        query(&input, "head(1) | filter_elements(gt(0))").render(),
        expected
    );
    assert_eq!(input.source_bytes(), source);
    let matrix = Input::parse(
        b"m = [| A: B: C: | R: 1, -1, 2 | S: 3, -2, 4 |];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&matrix, "filter_elements(gt(0))").render(),
        b"m = [| A:  C: | R: 1,  2 | S: 3,  4 |];"
    );
    assert_eq!(
        query(&matrix, "keys | names | json").render(),
        b"[\"A\",\"B\",\"C\",\"R\",\"S\"]\n"
    );
    assert_eq!(query(&matrix, "elements | count").render(), b"6\n");
    let comments = Input::parse(
        b"m = [| A: B: | R:\n% removed cell\n-1, 2 | S: -2, 3 |];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&comments, "filter_elements(gt(0))").render(),
        b"m = [|  B: | R:\n 2 | S:  3 |];"
    );
    for (source, message) in [
        ("x = [A: 1, -1, 2];", "starting-key"),
        (
            "x = [(A, C): 1, (A, D): -1, (B, C): 2, (B, D): 3];",
            "rectangular",
        ),
        ("x = [| 1, -1 | -1, 1 |];", "same surviving columns"),
        ("x = [| 1, 2 | 3 |];", "rectangular"),
        ("x = [A: 1, A: 2];", "duplicate"),
        ("x = [1.0: 1, 2];", "keys require integer"),
    ] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        let error = Query::parse("filter_elements(gt(0))", Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.message.contains(message), "{}", error.message);
    }
    let nested = Input::parse(b"x = [[1, -1], [2]];".to_vec(), FileMode::Data).unwrap();
    let filter_nested = "elements | head(1) | filter_elements(gt(0))";
    assert_eq!(query(&nested, filter_nested).render(), b"x = [[1 ], [2]];");
    for source in ["x = [A: [1, -1], A: [2]];", "x = [| [1, -1], [2] | [3] |];"] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        let error = Query::parse(filter_nested, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.message.contains("duplicate") || error.message.contains("rectangular"));
    }
    let input = Input::parse(
        b"x = [(A, C): 1, (A, D): -1, (B, C): 2, (B, D): -2];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    assert_eq!(
        query(&input, "filter_elements(gt(0))").render(),
        b"x = [(A, C): 1,  (B, C): 2 ];"
    );
}

#[test]
fn literal_failures_are_located_and_never_return_partial_candidates() {
    for (source, expression, message) in [
        ("x = [1]; y = [1+2];", "filter_elements(gt(0))", "computed"),
        ("x = [A];", "filter_elements(eq(\"A\"))", "incompatible"),
        (
            "x = [\"A\"];",
            "filter_elements(eq(member(\"A\")))",
            "incompatible",
        ),
        (
            "x = [9223372036854775808];",
            "filter_elements(gt(0))",
            "signed 64-bit",
        ),
        (
            "x = [9223372036854775807];",
            "filter_elements(gt(0.0))",
            "lose integer precision",
        ),
        ("x = [1e999];", "filter_elements(gt(0))", "overflows"),
        ("x = [1e-999];", "filter_elements(gt(0))", "underflows"),
        (
            "x = [0x1p0];",
            "filter_elements(gt(0))",
            "hexadecimal-float",
        ),
        ("x = f(1);", "values", "computed"),
    ] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(!error.range.is_empty());
        assert!(error.range.end <= source.len());
        assert!(error.message.contains(message), "{}", error.message);
        assert_eq!(input.source_bytes(), source.as_bytes());
    }
    for expression in [
        "head(0) | filter_elements(gt(9223372036854775808))",
        "filter_elements(gt(true))",
        "filter_elements(eq(1e999))",
        "filter_elements(eq(1e-999))",
        "filter_elements(gt(0)) | values",
        "filter_elements(eq(1.))",
        "filter_elements(eq(.1))",
    ] {
        assert_eq!(
            Query::parse(expression, Limits::default())
                .unwrap_err()
                .location,
            ErrorLocation::Query
        );
    }
    let input = Input::parse(b"x = [[1], [2]];".to_vec(), FileMode::Data).unwrap();
    for (expression, limits) in [
        (
            "values",
            Limits {
                collection: 2,
                ..Limits::default()
            },
        ),
        (
            "values",
            Limits {
                nesting: 1,
                ..Limits::default()
            },
        ),
        (
            "values",
            Limits {
                work: 4,
                ..Limits::default()
            },
        ),
        ("values | elements", Limits::default()),
        ("names | filter_elements(gt(0))", Limits::default()),
    ] {
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&input, limits)
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
    }
    let numeric = Input::parse(b"x = [1, -1];".to_vec(), FileMode::Data).unwrap();
    let duplicate = "subtree | subtree | filter(kind(\"array_literal\")) | filter_elements(gt(0))";
    assert!(
        Query::parse(duplicate, Limits::default())
            .unwrap()
            .evaluate(&numeric, Limits::default())
            .unwrap_err()
            .message
            .contains("overlap")
    );
    assert_eq!(
        query(
            &numeric,
            &duplicate.replace("| filter_elements", "| unique | filter_elements")
        )
        .render(),
        b"x = [1 ];"
    );
}

fn reduction_model(tag: &str, source: &str) -> zincite_lint::ModelContext {
    let path = std::env::temp_dir().join(format!(
        "zincite-reduction-{tag}-{}.mzn",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let model = zincite_lint::load_model(&path, &zincite_lint::ModelOptions::default());
    std::fs::remove_file(path).unwrap();
    assert!(model.errors.is_empty(), "{:?}", model.errors);
    model
}

fn reduce(
    input: &Input,
    model: Option<&zincite_lint::ModelContext>,
    expression: &str,
) -> zincite_query::ReductionResult {
    let result = Query::parse(expression, Limits::default())
        .unwrap()
        .evaluate_with_model(input, model, Limits::default())
        .unwrap();
    let QueryResult::Reduction(result) = result else {
        panic!("expected reduction")
    };
    result
}

#[test]
fn enum_reduction_keeps_identity_order_comments_and_whole_keyed_records() {
    let source = b"\xef\xbb\xbfGuests = ({'A', B, C});\r\nguests = [\r\n% keep \xff\r\n'A': (name: \"B\", B: 10),\r\n% removed record\r\nB: (name: \"gone\", B: B),\r\nC: (name: \"C\", B: 30)\r\n];\r\n";
    let input = Input::parse_named(source.to_vec(), FileMode::Data, "guests.dzn").unwrap();
    let result = reduce(
        &input,
        None,
        "head(0) | reduce_enum(\"Guests\", keep(\"C\", \"A\"))",
    );
    assert!(result.is_complete(), "{:?}", result.unresolved());
    assert_eq!(result.candidate().source_bytes(), b"\xef\xbb\xbfGuests = ({'A',  C});\r\nguests = [\r\n% keep \xff\r\n'A': (name: \"B\", B: 10),\r\n\r\nC: (name: \"C\", B: 30)\r\n];\r\n");
    assert_eq!(result.candidate().file(), "guests.dzn");
    let prefix = reduce(&input, None, "reduce_enum(\"Guests\", keep_first(1))");
    assert!(prefix.is_complete());
    assert!(
        !prefix
            .candidate()
            .source_bytes()
            .windows(5)
            .any(|bytes| bytes == b"C: (n")
    );
    for expression in [
        "reduce_enum(\"Guests\", keep())",
        "reduce_enum(\"Guests\", keep_first(0))",
    ] {
        let empty = reduce(&input, None, expression);
        assert!(empty.is_complete());
        assert_eq!(
            query(
                empty.candidate(),
                "filter(name(\"Guests\")) | elements | count"
            )
            .render(),
            b"0\n"
        );
        assert_eq!(
            query(
                empty.candidate(),
                "filter(name(\"guests\")) | elements | count"
            )
            .render(),
            b"0\n"
        );
    }
    assert_eq!(input.source_bytes(), source);
    for (source, expression, expected) in [
        (
            "Guests = {A, B};",
            "reduce_enum(\"Guests\", keep(\"C\"))",
            "unknown requested",
        ),
        (
            "Guests = anon_enum(3);",
            "reduce_enum(\"Guests\", keep_first(1))",
            "constructed",
        ),
        (
            "Guests = {A, A};",
            "reduce_enum(\"Guests\", keep_first(1))",
            "unique",
        ),
    ] {
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Data).unwrap();
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert!(error.message.contains(expected), "{error:?}");
    }
}

#[test]
fn enum_reduction_slices_positional_nested_and_rectangular_axes_by_model_identity() {
    let model = reduction_model(
        "axes",
        "enum Guests; enum Topics = {T, U};\ntype Info = record(array[Guests] of int: weights, string: name);\narray[Guests] of Info: guests; array[Guests] of int: scores;\narray[Guests, Guests] of int: matrix; array[Guests, Topics] of int: keyed; array[Topics, Guests] of int: by_topic; int: unchanged;\nsolve satisfy;\n",
    );
    let input = Input::parse(b"Guests = {A, B, C};\nguests = [(name: \"A\", weights: [11,12,13]), (name: \"B\", weights: [21,22,23]), (name: \"C\", weights: [31,32,33])];\nscores = [10,20,30];\nmatrix = [|1,2,3|4,5,6|7,8,9|];\nkeyed = [(C,U): 32, (A,T): 11, (B,T): 21, (C,T): 31, (A,U): 12, (B,U): 22];\nby_topic = [| C: A: B: | U: 3,1,2 | T: 6,4,5 |];\nunchanged = 20;\n".to_vec(), FileMode::Data).unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"C\", \"A\"))",
    );
    assert!(result.is_complete(), "{:?}", result.unresolved());
    let candidate = result.candidate();
    let source = String::from_utf8(candidate.source_bytes().to_vec()).unwrap();
    assert!(source.contains("scores = [10,30]"), "{source}");
    assert!(
        source.contains("weights: [11,13]") && source.contains("weights: [31,33]"),
        "{source}"
    );
    assert!(!source.contains("name: \"B\"") && !source.contains("21,22,23"));
    assert!(source.contains("matrix = [|1,3|7,9|]"), "{source}");
    assert!(
        source.contains("(C,U): 32, (A,T): 11,  (C,T): 31, (A,U): 12"),
        "{source}"
    );
    assert!(
        source.contains("by_topic = [| C: A:  | U: 3,1 | T: 6,4 |]"),
        "{source}"
    );
    assert!(source.contains("unchanged = 20;"));
    assert_eq!(
        query(
            candidate,
            "filter(name(\"keyed\")) | keys | subtree | names | json"
        )
        .render(),
        b"[\"C\",\"U\",\"A\",\"T\",\"C\",\"T\",\"A\",\"U\"]\n"
    );
}

#[test]
fn enum_reduction_exposes_remaining_dependencies_without_losing_independent_edits() {
    let model = reduction_model(
        "incomplete",
        "enum Guests; set of Guests: Subset;\ntype Info = record(array[Guests] of int: weights, Guests: computed);\nInfo: info; Guests: chosen; set of Guests: group;\narray[Subset] of int: ambiguous; array[int] of int: other;\nconstraint chosen != B; solve satisfy;\n",
    );
    let source = b"Guests = {A, B, C};\ninfo = (computed: other[B], weights: [1,2,3]);\nchosen = B; group = {A, B}; Subset = {A,C}; ambiguous = [10,20]; other = [9,8];\n";
    let input = Input::parse_named(source.to_vec(), FileMode::Data, "remaining.dzn").unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(!result.is_complete());
    let text = String::from_utf8(result.candidate().source_bytes().to_vec()).unwrap();
    assert!(
        text.contains("weights: [1,3]") && text.contains("computed: other[B]"),
        "{text}"
    );
    assert!(text.contains("chosen = B; group = {A }") && text.contains("ambiguous = [10,20]"));
    assert!(
        result
            .unresolved()
            .iter()
            .any(|dependency| dependency.message.contains("positional enum alignment"))
    );
    assert!(
        result
            .unresolved()
            .iter()
            .any(|dependency| dependency.location.path == model.root
                && dependency.message.contains("read-only model"))
    );
    let computed = result
        .unresolved()
        .iter()
        .find(|dependency| dependency.message.contains("computed enum"))
        .unwrap();
    assert_eq!(&source[computed.location.range.clone()], b"other[B]");
    assert_eq!(
        computed.location.path,
        std::path::Path::new("remaining.dzn")
    );
    let unknown = Input::parse(
        b"Guests = {A,B}; chosen = keyed[2]; values = [1,2]; keyed = [A: 10, B: 20];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = reduce(&unknown, None, "reduce_enum(\"Guests\", keep_first(1))");
    assert!(!result.is_complete());
    assert!(
        result
            .unresolved()
            .iter()
            .any(|dependency| dependency.message.contains("alignment"))
    );
    assert!(
        result
            .unresolved()
            .iter()
            .any(|dependency| dependency.message.contains("computed enum"))
    );
    let empty_axes = Input::parse(
        b"Guests = {A,B}; matrix = [| A: B: | A: 1,2 | B: 3,4 |];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = reduce(&empty_axes, None, "reduce_enum(\"Guests\", keep())");
    assert!(!result.is_complete());
    assert!(
        String::from_utf8_lossy(result.candidate().source_bytes())
            .contains("matrix = [| A: B: | A: 1,2 | B: 3,4 |]")
    );
    let model = reduction_model(
        "ambiguous",
        "enum Guests; array[Guests] of int: values; array[int] of int: values; set of Guests: group; solve satisfy;",
    );
    let input = Input::parse(
        b"Guests = {A,B,C}; values = [A: 1, B: 2, C: 3]; group = {A,C};".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(!result.is_complete());
    assert_eq!(result.unresolved().len(), 1);
    assert!(result.unresolved()[0].message.contains("ambiguous"));
    assert!(
        String::from_utf8_lossy(result.candidate().source_bytes())
            .contains("values = [A: 1,  C: 3]; group = {A,C};")
    );
}

#[test]
fn enum_reduction_prunes_nested_sets_and_repairs_original_group_indices() {
    let model = reduction_model(
        "groups",
        "enum Guests; enum Others = {X,Y};\ntype Info = record(set of Guests: friends, array[int] of set of Guests: groups, int: B, string: label);\nInfo: info; array[Guests] of Info: guests; set of Guests: before; set of Guests: after; set of Guests: nested; set of Guests: unshifted; tuple(array[int] of set of Guests, int): bundle; set of Guests: from_tuple; set of Others: other; int: unchanged;\nconstraint forall(g in Guests)(guests[g].B >= 0);\nint: local = let { array[1..4] of int: info = [1,2,3,4]; } in info[4];\nsolve satisfy;\n",
    );
    let source = b"Guests = {'A',B,C};\nbefore = info.groups[4];\ninfo = (friends: {B}, groups: [\n% retained group\n{A,B},\n% removed group\n{B},\n{},\n{'C'}], B: 4, label: \"B\");\nguests = [(friends: {A,B}, groups: [{B},{A},{C,B}], B: 10, label: \"A\"), (friends: {B}, groups: [{B}], B: 20, label: \"gone\"), (friends: {B,C}, groups: [{B,C}], B: 30, label: \"C\")];\nafter = info.groups[(4)]; nested = guests[A].groups[3]; unshifted = guests[C].groups[1];\nbundle = ([{B},{A,C}], 4); from_tuple = bundle.1[2];\nother = {X,Y}; unchanged = 4;\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(result.is_complete(), "{:?}", result.unresolved());
    let text = String::from_utf8_lossy(result.candidate().source_bytes());
    assert!(
        text.contains("before = info.groups[3]") && text.contains("after = info.groups[(3)]"),
        "{text}"
    );
    assert!(
        text.contains("nested = guests[A].groups[2]")
            && text.contains("unshifted = guests[C].groups[1]"),
        "{text}"
    );
    assert!(
        text.contains("bundle = ([{A,C}], 4); from_tuple = bundle.1[1]"),
        "{text}"
    );
    assert!(
        text.contains("friends: {}")
            && text.contains("% retained group")
            && !text.contains("% removed group"),
        "{text}"
    );
    assert!(
        text.contains("groups: [{A},{C}]") && !text.contains("label: \"gone\""),
        "{text}"
    );
    assert!(
        text.contains("B: 4, label: \"B\"") && text.contains("other = {X,Y}; unchanged = 4"),
        "{text}"
    );
    assert_eq!(
        query(
            result.candidate(),
            "filter(name(\"info\")) | fields | filter(name(\"groups\")) | elements | count"
        )
        .render(),
        b"3\n"
    );
    assert_eq!(input.source_bytes(), source);
}

#[test]
fn enum_reduction_keeps_required_empty_fields_and_fixed_cells_complete() {
    let model = reduction_model(
        "empty-sets",
        "enum Guests; type Info = record(set of Guests: friends); Info: info; array[1..3] of set of Guests: fixed; array[Guests] of set of Guests: keyed; array[1..2,1..2] of set of Guests: cells; solve satisfy;",
    );
    let input = Input::parse(b"Guests = {A,B,C}; info = (friends: {B}); fixed = [{B},{}, {A,B}]; keyed = [{B},{A},{B,C}]; cells = [|{B},{}|{A,B},{C}|];".to_vec(), FileMode::Data).unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(result.is_complete(), "{:?}", result.unresolved());
    let text = String::from_utf8_lossy(result.candidate().source_bytes());
    assert!(
        text.contains("info = (friends: {})")
            && text.contains("fixed = [{},{}, {A}]")
            && text.contains("keyed = [{},{C}]")
            && text.contains("cells = [|{},{}|{A},{C}|]"),
        "{text}"
    );
}

#[test]
fn enum_reduction_retains_unsupported_group_accesses_beside_independent_edits() {
    let model_source = "enum Guests; array[int] of set of Guests: groups; set of Guests: deleted; set of Guests: computed; set of Guests: mixed; int: n; set of Guests: read_only = groups[3]; solve satisfy;";
    let model = reduction_model("group-dependencies", model_source);
    let source = b"Guests = {A,B,C}; deleted = groups[2]; computed = groups[n]; groups = [{A,B},{B},{C}]; mixed = {A,B,f(B)}; n = 3;";
    let input = Input::parse_named(source.to_vec(), FileMode::Data, "groups.dzn").unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(!result.is_complete());
    let text = String::from_utf8_lossy(result.candidate().source_bytes());
    assert!(
        text.contains("groups = [{A},{C}]")
            && text.contains("mixed = {A,f(B)}")
            && text.contains("deleted = groups[2]; computed = groups[n]"),
        "{text}"
    );
    for expected in [b"groups[2]".as_slice(), b"groups[n]", b"f(B)"] {
        assert!(
            result
                .unresolved()
                .iter()
                .any(
                    |dependency| dependency.location.path == std::path::Path::new("groups.dzn")
                        && &source[dependency.location.range.clone()] == expected
                ),
            "{:?}",
            result.unresolved()
        );
    }
    assert!(
        result
            .unresolved()
            .iter()
            .any(|dependency| dependency.location.path == model.root
                && &model_source.as_bytes()[dependency.location.range.clone()] == b"groups[3]")
    );
}

#[test]
fn enum_reduction_retains_computed_model_access_dependencies_with_original_locations() {
    let source = "\u{feff}enum Guests; array[Guests] of int: scores;\nint: chosen = scores[to_enum(Guests, 2)];\nint: first = scores[A];\nint: unchanged = let { array[1..3] of int: scores = [1,2,3]; } in scores[2];\nsolve satisfy;\n";
    let model = reduction_model("model-access", source);
    let input = Input::parse_named(
        b"Guests = {A,B,C}; scores = [10,20,30];\n".to_vec(),
        FileMode::Data,
        "scores.dzn",
    )
    .unwrap();
    let result = reduce(
        &input,
        Some(&model),
        "reduce_enum(\"Guests\", keep(\"A\", \"C\"))",
    );
    assert!(!result.is_complete());
    assert_eq!(
        result.candidate().source_bytes(),
        b"Guests = {A,C}; scores = [10,30];\n"
    );
    assert_eq!(result.unresolved().len(), 1);
    let dependency = &result.unresolved()[0];
    assert_eq!(dependency.location.path, model.root);
    assert_eq!(
        &source.as_bytes()[dependency.location.range.clone()],
        b"scores[to_enum(Guests, 2)]"
    );
    assert_eq!(
        (dependency.location.line, dependency.location.column),
        (2, 15)
    );
    assert!(dependency.message.contains("computed enum-indexed access"));
}

fn semantic_model(
    tag: &str,
    root: &[u8],
    included: &[u8],
    core: &str,
) -> zincite_lint::ModelContext {
    let directory = std::env::temp_dir().join(format!(
        "zincite-query-semantic-{tag}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(directory.join("library/std")).unwrap();
    std::fs::write(directory.join("root.mzn"), root).unwrap();
    std::fs::write(directory.join("included.mzn"), included).unwrap();
    std::fs::write(directory.join("library/std/stdlib.mzn"), core).unwrap();
    let model = zincite_lint::load_model(
        directory.join("root.mzn"),
        &zincite_lint::ModelOptions {
            include_dirs: vec![],
            stdlib_dir: Some(directory.join("library")),
        },
    );
    std::fs::remove_dir_all(directory).unwrap();
    assert!(model.errors.is_empty(), "{:?}", model.errors);
    model
}

fn semantic<'a>(
    input: &'a Input,
    model: &'a zincite_lint::ModelContext,
    expression: &str,
) -> zincite_query::SemanticResult<'a> {
    let result = Query::parse(expression, Limits::default())
        .unwrap()
        .evaluate_with_model(input, Some(model), Limits::default())
        .unwrap();
    let QueryResult::Semantic(report) = result else {
        panic!("expected semantic inspection")
    };
    report
}

#[test]
fn semantic_navigation_keeps_shadowing_overloads_shared_nodes_and_actual_source_owners() {
    use zincite_lint::{
        BindingResolution, CallOutcome, DeclarationRole, SourceKind, TypeKind, resolve_bindings,
        resolve_callables,
    };
    use zincite_query::{SemanticEntity, SemanticStream};
    let source = b"\xef\xbb\xbfinclude \"included.mzn\";\r\nint: x = included;\r\nfunction int: choose(int: x = included) = let { int: x = x; } in x;\r\nfloat: floating = choose(1.0);\r\nint: chosen = choose(x);\r\narray[int] of int: pair = [i | i,j in 1..2];\r\nsolve satisfy;\r\n";
    let included = b"\xef\xbb\xbf% included\r\nint: 'included' = 2;\r\n";
    let model = semantic_model(
        "owners",
        source,
        included,
        "function float: choose(float: x) = x;\nfunction set of int: '..'(int: lower, int: upper);\n",
    );
    let input =
        Input::parse_named(source.to_vec(), FileMode::Model, "diagnostic-label.mzn").unwrap();
    let bindings = resolve_bindings(&model);
    let callables = resolve_callables(&model, &bindings);

    let references = semantic(&input, &model, "filter(name(\"x\")) | references");
    assert!(references.is_complete());
    assert!(references.callables().is_none() && references.instantiation_facts().is_none());
    let SemanticStream::Entities(rows) = references.stream() else {
        panic!()
    };
    let SemanticEntity::Reference(reference) = rows[0] else {
        panic!()
    };
    assert_eq!(
        references.location(rows[0]),
        &bindings.references[reference].location
    );
    let BindingResolution::Resolved(included_id) = bindings.references[reference].resolution else {
        panic!()
    };
    let declaration = semantic(
        &input,
        &model,
        "filter(name(\"x\")) | references | declarations",
    );
    let SemanticStream::Entities(rows) = declaration.stream() else {
        panic!()
    };
    assert_eq!(rows, &[SemanticEntity::Declaration(included_id)]);
    assert_eq!(
        declaration.location(rows[0]),
        &bindings.declarations[included_id.0].location
    );
    assert_eq!(
        model.files[declaration.file(rows[0])].kind,
        SourceKind::User
    );
    assert_eq!(declaration.source_bytes(rows[0]), b"int: 'included' = 2;");
    assert_eq!(
        &included[declaration.location(rows[0]).range.clone()],
        b"'included'"
    );
    let json = semantic(
        &input,
        &model,
        "filter(name(\"x\")) | references | declarations | json",
    );
    let value: serde_json::Value =
        serde_json::from_slice(&QueryResult::Semantic(json).render()).unwrap();
    assert_eq!(value["result"][0]["text"], "int: 'included' = 2;");
    assert_eq!(
        value["result"][0]["file"],
        model.files[bindings.declarations[included_id.0].file]
            .path
            .to_string_lossy()
            .as_ref()
    );

    let shadowed = semantic(
        &input,
        &model,
        "filter(name(\"choose\")) | references | filter(name(\"x\")) | declarations | unique",
    );
    let SemanticStream::Entities(rows) = shadowed.stream() else {
        panic!()
    };
    let roles: Vec<_> = rows
        .iter()
        .map(|entity| {
            let SemanticEntity::Declaration(id) = entity else {
                panic!()
            };
            bindings.declarations[id.0].role
        })
        .collect();
    assert_eq!(roles, [DeclarationRole::Parameter, DeclarationRole::Local]);
    let shared = semantic(
        &input,
        &model,
        "subtree | declarations | filter(name(\"i\") or name(\"j\")) | unique",
    );
    let SemanticStream::Entities(rows) = shared.stream() else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    let (SemanticEntity::Declaration(a), SemanticEntity::Declaration(b)) = (rows[0], rows[1])
    else {
        panic!()
    };
    assert_ne!(a, b);
    assert_eq!(
        bindings.declarations[a.0].syntax_range,
        bindings.declarations[b.0].syntax_range
    );

    for (name, kind, owner) in [
        ("chosen", TypeKind::Int, SourceKind::User),
        ("floating", TypeKind::Float, SourceKind::StandardLibrary),
    ] {
        let call = callables
            .calls
            .iter()
            .find(|call| {
                call.name == "choose"
                    && source[call.location.range.start..].starts_with(if name == "chosen" {
                        b"choose(x)"
                    } else {
                        b"choose(1.0)"
                    })
            })
            .unwrap();
        let CallOutcome::Resolved {
            declaration,
            return_type,
            ..
        } = &call.outcome
        else {
            panic!("{:?}", call.outcome)
        };
        let report = semantic(
            &input,
            &model,
            &format!(
                "filter(name(\"{name}\")) | references | filter(name(\"choose\")) | declarations | filter(source_kind(\"{}\")) | types",
                if owner == SourceKind::User {
                    "user"
                } else {
                    "standard_library"
                }
            ),
        );
        assert!(report.is_complete());
        let SemanticStream::Types(rows) = report.stream() else {
            panic!()
        };
        assert_eq!(rows[0].ty.as_ref().unwrap(), return_type);
        assert_eq!(rows[0].ty.as_ref().unwrap().kind, kind);
        assert_eq!(
            model.files[bindings.declarations[declaration.0].file].kind,
            owner
        );
        let uses = semantic(
            &input,
            &model,
            &format!(
                "filter(name(\"{name}\")) | references | filter(name(\"choose\")) | declarations | uses"
            ),
        );
        assert!(uses.is_complete(), "{:?}", uses.limitations());
        let SemanticStream::Entities(rows) = uses.stream() else {
            panic!()
        };
        assert_eq!(rows.len(), 1);
        assert_eq!(uses.location(rows[0]), &call.location);
    }
    let defaults = semantic(
        &input,
        &model,
        "filter(name(\"choose\")) | declarations | transitive_references(1) | names",
    );
    assert_eq!(
        QueryResult::Semantic(defaults).render(),
        b"included\nx\nx\n"
    );
}

#[test]
fn semantic_traversal_is_bounded_deterministic_and_reports_limit_errors() {
    use zincite_query::SemanticStream;
    let source = b"int: a = b; int: b = a; int: c = a; solve satisfy;";
    let model = semantic_model("cycle", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    for (depth, expected) in [
        (0, &b""[..]),
        (1, &b"b\n"[..]),
        (2, &b"b\na\n"[..]),
        (100, &b"b\na\n"[..]),
    ] {
        let expression =
            format!("filter(name(\"a\")) | declarations | transitive_references({depth}) | names");
        let report = semantic(&input, &model, &expression);
        assert!(report.is_complete());
        assert_eq!(QueryResult::Semantic(report).render(), expected);
    }
    let direct = semantic(&input, &model, "subtree | references");
    let unique = semantic(&input, &model, "subtree | references | unique");
    let SemanticStream::Entities(direct_rows) = direct.stream() else {
        panic!()
    };
    let SemanticStream::Entities(unique_rows) = unique.stream() else {
        panic!()
    };
    assert!(direct_rows.len() > unique_rows.len());
    let query = Query::parse(
        "declarations | transitive_references(100)",
        Limits::default(),
    )
    .unwrap();
    for limits in [
        Limits {
            work: 5,
            ..Limits::default()
        },
        Limits {
            collection: 5,
            ..Limits::default()
        },
    ] {
        let error = query
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert!(error.message.contains("limit exceeded"));
    }
}

#[test]
fn semantic_unknowns_survive_boolean_filters_projections_and_exact_root_checks() {
    use zincite_lint::{Instantiation, TypeKind};
    use zincite_query::SemanticStream;
    let source = b"function $T: identity($T: x) = x; par tuple(int, $T): mixed; var int: v; int: result = missing(v); int: read = mixed; int: duplicate; int: duplicate; int: ambiguous = duplicate; solve satisfy;";
    let model = semantic_model("unknown", source, b"", "");
    let input = Input::parse_named(source.to_vec(), FileMode::Model, "label-only.mzn").unwrap();
    let types = semantic(
        &input,
        &model,
        "filter(name(\"mixed\")) | declarations | types",
    );
    assert!(!types.is_complete());
    let SemanticStream::Types(rows) = types.stream() else {
        panic!()
    };
    assert!(matches!(
        rows[0].ty.as_ref().unwrap().kind,
        TypeKind::Tuple(_)
    ));
    let instantiations = semantic(
        &input,
        &model,
        "filter(name(\"mixed\")) | declarations | instantiations",
    );
    assert!(instantiations.is_complete());
    assert!(instantiations.callables().is_none());
    let SemanticStream::Instantiations(rows) = instantiations.stream() else {
        panic!()
    };
    assert_eq!(rows[0].instantiation, Instantiation::Parameter);
    let expression_types = semantic(
        &input,
        &model,
        "filter(name(\"read\")) | expressions | types",
    );
    assert!(!expression_types.is_complete());
    let SemanticStream::Types(rows) = expression_types.stream() else {
        panic!()
    };
    assert!(matches!(
        rows[0].ty.as_ref().unwrap().kind,
        TypeKind::Tuple(_)
    ));
    let expression_inst = semantic(
        &input,
        &model,
        "filter(name(\"read\")) | expressions | instantiations",
    );
    assert!(expression_inst.is_complete());
    let SemanticStream::Instantiations(rows) = expression_inst.stream() else {
        panic!()
    };
    assert_eq!(rows[0].instantiation, Instantiation::Parameter);
    assert!(rows[0].reason.is_none());
    let zincite_query::SemanticSubject::Node { file, location, .. } = &rows[0].subject else {
        panic!()
    };
    let produced = zincite_lint::resolve_instantiations(
        &model,
        expression_inst.bindings().unwrap(),
        expression_inst.callables().unwrap(),
    );
    assert!(produced.expressions.iter().any(|fact| fact.file == *file
        && fact.location == *location
        && fact.instantiation == rows[0].instantiation));
    for predicate in [
        "type(\"tuple\")",
        "not type(\"tuple\")",
        "not (type(\"tuple\") or name(\"no\"))",
        "type(\"tuple\") and instantiation(\"par\")",
    ] {
        let query = Query::parse(&format!("declarations | filter(name(\"mixed\")) | filter({predicate}) | names | count | json"), Limits::default()).unwrap();
        assert!(query.requires_model_facts());
        let QueryResult::Semantic(report) = query
            .evaluate_with_model(&input, Some(&model), Limits::default())
            .unwrap()
        else {
            panic!()
        };
        assert!(!report.is_complete());
        assert_eq!(report.limitations()[0].location.path, model.root);
        let json: serde_json::Value =
            serde_json::from_slice(&QueryResult::Semantic(report).render()).unwrap();
        assert_eq!(json["complete"], false);
        assert_eq!(json["result"], 0);
        assert!(!json["limitations"].as_array().unwrap().is_empty());
    }
    let known_or = semantic(
        &input,
        &model,
        "declarations | filter(name(\"mixed\")) | filter(type(\"tuple\") or name(\"mixed\")) | names",
    );
    assert!(!known_or.is_complete());
    assert_eq!(QueryResult::Semantic(known_or).render(), b"mixed\n");
    let missing = semantic(
        &input,
        &model,
        "filter(name(\"result\")) | references | declarations | head(0) | names | count | json",
    );
    assert!(!missing.is_complete());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&QueryResult::Semantic(missing).render())
            .unwrap()["result"],
        0
    );

    let ambiguous = semantic(
        &input,
        &model,
        "filter(name(\"ambiguous\")) | references | declarations",
    );
    assert!(!ambiguous.is_complete());
    let SemanticStream::Entities(rows) = ambiguous.stream() else {
        panic!()
    };
    assert!(rows.is_empty());
    assert!(ambiguous.bindings().unwrap().references.iter().any(|reference| matches!(&reference.resolution, zincite_lint::BindingResolution::Ambiguous(ids) if ids.len() == 2)));
    let source_only = Query::parse(
        "filter(not (source_kind(\"standard_library\") or name(\"no\"))) | count | json",
        Limits::default(),
    )
    .unwrap();
    assert!(source_only.requires_model_facts());
    let QueryResult::Semantic(source_only) = source_only
        .evaluate_with_model(&input, Some(&model), Limits::default())
        .unwrap()
    else {
        panic!()
    };
    assert!(source_only.bindings().is_none() && source_only.callables().is_none());

    let query = Query::parse("references", Limits::default()).unwrap();
    assert!(
        query
            .evaluate(&input, Limits::default())
            .unwrap_err()
            .message
            .contains("model context")
    );
    let mut with_bom = b"\xef\xbb\xbf".to_vec();
    with_bom.extend_from_slice(source);
    for bytes in [with_bom, b"int: unrelated;".to_vec()] {
        let other =
            Input::parse_named(bytes, FileMode::Model, model.root.to_string_lossy()).unwrap();
        let error = query
            .evaluate_with_model(&other, Some(&model), Limits::default())
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.message.contains("exactly match"));
    }
    let data = Input::parse(b"x=1;".to_vec(), FileMode::Data).unwrap();
    assert!(
        query
            .evaluate_with_model(&data, Some(&model), Limits::default())
            .unwrap_err()
            .message
            .contains("model-file mode")
    );
}

#[test]
fn semantic_intrinsic_targets_replace_lexical_uncertainty_without_loading_unused_facts() {
    use zincite_lint::{
        BindingResolution, CallOutcome, ReferenceKind, resolve_bindings, resolve_callables,
    };
    let source = b"int: x = +1; int: y = missing(1); solve satisfy;";
    let model = semantic_model("intrinsic-target", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let bindings = resolve_bindings(&model);
    let reference = bindings
        .references
        .iter()
        .find(|reference| reference.name == "+")
        .unwrap();
    assert_eq!(reference.kind, ReferenceKind::Callable);
    assert_eq!(reference.resolution, BindingResolution::Unresolved);
    let callables = resolve_callables(&model, &bindings);
    assert!(
        callables
            .calls
            .iter()
            .any(|call| call.location == reference.location
                && matches!(call.outcome, CallOutcome::Intrinsic { .. }))
    );

    let report = semantic(
        &input,
        &model,
        "filter(name(\"x\")) | references | declarations | json",
    );
    assert!(report.is_complete(), "{:?}", report.limitations());
    let json: serde_json::Value =
        serde_json::from_slice(&QueryResult::Semantic(report).render()).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["result"], serde_json::json!([]));
    let missing = semantic(
        &input,
        &model,
        "filter(name(\"y\")) | references | declarations | names | count | json",
    );
    assert!(!missing.is_complete());
    assert!(!missing.limitations().is_empty());
    let references_only = semantic(&input, &model, "filter(name(\"x\")) | references | count");
    assert!(references_only.callables().is_none());
    assert!(!references_only.is_complete());
}
