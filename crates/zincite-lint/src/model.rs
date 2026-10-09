//! Explicit model loading; ordinary parsing and syntax linting never load files.
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use zincite_syntax::{
    FileMode, NodeKind, ParsedFile, SyntaxElement, TokenKind, literal_include_path,
    parse_bytes_with_mode,
};

use crate::{Rule, item_suppressions};

pub type FileId = usize;

/// Configuration supplied by the caller. The library does not read environment
/// variables or discover an installed MiniZinc library.
#[derive(Debug, Default)]
pub struct ModelOptions {
    pub include_dirs: Vec<PathBuf>,
    pub stdlib_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    User,
    StandardLibrary,
}

/// A location in the original file, including any UTF-8 BOM in the byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub range: Range<usize>,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    /// Locate exact bytes, counting opaque invalid bytes as single columns.
    pub fn from_bytes(
        path: PathBuf,
        source: &[u8],
        range: Range<usize>,
        byte_offset: usize,
    ) -> Self {
        let (line, column) = zincite_syntax::byte_line_column(source, range.start);
        Self {
            path,
            range: range.start + byte_offset..range.end + byte_offset,
            line,
            column,
        }
    }

    pub fn new(path: PathBuf, source: &str, range: Range<usize>, byte_offset: usize) -> Self {
        let mut line = 1;
        let mut column = 1;
        let mut characters = source[..range.start].chars().peekable();
        while let Some(character) = characters.next() {
            if character == '\r' {
                if characters.peek() == Some(&'\n') {
                    characters.next();
                }
                line += 1;
                column = 1;
            } else if character == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
        }
        Self {
            path,
            range: range.start + byte_offset..range.end + byte_offset,
            line,
            column,
        }
    }

    pub(crate) fn from_parsed(
        path: PathBuf,
        parsed: &ParsedFile,
        range: Range<usize>,
        byte_offset: usize,
    ) -> Self {
        let (line, column) = parsed.line_column(range.start);
        Self {
            path,
            range: range.start + byte_offset..range.end + byte_offset,
            line,
            column,
        }
    }

    fn at_path(path: &Path) -> Self {
        Self::new(path.to_path_buf(), "", 0..0, 0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceDiagnostic {
    pub location: SourceLocation,
    pub message: String,
}

#[derive(Debug)]
pub struct ModelFile {
    /// First readable path, retained for diagnostics.
    pub path: PathBuf,
    pub canonical_path: PathBuf,
    /// Analysis view; opaque comment bytes are represented by one-byte markers.
    /// Use source_bytes for exact original spelling.
    pub parsed: ParsedFile,
    /// Exact body when opaque comments require a separate analysis view.
    pub original_bytes: Option<Vec<u8>>,
    pub byte_offset: usize,
    pub kind: SourceKind,
    /// Core declarations reached through std/stdlib.mzn are implicitly available.
    /// Consumers inspect their CST declarations rather than guessing builtins.
    pub implicit: bool,
    pub explicit: bool,
    /// One entry per top-level item. None means syntax/suppression validation failed.
    pub suppressions: Option<Vec<Vec<Rule>>>,
}

impl ModelFile {
    /// Exact source body, excluding the loader's separately retained BOM offset.
    pub fn source_bytes(&self) -> &[u8] {
        self.original_bytes
            .as_deref()
            .unwrap_or_else(|| self.parsed.source().as_bytes())
    }

    /// Retain exact original root bytes on demand, including the BOM removed by
    /// the loader. Calling this for an edit does not copy every included source.
    pub fn source_snapshot(&self) -> crate::SourceSnapshot {
        let source = self.source_bytes();
        if self.byte_offset == 3 {
            let mut bytes = Vec::with_capacity(source.len() + 3);
            bytes.extend_from_slice(b"\xef\xbb\xbf");
            bytes.extend_from_slice(source);
            crate::SourceSnapshot::new(&self.path, bytes)
        } else {
            crate::SourceSnapshot::new(&self.path, source)
        }
    }

    pub fn location(&self, range: Range<usize>) -> SourceLocation {
        SourceLocation::from_parsed(self.path.clone(), &self.parsed, range, self.byte_offset)
    }

    pub fn warnings_enabled(&self) -> bool {
        self.kind == SourceKind::User || self.explicit
    }
}

#[derive(Debug)]
pub struct IncludeEdge {
    pub from: FileId,
    pub target: Option<FileId>,
    pub location: SourceLocation,
}

#[derive(Debug)]
pub struct ModelContext {
    pub root: PathBuf,
    /// Canonical configured std/element.mzn identity retained at load time.
    pub standard_element: Option<PathBuf>,
    pub root_file: Option<FileId>,
    pub implicit_core: Option<FileId>,
    pub files: Vec<ModelFile>,
    pub includes: Vec<IncludeEdge>,
    pub errors: Vec<SourceDiagnostic>,
    pub limitations: Vec<SourceDiagnostic>,
}

/// Load one root independently, retaining readable sources even when another
/// dependency fails. Resolution uses the including directory, ordered include
/// directories, then the configured library's std directory.
pub fn load_model(root: impl AsRef<Path>, options: &ModelOptions) -> ModelContext {
    let root = root.as_ref().to_path_buf();
    let mut loader = Loader {
        context: ModelContext {
            root: root.clone(),
            standard_element: options
                .stdlib_dir
                .as_ref()
                .and_then(|path| path.join("std/element.mzn").canonicalize().ok()),
            root_file: None,
            implicit_core: None,
            files: Vec::new(),
            includes: Vec::new(),
            errors: Vec::new(),
            limitations: Vec::new(),
        },
        options,
        stdlib: options
            .stdlib_dir
            .as_ref()
            .and_then(|path| path.canonicalize().ok()),
        loaded: BTreeMap::new(),
        active: Vec::new(),
    };
    loader.context.root_file = loader.load_file(&root, false, true, None);
    if let Some(directory) = &options.stdlib_dir {
        loader.context.implicit_core =
            loader.load_file(&directory.join("std/stdlib.mzn"), true, false, None);
        if let Some(core) = loader.context.implicit_core {
            // A core file can have been reached first through a user's explicit
            // include. Propagate implicit availability through its retained edges.
            let mut pending = vec![core];
            let mut visited = vec![false; loader.context.files.len()];
            while let Some(id) = pending.pop() {
                if visited[id] {
                    continue;
                }
                visited[id] = true;
                loader.context.files[id].implicit = true;
                pending.extend(
                    loader
                        .context
                        .includes
                        .iter()
                        .filter(|edge| edge.from == id)
                        .filter_map(|edge| edge.target),
                );
            }
        }
        let prelude = directory.join("std/solver_redefinitions.mzn");
        match std::fs::symlink_metadata(&prelude) {
            Ok(_) => {
                let _ = loader.load_file(&prelude, false, false, None);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => loader.error(
                SourceLocation::at_path(&prelude),
                format!("cannot access '{}': {error}", prelude.display()),
            ),
        }
    } else {
        loader.context.limitations.push(SourceDiagnostic {
            location: SourceLocation::at_path(&root),
            message: "standard library is not configured; builtin declarations are unavailable"
                .into(),
        });
    }
    loader.context
}

struct Loader<'a> {
    context: ModelContext,
    options: &'a ModelOptions,
    stdlib: Option<PathBuf>,
    loaded: BTreeMap<PathBuf, FileId>,
    active: Vec<PathBuf>,
}

impl Loader<'_> {
    fn error(&mut self, location: SourceLocation, message: String) {
        self.context
            .errors
            .push(SourceDiagnostic { location, message });
    }

    fn load_file(
        &mut self,
        path: &Path,
        implicit: bool,
        explicit: bool,
        include_location: Option<SourceLocation>,
    ) -> Option<FileId> {
        let location = include_location.unwrap_or_else(|| SourceLocation::at_path(path));
        let canonical = match path.canonicalize() {
            Ok(path) => path,
            Err(error) => {
                self.error(
                    location,
                    format!("cannot load '{}': {error}", path.display()),
                );
                return None;
            }
        };
        if let Some(&id) = self.loaded.get(&canonical) {
            self.context.files[id].implicit |= implicit;
            self.context.files[id].explicit |= explicit;
            if let Some(start) = self.active.iter().position(|active| active == &canonical) {
                // Standard closures reuse non-self back-edges. Every file in the
                // cycle segment must be standard; a user root before it is allowed.
                let standard_reentry = start + 1 < self.active.len()
                    && self.active[start..].iter().all(|active| {
                        self.context.files[self.loaded[active]].kind == SourceKind::StandardLibrary
                    });
                if !standard_reentry {
                    self.error(
                        location,
                        format!("include cycle through '{}'", path.display()),
                    );
                }
            }
            return Some(id);
        }
        let mut bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.error(
                    location,
                    format!("cannot read '{}': {error}", path.display()),
                );
                return None;
            }
        };
        let byte_offset = usize::from(bytes.starts_with(b"\xef\xbb\xbf")) * 3;
        if byte_offset > 0 {
            drop(bytes.drain(..byte_offset));
        }
        // Encoding rejection consumes its input; only opaque input needs a
        // temporary copy for the original error coordinates.
        let encoding_source = std::str::from_utf8(&bytes).is_err().then(|| bytes.clone());
        let parsed = match parse_bytes_with_mode(bytes, FileMode::from_path(path)) {
            Ok(parsed) => parsed,
            Err(error) => {
                self.error(
                    SourceLocation::from_bytes(
                        path.to_path_buf(),
                        encoding_source
                            .as_deref()
                            .expect("encoding error has original bytes"),
                        error.range,
                        byte_offset,
                    ),
                    error.message,
                );
                return None;
            }
        };
        drop(encoding_source);
        let (parsed, original_bytes) = parsed.into_parts();
        let items: Vec<_> = parsed.tree().child_nodes().collect();
        let suppressions = if parsed.diagnostics().is_empty() {
            match item_suppressions(&parsed, &items) {
                Ok(suppressed) => Some(suppressed),
                Err(errors) => {
                    for error in errors {
                        self.error(
                            SourceLocation::from_parsed(
                                path.to_path_buf(),
                                &parsed,
                                error.range,
                                byte_offset,
                            ),
                            error.message,
                        );
                    }
                    None
                }
            }
        } else {
            for error in parsed.diagnostics() {
                self.error(
                    SourceLocation::from_parsed(
                        path.to_path_buf(),
                        &parsed,
                        error.range.clone(),
                        byte_offset,
                    ),
                    error.message.clone(),
                );
            }
            None
        };
        let includes: Vec<_> = items
            .into_iter()
            .filter(|item| item.kind() == NodeKind::Include)
            .map(|item| {
                (
                    item.child_nodes().next().map_or_else(
                        || item.range(),
                        |path| {
                            path.children()
                                .iter()
                                .find_map(|child| match child {
                                    SyntaxElement::Token(index)
                                        if parsed.tokens()[*index].kind
                                            == TokenKind::StringLiteral =>
                                    {
                                        Some(parsed.tokens()[*index].range.clone())
                                    }
                                    _ => None,
                                })
                                .unwrap_or_else(|| path.range())
                        },
                    ),
                    literal_include_path(&parsed, item),
                )
            })
            .collect();
        let id = self.context.files.len();
        self.context.files.push(ModelFile {
            path: path.to_path_buf(),
            canonical_path: canonical.clone(),
            parsed,
            original_bytes,
            byte_offset,
            kind: if self
                .stdlib
                .as_ref()
                .is_some_and(|root| canonical.starts_with(root))
            {
                SourceKind::StandardLibrary
            } else {
                SourceKind::User
            },
            implicit,
            explicit,
            suppressions,
        });
        self.loaded.insert(canonical.clone(), id);
        self.active.push(canonical);
        for (range, requested) in includes {
            let location = self.context.files[id].location(range);
            let target = if let Some(bytes) = requested {
                #[cfg(unix)]
                let requested = {
                    use std::os::unix::ffi::OsStringExt;
                    Some(PathBuf::from(std::ffi::OsString::from_vec(bytes)))
                };
                #[cfg(not(unix))]
                let requested = String::from_utf8(bytes).ok().map(PathBuf::from);
                if let Some(requested) = requested {
                    self.load_include(path, &requested, implicit, location.clone())
                } else {
                    self.context.limitations.push(SourceDiagnostic {
                        location: location.clone(),
                        message: "include path cannot be represented on this platform".into(),
                    });
                    None
                }
            } else {
                self.context.limitations.push(SourceDiagnostic {
                    location: location.clone(),
                    message: "include path requires evaluation; model context is incomplete".into(),
                });
                None
            };
            self.context.includes.push(IncludeEdge {
                from: id,
                target,
                location,
            });
        }
        self.active.pop();
        Some(id)
    }

    fn load_include(
        &mut self,
        including: &Path,
        requested: &Path,
        implicit: bool,
        location: SourceLocation,
    ) -> Option<FileId> {
        let mut directories = vec![including.parent().unwrap_or(Path::new(".")).to_path_buf()];
        directories.extend(self.options.include_dirs.iter().cloned());
        if let Some(stdlib) = &self.options.stdlib_dir {
            directories.push(stdlib.join("std"));
        }
        for directory in directories {
            let candidate = directory.join(requested);
            match std::fs::metadata(&candidate) {
                Ok(_) => return self.load_file(&candidate, implicit, false, Some(location)),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    self.error(
                        location,
                        format!("cannot access '{}': {error}", candidate.display()),
                    );
                    return None;
                }
            }
        }
        self.error(
            location,
            format!("cannot resolve include '{}'", requested.display()),
        );
        None
    }
}
