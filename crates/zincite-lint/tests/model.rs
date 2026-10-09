use std::path::{Path, PathBuf};
use zincite_lint::{
    LintOptions, ModelOptions, RuleOutcome, SourceKind, analyze_model, load_model, write_analysis,
};

fn temporary(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("zincite-model-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn generic_standard_prelude_loads_helpers_and_reports_failures() {
    let directory = temporary("generic-prelude");
    let library = directory.join("library");
    let core = library.join("std/stdlib.mzn");
    let prelude = library.join("std/solver_redefinitions.mzn");
    let helper = library.join("std/fzn_array_set_union.mzn");
    let root = directory.join("root.mzn");
    let prototype =
        "predicate fzn_array_set_union(array [int] of var set of int: x, var set of int: z);";
    let helper_body = "predicate fzn_array_set_union(array [int] of var set of int: x, var set of int: z) = same_sets(x, z);";
    write(
        &core,
        format!(
            "function set of int: '..'(int: left, int: right); {prototype} predicate same_sets(array [int] of var set of int: x, var set of int: z);"
        ),
    );
    write(
        &root,
        "array [1..1] of var set of int: groups; var set of int: combined; constraint fzn_array_set_union(groups, combined); solve satisfy; output [];",
    );
    let options = ModelOptions {
        stdlib_dir: Some(library.clone()),
        ..ModelOptions::default()
    };
    let minimal = load_model(&root, &options);
    assert!(minimal.errors.is_empty(), "{:?}", minimal.errors);
    assert_eq!(minimal.files.len(), 2);

    write(&prelude, "include \"fzn_array_set_union.mzn\";");
    write(&helper, helper_body);
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let core_file = context.implicit_core.unwrap();
    assert_eq!(
        context.files[core_file].canonical_path,
        core.canonicalize().unwrap()
    );
    assert!(context.files[core_file].implicit);
    for path in [&prelude, &helper] {
        let file = context
            .files
            .iter()
            .find(|file| file.canonical_path == path.canonicalize().unwrap())
            .expect("generic prelude and real helper body are loaded");
        assert_eq!(file.kind, SourceKind::StandardLibrary);
        assert!(!file.implicit && !file.explicit && !file.warnings_enabled());
    }
    let helper_file = context
        .files
        .iter()
        .position(|file| file.canonical_path == helper.canonicalize().unwrap())
        .unwrap();
    let bindings = zincite_lint::resolve_bindings(&context);
    let calls = zincite_lint::resolve_callables(&context, &bindings);
    let call = calls
        .calls
        .iter()
        .find(|call| call.file == context.root_file.unwrap() && call.name == "fzn_array_set_union")
        .unwrap();
    let zincite_lint::CallOutcome::Resolved { declaration, .. } = &call.outcome else {
        panic!(
            "unique real implementation must win over its prototype: {:?}",
            call.outcome
        );
    };
    assert_eq!(bindings.declarations[declaration.0].file, helper_file);
    let selected = LintOptions::from_selection("unused-declaration").unwrap();
    let analysis = analyze_model(&context, &selected);
    assert!(
        analysis.errors.is_empty() && analysis.limitations.is_empty(),
        "{analysis:?}"
    );
    assert!(matches!(analysis.rules[0].outcome, RuleOutcome::Completed));

    write(
        &helper,
        "predicate fzn_array_set_union(array [int] of var set of int: x, var set of int: z) = missing_helper(x, z);",
    );
    let unsupported = load_model(&root, &options);
    assert!(unsupported.errors.is_empty(), "{:?}", unsupported.errors);
    let analysis = analyze_model(&unsupported, &selected);
    assert!(matches!(
        analysis.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert!(
        analysis
            .limitations
            .iter()
            .any(|diagnostic| diagnostic.location.path == helper
                && diagnostic.message.contains("missing_helper")),
        "{analysis:?}"
    );

    std::fs::remove_file(&prelude).unwrap();
    std::fs::create_dir(&prelude).unwrap();
    let unreadable = load_model(&root, &options);
    assert!(
        unreadable
            .errors
            .iter()
            .any(|diagnostic| diagnostic.location.path == prelude
                && diagnostic.message.contains("cannot read")),
        "existing non-readable prelude must report an error: {:?}",
        unreadable.errors
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn write(path: impl AsRef<Path>, source: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
}

#[test]
fn opaque_model_comments_supply_exact_fix_snapshots_and_original_locations() {
    let directory = temporary("raw-fix");
    let path = directory.join("root.mzn");
    write(directory.join("library/std/stdlib.mzn"), "annotation core;");
    let options = ModelOptions {
        stdlib_dir: Some(directory.join("library")),
        ..ModelOptions::default()
    };
    let source = b"\xef\xbb\xbf/* \xff \xc3\xa9 */\r\narray[int] of int: values=[2 | unused in {1,2}];\r\nsolve satisfy;\r\n";
    write(&path, source);
    let context = load_model(&path, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let root = &context.files[context.root_file.unwrap()];
    assert_eq!(root.source_bytes(), &source[3..]);
    assert_eq!(root.source_snapshot().source_bytes(), source);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unused-generator-binding").unwrap(),
    );
    assert!(result.errors.is_empty(), "{result:?}");
    let finding = result
        .findings
        .iter()
        .find(|finding| finding.fix.is_some())
        .expect("unused comprehension supplies a fix");
    let start = source
        .windows(b"unused".len())
        .position(|bytes| bytes == b"unused")
        .unwrap();
    assert_eq!(finding.location.range, start..start + b"unused".len());
    assert_eq!(finding.location.line, 2);
    assert_eq!(
        finding.fix.as_ref().unwrap().snapshot.source_bytes(),
        source
    );
    for file in &context.files {
        if file.path != path {
            assert!(file.original_bytes.is_none());
        }
    }
    write(&path, b"\xef\xbb\xbf/* \xff \xc3\xa9 */ value=\xfe;");
    let rejected = load_model(&path, &options);
    let error = rejected
        .errors
        .iter()
        .find(|error| error.message.contains("invalid UTF-8"))
        .unwrap();
    assert_eq!(error.location.range, 20..21);
    assert_eq!((error.location.line, error.location.column), (1, 17));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn include_graph_preserves_search_order_identity_locations_and_independent_roots() {
    let directory = temporary("graph");
    let root = directory.join("root.mzn");
    let source = concat!(
        "include \"choice.mzn\"; include \"ordered.mzn\"; include \"standard.mzn\";\n",
        "include \"left.mzn\"; include \"right.mzn\"; include \"sub/../leaf.mzn\";\n",
        "include \"cycle.mzn\"; include \"missing.mzn\"; include \"unreadable.mzn\"; solve satisfy;",
    );
    write(&root, source);
    write(directory.join("choice.mzn"), "int: relative = 1;");
    for (name, value) in [("first", 2), ("second", 3), ("library/std", 4)] {
        write(
            directory.join(name).join("choice.mzn"),
            format!("int: other = {value};"),
        );
        write(
            directory.join(name).join("ordered.mzn"),
            format!("int: selected = {value};"),
        );
    }
    write(
        directory.join("library/std/standard.mzn"),
        "int: standard = 4;",
    );
    write(directory.join("library/std/stdlib.mzn"), "annotation core;");
    write(directory.join("left.mzn"), "include \"leaf.mzn\";");
    write(directory.join("right.mzn"), "include \"leaf.mzn\";");
    write(directory.join("leaf.mzn"), "int: shared = 1;");
    write(directory.join("cycle.mzn"), "include \"root.mzn\";");
    std::fs::create_dir_all(directory.join("sub")).unwrap();
    std::fs::create_dir_all(directory.join("unreadable.mzn")).unwrap();
    let options = ModelOptions {
        include_dirs: vec![directory.join("first"), directory.join("second")],
        stdlib_dir: Some(directory.join("library")),
    };
    let context = load_model(&root, &options);
    assert_eq!(context.errors.len(), 3, "{:?}", context.errors);
    assert!(context.limitations.is_empty());
    assert!(
        context
            .files
            .iter()
            .any(|file| file.path == directory.join("choice.mzn"))
    );
    assert!(
        context
            .files
            .iter()
            .any(|file| file.path == directory.join("first/ordered.mzn"))
    );
    assert!(
        !context
            .files
            .iter()
            .any(|file| file.path.starts_with(directory.join("second")))
    );
    assert_eq!(
        context
            .files
            .iter()
            .filter(|file| file.path.ends_with("leaf.mzn"))
            .count(),
        1
    );
    let root_id = context.root_file.unwrap();
    let edges: Vec<_> = context
        .includes
        .iter()
        .filter(|edge| edge.from == root_id)
        .collect();
    assert_eq!(edges.len(), 9);
    for (edge, name) in edges.iter().zip([
        "choice.mzn",
        "ordered.mzn",
        "standard.mzn",
        "left.mzn",
        "right.mzn",
        "sub/../leaf.mzn",
        "cycle.mzn",
        "missing.mzn",
        "unreadable.mzn",
    ]) {
        assert_eq!(&source[edge.location.range.clone()], format!("\"{name}\""));
    }
    assert_eq!(context.errors[0].location.path, directory.join("cycle.mzn"));
    assert!(context.errors[0].message.contains("cycle"));
    for error in &context.errors[1..] {
        assert_eq!(error.location.path, root);
        assert!(source[error.location.range.clone()].contains(
            if error.message.contains("resolve") {
                "missing"
            } else {
                "unreadable"
            }
        ));
    }
    write(directory.join("independent.mzn"), "int: good_name = 1;");
    let independent = load_model(directory.join("independent.mzn"), &options);
    assert!(independent.errors.is_empty());
    assert!(!independent.files.iter().any(|file| file.path == root));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn core_origin_suppressions_and_dependency_errors_remain_file_local() {
    let directory = temporary("origins");
    let library = directory.join("library");
    write(library.join("std/stdlib.mzn"), "include \"core.mzn\";");
    write(
        library.join("std/core.mzn"),
        "include \"nested.mzn\"; predicate BadCore();",
    );
    write(
        library.join("std/nested.mzn"),
        "predicate forall(array [int] of var bool: expressions);",
    );
    write(library.join("std/global.mzn"), "predicate BadGlobal();");
    write(
        directory.join("user.mzn"),
        "% zincite-lint: ignore naming\nint: BadSuppressed = 1; int: BadIncluded = 2; predicate user_builtin_like();",
    );
    let root = directory.join("root.mzn");
    write(
        &root,
        "% zincite-lint: ignore naming\ninclude \"core.mzn\"; include \"global.mzn\"; include \"user.mzn\"; solve satisfy;",
    );
    let options = ModelOptions {
        stdlib_dir: Some(library.clone()),
        ..ModelOptions::default()
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    for name in ["stdlib.mzn", "core.mzn", "nested.mzn"] {
        let file = context
            .files
            .iter()
            .find(|file| file.path.ends_with(name))
            .unwrap();
        assert_eq!(file.kind, SourceKind::StandardLibrary);
        assert!(file.implicit && !file.explicit && !file.warnings_enabled());
    }
    let global = context
        .files
        .iter()
        .find(|file| file.path.ends_with("global.mzn"))
        .unwrap();
    assert_eq!(global.kind, SourceKind::StandardLibrary);
    assert!(!global.implicit);
    let user = context
        .files
        .iter()
        .find(|file| file.path.ends_with("user.mzn"))
        .unwrap();
    assert_eq!(user.kind, SourceKind::User);
    assert!(!user.implicit && user.warnings_enabled());
    let result = analyze_model(&context, &LintOptions::default());
    assert_eq!(result.findings.len(), 1, "{:?}", result.findings);
    assert_eq!(result.findings[0].location.path, user.path);
    assert_eq!(
        &user.parsed.source()[result.findings[0].location.range.clone()],
        "BadIncluded"
    );
    let explicit = load_model(library.join("std/global.mzn"), &options);
    let selected = &explicit.files[explicit.root_file.unwrap()];
    assert_eq!(selected.kind, SourceKind::StandardLibrary);
    assert!(selected.explicit && selected.warnings_enabled());
    assert_eq!(
        analyze_model(&explicit, &LintOptions::default())
            .findings
            .len(),
        1
    );

    write(
        directory.join("broken.mzn"),
        "\u{feff}% é\r\nint: broken = ;",
    );
    write(
        directory.join("invalid.mzn"),
        b"\xef\xbb\xbf% \xc3\xa9\n\xff",
    );
    write(
        directory.join("suppression.mzn"),
        "% zincite-lint: ignore unknown\nint: BadName = 1;",
    );
    write(
        &root,
        "include \"broken.mzn\"; include \"invalid.mzn\"; include \"suppression.mzn\";",
    );
    let broken = load_model(&root, &options);
    assert_eq!(broken.errors.len(), 3, "{:?}", broken.errors);
    let syntax = &broken.errors[0];
    assert_eq!(syntax.location.path, directory.join("broken.mzn"));
    assert_eq!(syntax.location.line, 2);
    assert!(syntax.location.range.start >= 3);
    let encoding = &broken.errors[1];
    assert_eq!(encoding.location.path, directory.join("invalid.mzn"));
    assert_eq!(encoding.location.range, 8..9);
    assert_eq!((encoding.location.line, encoding.location.column), (2, 1));
    assert_eq!(
        encoding.message,
        "invalid UTF-8 is only supported inside comments"
    );
    assert_eq!(
        broken.errors[2].location.path,
        directory.join("suppression.mzn")
    );
    assert!(broken.errors[2].message.contains("suppression"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn incomplete_context_and_shared_reporter_keep_rule_execution_and_status_distinct() {
    let directory = temporary("report");
    let library = directory.join("library");
    write(library.join("std/stdlib.mzn"), "annotation core;");
    let root = directory.join("root.mzn");
    write(&root, "include \"dynamic \\(1).mzn\"; int: good_name = 1;");
    let options = ModelOptions {
        stdlib_dir: Some(library),
        ..ModelOptions::default()
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty());
    assert_eq!(context.limitations.len(), 1);
    let defaults = LintOptions::default();
    let result = analyze_model(&context, &defaults);
    assert!(result.findings.is_empty() && result.errors.is_empty());
    assert!(
        result
            .rules
            .iter()
            .all(|rule| rule.outcome == RuleOutcome::Completed)
    );
    let mut stderr = Vec::new();
    assert_eq!(write_analysis(&result, &mut stderr).unwrap(), 0);
    let message = String::from_utf8(stderr).unwrap();
    assert!(message.contains("analysis limitation") && message.contains("requires evaluation"));

    write(&root, "include \"dynamic \\(1).mzn\"; int: BadName = 1;");
    let warning = analyze_model(&load_model(&root, &options), &defaults);
    let mut stderr = Vec::new();
    assert_eq!(write_analysis(&warning, &mut stderr).unwrap(), 1);
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("warning [naming]")
    );
    let missing = directory.join("other.mzn");
    write(&missing, "include \"missing.mzn\";");
    let error = analyze_model(&load_model(&missing, &options), &defaults);
    let mut stderr = Vec::new();
    assert_eq!(
        write_analysis(&error, &mut stderr)
            .unwrap()
            .max(warning.status()),
        2
    );
    assert!(
        String::from_utf8(stderr)
            .unwrap()
            .contains("error: cannot resolve include")
    );
    for selection in ["thesis", "default,thesis", "naming,search-coverage"] {
        let analyzed = analyze_model(&context, &LintOptions::from_selection(selection).unwrap());
        assert_eq!(analyzed.status(), 0);
        assert!(
            analyzed
                .rules
                .iter()
                .any(|rule| matches!(rule.outcome, RuleOutcome::Limited { .. }))
        );
        assert!(analyzed.errors.is_empty());
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn standard_reentries_reuse_canonical_files_edges_and_core_availability() {
    let directory = temporary("standard-reentries");
    let library = directory.join("library");
    let root = directory.join("root.mzn");
    let source =
        "\u{feff}% é\r\ninclude \"first.mzn\"; include \"sub/../first.mzn\"; solve satisfy;\r\n";
    write(&root, source);
    write(library.join("std/stdlib.mzn"), "include \"first.mzn\";");
    write(library.join("std/first.mzn"), "include \"second.mzn\";");
    write(
        library.join("std/second.mzn"),
        "include \"sub/../first.mzn\";",
    );
    std::fs::create_dir_all(library.join("std/sub")).unwrap();
    let options = ModelOptions {
        stdlib_dir: Some(library.clone()),
        ..ModelOptions::default()
    };
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    assert!(context.limitations.is_empty());
    assert_eq!(context.files.len(), 4);
    let canonical: std::collections::BTreeSet<_> =
        context.files.iter().map(|f| &f.canonical_path).collect();
    assert_eq!(canonical.len(), context.files.len());
    let first = context
        .files
        .iter()
        .position(|f| f.path == library.join("std/first.mzn"))
        .unwrap();
    let second = context
        .files
        .iter()
        .position(|f| f.path == library.join("std/second.mzn"))
        .unwrap();
    let root_id = context.root_file.unwrap();
    let root_edges: Vec<_> = context
        .includes
        .iter()
        .filter(|e| e.from == root_id)
        .collect();
    assert_eq!(root_edges.len(), 2);
    for (edge, spelling) in root_edges
        .iter()
        .zip(["\"first.mzn\"", "\"sub/../first.mzn\""])
    {
        assert_eq!(edge.target, Some(first));
        assert_eq!(&source[edge.location.range.clone()], spelling);
        assert_eq!(
            (edge.location.line, edge.location.path.as_path()),
            (2, root.as_path())
        );
    }
    assert_eq!(context.includes.len(), 5);
    for edge in &context.includes {
        assert!(edge.target.is_some());
        let original = std::fs::read_to_string(&context.files[edge.from].path).unwrap();
        assert!(original[edge.location.range.clone()].starts_with('"'));
    }
    assert!(
        context
            .includes
            .iter()
            .any(|e| e.from == first && e.target == Some(second))
    );
    let back = context
        .includes
        .iter()
        .find(|e| e.from == second && e.target == Some(first))
        .unwrap();
    assert_eq!(
        &context.files[second].parsed.source()[back.location.range.clone()],
        "\"sub/../first.mzn\""
    );
    for (id, file) in context.files.iter().enumerate() {
        if id == root_id {
            assert_eq!(file.kind, SourceKind::User);
            assert!(file.explicit && !file.implicit);
        } else {
            assert_eq!(file.kind, SourceKind::StandardLibrary);
            assert!(file.implicit && !file.explicit && !file.warnings_enabled());
        }
    }
    let explicit = load_model(library.join("std/first.mzn"), &options);
    assert!(explicit.errors.is_empty(), "{:?}", explicit.errors);
    let selected = &explicit.files[explicit.root_file.unwrap()];
    assert_eq!(selected.kind, SourceKind::StandardLibrary);
    assert!(selected.explicit && selected.implicit && selected.warnings_enabled());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn self_user_and_mixed_cycles_keep_located_errors_and_target_edges() {
    for case in [
        "self-user",
        "self-standard",
        "user",
        "mixed-standard-target",
        "mixed-user-target",
    ] {
        let directory = temporary(case);
        let library = directory.join("library");
        let root = directory.join("root.mzn");
        write(library.join("std/stdlib.mzn"), "annotation core;");
        let (entry, error_path, target_path) = match case {
            "self-user" => {
                write(&root, "include \"root.mzn\";");
                (root.clone(), root.clone(), root.clone())
            }
            "self-standard" => {
                let path = library.join("std/first.mzn");
                write(&path, "include \"sub/../first.mzn\";");
                std::fs::create_dir_all(library.join("std/sub")).unwrap();
                (path.clone(), path.clone(), path)
            }
            "user" => {
                write(&root, "include \"bridge.mzn\";");
                write(directory.join("bridge.mzn"), "include \"root.mzn\";");
                (root.clone(), directory.join("bridge.mzn"), root.clone())
            }
            "mixed-standard-target" => {
                write(&root, "include \"first.mzn\";");
                write(
                    library.join("std/first.mzn"),
                    "include \"../../bridge.mzn\";",
                );
                write(directory.join("bridge.mzn"), "include \"first.mzn\";");
                (
                    root.clone(),
                    library.join("std/../../bridge.mzn"),
                    library.join("std/first.mzn"),
                )
            }
            _ => {
                write(&root, "include \"first.mzn\";");
                write(library.join("std/first.mzn"), "include \"../../root.mzn\";");
                (root.clone(), library.join("std/first.mzn"), root.clone())
            }
        };
        let context = load_model(
            entry,
            &ModelOptions {
                stdlib_dir: Some(library),
                ..ModelOptions::default()
            },
        );
        assert_eq!(context.errors.len(), 1, "{case}: {:?}", context.errors);
        let error = &context.errors[0];
        assert!(error.message.contains("include cycle"));
        assert_eq!(error.location.path, error_path);
        let edge = context
            .includes
            .iter()
            .find(|e| e.location == error.location)
            .unwrap();
        assert_eq!(
            context.files[edge.target.unwrap()].canonical_path,
            target_path.canonicalize().unwrap()
        );
        let original = std::fs::read_to_string(error_path).unwrap();
        assert!(original[error.location.range.clone()].starts_with('"'));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
