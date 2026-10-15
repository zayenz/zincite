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
fn predicate_nesting_counts_written_groups_and_not_only() {
    let input = Input::parse(b"x = 1; y = 2;".to_vec(), FileMode::Data).unwrap();
    let nested = format!(
        "filter({}{}name(\"x\"){}) | count",
        "(".repeat(32),
        "not ".repeat(32),
        ")".repeat(32),
    );
    let parsed = Query::parse(&nested, Limits::default()).unwrap();
    assert_eq!(
        parsed.evaluate(&input, Limits::default()).unwrap().render(),
        b"1\n"
    );
    let lower_limits = Limits {
        nesting: 63,
        ..Limits::default()
    };
    assert_eq!(
        parsed.evaluate(&input, lower_limits).unwrap().render(),
        b"1\n"
    );
    let error = Query::parse(&nested, lower_limits).unwrap_err();
    assert_eq!(error.location, ErrorLocation::Query);
    assert!(error.range.start >= "filter(".len());
    assert!(error.message.contains("nesting limit"));

    let flat = "filter(name(\"missing\") or name(\"x\") and kind(\"assignment\")) | count";
    let limits = Limits {
        nesting: 0,
        ..Limits::default()
    };
    assert_eq!(
        Query::parse(flat, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap()
            .render(),
        b"1\n"
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
        format!("{expression} | emit"),
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
fn semantic_source_facts_type_demanded_library_references_without_transitive_bodies() {
    use zincite_lint::{Instantiation, TypeKind};
    use zincite_query::SemanticStream;
    let source = b"var int: chosen = outer(); solve satisfy;";
    let model = semantic_model(
        "source-body-demand",
        source,
        b"",
        concat!(
            "function var int: outer(int: x = leaf(0)) = inner(x);\n",
            "function var int: inner(int: x) = leaf(x);\n",
            "function int: leaf(int: x) = missing(x);\n"
        ),
    );
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let outer = "filter(name(\"chosen\")) | references | filter(name(\"outer\"))";
    for projection in ["types", "declarations | types"] {
        let report = semantic(&input, &model, &format!("{outer} | {projection}"));
        assert!(report.is_complete(), "{:?}", report.limitations());
        let SemanticStream::Types(rows) = report.stream() else {
            panic!()
        };
        assert_eq!(rows[0].ty.as_ref().unwrap().kind, TypeKind::Int);
        assert_eq!(
            rows[0].ty.as_ref().unwrap().instantiation,
            Instantiation::Decision
        );
        let calls = &report.callables().unwrap().calls;
        assert!(calls.iter().any(|call| call.name == "leaf")); // signature default
        assert!(
            !calls
                .iter()
                .any(|call| call.name == "inner" || call.name == "missing")
        );
    }

    let inner = format!("{outer} | declarations | references | filter(name(\"inner\"))");
    for projection in ["types", "instantiations", "declarations"] {
        let report = semantic(&input, &model, &format!("{inner} | {projection}"));
        assert!(report.is_complete(), "{:?}", report.limitations());
        match report.stream() {
            SemanticStream::Types(rows) => {
                assert_eq!(rows[0].ty.as_ref().unwrap().kind, TypeKind::Int);
            }
            SemanticStream::Instantiations(rows) => {
                assert_eq!(rows[0].instantiation, Instantiation::Decision);
            }
            SemanticStream::Entities(rows) => {
                let zincite_query::SemanticEntity::Declaration(id) = rows[0] else {
                    panic!()
                };
                assert_eq!(report.bindings().unwrap().declarations[id.0].name, "inner");
            }
            _ => panic!(),
        }
        let calls = &report.callables().unwrap().calls;
        assert!(calls.iter().any(|call| call.name == "inner"));
        assert!(!calls.iter().any(|call| call.name == "missing"));
    }
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
fn declaration_uses_skip_unrelated_references_and_keep_order_duplicates_and_spelling() {
    use zincite_query::{SemanticEntity, SemanticStream};
    let source = include_bytes!("fixtures/native-composition.mzn");
    let core = (0..20)
        .map(|i| format!("int: unrelated_{i} = missing_{i}(1);\n"))
        .collect::<String>();
    let model = semantic_model("uses-related", source, b"", &core);
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let query = Query::parse(
        "filter(name(\"reads\")) | references | declarations | uses | json",
        Limits::default(),
    )
    .unwrap();
    let QueryResult::Semantic(report) = query
        .evaluate_with_model(
            &input,
            Some(&model),
            Limits {
                work: 3_000,
                collection: 4,
                ..Limits::default()
            },
        )
        .unwrap()
    else {
        panic!()
    };
    assert!(report.is_complete(), "{:?}", report.limitations());
    assert!(report.callables().is_none());
    let SemanticStream::Entities(rows) = report.stream() else {
        panic!()
    };
    let first = source
        .windows(9)
        .position(|bytes| bytes == b"[capacity")
        .unwrap()
        + 1;
    let second = first + b"capacity, ".len();
    assert_eq!(rows.len(), 4);
    for (entity, range) in rows.iter().zip([
        first..first + 8,
        second..second + 10,
        first..first + 8,
        second..second + 10,
    ]) {
        assert!(matches!(entity, SemanticEntity::Reference(_)));
        assert_eq!(report.location(*entity).path, model.root);
        assert_eq!(report.location(*entity).range, range);
        assert_eq!(report.source_bytes(*entity), &source[range]);
    }
}

#[test]
fn declaration_uses_keep_relevant_uncertainty_and_upstream_or_empty_selection_limits() {
    for (tag, source, name, uncertain) in [
        (
            "unresolved",
            "function int: identity(int: x) = x; int: unknown = x; int: unrelated = missing(1); solve satisfy;",
            "x",
            "x; int: unrelated",
        ),
        (
            "ambiguous",
            "int: x; int: x; int: unknown = x; int: unrelated = missing(1); solve satisfy;",
            "x",
            "x; int: unrelated",
        ),
        (
            "overloaded",
            "function int: choose(int: x) = x; function int: choose(int: y) = y; int: unknown = choose(1); int: unrelated = missing(1); solve satisfy;",
            "choose",
            "choose(1)",
        ),
    ] {
        let model = semantic_model(tag, source.as_bytes(), b"", "");
        let input = Input::parse(source.as_bytes().to_vec(), FileMode::Model).unwrap();
        let report = semantic(
            &input,
            &model,
            &format!(
                "subtree | declarations | filter(name(\"{name}\")) | uses | head(0) | names | count | json"
            ),
        );
        assert!(!report.is_complete());
        assert_eq!(report.limitations().len(), 1, "{:?}", report.limitations());
        let location = &report.limitations()[0].location;
        assert_eq!(location.path, model.root);
        assert_eq!(location.range.start, source.find(uncertain).unwrap());
        assert_eq!(&source[location.range.clone()], name);
        let json: serde_json::Value =
            serde_json::from_slice(&QueryResult::Semantic(report).render()).unwrap();
        assert_eq!(json["complete"], false);
        assert_eq!(json["result"], 0);

        let empty = semantic(&input, &model, "head(0) | declarations | uses | names");
        assert!(!empty.is_complete());
        assert!(
            empty
                .limitations()
                .iter()
                .any(|limitation| { &source[limitation.location.range.clone()] == "missing" })
        );
        let upstream = semantic(
            &input,
            &model,
            &format!(
                "filter(name(\"unrelated\")) | references | declarations | filter(name(\"{name}\")) | uses | count"
            ),
        );
        assert!(!upstream.is_complete());
        assert!(
            upstream
                .limitations()
                .iter()
                .any(|limitation| { &source[limitation.location.range.clone()] == "missing" })
        );
    }
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
            collection: 3,
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
fn semantic_model_admission_is_distinct_from_result_limits() {
    let source = b"int: capacity = 2;";
    let core: String = (0..20).map(|i| format!("int: unused_{i} = 1; ")).collect();
    let model = semantic_model("admission", source, b"", &core);
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let query = Query::parse("declarations | types | instantiations", Limits::default()).unwrap();
    let result = query
        .evaluate_with_model(
            &input,
            Some(&model),
            Limits {
                collection: 1,
                work: 500,
                ..Limits::default()
            },
        )
        .unwrap();
    let QueryResult::Semantic(report) = result else {
        panic!()
    };
    assert!(report.is_complete());
    assert!(report.bindings().is_some());
    assert!(report.callables().is_some());
    let zincite_query::SemanticStream::Instantiations(rows) = report.stream() else {
        panic!()
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].instantiation,
        zincite_lint::Instantiation::Parameter
    );
    let error = query
        .evaluate_with_model(
            &input,
            Some(&model),
            Limits {
                work: 100,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Query);
    assert_eq!(error.range, 0..12);
    assert!(error.message.contains("work limit exceeded"));

    let source = b"int: x = (a, a, a);";
    let model = semantic_model("result-limit", source, b"", "int: a = 1;");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let error = Query::parse("references | json", Limits::default())
        .unwrap()
        .evaluate_with_model(
            &input,
            Some(&model),
            Limits {
                collection: 2,
                ..Limits::default()
            },
        )
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Query);
    assert_eq!(error.range, 0..10);
    assert!(error.message.contains("collection limit exceeded"));
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

#[test]
fn programs_retain_native_reports_and_one_fact_owner() {
    use std::rc::Rc;
    use zincite_query::{Program, ProgramLimits, ReportValue, SemanticStream};
    let source = b"int: x = 1; int: y = x; solve satisfy;";
    let model = semantic_model("program-report", source, b"", "");
    let input = Input::parse_named(source.to_vec(), FileMode::Model, "root.mzn").unwrap();
    let program = Program::parse(
        r#"
        let selected = declarations;
        let n = selected | count;
        let histogram = selected | names | tally;
        let references = references;
        report (count: n, histogram: histogram, 'selected rows': selected, references: references);
    "#,
        ProgramLimits::default(),
    )
    .unwrap();
    assert!(program.requires_model_facts());
    let mut result = program
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(result.is_complete());
    let fields = result.emissions()[0].value().fields().unwrap();
    let ReportValue::Query(SemanticStream::Inspection(count)) = &fields[0].1 else {
        panic!("native count");
    };
    assert!(matches!(count.as_ref(), QueryResult::Count(2)));
    let ReportValue::Query(SemanticStream::Inspection(histogram)) = &fields[1].1 else {
        panic!("native histogram");
    };
    let QueryResult::Tally(histogram) = histogram.as_ref() else {
        panic!("native histogram");
    };
    assert_eq!(
        histogram
            .iter()
            .find(|(key, _)| key == "x")
            .map(|(_, count)| count),
        Some(&1)
    );
    let Some(SemanticStream::Entities(selected)) = fields[2].1.stream() else {
        panic!("native declarations");
    };
    let single = semantic(&input, &model, "declarations");
    let SemanticStream::Entities(expected) = single.stream() else {
        panic!("native declarations");
    };
    assert_eq!(selected, expected);
    for entity in selected {
        assert_eq!(
            result.semantic().unwrap().location(*entity),
            single.location(*entity)
        );
    }
    let bytes = result.render().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["result"]["count"], 2);
    assert_eq!(json["result"]["histogram"]["y"], 1);
    assert_eq!(json["result"]["selected rows"][0]["text"], "int: x = 1;");
    let facts = Rc::new(zincite_lint::ModelFacts::new(&model));
    let mut reused = program
        .evaluate_with_facts(&input, facts.clone(), ProgramLimits::default())
        .unwrap();
    assert!(std::ptr::eq(
        reused.semantic().unwrap().bindings().unwrap(),
        facts.bindings()
    ));
    assert_eq!(reused.render().unwrap(), bytes);
}

#[test]
fn program_frontend_and_aggregate_refuse_invalid_requests() {
    use zincite_query::{Program, ProgramLimits};
    for source in [
        "let a = b; let b = count;",
        "let a = count; let a = count;",
        "report (n: count, n: count);",
        "count | items;",
        "source a = count;",
    ] {
        let error = Program::parse(source, ProgramLimits::default()).unwrap_err();
        assert!(matches!(
            error.location,
            zincite_query::ErrorLocation::Query
        ));
    }
    let input = Input::parse_named(b"a = 1; b = 2;".to_vec(), FileMode::Data, "data.dzn").unwrap();
    let program = Program::parse("let a = items; a; a;", ProgramLimits::default()).unwrap();
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                results: 7,
                ..ProgramLimits::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("result limit"), "{error:?}");
}

#[test]
fn positional_programs_keep_constructed_order_and_native_projections() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let input = Input::parse(
        b"zeta = [3, /*\xff*/ 5]; alpha = (+ /*\xff*/ 2);".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let source = r#"let labels = names;
        let numbers = items | filter(name("zeta")) | values | one;
        let n = count;
        let rows = [(name: label, pair: (i,j), value: numbers[i] + k, total: n)
            | i,j in index_set(numbers), label in [labels[i]], k = i+(i,j).2, _ in 1..1 where j <= i];
        report rows;"#;
    let program = Program::parse(source, ProgramLimits::default()).unwrap();
    let mut result = program.evaluate(&input, ProgramLimits::default()).unwrap();
    let ReportValue::Constructed(V::Array(rows)) = result.emissions()[0].value() else {
        panic!("constructed positional array")
    };
    let values: Vec<_> = rows
        .iter()
        .map(|row| {
            let V::Record(fields) = row else { panic!() };
            assert_eq!(
                fields.iter().map(|f| f.0.as_str()).collect::<Vec<_>>(),
                ["name", "pair", "value", "total"]
            );
            let V::String(name) = &fields[0].1 else {
                panic!()
            };
            let V::Tuple(pair) = &fields[1].1 else {
                panic!()
            };
            assert_eq!(fields[3].1, V::Integer(2));
            (name.as_str(), &pair[0], &pair[1], &fields[2].1)
        })
        .collect();
    assert_eq!(
        values,
        [
            ("zeta", &V::Integer(1), &V::Integer(1), &V::Integer(5)),
            ("alpha", &V::Integer(2), &V::Integer(1), &V::Integer(8)),
            ("alpha", &V::Integer(2), &V::Integer(2), &V::Integer(9))
        ]
    );
    let bytes = result.render().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["limitations"], serde_json::json!([]));
    assert_eq!(
        json["result"]["axes"][0][2],
        serde_json::json!({"kind":"integer","value":3})
    );
    assert_eq!(
        json["result"]["entries"][2]["value"]["pair"]["kind"],
        "tuple"
    );
    assert_eq!(result.render().unwrap(), bytes);
    let mut limited = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 20000,
                ..ProgramLimits::default()
            },
        )
        .unwrap();
    let mut renders = 0;
    while limited.render().is_ok() {
        renders += 1;
    }
    assert!(renders > 0);

    // A record binding retains tagged scalar fields in an ordered object.
    let explicit = Program::parse(
        r#"let n = count; let r = (count: n); report r;"#,
        ProgramLimits::default(),
    )
    .unwrap();
    let mut explicit = explicit.evaluate(&input, ProgramLimits::default()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&explicit.render().unwrap()).unwrap();
    assert!(json["result"].is_object());
    assert_eq!(
        json["result"]["count"],
        serde_json::json!({"kind":"integer","value":2})
    );

    // Literal inspection projects nested positional values from the same CST.
    let inspected = Program::parse(
        r#"let written = values; let r = written; report r;"#,
        ProgramLimits::default(),
    )
    .unwrap()
    .evaluate(&input, ProgramLimits::default())
    .unwrap();
    assert!(matches!(
        inspected.emissions()[0].value(),
        ReportValue::Constructed(V::Array(values))
            if values == &[V::Array(vec![V::Integer(3), V::Integer(5)]), V::Integer(2)]
    ));
}

#[test]
fn positional_programs_validate_branches_and_preserve_failure_locations() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(b"a = [1];".to_vec(), FileMode::Data).unwrap();
    let skipped = Program::parse(
        "let r = (value: 7); report if true then r.value else [1][8] div 0 endif;",
        limits,
    )
    .unwrap();
    let result = skipped.evaluate(&input, limits).unwrap();
    assert!(matches!(
        result.emissions()[0].value(),
        ReportValue::Constructed(V::Integer(7))
    ));
    for expression in [
        "[i | i in i]",
        "if true then 1 else unsupported_summary([1]) endif",
        "if true then 1 else 1 + false endif",
        "if true then 1 else 1e-999 endif",
        "[true: 3]",
        "if 1 then 2 else 3 endif",
    ] {
        let source = format!("report {expression};");
        let error = Program::parse(&source, limits).unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert!(
            error.range.start >= 7 && error.range.end <= source.len(),
            "{error:?}"
        );
    }
    for expression in [
        "9223372036854775807 + 1",
        "-9223372036854775808 div -1",
        "-9223372036854775808 mod -1",
        "1 div 0",
        "9007199254740993 + 1.0",
        "1e-300 * 1e-300",
    ] {
        let source = format!("report {expression};");
        let program = Program::parse(&source, limits).unwrap();
        let error = program.evaluate(&input, limits).unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert_eq!(&source[error.range], expression);
    }
    let source = "report [i | i in 1..1000 where false];";
    let program = Program::parse(source, limits).unwrap();
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                results: 50,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("result limit"), "{error:?}");
    let bom = Input::parse(b"\xef\xbb\xbfa = 1 + 2;".to_vec(), FileMode::Data).unwrap();
    let program = Program::parse(
        r#"let x = items | filter(name("a")) | values | one; report x;"#,
        limits,
    )
    .unwrap();
    let error = program.evaluate(&bom, limits).unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert_eq!(error.range, 7..12);
    let mut bytes = b"\xef\xbb\xbfa = ".to_vec();
    bytes.extend_from_slice("0".repeat(10_000).as_bytes());
    bytes.extend_from_slice(b"1;");
    let input = Input::parse(bytes, FileMode::Data).unwrap();
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 1000,
                ..limits
            },
        )
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert_eq!(error.range, 7..10008);
    assert!(error.message.contains("work limit"), "{error:?}");
}

#[test]
fn constructed_reports_retain_semantic_limitations() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    let source = b"int: value = missing; solve satisfy;";
    let model = semantic_model("constructed-incomplete", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let program = Program::parse(r#"let n = references | declarations | names | count; let r = (count: n, cells: entries(zip_exact([1:n],[1:n]))); report r;"#, ProgramLimits::default()).unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(matches!(
        result.emissions()[0].value(),
        ReportValue::Constructed(_)
    ));
    assert!(!result.is_complete());
    let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(json["complete"], false);
    assert!(!json["limitations"].as_array().unwrap().is_empty());
    assert_eq!(json["result"]["count"]["kind"], "integer");
}

