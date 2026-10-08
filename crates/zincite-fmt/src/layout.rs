use std::ops::Range;

use zincite_syntax::{Diagnostic, lexical_protected_ranges};

use crate::{FormatOptions, FormattedOutput, LineEnding};

/// Convert only editable layout, keeping rendered literals/comments and raw
/// directive spans exact, including text copied through matrix cell previews.
/// Requested comment ranges use returned-output coordinates only on the
/// unchanged LF path; rewritten layout returns None for a final-output rescan.
pub(super) fn apply_layout(
    output: String,
    mut protected: Vec<Range<usize>>,
    options: &FormatOptions,
    retain_comments: bool,
) -> Result<FormattedOutput, Vec<Diagnostic>> {
    let mut lexical = lexical_protected_ranges(&output)?;
    if retain_comments && options.line_ending == LineEnding::Lf {
        protected.extend(lexical.iter().cloned());
    } else {
        protected.append(&mut lexical);
    }
    protected.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in protected {
        if let Some(last) = merged.last_mut().filter(|last| last.end >= range.start) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let source = output;
    if options.line_ending == LineEnding::Lf {
        let mut position = 0;
        let mut unchanged = true;
        for range in &merged {
            unchanged &= already_lf(&source[position..range.start], options);
            position = range.end;
        }
        if unchanged && already_lf(&source[position..], options) {
            let comments = retain_comments.then(|| {
                // These are independent lexical spans, before directive/literal
                // merging. String chunks cannot begin with comment delimiters.
                lexical.retain(|range| {
                    let text = &source[range.clone()];
                    text.starts_with('%') || text.starts_with("/*")
                });
                lexical
            });
            return Ok(FormattedOutput {
                text: source,
                comment_ranges: comments,
            });
        }
    }
    drop(lexical);
    let mut result = String::with_capacity(source.len());
    let mut position = 0;
    for range in merged {
        editable(&source[position..range.start], &mut result, options);
        result.push_str(&source[range.clone()]);
        position = range.end;
    }
    editable(&source[position..], &mut result, options);
    Ok(FormattedOutput {
        text: result,
        comment_ranges: None,
    })
}

fn already_lf(text: &str, options: &FormatOptions) -> bool {
    let bytes = text.as_bytes();
    !bytes.contains(&b'\r')
        && (!options.trim_trailing_whitespace
            || !bytes
                .windows(2)
                .any(|pair| matches!(pair, [b' ' | b'\t', b'\n'])))
}

fn editable(text: &str, output: &mut String, options: &FormatOptions) {
    // Trimming may reach back within this editable segment, never into the
    // protected literal or comment immediately before it.
    let start = output.len();
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if matches!(character, '\r' | '\n') {
            if character == '\r' && characters.peek() == Some(&'\n') {
                characters.next();
            }
            if options.trim_trailing_whitespace {
                while output.len() > start && output.ends_with([' ', '\t']) {
                    output.pop();
                }
            }
            output.push_str(options.line_ending.text());
        } else {
            output.push(character);
        }
    }
}
