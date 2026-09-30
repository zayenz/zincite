use std::ops::Range;

use zincite_syntax::{NodeKind, ParsedFile, SyntaxElement, TokenKind};

pub(super) struct IncludeGroup {
    pub children: Range<usize>,
    pub entries: Vec<Range<usize>>,
}

struct Include {
    children: Range<usize>,
    path: Vec<u8>,
}

/// Collect adjacent sortable includes, keeping their attached comments together.
pub(super) fn include_groups(parsed: &ParsedFile, protected: &[Range<usize>]) -> Vec<IncludeGroup> {
    let children = parsed.tree().children();
    let mut groups = Vec::new();
    let mut current: Vec<Include> = Vec::new();
    for (position, child) in children.iter().enumerate() {
        let SyntaxElement::Node(item) = child else {
            continue;
        };
        let previous = children[..position]
            .iter()
            .rposition(|child| matches!(child, SyntaxElement::Node(_)))
            .map_or(0, |position| position + 1);
        let suppressed = children[previous..position].iter().any(|child| {
            matches!(child, SyntaxElement::Token(index)
                if parsed.tokens()[*index].kind == TokenKind::LineComment
                    && parsed.source()[parsed.tokens()[*index].range.clone()].starts_with("% zincite-lint:"))
        });
        let end = inline_end(parsed, position);
        let item_end = match &children[end - 1] {
            SyntaxElement::Node(item) => item.range().end,
            SyntaxElement::Token(index) => parsed.tokens()[*index].range.end,
        };
        let path = if item.kind() == NodeKind::Include
            && !suppressed
            && !protected
                .iter()
                .any(|range| range.start < item_end && item.range().start < range.end)
        {
            zincite_syntax::literal_include_path(parsed, item)
        } else {
            None
        };
        let Some(path) = path else {
            finish_group(&mut current, &mut groups);
            continue;
        };
        let start = attached_start(parsed, position, protected);
        if current.last().is_some_and(|last| {
            children[last.children.end..start].iter().any(|child| {
                !matches!(child, SyntaxElement::Token(index)
                    if parsed.tokens()[*index].kind == TokenKind::Whitespace
                        && !has_blank_line(&parsed.source()[parsed.tokens()[*index].range.clone()])
                        && !protected.iter().any(|range| range.contains(&parsed.tokens()[*index].range.start)))
            })
        }) {
            finish_group(&mut current, &mut groups);
        }
        current.push(Include {
            children: start..end,
            path,
        });
    }
    finish_group(&mut current, &mut groups);
    groups
}

fn finish_group(current: &mut Vec<Include>, groups: &mut Vec<IncludeGroup>) {
    if current.len() > 1 {
        let children = current[0].children.start..current.last().unwrap().children.end;
        current.sort_by(|left, right| {
            (left.path.as_slice() != b"globals.mzn", &left.path)
                .cmp(&(right.path.as_slice() != b"globals.mzn", &right.path))
        });
        groups.push(IncludeGroup {
            children,
            entries: current.iter().map(|item| item.children.clone()).collect(),
        });
    }
    current.clear();
}

fn attached_start(parsed: &ParsedFile, position: usize, protected: &[Range<usize>]) -> usize {
    let children = parsed.tree().children();
    let mut start = position;
    for candidate in (0..position).rev() {
        let SyntaxElement::Token(index) = children[candidate] else {
            break;
        };
        let token = &parsed.tokens()[index];
        if protected
            .iter()
            .any(|range| range.contains(&token.range.start))
        {
            break;
        }
        match token.kind {
            TokenKind::Whitespace if !has_blank_line(&parsed.source()[token.range.clone()]) => {}
            TokenKind::LineComment | TokenKind::BlockComment
                if standalone(parsed, token.range.start) && !is_directive(parsed, index) =>
            {
                start = candidate;
            }
            _ => break,
        }
    }
    start
}

fn inline_end(parsed: &ParsedFile, position: usize) -> usize {
    let children = parsed.tree().children();
    let mut end = position + 1;
    for (candidate, child) in children.iter().enumerate().skip(position + 1) {
        let SyntaxElement::Token(index) = child else {
            break;
        };
        let token = &parsed.tokens()[*index];
        match token.kind {
            TokenKind::Whitespace
                if !parsed.source()[token.range.clone()].contains(['\r', '\n']) => {}
            TokenKind::LineComment | TokenKind::BlockComment
                if !standalone(parsed, token.range.start) && !is_directive(parsed, *index) =>
            {
                end = candidate + 1;
            }
            _ => break,
        }
    }
    end
}

fn standalone(parsed: &ParsedFile, start: usize) -> bool {
    let source = parsed.source();
    let line = source[..start].rfind(['\r', '\n']).map_or(0, |end| end + 1);
    source[line..start]
        .chars()
        .all(|character| matches!(character, ' ' | '\t'))
}

fn is_directive(parsed: &ParsedFile, index: usize) -> bool {
    let token = &parsed.tokens()[index];
    token.kind == TokenKind::LineComment
        && ["% zincite-fmt:", "% zincite-lint:"]
            .iter()
            .any(|prefix| parsed.source()[token.range.clone()].starts_with(prefix))
}

fn has_blank_line(text: &str) -> bool {
    text.replace("\r\n", "\n")
        .chars()
        .filter(|character| matches!(character, '\r' | '\n'))
        .count()
        > 1
}
