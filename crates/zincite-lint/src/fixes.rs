//! Read-only planning of atomic edits against exact original source bytes.
//! Lint callers supply findings from selected, unsuppressed diagnostics.
//! Eligibility and syntax validation do not prove semantic safety. No file IO occurs.

use std::fmt;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// One file's exact original content, including its UTF-8 BOM if present.
/// Clones share the retained bytes. Equality compares both path and content;
/// rebuilding a snapshot from the same exact source is equivalent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceSnapshot {
    path: PathBuf,
    source: Arc<[u8]>,
}
impl SourceSnapshot {
    pub fn new(path: impl Into<PathBuf>, source: impl AsRef<[u8]>) -> Self {
        Self {
            path: path.into(),
            source: Arc::from(source.as_ref()),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn source_bytes(&self) -> &[u8] {
        &self.source
    }
}

/// UTF-8 insertions and exact copies from the same original snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditPart {
    Text(String),
    /// A range within the owning edit's target. Copies retain opaque bytes.
    Original(Range<usize>),
}

/// Half-open original-file byte coordinates, including any BOM offset.
/// Untouched bytes are never normalized. Every replacement copy is validated
/// against the owning target range before any candidate is constructed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub replacement: Vec<EditPart>,
}

/// Safety established by a fix producer, not by edit validation or reparsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixSafety {
    /// Preserves solutions, objectives, output, definedness, annotations and comments.
    Safe,
    /// The producer describes the specific behavioural uncertainty.
    Unsafe { reason: String },
}

/// A titled atomic edit group. Every edit applies, or none of them does.
/// `applicability` describes the conditions established by the producer. Missing
/// semantic support cannot justify Safe. Unsafe fixes require separate explicit
/// eligibility from the caller; passing one here does not grant that permission.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    pub title: String,
    pub safety: FixSafety,
    pub applicability: String,
    pub snapshot: SourceSnapshot,
    pub edits: Vec<TextEdit>,
}

/// An omitted group and every other group whose edits conflict with it.
/// Indices refer to the supplied fixes. Every listed group is omitted, even if
/// it also conflicts with another omitted group; there is no first-wins choice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixConflict {
    pub group: usize,
    pub conflicts_with: Vec<usize>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreparedEdits {
    /// Complete source for the caller to reparse before any replacement.
    pub candidate: Vec<u8>,
    pub conflicts: Vec<FixConflict>,
}

/// Invalid input rejects the entire plan without returning a partial candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditPlanError {
    StaleSource,
    SourceMismatch {
        group: usize,
    },
    InvalidGroup {
        group: usize,
        reason: &'static str,
    },
    InvalidEdit {
        group: usize,
        edit: usize,
        reason: &'static str,
    },
    OverlappingEdits {
        group: usize,
        first: usize,
        second: usize,
    },
}
impl fmt::Display for EditPlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaleSource => f.write_str("current source differs from the original snapshot"),
            Self::SourceMismatch { group } => {
                write!(f, "fix group {group} targets a different source snapshot")
            }
            Self::InvalidGroup { group, reason } => {
                write!(f, "invalid fix group {group}: {reason}")
            }
            Self::InvalidEdit {
                group,
                edit,
                reason,
            } => write!(f, "invalid edit {edit} in fix group {group}: {reason}"),
            Self::OverlappingEdits {
                group,
                first,
                second,
            } => write!(
                f,
                "edits {first} and {second} overlap within fix group {group}"
            ),
        }
    }
}
impl std::error::Error for EditPlanError {}

/// Validate all groups against one exact snapshot and the current complete
/// source. A change to any byte, including an untouched comment, is stale.
///
/// Malformed groups reject the whole plan. Across valid groups, every conflict
/// is found before omission, so independent edits survive in any input order.
/// Adjacent nonempty ranges are allowed. Coincident insertions, and insertions
/// on either boundary or inside a replacement, conflict conservatively.
///
/// The returned source preserves all untouched bytes. The caller must reparse
/// it and compare original bytes again before a write; neither parsing nor this
/// operation upgrades a producer's safety claim. No file IO occurs here.
pub fn prepare_edits(
    snapshot: &SourceSnapshot,
    current_source: &[u8],
    groups: &[Fix],
) -> Result<PreparedEdits, EditPlanError> {
    let source = snapshot.source_bytes();
    if current_source != source {
        return Err(EditPlanError::StaleSource);
    }
    for (group, fix) in groups.iter().enumerate() {
        if &fix.snapshot != snapshot {
            return Err(EditPlanError::SourceMismatch { group });
        }
        let invalid = if fix.title.trim().is_empty() {
            Some("title is empty")
        } else if fix.applicability.trim().is_empty() {
            Some("applicability conditions are empty")
        } else if matches!(&fix.safety, FixSafety::Unsafe { reason } if reason.trim().is_empty()) {
            Some("unsafe fix has no described uncertainty")
        } else if fix.edits.is_empty() {
            Some("edit group is empty")
        } else {
            None
        };
        if let Some(reason) = invalid {
            return Err(EditPlanError::InvalidGroup { group, reason });
        }
        validate_edits(source, group, &fix.edits)?;
    }
    let mut conflicts = vec![Vec::new(); groups.len()];
    for first in 0..groups.len() {
        for second in first + 1..groups.len() {
            if groups[first].edits.iter().any(|a| {
                groups[second]
                    .edits
                    .iter()
                    .any(|b| overlaps(&a.range, &b.range))
            }) {
                conflicts[first].push(second);
                conflicts[second].push(first);
            }
        }
    }
    let mut edits: Vec<_> = groups
        .iter()
        .zip(&conflicts)
        .filter(|(_, conflicts)| conflicts.is_empty())
        .flat_map(|(fix, _)| &fix.edits)
        .collect();
    edits.sort_by_key(|edit| (edit.range.start, edit.range.end));
    let candidate = apply_ordered_edits(source, &edits);
    Ok(PreparedEdits {
        candidate,
        conflicts: conflicts
            .into_iter()
            .enumerate()
            .filter(|(_, with)| !with.is_empty())
            .map(|(group, conflicts_with)| FixConflict {
                group,
                conflicts_with,
            })
            .collect(),
    })
}

