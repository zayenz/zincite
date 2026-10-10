use std::ops::Range;
use zincite_lint::{
    EditPart, EditPlanError, FileFinding, Fix, FixSafety, FixSupport, LintOptions, ModelOptions,
    Rule, SourceLocation, SourceSnapshot, TextEdit, analyze_file, analyze_model, lint_with_options,
    load_model, prepare_edits, write_analysis,
};
use zincite_syntax::parse;

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

fn edit(range: Range<usize>, replacement: &str) -> TextEdit {
    TextEdit {
        range,
        replacement: vec![EditPart::Text(replacement.into())],
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
fn opaque_copy_edits_validate_boundaries_staleness_and_candidate_errors() {
    use zincite_lint::{
        FileFixError, FixOptions, prepare_fixes, replace_fixed_file, write_fix_diff,
    };
    let directory = std::env::temp_dir().join(format!("zincite-raw-edits-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("values.dzn");
    let source = b"\xef\xbb\xbf/* \xff \xc3\xa9 */\r\nvalue=(1 /* \xfe */);";
    std::fs::write(&path, source).unwrap();
    let snapshot = SourceSnapshot::new(&path, source);
    assert_eq!(snapshot.source_bytes(), source);
    let start = source.windows(2).position(|bytes| bytes == b"(1").unwrap();
    let end = source.len() - 1;
    let mut group = fix(
        &snapshot,
        "Retain the original operand",
        vec![TextEdit {
            range: start..end,
            replacement: vec![EditPart::Text(" ".into()), EditPart::Original(start..end)],
        }],
    );
    group.safety = FixSafety::Safe;
    group.applicability =
        "Only leading operand whitespace changes; the original operand and comments remain exact"
            .into();
    let findings = [supplied_finding(&snapshot, Rule::Naming, group.clone())];
    let prepared = prepare_fixes(&snapshot, source, &findings, &FixOptions::default()).unwrap();
    let mut expected = source[..start].to_vec();
    expected.push(b' ');
    expected.extend_from_slice(&source[start..]);
    assert_eq!(prepared.candidate, expected);
    let mut diff = Vec::new();
    write_fix_diff(&snapshot, &prepared.candidate, &mut diff).unwrap();
    let raw_line = b"-\xef\xbb\xbf/* \xff \xc3\xa9 */";
    assert!(diff.windows(raw_line.len()).any(|bytes| bytes == raw_line));
    assert_eq!(
        diff.windows(b"\\ No newline at end of file".len())
            .filter(|bytes| *bytes == b"\\ No newline at end of file")
            .count(),
        2
    );
    let mut changed = source.to_vec();
    changed[6] = 0xfd;
    assert_eq!(
        prepare_edits(&snapshot, &changed, std::slice::from_ref(&group)),
        Err(EditPlanError::StaleSource)
    );
    for copy in [0..1, end..start] {
        let mut invalid = group.clone();
        invalid.edits[0].replacement = vec![EditPart::Original(copy)];
        assert!(matches!(
            prepare_edits(&snapshot, source, &[invalid]),
            Err(EditPlanError::InvalidEdit { .. })
        ));
    }
    let mut split_scalar = group;
    split_scalar.edits[0] = TextEdit {
        range: 3..source.len(),
        replacement: vec![EditPart::Original(9..10)],
    };
    assert!(matches!(
        prepare_edits(&snapshot, source, &[split_scalar]),
        Err(EditPlanError::InvalidEdit { .. })
    ));
    for invalid in [
        b"\xef\xbb\xbf/* \xff */\r\nvalue=\xfe;".as_slice(),
        b"\xef\xbb\xbf/* \xff */\r\nvalue=\"\xfe\";".as_slice(),
        b"\xef\xbb\xbf/* \xff */\r\n% zincite-lint: ignore naming\xfe\r\nvalue=1;".as_slice(),
    ] {
        assert!(matches!(
            replace_fixed_file(&snapshot, invalid),
            Err(FileFixError::Candidate(_))
        ));
        assert_eq!(std::fs::read(&path).unwrap(), source);
    }
    replace_fixed_file(&snapshot, &prepared.candidate).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), expected);
    std::fs::write(&path, &changed).unwrap();
    assert!(matches!(
        replace_fixed_file(&snapshot, &prepared.candidate),
        Err(FileFixError::StaleSource)
    ));
    assert_eq!(std::fs::read(&path).unwrap(), changed);
    std::fs::remove_dir_all(directory).unwrap();
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
    let prepared = prepare_edits(
        &snapshot,
        source.as_bytes(),
        &[objective.clone(), literals.clone()],
    )
    .unwrap();
    assert!(prepared.conflicts.is_empty());
    assert_eq!(
        text(&prepared.candidate),
        source
            .replace("=1", "=0x1")
            .replace("satisfy", "minimize x")
    );
    // The existing parser boundary removes a BOM; candidate retains the full file.
    let parsed = parse(text(&prepared.candidate).strip_prefix('\u{feff}').unwrap());
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    assert_eq!(snapshot.source_bytes(), source.as_bytes());
    assert_eq!(snapshot.path().to_str(), Some("model.mzn"));
    assert_eq!(literals.safety, FixSafety::Safe);
    assert!(
        matches!(objective.safety, FixSafety::Unsafe { reason } if reason.contains("objective"))
    );
    assert!(text(&prepared.candidate).contains("% π comment\r\n"));
    assert!(text(&prepared.candidate).contains("% keep\r\nconstraint :: \"kept label\""));
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
            source.replace('π', "x").as_bytes(),
            std::slice::from_ref(&valid)
        ),
        Err(EditPlanError::StaleSource)
    );
    let mut other = valid.clone();
    other.snapshot = SourceSnapshot::new("other.mzn", source);
    assert_eq!(
        prepare_edits(&snapshot, source.as_bytes(), &[other]),
        Err(EditPlanError::SourceMismatch { group: 0 })
    );
    let mut changed = valid.clone();
    changed.snapshot = SourceSnapshot::new("model.mzn", source.replace("satisfy", "minimize x"));
    assert_eq!(
        prepare_edits(&snapshot, source.as_bytes(), &[changed]),
        Err(EditPlanError::SourceMismatch { group: 0 })
    );
    let pi = source.find('π').unwrap();
    for range in [pi + 1..pi + 2, source.len()..source.len() + 1, one + 1..one] {
        let mut group = valid.clone();
        group.edits.push(edit(range, "x"));
        assert!(matches!(
            prepare_edits(&snapshot, source.as_bytes(), &[group]),
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
        prepare_edits(
            &snapshot,
            source.as_bytes(),
            std::slice::from_ref(&internal_overlap)
        ),
        Err(EditPlanError::OverlappingEdits { group: 0, .. })
    ));
    use zincite_lint::prepare_text_edits;
    assert_eq!(
        prepare_text_edits(&snapshot, b"stale", &valid.edits),
        Err(EditPlanError::StaleSource)
    );
    assert!(matches!(
        prepare_text_edits(&snapshot, source.as_bytes(), &internal_overlap.edits),
        Err(EditPlanError::OverlappingEdits { .. })
    ));
    assert_eq!(
        prepare_text_edits(&snapshot, source.as_bytes(), &[]).unwrap(),
        source.as_bytes()
    );
    assert_eq!(
        prepare_text_edits(&snapshot, source.as_bytes(), &valid.edits).unwrap(),
        source.replace("=1", "=0x1").as_bytes()
    );
    let empty = fix(&snapshot, "No edits", vec![]);
    assert!(matches!(
        prepare_edits(&snapshot, source.as_bytes(), &[empty]),
        Err(EditPlanError::InvalidGroup { .. })
    ));
    let mut undescribed = valid;
    undescribed.safety = FixSafety::Unsafe { reason: " ".into() };
    assert!(matches!(
        prepare_edits(&snapshot, source.as_bytes(), &[undescribed]),
        Err(EditPlanError::InvalidGroup { .. })
    ));
    assert_eq!(snapshot.source_bytes(), source.as_bytes());
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
        let result = prepare_edits(&snapshot, source.as_bytes(), &groups).unwrap();
        assert_eq!(
            text(&result.candidate),
            format!("{source}% independent\r\n")
        );
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
    let result = prepare_edits(
        &snapshot,
        source.as_bytes(),
        &[replacement, start, end, independent],
    )
    .unwrap();
    assert_eq!(result.conflicts.len(), 3);
    assert_eq!(
        text(&result.candidate),
        format!("{source}% independent\r\n")
    );
    let insertion = fix(&snapshot, "First insertion", vec![edit(x..x, "_")]);
    let same = fix(&snapshot, "Coincident insertion", vec![edit(x..x, "a")]);
    let result = prepare_edits(&snapshot, source.as_bytes(), &[same, insertion]).unwrap();
    assert_eq!(result.conflicts.len(), 2);
    assert_eq!(text(&result.candidate), source);
    let adjacent = SourceSnapshot::new("adjacent.mzn", "var  int: x=1; solve satisfy;");
    let left = fix(&adjacent, "First space", vec![edit(3..4, "\t")]);
    let right = fix(&adjacent, "Second space", vec![edit(4..5, " ")]);
    let result = prepare_edits(&adjacent, adjacent.source_bytes(), &[right, left]).unwrap();
    assert!(result.conflicts.is_empty());
    assert_eq!(text(&result.candidate), "var\t int: x=1; solve satisfy;");
    assert!(parse(text(&result.candidate)).diagnostics().is_empty());
    assert_eq!(snapshot.source_bytes(), source.as_bytes());
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
    let prepared = prepare_edits(&snapshot, source.as_bytes(), &groups).unwrap();
    assert_eq!(
        text(&prepared.candidate),
        source.replace("BadName=", "BadName =")
    );
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
        prepare_edits(&snapshot, source.as_bytes(), &empty)
            .unwrap()
            .candidate,
        source.as_bytes()
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
    assert!(Rule::all().all(|r| r.metadata().fix_support
        == if matches!(r, Rule::ElementPredicate | Rule::UnusedGeneratorBinding) {
            FixSupport::Sometimes
        } else {
            FixSupport::None
        }));
    assert_eq!(Rule::DEFAULT.len(), 2);
    assert_eq!(Rule::THESIS.len(), 14);
    assert_eq!(snapshot.source_bytes(), source.as_bytes());
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
    let prepared = prepare_fixes(
        &snapshot,
        source.as_bytes(),
        &findings,
        &FixOptions::default(),
    )
    .unwrap();
    assert_eq!(text(&prepared.candidate), source.replace("=1", "=0x1"));
    let mut preview = Vec::new();
    write_fix_diff(&snapshot, &prepared.candidate, &mut preview).unwrap();
    let preview = String::from_utf8(preview).unwrap();
    assert!(preview.starts_with("--- a/model.mzn\n+++ b/model.mzn\n@@ -1,3 +1,3 @@\n"));
    assert!(preview.contains("-% π\r\n") || preview.contains("-\u{feff}% π\r\n"));
    assert_eq!(preview.matches("\\ No newline at end of file").count(), 2);
    assert_eq!(snapshot.source_bytes(), source.as_bytes());
    let options = FixOptions {
        unsafe_fixes: true,
        ..FixOptions::default()
    };
    assert!(
        text(
            &prepare_fixes(&snapshot, source.as_bytes(), &findings, &options)
                .unwrap()
                .candidate
        )
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
            source.as_bytes(),
            &findings,
            &settings.resolve_fixes().unwrap()
        )
        .unwrap()
        .candidate,
        source.as_bytes()
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
    let prepared = prepare_fixes(&snapshot, source.as_bytes(), &mixed, &options).unwrap();
    assert_eq!(text(&prepared.candidate), source);
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
        prepare_fixes(&snapshot, source.as_bytes(), &[invalid], &options),
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
                data.source_bytes().len()..data.source_bytes().len(),
                "solve satisfy;",
            )],
        ),
    );
    assert!(
        matches!(prepare_fixes(&data, data.source_bytes(), &[invalid], &options), Err(FixPreparationError::Syntax(errors)) if errors.iter().all(|e| e.location.range.start >= 3))
    );
    let mut empty = Vec::new();
    write_fix_diff(&snapshot, source.as_bytes(), &mut empty).unwrap();
    assert!(empty.is_empty());
}