#[test]
fn keyed_comparison_keeps_native_pairs_entries_and_explicit_cell_deltas() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::{ConstructedValue as V, ValueKey as K, ValueKeyType};
    let limits = ProgramLimits::default();
    let bytes = include_bytes!("fixtures/keyed-comparison.dzn");
    let input = Input::parse(bytes.to_vec(), FileMode::Data).unwrap();
    let mut delta = Program::parse(include_str!("fixtures/keyed-comparison.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(V::KeyedArray { axes, entries }) = delta.emissions()[0].value()
    else {
        panic!("keyed delta")
    };
    assert!(
        axes.iter()
            .all(|axis| axis.key_type == ValueKeyType::Integer
                && axis.keys == [K::Integer(1), K::Integer(2)])
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| (&entry.key, &entry.value))
            .collect::<Vec<_>>(),
        vec![
            (&vec![K::Integer(1), K::Integer(1)], &V::Integer(1)),
            (&vec![K::Integer(1), K::Integer(2)], &V::Integer(2)),
            (&vec![K::Integer(2), K::Integer(1)], &V::Integer(3)),
            (&vec![K::Integer(2), K::Integer(2)], &V::Integer(4)),
        ]
    );
    let pairs = Program::parse(
        r#"let a=items | filter(name("before")) | values | one; let b=items | filter(name("after")) | values | one;
        report (pairs: zip_exact(a,b), cells: entries(zip_exact(a,b)));"#,
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = pairs.emissions()[0].value() else {
        panic!("pair record")
    };
    let V::KeyedArray {
        axes: paired_axes,
        entries: paired_entries,
    } = &fields[0].1
    else {
        panic!("paired array")
    };
    assert_eq!(paired_axes, axes);
    assert_eq!(paired_entries[0].key, entries[0].key);
    assert_eq!(
        paired_entries[0].value,
        V::Tuple(vec![V::Integer(3), V::Integer(4)])
    );
    let V::Array(cells) = &fields[1].1 else {
        panic!("entry records")
    };
    assert_eq!(
        cells[3],
        V::Record(vec![
            ("key".into(), V::Tuple(vec![V::Integer(2), V::Integer(2)])),
            (
                "value".into(),
                V::Tuple(vec![V::Integer(6), V::Integer(10)])
            ),
        ])
    );
    assert!(delta.is_complete() && pairs.is_complete());
    let rendered = delta.render().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&rendered).unwrap();
    assert_eq!(json["result"]["indexing"], "explicit");
    assert_eq!(json["result"]["entries"][3]["value"]["value"], 4);
    let shifted = "report 7; report zip_exact([1:3,2:4], [2:3,3:4]);";
    let error = Program::parse(shifted, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap_err();
    assert_eq!(&shifted[error.range], "zip_exact([1:3,2:4], [2:3,3:4])");
    assert!(error.message.contains("ordered axes"));
}

#[test]
fn keyed_reports_retain_model_identity_axes_and_row_major_cells() {
    use std::rc::Rc;
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::{ConstructedValue as V, ValueKey as K};
    let model_source = b"enum Guest = {Zulu} ++ {Alpha}; array[2..3, Guest] of int: table; array[3..4] of int: tail; array[1..2,1..2] of int: matrix; set of Guest: vacant; array[1..0,Guest] of int: empty; type Info=record(array[Guest] of int: values); Info:info; solve satisfy;";
    let model = semantic_model("keyed-report", model_source, b"", "");
    let input = Input::parse_named(b"\xef\xbb\xbftable = [(3,Alpha):32, (2,Zulu):21, (3,Zulu):31, (2,Alpha):22]; tail=[3:7,8]; matrix=[|3,4|5,6|]; vacant={}; empty=[]; info=(values:[7,9]);".to_vec(), FileMode::Data, "report.dzn").unwrap();
    let source = r#"let t = items | filter(name("table")) | values | one; let tail = items | filter(name("tail")) | values | one;
        let matrix = items | filter(name("matrix")) | values | one; let vacant = items | filter(name("vacant")) | values | one; let empty = items | filter(name("empty")) | values | one; let info=items | filter(name("info")) | values | one;
        let members = {enum_member(Guest,"Alpha"), enum_member(Guest,"Zulu"), enum_member(Guest,"Zulu")};
        let cells = [(i,e): t[i-1,e] | i in index_set(tail), e in members];
        let zero = [(i,e): 1 | i in 1..0, e in members];
        report (cells: cells, written: t, pair: (tail[3],matrix[2,1]), nested: [[1],[2,3]], members: members, zero: zero, vacant: vacant, empty: empty, info: info, aligned: zip_exact(t,t), inspected: entries(zip_exact(t,t)), empty_pairs: zip_exact(empty,empty));"#;
    let limits = ProgramLimits::default();
    let program = Program::parse(source, limits).unwrap();
    let facts = Rc::new(zincite_lint::ModelFacts::new(&model));
    let mut result = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert!(std::ptr::eq(result.model_facts().unwrap().model(), &model));
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("native constructed record")
    };
    let V::KeyedArray { axes, entries } = fields[0].1.data() else {
        panic!("native keyed result")
    };
    assert_eq!(axes.len(), 2);
    assert_eq!(axes[0].keys, [K::Integer(3), K::Integer(4)]);
    let enum_id = facts
        .bindings()
        .declarations
        .iter()
        .find(|d| d.name == "Guest")
        .unwrap()
        .id
        .0 as u64;
    let names: Vec<_> = axes[1]
        .keys
        .iter()
        .map(|key| {
            let K::Member(member) = key else {
                panic!("nominal key")
            };
            assert_eq!(member.type_id.0, enum_id);
            let declaration = facts
                .bindings()
                .declarations
                .iter()
                .find(|d| d.name == member.member_name)
                .unwrap();
            assert_eq!(member.member_id.0, declaration.id.0 as u64);
            (member.member_name.as_str(), member.declaration_order)
        })
        .collect();
    assert_eq!(names, [("Zulu", 0), ("Alpha", 1)]);
    assert_eq!(
        entries.iter().map(|e| e.value.data()).collect::<Vec<_>>(),
        [
            &V::Integer(21),
            &V::Integer(22),
            &V::Integer(31),
            &V::Integer(32)
        ]
    );
    assert_eq!(entries[0].key[0], K::Integer(3));
    let V::KeyedArray { axes, entries } = fields[5].1.data() else {
        panic!("known empty result")
    };
    assert_eq!(axes.len(), 2);
    assert!(axes[0].keys.is_empty());
    assert_eq!(axes[1].keys.len(), 2);
    assert!(entries.is_empty());
    let V::Set {
        element_type: Some(zirql::SetElementType::Member(id)),
        elements,
    } = fields[6].1.data()
    else {
        panic!("typed empty set")
    };
    assert_eq!(id.0, enum_id);
    assert!(elements.is_empty());
    let V::KeyedArray { axes, entries } = fields[7].1.data() else {
        panic!("declared empty array")
    };
    assert_eq!(axes.len(), 2);
    assert!(axes[0].keys.is_empty());
    assert_eq!(axes[1].keys.len(), 2);
    assert!(entries.is_empty());
    let V::Record(info) = fields[8].1.data() else {
        panic!("nested model-backed record")
    };
    let V::KeyedArray { axes, entries } = info[0].1.data() else {
        panic!("written alias field axes")
    };
    assert!(matches!(&axes[0].keys[0], K::Member(member) if member.member_name == "Zulu"));
    assert_eq!(entries[1].value.data(), &V::Integer(9));
    let V::KeyedArray {
        axes: paired_axes,
        entries: pairs,
    } = fields[9].1.data()
    else {
        panic!("nominal pairs")
    };
    let V::KeyedArray {
        axes: original_axes,
        ..
    } = fields[1].1.data()
    else {
        panic!("original axes")
    };
    assert_eq!(paired_axes, original_axes);
    assert!(
        matches!(&paired_axes[1].keys[0], K::Member(member) if member.type_id.0 == enum_id && member.member_name == "Zulu")
    );
    let V::Tuple(pair) = pairs[0].value.data() else {
        panic!("aligned tuple")
    };
    assert!(pair.iter().all(|value| value.data() == &V::Integer(21)));
    let V::Array(inspected) = fields[10].1.data() else {
        panic!("nominal entries")
    };
    assert!(
        matches!(&inspected[0], V::Record(row) if matches!(&row[0].1,
        V::Tuple(key) if matches!(&key[1], V::Member(member) if member.type_id.0 == enum_id && member.member_name == "Zulu")))
    );
    assert_eq!(fields[11].1.data(), fields[7].1.data());
    assert!(result.is_complete());
    let bytes = result.render().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["limitations"], serde_json::json!([]));
    assert_eq!(json["result"]["cells"]["indexing"], "explicit");
    assert_eq!(
        json["result"]["cells"]["axes"][1][0],
        serde_json::json!({"kind":"member","enum":"Guest","name":"Zulu"})
    );
    assert_eq!(json["result"]["pair"]["kind"], "tuple");
    let mut shared = program
        .evaluate_with_facts(&input, Rc::clone(&facts), limits)
        .unwrap();
    assert_eq!(shared.render().unwrap(), bytes);
    let mut with_function =
        Program::parse(&format!("fn unused(n:int)->int {{ n }} {source}"), limits)
            .unwrap()
            .evaluate_with_facts(&input, Rc::clone(&facts), limits)
            .unwrap();
    assert!(std::ptr::eq(
        with_function.model_facts().unwrap(),
        facts.as_ref()
    ));
    assert_eq!(with_function.render().unwrap(), bytes);
    let projected = Program::parse(
        r#"let rows = filter(name("table")) | values; let row = rows[1]; report row;"#,
        limits,
    )
    .unwrap()
    .evaluate_with_facts(&input, Rc::clone(&facts), limits)
    .unwrap();
    assert!(
        matches!(projected.emissions()[0].value(), ReportValue::Constructed(value) if matches!(value.data(), V::KeyedArray { axes, entries } if axes.len() == 2 && entries[0].value.data() == &V::Integer(21)))
    );
    let empty_delta = Program::parse(
        r#"let a=items | filter(name("empty")) | values | one;
        report [e.key: e.value.2 - e.value.1 | e in entries(zip_exact(a,a))];"#,
        limits,
    )
    .unwrap()
    .evaluate_with_facts(&input, Rc::clone(&facts), limits)
    .unwrap_err();
    assert!(empty_delta.message.contains("empty"));
    let literals = Program::parse(
        "let m = [| 2:1: | 4: 42,41 | 3:32,31 |]; report m[3,1];",
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    assert!(matches!(
        literals.emissions()[0].value(),
        ReportValue::Constructed(V::Integer(31))
    ));
}

#[test]
fn keyed_reports_refuse_shape_identity_and_unestablished_empty_axes() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"x=1;".to_vec(), FileMode::Data).unwrap();
    for (source, expected) in [
        ("report [1:1,1:2];", "duplicate"),
        ("report [i+1: i | i in 1..0];", "empty"),
        ("report [(i,j):1 | i in 1..0,j in 1..i];", "empty"),
        ("report [i:1 | i in []];", "empty"),
        ("report {\"no\"};", "set"),
        (
            "report if true then 1 else enum_member(Guest,\"Zulu\") endif;",
            "model context",
        ),
        (
            "let a=[1:1]; report index_set([ (i,j):1 | i,j in 1..2]);",
            "rank one",
        ),
    ] {
        let error = Program::parse(source, limits)
            .and_then(|p| p.evaluate(&input, limits))
            .unwrap_err();
        assert!(error.message.contains(expected), "{source}: {error:?}");
        assert_eq!(error.location, ErrorLocation::Query);
        assert!(error.range.end <= source.len());
        assert!(!error.message.contains('\0'));
    }
    let skipped = Program::parse(
        "report if false then [v|v in [(1,1):1,(1,2):2,(2,1):3]] else [7] endif;",
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    assert!(
        matches!(skipped.emissions()[0].value(),zincite_query::ReportValue::Constructed(zirql::ConstructedValue::Array(values)) if values == &[zirql::ConstructedValue::Integer(7)])
    );
    let model = semantic_model(
        "keyed-refusals",
        b"enum Guest={Zulu,Alpha,Beta}; enum Other={Gamma}; solve satisfy;",
        b"",
        "",
    );
    for (source, expected) in [
        (
            "report [1:1,enum_member(Guest,\"Zulu\"):2];",
            "incompatible",
        ),
        ("report enum_member(Guest,\"absent\");", "unavailable"),
        ("report enum_member(Other,\"Zulu\");", "another enum"),
    ] {
        let p = Program::parse(source, limits).unwrap();
        let error = p
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap_err();
        assert!(error.message.contains(expected), "{error:?}");
        assert_eq!(error.location, ErrorLocation::Query);
    }
    let ambiguous = semantic_model(
        "keyed-ambiguous",
        b"enum Guest={Same}; enum Other={Same}; solve satisfy;",
        b"",
        "",
    );
    let p = Program::parse("report enum_member(Guest,\"Same\");", limits).unwrap();
    assert!(
        p.evaluate_with_model(&input, Some(&ambiguous), limits)
            .unwrap_err()
            .message
            .contains("ambiguous")
    );
    let computed = semantic_model(
        "keyed-computed",
        b"enum Guest={Zulu} ++ anon_enum(2); solve satisfy;",
        b"",
        "",
    );
    let p = Program::parse("report enum_member(Guest,\"Zulu\");", limits).unwrap();
    let error = p
        .evaluate_with_model(&input, Some(&computed), limits)
        .unwrap_err();
    assert!(matches!(error.location, ErrorLocation::Model(_)));
    assert!(error.message.contains("computed"));
    let subset_model=semantic_model("keyed-subset", b"enum Guest={Zulu,Alpha}; set of Guest:Subset; type Info=record(array[Subset] of int:values); Info:info; solve satisfy;", b"", "");
    let subset_input = Input::parse(b"info=(values:[1,2]);".to_vec(), FileMode::Data).unwrap();
    let p = Program::parse(
        r#"let info=items | filter(name("info")) | values | one; report info;"#,
        limits,
    )
    .unwrap();
    let error = p
        .evaluate_with_model(&subset_input, Some(&subset_model), limits)
        .unwrap_err();
    assert!(error.message.contains("axis domain"), "{error:?}");
    assert_eq!(error.location, ErrorLocation::Input);
    let bom = Input::parse(b"\xef\xbb\xbfx=[Alpha:1,2];".to_vec(), FileMode::Data).unwrap();
    let p = Program::parse(
        r#"let a=items | filter(name("x")) | values | one; report a;"#,
        limits,
    )
    .unwrap();
    let error = p
        .evaluate_with_model(&bom, Some(&model), limits)
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert_eq!(&bom.source_bytes()[error.range], b"2");
}

