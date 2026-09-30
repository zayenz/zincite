//! One-file developer acceptance check. The Python driver supplies timeouts and
//! inventory/reporting; external source is only read and never rewritten.
use std::collections::HashMap;
use zincite_syntax::{NodeKind, ParsedFile, SyntaxElement, SyntaxNode, TokenKind};

// Source-only expected include ordering reuses the bounded existing groups.
// Independent spelling counts and actual output order remain checked below.
#[path = "../src/includes.rs"]
mod includes;

#[derive(Debug, PartialEq, Eq)]
enum Event<'a> {
    Start(NodeKind),
    End,
    Token(TokenKind, &'a str),
}

fn trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}

fn punctuation_allowed(parsed: &ParsedFile, node: &SyntaxNode, index: usize) -> bool {
    let token = &parsed.tokens()[index];
    if token.kind != TokenKind::Comma
        || !matches!(
            node.kind(),
            NodeKind::ArrayLiteral
                | NodeKind::SetLiteral
                | NodeKind::TupleLiteral
                | NodeKind::RecordLiteral
                | NodeKind::TupleType
                | NodeKind::RecordType
                | NodeKind::ParameterList
                | NodeKind::ArrayType
                | NodeKind::ArrayAccessExpression
                | NodeKind::CallExpression
                | NodeKind::IndexTuple
                | NodeKind::EnumCases
                | NodeKind::GeneratorList
        )
    {
        return false;
    }
    parsed.tokens()[index + 1..]
        .iter()
        .find(|token| !trivia(token.kind))
        .is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::RightParen | TokenKind::RightBracket | TokenKind::RightBrace
            )
        })
}

fn events<'a>(parsed: &ParsedFile, source: &'a str, sort_includes: bool) -> Vec<Event<'a>> {
    let mut result = vec![Event::Start(NodeKind::Root)];
    let children = parsed.tree().children();
    let protected = zincite_fmt::protected_ranges(parsed).unwrap_or_default();
    let groups = if sort_includes {
        includes::include_groups(parsed, &protected)
    } else {
        Vec::new()
    };
    let mut groups = groups.iter().peekable();
    let mut position = 0;
    while position < children.len() {
        if let Some(group) = groups
            .peek()
            .filter(|group| group.children.start == position)
        {
            for range in &group.entries {
                for child in &children[range.clone()] {
                    append(
                        child,
                        parsed,
                        source,
                        matches!(child, SyntaxElement::Node(_)),
                        &mut result,
                    );
                }
            }
            position = group.children.end;
            groups.next();
        } else {
            append(
                &children[position],
                parsed,
                source,
                matches!(&children[position], SyntaxElement::Node(_)),
                &mut result,
            );
            position += 1;
        }
    }
    result.push(Event::End);
    result
}

fn append<'a>(
    child: &SyntaxElement,
    parsed: &ParsedFile,
    source: &'a str,
    final_item: bool,
    events: &mut Vec<Event<'a>>,
) {
    match child {
        SyntaxElement::Node(node) => {
            events.push(Event::Start(node.kind()));
            for child in node.children() {
                match child {
                    SyntaxElement::Token(index) if punctuation_allowed(parsed, node, *index) => {}
                    _ => append(child, parsed, source, false, events),
                }
            }
            // Valid top-level items only omit their terminator at EOF. Canonicalize
            // that permitted insertion before include sorting can move the item.
            if final_item
                && !node.children().iter().any(|child| {
                    matches!(child,
                SyntaxElement::Token(index) if parsed.tokens()[*index].kind == TokenKind::Semicolon)
                })
            {
                events.push(Event::Token(TokenKind::Semicolon, ";"));
            }
            events.push(Event::End);
        }
        SyntaxElement::Token(index) => {
            let token = &parsed.tokens()[*index];
            if token.kind != TokenKind::Whitespace {
                events.push(Event::Token(token.kind, &source[token.range.clone()]));
            }
        }
    }
}

