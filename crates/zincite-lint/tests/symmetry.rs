use std::path::PathBuf;
use zincite_lint::{
    CallOutcome, DeclarationRole, Instantiation, LintOptions, ModelOptions, Rule, RuleOutcome,
    SourceKind, SymmetryUseOutcome, TypeInst, TypeKind, analyze_model, load_model,
    resolve_bindings, resolve_callables, resolve_symmetry_uses,
};

const FAMILY: [&str; 11] = [
    "lex2",
    "lex_greater",
    "lex_greatereq",
    "lex_less",
    "lex_lesseq",
    "strict_lex2",
    "seq_precede_chain",
    "value_precede",
    "value_precede_chain",
    "increasing",
    "decreasing",
];

fn setup(name: &str, core: &str) -> (PathBuf, ModelOptions) {
    let directory =
        std::env::temp_dir().join(format!("zincite-symmetry-{name}-{}", std::process::id()));
    let library = directory.join("library");
    std::fs::create_dir_all(library.join("std")).unwrap();
    std::fs::write(library.join("std/stdlib.mzn"), core).unwrap();
    let declarations = FAMILY
        .iter()
        .map(|name| format!("predicate {name}(var int: x)=true;\n"))
        .collect::<String>();
    std::fs::write(library.join("std/family.mzn"), declarations).unwrap();
    (
        directory,
        ModelOptions {
            include_dirs: Vec::new(),
            stdlib_dir: Some(library),
        },
    )
}

#[test]
fn complete_family_markers_user_overloads_suppression_ranges_and_advisory_cli() {
    for redefined in [false, true] {
        check_family_markers(redefined);
    }
}

