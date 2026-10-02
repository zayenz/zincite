use std::ops::Range;
use zincite_lint::{
    EditPlanError, FileFinding, Fix, FixSafety, FixSupport, LintOptions, ModelOptions, Rule,
    SourceLocation, SourceSnapshot, TextEdit, analyze_file, analyze_model, lint_with_options,
    load_model, prepare_edits, write_analysis,
};
use zincite_syntax::parse;

fn edit(range: Range<usize>, replacement: &str) -> TextEdit {
    TextEdit {
        range,
        replacement: replacement.into(),
    }
}
fn fix(snapshot: &SourceSnapshot, title: &str, edits: Vec<TextEdit>) -> Fix {
    Fix {
        title: title.into(),
        safety: FixSafety::Unsafe {
            reason: "These synthetic edits can change declared names or values".into(),
        },
        applicability: "Explicit test groups exercise byte-edit validation only".into(),
        snapshot: snapshot.clone(),
        edits,
    }
}

#[test]
fn atomic_candidate_preserves_original_bytes_and_returns_metadata_for_reparse() {
    let source = "\u{feff}% π comment\r\nvar int: x=1; % keep\r\nconstraint :: \"kept label\" x=1;\r\nsolve satisfy;\r\n";
    let snapshot = SourceSnapshot::new("model.mzn", source);
    let first = source.find("=1").unwrap() + 1;
    let second = source.rfind("=1").unwrap() + 1;
    let mut literals = fix(
        &snapshot,
        "Spell equivalent integer literals",
        vec![
            edit(second..second + 1, "0x1"),
            edit(first..first + 1, "0x1"),
        ],
    );
    literals.safety = FixSafety::Safe;
    literals.applicability = "Both written literals denote the same integer one".into();
    let solve = source.find("satisfy").unwrap();
    let mut objective = fix(
        &snapshot,
        "Add an objective",
        vec![edit(solve..solve + 7, "minimize x")],
    );
    objective.safety = FixSafety::Unsafe {
        reason: "Adds an objective value instead of satisfaction alone".into(),
    };
    objective.applicability = "The selected solve item is satisfy and x is an integer".into();
    let prepared =
        prepare_edits(&snapshot, source, &[objective.clone(), literals.clone()]).unwrap();
    assert!(prepared.conflicts.is_empty());
    assert_eq!(
        prepared.candidate,
        source
            .replace("=1", "=0x1")
            .replace("satisfy", "minimize x")
    );
    // The existing parser boundary removes a BOM; candidate retains the full file.
    let parsed = parse(prepared.candidate.strip_prefix('\u{feff}').unwrap());
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(snapshot.source(), source);
    assert_eq!(snapshot.path().to_str(), Some("model.mzn"));
    assert_eq!(literals.safety, FixSafety::Safe);
    assert!(
        matches!(objective.safety, FixSafety::Unsafe { reason } if reason.contains("objective"))
    );
    assert!(prepared.candidate.contains("% π comment\r\n"));
    assert!(
        prepared
            .candidate
            .contains("% keep\r\nconstraint :: \"kept label\"")
    );
}

