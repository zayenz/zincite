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

#[test]
fn eligibility_validation_and_preview_keep_fix_selection_independent() {
    use zincite_lint::settings::LintSettings;
    use zincite_lint::{FixOptions, FixPreparationError, prepare_fixes, write_fix_diff};
    let source = "\u{feff}% π\r\nvar int:BadName=1;\r\nsolve satisfy;";
    let snapshot = SourceSnapshot::new("model.mzn", source);
    let one = source.find("=1").unwrap() + 1;
    let mut safe = fix(
        &snapshot,
        "Equivalent literal",
        vec![edit(one..one + 1, "0x1")],
    );
    safe.safety = FixSafety::Safe;
    safe.applicability =
        "Both literal spellings denote integer one; all other source bytes remain unchanged".into();
    let solve = source.find("satisfy").unwrap();
    let unsafe_fix = fix(
        &snapshot,
        "Test objective",
        vec![edit(solve..solve + 7, "minimize BadName")],
    );
    let findings = [
        supplied_finding(&snapshot, Rule::Naming, safe.clone()),
        supplied_finding(&snapshot, Rule::MissingConstraintLabel, unsafe_fix),
    ];
    let prepared = prepare_fixes(&snapshot, source, &findings, &FixOptions::default()).unwrap();
    assert_eq!(prepared.candidate, source.replace("=1", "=0x1"));
    let mut preview = Vec::new();
    write_fix_diff(&snapshot, &prepared.candidate, &mut preview).unwrap();
    let preview = String::from_utf8(preview).unwrap();
    assert!(preview.starts_with("--- a/model.mzn\n+++ b/model.mzn\n@@ -1,3 +1,3 @@\n"));
    assert!(preview.contains("-% π\r\n") || preview.contains("-\u{feff}% π\r\n"));
    assert_eq!(preview.matches("\\ No newline at end of file").count(), 2);
    assert_eq!(snapshot.source(), source);
    let options = FixOptions {
        unsafe_fixes: true,
        ..FixOptions::default()
    };
    assert!(
        prepare_fixes(&snapshot, source, &findings, &options)
            .unwrap()
            .candidate
            .contains("minimize BadName")
    );
    let settings = LintSettings::from_toml(
        "[lint]\nselect=[]\nfixable=['family:style']\nunfixable=['naming']\n",
    )
    .unwrap();
    assert!(settings.resolve().unwrap().rules.is_empty());
    assert_eq!(
        settings
            .resolve_selection(&["all".into()])
            .unwrap()
            .rules
            .len(),
        Rule::all().count()
    );
    assert_eq!(
        prepare_fixes(
            &snapshot,
            source,
            &findings,
            &settings.resolve_fixes().unwrap()
        )
        .unwrap()
        .candidate,
        source
    );
    for invalid in [
        "[lint]\nselect=[]\nfixable=['typo']",
        "[lint]\nunfixable=['family:typo']",
    ] {
        assert!(LintSettings::from_toml(invalid).is_err());
    }
    let conflict = supplied_finding(&snapshot, Rule::Naming, safe.clone());
    let mut mixed = vec![supplied_finding(
        &SourceSnapshot::new("include.mzn", source),
        Rule::Naming,
        safe.clone(),
    )];
    mixed.extend([supplied_finding(&snapshot, Rule::Naming, safe), conflict]);
    let prepared = prepare_fixes(&snapshot, source, &mixed, &options).unwrap();
    assert_eq!(prepared.candidate, source);
    assert_eq!(
        prepared
            .conflicts
            .iter()
            .map(|c| (c.finding, c.title.as_str(), c.conflicts_with.clone()))
            .collect::<Vec<_>>(),
        [
            (1, "Equivalent literal", vec![2]),
            (2, "Equivalent literal", vec![1])
        ]
    );
    let invalid = supplied_finding(
        &snapshot,
        Rule::Naming,
        fix(&snapshot, "Break syntax", vec![edit(one..one + 1, ";")]),
    );
    assert!(matches!(
        prepare_fixes(&snapshot, source, &[invalid], &options),
        Err(FixPreparationError::Syntax(_))
    ));
    let data = SourceSnapshot::new("values.dzn", "\u{feff}x=1;\r\n");
    let invalid = supplied_finding(
        &data,
        Rule::Naming,
        fix(
            &data,
            "Model-only item",
            vec![edit(
                data.source().len()..data.source().len(),
                "solve satisfy;",
            )],
        ),
    );
    assert!(
        matches!(prepare_fixes(&data, data.source(), &[invalid], &options), Err(FixPreparationError::Syntax(errors)) if errors.iter().all(|e| e.location.range.start >= 3))
    );
    let mut empty = Vec::new();
    write_fix_diff(&snapshot, source, &mut empty).unwrap();
    assert!(empty.is_empty());
}

