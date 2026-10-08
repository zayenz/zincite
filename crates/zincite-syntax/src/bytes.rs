use crate::{Diagnostic, FileMode, ParsedFile, TokenKind, lex, parse_lexed, parse_with_mode};

/// Parse model bytes, accepting invalid UTF-8 only inside comments.
/// Syntax errors remain in the analysis file's diagnostics. Unsupported byte
/// input returns a diagnostic at its original byte range.
pub fn parse_bytes(source: Vec<u8>) -> Result<ByteParsedFile, Diagnostic> {
    parse_bytes_with_mode(source, FileMode::Model)
}

/// Parse model or assignment-only data bytes without resolving includes.
/// All ranges refer to the supplied bytes; a BOM is not stripped automatically.
pub fn parse_bytes_with_mode(
    source: Vec<u8>,
    mode: FileMode,
) -> Result<ByteParsedFile, Diagnostic> {
    let source = match String::from_utf8(source) {
        Ok(source) => {
            return Ok(ByteParsedFile {
                original_bytes: None,
                analysis_file: parse_with_mode(source, mode),
            });
        }
        Err(error) => error.into_bytes(),
    };
    let mut analysis_bytes = source.clone();
    let mut invalid_ranges = Vec::new();
    let mut position = 0;
    while let Err(error) = std::str::from_utf8(&source[position..]) {
        let start = position + error.valid_up_to();
        let end = start + error.error_len().unwrap_or(source.len() - start);
        // Keep delimiters and valid UTF-8 unchanged. A non-whitespace marker
        // cannot turn a malformed comment directive into an accepted one.
        analysis_bytes[start..end].fill(b'?');
        invalid_ranges.push(start..end);
        position = end;
    }
    let analysis_source =
        String::from_utf8(analysis_bytes).expect("invalid UTF-8 was replaced with ASCII");
    let lexed = lex(analysis_source);
    let mut token_index = 0;
    for range in invalid_ranges {
        while lexed.tokens()[token_index].range.end <= range.start {
            token_index += 1;
        }
        let token = &lexed.tokens()[token_index];
        if !matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment)
            || range.end > token.range.end
        {
            return Err(Diagnostic {
                range,
                message: "invalid UTF-8 is only supported inside comments".into(),
            });
        }
    }
    Ok(ByteParsedFile {
        original_bytes: Some(source),
        analysis_file: parse_lexed(lexed, mode),
    })
}

/// Exact source bytes and their syntax, with a separate UTF-8 analysis view.
#[derive(Debug)]
pub struct ByteParsedFile {
    original_bytes: Option<Vec<u8>>,
    analysis_file: ParsedFile,
}

impl ByteParsedFile {
    /// Consume the file into its UTF-8 analysis view and optional original bytes.
    /// When bytes are absent, the analysis source is the exact UTF-8 input.
    pub fn into_parts(self) -> (ParsedFile, Option<Vec<u8>>) {
        (self.analysis_file, self.original_bytes)
    }

    /// Return the exact supplied bytes, including opaque comment bytes.
    pub fn source_bytes(&self) -> &[u8] {
        self.original_bytes
            .as_deref()
            .unwrap_or_else(|| self.analysis_file.source().as_bytes())
    }

    /// Return one retained token's original spelling.
    /// Panics if the token index is out of bounds.
    pub fn token_bytes(&self, index: usize) -> &[u8] {
        &self.source_bytes()[self.analysis_file.tokens()[index].range.clone()]
    }

    /// Return syntax and diagnostics at the original byte coordinates.
    /// Invalid UTF-8 comment bytes appear as one-byte non-whitespace markers in
    /// this view. Its source text is analysis input, not exact original source;
    /// use `source_bytes` or `token_bytes` for original spelling.
    pub fn analysis_file(&self) -> &ParsedFile {
        &self.analysis_file
    }

    /// Return the one-based line and column at an original byte offset.
    /// Valid UTF-8 scalars and opaque invalid bytes each occupy one column.
    /// CR/LF handling and boundary panics follow `ParsedFile::line_column`.
    pub fn line_column(&self, byte_offset: usize) -> (usize, usize) {
        self.analysis_file.line_column(byte_offset)
    }
}

/// Return whether an original-byte offset avoids splitting a valid UTF-8 scalar.
/// Each invalid byte is a boundary; offsets beyond the source are not boundaries.
pub fn is_utf8_boundary(source: &[u8], offset: usize) -> bool {
    if offset > source.len() {
        return false;
    }
    for start in offset.saturating_sub(3)..offset {
        let width = match source[start] {
            0xc2..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf4 => 4,
            _ => continue,
        };
        let end = start + width;
        if offset < end && end <= source.len() && std::str::from_utf8(&source[start..end]).is_ok() {
            return false;
        }
    }
    true
}

/// Return one-based line and column at an original byte boundary.
/// Valid UTF-8 scalars and invalid bytes each occupy one column; CRLF is one break.
/// Panics for an out-of-range offset or one inside a valid UTF-8 scalar.
pub fn byte_line_column(source: &[u8], byte_offset: usize) -> (usize, usize) {
    assert!(
        is_utf8_boundary(source, byte_offset),
        "invalid byte boundary"
    );
    let mut prefix = &source[..byte_offset];
    let mut line = 1;
    let mut column = 1;
    let mut after_cr = false;
    loop {
        let (text, invalid_bytes) = match std::str::from_utf8(prefix) {
            Ok(text) => (text, 0),
            Err(error) => (
                std::str::from_utf8(&prefix[..error.valid_up_to()]).unwrap(),
                error
                    .error_len()
                    .unwrap_or(prefix.len() - error.valid_up_to()),
            ),
        };
        for character in text.chars() {
            match character {
                '\r' => {
                    line += 1;
                    column = 1;
                }
                '\n' if after_cr => {}
                '\n' => {
                    line += 1;
                    column = 1;
                }
                _ => column += 1,
            }
            after_cr = character == '\r';
        }
        if invalid_bytes == 0 {
            return (line, column);
        }
        column += invalid_bytes;
        after_cr = false;
        prefix = &prefix[text.len() + invalid_bytes..];
    }
}