#[test]
fn stale_and_invalid_atomic_groups_reject_without_a_candidate() {
    let source = "% π\r\nvar int: x=1; solve satisfy;\r\n";
    let snapshot = SourceSnapshot::new("model.mzn", source);
    let one = source.find("=1").unwrap() + 1;
    let valid = fix(
        &snapshot,
        "Spell equivalent literal",
        vec![edit(one..one + 1, "0x1")],
    );
    assert_eq!(
        prepare_edits(
            &snapshot,
            &source.replace('π', "x"),
            std::slice::from_ref(&valid)
        ),
        Err(EditPlanError::StaleSource)
    );
    let mut other = valid.clone();
    other.snapshot = SourceSnapshot::new("other.mzn", source);
    assert_eq!(
        prepare_edits(&snapshot, source, &[other]),
        Err(EditPlanError::SourceMismatch { group: 0 })
    );
    let mut changed = valid.clone();
    changed.snapshot = SourceSnapshot::new("model.mzn", source.replace("satisfy", "minimize x"));
    assert_eq!(
        prepare_edits(&snapshot, source, &[changed]),
        Err(EditPlanError::SourceMismatch { group: 0 })
    );
    let pi = source.find('π').unwrap();
    for range in [pi + 1..pi + 2, source.len()..source.len() + 1, one + 1..one] {
        let mut group = valid.clone();
        group.edits.push(edit(range, "x"));
        assert!(matches!(
            prepare_edits(&snapshot, source, &[group]),
            Err(EditPlanError::InvalidEdit {
                group: 0,
                edit: 1,
                ..
            })
        ));
    }
    let mut internal_overlap = valid.clone();
    internal_overlap.edits.push(edit(one..one + 1, "2"));
    assert!(matches!(
        prepare_edits(&snapshot, source, &[internal_overlap]),
        Err(EditPlanError::OverlappingEdits { group: 0, .. })
    ));
    let empty = fix(&snapshot, "No edits", vec![]);
    assert!(matches!(
        prepare_edits(&snapshot, source, &[empty]),
        Err(EditPlanError::InvalidGroup { .. })
    ));
    let mut undescribed = valid;
    undescribed.safety = FixSafety::Unsafe { reason: " ".into() };
    assert!(matches!(
        prepare_edits(&snapshot, source, &[undescribed]),
        Err(EditPlanError::InvalidGroup { .. })
    ));
    assert_eq!(snapshot.source(), source);
}

#[test]
fn all_conflicting_groups_are_omitted_and_independent_edits_survive_any_order() {
    let source = "var int: x=1; % keep\r\nsolve satisfy;\r\n";
    let snapshot = SourceSnapshot::new("model.mzn", source);
    let x = source.find("x=").unwrap();
    let one = x + 2;
    let atomic = fix(
        &snapshot,
        "Two edits",
        vec![edit(x..x + 1, "y"), edit(one..one + 1, "2")],
    );
    let name = fix(&snapshot, "Overlapping name", vec![edit(x..x + 1, "z")]);
    let value = fix(
        &snapshot,
        "Overlapping value",
        vec![edit(one..one + 1, "3")],
    );
    let independent = fix(
        &snapshot,
        "Independent comment",
        vec![edit(source.len()..source.len(), "% independent\r\n")],
    );
    for groups in [
        vec![
            atomic.clone(),
            independent.clone(),
            name.clone(),
            value.clone(),
        ],
        vec![value, name, independent.clone(), atomic],
    ] {
        let result = prepare_edits(&snapshot, source, &groups).unwrap();
        assert_eq!(result.candidate, format!("{source}% independent\r\n"));
        assert_eq!(result.conflicts.len(), 3);
        assert!(result.conflicts.iter().all(
            |c| !c.conflicts_with.is_empty() && groups[c.group].title != "Independent comment"
        ));
        for c in &result.conflicts {
            for other in &c.conflicts_with {
                assert!(
                    result
                        .conflicts
                        .iter()
                        .any(|back| back.group == *other && back.conflicts_with.contains(&c.group))
                );
            }
        }
    }
    let replacement = fix(&snapshot, "Replacement", vec![edit(x..x + 1, "y")]);
    let start = fix(&snapshot, "Start insertion", vec![edit(x..x, "_")]);
    let end = fix(&snapshot, "End insertion", vec![edit(x + 1..x + 1, "_")]);
    let result = prepare_edits(&snapshot, source, &[replacement, start, end, independent]).unwrap();
    assert_eq!(result.conflicts.len(), 3);
    assert_eq!(result.candidate, format!("{source}% independent\r\n"));
    let insertion = fix(&snapshot, "First insertion", vec![edit(x..x, "_")]);
    let same = fix(&snapshot, "Coincident insertion", vec![edit(x..x, "a")]);
    let result = prepare_edits(&snapshot, source, &[same, insertion]).unwrap();
    assert_eq!(result.conflicts.len(), 2);
    assert_eq!(result.candidate, source);
    let adjacent = SourceSnapshot::new("adjacent.mzn", "var  int: x=1; solve satisfy;");
    let left = fix(&adjacent, "First space", vec![edit(3..4, "\t")]);
    let right = fix(&adjacent, "Second space", vec![edit(4..5, " ")]);
    let result = prepare_edits(&adjacent, adjacent.source(), &[right, left]).unwrap();
    assert!(result.conflicts.is_empty());
    assert_eq!(result.candidate, "var\t int: x=1; solve satisfy;");
    assert!(parse(result.candidate).diagnostics().is_empty());
    assert_eq!(snapshot.source(), source);
}