// Protected spans include their boundary trivia; neither layout nor optional
// punctuation normalization applies to these original bytes.
fn protected_slices<'a>(parsed: &ParsedFile, source: &'a str) -> Option<Vec<&'a str>> {
    zincite_fmt::protected_ranges(parsed)
        .ok()
        .map(|ranges| ranges.into_iter().map(|range| &source[range]).collect())
}

fn spelling_counts<'a>(events: &[Event<'a>]) -> HashMap<(usize, &'a str), usize> {
    let mut counts = HashMap::new();
    for event in events {
        if let Event::Token(kind, text) = event {
            *counts.entry((*kind as usize, *text)).or_insert(0) += 1;
        }
    }
    counts
}

fn coverage(parsed: &ParsedFile) -> (bool, bool) {
    let mut position = 0;
    let tokens_ok = parsed.tokens().iter().all(|token| {
        let valid = token.range.start == position
            && token.range.end > position
            && parsed.source().is_char_boundary(token.range.start)
            && parsed.source().is_char_boundary(token.range.end);
        position = token.range.end;
        valid
    }) && position == parsed.source().len();
    fn leaves(node: &SyntaxNode, next: &mut usize) -> bool {
        node.children().iter().all(|child| match child {
            SyntaxElement::Node(node) => leaves(node, next),
            SyntaxElement::Token(index) => {
                let valid = *index == *next;
                *next += 1;
                valid
            }
        })
    }
    let mut next = 0;
    let tree_ok = leaves(parsed.tree(), &mut next) && next == parsed.tokens().len();
    (tokens_ok, tree_ok)
}

fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            value if value.is_control() => result.push_str(&format!("\\u{:04x}", value as u32)),
            value => result.push(value),
        }
    }
    result.push('"');
    result
}

fn messages(errors: &[zincite_syntax::Diagnostic]) -> String {
    let values: Vec<_> = errors
        .iter()
        .take(12)
        .map(|error| {
            format!(
                "{{\"start\":{},\"end\":{},\"message\":{}}}",
                error.range.start,
                error.range.end,
                quoted(&error.message)
            )
        })
        .collect();
    format!("[{}]", values.join(","))
}

