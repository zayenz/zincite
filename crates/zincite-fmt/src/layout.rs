use std::ops::Range;

use zincite_syntax::{Diagnostic, TokenKind, lex};

use crate::FormatOptions;

/// Convert only editable layout, keeping rendered literals/comments and raw
/// directive spans exact, including text copied through matrix cell previews.
pub(super) fn apply_layout(
    output: String,
    mut protected: Vec<Range<usize>>,
    options: &FormatOptions,
) -> Result<String, Vec<Diagnostic>> {
    let lexed = lex(output);
    if !lexed.diagnostics().is_empty() {
        return Err(lexed.diagnostics().to_vec());
    }
    protected.extend(
        lexed
            .tokens()
            .iter()
            .filter(|token| {
                matches!(
                    token.kind,
                    TokenKind::LineComment
                        | TokenKind::BlockComment
                        | TokenKind::StringLiteral
                        | TokenKind::StringHead
                        | TokenKind::StringMiddle
                        | TokenKind::StringTail
                )
            })
            .map(|token| token.range.clone()),
    );
    protected.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in protected {
        if let Some(last) = merged.last_mut().filter(|last| last.end >= range.start) {
            last.end = last.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    let source = lexed.source();
    let mut result = String::with_capacity(source.len());
    let mut position = 0;
    for range in merged {
        editable(&source[position..range.start], &mut result, options);
        result.push_str(&source[range.clone()]);
        position = range.end;
    }
    editable(&source[position..], &mut result, options);
    Ok(result)
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