fn check_family_markers(redefined: bool) {
    let (directory, options) = setup(
        if redefined {
            "family-redefined"
        } else {
            "family"
        },
        if redefined {
            "predicate symmetry_breaking_constraint(var bool: b);"
        } else {
            "predicate symmetry_breaking_constraint(var bool: b)=b;"
        },
    );
    let wrapper_path = options.stdlib_dir.as_ref().unwrap().join(if redefined {
        "std/solver_redefinitions.mzn"
    } else {
        "std/stdlib.mzn"
    });
    if redefined {
        std::fs::write(
            &wrapper_path,
            "predicate symmetry_breaking_constraint(var bool: b)=b;",
        )
        .unwrap();
    }
    let mut source = "\u{feff}% é\r\ninclude \"family.mzn\"; var int: x;\n".to_owned();
    for name in FAMILY {
        source.push_str(&format!(
            "constraint {name}(x);\nconstraint symmetry_breaking_constraint({name}(x));\n"
        ));
    }
    source.push_str(concat!(
        "constraint symmetry_breaking_constraint(symmetry_breaking_constraint(increasing(x)));\n",
        "predicate increasing(int: x)=true; constraint increasing(1);\n",
        "predicate symmetry_breaking_constraint(var int: x)=true;\n",
        "constraint symmetry_breaking_constraint(let { constraint increasing(x); } in 1);\n",
        "constraint symmetry_breaking_constraint(symmetry_breaking_constraint(let { constraint increasing(x); } in 1));\n",
        "% zincite-lint: ignore unmarked-symmetry-breaking\nconstraint increasing(x);\nsolve satisfy;\n"
    ));
    let root = directory.join("root.mzn");
    std::fs::write(&root, &source).unwrap();
    let context = load_model(&root, &options);
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    let bindings = resolve_bindings(&context);
    let calls = resolve_callables(&context, &bindings);
    let wrapper = calls
        .calls
        .iter()
        .find(|call| {
            Some(call.file) == context.root_file && call.name == "symmetry_breaking_constraint"
        })
        .unwrap();
    let CallOutcome::Resolved {
        declaration,
        parameters,
        return_type,
    } = &wrapper.outcome
    else {
        panic!("{:?}", wrapper.outcome);
    };
    let owner = &bindings.declarations[declaration.0];
    assert_eq!(owner.role, DeclarationRole::Predicate);
    assert_eq!(context.files[owner.file].path, wrapper_path);
    assert_eq!(context.files[owner.file].kind, SourceKind::StandardLibrary);
    assert_eq!(context.files[owner.file].implicit, !redefined);
    let boolean = TypeInst {
        instantiation: Instantiation::Decision,
        optional: false,
        kind: TypeKind::Bool,
    };
    assert_eq!(parameters.as_slice(), std::slice::from_ref(&boolean));
    assert_eq!(*return_type, boolean);
    let facts = resolve_symmetry_uses(&context, &bindings, &calls);
    assert_eq!(
        facts
            .uses
            .iter()
            .filter(|u| u.outcome == SymmetryUseOutcome::Marked)
            .count(),
        13
    );
    assert_eq!(
        facts
            .uses
            .iter()
            .filter(|u| u.outcome == SymmetryUseOutcome::Unmarked)
            .count(),
        13
    );
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unmarked-symmetry-breaking").unwrap(),
    );
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{:?} {:?}",
        result.errors,
        result.limitations
    );
    assert_eq!(result.rules[0].outcome, RuleOutcome::Completed);
    assert_eq!(result.findings.len(), 12);
    for (name, finding) in FAMILY.iter().zip(&result.findings) {
        assert_eq!(
            &source[finding.location.range.clone()],
            format!("{name}(x)")
        );
        assert_eq!(finding.location.path, root);
        assert!(finding.message.contains("intended to break symmetry?"));
        assert!(finding.message.contains("required model logic"));
    }
    let combined = analyze_model(
        &context,
        &LintOptions::from_selection("unmarked-symmetry-breaking,missing-constraint-label")
            .unwrap(),
    );
    let suppressed_item = source.rfind("constraint increasing(x);").unwrap();
    assert!(combined.findings.iter().any(
        |f| f.rule == Rule::MissingConstraintLabel && f.location.range.start == suppressed_item
    ));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    let run = |path: &std::path::Path| {
        std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
            .args(["--rules", "unmarked-symmetry-breaking", "--stdlib-dir"])
            .arg(options.stdlib_dir.as_ref().unwrap())
            .arg(path)
            .output()
            .unwrap()
    };
    let output = run(&root);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr)
            .unwrap()
            .matches("warning [unmarked-symmetry-breaking]")
            .count(),
        12
    );
    let wrapped = directory.join("wrapped.mzn");
    std::fs::write(&wrapped, "include \"family.mzn\"; var int: x; constraint symmetry_breaking_constraint(increasing(x)); solve satisfy;").unwrap();
    assert_eq!(run(&wrapped).status.code(), Some(0));
    let invalid = directory.join("invalid.mzn");
    std::fs::write(&invalid, "constraint ;").unwrap();
    assert_eq!(run(&invalid).status.code(), Some(2));
    assert_eq!(
        Rule::THESIS
            .iter()
            .filter(|r| !r.is_available())
            .copied()
            .collect::<Vec<_>>(),
        Vec::<Rule>::new()
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unknown_wrapper_or_target_selection_is_limited_without_guessing() {
    let (directory, options) = setup(
        "limits",
        "predicate symmetry_breaking_constraint(MissingType: b)=true;",
    );
    let root = directory.join("root.mzn");
    std::fs::write(&root, "include \"family.mzn\"; var int: x; constraint symmetry_breaking_constraint(increasing(x)); constraint decreasing(missing()); solve satisfy;").unwrap();
    let context = load_model(&root, &options);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("unmarked-symmetry-breaking,naming").unwrap(),
    );
    assert_eq!(result.status(), 0);
    assert!(result.errors.is_empty() && result.findings.is_empty());
    assert!(matches!(
        result.rules[0].outcome,
        RuleOutcome::Limited { .. }
    ));
    assert_eq!(result.rules[1].outcome, RuleOutcome::Completed);
    assert_eq!(result.limitations.len(), 2);
    assert!(
        result
            .limitations
            .iter()
            .any(|l| l.message.contains("wrapper identity is unknown"))
    );
    std::fs::remove_dir_all(directory).unwrap();
}