/// Apply one atomic batch of original-coordinate edits without lint fix metadata
/// or eligibility policy. Stale source, invalid ranges/copies and any overlap
/// reject the whole batch. An empty batch reproduces the original bytes.
/// The caller must validate the complete candidate before output or replacement.
pub fn prepare_text_edits(
    snapshot: &SourceSnapshot,
    current_source: &[u8],
    edits: &[TextEdit],
) -> Result<Vec<u8>, EditPlanError> {
    let source = snapshot.source_bytes();
    if current_source != source {
        return Err(EditPlanError::StaleSource);
    }
    for (index, edit) in edits.iter().enumerate() {
        validate_edit(source, 0, index, edit)?;
    }
    let mut ordered: Vec<_> = edits.iter().enumerate().collect();
    ordered.sort_by_key(|(_, edit)| (edit.range.start, edit.range.end));
    for pair in ordered.windows(2) {
        if overlaps(&pair[0].1.range, &pair[1].1.range) {
            return Err(EditPlanError::OverlappingEdits {
                group: 0,
                first: pair[0].0,
                second: pair[1].0,
            });
        }
    }
    let edits: Vec<_> = ordered.into_iter().map(|(_, edit)| edit).collect();
    Ok(apply_ordered_edits(source, &edits))
}

fn validate_edits(source: &[u8], group: usize, edits: &[TextEdit]) -> Result<(), EditPlanError> {
    for (edit, change) in edits.iter().enumerate() {
        validate_edit(source, group, edit, change)?;
        for (first, previous) in edits[..edit].iter().enumerate() {
            if overlaps(&previous.range, &change.range) {
                return Err(EditPlanError::OverlappingEdits {
                    group,
                    first,
                    second: edit,
                });
            }
        }
    }
    Ok(())
}

fn validate_edit(
    source: &[u8],
    group: usize,
    edit: usize,
    change: &TextEdit,
) -> Result<(), EditPlanError> {
    let range = &change.range;
    let invalid = if range.start > range.end {
        Some("range is reversed")
    } else if range.end > source.len() {
        Some("range is outside the original source")
    } else if !zincite_syntax::is_utf8_boundary(source, range.start)
        || !zincite_syntax::is_utf8_boundary(source, range.end)
    {
        Some("range splits a UTF-8 character")
    } else {
        None
    };
    if let Some(reason) = invalid {
        return Err(EditPlanError::InvalidEdit {
            group,
            edit,
            reason,
        });
    }
    for part in &change.replacement {
        if let EditPart::Original(copy) = part {
            let invalid = if copy.start > copy.end {
                Some("copy range is reversed")
            } else if copy.start < range.start || copy.end > range.end {
                Some("copy range is outside the owning edit")
            } else if !zincite_syntax::is_utf8_boundary(source, copy.start)
                || !zincite_syntax::is_utf8_boundary(source, copy.end)
            {
                Some("copy range splits a UTF-8 character")
            } else {
                None
            };
            if let Some(reason) = invalid {
                return Err(EditPlanError::InvalidEdit {
                    group,
                    edit,
                    reason,
                });
            }
        }
    }
    Ok(())
}

fn apply_ordered_edits(source: &[u8], edits: &[&TextEdit]) -> Vec<u8> {
    let mut candidate = Vec::new();
    let mut cursor = 0;
    for edit in edits {
        candidate.extend_from_slice(&source[cursor..edit.range.start]);
        for part in &edit.replacement {
            match part {
                EditPart::Text(text) => candidate.extend_from_slice(text.as_bytes()),
                EditPart::Original(range) => candidate.extend_from_slice(&source[range.clone()]),
            }
        }
        cursor = edit.range.end;
    }
    candidate.extend_from_slice(&source[cursor..]);
    candidate
}

fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    if a.is_empty() {
        b.start <= a.start && a.start <= b.end
    } else if b.is_empty() {
        a.start <= b.start && b.start <= a.end
    } else {
        a.start < b.end && b.start < a.end
    }
}

/// Edit eligibility, independent of which diagnostics are selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixOptions {
    pub fixable: Vec<crate::Rule>,
    pub unfixable: Vec<crate::Rule>,
    pub unsafe_fixes: bool,
}
impl Default for FixOptions {
    fn default() -> Self {
        Self {
            fixable: crate::Rule::all().collect(),
            unfixable: Vec::new(),
            unsafe_fixes: false,
        }
    }
}
impl FixOptions {
    pub fn allows(&self, rule: crate::Rule, safety: &FixSafety) -> bool {
        self.fixable.contains(&rule)
            && !self.unfixable.contains(&rule)
            && (matches!(safety, FixSafety::Safe) || self.unsafe_fixes)
    }
}

/// One omitted atomic group, identified in the supplied findings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OmittedFix {
    pub finding: usize,
    pub title: String,
    pub conflicts_with: Vec<usize>,
}
#[derive(Debug, PartialEq, Eq)]
pub struct PreparedFixes {
    pub candidate: Vec<u8>,
    pub conflicts: Vec<OmittedFix>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum FixPreparationError {
    Edits(EditPlanError),
    Syntax(Vec<crate::SourceDiagnostic>),
}
impl fmt::Display for FixPreparationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Edits(error) => error.fmt(f),
            Self::Syntax(errors) => {
                write!(f, "fix candidate has {} syntax error(s)", errors.len())
            }
        }
    }
}
impl std::error::Error for FixPreparationError {}

/// Prepare eligible atomic fixes from selected, unsuppressed findings for this
/// file. Include-only findings and other files are ignored. Conflicts identify
/// their original finding indices and titles. The complete candidate is parsed
/// as model or data according to its path, including original BOM coordinates.
/// Parsing establishes syntax only; it does not strengthen the producer's
/// applicability or safety claim. Neither this function nor prepare_edits writes.
pub fn prepare_fixes(
    snapshot: &SourceSnapshot,
    current_source: &[u8],
    findings: &[crate::FileFinding],
    options: &FixOptions,
) -> Result<PreparedFixes, FixPreparationError> {
    let eligible: Vec<_> = findings
        .iter()
        .enumerate()
        .filter_map(|(index, finding)| {
            let fix = finding.fix.as_ref()?;
            (finding.location.path == snapshot.path() && options.allows(finding.rule, &fix.safety))
                .then_some((index, fix))
        })
        .collect();
    let groups: Vec<_> = eligible.iter().map(|(_, fix)| (*fix).clone()).collect();
    let prepared =
        prepare_edits(snapshot, current_source, &groups).map_err(FixPreparationError::Edits)?;
    validate_candidate(snapshot, &prepared.candidate)?;
    let conflicts = prepared
        .conflicts
        .into_iter()
        .map(|conflict| OmittedFix {
            finding: eligible[conflict.group].0,
            title: eligible[conflict.group].1.title.clone(),
            conflicts_with: conflict
                .conflicts_with
                .into_iter()
                .map(|group| eligible[group].0)
                .collect(),
        })
        .collect();
    Ok(PreparedFixes {
        candidate: prepared.candidate,
        conflicts,
    })
}

pub(crate) fn validate_candidate(
    snapshot: &SourceSnapshot,
    candidate: &[u8],
) -> Result<(), FixPreparationError> {
    validate_source_candidate(snapshot, candidate, true)
}

pub(crate) fn validate_source_candidate(
    snapshot: &SourceSnapshot,
    candidate: &[u8],
    check_suppressions: bool,
) -> Result<(), FixPreparationError> {
    let body = candidate.strip_prefix(b"\xef\xbb\xbf").unwrap_or(candidate);
    let offset = candidate.len() - body.len();
    let parsed = zincite_syntax::parse_bytes_with_mode(
        body.to_vec(),
        zincite_syntax::FileMode::from_path(snapshot.path()),
    )
    .map_err(|diagnostic| {
        FixPreparationError::Syntax(vec![crate::SourceDiagnostic {
            location: crate::SourceLocation::from_bytes(
                snapshot.path().to_path_buf(),
                body,
                diagnostic.range,
                offset,
            ),
            message: diagnostic.message,
        }])
    })?;
    let parsed = parsed.analysis_file();
    let diagnostics = if parsed.diagnostics().is_empty() && check_suppressions {
        let items: Vec<_> = parsed.tree().child_nodes().collect();
        crate::item_suppressions(parsed, &items)
            .err()
            .unwrap_or_default()
    } else {
        parsed.diagnostics().to_vec()
    };
    let errors: Vec<_> = diagnostics
        .into_iter()
        .map(|diagnostic| crate::SourceDiagnostic {
            location: crate::SourceLocation::from_parsed(
                snapshot.path().to_path_buf(),
                parsed,
                diagnostic.range,
                offset,
            ),
            message: diagnostic.message,
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(FixPreparationError::Syntax(errors))
    }
}
