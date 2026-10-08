//! Exact-source edit builders for already established semantic eligibility.
use crate::{
    EditPart, FileId, Fix, FixSafety, ModelContext, SourceLocation, SourceSnapshot, TextEdit,
};
use std::collections::BTreeMap;

pub(super) fn snapshot<'a>(
    context: &ModelContext,
    file: FileId,
    sources: &'a mut BTreeMap<FileId, SourceSnapshot>,
) -> &'a SourceSnapshot {
    sources
        .entry(file)
        .or_insert_with(|| context.files[file].source_snapshot())
}
pub(super) fn unused_name(snapshot: &SourceSnapshot, name: &SourceLocation) -> Fix {
    Fix {
        title: "Replace the unused generator name with _".into(),
        safety: FixSafety::Safe,
        applicability: "The comprehension binder is completely unused; all generators, candidates, repeated terms, guards and evaluations remain in place".into(),
        snapshot: snapshot.clone(),
        edits: vec![TextEdit {
            range: name.range.clone(),
            replacement: vec![EditPart::Text("_".into())],
        }],
    }
}

/// Delimiters come from the proved call's direct CST tokens. All slices between
/// them, including argument annotations and comments, occur exactly once.
pub(super) fn element(
    snapshot: &SourceSnapshot,
    call: &SourceLocation,
    head: &SourceLocation,
    open: usize,
    commas: [usize; 2],
    close: usize,
) -> Fix {
    let replacement = vec![
        EditPart::Original(call.range.start..head.range.start),
        EditPart::Text("(".into()),
        EditPart::Original(head.range.end..open),
        EditPart::Text("(".into()),
        EditPart::Original(commas[1] + 1..close),
        EditPart::Text(") = (".into()),
        EditPart::Original(commas[0] + 1..commas[1]),
        EditPart::Text(")[(".into()),
        EditPart::Original(open + 1..commas[0]),
        EditPart::Text(")])".into()),
        EditPart::Original(close + 1..call.range.end),
    ];
    Fix {
        title: "Express element as indexing equality".into(),
        safety: FixSafety::Safe,
        applicability: "The selected standard predicate has the relation value = array[index], with matching nonoptional scalar types, proved actual index membership, total operands and core equality; original annotations and comments are retained".into(),
        snapshot: snapshot.clone(),
        edits: vec![TextEdit {
            range: call.range.clone(),
            replacement,
        }],
    }
}
