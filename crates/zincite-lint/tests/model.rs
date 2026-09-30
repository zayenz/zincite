use std::path::{Path, PathBuf};
use zincite_lint::{
    LintOptions, ModelOptions, RuleOutcome, SourceKind, analyze_model, load_model, write_analysis,
};

fn temporary(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("zincite-model-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write(path: impl AsRef<Path>, source: impl AsRef<[u8]>) {
    let path = path.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, source).unwrap();
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
    assert_eq!(encoding.location.range, 8..8);
    assert_eq!((encoding.location.line, encoding.location.column), (2, 1));
    assert!(encoding.message.contains("UTF-8 at byte 8"));
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
    for selection in ["thesis", "all", "naming,effective-zero-one"] {
        let unavailable = analyze_model(&context, &LintOptions::from_selection(selection).unwrap());
        assert_eq!(unavailable.status(), 2);
        assert!(unavailable.findings.is_empty());
        assert!(
            unavailable
                .rules
                .iter()
                .all(|rule| matches!(rule.outcome, RuleOutcome::NotRun { .. }))
        );
        assert!(unavailable.errors[0].message.contains("unavailable rules"));
    }
    std::fs::remove_dir_all(directory).unwrap();
}