fn supplied_finding(snapshot: &SourceSnapshot, rule: Rule, fix: Fix) -> FileFinding {
    FileFinding {
        location: SourceLocation::from_bytes(
            snapshot.path().to_path_buf(),
            snapshot.source_bytes(),
            0..0,
            0,
        ),
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
    replace_fixed_file(&snapshot, candidate.as_bytes()).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), candidate);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(matches!(
        replace_fixed_file(&snapshot, candidate.as_bytes()),
        Err(FileFixError::StaleSource)
    ));
    let current = SourceSnapshot::new(&path, candidate.clone());
    assert!(matches!(
        replace_fixed_file(&current, b"solve satisfy;"),
        Err(FileFixError::Candidate(_))
    ));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), candidate);
    std::fs::write(&path, candidate.replace('π', "changed")).unwrap();
    assert!(matches!(
        replace_fixed_file(&current, source.as_bytes()),
        Err(FileFixError::StaleSource)
    ));
    let alias = dir.join("alias.dzn");
    symlink(&path, &alias).unwrap();
    let alias_snapshot = SourceSnapshot::new(&alias, std::fs::read_to_string(&path).unwrap());
    assert!(matches!(
        replace_fixed_file(&alias_snapshot, source.as_bytes()),
        Err(FileFixError::NotRegular)
    ));
    let blocked = dir.join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    let blocked_path = blocked.join("values.dzn");
    std::fs::write(&blocked_path, &source).unwrap();
    let blocked_snapshot = SourceSnapshot::new(&blocked_path, source.clone());
    std::fs::set_permissions(&blocked, std::fs::Permissions::from_mode(0o555)).unwrap();
    let failed = replace_fixed_file(&blocked_snapshot, candidate.as_bytes());
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
        candidate.as_bytes(),
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
            .source_bytes(),
        std::fs::read(&root).unwrap()
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
                source.as_bytes(),
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
    // A single supplied pass can leave warnings; malformed directives must
    // reject the candidate before replacement.
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
        let prepared = prepare_fixes(&snapshot, source.as_bytes(), &result.findings, &eligibility);
        if expected == 2 {
            assert!(matches!(
                prepared,
                Err(zincite_lint::FixPreparationError::Syntax(_))
            ));
            assert_eq!(std::fs::read(&remaining).unwrap(), source.as_bytes());
            continue;
        }
        let prepared = prepared.unwrap();
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
