//! Read-only input discovery shared by the formatter and linter.
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};

/// Files in positional-input order, with sorted traversal within each directory.
#[derive(Default)]
pub struct InputFiles {
    pub files: Vec<PathBuf>,
    pub errors: Vec<(PathBuf, io::Error)>,
}

/// Expand directories to `.mzn`/`.dzn` files, retaining the first path to each
/// canonical file. Explicit files keep their spelling and need no extension.
/// Follow directory symlinks once, skip `.git`, and retain independent errors.
pub fn discover_inputs(paths: &[PathBuf]) -> InputFiles {
    let mut result = InputFiles::default();
    let mut directories = HashSet::new();
    let mut files = HashSet::new();
    for path in paths {
        visit(path, true, &mut directories, &mut files, &mut result);
    }
    result
}

fn visit(
    path: &Path,
    explicit: bool,
    directories: &mut HashSet<PathBuf>,
    files: &mut HashSet<PathBuf>,
    result: &mut InputFiles,
) {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => {
            result.errors.push((path.to_owned(), error));
            return;
        }
    };
    if metadata.is_dir() && path.file_name().is_some_and(|name| name == ".git") {
        return;
    }
    if !metadata.is_dir() && !explicit && !source_extension(path) {
        return;
    }
    let canonical = match path.canonicalize() {
        Ok(canonical) => canonical,
        Err(error) => {
            result.errors.push((path.to_owned(), error));
            return;
        }
    };
    if metadata.is_dir() {
        if !directories.insert(canonical) {
            return;
        }
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) => {
                result.errors.push((path.to_owned(), error));
                return;
            }
        };
        let mut children = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) if entry.file_name() != ".git" => children.push(entry.path()),
                Ok(_) => {}
                Err(error) => result.errors.push((path.to_owned(), error)),
            }
        }
        children.sort();
        for child in children {
            visit(&child, false, directories, files, result);
        }
    } else if metadata.is_file() && files.insert(canonical) {
        result.files.push(path.to_owned());
    } else if explicit && !metadata.is_file() {
        result.errors.push((
            path.to_owned(),
            io::Error::new(io::ErrorKind::InvalidInput, "input is not a regular file"),
        ));
    }
}

fn source_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mzn") || extension.eq_ignore_ascii_case("dzn")
        })
}
