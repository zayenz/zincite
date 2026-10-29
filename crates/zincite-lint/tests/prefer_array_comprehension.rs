use std::path::PathBuf;
use zincite_lint::*;

const CORE: &str = concat!(
    "function set of int: '..'(int:a,int:b);\n",
    "function array[$$E] of any $V: array1d(set of $$E:S,array[$U] of any $V:x);\n",
    "function array[$$E,$$F] of any $V: array2d(set of $$E:S,set of $$F:T,array[$U] of any $V:x);\n",
    "function array[$$E,$$F,$$G] of any $V: array3d(set of $$E:S,set of $$F:T,set of $$G:U,array[$V] of any $W:x);\n",
);
fn model(name: &str, source: &str) -> (PathBuf, ModelContext) {
    let dir = std::env::temp_dir().join(format!(
        "zincite-direct-comprehension-{name}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(dir.join("library/std")).unwrap();
    std::fs::write(dir.join("library/std/stdlib.mzn"), CORE).unwrap();
    std::fs::write(dir.join("root.mzn"), source).unwrap();
    let context = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..ModelOptions::default()
        },
    );
    assert!(context.errors.is_empty(), "{:?}", context.errors);
    (dir, context)
}

#[test]
fn fixed_sets_preserve_keys_axis_order_and_exact_retained_text() {
    let source = concat!(
        "set of int: One=1..3; set of int: Shift=3..4; enum Color={red,blue};\n",
        "array[int] of int:plain=array1d(One,[i /* body */ | i in One]);\n",
        "array[Shift] of int:shift=array1d(Shift,[i | i in Shift]);\n",
        "array[Color] of Color:colors=array1d(Color,[c | c in Color]);\n",
        "array[int,int] of int:matrix=array2d(Shift,One,[j | i in Shift,j in One]);\n",
        "array[Color,int,int] of int:cube=array3d(Color,One,Shift,[j | c in Color,j in One,k in Shift]);\n",
        "array[int,int] of int:grouped=array2d(One,One,[i | i,j in One]);\n",
        "int: count; array[int] of int:symbolic=array1d(1..count,[i | i in 1..count]);\n",
        "array[int] of int:empty=array1d(1..0,[i | i in 1..0]); solve satisfy;\n",
    );
    let (dir, context) = model("positive", source);
    let options = LintOptions::from_selection("prefer-array-comprehension").unwrap();
    let result = analyze_model(&context, &options);
    assert!(
        result.errors.is_empty() && result.limitations.is_empty(),
        "{result:?}"
    );
    assert_eq!(result.findings.len(), 8, "{result:?}");
    assert!(
        result
            .findings
            .iter()
            .all(|f| f.fix.as_ref().is_some_and(|f| f.safety == FixSafety::Safe))
    );
    let snapshot = result.findings[0].fix.as_ref().unwrap().snapshot.clone();
    let edits = prepare_fixes(
        &snapshot,
        source.as_bytes(),
        &result.findings,
        &FixOptions::default(),
    )
    .unwrap();
    let fixed = String::from_utf8(edits.candidate).unwrap();
    for expected in [
        "plain=[i /* body */ | i in One]",
        "shift=[i: i | i in Shift]",
        "colors=[c: c | c in Color]",
        "matrix=[(i,j): j | i in Shift,j in One]",
        "cube=[(c,j,k): j | c in Color,j in One,k in Shift]",
        "grouped=[(i,j): i | i,j in One]",
        "empty=[i | i in 1..0]",
        "symbolic=[i | i in 1..count]",
    ] {
        assert!(fixed.contains(expected), "{fixed}");
    }
    std::fs::write(dir.join("root.mzn"), &fixed).unwrap();
    let after = load_model(
        dir.join("root.mzn"),
        &ModelOptions {
            stdlib_dir: Some(dir.join("library")),
            ..ModelOptions::default()
        },
    );
    assert!(analyze_model(&after, &options).findings.is_empty());
    assert_eq!(
        analyze_model(&context, &LintOptions::default())
            .findings
            .iter()
            .filter(|f| f.rule == Rule::PreferArrayComprehension)
            .count(),
        8
    );
    assert!(Rule::PreferArrayComprehension.requires_model());
    assert_eq!(
        Rule::PreferArrayComprehension.metadata().family,
        RuleFamily::Style
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn exclusions_keep_unknown_effects_filters_dependencies_and_wrapper_comments() {
    let source = concat!(
        "set of int: Unknown; enum Open; set of int: One=1..3;\n",
        "function set of int: effect(set of int:s)=s;\n",
        "array[int] of int:keyed=array1d(Unknown,[i | i in Unknown]);\n",
        "array[int] of int:effectful=array1d(effect(One),[i | i in effect(One)]);\n",
        "array[int] of int:filtered=array1d(One,[i | i in One where true]);\n",
        "array[int,int] of int:dependent=array2d(One,One,[j | i in One,j in 1..i]);\n",
        "array[int,int] of int:uncertain=array2d(Unknown,One,[j | i in Unknown,j in One]);\n",
        "array[int,int] of int:empty=array2d(1..0,One,[j | i in 1..0,j in One]);\n",
        "array[Open,int] of int:open_enum=array2d(Open,One,[j | i in Open,j in One]);\n",
        "array[int] of int:commented=array1d(/* retained wrapper */One,[i | i in One]);\n",
        "% zincite-lint: ignore prefer-array-comprehension\n",
        "array[int] of int:suppressed=array1d(One,[i | i in One]);\n",
        "solve satisfy;\n",
    );
    let (dir, context) = model("excluded", source);
    let result = analyze_model(
        &context,
        &LintOptions::from_selection("prefer-array-comprehension").unwrap(),
    );
    assert_eq!(result.findings.len(), 2, "{result:?}");
    assert!(
        result
            .findings
            .iter()
            .any(|f| f.fix.is_none()
                && source[f.location.range.clone()].contains("retained wrapper"))
    );
    let fix = result.findings.iter().find_map(|f| f.fix.as_ref()).unwrap();
    let fixed = prepare_edits(&fix.snapshot, source.as_bytes(), std::slice::from_ref(fix)).unwrap();
    assert!(
        String::from_utf8(fixed.candidate)
            .unwrap()
            .contains("keyed=[i: i | i in Unknown]")
    );
    std::fs::remove_dir_all(dir).unwrap();
    let (dir, context) = model(
        "shadow",
        "function array[int] of int: array1d(set of int:s,array[int] of int:a)=a; array[int] of int:a=array1d(1..3,[i | i in 1..3]); solve satisfy;",
    );
    assert!(
        analyze_model(
            &context,
            &LintOptions::from_selection("prefer-array-comprehension").unwrap()
        )
        .findings
        .is_empty()
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn standalone_cli_default_diff_fix_selection_and_suppression() {
    let source = "set of int: Index=2..3; array[Index] of int:values=array1d(Index,[i /* keep */ | i in Index]); constraint values[2] = 2; solve satisfy;\n";
    let (dir, _) = model("cli", source);
    let root = dir.join("root.mzn");
    let library = dir.join("library");
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
            .args(["--isolated", "--stdlib-dir", library.to_str().unwrap()])
            .args(args)
            .arg(&root)
            .output()
            .unwrap()
    };
    let default = run(&[]);
    assert_eq!(default.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&default.stderr).contains("[prefer-array-comprehension]"));
    let diff = run(&["--diff"]);
    assert!(String::from_utf8_lossy(&diff.stdout).contains("[i: i /* keep */ | i in Index]"));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    std::fs::write(
        dir.join("zincite.toml"),
        "[lint]\nunfixable=['prefer-array-comprehension']\n",
    )
    .unwrap();
    let excluded = std::process::Command::new(env!("CARGO_BIN_EXE_zincite-lint"))
        .args(["--stdlib-dir", library.to_str().unwrap(), "--fix"])
        .arg(&root)
        .output()
        .unwrap();
    assert_eq!(excluded.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), source);
    assert_eq!(
        run(&["--rules", "prefer-array-comprehension", "--fix"])
            .status
            .code(),
        Some(0)
    );
    let fixed = std::fs::read_to_string(&root).unwrap();
    assert!(fixed.contains("[i: i /* keep */ | i in Index]"));
    assert_eq!(run(&["--fix"]).status.code(), Some(0));
    assert_eq!(std::fs::read_to_string(&root).unwrap(), fixed);
    std::fs::write(
        &root,
        source.replace(
            "Index=2..3; array",
            "Index=2..3;\n% zincite-lint: ignore prefer-array-comprehension\narray",
        ),
    )
    .unwrap();
    let suppressed = run(&["--rules", "prefer-array-comprehension", "--fix"]);
    assert_eq!(suppressed.status.code(), Some(0));
    assert!(
        std::fs::read_to_string(&root)
            .unwrap()
            .contains("array1d(Index")
    );
    std::fs::remove_dir_all(dir).unwrap();
}