#[test]
fn selected_unsuppressed_findings_supply_groups_and_conversion_keeps_bom_coordinates() {
    let source = "\u{feff}% zincite-lint: ignore naming\r\nvar int:SuppressedName=1;\r\nvar int:BadName=1; % π preserved\r\nsolve satisfy;\r\n";
    let body = source.strip_prefix('\u{feff}').unwrap();
    let parsed = parse(body);
    let options = LintOptions::from_selection("naming").unwrap();
    let mut diagnostics = lint_with_options(&parsed, &options).unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains("BadName"));
    assert!(diagnostics[0].fix.is_none());
    let snapshot = SourceSnapshot::new("model.mzn", source);
    let mut diagnostic = diagnostics.remove(0);
    let location = SourceLocation::new("model.mzn".into(), body, diagnostic.range.clone(), 3);
    let end = location.range.end;
    let mut supplied = fix(
        &snapshot,
        "Test-only token spacing",
        vec![edit(end..end, " ")],
    );
    supplied.safety = FixSafety::Safe;
    supplied.applicability = "Whitespace separates the same identifier and equals token".into();
    diagnostic.fix = Some(supplied.clone());
    let converted = FileFinding::from_diagnostic(diagnostic, location.clone());
    assert_eq!(converted.location, location);
    assert_eq!(converted.fix.as_ref(), Some(&supplied));
    assert_eq!(converted.fix.as_ref().unwrap().edits[0].range, end..end);
    let result = analyze_file(&parsed, "model.mzn", 3, &options);
    assert_eq!(result.findings.len(), 1);
    assert!(result.findings[0].fix.is_none());
    let mut before = Vec::new();
    assert_eq!(write_analysis(&result, &mut before).unwrap(), 1);
    let mut attached = result;
    attached.findings[0] = converted;
    let mut after = Vec::new();
    assert_eq!(write_analysis(&attached, &mut after).unwrap(), 1);
    assert_eq!(after, before);
    let groups: Vec<_> = attached
        .findings
        .iter()
        .filter_map(|f| f.fix.clone())
        .collect();
    let prepared = prepare_edits(&snapshot, source, &groups).unwrap();
    assert_eq!(prepared.candidate, source.replace("BadName=", "BadName ="));
    let disabled = analyze_file(
        &parsed,
        "model.mzn",
        3,
        &LintOptions::from_selection("missing-constraint-label").unwrap(),
    );
    let empty: Vec<_> = disabled
        .findings
        .iter()
        .filter_map(|f| f.fix.clone())
        .collect();
    assert!(empty.is_empty());
    assert_eq!(
        prepare_edits(&snapshot, source, &empty).unwrap().candidate,
        source
    );
    let dir = std::env::temp_dir().join(format!("zincite-fix-model-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("model.mzn");
    std::fs::write(&path, source).unwrap();
    let context = load_model(&path, &ModelOptions::default());
    let model = analyze_model(&context, &options);
    assert!(model.errors.is_empty());
    assert_eq!(model.findings.len(), 1);
    assert!(model.findings[0].fix.is_none());
    assert!(model.findings[0].message.contains("BadName"));
    assert_eq!(std::fs::read_to_string(path).unwrap(), source);
    std::fs::remove_dir_all(dir).unwrap();
    assert!(Rule::all().all(|r| r.metadata().fix_support == FixSupport::None));
    assert_eq!(Rule::DEFAULT.len(), 2);
    assert_eq!(Rule::THESIS.len(), 14);
    assert_eq!(snapshot.source(), source);
}
