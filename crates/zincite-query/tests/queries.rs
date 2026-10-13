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
    let source = b"\xef\xbb\xbfint: 'capacity';\r\narray[int] of int: reads = [capacity, 'capacity'];\r\nfunction int: shadow(int: capacity) = capacity;\r\nsolve satisfy;\r\n";
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
        source selected = query("declarations");
        source n = selected | query("count");
        source histogram = selected | query("names | tally");
        source references = query("references");
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
    assert_eq!(histogram.get("x"), Some(&1));
    let ReportValue::Query(SemanticStream::Entities(selected)) = &fields[2].1 else {
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
        "source a = b; source b = query(\"count\");",
        "source a = query(\"count\"); source a = query(\"count\");",
        "report (n: query(\"count\"), n: query(\"count\"));",
        "query(\"count | items\");",
    ] {
        let error = Program::parse(source, ProgramLimits::default()).unwrap_err();
        assert!(matches!(
            error.location,
            zincite_query::ErrorLocation::Query
        ));
    }
    let input = Input::parse_named(b"a = 1; b = 2;".to_vec(), FileMode::Data, "data.dzn").unwrap();
    let program =
        Program::parse("source a = query(\"\"); a; a;", ProgramLimits::default()).unwrap();
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
    let source = r#"source labels = query("names");
        source numbers = literal("zeta");
        source n = query("count");
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
        json["result"]["entries"][2]["value"]["fields"][1]["value"]["kind"],
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

    // An explicit let constructs a tagged record even in the legacy overlap.
    let explicit = Program::parse(
        r#"source n = query("count"); let r = (count: n); report r;"#,
        ProgramLimits::default(),
    )
    .unwrap();
    let mut explicit = explicit.evaluate(&input, ProgramLimits::default()).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&explicit.render().unwrap()).unwrap();
    assert_eq!(json["result"]["kind"], "record");
    assert_eq!(
        json["result"]["fields"][0]["value"],
        serde_json::json!({"kind":"integer","value":2})
    );

    // Literal inspection projects nested positional values from the same CST.
    let inspected = Program::parse(
        r#"source written = query("values"); let r = written; report r;"#,
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
        "if true then 1 else max([1]) endif",
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
    let program = Program::parse(r#"source x = literal("a"); report x;"#, limits).unwrap();
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
    let program = Program::parse(r#"source n = query("references | declarations | names | count"); let r = (count: n); report r;"#, ProgramLimits::default()).unwrap();
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
    assert_eq!(json["result"]["fields"][0]["value"]["kind"], "integer");
}

#[test]
fn keyed_reports_retain_model_identity_axes_and_row_major_cells() {
    use std::rc::Rc;
    use zincite_query::{Program, ProgramLimits, ReportValue};
    use zirql::{ConstructedValue as V, ValueKey as K};
    let model_source = b"enum Guest = {Zulu} ++ {Alpha}; array[2..3, Guest] of int: table; array[3..4] of int: tail; array[1..2,1..2] of int: matrix; set of Guest: vacant; array[1..0,Guest] of int: empty; type Info=record(array[Guest] of int: values); Info:info; solve satisfy;";
    let model = semantic_model("keyed-report", model_source, b"", "");
    let input = Input::parse_named(b"\xef\xbb\xbftable = [(3,Alpha):32, (2,Zulu):21, (3,Zulu):31, (2,Alpha):22]; tail=[3:7,8]; matrix=[|3,4|5,6|]; vacant={}; empty=[]; info=(values:[7,9]);".to_vec(), FileMode::Data, "report.dzn").unwrap();
    let source = r#"source t = literal("table"); source tail = literal("tail");
        source matrix = literal("matrix"); source vacant = literal("vacant"); source empty = literal("empty"); source info=literal("info");
        let members = {enum_member(Guest,"Alpha"), enum_member(Guest,"Zulu"), enum_member(Guest,"Zulu")};
        let cells = [(i,e): t[i-1,e] | i in index_set(tail), e in members];
        let zero = [(i,e): 1 | i in 1..0, e in members];
        report (cells: cells, written: t, pair: (tail[3],matrix[2,1]), nested: [[1],[2,3]], members: members, zero: zero, vacant: vacant, empty: empty, info: info);"#;
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
    let V::KeyedArray { axes, entries } = &fields[0].1 else {
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
        entries.iter().map(|e| &e.value).collect::<Vec<_>>(),
        [
            &V::Integer(21),
            &V::Integer(22),
            &V::Integer(31),
            &V::Integer(32)
        ]
    );
    assert_eq!(entries[0].key[0], K::Integer(3));
    let V::KeyedArray { axes, entries } = &fields[5].1 else {
        panic!("known empty result")
    };
    assert_eq!(axes.len(), 2);
    assert!(axes[0].keys.is_empty());
    assert_eq!(axes[1].keys.len(), 2);
    assert!(entries.is_empty());
    let V::Set {
        element_type: Some(zirql::SetElementType::Member(id)),
        elements,
    } = &fields[6].1
    else {
        panic!("typed empty set")
    };
    assert_eq!(id.0, enum_id);
    assert!(elements.is_empty());
    let V::KeyedArray { axes, entries } = &fields[7].1 else {
        panic!("declared empty array")
    };
    assert_eq!(axes.len(), 2);
    assert!(axes[0].keys.is_empty());
    assert_eq!(axes[1].keys.len(), 2);
    assert!(entries.is_empty());
    let V::Record(info) = &fields[8].1 else {
        panic!("nested model-backed record")
    };
    let V::KeyedArray { axes, entries } = &info[0].1 else {
        panic!("written alias field axes")
    };
    assert!(matches!(&axes[0].keys[0], K::Member(member) if member.member_name == "Zulu"));
    assert_eq!(entries[1].value, V::Integer(9));
    assert!(result.is_complete());
    let bytes = result.render().unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["complete"], true);
    assert_eq!(json["limitations"], serde_json::json!([]));
    assert_eq!(json["result"]["fields"][0]["value"]["indexing"], "explicit");
    assert_eq!(
        json["result"]["fields"][0]["value"]["axes"][1][0],
        serde_json::json!({"kind":"member","enum":"Guest","name":"Zulu"})
    );
    assert_eq!(json["result"]["fields"][2]["value"]["kind"], "tuple");
    let mut shared = program
        .evaluate_with_facts(&input, Rc::clone(&facts), limits)
        .unwrap();
    assert_eq!(shared.render().unwrap(), bytes);
    let projected = Program::parse(r#"source rows = query("filter(name(\"table\")) | values"); let row = rows[1]; report row;"#, limits).unwrap().evaluate_with_facts(&input, Rc::clone(&facts), limits).unwrap();
    assert!(
        matches!(projected.emissions()[0].value(), ReportValue::Constructed(V::KeyedArray { axes, entries }) if axes.len() == 2 && entries[0].value == V::Integer(21))
    );
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
        (
            "let bad=[(1,1):1,(1,2):2,(2,1):3]; report [v | v in bad];",
            "missing cells",
        ),
        (
            "report [v | v in [(1,1):1,(1,2):2,(2,1):3]];",
            "missing cells",
        ),
        ("report [i: i | i in 1..3 where i != 2];", "contiguous"),
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
        (
            "report [enum_member(Guest,\"Zulu\"):1,enum_member(Guest,\"Beta\"):2];",
            "contiguous",
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
    let p = Program::parse(r#"source info=literal("info"); report info;"#, limits).unwrap();
    let error = p
        .evaluate_with_model(&subset_input, Some(&subset_model), limits)
        .unwrap_err();
    assert!(error.message.contains("axis domain"), "{error:?}");
    assert_eq!(error.location, ErrorLocation::Input);
    let bom = Input::parse(b"\xef\xbb\xbfx=[Alpha:1,2];".to_vec(), FileMode::Data).unwrap();
    let p = Program::parse(r#"source a=literal("x"); report a;"#, limits).unwrap();
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
        r#"source x=literal("x"); let a=[i:x[i] | i in index_set(x)]; report a;"#,
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
    let source = r#"source old = literal("counts"); let saved = old; replace counts = [saved[i]+1 | i in index_set(saved)]; create keyed = [enum_member(E,"Zulu"):7,enum_member(E,"alpha beta"):9]; create r = (n:1,empty:[(i+0,j):1 | i in 1..0,j in 2..3]); create empty = []; emit;"#;
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
    let request = Program::parse(r#"source a=literal("a"); replace n=3; replace a=[i:if i=4 then 7 else a[i] endif | i in 2..4]; emit;"#, ProgramLimits::default()).unwrap();
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

    let mut short = Program::parse(r#"source d = literal("demand"); report sum(d);"#, limits)
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
        r#"report require(false, max([1]), "unused");"#,
        r#"report require(false, sum([true]), "unused");"#,
        r#"report require(false, missing, "unused");"#,
        r#"report require(1, 2, "unused");"#,
        r#"report require(true, 2, "computed" ++ "message");"#,
        "report require(true, 2);",
        "report sum([1],[2]);",
        "report forall(i in 1..2)(true);",
    ] {
        assert!(Program::parse(source, limits).is_err(), "{source}");
    }
    for (source, call) in [
        ("let unused = sum([1]); replace a=[2]; emit;", "sum([1])"),
        (
            r#"let unused = if false then require(true,1,"x") else 0 endif; create b=1; emit;"#,
            r#"require(true,1,"x")"#,
        ),
        (
            "replace a=if true then [2] else [length([1])] endif; emit;",
            "length([1])",
        ),
    ] {
        let error = Program::parse(source, limits).unwrap_err();
        assert_eq!(&source[error.range], call);
        assert!(error.message.contains("report-only"));
    }
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
    let dynamic = Program::parse(r#"param selected:string; param limit:int; let n=limit+1; source names=query("filter(name(selected)) | head(n % ignored )\n + 0) | names"); report names;"#,limits).unwrap();
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
        r#"param n:int; source names=query("items | head(n /* é ) */) | names"); report names;"#,
        limits,
    )
    .unwrap();
    let unicode_result = unicode
        .evaluate_with_parameters(&input, P::Typed(&[("n".into(), V::Integer(1))]), limits)
        .unwrap();
    assert!(
        matches!(unicode_result.emissions()[0].value(), ReportValue::Query(zincite_query::SemanticStream::Inspection(value)) if matches!(&**value,zincite_query::QueryResult::Strings(names) if names==&vec!["x".to_owned()]))
    );
    let escaped =
        r#"param n:int; source q=query("filter(name(\"x\")) | head(/* é */ \"bad\")"); report q;"#;
    let error = Program::parse(escaped, limits).unwrap_err();
    assert_eq!(&escaped[error.range], r#"\"bad\""#);
    let unchosen = r#"param n:int; source q=query("filter(name(\"missing\") and name(n)) | head(0)"); report q;"#;
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
        "param field:string; source data=literal(field); report data;",
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