#[test]
fn keyed_report_work_and_repeated_rendering_share_the_request_allowance() {
    use std::rc::Rc;
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let model = semantic_model(
        "keyed-budget",
        b"enum Guest={Zulu,Alpha}; solve satisfy;",
        b"",
        "",
    );
    let input = Input::parse(b"x=[Alpha:2,Zulu:1];".to_vec(), FileMode::Data).unwrap();
    let facts = Rc::new(zincite_lint::ModelFacts::new(&model));
    facts.source_domains();
    let p = Program::parse(
        r#"let x=items | filter(name("x")) | values | one; let a=[i:x[i] | i in index_set(x)]; report a;"#,
        limits,
    )
    .unwrap();
    let mut result = p
        .evaluate_with_facts(
            &input,
            Rc::clone(&facts),
            ProgramLimits {
                work: 12000,
                ..limits
            },
        )
        .unwrap();
    assert!(!result.render().unwrap().is_empty());
    while result.render().is_ok() {}
    assert!(result.render().unwrap_err().message.contains("limit"));
    let error = p
        .evaluate_with_facts(
            &input,
            facts,
            ProgramLimits {
                work: 600,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work limit"));
    let error = p
        .evaluate_with_model(
            &input,
            Some(&model),
            ProgramLimits {
                results: 60,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("result limit"));
}

#[test]
fn data_programs_create_replace_and_preserve_declared_keys_and_source() {
    use zincite_query::{Program, ProgramLimits};
    let model = semantic_model("data-create", b"enum E={Zulu,'alpha beta'}; array[1..2] of int: counts; array[E] of int: keyed; record(1..9: n,array[1..0,2..3] of int: empty): r; array[1..0,2..3] of int: empty; array[1..2] of 1..3: limited; solve satisfy;", b"", "");
    let original = b"\xef\xbb\xbfcounts = [3, 5]; % keep\r\n\r\n% detached\r\n";
    let input = Input::parse_named(original.to_vec(), FileMode::Data, "data.dzn").unwrap();
    let source = r#"let old = items | filter(name("counts")) | values | one; let saved = old; replace counts = [saved[i]+1 | i in index_set(saved)]; create keyed = [enum_member(E,"Zulu"):7,enum_member(E,"alpha beta"):9]; create r = (n:1,empty:[(i+0,j):1 | i in 1..0,j in 2..3]); create empty = []; emit;"#;
    let program = Program::parse(source, ProgramLimits::default()).unwrap();
    assert!(program.is_editing());
    let mut result = program
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(result.is_complete());
    let candidate = result.render().unwrap();
    assert_eq!(candidate, b"\xef\xbb\xbfcounts = [4, 6]; % keep\r\nkeyed = [Zulu: 7, 'alpha beta': 9];\r\nr = (n: 1, empty: []);\r\nempty = [];\r\n\r\n% detached\r\n");
    assert_eq!(result.candidate().unwrap().source_bytes(), candidate);
    let parameter_source = format!(
        "param increment:int; {}",
        source.replace("saved[i]+1", "saved[i]+increment")
    );
    let parameter_program = Program::parse(&parameter_source, ProgramLimits::default()).unwrap();
    let arguments = [("increment".into(), zirql::ConstructedValue::Integer(1))];
    let mut parameter_result = parameter_program
        .evaluate_with_model_and_parameters(
            &input,
            Some(&model),
            zirql::ParameterInputs::Typed(&arguments),
            ProgramLimits::default(),
        )
        .unwrap();
    assert_eq!(parameter_result.render().unwrap(), candidate);
    let facts = std::rc::Rc::new(zincite_lint::ModelFacts::new(&model));
    let mut facts_result = parameter_program
        .evaluate_with_facts_and_parameters(
            &input,
            facts,
            zirql::ParameterInputs::Typed(&arguments),
            ProgramLimits::default(),
        )
        .unwrap();
    assert_eq!(facts_result.render().unwrap(), candidate);
    assert!(
        parameter_program
            .evaluate_with_model_and_parameters(
                &input,
                Some(&model),
                zirql::ParameterInputs::Typed(&[]),
                ProgramLimits::default()
            )
            .is_err()
    );
    assert_eq!(input.source_bytes(), original);

    let mut alias = Program::parse(
        &source.replace("emit;", "emit_document;"),
        ProgramLimits::default(),
    )
    .unwrap()
    .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
    .unwrap();
    assert_eq!(alias.render().unwrap(), candidate);

    let replaced_record_input = Input::parse(candidate.clone(), FileMode::Data).unwrap();
    let record = Program::parse("replace r=(n:2,empty:[]); emit;", ProgramLimits::default())
        .unwrap()
        .evaluate_with_model(
            &replaced_record_input,
            Some(&model),
            ProgramLimits::default(),
        )
        .unwrap();
    assert!(
        record
            .candidate()
            .unwrap()
            .source_bytes()
            .windows(b"r = (n: 2, empty: []);".len())
            .any(|bytes| bytes == b"r = (n: 2, empty: []);")
    );

    assert_eq!(input.source_bytes(), original);
    let empty_shape = Program::parse(
        "create empty=[(i,j):1 | i in 1..2 where false,j in 2..3]; emit;",
        ProgramLimits::default(),
    )
    .unwrap()
    .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
    .unwrap();
    assert_eq!(
        empty_shape.candidate().unwrap().source_bytes(),
        b"\xef\xbb\xbfcounts = [3, 5]; % keep\r\nempty = [];\r\n\r\n% detached\r\n"
    );
    let empty = Input::parse(Vec::new(), FileMode::Data).unwrap();
    let mut created = Program::parse(
        "create counts = [i | i in 1..2]; emit;",
        ProgramLimits::default(),
    )
    .unwrap()
    .evaluate_with_model(&empty, Some(&model), ProgramLimits::default())
    .unwrap();
    assert_eq!(created.render().unwrap(), b"counts = [1, 2];\n");
    let limited = Program::parse("create limited=[1,3]; emit;", ProgramLimits::default())
        .unwrap()
        .evaluate_with_model(&empty, Some(&model), ProgramLimits::default())
        .unwrap();
    assert_eq!(
        limited.candidate().unwrap().source_bytes(),
        b"limited = [1, 3];\n"
    );
    for request in [
        "emit;",
        "emit_document;",
        "create counts=[1,2]; emit;",
        "replace missing=1; emit;",
        "replace counts=[1]; emit;",
        "replace counts=[1,2]; create 'counts'=[1,2]; emit;",
        "report 1; replace counts=[1,2]; emit;",
        "replace counts=[1,2];",
        "replace counts=[1,2]; emit; report 1;",
        "create keyed=[1:7,2:9]; emit;",
        "create empty=[(1,2):1]; emit;",
        "create limited=[1,4]; emit;",
        "replace counts=[1,1 div 0]; emit;",
        "replace counts=[1,abs(2)]; emit;",
    ] {
        let error = Program::parse(request, ProgramLimits::default())
            .and_then(|program| {
                program.evaluate_with_model(&input, Some(&model), ProgramLimits::default())
            })
            .unwrap_err();
        assert!(error.range.start <= request.len(), "{request}: {error:?}");
    }
    let commented =
        Input::parse(b"counts=[3, /* placement */5];".to_vec(), FileMode::Data).unwrap();
    assert!(
        Program::parse("replace counts=[4,6]; emit;", ProgramLimits::default())
            .unwrap()
            .evaluate_with_model(&commented, Some(&model), ProgramLimits::default())
            .unwrap_err()
            .message
            .contains("internal comments")
    );
}

#[test]
fn data_programs_double_entities_and_validate_final_scalar_axes() {
    use std::rc::Rc;
    use zincite_query::{Program, ProgramLimits};
    let model = semantic_model(
        "data-double",
        include_bytes!("fixtures/doubled.mzn"),
        b"",
        "",
    );
    let facts = Rc::new(zincite_lint::ModelFacts::new(&model));
    let input = Input::parse_named(
        include_bytes!("fixtures/doubled.dzn").to_vec(),
        FileMode::Data,
        "double.dzn",
    )
    .unwrap();
    let program =
        Program::parse(include_str!("fixtures/double.zq"), ProgramLimits::default()).unwrap();
    let mut result = program
        .evaluate_with_facts(&input, facts.clone(), ProgramLimits::default())
        .unwrap();
    assert!(std::ptr::eq(result.model_facts().unwrap(), facts.as_ref()));
    let candidate = result.render().unwrap();
    assert_eq!(candidate, b"entity_count = 4;\ngroup_count = 2;\nentities = [(weight: 7, peer: 2, group: 1), (weight: 11, peer: 1, group: 1), (weight: 7, peer: 4, group: 2), (weight: 11, peer: 3, group: 2)];\ncapacities = [| 3, 4 | 5, 6 | 3, 4 | 5, 6 |];\ngroups = [1: {1, 2}, 2: {3, 4}];\n");
    for request in [
        "replace entity_count=4; emit;",
        "replace entity_count=4; replace entities=[(weight:1,peer:1,group:1) | i in 1..3]; emit;",
    ] {
        assert!(
            Program::parse(request, ProgramLimits::default())
                .unwrap()
                .evaluate_with_facts(&input, facts.clone(), ProgramLimits::default())
                .is_err()
        );
    }
    drop(result);
    let computed = Input::parse(
        include_str!("fixtures/doubled.dzn")
            .replace("entity_count = 2", "entity_count = 1+1")
            .into_bytes(),
        FileMode::Data,
    )
    .unwrap();
    assert!(
        program
            .evaluate_with_facts(&computed, facts, ProgramLimits::default())
            .unwrap_err()
            .message
            .contains("computed")
    );
    let axes = semantic_model(
        "data-axis-expression",
        b"int: n; array[2..n+1] of int: a; solve satisfy;",
        b"",
        "",
    );
    let data = Input::parse(b"n=2; a=[2:5,3:6];".to_vec(), FileMode::Data).unwrap();
    let request = Program::parse(r#"let a=items | filter(name("a")) | values | one; replace n=3; replace a=[i:if i=4 then 7 else a[i] endif | i in 2..4]; emit;"#, ProgramLimits::default()).unwrap();
    let mut result = request
        .evaluate_with_model(&data, Some(&axes), ProgramLimits::default())
        .unwrap();
    assert_eq!(result.render().unwrap(), b"n=3; a=[2: 5, 3: 6, 4: 7];");
}

#[test]
fn scalar_summaries_report_capacity_and_short_literal_totals() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(
        b"demand = [2,3,4]; capacity = [6,8,5];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let program = Program::parse(include_str!("fixtures/capacity.zq"), limits).unwrap();
    let mut result = program.evaluate(&input, limits).unwrap();
    assert!(result.is_complete());
    assert!(result.limitations().is_empty());
    let row = |index, demand, capacity, headroom| {
        V::Record(vec![
            ("index".into(), V::Integer(index)),
            ("demand".into(), V::Integer(demand)),
            ("capacity".into(), V::Integer(capacity)),
            ("headroom".into(), V::Integer(headroom)),
        ])
    };
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Constructed(value) if value == &V::Record(vec![
            ("rows".into(), V::Array(vec![row(1,4,6,2), row(2,6,8,2), row(3,8,5,-3)])),
            ("total_demand".into(), V::Integer(18)), ("total_capacity".into(), V::Integer(19)),
            ("totals_fit".into(), V::Boolean(true)), ("cells_fit".into(), V::Boolean(false)),
            ("violations".into(), V::Array(vec![row(3,8,5,-3)])),
        ]))
    );
    let report: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(report["complete"], true);
    assert_eq!(report["limitations"], serde_json::json!([]));
    let checker = Program::parse(include_str!("fixtures/capacity-check.zq"), limits).unwrap();
    let checked = checker.evaluate(&input, limits).unwrap();
    assert!(checked.is_complete() && checked.limitations().is_empty());
    assert!(matches!(
        checked.emissions()[0].value(),
        ReportValue::Constructed(V::Boolean(true))
    ));
    let invalid = Input::parse(
        b"demand=[2,3,4]; capacity=[6,8,3];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let error = checker.evaluate(&invalid, limits).err().unwrap();
    assert!(
        error
            .message
            .contains("each capacity must cover its nonnegative demand")
    );

    let parameter_program =
        Program::parse(include_str!("fixtures/capacity-parameters.zq"), limits).unwrap();
    let typed = [("factor".into(), V::Integer(2))];
    let raw = ["factor=+2".to_owned()];
    for inputs in [
        zirql::ParameterInputs::Typed(&typed),
        zirql::ParameterInputs::Cli(&raw),
    ] {
        let mut parameter_result = parameter_program
            .evaluate_with_parameters(&input, inputs, limits)
            .unwrap();
        assert!(parameter_result.is_complete() && parameter_result.limitations().is_empty());
        let (ReportValue::Constructed(expected), ReportValue::Constructed(actual)) = (
            result.emissions()[0].value(),
            parameter_result.emissions()[0].value(),
        ) else {
            panic!("native data")
        };
        assert_eq!(actual, expected);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&parameter_result.render().unwrap())
                .unwrap(),
            report
        );
    }

    let mut short = Program::parse(
        r#"let d = items | filter(name("demand")) | values | one; report sum(d);"#,
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    assert!(matches!(
        short.emissions()[0].value(),
        ReportValue::Constructed(V::Integer(9))
    ));
    let rendered: serde_json::Value = serde_json::from_slice(&short.render().unwrap()).unwrap();
    assert_eq!(
        rendered,
        serde_json::json!({"complete":true,"limitations":[],"result":{"kind":"integer","value":9}})
    );

    let empty = Input::parse(b"demand=[]; capacity=[];".to_vec(), FileMode::Data).unwrap();
    let empty_result = program.evaluate(&empty, limits).unwrap();
    assert!(
        matches!(empty_result.emissions()[0].value(), ReportValue::Constructed(value) if value == &V::Record(vec![
            ("rows".into(), V::Array(vec![])), ("total_demand".into(), V::Integer(0)),
            ("total_capacity".into(), V::Integer(0)), ("totals_fit".into(), V::Boolean(true)),
            ("cells_fit".into(), V::Boolean(true)), ("violations".into(), V::Array(vec![])),
        ]))
    );
}

#[test]
fn report_call_preflight_keeps_literal_messages_and_the_data_whitelist() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(b"a=[1];".to_vec(), FileMode::Data).unwrap();
    let successful = Program::parse(r#"report (n:length(["x",[1]]), total:sum([]), all:forall([]), some:exists([]), value:require(true,7,"unused"));"#, limits).unwrap().evaluate(&input, limits).unwrap();
    assert!(
        matches!(successful.emissions()[0].value(), ReportValue::Constructed(value) if value == &V::Record(vec![
            ("n".into(), V::Integer(2)), ("total".into(), V::Integer(0)), ("all".into(), V::Boolean(true)), ("some".into(), V::Boolean(false)), ("value".into(), V::Integer(7)),
        ]))
    );
    for (source, responsible) in [
        (
            r#"report require(false, (1 div 0, [1][8]), "stopped");"#,
            r#"require(false, (1 div 0, [1][8]), "stopped")"#,
        ),
        (
            "report sum([9223372036854775807,1]);",
            "sum([9223372036854775807,1])",
        ),
        (
            "report forall([if i=1 then false else (1 div 0)=0 endif | i in 1..2]);",
            "1 div 0",
        ),
    ] {
        let error = Program::parse(source, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap_err();
        assert_eq!(&source[error.range], responsible);
    }
    for source in [
        r#"report require(false, unsupported_summary([1]), "unused");"#,
        r#"report require(false, sum([true]), "unused");"#,
        r#"report require(false, missing, "unused");"#,
        r#"report require(1, 2, "unused");"#,
        r#"report require(true, 2, "computed" ++ "message");"#,
        "report require(true, 2);",
        "report sum([1],[2]);",
        "report forall(i in 1..2)(true);",
        "report group_by([], 1);",
        "report group_by([]);",
        "report entries();",
        "report entries([1:1], [1:1]);",
        "report entries([]);",
        "report zip_exact([1:1]);",
        "report zip_exact([1:1], [1:1], [1:1]);",
        "report if true then 1 else zip_exact([1:1], []) endif;",
        "report [i | i in entries(i)];",
        "report [i | i in zip_exact([1:1], i)];",
    ] {
        assert!(Program::parse(source, limits).is_err(), "{source}");
    }
    for (source, call) in [
        ("let unused = sum([1]); replace a=[2]; emit;", "sum([1])"),
        (
            r#"let unused=items | filter(name(if false then require(true,"a","hidden") else "a" endif)) | values | one; replace a=[2]; emit;"#,
            r#"require(true,"a","hidden")"#,
        ),
        (
            r#"let unused=head(sum([1])); replace a=[2]; emit;"#,
            "sum([1])",
        ),
        (
            "let unused=entries([1:1]); replace a=[2]; emit_document;",
            "entries([1:1])",
        ),
        (
            "let unused=if false then zip_exact([1:1],[1:1]) else [1:1] endif; create b=1; emit;",
            "zip_exact([1:1],[1:1])",
        ),
        ("create b=entries([1:1]); emit;", "entries([1:1])"),
        (
            "replace a=if true then [2] else zip_exact([1:1],[1:1]) endif; emit_document;",
            "zip_exact([1:1],[1:1])",
        ),
        (
            "let unused = group_by([], \"machine\"); replace a=[2]; emit;",
            "group_by([], \"machine\")",
        ),
        (
            r#"let unused = if false then require(true,1,"x") else 0 endif; create b=1; emit;"#,
            r#"require(true,1,"x")"#,
        ),
        (
            "replace a=if true then [2] else [length([1])] endif; emit;",
            "length([1])",
        ),
        (
            "replace a=if true then [2] else group_by([], \"machine\") endif; emit_document;",
            "group_by([], \"machine\")",
        ),
        (
            "let unused=if false then group_by([], \"machine\") else [] endif; create b=1; emit;",
            "group_by([], \"machine\")",
        ),
    ] {
        let error = Program::parse(source, limits).unwrap_err();
        assert_eq!(&source[error.range], call);
        assert!(error.message.contains("report-only"));
    }
}

#[test]
fn machine_groups_keep_native_rows_duplicates_and_ordered_summaries() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let source = include_bytes!("fixtures/machine-groups.dzn");
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    let mut summary = Program::parse(include_str!("fixtures/machine-groups.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let row = |machine: &str, count, total| {
        V::Record(vec![
            ("machine".into(), V::String(machine.into())),
            ("count".into(), V::Integer(count)),
            ("total".into(), V::Integer(total)),
        ])
    };
    assert!(
        matches!(summary.emissions()[0].value(), ReportValue::Constructed(value)
        if value == &V::Array(vec![row("b",3,13),row("a",1,5)]))
    );
    assert!(summary.is_complete() && summary.limitations().is_empty());
    let rendered: serde_json::Value = serde_json::from_slice(&summary.render().unwrap()).unwrap();
    assert_eq!(rendered["complete"], true);
    assert_eq!(rendered["limitations"], serde_json::json!([]));

    let groups = Program::parse(
        r#"let rows=items | filter(name("jobs")) | values | one; report group_by(rows, "machine");"#,
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    let row = |machine: &str, demand| {
        V::Record(vec![
            ("machine".into(), V::String(machine.into())),
            ("demand".into(), V::Integer(demand)),
        ])
    };
    assert!(
        matches!(groups.emissions()[0].value(), ReportValue::Constructed(value)
        if value == &V::Array(vec![
            V::Record(vec![("key".into(),V::String("b".into())),("items".into(),V::Array(vec![row("b",3),row("b",7),row("b",3)]))]),
            V::Record(vec![("key".into(),V::String("a".into())),("items".into(),V::Array(vec![row("a",5)]))]),
        ]))
    );
    for source in [
        r#"let rows=items | filter(name("jobs")) | values | one; report group_by(rows, "absent");"#,
        r#"report group_by([(machine: "b"), (machine: 1)], "machine");"#,
    ] {
        let error = Program::parse(source, limits)
            .and_then(|p| p.evaluate(&input, limits))
            .unwrap_err();
        assert_eq!(
            &source[error.range],
            if source.contains("absent") {
                "\"absent\""
            } else {
                "\"machine\""
            }
        );
    }
    assert_eq!(input.source_bytes(), source);
}

#[test]
fn typed_parameters_and_native_arguments_keep_scope_and_locations() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::{ConstructedValue as V, ParameterInputs as P};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"x=2; y=3;".to_vec(), FileMode::Data).unwrap();
    let program = Program::parse(
        "report [factor | factor in [1]]; param factor: int; report factor;",
        limits,
    )
    .unwrap();
    let supplied = [("factor".into(), V::Integer(2))];
    let result = program
        .evaluate_with_parameters(&input, P::Typed(&supplied), limits)
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Constructed(V::Array(values)) if values==&vec![V::Integer(1)])
    );
    assert!(matches!(
        result.emissions()[1].value(),
        ReportValue::Constructed(V::Integer(2))
    ));
    for source in [
        "report factor; param factor:int; report factor;",
        "param factor:int; report [factor | factor in [1]];",
        "param factor:int; let factor=2; report factor;",
    ] {
        assert!(Program::parse(source, limits).is_err(), "{source}");
    }
    let dynamic = Program::parse(r#"param selected:string; param limit:int; let n=limit+1; let names=filter(name(selected)) | head(n % ignored )
 + 0) | names; report names;"#,limits).unwrap();
    let result = dynamic
        .evaluate_with_parameters(
            &input,
            P::Typed(&[
                ("selected".into(), V::String("x".into())),
                ("limit".into(), V::Integer(0)),
            ]),
            limits,
        )
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(),ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) if matches!(&**value,zincite_query::QueryResult::Strings(names) if names==&vec!["x".to_owned()]))
    );
    let unicode = Program::parse(
        r#"param n:int; let names=items | head(n /* é ) */) | names; report names;"#,
        limits,
    )
    .unwrap();
    let unicode_result = unicode
        .evaluate_with_parameters(&input, P::Typed(&[("n".into(), V::Integer(1))]), limits)
        .unwrap();
    assert!(
        matches!(unicode_result.emissions()[0].value(), ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) if matches!(&**value,zincite_query::QueryResult::Strings(names) if names==&vec!["x".to_owned()]))
    );
    let escaped = r#"param n:int; let q=filter(name("x")) | head(/* é */ "bad"); report q;"#;
    let error = Program::parse(escaped, limits).unwrap_err();
    assert_eq!(&escaped[error.range], r#""bad""#);
    let unchosen = r#"param n:int; let q=filter(name("missing") and name(n)) | head(0); report q;"#;
    let error = Program::parse(unchosen, limits).unwrap_err();
    assert_eq!(&unchosen[error.range], "n");
    let string = Program::parse("param text:string; report text;", limits).unwrap();
    let raw = ["text=  \"x\"=\\  ".to_owned()];
    let result = string
        .evaluate_with_parameters(&input, P::Cli(&raw), limits)
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(),ReportValue::Constructed(V::String(text)) if text=="  \"x\"=\\  ")
    );
    let integer = Program::parse("param n:int; report n;", limits).unwrap();
    for raw in [
        vec![],
        vec!["n=2".into(), "n=3".into()],
        vec!["n=2".into(), "extra=0".into()],
        vec!["n=2.0".into()],
        vec!["n=9223372036854775808".into()],
        vec!["n".into()],
    ] {
        assert!(
            integer
                .evaluate_with_parameters(&input, P::Cli(&raw), limits)
                .is_err()
        );
    }
    assert!(
        integer
            .evaluate_with_parameters(&input, P::Typed(&[("n".into(), V::Boolean(true))]), limits)
            .is_err()
    );
    assert!(
        string
            .evaluate_with_parameters(
                &input,
                P::Typed(&[("text".into(), V::String("x".repeat(1000)))]),
                ProgramLimits {
                    work: 100,
                    ..limits
                }
            )
            .is_err()
    );

    let source_name = Program::parse(
        "param field:string; let data=items | filter(name(field)) | values | one; report data;",
        limits,
    )
    .unwrap();
    assert!(matches!(
        source_name
            .evaluate_with_parameters(
                &input,
                P::Typed(&[("field".into(), V::String("x".into()))]),
                limits
            )
            .unwrap()
            .emissions()[0]
            .value(),
        ReportValue::Constructed(V::Integer(2))
    ));
}

#[test]
fn native_composition_preserves_correlated_declarations_captures_and_empty_schemas() {
    use std::rc::Rc;
    use zincite_lint::ModelFacts;
    use zincite_query::{
        ConstructedValue as V, Program, ProgramLimits, ReportValue, SemanticEntity, SemanticStream,
    };
    let source = include_bytes!("fixtures/native-composition.mzn");
    let model = semantic_model("native-composition", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let facts = Rc::new(ModelFacts::new(&model));
    let program = Program::parse(
        include_str!("fixtures/native-composition.zq"),
        ProgramLimits::default(),
    )
    .unwrap();
    assert!(program.requires_model_facts());
    let mut result = program
        .evaluate_with_facts(&input, Rc::clone(&facts), ProgramLimits::default())
        .unwrap();
    assert!(result.is_complete(), "{:?}", result.limitations());
    assert!(std::ptr::eq(result.model_facts().unwrap(), facts.as_ref()));
    let ReportValue::Constructed(V::Array(rows)) = result.emissions()[0].value() else {
        panic!()
    };
    let expected = [
        ("capacity", 2),
        ("reads", 0),
        ("shadow", 0),
        ("capacity", 1),
    ];
    assert_eq!(rows.len(), expected.len(), "{rows:?}");
    for (row, (name, count)) in rows.iter().zip(expected) {
        assert_eq!(
            row,
            &V::Record(vec![
                ("name".into(), V::String(name.into())),
                ("uses".into(), V::Integer(count))
            ])
        );
    }
    let ReportValue::Captures(captures) = result.emissions()[1].value() else {
        panic!()
    };
    let report = result.semantic().unwrap();
    let mut ids = Vec::new();
    let mut written = Vec::new();
    for capture in captures.rows() {
        let Some(SemanticStream::Entities(d)) = capture.fields[0].1.stream() else {
            panic!()
        };
        let Some(SemanticStream::Entities(u)) = capture.fields[1].1.stream() else {
            panic!()
        };
        assert!(matches!(d[0], SemanticEntity::Declaration(_)));
        assert!(matches!(u[0], SemanticEntity::Reference(_)));
        ids.push(d[0]);
        written.push(report.source_bytes(u[0]));
        assert_eq!(report.location(u[0]).path, model.root);
        assert_eq!(
            report.source_bytes(u[0]),
            &source[report.location(u[0]).range.clone()]
        );
    }
    assert_eq!(ids.len(), 3);
    assert_eq!(ids[0], ids[1]);
    assert_ne!(ids[0], ids[2]);
    assert_eq!(
        written,
        [
            b"capacity".as_slice(),
            b"'capacity'".as_slice(),
            b"capacity".as_slice()
        ]
    );
    let ReportValue::Constructed(V::Array(rows)) = result.emissions()[2].value() else {
        panic!()
    };
    for (row, text) in rows.iter().zip(["capacity", "'capacity'", "capacity"]) {
        assert_eq!(
            row,
            &V::Record(vec![
                ("name".into(), V::String("capacity".into())),
                ("text".into(), V::String(text.into()))
            ])
        );
    }
    result.render().unwrap();
    let empty = Program::parse(r#"let matches = match { from d in (head(0) | declarations); }; report matches | project(m, {"name": m.d | names | one});"#, ProgramLimits::default()).unwrap();
    let result = empty
        .evaluate_with_facts(&input, Rc::clone(&facts), ProgramLimits::default())
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Constructed(V::Array(rows)) if rows.is_empty())
    );
    for (source, message) in [
        (
            r#"let matches = match { from d in (head(0) | declarations); }; report matches | project(m, {"name": m.missing | names | one});"#,
            "unknown native capture field",
        ),
        (
            r#"let d = subtree | declarations; report d | project(d, {"name": d | names | one});"#,
            "shadows",
        ),
        (
            r#"report subtree | declarations | project(d, {"name": d | names | json});"#,
            "cannot emit",
        ),
        (
            r#"let m = match { from d in (subtree | declarations); }; replace capacity=1; emit;"#,
            "report-only",
        ),
    ] {
        let error = Program::parse(source, ProgramLimits::default()).unwrap_err();
        assert!(error.message.contains(message), "{error:?}");
    }
    let error = program
        .evaluate_with_facts(
            &input,
            facts,
            ProgramLimits {
                results: 250,
                ..ProgramLimits::default()
            },
        )
        .unwrap_err();
    assert!(error.message.contains("result limit"), "{error:?}");
}

