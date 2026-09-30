use std::ops::Range;

use zincite_syntax::{Diagnostic, ParsedFile, TokenKind};

#[derive(Clone, Copy)]
enum Directive {
    Skip,
    Off,
    On,
}

enum Pending {
    Skip(usize),
    Off(Range<usize>, usize),
}

/// Validate formatting directives and return their protected source ranges.
///
/// Ranges include marker lines and boundary trivia, from the preceding complete
/// item's end to the following item's start. They are ordered and disjoint, and
/// can also serve as barriers when sorting includes. Skipped syntax is checked
/// before directives; errors produce no ranges or formatted output.
pub fn protected_ranges(parsed: &ParsedFile) -> Result<Vec<Range<usize>>, Vec<Diagnostic>> {
    if !parsed.diagnostics().is_empty() {
        return Err(parsed.diagnostics().to_vec());
    }
    let items: Vec<_> = parsed
        .tree()
        .child_nodes()
        .map(|item| item.range())
        .collect();
    let source = parsed.source();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    let mut pending = None;
    for token in parsed.tokens() {
        if token.kind != TokenKind::LineComment {
            continue;
        }
        let text = &source[token.range.clone()];
        if !text.starts_with("% zincite-fmt:") {
            continue;
        }
        let error = |message: &str| {
            vec![Diagnostic {
                range: token.range.clone(),
                message: message.into(),
            }]
        };
        let directive = match text.trim_end() {
            "% zincite-fmt: skip" => Directive::Skip,
            "% zincite-fmt: off" => Directive::Off,
            "% zincite-fmt: on" => Directive::On,
            _ => return Err(error("unknown or malformed formatting directive")),
        };
        let line_start = source[..token.range.start]
            .rfind(['\r', '\n'])
            .map_or(0, |position| position + 1);
        if !source[line_start..token.range.start]
            .chars()
            .all(|character| matches!(character, ' ' | '\t'))
        {
            return Err(error("formatting directive must be a standalone comment"));
        }
        if items.iter().any(|item| item.contains(&token.range.start)) {
            return Err(error(
                "formatting directive must be between complete top-level items",
            ));
        }
        // A skip remains pending until its item, preventing another marker from
        // changing what it controls. Later markers start independent spans.
        if matches!(pending, Some(Pending::Skip(end)) if token.range.start >= end) {
            pending = None;
        }
        let previous_end = items
            .iter()
            .rev()
            .find(|item| item.end <= token.range.start)
            .map_or(0, |item| item.end);
        let next = items.iter().position(|item| item.start >= token.range.end);
        let range = match directive {
            Directive::Skip => {
                if pending.is_some() {
                    return Err(error("nested formatting directive"));
                }
                let Some(next) = next else {
                    return Err(error("formatting skip directive has no following item"));
                };
                pending = Some(Pending::Skip(items[next].end));
                Some(previous_end..items.get(next + 1).map_or(source.len(), |item| item.start))
            }
            Directive::Off => {
                if pending.is_some() {
                    return Err(error("nested formatting directive"));
                }
                pending = Some(Pending::Off(token.range.clone(), previous_end));
                None
            }
            Directive::On => {
                let Some(Pending::Off(_, start)) = pending.take() else {
                    return Err(error(
                        "formatting on directive has no matching off directive",
                    ));
                };
                Some(start..next.map_or(source.len(), |next| items[next].start))
            }
        };
        if let Some(range) = range {
            if let Some(previous) = ranges
                .last_mut()
                .filter(|previous| previous.end >= range.start)
            {
                previous.end = previous.end.max(range.end);
            } else {
                ranges.push(range);
            }
        }
    }
    if let Some(Pending::Off(range, _)) = pending {
        return Err(vec![Diagnostic {
            range,
            message: "formatting off directive has no matching on directive".into(),
        }]);
    }
    Ok(ranges)
}