fn supplied_finding(snapshot: &SourceSnapshot, rule: Rule, fix: Fix) -> FileFinding {
    FileFinding {
        location: SourceLocation::new(snapshot.path().to_path_buf(), snapshot.source(), 0..0, 0),
        rule,
        severity: zincite_lint::Severity::Warning,
        message: "Test-only supplied edit".into(),
        fix: Some(fix),
    }
}

#[cfg(unix)]
#[test]
fn explicit_replacement_preserves_bytes_permissions_and_refuses_failed_targets() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    use zincite_lint::{FileFixError, replace_fixed_file};
    let dir = std::env::temp_dir().join(format!("zincite-fix-write-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let source = "\u{feff}% π retained\r\nint: x=1;\r\n";
    let path = dir.join("values.dzn");
    // Use data syntax for the data file.
    let source = source.replace("int: x", "x");
    std::fs::write(&path, &source).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let snapshot = SourceSnapshot::new(&path, source.clone());
    let candidate = source.replace("=1", "=0x1");
    replace_fixed_file(&snapshot, &candidate).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), candidate);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(matches!(
        replace_fixed_file(&snapshot, &candidate),
        Err(FileFixError::StaleSource)
    ));
    let current = SourceSnapshot::new(&path, candidate.clone());
    assert!(matches!(
        replace_fixed_file(&current, "solve satisfy;"),
        Err(FileFixError::Candidate(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), candidate);
    std::fs::write(&path, candidate.replace('π', "changed")).unwrap();
    assert!(matches!(
        replace_fixed_file(&current, &source),
        Err(FileFixError::StaleSource)
    ));
    let alias = dir.join("alias.dzn");
    symlink(&path, &alias).unwrap();
    let alias_snapshot = SourceSnapshot::new(&alias, std::fs::read_to_string(&path).unwrap());
    assert!(matches!(
        replace_fixed_file(&alias_snapshot, &source),
        Err(FileFixError::NotRegular)
    ));
    let blocked = dir.join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    let blocked_path = blocked.join("values.dzn");
    std::fs::write(&blocked_path, &source).unwrap();
    let blocked_snapshot = SourceSnapshot::new(&blocked_path, source.clone());
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = replace_fixed_file(&blocked_snapshot, &candidate);
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        matches!(failed, Err(FileFixError::Io(_))),
        "expected a real sibling-write permission failure: {failed:?}"
    );
    assert_eq!(std::fs::read_to_string(&blocked_path).unwrap(), source);
    let independent = dir.join("independent.dzn");
    std::fs::write(&independent, &source).unwrap();
    replace_fixed_file(
        &SourceSnapshot::new(&independent, source.clone()),
        &candidate,
    )
    .unwrap();
    assert_eq!(std::fs::read_to_string(independent).unwrap(), candidate);
    assert!(std::fs::read_dir(&dir).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
    assert_eq!(std::fs::read_dir(&blocked).unwrap().count(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn supplied_one_pass_writes_all_roots_before_final_include_analysis() {
    use zincite_lint::{FixOptions, prepare_fixes, replace_fixed_file};
    let dir = std::env::temp_dir().join(format!("zincite-fix-pass-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let root = dir.join("root.mzn");
    let dependency = dir.join("dependency.mzn");
    std::fs::write(
        &root,
        "\u{feff}include \"dependency.mzn\";\r\nvar int:BadRoot=1; solve satisfy;\r\n",
    )
    .unwrap();
    std::fs::write(&dependency, "% keep\r\nint:BadDependency=1;\r\n").unwrap();
    let options = LintOptions::from_selection("naming").unwrap();
    let context = load_model(&root, &ModelOptions::default());
    assert_eq!(analyze_model(&context, &options).status(), 1);
    assert_eq!(
        context.files[context.root_file.unwrap()]
            .source_snapshot()
            .source(),
        std::fs::read_to_string(&root).unwrap()
    );
    drop(context);
    let mut planned = Vec::new();
    for (path, old, new) in [
        (&root, "BadRoot", "good_root"),
        (&dependency, "BadDependency", "good_dependency"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        let snapshot = SourceSnapshot::new(path, source.clone());
        let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
        let mut result = analyze_file(&parse(text), path, source.len() - text.len(), &options);
        assert_eq!(result.status(), 1);
        let start = source.find(old).unwrap();
        let mut group = fix(
            &snapshot,
            "Test-only naming edit",
            vec![edit(start..start + old.len(), new)],
        );
        group.safety = FixSafety::Unsafe {
            reason: "Renames public declarations and may change data binding or output names"
                .into(),
        };
        result.findings[0].fix = Some(group);
        planned.push((
            snapshot.clone(),
            prepare_fixes(
                &snapshot,
                &source,
                &result.findings,
                &FixOptions {
                    unsafe_fixes: true,
                    ..FixOptions::default()
                },
            )
            .unwrap(),
        ));
    }
    for (snapshot, prepared) in planned {
        replace_fixed_file(&snapshot, &prepared.candidate).unwrap();
    }
    let final_context = load_model(&root, &ModelOptions::default());
    assert_eq!(analyze_model(&final_context, &options).status(), 0);
    // A single supplied pass can leave warnings or create an actual directive
    // error. Syntax validation neither resolves diagnostics nor proves safety.
    let remaining = dir.join("remaining.mzn");
    let source = "int:BadName=1;\r\n";
    for (replacement, expected) in [(" ", 1), ("% zincite-lint: ignore unknown-rule\r\n", 2)] {
        std::fs::write(&remaining, source).unwrap();
        let snapshot = SourceSnapshot::new(&remaining, source);
        let mut result = analyze_file(&parse(source), &remaining, 0, &options);
        let mut group = fix(
            &snapshot,
            "Test-only prefix edit",
            vec![edit(0..0, replacement)],
        );
        group.safety = if expected == 1 {
            FixSafety::Safe
        } else {
            FixSafety::Unsafe {
                reason: "Introduces an invalid suppression directive".into(),
            }
        };
        if expected == 1 {
            group.applicability =
                "A leading space preserves tokens, values, comments and output".into();
        }
        result.findings[0].fix = Some(group);
        let eligibility = FixOptions {
            unsafe_fixes: true,
            ..FixOptions::default()
        };
        let prepared = prepare_fixes(&snapshot, source, &result.findings, &eligibility).unwrap();
        replace_fixed_file(&snapshot, &prepared.candidate).unwrap();
        let current = std::fs::read_to_string(&remaining).unwrap();
        let final_result = analyze_file(&parse(&current), &remaining, 0, &options);
        assert_eq!(final_result.status(), expected);
        assert!(
            final_result
                .findings
                .iter()
                .all(|finding| finding.fix.is_none())
        );
        assert_eq!(current, format!("{replacement}{source}"));
    }
    assert!(analyze_model(&final_context, &options).findings.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}