fn main() {
    let path = std::env::args_os()
        .nth(1)
        .expect("usage: check-corpus-file FILE");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            println!(
                "{{\"input_status\":\"io_error\",\"message\":{}}}",
                quoted(&error.to_string())
            );
            return;
        }
    };
    let source = match String::from_utf8(bytes) {
        Ok(source) => source,
        Err(error) => {
            println!(
                "{{\"input_status\":\"invalid_utf8\",\"offset\":{}}}",
                error.utf8_error().valid_up_to()
            );
            return;
        }
    };
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let mode = zincite_syntax::FileMode::from_path(&path);
    let parsed = zincite_syntax::parse_with_mode(source, mode);
    let (token_coverage, tree_coverage) = coverage(&parsed);
    let parse_errors = messages(parsed.diagnostics());
    let parse_count = parsed.diagnostics().len();
    let data_model_items = mode == zincite_syntax::FileMode::Data && parse_count > 0 && {
        let model = zincite_syntax::parse(source);
        model.diagnostics().is_empty()
            && model
                .tree()
                .child_nodes()
                .any(|node| node.kind() != NodeKind::Assignment)
    };
    let (lint_status, warnings, naming, labels, lint_errors) = if parse_count > 0 {
        ("skipped_parse_errors", 0, 0, 0, "[]".into())
    } else {
        match zincite_lint::lint(&parsed) {
            Ok(warnings) => {
                let naming = warnings
                    .iter()
                    .filter(|warning| warning.rule == zincite_lint::Rule::Naming)
                    .count();
                let labels = warnings.len() - naming;
                ("ok", warnings.len(), naming, labels, "[]".into())
            }
            Err(errors) => ("directive_error", 0, 0, 0, messages(&errors)),
        }
    };
    let mut format_status = "skipped_parse_errors";
    let mut format_errors = "[]".to_owned();
    let mut reparse_count = 0;
    let mut spellings = "not_run";
    let mut structure = "not_run";
    let mut protected_bytes = "not_run";
    let mut idempotence = "not_run";
    let mut first_difference = "null".to_owned();
    if parse_count == 0 {
        match zincite_fmt::format(&parsed) {
            Err(errors) => {
                format_status = "directive_error";
                format_errors = messages(&errors);
            }
            Ok(formatted) => {
                format_status = "ok";
                let original_protected = protected_slices(&parsed, source);
                let original_counts = spelling_counts(&events(&parsed, source, false));
                let expected = events(&parsed, source, true);
                drop(parsed);
                let reparsed = zincite_syntax::parse_with_mode(&formatted, mode);
                reparse_count = reparsed.diagnostics().len();
                if reparse_count == 0 {
                    protected_bytes = if original_protected.is_some()
                        && original_protected == protected_slices(&reparsed, &formatted)
                    {
                        "ok"
                    } else {
                        "mismatch"
                    };
                    let actual = events(&reparsed, &formatted, false);
                    spellings = if original_counts == spelling_counts(&actual) {
                        "ok"
                    } else {
                        "mismatch"
                    };
                    structure = if expected == actual { "ok" } else { "mismatch" };
                    if structure == "mismatch" {
                        let position = expected
                            .iter()
                            .zip(&actual)
                            .position(|(a, b)| a != b)
                            .unwrap_or(expected.len().min(actual.len()));
                        first_difference = format!(
                            "{{\"event\":{position},\"expected\":{},\"actual\":{}}}",
                            quoted(&format!("{:?}", expected.get(position))),
                            quoted(&format!("{:?}", actual.get(position)))
                        );
                    }
                    idempotence = match zincite_fmt::format(&reparsed) {
                        Ok(second) if second == formatted => "ok",
                        Ok(second) => {
                            if first_difference == "null" {
                                let offset = second
                                    .bytes()
                                    .zip(formatted.bytes())
                                    .position(|(a, b)| a != b)
                                    .unwrap_or(second.len().min(formatted.len()));
                                first_difference = format!(
                                    "{{\"second_pass_byte\":{offset},\"first_length\":{},\"second_length\":{}}}",
                                    formatted.len(),
                                    second.len()
                                );
                            }
                            "mismatch"
                        }
                        Err(_) => "error",
                    };
                }
            }
        }
    }
    println!(
        "{{\"input_status\":\"ok\",\"token_coverage\":{token_coverage},\"tree_coverage\":{tree_coverage},\"parse_count\":{parse_count},\"parse_errors\":{parse_errors},\"data_model_items\":{data_model_items},\"format_status\":{},\"format_errors\":{format_errors},\"reparse_count\":{reparse_count},\"spellings\":{},\"structure\":{},\"protected_bytes\":{},\"idempotence\":{},\"first_difference\":{first_difference},\"lint_status\":{},\"warnings\":{warnings},\"naming_warnings\":{naming},\"label_warnings\":{labels},\"lint_errors\":{lint_errors}}}",
        quoted(format_status),
        quoted(spellings),
        quoted(structure),
        quoted(protected_bytes),
        quoted(idempotence),
        quoted(lint_status)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_bytes_detect_layout_and_punctuation_changes_and_accept_real_output() {
        let source = "% zincite-fmt: skip\nany: x = [1,  2,];\n";
        let parsed = zincite_syntax::parse(source);
        for changed in [
            "% zincite-fmt: skip\nany: x = [1, 2,];\n",
            "% zincite-fmt: skip\nany: x = [1,  2];\n",
            "any: x = [1,  2,];\n",
        ] {
            let changed_parsed = zincite_syntax::parse(changed);
            assert_ne!(
                protected_slices(&parsed, source),
                protected_slices(&changed_parsed, changed)
            );
        }
        for source in [
            source,
            "% zincite-fmt: off\r\nany: x = [1,  2,];\r\n% zincite-fmt: on\r\n\r\nint:y=1;\n",
        ] {
            let parsed = zincite_syntax::parse(source);
            let formatted = zincite_fmt::format(&parsed).unwrap();
            let reparsed = zincite_syntax::parse(&formatted);
            assert_eq!(
                protected_slices(&parsed, source),
                protected_slices(&reparsed, &formatted)
            );
        }
    }
}
