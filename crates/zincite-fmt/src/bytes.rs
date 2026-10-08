use std::ops::Range;

use zincite_syntax::{ByteParsedFile, Diagnostic, ParsedFile, SyntaxElement, TokenKind, lex};

use crate::{FormatOptions, format_with_options, includes, protected_ranges};

/// Format byte input, preserving opaque comment bytes and original spelling.
/// Syntax and directive errors return diagnostics without partial output.
pub fn format_bytes(parsed: &ByteParsedFile) -> Result<Vec<u8>, Vec<Diagnostic>> {
    format_bytes_with_options(parsed, &FormatOptions::default())
}

/// Format byte input with the supplied options. Protected spans retain their
/// original bytes; editable layout follows the ordinary formatter contract.
pub fn format_bytes_with_options(
    parsed: &ByteParsedFile,
    options: &FormatOptions,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let analysis = parsed.analysis_file();
    let formatted = format_with_options(analysis, options)?;
    if parsed.source_bytes() == analysis.source().as_bytes() {
        return Ok(formatted.into_bytes());
    }
    let protected = protected_ranges(analysis)?;
    let original_comments = ordered_comments(analysis, &protected);
    let lexed = lex(formatted);
    let mut output_comments = lexed
        .tokens()
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment));
    let mut replacements = Vec::with_capacity(original_comments.len());
    for index in original_comments {
        let original = &analysis.tokens()[index];
        let error = || {
            vec![Diagnostic {
                range: original.range.clone(),
                message: "formatted comment does not match its original spelling or order".into(),
            }]
        };
        let Some(output) = output_comments.next() else {
            return Err(error());
        };
        if output.kind != original.kind
            || lexed.source()[output.range.clone()] != analysis.source()[original.range.clone()]
        {
            return Err(error());
        }
        replacements.push((output.range.clone(), index));
    }
    if output_comments.next().is_some() {
        return Err(vec![Diagnostic {
            range: analysis.tree().range(),
            message: "formatted source contains an additional comment".into(),
        }]);
    }
    let mut formatted = lexed.into_source().into_bytes();
    for (range, index) in replacements {
        formatted[range].copy_from_slice(parsed.token_bytes(index));
    }
    Ok(formatted)
}

fn ordered_comments(parsed: &ParsedFile, protected: &[Range<usize>]) -> Vec<usize> {
    let children = parsed.tree().children();
    let mut groups = includes::include_groups(parsed, protected)
        .into_iter()
        .peekable();
    let mut comments = Vec::new();
    let mut position = 0;
    while position < children.len() {
        if groups
            .peek()
            .is_some_and(|group| group.children.start == position)
        {
            let group = groups.next().unwrap();
            for entry in group.entries {
                for child in &children[entry] {
                    collect_comments(child, parsed, &mut comments);
                }
            }
            position = group.children.end;
        } else {
            collect_comments(&children[position], parsed, &mut comments);
            position += 1;
        }
    }
    comments
}

fn collect_comments(child: &SyntaxElement, parsed: &ParsedFile, comments: &mut Vec<usize>) {
    match child {
        SyntaxElement::Node(node) => {
            for child in node.children() {
                collect_comments(child, parsed, comments);
            }
        }
        SyntaxElement::Token(index) => {
            if matches!(
                parsed.tokens()[*index].kind,
                TokenKind::LineComment | TokenKind::BlockComment
            ) {
                comments.push(*index);
            }
        }
    }
}
