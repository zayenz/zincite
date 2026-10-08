//! Explicit file replacement and read-only preview for caller-supplied fixes.
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use crate::fixes::validate_candidate;
use crate::{FixPreparationError, SourceSnapshot};

#[derive(Debug)]
pub enum FileFixError {
    Candidate(FixPreparationError),
    Io(io::Error),
    NotRegular,
    StaleSource,
}
impl std::fmt::Display for FileFixError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Candidate(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::NotRegular => f.write_str("fix target must be a regular non-symlink file"),
            Self::StaleSource => f.write_str("current source differs from the original snapshot"),
        }
    }
}
impl std::error::Error for FileFixError {}
impl From<io::Error> for FileFixError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
fn check_original(snapshot: &SourceSnapshot) -> Result<fs::Metadata, FileFixError> {
    let metadata = fs::symlink_metadata(snapshot.path())?;
    if !metadata.file_type().is_file() {
        return Err(FileFixError::NotRegular);
    }
    if fs::read(snapshot.path())? != snapshot.source_bytes() {
        return Err(FileFixError::StaleSource);
    }
    Ok(metadata)
}

/// Replace only the explicit regular file named by the exact snapshot. Validate
/// model/data syntax again, preserve permissions and sync a completed sibling
/// temporary. Immediately before rename, recheck the target type and every
/// original byte. Failures remove the temporary and leave the original intact.
/// This operation does not establish semantic equivalence, select fixes, load
/// dependencies or perform another fix pass. Callers decide eligibility first.
pub fn replace_fixed_file(snapshot: &SourceSnapshot, candidate: &[u8]) -> Result<(), FileFixError> {
    validate_candidate(snapshot, candidate).map_err(FileFixError::Candidate)?;
    let metadata = check_original(snapshot)?;
    if candidate == snapshot.source_bytes() {
        return Ok(());
    }
    let parent = snapshot
        .path()
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut attempt = 0;
    let (temporary, mut file) = loop {
        let temporary = parent.join(format!(
            ".zincite-lint-{}-{attempt}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => attempt += 1,
            Err(error) => return Err(error.into()),
        }
    };
    let result = (|| {
        file.write_all(candidate)?;
        file.set_permissions(metadata.permissions())?;
        file.sync_all()?;
        drop(file);
        check_original(snapshot)?;
        fs::rename(&temporary, snapshot.path())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

/// Render one deterministic whole-file unified hunk to the supplied writer.
/// Unchanged candidates produce no output. Original CRLF and missing final
/// newlines remain visible; rendering never changes either source or file.
pub fn write_fix_diff(
    snapshot: &SourceSnapshot,
    candidate: &[u8],
    output: &mut impl Write,
) -> io::Result<()> {
    if snapshot.source_bytes() == candidate {
        return Ok(());
    }
    let before: Vec<_> = snapshot
        .source_bytes()
        .split_inclusive(|&byte| byte == b'\n')
        .collect();
    let after: Vec<_> = candidate.split_inclusive(|&byte| byte == b'\n').collect();
    writeln!(output, "--- a/{}", snapshot.path().display())?;
    writeln!(output, "+++ b/{}", snapshot.path().display())?;
    writeln!(
        output,
        "@@ -{},{} +{},{} @@",
        usize::from(!before.is_empty()),
        before.len(),
        usize::from(!after.is_empty()),
        after.len()
    )?;
    for (prefix, lines) in [(b'-', before), (b'+', after)] {
        for line in lines {
            output.write_all(&[prefix])?;
            output.write_all(line)?;
            if !line.ends_with(b"\n") {
                writeln!(output, "\n\\ No newline at end of file")?;
            }
        }
    }
    Ok(())
}
