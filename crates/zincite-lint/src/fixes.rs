//! Read-only planning of atomic edits against exact original UTF-8 source.
//! Callers supply findings from selected, unsuppressed diagnostics.
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
    source: Arc<str>,
}
impl SourceSnapshot {
    pub fn new(path: impl Into<PathBuf>, source: impl Into<Arc<str>>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Half-open original-file byte coordinates, including any BOM offset.
/// Replacements are UTF-8 text; untouched bytes are never normalized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextEdit {
    pub range: Range<usize>,
    pub replacement: String,
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
    pub candidate: String,
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
    current_source: &str,
    groups: &[Fix],
) -> Result<PreparedEdits, EditPlanError> {
    let source = snapshot.source();
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
        for (edit, change) in fix.edits.iter().enumerate() {
            let range = &change.range;
            let invalid = if range.start > range.end {
                Some("range is reversed")
            } else if range.end > source.len() {
                Some("range is outside the original source")
            } else if !source.is_char_boundary(range.start) || !source.is_char_boundary(range.end) {
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
            for (first, previous) in fix.edits[..edit].iter().enumerate() {
                if overlaps(&previous.range, range) {
                    return Err(EditPlanError::OverlappingEdits {
                        group,
                        first,
                        second: edit,
                    });
                }
            }
        }
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
    let mut candidate = String::new();
    let mut cursor = 0;
    for edit in edits {
        candidate.push_str(&source[cursor..edit.range.start]);
        candidate.push_str(&edit.replacement);
        cursor = edit.range.end;
    }
    candidate.push_str(&source[cursor..]);
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
    pub candidate: String,
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
    current_source: &str,
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
    candidate: &str,
) -> Result<(), FixPreparationError> {
    let text = candidate.strip_prefix('\u{feff}').unwrap_or(candidate);
    let offset = candidate.len() - text.len();
    let parsed =
        zincite_syntax::parse_with_mode(text, zincite_syntax::FileMode::from_path(snapshot.path()));
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .map(|diagnostic| crate::SourceDiagnostic {
            location: crate::SourceLocation::new(
                snapshot.path().to_path_buf(),
                text,
                diagnostic.range.clone(),
                offset,
            ),
            message: diagnostic.message.clone(),
        })
        .collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(FixPreparationError::Syntax(errors))
    }
}
