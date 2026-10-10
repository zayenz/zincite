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