#[test]
fn native_match_unknown_guards_keep_host_locations_and_boolean_control() {
    use zincite_query::{ErrorLocation, Program, ProgramLimits};
    use zirql::ConstructedValue as V;
    let source = b"function $T: identity($T: x) = x; par tuple(int, $T): mixed; var int: v; int: result = missing(v); int: read = mixed; int: duplicate; int: duplicate; int: ambiguous = duplicate; solve satisfy;";
    let model = semantic_model("composition-unknown", source, b"", "");
    let input = Input::parse_named(source.to_vec(), FileMode::Model, "label-only.mzn").unwrap();
    let known = Program::parse(r#"let m = match { from d in (subtree | declarations | unique | filter(name("mixed"))); where type(d, "tuple") or name(d, "mixed"); }; let rows = m | project(r, {"name": r.d | names | one}); report group_by(rows, "name");"#, ProgramLimits::default()).unwrap();
    let mut result = known
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(!result.is_complete());
    assert!(!result.limitations().is_empty());
    assert!(
        matches!(result.emissions()[0].value(), zincite_query::ReportValue::Constructed(V::Array(groups)) if groups.len()==1)
    );
    let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(json["complete"], false);
    assert_eq!(
        json["result"]["entries"][0]["value"]["key"]["value"],
        "mixed"
    );
    let unknown = Program::parse(r#"report match { from d in (subtree | declarations | unique | filter(name("mixed"))); where not type(d, "tuple"); };"#, ProgramLimits::default()).unwrap();
    let error = unknown
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap_err();
    let ErrorLocation::Model(location) = error.location else {
        panic!("{error:?}")
    };
    assert_eq!(location.path, model.root);
    assert_eq!(&source[location.range], b"mixed");
    assert!(error.message.contains("unresolved"), "{}", error.message);
    let incomplete_source = b"int: value = missing; solve satisfy;";
    let incomplete_model = semantic_model("composition-incomplete", incomplete_source, b"", "");
    let input = Input::parse(incomplete_source.to_vec(), FileMode::Model).unwrap();
    let program = Program::parse(r#"let unresolved = references | declarations | names | count; report match { from d in (subtree | declarations | unique); }; report subtree | declarations | unique | project(d, {"name": d | names | one});"#, ProgramLimits::default()).unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&incomplete_model), ProgramLimits::default())
        .unwrap();
    assert!(!result.is_complete());
    assert!(!result.limitations().is_empty());
    assert!(
        String::from_utf8(result.render().unwrap())
            .unwrap()
            .contains("\"complete\":false")
    );
}

#[test]
fn bounded_paths_keep_declaration_witnesses_owner_coordinates_and_projection() {
    use std::rc::Rc;
    use zincite_lint::ModelFacts;
    use zincite_query::{Program, ProgramLimits, ReportValue, SemanticEntity, SemanticStream};
    assert!(Program::parse(r#"subtree | references | filter(name("missing")) | paths(referenced_declarations, to=name("c"), max_depth=0) | json"#, ProgramLimits::default()).is_err());
    let source = include_bytes!("fixtures/bounded-paths.mzn");
    let model = semantic_model("bounded-paths", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let facts = Rc::new(ModelFacts::new(&model));
    let short = include_str!("fixtures/bounded-paths.zq");
    let program = Program::parse(short, ProgramLimits::default()).unwrap();
    let mut result = program
        .evaluate_with_facts(&input, Rc::clone(&facts), ProgramLimits::default())
        .unwrap();
    assert!(result.is_complete());
    assert!(std::ptr::eq(result.model_facts().unwrap(), facts.as_ref()));
    let ReportValue::Captures(paths) = result.emissions()[0].value() else {
        panic!("native paths")
    };
    assert_eq!(paths.rows().len(), 1);
    let fields = &paths.rows()[0].fields;
    assert_eq!(
        fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["source", "sink", "steps"]
    );
    let native = |index: usize| {
        let Some(SemanticStream::Entities(ids)) = fields[index].1.stream() else {
            panic!("native declarations")
        };
        ids.as_slice()
    };
    assert_eq!(native(0), &native(2)[..1]);
    assert_eq!(native(1), &native(2)[2..]);
    let report = result.semantic().unwrap();
    for (entity, name) in native(2).iter().zip([b"a", b"b", b"c"]) {
        assert!(matches!(entity, SemanticEntity::Declaration(_)));
        let location = report.location(*entity);
        assert_eq!(location.path, model.root);
        assert_eq!(&source[location.range.clone()], name);
    }
    let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(
        json["result"][0]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    let one = Program::parse(
        &short.replace("max_depth=2", "max_depth=1"),
        ProgramLimits::default(),
    )
    .unwrap();
    let result = one
        .evaluate_with_facts(&input, Rc::clone(&facts), ProgramLimits::default())
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Captures(paths) if paths.rows().is_empty())
    );
    let projection = Program::parse(r#"let roots = subtree | declarations | filter(name("a")); let p = roots | paths(referenced_declarations, to=name("c"), max_depth=2); report p | project(w, {"source": w.source | names | one, "sink": w.sink | names | one, "steps": w.steps | names});"#, ProgramLimits::default()).unwrap();
    let mut result = projection
        .evaluate_with_facts(&input, facts, ProgramLimits::default())
        .unwrap();
    assert!(
        String::from_utf8(result.render().unwrap())
            .unwrap()
            .contains("\"value\":\"b\"")
    );
    assert_eq!(input.source_bytes(), source);
}

#[test]
fn bounded_paths_keep_proven_edge_order_cycles_incompleteness_and_unknown_refusals() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    let source =
        b"int: c = a; int: b = c; int: a = (b + c + b); int: unresolved = missing; solve satisfy;";
    let model = semantic_model("path-order", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let program = Program::parse(r#"subtree | declarations | filter(name("a")) | paths(referenced_declarations, to=name("c") or name("a"), max_depth=10) | json"#, ProgramLimits::default()).unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(json["result"].as_array().unwrap().len(), 2);
    assert_eq!(json["result"][0]["steps"].as_array().unwrap().len(), 1);
    assert_eq!(
        json["result"][1]["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "c"]
    );
    let incomplete = Program::parse(r#"subtree | declarations | filter(name("unresolved")) | paths(referenced_declarations, to=name("c"), max_depth=2) | json"#, ProgramLimits::default()).unwrap();
    let result = incomplete
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(!result.is_complete());
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Captures(paths) if paths.rows().is_empty())
    );
    assert!(
        result
            .limitations()
            .iter()
            .any(|d| &source[d.location.range.clone()] == b"missing")
    );
    let source = b"function $T: identity($T: x) = x; par tuple(int, $T): mixed; solve satisfy;";
    let model = semantic_model("path-unknown", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let program = Program::parse(r#"subtree | declarations | filter(name("mixed")) | paths(referenced_declarations, to=not type("tuple"), max_depth=0) | json"#, ProgramLimits::default()).unwrap();
    let error = program
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap_err();
    let ErrorLocation::Model(location) = error.location else {
        panic!("located unknown")
    };
    assert_eq!(location.path, model.root);
    assert_eq!(&source[location.range], b"mixed");
    let known = Program::parse(r#"subtree | declarations | filter(name("mixed")) | paths(referenced_declarations, to=type("tuple") or name("mixed"), max_depth=0) | json"#, ProgramLimits::default()).unwrap();
    let result = known
        .evaluate_with_model(&input, Some(&model), ProgramLimits::default())
        .unwrap();
    assert!(!result.is_complete());
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Captures(paths) if paths.rows().len()==1)
    );
}

#[test]
fn optional_reports_keep_model_contracts_sparse_membership_and_lazy_projection() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let model = semantic_model(
        "optional-report",
        include_bytes!("fixtures/optional-report.mzn"),
        b"",
        "",
    );
    let input = Input::parse(
        include_bytes!("fixtures/optional-report.dzn").to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let mut result = Program::parse(include_str!("fixtures/optional-report.zq"), limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("constructed report")
    };
    assert_eq!(fields[0].1.data(), &V::Integer(4));
    assert_eq!(fields[1].1.data(), &V::Boolean(true));
    assert_eq!(fields[2].1.data(), &V::Boolean(true));
    assert_eq!(fields[3].1.data(), &V::Boolean(false));
    assert_eq!(fields[4].1.data(), &V::Boolean(true));
    assert_eq!(fields[5].1.data(), &V::Boolean(false));
    assert_eq!(fields[6].1.data(), &V::Integer(8));
    assert_eq!(fields[7].1.data(), &V::Integer(0));
    assert_eq!(fields[8].1.data(), &V::Boolean(false));
    assert_eq!(fields[9].1.data(), &V::Array(vec![]));
    assert_eq!(fields[10].1.data(), &V::Integer(7));
    assert_eq!(fields[11].1.data(), &V::Integer(1));
    let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(json["result"]["info"].as_object().unwrap().len(), 2);
    assert_eq!(json["result"]["info"]["stored"]["kind"], "absent");
    assert_eq!(
        json["result"]["cells"]["entries"].as_array().unwrap().len(),
        3
    );
    for source in [
        r#"let info=items | filter(name("info")) | values | one; report lookup(info,"outside");"#,
        r#"let info=items | filter(name("info")) | values | one; report lookup(info,"missing") default true;"#,
        r#"let cells=items | filter(name("cells")) | values | one; report cells[2,2];"#,
        r#"let cells=items | filter(name("cells")) | values | one; report lookup(cells,1);"#,
    ] {
        assert!(
            Program::parse(source, limits)
                .unwrap()
                .evaluate_with_model(&input, Some(&model), limits)
                .is_err(),
            "{source}"
        );
    }
    let projected = Program::parse(r#"let info=items | filter(name("info")) | values | one; report items | project(d, {"fallback": lookup(info,"missing") default 8, "stored": <>, "native": d | count});"#, limits).unwrap()
        .evaluate_with_model(&input, Some(&model), limits).unwrap();
    let ReportValue::Constructed(V::Array(rows)) = projected.emissions()[0].value() else {
        panic!("shared projection")
    };
    assert_eq!(rows.len(), 2);
    let V::Record(fields) = &rows[0] else {
        panic!("projected row")
    };
    assert_eq!(fields[0].1.data(), &V::Integer(8));
    assert_eq!(fields[1].1.data(), &V::Absent);
    assert_eq!(fields[2].1.data(), &V::Integer(1));
    for value in [
        "info=(stored:<>);",
        "info=(id:4,extra:1);",
        "info=(id:4,stored:true);",
    ] {
        let invalid = Input::parse(value.as_bytes().to_vec(), FileMode::Data).unwrap();
        let program = Program::parse(
            r#"let info=items | filter(name("info")) | values | one; report info;"#,
            limits,
        )
        .unwrap();
        let error = program
            .evaluate_with_model(&invalid, Some(&model), limits)
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.range.end <= value.len());
    }
    // The written fallback member is not resolved until absence selects it.
    let skipped = Program::parse(
        r#"report 7 default enum_member(Missing,"unavailable");"#,
        limits,
    )
    .unwrap();
    assert!(skipped.evaluate(&input, limits).is_ok());
    let selected = Program::parse(
        r#"report <> default enum_member(Missing,"unavailable");"#,
        limits,
    )
    .unwrap();
    assert!(selected.evaluate(&input, limits).is_err());
    let source = format!("values=[{}];", vec!["1"; 2000].join(","));
    let large = Input::parse(source.into_bytes(), FileMode::Data).unwrap();
    let skipped = Program::parse(r#"let rows=values; report [] default rows;"#, limits).unwrap();
    let selected = Program::parse(r#"let rows=values; report <> default rows;"#, limits).unwrap();
    let tight = ProgramLimits {
        work: 30_000,
        results: 20_000,
        ..limits
    };
    assert!(skipped.evaluate(&large, tight).is_ok());
    assert!(selected.evaluate(&large, tight).is_err());
    for source in [
        "let unused=<>; replace cells=[1]; emit;",
        "let unused=7 default 9; replace cells=[1]; emit;",
        "replace cells=[<>]; emit;",
    ] {
        assert!(Program::parse(source, limits).is_err(), "{source}");
    }
}

#[test]
fn source_declarations_preserve_nested_array_representation() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::{ConstructedValue as V, ValueKey};
    let limits = ProgramLimits::default();
    let model = semantic_model("nested-source-contract", b"array[1..2] of record(array[int] of int: xs): rows; record(array[int] of int: xs): flat; array[1..2] of opt record(array[int] of int: xs): optional_rows; solve satisfy;", b"", "");
    let source = r#"let rows=items | filter(name("rows")) | values | one; let flat=items | filter(name("flat")) | values | one; let optional_rows=items | filter(name("optional_rows")) | values | one; report (nested:rows[1], flat:flat, empty:rows[2], optional:lookup(optional_rows,2), missing:lookup(optional_rows,1));"#;
    let program = Program::parse(source, limits).unwrap();
    let input = Input::parse(
        b"rows=[(xs:[1,2]),(xs:[])]; flat=(xs:[1,2]); optional_rows=[<>,(xs:[1,2])];".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("source report")
    };
    assert_eq!(fields[0].1.data(), fields[1].1.data());
    assert_eq!(fields[0].1.data(), fields[3].1.data());
    let V::Record(record) = fields[0].1.data() else {
        panic!("nested record")
    };
    assert_eq!(
        record[0].1.data(),
        &V::Array(vec![V::Integer(1), V::Integer(2)])
    );
    let V::Record(empty) = fields[2].1.data() else {
        panic!("empty nested record")
    };
    assert_eq!(empty[0].1.data(), &V::Array(vec![]));
    assert_eq!(fields[4].1.data(), &V::Absent);
    let keyed = Input::parse(b"rows=[(xs:[3:7,4:8]),(xs:[3:9])]; flat=(xs:[3:7,4:8]); optional_rows=[<>,(xs:[3:7,4:8])];".to_vec(), FileMode::Data).unwrap();
    let result = program
        .evaluate_with_model(&keyed, Some(&model), limits)
        .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("keyed source report")
    };
    assert_eq!(fields[0].1.data(), fields[1].1.data());
    assert_eq!(fields[0].1.data(), fields[3].1.data());
    let V::Record(record) = fields[0].1.data() else {
        panic!("keyed nested record")
    };
    let V::KeyedArray { axes, entries } = record[0].1.data() else {
        panic!("retained keyed representation")
    };
    assert_eq!(axes[0].keys, [ValueKey::Integer(3), ValueKey::Integer(4)]);
    assert_eq!(entries[1].value.data(), &V::Integer(8));
    let mixed = Input::parse(
        b"rows=[(xs:[1,2]),(xs:[3:7,4:8])]; flat=(xs:[3:7,4:8]); optional_rows=[<>,(xs:[1,2])];"
            .to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let result = program
        .evaluate_with_model(&mixed, Some(&model), limits)
        .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("mixed source report")
    };
    assert_eq!(fields[0].1.data(), fields[3].1.data());
    assert_eq!(fields[1].1.data(), fields[2].1.data());
    let V::Record(positional) = fields[0].1.data() else {
        panic!("positional sibling")
    };
    assert!(matches!(positional[0].1.data(), V::Array(_)));
    let V::Record(keyed) = fields[2].1.data() else {
        panic!("keyed sibling")
    };
    assert!(matches!(keyed[0].1.data(), V::KeyedArray { .. }));
    // The model schema still rejects a missing/extra field or wrong nested element.
    let admission = Program::parse(
        r#"let rows=items | filter(name("rows")) | values | one; report rows;"#,
        limits,
    )
    .unwrap();
    for data in [
        "rows=[(xs:[1,2]),(xs:[3:true])];",
        "rows=[(xs:[1,2]),(wrong:[3:7])];",
    ] {
        let input = Input::parse(data.as_bytes().to_vec(), FileMode::Data).unwrap();
        let error = admission
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap_err();
        assert_eq!(error.location, ErrorLocation::Input);
        assert!(error.range.end <= data.len());
    }
}

#[test]
fn prepared_native_arguments_keep_selected_members_and_correlated_rows() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(b"x=2; y=3;".to_vec(), FileMode::Data).unwrap();
    let source = r#"let limits=[1]; report items | project(d, {
        "name": d | names | one,
        "n": d | head(length(limits) default (if enum_member(Missing,"a") = enum_member(Missing,"a") then 1 else 0 endif)) | count
    });"#;
    let result = Program::parse(source, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(V::Array(rows)) = result.emissions()[0].value() else {
        panic!("constructed rows");
    };
    assert_eq!(
        rows,
        &vec![
            V::Record(vec![
                ("name".into(), V::String("x".into())),
                ("n".into(), V::Integer(1))
            ]),
            V::Record(vec![
                ("name".into(), V::String("y".into())),
                ("n".into(), V::Integer(1))
            ])
        ]
    );
    let eager = source.replace(
        "length(limits) default (if",
        "(if true then length(limits) elseif",
    );
    assert!(
        Program::parse(&eager, limits)
            .unwrap()
            .evaluate(&input, limits)
            .is_err()
    );
    let selected = source.replace("length(limits) default", "<> default");
    assert!(
        Program::parse(&selected, limits)
            .unwrap()
            .evaluate(&input, limits)
            .is_err()
    );
}

#[test]
fn typed_query_functions_share_native_trees_tail_frames_and_fresh_requests() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let input = Input::parse(
        include_bytes!("fixtures/recursive-functions.mzn").to_vec(),
        FileMode::Model,
    )
    .unwrap();
    let limits = ProgramLimits::default();
    let program = Program::parse(include_str!("fixtures/recursive-functions.zq"), limits).unwrap();
    let result = program.evaluate(&input, limits).unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("constructed function report")
    };
    assert_eq!(
        fields
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["sum_calls", "accumulated", "is_even"]
    );
    assert_eq!(fields[0].1.data(), &V::Integer(2));
    assert_eq!(fields[1].1.data(), &V::Integer(500500));
    assert_eq!(fields[2].1.data(), &V::Boolean(true));
    let source = r#"
        fn walk(n: int, node: Node) -> string {
            let kind = node_kind(node);
            if n = 0 then kind else walk(n - 1, node) endif
        }
        fn number(n: int) -> int { n }
        fn even(n: int) -> bool { if n = 0 then true else odd(n - 1) endif }
        fn odd(n: int) -> bool { if n = 0 then false else even(n - 1) endif }
        let roots = items;
        report roots | project(root, {"kind": walk(100, root), "even": even(1000), "direct":node_kind(root), "count":number(root | count)});
    "#;
    let program = Program::parse(source, limits)
        .unwrap()
        .with_function_limits(zirql::FunctionLimits { active_depth: 1 });
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 2000,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work"), "{error:?}");
    let fresh = Input::parse(
        include_bytes!("fixtures/recursive-functions.mzn").to_vec(),
        FileMode::Model,
    )
    .unwrap();
    let mut result = program.evaluate(&fresh, limits).unwrap();
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(output.contains("declaration"));
    assert!(output.contains("true"));
    let bom = Input::parse_named(
        [b"\xef\xbb\xbf".as_slice(), input.source_bytes()].concat(),
        FileMode::Model,
        "calls.mzn",
    )
    .unwrap();
    let mut bom_result = Program::parse(include_str!("fixtures/recursive-functions.zq"), limits)
        .unwrap()
        .evaluate(&bom, limits)
        .unwrap();
    assert!(
        String::from_utf8(bom_result.render().unwrap())
            .unwrap()
            .contains("500500")
    );
    let aliases=Program::parse("fn identity(n: int) -> int { n } let n=items | count; let alias=n; report (value:identity(alias), unchanged:identity(n));",limits).unwrap();
    aliases.evaluate(&input, limits).unwrap();
    let native_alias=Program::parse("fn keep(xs:sequence<Node>)->sequence<Node> { xs } let xs=items; report keep(xs) | count; report xs | count;",limits).unwrap();
    native_alias.evaluate(&input, limits).unwrap();
    let common=Program::parse("fn countdown(n: int) -> int { if n == 0 { 0 } else { countdown(n - 1) } } report countdown(1000);",limits).unwrap().with_function_limits(zirql::FunctionLimits{active_depth:1});
    common.evaluate(&input, limits).unwrap();
    for source in [
        "fn f(n:int)->int { if n in {1,2} then 1 else 0 endif } report f(1);",
        r#"fn f(s:string)->int { if s = "{" then 1 else 0 endif } report f("{");"#,
        "fn f(n:int)->int { if /* { */ n = 1 then 1 else 0 endif } report f(1);",
        "fn f(n:int)->int { if n == 1 { 1 } else { 0 } } report f(1);",
    ] {
        let mut result = Program::parse(source, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let output: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
        assert_eq!(output["result"]["value"], 1, "{source}");
    }
}

#[test]
fn typed_query_functions_refuse_written_scope_purity_kinds_and_limits() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"int: value = 1; solve satisfy;".to_vec(), FileMode::Model).unwrap();
    for source in [
        "fn wrong(x: int) -> int { x } report wrong(\"x\");",
        "fn wrong() -> int { \"x\" } report wrong();",
        "fn wrong(node: Node) -> int { 1 } let roots=items; report wrong(roots);",
        "fn wrong(nodes: sequence<Declaration>) -> int { 1 } let empty=head(0); report wrong(empty);",
        "fn unused() -> int { missing() } report 1;",
        "fn unused() -> int { length(direct_children(1)) } report 1;",
        "fn unused() -> int { length(direct_children([])) } report 1;",
        "fn unused() -> int { if false then length(direct_children(1)) else 1 endif } report 1;",
        "fn floating() -> float { 1.0 } report items | head(floating());",
        "fn identity(n:int)->int { n } let unused=items | filter(name(7)) | values | one; report 1;",
        "fn unused() -> int { if false then missing() else 1 endif } report 1;",
        "let outer=7; fn unused() -> int { outer } report 1;",
        "fn unused(x: int) -> int { let x=2; x } report 1;",
        "fn unused() -> int { emit() } report 1;",
        "fn wrong(x: int) -> int { x } report wrong();",
        "fn count() -> int { 1 } report 1;",
        "fn same() -> int { 1 } fn same() -> int { 2 } report same();",
    ] {
        let result =
            Program::parse(source, limits).and_then(|program| program.evaluate(&input, limits));
        assert!(result.is_err(), "accepted {source}");
    }
    let deep = Program::parse(
        "fn deep(n: int) -> int { if n = 0 then 0 else 1 + deep(n - 1) endif } report deep(20);",
        limits,
    )
    .unwrap()
    .with_function_limits(zirql::FunctionLimits { active_depth: 1 });
    let error = deep.evaluate(&input, limits).unwrap_err();
    assert!(error.message.contains("active function depth"));
    assert!(error.message.contains("in deep"));
    for body in [
        "again(n)",
        "if n = 0 then 1 else again(n - 1) + again(n - 1) endif",
    ] {
        let source = format!("fn again(n: int) -> int {{ {body} }} report again(30);");
        let program = Program::parse(&source, limits).unwrap();
        let error = program
            .evaluate(
                &input,
                ProgramLimits {
                    work: 10000,
                    ..limits
                },
            )
            .unwrap_err();
        assert!(error.message.contains("work"), "{error:?}");
    }
    let data = Program::parse("fn unused() -> int { 1 } create x=1; emit;", limits).unwrap_err();
    assert!(data.message.contains("functions"));
}

#[test]
fn typed_query_signatures_resolve_current_enum_identity_and_lazy_defaults() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let model = semantic_model(
        "function-enums",
        b"enum Colour={Red,Blue}; enum Shade={Dark}; solve satisfy;",
        b"",
        "",
    );
    let input = Input::parse(b"value=1;".to_vec(), FileMode::Data).unwrap();
    let source = r#"fn identity(colour: Colour) -> Colour { colour } fn optional(value: opt int) -> int { value default (1 div 0) } report (member:identity(enum_member(Colour,"Red")), saved:optional(7));"#;
    let result = Program::parse(source, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let ReportValue::Constructed(V::Record(fields)) = result.emissions()[0].value() else {
        panic!("enum report")
    };
    let V::Member(member) = fields[0].1.data() else {
        panic!("nominal member")
    };
    assert_eq!(member.member_name, "Red");
    assert_eq!(fields[1].1.data(), &V::Integer(7));
    let nested=Program::parse(r#"fn groups(xs:sequence<set<Colour>>)->sequence<set<Colour>> { xs } report groups([{enum_member(Colour,"Red")}]);"#,limits).unwrap();
    nested
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let wrong_nested=Program::parse(r#"fn groups(xs:sequence<set<Colour>>)->sequence<set<Colour>> { xs } report groups([{enum_member(Shade,"Dark")}]);"#,limits).unwrap();
    assert!(
        wrong_nested
            .evaluate_with_model(&input, Some(&model), limits)
            .is_err()
    );
    let wrapped = Program::parse(
        r#"fn limit(n:int)->int { n } let selected=head(limit(1)); report selected;"#,
        limits,
    )
    .unwrap();
    wrapped.evaluate(&input, limits).unwrap();
    let wrong=Program::parse(r#"fn identity(colour: Colour) -> Colour { colour } report identity(enum_member(Shade,"Dark"));"#,limits).unwrap();
    let error = wrong
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap_err();
    assert!(error.message.contains("nominal"));
    let lazy=Program::parse(r#"fn present(value: opt int) -> int { value default enum_member(Missing,"no") } report present(7);"#,limits).unwrap();
    lazy.evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
}

#[test]
fn typed_query_functions_keep_active_match_guards_paths_and_native_records() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let source = b"int: a=b; int: b=1; int: mixed=missing; solve satisfy;";
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let model = semantic_model("function-native-clauses", source, b"", "");
    let fields = "fn f()->record{fixpoint:int,where_exists:int}{let x=(fixpoint:1,where_exists:2); (fixpoint:x.fixpoint,where_exists:x.where_exists)} f()";
    let mut result = Program::parse(fields, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let output: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    assert_eq!(
        output["result"],
        serde_json::json!({
            "fixpoint": {"kind":"integer", "value":1},
            "where_exists": {"kind":"integer", "value":2}
        })
    );
    let empty_reference=Program::parse("fn accept(xs:sequence<Declaration>)->int {0} let empty=subtree | references | head(0); report accept(empty);",limits).unwrap();
    let error = empty_reference
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap_err();
    assert!(error.message.contains("element contract"), "{error:?}");
    let empty_declaration=Program::parse("fn accept(xs:sequence<Declaration>)->int {0} let empty=subtree | declarations | tail(0) | reverse | check(0); report accept(empty);",limits).unwrap();
    empty_declaration
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let written = r#"
        fn same(node: Node) -> Node { node }
        fn label(value: string) -> string { value }
        let matches=match { from first in items; bind second=same(first); where name(second,label("a")); };
        report matches | project(row,{"name":row.second | names | one});
        report subtree | declarations | filter(name(label("a"))) | paths(referenced_declarations,to=name(label("b")),max_depth=1);
    "#;
    let program = Program::parse(written, limits).unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(output.contains("\"value\":\"a\""), "{output}");
    assert!(output.contains("\"name\":\"b\""), "{output}");
    let recursive = r#"
        fn same(node: Node) -> Node { node }
        fn select(node: Node) -> sequence<record{chosen: Node}> { match { bind chosen=same(node); } }
        let roots=items | head(1); report roots | project(root,{"chosen":select(root) | project(pair,{"name":pair.chosen | names | one})});
    "#;
    let program = Program::parse(recursive, limits).unwrap();
    program.evaluate(&input, limits).unwrap();
    let shallow = program.with_function_limits(zirql::FunctionLimits { active_depth: 1 });
    let error = shallow.evaluate(&input, limits).unwrap_err();
    assert!(error.message.contains("active function depth"), "{error:?}");
    for replacement in [
        "where name(second,label(1));",
        "where type(second,label(\"tuple\"));",
    ] {
        let source = written.replace("where name(second,label(\"a\"));", replacement);
        assert!(
            Program::parse(&source, limits)
                .unwrap()
                .evaluate_with_model(&input, Some(&model), limits)
                .is_err()
        );
    }
    let lazy = written.replace(
        "where name(second,label(\"a\"));",
        "where name(second,\"a\") or name(second,label(1 div 0));",
    );
    // The invalid call signature is still rejected in an unselected Boolean arm.
    assert!(
        Program::parse(&lazy, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .is_err()
    );
}

#[test]
fn typed_query_path_targets_retain_unknown_and_selected_boolean_laziness() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let bytes = b"function $T: identity($T: x) = x; par tuple(int, $T): mixed; solve satisfy;";
    let model = semantic_model("function-unknown-target", bytes, b"", "");
    let input = Input::parse(bytes.to_vec(), FileMode::Model).unwrap();
    let prefix = "fn label(value: string) -> string { value } fn skipped() -> string { if 1 div 0 = 0 then \"a\" else \"b\" endif } ";
    let selected = r#"subtree | declarations | filter(name(label("mixed"))) | paths(referenced_declarations,to=type(label("tuple")) or name(label("mixed")) or name(skipped()),max_depth=0) | json"#;
    let mut result = Program::parse(&format!("{prefix}{selected}"), limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert!(!result.is_complete());
    assert!(!result.limitations().is_empty());
    assert!(
        String::from_utf8(result.render().unwrap())
            .unwrap()
            .contains("mixed")
    );
    let unknown = selected.replace(
        "type(label(\"tuple\")) or name(label(\"mixed\")) or name(skipped())",
        "not type(label(\"tuple\"))",
    );
    let error = Program::parse(&format!("{prefix}{unknown}"), limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap_err();
    let ErrorLocation::Model(location) = error.location else {
        panic!("located Unknown")
    };
    assert_eq!(&bytes[location.range], b"mixed");
}

#[test]
fn exact_string_matching_filters_projected_names_and_keeps_data_closed() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(
        include_bytes!("fixtures/string-matching.mzn").to_vec(),
        FileMode::Model,
    )
    .unwrap();
    let result = Program::parse(include_str!("fixtures/string-matching.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Constructed(value) if value == &V::Record(vec![
            ("names".into(), V::Array(vec![V::String("work_load".into())])),
            ("controls".into(), V::Array(vec![true, false, true, false, true].into_iter().map(V::Boolean).collect())),
        ]))
    );
    let bad = r#"report if false then contains("x", 1) else false endif;"#;
    let error = Program::parse(bad, limits).unwrap_err();
    assert_eq!(&bad[error.range], r#"contains("x", 1)"#);
    for source in [
        r#"let unused=contains("x","x"); create b=1; emit;"#,
        r#"replace a=if true then 2 else starts_with("x","x") endif; emit;"#,
        r#"let unused=if false then ends_with("x","") else false endif; create b=1; emit;"#,
    ] {
        assert!(
            Program::parse(source, limits)
                .unwrap_err()
                .message
                .contains("report-only")
        );
    }
    let function = Program::parse(
        r#"fn has_part(text:string)->bool { contains(text,"str") } report has_part("Ångström");"#,
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    assert!(
        matches!(function.emissions()[0].value(), ReportValue::Constructed(value) if value.data() == &V::Boolean(true))
    );
    assert!(
        Program::parse(
            "fn contains(text:string)->bool { true } report true;",
            limits
        )
        .unwrap()
        .evaluate(&input, limits)
        .is_err()
    );
    let long = "a".repeat(4_000);
    let tight = ProgramLimits {
        work: 20_000,
        ..limits
    };
    let prefix = Program::parse(&format!("report starts_with(\"{long}\",\"a\");"), limits).unwrap();
    prefix.evaluate(&input, tight).unwrap();
    let scan = Program::parse(&format!("report contains(\"{long}\",\"z\");"), limits).unwrap();
    assert!(
        scan.evaluate(&input, tight)
            .unwrap_err()
            .message
            .contains("work")
    );
}

#[test]
fn report_ordering_keeps_rows_ties_optional_results_and_selector_ranges() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(b"a=1; b=2;".to_vec(), FileMode::Data).unwrap();
    let result = Program::parse(include_str!("fixtures/report-ordering.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let row = |id: &str, score| {
        V::Record(vec![
            ("id".into(), V::String(id.into())),
            ("score".into(), V::Integer(score)),
        ])
    };
    let expected = V::Record(vec![
        (
            "ascending".into(),
            V::Array(vec![row("B", 1), row("D", 1), row("A", 2), row("C", 2)]),
        ),
        (
            "descending".into(),
            V::Array(vec![row("A", 2), row("C", 2), row("B", 1), row("D", 1)]),
        ),
        ("minimum".into(), row("B", 1)),
        ("maximum".into(), row("A", 2)),
        (
            "scalar".into(),
            V::Array(vec![
                V::Integer(1),
                V::Integer(1),
                V::Integer(2),
                V::Integer(2),
            ]),
        ),
        ("scalar_min".into(), V::Integer(1)),
        ("scalar_max".into(), V::Integer(2)),
        ("empty_min".into(), V::Absent),
        ("empty_max".into(), V::Absent),
        ("fallback".into(), V::Integer(7)),
    ]);
    assert!(
        matches!(result.emissions()[0].value(),ReportValue::Constructed(value) if value==&expected)
    );
    let dictionary_sorts = r#"
        let key="pear";
        let dictionary={key:2,"ash":1,"elm":2};
        report (
            literal:sort_by({"pear":2,"ash":1},value),
            computed:sort_by({key:2,"ash":1},value),
            saved:sort_by(dictionary,value),
            descending:sort_by(dictionary,value,descending=true),
            cost:sort_by({"pear":{"cost":2},"ash":{"cost":1}},"cost"),
            quoted:sort_by({"pear":{"value":2},"ash":{"value":1}},"value")
        );"#;
    let result = Program::parse(dictionary_sorts, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(value) = result.emissions()[0].value() else {
        panic!("constructed dictionary sorts")
    };
    let V::Record(fields) = value.data() else {
        panic!("dictionary sort report")
    };
    let order = |value: &V| {
        let V::Record(entries) = value.data() else {
            panic!("same-kind dictionary")
        };
        entries
            .iter()
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(order(&fields[0].1), ["ash", "pear"]);
    assert_eq!(order(&fields[1].1), ["ash", "pear"]);
    assert_eq!(order(&fields[2].1), ["ash", "pear", "elm"]);
    assert_eq!(order(&fields[3].1), ["pear", "elm", "ash"]);
    assert_eq!(order(&fields[4].1), ["ash", "pear"]);
    assert_eq!(order(&fields[5].1), ["ash", "pear"]);
    let quoted_failure = r#"sort_by({"pear":2,"ash":1}, "value")"#;
    let error = Program::parse(quoted_failure, limits)
        .and_then(|p| p.evaluate(&input, limits))
        .unwrap_err();
    assert_eq!(&quoted_failure[error.range], "\"value\"");
    assert!(error.message.contains("record rows"));
    for source in [
        r#"report min_by([(score: 1), (other: 2)], "score");"#,
        r#"report max_by([(score: 1), (score: <>)], "score");"#,
        r#"report sort_by([(score: 1), (score: "2")], "score");"#,
    ] {
        let failure = Program::parse(source, limits)
            .and_then(|p| p.evaluate(&input, limits))
            .unwrap_err();
        assert_eq!(&source[failure.range], "\"score\"");
    }
    for source in [
        "report sort([1], descending=1);",
        "report sort([1], descending=true, descending=false);",
        "report sort([1], other=true);",
        "report min([1], descending=true);",
        "report sort([1, 2.0]);",
    ] {
        assert!(
            Program::parse(source, limits)
                .and_then(|p| p.evaluate(&input, limits))
                .is_err(),
            "{source}"
        );
    }
    let defaults=Program::parse(r#"report min_by([(id:1, score: lookup((id:1), "score") default 0), (id:2, score:2)], "score");"#,limits).unwrap().evaluate(&input,limits).unwrap();
    assert!(
        matches!(defaults.emissions()[0].value(), ReportValue::Constructed(value) if value==&V::Record(vec![("id".into(),V::Integer(1)),("score".into(),V::Integer(0))]))
    );
    let prepared=Program::parse("fn bound(n:int)->int { max(sort([n,1], descending=false)) default 0 } report items | head(bound(2)) | count;",limits).unwrap().evaluate(&input,limits).unwrap();
    assert!(
        matches!(prepared.emissions()[0].value(),ReportValue::Constructed(value) if value.data()==&V::Integer(2))
    );
    let model = semantic_model(
        "ordering-enums",
        b"enum Colour={Red,Blue}; enum Shade={Dark}; solve satisfy;",
        b"",
        "",
    );
    let nominal = Program::parse(
        r#"report sort([enum_member(Colour,"Blue"),enum_member(Colour,"Red")]);"#,
        limits,
    )
    .unwrap()
    .evaluate_with_model(&input, Some(&model), limits)
    .unwrap();
    let ReportValue::Constructed(V::Array(values)) = nominal.emissions()[0].value() else {
        panic!("nominal array")
    };
    let (V::Member(first), V::Member(second)) = (&values[0], &values[1]) else {
        panic!("nominal members")
    };
    assert_eq!(
        (first.member_name.as_str(), second.member_name.as_str()),
        ("Red", "Blue")
    );
    assert_eq!(first.type_id, second.type_id);
    assert_ne!(first.member_id, second.member_id);
    let source = r#"report min_by([(score:enum_member(Colour,"Red")),(score:enum_member(Shade,"Dark"))],"score");"#;
    let error = Program::parse(source, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap_err();
    assert_eq!(&source[error.range], "\"score\"");
    let limited = Program::parse("report sort([4,3,2,1]);", limits)
        .unwrap()
        .evaluate(&input, ProgramLimits { work: 50, ..limits })
        .unwrap_err();
    assert!(limited.message.contains("work") && limited.range.end <= 25);
    for source in [
        "let unused=sort([1]); replace a=2; emit;",
        "replace a=if true then 2 else min([1]) endif; emit;",
    ] {
        let error = Program::parse(source, limits).unwrap_err();
        assert!(error.message.contains("report-only"));
    }
}

#[test]
fn native_input_bindings_interleave_with_calculations_and_keep_original_reads() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let model = semantic_model("let-inputs", b"int: a; int: b; solve satisfy;", b"", "");
    let original = b"a=1; b=2;";
    let input = Input::parse(original.to_vec(), FileMode::Data).unwrap();
    let request = r#"let increment=2; let field="a"; let old=items | filter(name(field)) | values | one; let next=old+increment; let saved=items | filter(name("b")) | values | one; replace a=next; replace b=old; emit;"#;
    let mut result = Program::parse(request, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert_eq!(result.render().unwrap(), b"a=3; b=1;");
    assert_eq!(input.source_bytes(), original);
    // Read-only inputs may follow calculations, and a computed literal name stays checked.
    let request =
        r#"let field="a"; let old=items | filter(name(field)) | values | one; report old;"#;
    let mut result = Program::parse(request, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    assert!(
        String::from_utf8(result.render().unwrap())
            .unwrap()
            .contains(r#""value":1"#)
    );
    for request in [
        r#"let a=count | json; report a;"#,
        r#"let a=items; let b=a | count | json; report b;"#,
        r#"let a=items | project(d, {"n":d | count}); replace a=1; emit;"#,
        r#"let a=match { from d in items; }; replace a=1; emit;"#,
        r#"let a=items | paths(referenced_declarations,to=name("a"),max_depth=1); replace a=1; emit;"#,
        r#"let a=(n:count); replace a=1; emit;"#,
        r#"let a=items | filter(name("a")) | values | one; report a; replace b=1; emit;"#,
    ] {
        assert!(Program::parse(request, limits).is_err(), "{request}");
    }
}

#[test]
fn native_literal_extraction_and_final_values_share_one_request() {
    use zincite_query::{ErrorLocation, Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"required=[2,3,4];".to_vec(), FileMode::Data).unwrap();
    let body = r#"let required=items | filter(name("required")) | values | one; sum(required)"#;
    let mut expected = None;
    for prefix in ["", "fn unused(n:int)->int { n } "] {
        for end in ["", ";"] {
            let program = Program::parse(&format!("{prefix}{body}{end}"), limits).unwrap();
            let mut result = program.evaluate(&input, limits).unwrap();
            assert!(result.is_complete());
            let bytes = result.render().unwrap();
            let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                json["result"],
                serde_json::json!({"kind":"integer","value":9})
            );
            if let Some(expected) = &expected {
                assert_eq!(&bytes, expected);
            } else {
                expected = Some(bytes);
            }
        }
    }
    let program = Program::parse(body, limits).unwrap();
    for source in [b"other=[1];".as_slice(), b"required=[1]; required=[2];"] {
        let data = Input::parse(source.to_vec(), FileMode::Data).unwrap();
        let error = program.evaluate(&data, limits).unwrap_err();
        assert_eq!(error.location, ErrorLocation::Query);
        assert_eq!(&body[error.range], "one");
        assert!(error.message.contains("exactly one"));
    }
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 100,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work limit"));
    for source in ["let x=1", "let x=1 x", "let x=1; let y=x"] {
        assert!(Program::parse(source, limits).is_err(), "{source}");
    }
}

#[test]
fn patterns_and_control_values_agree_with_owned_typed_programs() {
    use zincite_query::{Program, ProgramFunction, ProgramLimits, ProgramValue, ReportValue};
    use zirql::{
        ConstructedValue as V, Located as L, Pattern, PatternKind as P, SignatureType as S,
        ValueBinaryOperator as B, ValueExpressionKind as K,
    };
    type Q = zirql::Expression<zincite_query::ProgramNative, zirql::NativeSetOperator, ()>;
    fn located<T>(value: T) -> L<T> {
        L::new(value, 10..20)
    }
    fn e(value: K<Q>) -> ProgramValue {
        located(value)
    }
    fn name(name: &str) -> ProgramValue {
        e(K::Name(name.into()))
    }
    fn int(value: i64) -> ProgramValue {
        e(K::Literal(V::Integer(value)))
    }
    fn pattern(name: &str) -> Pattern {
        located(P::Name(located(name.into())))
    }
    fn record_pattern(fields: Vec<(&str, Pattern)>, ignore_rest: bool) -> Pattern {
        located(P::Record {
            fields: fields
                .into_iter()
                .map(|(n, p)| (located(n.into()), p))
                .collect(),
            ignore_rest,
        })
    }
    fn record(fields: Vec<(&str, ProgramValue)>) -> ProgramValue {
        e(K::Record(
            fields
                .into_iter()
                .map(|(n, v)| (located(n.into()), v))
                .collect(),
        ))
    }
    fn call(function: &str, arguments: Vec<ProgramValue>) -> ProgramValue {
        e(K::Call {
            function: located(function.into()),
            arguments,
        })
    }
    fn binary(operator: B, left: ProgramValue, right: ProgramValue) -> ProgramValue {
        e(K::Binary {
            operator,
            left: Box::new(left),
            right: Box::new(right),
        })
    }
    fn function(
        function: &str,
        parameter: Pattern,
        declaration: S,
        result: S,
        value: ProgramValue,
    ) -> ProgramFunction {
        ProgramFunction {
            name: located(function.into()),
            parameters: vec![located(zirql::UnresolvedPatternFunctionParameter {
                pattern: parameter,
                declaration: located(declaration),
            })],
            result: located(result),
            value,
        }
    }
    let limits = ProgramLimits::default();
    let input = Input::parse(
        include_bytes!("fixtures/recursive-functions.mzn").to_vec(),
        FileMode::Model,
    )
    .unwrap();
    let scalar_fields = || {
        S::Record(vec![
            (located("total".into()), located(S::Integer)),
            (located("label".into()), located(S::String)),
        ])
    };
    let nodes = || S::Sequence(Box::new(located(S::Named(located("Node".into())))));
    let native_fields = || S::Record(vec![(located("selected".into()), located(nodes()))]);
    let describe = function(
        "describe",
        record_pattern(
            vec![
                ("pair", located(P::Tuple(vec![pattern("a"), pattern("b")]))),
                ("label", pattern("label")),
            ],
            false,
        ),
        S::Record(vec![
            (
                located("pair".into()),
                located(S::Tuple(vec![located(S::Integer), located(S::Integer)])),
            ),
            (located("label".into()), located(S::String)),
        ]),
        scalar_fields(),
        record(vec![
            ("total", binary(B::Add, name("a"), name("b"))),
            ("label", name("label")),
        ]),
    );
    let keep = function(
        "keep",
        pattern("nodes"),
        nodes(),
        native_fields(),
        record(vec![("selected", name("nodes"))]),
    );
    let query = zincite_query::Query::parse(
        "items | head(1) | tail(1) | reverse | check(1)",
        zincite_query::Limits::default(),
    )
    .unwrap();
    let native = located(zirql::ExpressionKind::Sequence {
        input: Box::new(located(zirql::ExpressionKind::Input)),
        steps: vec![located(zirql::ExpressionStep::Stage(
            zincite_query::ProgramNative::Query(query),
        ))],
    });
    let typed = Program::from_values(
        vec![describe, keep],
        vec![
            (
                record_pattern(
                    vec![("total", pattern("total")), ("label", pattern("title"))],
                    false,
                ),
                call(
                    "describe",
                    vec![record(vec![
                        ("pair", e(K::Tuple(vec![int(3), int(4)]))),
                        ("label", e(K::Literal(V::String("sum".into())))),
                    ])],
                ),
            ),
            (pattern("roots"), e(K::NativeValue(Box::new(native)))),
            (
                record_pattern(vec![("selected", pattern("root"))], false),
                call("keep", vec![name("roots")]),
            ),
        ],
        record(vec![
            ("total", name("total")),
            ("title", name("title")),
            ("root", name("root")),
        ]),
        limits,
    )
    .unwrap();
    let mut actual = typed.evaluate(&input, limits).unwrap();
    let mut written = Program::parse(include_str!("fixtures/destructured-report.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    for result in [&actual, &written] {
        assert!(result.is_complete());
        let fields = result.emissions()[0].value().fields().unwrap();
        let nodes = fields
            .iter()
            .find(|(name, _)| name == "root")
            .unwrap()
            .1
            .stream()
            .unwrap();
        let zincite_query::SemanticStream::Inspection(nodes) = nodes else {
            panic!("native nodes: {nodes:?}")
        };
        let zincite_query::QueryResult::Selection(nodes) = nodes.as_ref() else {
            panic!("native selection")
        };
        assert_eq!(nodes.items()[0].range, 0..31);
        assert!(std::ptr::eq(nodes.input(), &input));
    }
    assert_eq!(actual.render().unwrap(), written.render().unwrap());
    let block = e(K::Block {
        bindings: vec![(
            located(P::Tuple(vec![pattern("first"), pattern("second")])),
            e(K::Tuple(vec![name("value"), int(2)])),
        )],
        value: Box::new(e(K::Conditional {
            branches: vec![(
                binary(B::Greater, name("first"), name("second")),
                e(K::Block {
                    bindings: vec![(
                        pattern("result"),
                        binary(B::Add, name("first"), name("second")),
                    )],
                    value: Box::new(name("result")),
                }),
            )],
            otherwise: Box::new(int(0)),
        })),
    });
    let present = || located(P::Present(Box::new(pattern("value"))));
    let typed = Program::from_values(
        vec![],
        vec![(
            record_pattern(vec![("value", pattern("candidate"))], true),
            record(vec![
                ("value", int(7)),
                ("ignored", e(K::Literal(V::String("kept out".into())))),
            ]),
        )],
        e(K::ValueMatch {
            subject: Box::new(name("candidate")),
            arms: vec![
                zirql::ValueMatchArm {
                    pattern: located(P::Absent),
                    guard: None,
                    value: e(K::Literal(V::String("missing".into()))),
                },
                zirql::ValueMatchArm {
                    pattern: present(),
                    guard: Some(binary(B::Less, name("value"), int(0))),
                    value: e(K::Literal(V::String("negative".into()))),
                },
                zirql::ValueMatchArm {
                    pattern: present(),
                    guard: Some(binary(B::Greater, name("value"), int(0))),
                    value: block,
                },
                zirql::ValueMatchArm {
                    pattern: located(P::Wildcard),
                    guard: None,
                    value: int(0),
                },
            ],
        }),
        limits,
    )
    .unwrap();
    let mut actual = typed.evaluate(&input, limits).unwrap();
    let mut written = Program::parse(include_str!("fixtures/optional-match.zq"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    assert!(matches!(
        actual.emissions()[0].value(),
        ReportValue::Constructed(V::Integer(9))
    ));
    assert_eq!(actual.render().unwrap(), written.render().unwrap());
    let mut deep = int(0);
    for _ in 0..2000 {
        deep = e(K::Tuple(vec![deep]));
    }
    let rejected = Program::from_values(vec![], vec![], deep, limits).unwrap_err();
    assert_eq!(rejected.range, 10..20);
    assert!(rejected.message.contains("nesting"));
}

#[test]
fn patterns_and_control_values_preflight_scope_shapes_purity_and_tail_calls() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"int: value = 1; solve satisfy;".to_vec(), FileMode::Model).unwrap();
    for source in [
        "let (a,b)=(1,); report 0;",
        "let {a}=(a:1,b:2); report a;",
        "let {a,b}=(a:1); report a;",
        "fn wrong((a,b):tuple<int,int>)->int{a} report wrong((1,\"x\"));",
        "let a=1; report match(1){ present(a)=>0,_=>1 };",
        "report match(1){ absent=>{let (a,a)=(1,2);0},_=>1 };",
        "report match(1){ present(value) if 1=>value,_=>0 };",
        "report match(1){ present(value) if <>=>value,_=>0 };",
        "report match(1){ present(value)=>value };",
        "report if true { 1 } else { emit() };",
        "report match(1){ present(value)=>1 div 0,_=>0 };",
        "let (a,b)=(1,2); create value=1; emit;",
        "let a=if false { loop(n=0){break n} } else {0}; create value=1; emit;",
    ] {
        let result =
            Program::parse(source, limits).and_then(|program| program.evaluate(&input, limits));
        assert!(result.is_err(), "accepted {source}");
        let error = result.unwrap_err();
        assert!(error.range.end <= source.len(), "{source}: {error:?}");
    }
    let source = "fn down(n:int)->int { match(n) { 0=>0, _=>{ let next=n-1; if next >= 0 { down(next) } else {0} } } } report down(1000);";
    let program = Program::parse(source, limits)
        .unwrap()
        .with_function_limits(zirql::FunctionLimits { active_depth: 1 });
    program.evaluate(&input, limits).unwrap();
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 2000,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work"), "{error:?}");
    assert!(error.range.end <= source.len());
    let mut contextual = Program::parse(
        "let match=1; let if=2; let present=3; let absent=4; report (match,if,present,absent);",
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
    assert!(!contextual.render().unwrap().is_empty());
    Program::parse("let 'quoted name'=1; report 'quoted name';", limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    Program::parse("report match(if /* subject */ true then (1,2) else (3,4) endif) { (a,b) if a in {1,3} => a+b, _ => 0 };",limits).unwrap().evaluate(&input,limits).unwrap();
    Program::parse(
        "let {a,..}=(a:1,b:2); report if a=1 { a } else { 1 div 0 };",
        limits,
    )
    .unwrap()
    .evaluate(&input, limits)
    .unwrap();
}

#[test]
fn shared_controls_compose_inside_minizinc_expressions() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"int: value = 1; solve satisfy;".to_vec(), FileMode::Model).unwrap();
    let source = "fn f(x:int)->int{x} report (value: match(1){1=>7,_=>0}, choices: [if true {1} else {0}], call: f(match(1){1=>7,_=>0}), combined: -match(1){1=>7,_=>0} + sum([if i > 1 {i} else {0} | i in 1..3]) + if true then 1 else 0 endif);";
    let mut result = Program::parse(source, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let rendered: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
    let fields = &rendered["result"];
    assert_eq!(fields["value"]["value"], 7);
    assert_eq!(fields["choices"]["entries"][0]["value"]["value"], 1);
    assert_eq!(fields["call"]["value"], 7);
    assert_eq!(fields["combined"]["value"], -1);
}

#[test]
fn owned_typed_admission_bounds_literals_and_metadata_before_use() {
    use zincite_query::{Program, ProgramFunction, ProgramLimits, ProgramValue};
    use zirql::{
        ConstructedValue as V, Located as L, SignatureType as S, ValueExpressionKind as K,
    };
    fn value() -> ProgramValue {
        L::new(K::Literal(V::Integer(0)), 10..20)
    }
    std::thread::spawn(|| {
        let mut literal = V::Integer(0);
        for _ in 0..20_000 {
            literal = V::Tuple(vec![literal]);
        }
        let error = Program::from_values(
            vec![],
            vec![],
            L::new(K::Literal(literal), 10..20),
            ProgramLimits::default(),
        )
        .unwrap_err();
        assert_eq!(error.range, 10..20);
        assert!(error.message.contains("limit"), "{error:?}");
    })
    .join()
    .unwrap();
    std::thread::spawn(|| {
        let mut selector = zirql::Located::new(zirql::ExpressionKind::Input, 10..20);
        for _ in 0..20_000 {
            selector = zirql::Located::new(
                zirql::ExpressionKind::Sequence {
                    input: Box::new(zirql::Located::new(zirql::ExpressionKind::Input, 10..20)),
                    steps: vec![zirql::Located::new(
                        zirql::ExpressionStep::Stage(zincite_query::ProgramNative::Ordering(
                            zirql::NativeOrdering {
                                order: zirql::NativeOrder::Sort,
                                selector: Some(selector),
                            },
                        )),
                        10..20,
                    )],
                },
                10..20,
            );
        }
        assert!(
            Program::from_values(
                vec![],
                vec![],
                zirql::Located::new(K::NativeValue(Box::new(selector)), 10..20),
                ProgramLimits::default()
            )
            .is_err()
        );
    })
    .join()
    .unwrap();
    let limits = ProgramLimits {
        work: 10,
        results: 2,
        ..ProgramLimits::default()
    };
    let function = ProgramFunction {
        name: L::new("unused".into(), 10..20),
        parameters: vec![],
        result: L::new(
            S::Tuple((0..128).map(|_| L::new(S::Integer, 10..20)).collect()),
            10..20,
        ),
        value: value(),
    };
    assert!(Program::from_values(vec![function], vec![], value(), limits).is_err());
    let array = L::new(K::Array((0..128).map(|_| value()).collect()), 10..20);
    assert!(Program::from_values(vec![], vec![], array, limits).is_err());
    let native = L::new(
        zirql::ExpressionKind::Record(
            (0..128)
                .map(|i| {
                    (
                        L::new(format!("f{i}"), 10..20),
                        L::new(zirql::ExpressionKind::Input, 10..20),
                    )
                })
                .collect(),
        ),
        10..20,
    );
    assert!(
        Program::from_values(
            vec![],
            vec![],
            L::new(K::NativeValue(Box::new(native)), 10..20),
            limits
        )
        .is_err()
    );
}

mod worklists {
    use super::*;
    use zincite_query::{Program, ProgramFunction, ProgramLimits, ProgramValue, ReportValue};
    use zirql::{
        ConstructedValue as V, Located as L, PatternKind as P, SignatureType as S,
        ValueExpressionKind as K,
    };
    type Q = zirql::Expression<zincite_query::ProgramNative, zirql::NativeSetOperator, ()>;
    fn l<T>(value: T) -> L<T> {
        L::new(value, 10..20)
    }
    fn e(value: K<Q>) -> ProgramValue {
        l(value)
    }
    fn name(value: &str) -> ProgramValue {
        e(K::Name(value.into()))
    }
    fn pattern(value: &str) -> zirql::Pattern {
        l(P::Name(l(value.into())))
    }
    fn call(value: &str, arguments: Vec<ProgramValue>) -> ProgramValue {
        e(K::Call {
            function: l(value.into()),
            arguments,
        })
    }
    fn boolean(value: bool) -> ProgramValue {
        e(K::Literal(V::Boolean(value)))
    }
    fn native(query: &str) -> ProgramValue {
        e(K::NativeValue(Box::new(l(
            zirql::ExpressionKind::Sequence {
                input: Box::new(l(zirql::ExpressionKind::Input)),
                steps: vec![l(zirql::ExpressionStep::Stage(
                    zincite_query::ProgramNative::Query(
                        Query::parse(query, Limits::default()).unwrap(),
                    ),
                ))],
            },
        ))))
    }
    fn value<'v, 'a>(mut value: &'v ReportValue<'a>) -> &'v ReportValue<'a> {
        loop {
            match value {
                ReportValue::Retained(v) => value = v,
                ReportValue::Item(v) | ReportValue::EntitySequence { value: v, .. } => value = v,
                _ => return value,
            }
        }
    }
    fn transfer(pending: ProgramValue, seen: ProgramValue) -> ProgramValue {
        e(K::Continue(vec![pending, seen]))
    }
    fn reaches() -> ProgramFunction {
        let advance = e(K::Block {
            bindings: vec![(
                l(P::Record {
                    fields: vec![(l("targets".into()), pattern("targets"))],
                    ignore_rest: true,
                }),
                call("reference_info", vec![name("current")]),
            )],
            value: Box::new(transfer(
                e(K::PushBackAll {
                    queue: Box::new(name("remaining")),
                    items: Box::new(name("targets")),
                }),
                e(K::Insert {
                    set: Box::new(name("seen")),
                    item: Box::new(name("current")),
                }),
            )),
        });
        let body = e(K::ValueMatch {
            subject: Box::new(e(K::PopFront(Box::new(name("pending"))))),
            arms: vec![
                zirql::ValueMatchArm {
                    pattern: l(P::Present(Box::new(l(P::Tuple(vec![
                        pattern("current"),
                        pattern("remaining"),
                    ]))))),
                    guard: None,
                    value: e(K::Conditional {
                        branches: vec![
                            (
                                e(K::Binary {
                                    operator: zirql::ValueBinaryOperator::In,
                                    left: Box::new(name("current")),
                                    right: Box::new(name("seen")),
                                }),
                                transfer(name("remaining"), name("seen")),
                            ),
                            (
                                call("same_declaration", vec![name("current"), name("target")]),
                                e(K::Break(Box::new(boolean(true)))),
                            ),
                        ],
                        otherwise: Box::new(advance),
                    }),
                },
                zirql::ValueMatchArm {
                    pattern: l(P::Wildcard),
                    guard: None,
                    value: e(K::Break(Box::new(boolean(false)))),
                },
            ],
        });
        ProgramFunction {
            name: l("reaches".into()),
            parameters: vec![l(zirql::UnresolvedPatternFunctionParameter {
                pattern: l(P::Tuple(vec![pattern("start"), pattern("target")])),
                declaration: l(S::Tuple(vec![
                    l(S::Named(l("Declaration".into()))),
                    l(S::Named(l("Declaration".into()))),
                ])),
            })],
            result: l(S::Boolean),
            value: e(K::Loop {
                states: vec![
                    (
                        l("pending".into()),
                        e(K::Queue {
                            items: Box::new(e(K::Array(vec![name("start")]))),
                            element: None,
                        }),
                    ),
                    (
                        l("seen".into()),
                        e(K::IdentitySet {
                            element: l(S::Named(l("Declaration".into()))),
                        }),
                    ),
                ],
                body: Box::new(body),
            }),
        }
    }
    fn reach_report(target: &str) -> ProgramValue {
        let clause = |name: &str, source| {
            l(zirql::ValueGenerator::In {
                binders: vec![l(Some(name.into()))],
                source,
                filter: None,
            })
        };
        e(K::One(Box::new(e(K::Comprehension {
            constructor: zirql::ValueConstructor::Array,
            clauses: vec![
                clause("start", name("starts")),
                clause("target", name(target)),
            ],
            head: Box::new(call(
                "reaches",
                vec![e(K::Tuple(vec![name("start"), name("target")]))],
            )),
        }))))
    }
    #[test]
    fn declaration_graph_text_and_owned_trees_agree_before_rendering() {
        let source = include_bytes!("fixtures/worklist-reachability.mzn");
        let model = semantic_model("worklist-graph", source, b"", "");
        let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
        let limits = ProgramLimits::default();
        let mut written = Program::parse(include_str!("fixtures/worklist-reachability.zq"), limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let typed = Program::from_values(
            vec![reaches()],
            vec![
                (
                    pattern("starts"),
                    native("declarations | filter(name(\"A\"))"),
                ),
                (pattern("A"), name("starts")),
                (pattern("D"), native("declarations | filter(name(\"D\"))")),
                (pattern("E"), native("declarations | filter(name(\"E\"))")),
            ],
            e(K::Record(vec![
                (l("zero_edges".into()), reach_report("A")),
                (l("reachable".into()), reach_report("D")),
                (l("disconnected".into()), reach_report("E")),
            ])),
            limits,
        )
        .unwrap();
        let mut typed = typed
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let ReportValue::Constructed(V::Record(fields)) = value(typed.emissions()[0].value())
        else {
            panic!("native Boolean report")
        };
        assert_eq!(
            fields.iter().map(|(_, v)| v.data()).collect::<Vec<_>>(),
            [&V::Boolean(true), &V::Boolean(true), &V::Boolean(false)]
        );
        assert_eq!(typed.render().unwrap(), written.render().unwrap());
        let inspected = "let ds=declarations | filter(name(\"A\")); let present((d,_))=pop_front(queue<Declaration>(ds)); reference_info(d)";
        let info = Program::parse(inspected, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let targets = info.emissions()[0].value().fields().unwrap()[0]
            .1
            .stream()
            .unwrap();
        let zincite_query::SemanticStream::Entities(targets) = targets else {
            panic!("native targets")
        };
        let facts = info.model_facts().unwrap();
        let names = targets
            .iter()
            .map(|entity| match entity {
                zincite_query::SemanticEntity::Declaration(id) => {
                    facts.bindings().declarations[id.0].name.as_str()
                }
                _ => panic!(),
            })
            .collect::<Vec<_>>();
        assert_eq!(names, ["B", "C"]);
        let identity = "let ds=declarations | filter(name(\"A\")); let present((a,_))=pop_front(queue<Declaration>(ds)); let other_ds=declarations | filter(name(\"D\")); let present((b,_))=pop_front(queue<Declaration>(other_ds)); (same:same_declaration(a,a),different:same_declaration(a,b),member:a in insert(identity_set<Declaration>(),a),other:b in insert(identity_set<Declaration>(),a))";
        let result = Program::parse(identity, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let ReportValue::Constructed(V::Record(fields)) = value(result.emissions()[0].value())
        else {
            panic!()
        };
        assert_eq!(
            fields.iter().map(|(_, v)| v.data()).collect::<Vec<_>>(),
            [
                &V::Boolean(true),
                &V::Boolean(false),
                &V::Boolean(true),
                &V::Boolean(false)
            ]
        );
    }
    #[test]
    fn reducers_keep_destructured_state_and_native_snapshots() {
        let input =
            Input::parse(b"int: a=1; int: b=2; int: c=3;".to_vec(), FileMode::Model).unwrap();
        let limits = ProgramLimits::default();
        let mut result = Program::parse(include_str!("fixtures/fold-scan.zq"), limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let integer = |v| e(K::Literal(V::Integer(v)));
        let tuple = |a, b| e(K::Tuple(vec![integer(a), integer(b)]));
        let add = |a, b| {
            e(K::Binary {
                operator: zirql::ValueBinaryOperator::Add,
                left: Box::new(a),
                right: Box::new(b),
            })
        };
        let step = ProgramFunction {
            name: l("step".into()),
            parameters: vec![
                l(zirql::UnresolvedPatternFunctionParameter {
                    pattern: l(P::Tuple(vec![pattern("count"), pattern("total")])),
                    declaration: l(S::Tuple(vec![l(S::Integer), l(S::Integer)])),
                }),
                l(zirql::UnresolvedPatternFunctionParameter {
                    pattern: pattern("item"),
                    declaration: l(S::Integer),
                }),
            ],
            result: l(S::Tuple(vec![l(S::Integer), l(S::Integer)])),
            value: e(K::Tuple(vec![
                add(name("count"), integer(1)),
                add(name("total"), name("item")),
            ])),
        };
        let reduce = |scan, empty| {
            let items = Box::new(e(K::Array(if empty {
                vec![]
            } else {
                vec![integer(2), integer(3), integer(5)]
            })));
            let initial = Box::new(tuple(0, 0));
            let step = l("step".into());
            e(if scan {
                K::Scan {
                    items,
                    initial,
                    step,
                }
            } else {
                K::Fold {
                    items,
                    initial,
                    step,
                }
            })
        };
        let typed = Program::from_values(
            vec![step],
            vec![],
            e(K::Record(vec![
                (l("folded".into()), reduce(false, false)),
                (l("scanned".into()), reduce(true, false)),
                (l("empty_fold".into()), reduce(false, true)),
                (l("empty_scan".into()), reduce(true, true)),
            ])),
            limits,
        )
        .unwrap();
        let mut typed = typed.evaluate(&input, limits).unwrap();
        let rendered = result.render().unwrap();
        assert_eq!(typed.render().unwrap(), rendered);
        let json: serde_json::Value = serde_json::from_slice(&rendered).unwrap();
        assert_eq!(json["result"]["folded"]["kind"], "tuple");
        assert_eq!(json["result"]["folded"]["elements"][1]["value"], 10);
        let source = "fn append(q:queue<Node>,item:Node)->queue<Node>{push_back_all(q,[item])} let nodes=items; let original=queue<Node>(nodes); let present((first,rest))=pop_front(original); report (original:original, rest:rest, snapshots:scan(nodes,queue<Node>([]),append));";
        let mut result = Program::parse(source, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let fields = result.emissions()[0].value().fields().unwrap();
        let ReportValue::Computational(original) = value(&fields[0].1) else {
            panic!("queue alias")
        };
        let ReportValue::Computational(rest) = value(&fields[1].1) else {
            panic!("queue tail")
        };
        assert_eq!((original.len(), rest.len()), (3, 2));
        let ReportValue::Sequence(states) = value(&fields[2].1) else {
            panic!("native scan states")
        };
        assert_eq!(
            states
                .values()
                .iter()
                .map(|state| match value(state) {
                    ReportValue::Computational(q) => q.len(),
                    _ => panic!(),
                })
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
        let error = result.render().unwrap_err();
        assert!(error.message.contains("cannot be rendered"));
        assert!(source[error.range].contains("queue"));
    }
    #[test]
    fn uncertain_adjacency_is_a_located_error_even_after_legacy_lookup() {
        let limits = ProgramLimits::default();
        let source = b"\xef\xbb\xbfint: A=missing; int: E=+1; solve satisfy;";
        let model = semantic_model("worklist-unknown", source, b"", "");
        let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
        let query = "fn info(d:Declaration)->record{targets:sequence<Declaration>}{reference_info(d)} let ds=declarations | filter(name(\"A\")); let old=ds | referenced_declarations; [info(d) | d in ds]";
        let error = Program::parse(query, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap_err();
        assert!(error.message.contains("complete adjacency"), "{error:?}");
        assert_eq!(&source[error.range.clone()], b"missing");
        assert!(matches!(
            error.location,
            zincite_query::ErrorLocation::Model(_)
        ));
        let known = "let ds=declarations | filter(name(\"E\")); let present((d,_))=pop_front(queue<Declaration>(ds)); reference_info(d)";
        let mut result = Program::parse(known, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        assert!(result.is_complete());
        assert!(!result.render().unwrap().is_empty());
    }
    #[test]
    fn transfers_and_new_children_keep_purity_and_runtime_limits() {
        let limits = ProgramLimits::default();
        let input = Input::parse(b"int: value=1;".to_vec(), FileMode::Model).unwrap();
        for source in [
            "break 1",
            "continue(1)",
            "loop(n=0){n}",
            "loop(n=0){continue()}",
            "loop(n=0){continue(\"wrong\")}",
            "fn bad(a:int)->int{a} fold([],0,bad)",
            "fn bad(a:int,b:int)->int{emit()} scan([],0,bad)",
            "let unused=queue<int>([]); create value=1; emit;",
            "replace value=if true then 1 else fold([],0,missing) endif; emit;",
            "let bad=if false { loop(n=0){break n} } else {0}; create value=1; emit;",
            "queue<int>([\"wrong\"])",
        ] {
            assert!(
                Program::parse(source, limits)
                    .and_then(|p| p.evaluate(&input, limits))
                    .is_err(),
                "accepted {source}"
            );
        }
        let source = "loop(a=1,b=2,n=0){if n=1{break (a,b)}else{continue(b,a,n+1)}}";
        let mut result = Program::parse(source, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let ReportValue::Constructed(tuple) = value(result.emissions()[0].value()) else {
            panic!("simultaneous state")
        };
        let V::Tuple(values) = tuple.data() else {
            panic!("tuple state")
        };
        assert_eq!(
            values.iter().map(V::data).collect::<Vec<_>>(),
            [&V::Integer(2), &V::Integer(1)]
        );
        assert!(!result.render().unwrap().is_empty());
        let nested = "(values:[loop(n=i){if n>0{break n}else{continue(n+1)}} | i in [0,1]], count:length(scan([],0,step)))";
        let source = format!("fn step(a:int,b:int)->int{{a+b}} {nested}");
        Program::parse(&source, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let looping = Program::parse("loop(n=0){continue(n+1)}", limits).unwrap();
        let error = looping
            .evaluate(
                &input,
                ProgramLimits {
                    work: 2000,
                    ..limits
                },
            )
            .unwrap_err();
        assert!(error.message.contains("work"), "{error:?}");
    }
}

#[test]
fn saved_native_and_minizinc_values_compose_with_shared_suffixes() {
    use zincite_query::{ConstructedValue as V, Program, ProgramLimits, ReportValue};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"b=2; a=1;".to_vec(), FileMode::Data).unwrap();
    let body = r#"let selected=items; let labels=selected | names; let n=selected | count;
        let rows=[label | label in labels]; let ordered=(rows) | sort | unique;
        report (names:ordered,count:n,selected:selected);"#;
    let mut expected = None;
    for prefix in ["", "fn unused(n:int)->int { n } "] {
        let request = format!("{prefix}{body}");
        let mut result = Program::parse(&request, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        assert!(result.is_complete());
        let fields = match result.emissions()[0].value() {
            ReportValue::Record(fields) => fields,
            ReportValue::Capture(capture) => &capture.fields,
            value => panic!("native record: {value:?}"),
        };
        assert_eq!(fields[0].0, "names");
        assert!(
            matches!(&fields[0].1, ReportValue::Constructed(value) if value.data()==&V::Array(vec![V::String("a".into()), V::String("b".into())]))
        );
        assert!(
            matches!(&fields[2].1, ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) if matches!(value.as_ref(), zincite_query::QueryResult::Selection(items) if items.items().len()==2))
        );
        if let ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) = &fields[2].1 {
            let zincite_query::QueryResult::Selection(selected) = value.as_ref() else {
                panic!("native selection")
            };
            assert!(std::ptr::eq(selected.input(), &input));
            assert_eq!(
                &input.source_bytes()[selected.items()[1].range.clone()],
                b"a=1;"
            );
        }
        let rendered = result.render().unwrap();
        if let Some(expected) = &expected {
            assert_eq!(&rendered, expected);
        } else {
            expected = Some(rendered);
        }
    }
    let siblings = r#"let selected=items | head(1);
        (saved:selected | count, current:count, names:selected | names)"#;
    for prefix in ["", "fn unused(n:int)->int { n } "] {
        let mut result = Program::parse(&format!("{prefix}{siblings}"), limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        let output: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
        assert_eq!(output["result"]["saved"], 1);
        assert_eq!(output["result"]["current"], 2);
        assert_eq!(output["result"]["names"], serde_json::json!(["b"]));
    }
    let filtered = r#"fn words()->sequence<string> { ["ash","birch","apple","ash"] }
        let saved=words();
        let matching=(saved) | filter(starts_with("a"));
        matching | sort | unique | count"#;
    let result = Program::parse(filtered, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    assert!(
        matches!(result.emissions()[0].value(), ReportValue::Constructed(value) if value.data()==&V::Integer(2))
    );
    for request in [
        "let empty=[]; empty | names",
        "let empty=items | head(0) | count; empty | items",
        "query(\"count\")",
    ] {
        let error = Program::parse(request, limits)
            .and_then(|program| program.evaluate(&input, limits))
            .unwrap_err();
        assert!(!error.range.is_empty(), "{error:?}");
        if request.ends_with("names") {
            assert_eq!(&request[error.range], "names");
        } else if request.ends_with("items") {
            assert_eq!(&request[error.range], "items");
        }
    }
    let function = r#"fn ordered(labels:sequence<string>)->sequence<string> {
        let rows=[label | label in labels]; [label | label in rows] | sort | unique
    } let selected=items; let labels=selected | names; ordered(labels); (ordered(labels)) | tally"#;
    let result = Program::parse(function, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(value) = result.emissions()[0].value() else {
        panic!("constructed result")
    };
    let V::Array(values) = value.data() else {
        panic!("array result")
    };
    assert_eq!(
        values.iter().map(V::data).collect::<Vec<_>>(),
        [&V::String("a".into()), &V::String("b".into())]
    );
    assert!(
        matches!(result.emissions()[1].value(), ReportValue::Constructed(value) if value.data()==&V::Record(vec![("a".into(),V::Integer(1)),("b".into(),V::Integer(1))]))
    );
    for prefix in ["", "fn unused(n:int)->int { n } "] {
        let request = format!(
            r#"{prefix}let wanted=["a"]; report items | filter(name(([s | s in wanted] | unique)[1])) | head(length(([2,1,2] | unique))) | count;"#
        );
        let result = Program::parse(&request, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        match result.emissions()[0].value() {
            ReportValue::Constructed(value) => assert_eq!(value.data(), &V::Integer(1)),
            ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) => {
                assert!(matches!(
                    value.as_ref(),
                    zincite_query::QueryResult::Count(1)
                ));
            }
            value => panic!("count result: {value:?}"),
        }
    }
    for (body, values) in [
        (
            "let rows=[3,1,3]; rows | unique",
            vec![V::Integer(3), V::Integer(1)],
        ),
        (
            "let rows=[3,1,3] | unique; rows | sort",
            vec![V::Integer(1), V::Integer(3)],
        ),
    ] {
        let values = V::Array(values);
        let mut expected = None;
        for prefix in ["", "fn unused(n:int)->int { n } "] {
            let request = format!("{prefix}{body}");
            let mut result = Program::parse(&request, limits)
                .unwrap()
                .evaluate(&input, limits)
                .unwrap();
            assert!(
                matches!(result.emissions()[0].value(), ReportValue::Constructed(value) if value.data()==&values),
                "{request}"
            );
            let rendered = result.render().unwrap();
            if let Some(expected) = &expected {
                assert_eq!(&rendered, expected);
            } else {
                expected = Some(rendered);
            }
        }
    }
    for (request, value) in [
        (
            "[3,1,3,2] | sort | unique",
            V::Array(vec![V::Integer(1), V::Integer(2), V::Integer(3)]),
        ),
        (
            r#"["b","a","b"] | tally"#,
            V::Record(vec![
                ("b".into(), V::Integer(2)),
                ("a".into(), V::Integer(1)),
            ]),
        ),
        ("[] | sort | unique", V::Array(vec![])),
        ("length(([x | x in [2,1,2]] | unique))", V::Integer(2)),
        (
            "if false then ([2,1] | sort) else ([4,3] | sort) endif",
            V::Array(vec![V::Integer(3), V::Integer(4)]),
        ),
        (
            "[e.value | e in entries([2:3, 5:1])] | sort",
            V::Array(vec![V::Integer(1), V::Integer(3)]),
        ),
    ] {
        let result = Program::parse(request, limits)
            .unwrap()
            .evaluate(&input, limits)
            .unwrap();
        assert!(
            matches!(result.emissions()[0].value(), ReportValue::Constructed(actual) if actual.data()==&value),
            "{request}: {:?}",
            result.emissions()[0].value()
        );
    }
    for (request, stage) in [
        ("[1] | tally", "tally"),
        (r#"["a"] | tally | sort"#, "sort"),
        ("[2:3,5:1] | sort", "sort"),
        ("[1] | names", "names"),
    ] {
        let error = Program::parse(request, limits)
            .and_then(|p| p.evaluate(&input, limits))
            .unwrap_err();
        assert_eq!(&request[error.range], stage, "{}", error.message);
    }
    for request in [
        "let unused=[2,1] | sort; replace a=1; emit;",
        "let unused=if false then ([2,1] | sort) else [] endif; replace a=1; emit;",
    ] {
        assert!(
            Program::parse(request, limits)
                .and_then(|p| p.evaluate(&input, limits))
                .is_err(),
            "{request}"
        );
    }
    let typed_empty =
        "fn bad(xs:sequence<record{x:int}>)->sequence<record{x:int}> { (xs) | sort } bad([])";
    let error = Program::parse(typed_empty, limits)
        .and_then(|p| p.evaluate(&input, limits))
        .unwrap_err();
    assert_eq!(&typed_empty[error.range], "sort", "{}", error.message);
    let program = Program::parse(body, limits).unwrap();
    let error = program
        .evaluate(
            &input,
            ProgramLimits {
                work: 100,
                ..limits
            },
        )
        .unwrap_err();
    assert!(error.message.contains("work limit"));
}

#[test]
fn native_stream_controls_keep_suffix_order_duplicates_and_original_ranges() {
    let source = b"% first\r\nx=1;\r\n% zincite-fmt: skip\r\ny=2;\r\n";
    let input = Input::parse(source.to_vec(), FileMode::Data).unwrap();
    let original = query(&input, "items");
    let QueryResult::Selection(original) = &original else {
        panic!()
    };
    let checked = query(
        &input,
        "check | check(2) | check(\"rows\") | check(2, \"rows\")",
    );
    assert_eq!(checked.render(), source);
    let QueryResult::Selection(checked) = checked else {
        panic!()
    };
    assert!(checked.is_document());
    assert!(std::ptr::eq(checked.input(), &input));
    let reversed = query(&input, "tail(99) | reverse | check(2)");
    assert_eq!(
        reversed.render(),
        b"% zincite-fmt: skip\r\ny=2;\r\n% first\r\nx=1;\r\n"
    );
    let QueryResult::Selection(reversed) = reversed else {
        panic!()
    };
    for (item, original) in reversed.items().iter().zip(original.items().iter().rev()) {
        assert_eq!(item.range, original.range);
        assert_eq!(item.source_range, original.source_range);
        assert!(std::ptr::eq(item.node, original.node));
    }
    assert_eq!(query(&input, "tail(1) | names").render(), b"y\n");
    assert!(
        query(&input, "tail(0) | reverse | check(0)")
            .render()
            .is_empty()
    );
    let nodes = query(&input, "subtree | subtree");
    let reversed = query(&input, "subtree | subtree | reverse | tail(3) | check(3)");
    let (QueryResult::Nodes(nodes), QueryResult::Nodes(reversed)) = (nodes, reversed) else {
        panic!()
    };
    assert!(nodes.nodes().iter().enumerate().any(|(i, node)| {
        nodes.nodes()[..i]
            .iter()
            .any(|other| other.range == node.range)
    }));
    assert!(std::ptr::eq(reversed.input(), &input));
    for (node, original) in reversed.nodes().iter().zip(nodes.nodes()[..3].iter().rev()) {
        assert_eq!(node.range, original.range);
        assert!(std::ptr::eq(node.node, original.node));
    }
    let literals = query(&input, "values | reverse | tail(1) | check(1)");
    let QueryResult::Literals(literals) = literals else {
        panic!()
    };
    assert!(std::ptr::eq(literals.input(), &input));
    assert_eq!(
        &input.source_bytes()[literals.values()[0].range.clone()],
        b"1"
    );
    for (expression, diagnostic) in [
        ("tail(0) | check", "expected at least one item, got 0"),
        (
            "check(1, \"need one\")",
            "expected 1 items, got 2: need one",
        ),
        (
            "tail(0) | check(\"need rows\")",
            "expected at least one item, got 0: need rows",
        ),
    ] {
        let error = Query::parse(expression, Limits::default())
            .unwrap()
            .evaluate(&input, Limits::default())
            .unwrap_err();
        assert!(error.message.contains(diagnostic), "{error:?}");
        assert_eq!(
            expression[error.range].trim(),
            expression.split(" | ").last().unwrap()
        );
    }
    for expression in [
        "check()",
        "check(1,2)",
        "check(1,\"x\",2)",
        "reverse()",
        "tail(-1)",
    ] {
        assert!(
            Query::parse(expression, Limits::default()).is_err(),
            "{expression}"
        );
    }
    let paired = Input::parse(
        b"% zincite-fmt: off\nx=1;\ny=2;\n% zincite-fmt: on\nz=3;\n".to_vec(),
        FileMode::Data,
    )
    .unwrap();
    let error = Query::parse("reverse", Limits::default())
        .unwrap()
        .evaluate(&paired, Limits::default())
        .unwrap_err();
    assert_eq!(error.location, ErrorLocation::Input);
    assert!(error.message.contains("unbalance"));
}

#[test]
fn native_stream_controls_reuse_saved_values_and_scoped_arguments() {
    use zincite_query::{ConstructedValue as V, ParameterInputs, Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let input = Input::parse(b"a=1; b=2; c=3;".to_vec(), FileMode::Data).unwrap();
    let source = r#"param message:string; fn bound(n:int)->int { let chosen=n-1; chosen }
        let selected=items; let n=selected | count;
        report selected | tail(bound(n)) | reverse | check(n-1, message) | check(message) | names;
        report selected | names;
        report selected | project(row, {"n": row | tail(bound(2)) | check(node_kind(row)) | check(message) | count});"#;
    let mut result = Program::parse(source, limits)
        .unwrap()
        .evaluate_with_parameters(
            &input,
            ParameterInputs::Typed(&[("message".into(), V::String("rows".into()))]),
            limits,
        )
        .unwrap();
    let json = String::from_utf8(result.render().unwrap()).unwrap();
    let reports: Vec<serde_json::Value> = json
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        reports[0]["result"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["value"]["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["c", "b"]
    );
    assert_eq!(
        reports[1]["result"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["value"]["value"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    assert_eq!(reports[2]["result"]["entries"][0]["value"]["n"]["value"], 1);
    let direct = Program::parse(
        r#"let message="rows"; let selected=items; let n=selected | count;
        selected | tail(n-1) | reverse | check(message) | check(2,message) | names"#,
        limits,
    )
    .unwrap();
    assert_eq!(
        direct.evaluate(&input, limits).unwrap().render().unwrap(),
        b"c\nb\n"
    );
    for source in [
        r#"let unused=head(0) | check(true); report 1;"#,
        r#"fn unused()->int { let bad=head(0) | check(false); 1 } report 1;"#,
        r#"report [1,2] | tail(1);"#,
        r#"let unused=items | reverse; replace a=2; emit;"#,
    ] {
        assert!(
            Program::parse(source, limits)
                .and_then(|program| program.evaluate(&input, limits))
                .is_err(),
            "{source}"
        );
    }
    let escaped = r#"let unused=head(0) | check(1, "bad") | check(/* é */ true); report 1;"#;
    let error = Program::parse(escaped, limits).unwrap_err();
    assert_eq!(&escaped[error.range], "true");
    for limited in [
        ProgramLimits { work: 20, ..limits },
        ProgramLimits {
            results: 1,
            ..limits
        },
    ] {
        let error = direct.evaluate(&input, limited).unwrap_err();
        assert!(error.message.contains("limit"), "{error:?}");
    }
}

#[test]
fn native_stream_checks_preserve_incomplete_semantic_rows() {
    use zincite_query::SemanticStream;
    let source = b"int:a=1; int:b=a+missing; int:c=b; solve satisfy;";
    let model = semantic_model("native-stream-controls", source, b"", "");
    let input = Input::parse(source.to_vec(), FileMode::Model).unwrap();
    let original = semantic(&input, &model, "references | declarations");
    let checked = semantic(
        &input,
        &model,
        "references | declarations | tail(2) | reverse | check(2)",
    );
    assert!(!checked.is_complete());
    assert!(std::ptr::eq(checked.model(), &model));
    assert_eq!(checked.limitations(), original.limitations());
    assert!(
        checked
            .limitations()
            .iter()
            .any(|limitation| &source[limitation.location.range.clone()] == b"missing")
    );
    let (SemanticStream::Entities(original_rows), SemanticStream::Entities(rows)) =
        (original.stream(), checked.stream())
    else {
        panic!()
    };
    assert_eq!(
        rows,
        &original_rows.iter().rev().copied().collect::<Vec<_>>()
    );
    for &row in rows {
        assert_eq!(checked.location(row), original.location(row));
        assert_eq!(checked.location(row).path, model.root);
    }
}

#[test]
fn native_ordering_and_operand_order_sets_keep_values_and_owners() {
    use zincite_query::{Program, ProgramLimits, ProgramNative, ReportValue};
    let limits = ProgramLimits::default();
    let model = semantic_model(
        "native-ordering",
        include_bytes!("fixtures/native-ordering-and-sets.mzn"),
        b"",
        "",
    );
    let input = Input::parse(
        include_bytes!("fixtures/native-ordering-and-sets.mzn").to_vec(),
        FileMode::Model,
    )
    .unwrap();
    let program =
        Program::parse(include_str!("fixtures/native-ordering-and-sets.zq"), limits).unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(
        output.contains(r#""union_names":["C","A","B"]"#),
        "{output}"
    );
    assert!(output.contains(r#""intersection_names":["A"]"#), "{output}");
    assert!(output.contains(r#""difference_names":["C"]"#), "{output}");
    assert!(
        output.contains(r#""sorted_names":["A","B","C"]"#),
        "{output}"
    );
    assert!(
        output.contains(r#""nominal_names":["second","first"]"#),
        "{output}"
    );
    for (stage, names) in [
        (
            "sort_by(expressions | tail(1) | values)",
            vec!["A", "B", "C"],
        ),
        ("min_by(expressions | tail(1) | values)", vec!["A"]),
        ("max_by(expressions | tail(1) | values)", vec!["C"]),
        ("min_all_by(expressions | tail(1) | values)", vec!["A", "B"]),
        ("max_all_by(expressions | tail(1) | values)", vec!["C"]),
    ] {
        let source = format!(
            r#"let rows = items | filter(name("A") or name("B") or name("C")); report rows | {stage};"#
        );
        let program = Program::parse(&source, limits).unwrap();
        let result = program
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let value = result.emissions()[0].value();
        let ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) = value else {
            panic!("{value:?}")
        };
        let QueryResult::Selection(selection) = value.as_ref() else {
            panic!("{value:?}")
        };
        assert!(std::ptr::eq(selection.input(), &input));
        assert_eq!(
            selection
                .items()
                .iter()
                .map(|item| item.name.unwrap())
                .collect::<Vec<_>>(),
            names
        );
        for item in selection.items() {
            assert_eq!(item.range, item.node.range());
        }
    }
    for (source, expected) in [
        (
            r#"items | filter(name("first") or name("second")) | sort_by(expressions | tail(1) | values) | names"#,
            vec!["second", "first"],
        ),
        (
            r#"items | filter(name("positive") or name("negative")) | sort_by(expressions | tail(1) | values) | names"#,
            vec!["negative", "positive"],
        ),
        (
            r#"items | filter(name("C") or name("A")) | names | sort"#,
            vec!["A", "C"],
        ),
    ] {
        let mut result = Program::parse(&format!("report {source};"), limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&result.render().unwrap()).unwrap();
        assert_eq!(json["result"], serde_json::json!(expected));
    }
    let prepared = r#"fn key(n:Node)->string { n | names | one }
        fn id(s:string)->string { s }
        let label = "tie";
        let rows = items | filter(name("A") or name("B") or name("C")) | reverse;
        report rows | sort_by(key(input)) | names;
        report rows | sort_by(id(label)) | names;
        report rows | sort_by((key(input))) | names;
        report rows | sort_by((id(label))) | names;
        report rows | sort_by((items | names | count)) | names;"#;
    let mut result = Program::parse(prepared, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    fn constructed<'v>(value: &'v ReportValue<'_>) -> &'v zirql::ConstructedValue {
        match value {
            ReportValue::Retained(value) => constructed(value),
            ReportValue::Constructed(value) => value,
            ReportValue::SharedConstructed(value) => value,
            _ => panic!("constructed strings: {value:?}"),
        }
    }
    for (emission, names) in result.emissions().iter().zip([
        ["A", "B", "C"],
        ["C", "B", "A"],
        ["A", "B", "C"],
        ["C", "B", "A"],
        ["C", "B", "A"],
    ]) {
        assert_eq!(
            constructed(emission.value()).data(),
            &zirql::ConstructedValue::Array(
                names
                    .into_iter()
                    .map(|name| zirql::ConstructedValue::String(name.into()))
                    .collect()
            )
        );
    }
    result.render().unwrap();
    for source in [
        r#"items | filter(name("missing")) | sort"#,
        r#"items | filter(name("missing")) | sort_by(true)"#,
        r#"fn bad(n:Node)->bool { true } items | filter(name("missing")) | sort_by(bad(input))"#,
        r#"items | sort_by(<>)"#,
        r#"items | filter(name("A") or name("positive")) | sort_by(expressions | tail(1) | values)"#,
        r#"items | sort_by(names | head(0))"#,
        r#"items | sort_by(values | emit)"#,
        r#"(items | values) union (items | values)"#,
        r#"if true { 1 } else { (items | values) union (items | values) }"#,
        r#"if true { 1 } else { items | sort_by(remove) }"#,
    ] {
        let error = match Program::parse(source, limits) {
            Ok(program) => program
                .evaluate_with_model(&input, Some(&model), limits)
                .unwrap_err(),
            Err(error) => error,
        };
        assert_eq!(
            error.location,
            zincite_query::ErrorLocation::Query,
            "{source}: {error:?}"
        );
    }
    for source in [
        r#"items | filter(name("missing")) | expressions | min_by(text)"#,
        r#"items | filter(name("missing")) | declarations | min_by(names)"#,
        r#"(items | filter(name("missing")) | references) union (items | filter(name("missing")) | references)"#,
    ] {
        let result = Program::parse(source, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        match result.emissions()[0].value().stream().unwrap() {
            zincite_query::SemanticStream::Entities(rows) => assert!(rows.is_empty()),
            zincite_query::SemanticStream::Inspection(value) => {
                let QueryResult::Nodes(nodes) = value.as_ref() else {
                    panic!("node selection")
                };
                assert!(std::ptr::eq(nodes.input(), &input));
                assert!(nodes.nodes().is_empty());
            }
            _ => panic!("same-kind empty selection"),
        }
    }
    let singletons = r#"let present((a,_)) = pop_front(queue<Node>(items | filter(name("A"))));
        let present((b,_)) = pop_front(queue<Node>(items | filter(name("B"))));
        report (a union b union a) | names;
        report (a except a) | names;"#;
    let result = Program::parse(singletons, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert_eq!(
        constructed(result.emissions()[0].value()).data(),
        &zirql::ConstructedValue::Array(vec![
            zirql::ConstructedValue::String("A".into()),
            zirql::ConstructedValue::String("B".into())
        ])
    );
    assert_eq!(
        constructed(result.emissions()[1].value()).data(),
        &zirql::ConstructedValue::Array(vec![])
    );
    let tally_input = Input::parse(b"int: z; int: a; int: z;".to_vec(), FileMode::Model).unwrap();
    let mut tally = Program::parse(
        "let counts = items | names | tally; report (counts: counts);",
        limits,
    )
    .unwrap()
    .evaluate(&tally_input, limits)
    .unwrap();
    let output = String::from_utf8(tally.render().unwrap()).unwrap();
    assert!(output.contains(r#""counts":{"z":2,"a":1}"#), "{output}");
    fn l<T>(value: T) -> zirql::Located<T> {
        zirql::Located::new(value, 10..20)
    }
    fn native_query(text: &str) -> zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()> {
        l(zirql::ExpressionKind::Sequence {
            input: Box::new(l(zirql::ExpressionKind::Input)),
            steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Query(
                Query::parse(text, Limits::default()).unwrap(),
            )))],
        })
    }
    let impure = l(zirql::ExpressionKind::Sequence {
        input: Box::new(native_query(r#"items | filter(name("missing"))"#)),
        steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Ordering(
            zirql::NativeOrdering {
                order: zirql::NativeOrder::Sort,
                selector: Some(native_query("names | emit")),
            },
        )))],
    });
    assert!(
        Program::from_values(
            vec![],
            vec![],
            l(zirql::ValueExpressionKind::NativeValue(Box::new(impure))),
            limits
        )
        .is_err()
    );
    let mixed = Input::parse_named(
        b"\xef\xbb\xbfint: A = 1;\r\n".to_vec(),
        FileMode::Model,
        "mixed.mzn",
    )
    .unwrap();
    let QueryResult::Selection(items) = query(&mixed, "items") else {
        panic!("items")
    };
    let QueryResult::Nodes(expressions) = query(&mixed, "items | expressions") else {
        panic!("expressions")
    };
    let declaration = items.items()[0].node;
    let literal = expressions.nodes()[0].node;
    for (left, right, expected) in [
        ("items", "items | expressions", vec![declaration, literal]),
        ("items | expressions", "items", vec![literal, declaration]),
        ("items | head(0)", "items | expressions", vec![literal]),
    ] {
        let source = format!(
            "let left = {left}; let right = {right}; let combined = left union right; report combined; report combined | count;"
        );
        let result = Program::parse(&source, limits)
            .unwrap()
            .evaluate(&mixed, limits)
            .unwrap();
        let Some(zincite_query::SemanticStream::Inspection(value)) =
            result.emissions()[0].value().stream()
        else {
            panic!("native nodes")
        };
        let QueryResult::Nodes(nodes) = value.as_ref() else {
            panic!("compatible node selection")
        };
        assert!(std::ptr::eq(nodes.input(), &mixed));
        assert_eq!(nodes.nodes().len(), expected.len());
        for (actual, expected) in nodes.nodes().iter().zip(&expected) {
            assert!(std::ptr::eq(actual.node, *expected));
            assert_eq!(actual.file, mixed.file());
            assert_eq!(
                actual.range,
                expected.range().start + 3..expected.range().end + 3
            );
        }
        let Some(zincite_query::SemanticStream::Inspection(count)) =
            result.emissions()[1].value().stream()
        else {
            panic!("native count")
        };
        assert!(matches!(count.as_ref(), QueryResult::Count(count) if *count == expected.len()));
    }
    let typed_mixed = l(zirql::ExpressionKind::Set {
        operator: zirql::NativeSetOperator::Union,
        left: Box::new(native_query("items | expressions")),
        right: Box::new(native_query("items")),
    });
    let program = Program::from_values(
        vec![],
        vec![],
        l(zirql::ValueExpressionKind::NativeValue(Box::new(
            typed_mixed,
        ))),
        limits,
    )
    .unwrap();
    let result = program.evaluate(&mixed, limits).unwrap();
    let Some(zincite_query::SemanticStream::Inspection(value)) =
        result.emissions()[0].value().stream()
    else {
        panic!("native nodes")
    };
    let QueryResult::Nodes(nodes) = value.as_ref() else {
        panic!("compatible node selection")
    };
    assert!(std::ptr::eq(nodes.input(), &mixed));
    assert!(std::ptr::eq(nodes.nodes()[0].node, literal));
    assert!(std::ptr::eq(nodes.nodes()[1].node, declaration));
    for (order, expected) in [
        (zirql::NativeOrder::Sort, vec!["B", "A", "C"]),
        (zirql::NativeOrder::Min, vec!["B"]),
        (zirql::NativeOrder::Max, vec!["C"]),
        (zirql::NativeOrder::MinAll, vec!["B", "A"]),
        (zirql::NativeOrder::MaxAll, vec!["C"]),
    ] {
        let value = l(zirql::ExpressionKind::Sequence {
            input: Box::new(native_query(
                r#"items | filter(name("A") or name("B") or name("C")) | reverse"#,
            )),
            steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Ordering(
                zirql::NativeOrdering {
                    order,
                    selector: Some(native_query("expressions | tail(1) | values")),
                },
            )))],
        });
        let program = Program::from_values(
            vec![],
            vec![],
            l(zirql::ValueExpressionKind::NativeValue(Box::new(value))),
            limits,
        )
        .unwrap();
        let result = program
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let Some(zincite_query::SemanticStream::Inspection(value)) =
            result.emissions()[0].value().stream()
        else {
            panic!("native nodes")
        };
        let QueryResult::Selection(selection) = value.as_ref() else {
            panic!("selection")
        };
        assert!(std::ptr::eq(selection.input(), &input));
        assert_eq!(
            selection
                .items()
                .iter()
                .map(|item| item.name.unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
    for (operator, expected) in [
        (zirql::NativeSetOperator::Union, vec!["C", "A", "B"]),
        (zirql::NativeSetOperator::Intersect, vec!["A"]),
        (zirql::NativeSetOperator::Except, vec!["C"]),
    ] {
        let set = l(zirql::ExpressionKind::Set {
            operator,
            left: Box::new(native_query(
                r#"items | filter(name("left")) | references | declarations"#,
            )),
            right: Box::new(native_query(
                r#"items | filter(name("right")) | references | declarations"#,
            )),
        });
        let program = Program::from_values(
            vec![],
            vec![],
            l(zirql::ValueExpressionKind::NativeValue(Box::new(set))),
            limits,
        )
        .unwrap();
        let result = program
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let Some(zincite_query::SemanticStream::Entities(rows)) =
            result.emissions()[0].value().stream()
        else {
            panic!("native entities")
        };
        let report = result.semantic().unwrap();
        let names = rows
            .iter()
            .map(|entity| {
                let zincite_query::SemanticEntity::Declaration(id) = entity else {
                    panic!("declaration")
                };
                assert_eq!(report.location(*entity).path, model.root);
                report.bindings().unwrap().declarations[id.0].name.as_str()
            })
            .collect::<Vec<_>>();
        assert_eq!(names, expected);
    }
    let native = l(zirql::ExpressionKind::Sequence {
        input: Box::new(l(zirql::ExpressionKind::Input)),
        steps: vec![
            l(zirql::ExpressionStep::Stage(ProgramNative::Query(
                Query::parse(r#"items | filter(name("missing"))"#, Limits::default()).unwrap(),
            ))),
            l(zirql::ExpressionStep::Stage(ProgramNative::Ordering(
                zirql::NativeOrdering {
                    order: zirql::NativeOrder::Min,
                    selector: Some(l(zirql::ExpressionKind::Sequence {
                        input: Box::new(l(zirql::ExpressionKind::Input)),
                        steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Query(
                            Query::parse("names", Limits::default()).unwrap(),
                        )))],
                    })),
                },
            ))),
        ],
    });
    let program = Program::from_values(
        vec![],
        vec![],
        l(zirql::ValueExpressionKind::NativeValue(Box::new(native))),
        limits,
    )
    .unwrap();
    let mut result = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert!(
        String::from_utf8(result.render().unwrap())
            .unwrap()
            .contains(r#""result":[]"#)
    );
}

#[test]
fn native_dictionaries_keep_payloads_order_and_preflight_contracts() {
    use zincite_query::{Program, ProgramLimits, ProgramNative, SemanticStream};
    let limits = ProgramLimits::default();
    let source = include_bytes!("fixtures/native-dictionaries.mzn");
    let model = semantic_model("native-dictionaries", source, b"", "");
    let input = Input::parse_named(source.to_vec(), FileMode::Model, "dictionary.mzn").unwrap();
    let mut result = Program::parse(include_str!("fixtures/native-dictionaries.zq"), limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let report = result.emissions()[0].value().fields().unwrap();
    let counts = report[0].1.fields().unwrap();
    assert_eq!(
        counts.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
    let ranked = report[1].1.fields().unwrap();
    assert_eq!(
        ranked.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(),
        ["B", "A"]
    );
    assert!(report[3].1.fields().unwrap().is_empty());
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(output.contains(r#""ranked":{"B":2,"A":1}"#), "{output}");
    assert!(output.contains(r#""reranked":{"C":{"kind":"integer","value":1},"B":{"kind":"integer","value":1},"A":{"kind":"integer","value":1}}"#), "{output}");
    assert!(
        output.contains(
            r#""empty":{},"nested":{"A":{"A":1,"B":2,"C":0}},"tally":{"A":1,"B":1,"C":1}"#
        ),
        "{output}"
    );
    assert!(output.contains(r#""result":{"z":{"inner":{"kind":"integer","value":3}},"a":{"kind":"integer","value":1}}"#), "{output}");

    let retained = r#"let payload = items | filter(name("A") or name("B") or name("C")); let dict = payload | map_by(names, input); report dict | reverse | tail(100);"#;
    let result = Program::parse(retained, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    for (_, value) in result.emissions()[0].value().fields().unwrap() {
        let Some(SemanticStream::Inspection(value)) = value.stream() else {
            panic!("native selection")
        };
        let QueryResult::Selection(selection) = value.as_ref() else {
            panic!("source selection")
        };
        assert!(std::ptr::eq(selection.input(), &input));
        assert_eq!(selection.items().len(), 1);
        assert_eq!(
            selection.items()[0].range,
            selection.items()[0].node.range()
        );
    }
    let payloads = r#"let ds=items | filter(name("A") or name("B") or name("C"));
        let ints=ds | map_by(names, expressions | tail(1) | values);
        let floats=items | filter(name("positive") or name("negative")) | map_by(names, expressions | tail(1) | values);
        let members=items | filter(name("first") or name("second")) | map_by(names, expressions | tail(1) | values);
        let strings=ds | map_by(names, names) | reverse;
        report (ints:ints | sort_by(value), floats:floats | sort_by(value), members:members | sort_by(value), strings:strings | sort_by(value));"#;
    let mut result = Program::parse(payloads, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let fields = result.emissions()[0].value().fields().unwrap();
    assert_eq!(
        fields[0]
            .1
            .fields()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["A", "B", "C"]
    );
    assert_eq!(
        fields[1]
            .1
            .fields()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["negative", "positive"]
    );
    assert_eq!(
        fields[2]
            .1
            .fields()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["second", "first"]
    );
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(
        output.contains(r#""strings":{"A":["A"],"B":["B"],"C":["C"]}"#),
        "{output}"
    );

    let prepared = r#"fn key(n:Node)->string { n | names | one }
        fn key_list(n:Node)->sequence<string> { let name=n | names | one; [name] }
        fn id(n:int)->int { n }
        let saved = 7;
        let rows=items | filter(name("A") or name("B"));
        report rows | map_by((key(input)), (id(saved))) | reverse | sort_by(value);
        report rows | map_by(names, items | count);
        report rows | map_by(key_list(input), names);"#;
    let mut result = Program::parse(prepared, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    assert_eq!(result.emissions()[0].value().fields().unwrap()[0].0, "B");
    let QueryResult::Count(item_count) = query(&input, "items | count") else {
        panic!()
    };
    for (_, value) in result.emissions()[1].value().fields().unwrap() {
        let Some(SemanticStream::Inspection(value)) = value.stream() else {
            panic!("original count: {value:?}")
        };
        assert!(matches!(value.as_ref(),QueryResult::Count(n) if *n == item_count));
    }
    assert_eq!(
        result.emissions()[2]
            .value()
            .fields()
            .unwrap()
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    result.render().unwrap();
    let bounds = r#"let empty=items | head(0) | map_by(names,names); let full=items | filter(name("A") or name("B") or name("C")) | map_by(names,names); let n=0;
        report (empty:empty | sort_by(value) | reverse | tail(100), zero:full | tail(n), full:full | head(100));"#;
    let mut result = Program::parse(bounds, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let output = String::from_utf8(result.render().unwrap()).unwrap();
    assert!(
        output.contains(r#""empty":{},"zero":{},"full":{"A""#),
        "{output}"
    );

    for (source, expected) in [
        (
            r#"items | head(0) | map_by(count,names)"#,
            "exactly one string",
        ),
        (r#"count | map_by(names,names)"#, "require a selection"),
        (
            r#"fn bad(n:Node)->sequence<int> { [1] } let unused=items | head(0) | map_by(bad(input),names); report 1;"#,
            "exactly one string",
        ),
        (
            r#"let key=[["x"]]; items | head(0) | map_by(key,names)"#,
            "exactly one string",
        ),
        (
            r#"let b=true; let unused=items | head(0) | map_by(names,b) | sort_by(value); report 1;"#,
            "supported scalar values",
        ),
        (
            r#"items | head(0) | map_by(names,names | emit)"#,
            "cannot emit",
        ),
        (
            r#"let bad=items | head(0) | map_by(names, input) | sort_by(value); report 1;"#,
            "supported scalar values",
        ),
        (
            r#"fn nested(n:Node)->record{x:int} { (x:1) } let unused=items | head(0) | map_by(names,nested(input)) | sort_by(value); report 1;"#,
            "supported scalar values",
        ),
        (
            r#"items | head(1) | map_by(names | head(0),names)"#,
            "exactly one string",
        ),
        (
            r#"items | head(1) | map_by(items | names,names)"#,
            "exactly one string",
        ),
        (
            r#"items | filter(name("A") or name("positive")) | map_by(names, expressions | tail(1) | values) | sort_by(value)"#,
            "homogeneous key types",
        ),
    ] {
        let error = Program::parse(source, limits)
            .and_then(|program| program.evaluate_with_model(&input, Some(&model), limits))
            .unwrap_err();
        assert!(error.message.contains(expected), "{source}: {error:?}");
    }
    let duplicate = r#"fn payload(n:Node)->int { if (n | names | one) = "B" { 1 div 0 } else { 1 } }
        let key="same"; items | filter(name("A") or name("B")) | map_by(key,payload(input));"#;
    let error = Program::parse(duplicate, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap_err();
    assert!(error.message.contains("duplicate key `same`"), "{error:?}");
    let limited_source = b"int: A=1; int: value=missing + A; solve satisfy;";
    let limited_model = semantic_model("map-limited", limited_source, b"", "");
    let limited_input = Input::parse(limited_source.to_vec(), FileMode::Model).unwrap();
    let mut limited = Program::parse(
        "references | declarations | unique | map_by(names, input)",
        limits,
    )
    .unwrap()
    .evaluate_with_model(&limited_input, Some(&limited_model), limits)
    .unwrap();
    assert!(!limited.is_complete());
    assert!(!limited.limitations().is_empty());
    limited.render().unwrap();

    type Q = zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>;
    fn l<T>(value: T) -> zirql::Located<T> {
        zirql::Located::new(value, 10..20)
    }
    fn native(source: &str) -> Q {
        l(zirql::ExpressionKind::Sequence {
            input: Box::new(l(zirql::ExpressionKind::Input)),
            steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Query(
                Query::parse(source, Limits::default()).unwrap(),
            )))],
        })
    }
    fn mapped(value: &str) -> Q {
        l(zirql::ExpressionKind::Sequence {
            input: Box::new(native(
                r#"declarations | filter(name("A") or name("B") or name("C"))"#,
            )),
            steps: vec![
                l(zirql::ExpressionStep::Stage(ProgramNative::Mapping(
                    zirql::NativeMap {
                        key: native("names"),
                        value: native(value),
                    },
                ))),
                l(zirql::ExpressionStep::Stage(ProgramNative::MapSorting)),
            ],
        })
    }
    let mut typed = Program::from_values(
        vec![],
        vec![],
        l(zirql::ValueExpressionKind::NativeValue(Box::new(mapped(
            "uses | count",
        )))),
        limits,
    )
    .unwrap()
    .evaluate_with_model(&input, Some(&model), limits)
    .unwrap();
    let mut text=Program::parse(r#"declarations | filter(name("A") or name("B") or name("C")) | map_by(names,uses | count) | sort_by(value)"#,limits).unwrap().evaluate_with_model(&input,Some(&model),limits).unwrap();
    assert_eq!(
        typed.emissions()[0]
            .value()
            .fields()
            .unwrap()
            .iter()
            .map(|(k, _)| k.as_str())
            .collect::<Vec<_>>(),
        ["C", "A", "B"]
    );
    assert_eq!(typed.render().unwrap(), text.render().unwrap());
    assert!(
        Program::from_values(
            vec![],
            vec![],
            l(zirql::ValueExpressionKind::NativeValue(Box::new(mapped(
                "names | emit"
            )))),
            limits
        )
        .is_err()
    );
    let mut tally=Program::parse(r#"let native=items | names | tally; let made=["z","a","z"] | tally; report native | reverse | sort_by(value); report made;"#,limits).unwrap().evaluate(&input,limits).unwrap();
    let output = String::from_utf8(tally.render().unwrap()).unwrap();
    assert!(
        output.contains(
            r#""result":{"z":{"kind":"integer","value":2},"a":{"kind":"integer","value":1}}"#
        ),
        "{output}"
    );
}

#[test]
fn native_existence_and_fixed_points_keep_subjects_and_replacement_order() {
    use zincite_query::{Program, ProgramLimits, ProgramNative, SemanticStream};
    let limits = ProgramLimits::default();
    let bytes = include_bytes!("fixtures/worklist-reachability.mzn");
    let model = semantic_model("native-fixed-point", bytes, b"", "");
    let input = Input::parse(bytes.to_vec(), FileMode::Model).unwrap();
    let ids = |source: &str| {
        let result = Program::parse(source, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap();
        let SemanticStream::Entities(rows) = result.emissions()[0].value().stream().unwrap() else {
            panic!("native declarations")
        };
        rows.clone()
    };
    let originals = ids("declarations | filter(name(\"A\") or name(\"C\"))");
    assert_eq!(
        ids("declarations | where_exists(referenced_declarations | filter(name(\"B\")))"),
        originals
    );
    assert!(
        ids("declarations | where_exists(referenced_declarations | filter(name(\"E\")))")
            .is_empty()
    );
    let repeated = "declarations | filter(name(\"B\")) | uses | declarations | reverse";
    let before = ids(repeated);
    assert!(before.len() > 1);
    assert_eq!(ids(&format!("{repeated} | where_exists(input)")), before);
    let saved = "let target=declarations | filter(name(\"B\")); fn related(d:Declaration, wanted:sequence<Declaration>)->sequence<Declaration>{d | where_exists(referenced_declarations intersect wanted)} declarations | where_exists(related(input,target))";
    assert_eq!(ids(saved), originals);
    let single = "let ds=declarations | filter(name(\"A\")); let present((d,_))=pop_front(queue<Declaration>(ds));";
    assert_eq!(
        ids(&format!(
            "{single} d | where_exists(referenced_declarations)"
        )),
        ids("declarations | filter(name(\"A\"))")
    );
    assert!(
        ids(&format!(
            "{single} d | where_exists(referenced_declarations | filter(name(\"E\")))"
        ))
        .is_empty()
    );
    let closure = "let seed=declarations | filter(name(\"A\")); seed | fixpoint(input union referenced_declarations)";
    assert_eq!(ids(closure), ids("declarations | filter(not name(\"E\"))"));
    assert_eq!(
        ids(
            "fn reach(ds:sequence<Declaration>)->sequence<Declaration>{ds | fixpoint(input union referenced_declarations)} let seed=declarations | filter(name(\"A\")); reach(seed)"
        ),
        ids(closure)
    );
    assert_eq!(ids("declarations | fixpoint(input)"), ids("declarations"));
    assert!(
        ids("declarations | filter(name(\"E\")) | fixpoint(referenced_declarations)").is_empty()
    );
    for (source, expected) in [
        ("declarations | fixpoint(reverse)", "cycles"),
        (
            "declarations | head(0) | fixpoint(names)",
            "exact native kind",
        ),
        (
            "declarations | head(0) | where_exists(count)",
            "identity-bearing",
        ),
        ("declarations | head(0) | where_exists(emit)", "emit"),
        ("declarations | head(0) | fixpoint(emit)", "emit"),
    ] {
        let error = Program::parse(source, limits)
            .and_then(|p| p.evaluate_with_model(&input, Some(&model), limits))
            .unwrap_err();
        assert!(error.message.contains(expected), "{source}: {error:?}");
        assert!(!error.range.is_empty());
    }
    let program = Program::parse(closure, limits).unwrap();
    assert!(
        program
            .evaluate_with_model(
                &input,
                Some(&model),
                ProgramLimits {
                    work: 100,
                    ..limits
                }
            )
            .is_err()
    );
    type Q = zirql::Expression<ProgramNative, zirql::NativeSetOperator, ()>;
    fn l<T>(value: T) -> zirql::Located<T> {
        zirql::Located::new(value, 10..20)
    }
    fn native(source: &str) -> Q {
        l(zirql::ExpressionKind::Sequence {
            input: Box::new(l(zirql::ExpressionKind::Input)),
            steps: vec![l(zirql::ExpressionStep::Stage(ProgramNative::Query(
                Query::parse(source, Limits::default()).unwrap(),
            )))],
        })
    }
    let body = l(zirql::ExpressionKind::Set {
        operator: zirql::NativeSetOperator::Union,
        left: Box::new(l(zirql::ExpressionKind::Input)),
        right: Box::new(native("referenced_declarations")),
    });
    let expression = l(zirql::ExpressionKind::Sequence {
        input: Box::new(native("declarations | filter(name(\"A\"))")),
        steps: vec![
            l(zirql::ExpressionStep::Stage(ProgramNative::FixedPoint(
                Box::new(body),
            ))),
            l(zirql::ExpressionStep::Stage(ProgramNative::Exists(
                Box::new(native("referenced_declarations")),
            ))),
        ],
    });
    let program = Program::from_values(
        vec![],
        vec![],
        l(zirql::ValueExpressionKind::NativeValue(Box::new(
            expression,
        ))),
        limits,
    )
    .unwrap();
    let typed = program
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    let SemanticStream::Entities(rows) = typed.emissions()[0].value().stream().unwrap() else {
        panic!("typed native declarations")
    };
    assert_eq!(*rows, ids(closure));
}

#[test]
fn native_nested_observations_refuse_saved_partial_results_and_final_unknown() {
    use zincite_query::{Program, ProgramLimits};
    let limits = ProgramLimits::default();
    let bytes = b"\xef\xbb\xbfint: A=missing; int: E=+1; solve satisfy;";
    let model = semantic_model("native-partial", bytes, b"", "");
    let input = Input::parse(bytes.to_vec(), FileMode::Model).unwrap();
    let prefix = "let ds=declarations | filter(name(\"A\")); let old=ds | referenced_declarations;";
    for suffix in [
        "ds | where_exists(referenced_declarations)",
        "declarations | filter(name(\"E\")) | where_exists(old)",
        "old | fixpoint(input)",
        "let alias=old | reverse | head(0); declarations | filter(name(\"E\")) | where_exists(alias)",
        "let empty=declarations | head(0); let discarded=empty intersect old; empty | fixpoint(discarded)",
    ] {
        let source = format!("{prefix}{suffix}");
        let error = Program::parse(&source, limits)
            .unwrap()
            .evaluate_with_model(&input, Some(&model), limits)
            .unwrap_err();
        assert!(
            matches!(error.location, ErrorLocation::Model(_)),
            "{source}: {error:?}"
        );
        assert_eq!(&bytes[error.range], b"missing");
    }
    let mut result = Program::parse(
        &format!("{prefix}declarations | filter(name(\"E\")) | where_exists(input)"),
        limits,
    )
    .unwrap()
    .evaluate_with_model(&input, Some(&model), limits)
    .unwrap();
    assert!(!result.is_complete());
    assert!(!result.render().unwrap().is_empty());
    let bytes = b"function $T: identity($T:x)=x; par tuple(int,$T): mixed; solve satisfy;";
    let model = semantic_model("native-final-unknown", bytes, b"", "");
    let input = Input::parse(bytes.to_vec(), FileMode::Model).unwrap();
    let source = "declarations | filter(name(\"mixed\")) | where_exists(filter(type(\"tuple\")))";
    let error = Program::parse(source, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap_err();
    assert!(
        matches!(error.location, ErrorLocation::Model(_)),
        "{error:?}"
    );
    let source = "declarations | filter(name(\"mixed\")) | where_exists(filter(type(\"tuple\") or name(\"mixed\")))";
    let mut result = Program::parse(source, limits)
        .unwrap()
        .evaluate_with_model(&input, Some(&model), limits)
        .unwrap();
    assert!(!result.render().unwrap().is_empty());
}

#[test]
fn brace_collections_keep_data_order_function_locals_and_minizinc_forms() {
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::ConstructedValue as V;
    let limits = ProgramLimits::default();
    let input = Input::parse(b"n=1;".to_vec(), FileMode::Data).unwrap();
    let result = Program::parse(include_str!("fixtures/brace-collections.zirql"), limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(value) = result.emissions()[0].value() else {
        panic!("constructed report")
    };
    let V::Record(fields) = value.data() else {
        panic!("dictionary")
    };
    assert_eq!(
        fields
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["set", "dictionary", "access", "empty", "typed", "sorted"]
    );
    let V::Record(entries) = fields[1].1.data() else {
        panic!("computed entries")
    };
    assert_eq!(
        entries
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["pear!", "apple!"]
    );
    assert!(
        matches!(fields[0].1.data(),V::Set {elements,..} if elements == &[V::Integer(3),V::Integer(1)])
    );
    assert!(
        Program::parse(r#"let held={"pear":1}; create report=held; emit;"#, limits)
            .unwrap_err()
            .message
            .contains("report-only")
    );
    let source = r#"fn make(k:string) -> record {"pear":set<int>} { let key=k; {key:{3,1,3}} } let saved=make("pear"); report saved["pear"]; report [{3,1,3}, {n mod 3 | n in 1..5}, (score: 7)];"#;
    let result = Program::parse(source, limits)
        .unwrap()
        .evaluate(&input, limits)
        .unwrap();
    let ReportValue::Constructed(value) = result.emissions()[0].value() else {
        panic!("function result")
    };
    assert!(
        matches!(value.data(),V::Set {elements,..} if elements == &[V::Integer(3),V::Integer(1)])
    );
    let ReportValue::Constructed(value) = result.emissions()[1].value() else {
        panic!("natural forms")
    };
    let V::Array(values) = value.data() else {
        panic!("array")
    };
    assert!(
        matches!(values[1].data(),V::Set {elements,..} if elements == &[V::Integer(1),V::Integer(2),V::Integer(0)])
    );
    assert!(matches!(values[2].data(),V::Record(fields) if fields[0].0=="score"));
}
