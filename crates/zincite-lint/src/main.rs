use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use zincite_lint::settings::LintSettings;
use zincite_lint::{
    AnalysisResult, FixOptions, LintOptions, ModelOptions, PreparedFixes, Rule, SourceSnapshot,
    analyze_file, analyze_model, load_model, prepare_fixes, replace_fixed_file, write_analysis,
    write_fix_diff,
};
use zincite_syntax::{FileMode, parse_with_mode};

const HELP: &str = "Usage: zincite-lint [--list-rules | --explain RULE]
       zincite-lint [--rules SELECTION] [--config PATH | --isolated] [--show-settings | --fix | --diff] [--unsafe-fixes] [-I DIR] [--stdlib-dir DIR] [--stdin-filepath PATH] [FILE|DIR...|-]

Report MiniZinc modelling advice on stderr. Ordinary linting never rewrites source.
No input or '-' reads stdin; stdin cannot be mixed with files.
--stdin-filepath PATH selects the stdin language mode, diagnostic path and settings lookup anchor.
An unsaved filepath works for settings lookup; plain stdin does not discover settings.
Directories recursively discover .mzn/.dzn files, including hidden/ignored files.
Discovery skips .git, visits directory symlinks once and sorts directory entries.
Overlapping inputs use the first path to each file.
.dzn paths use assignment-only data syntax; other paths use model syntax.

--rules accepts comma-separated IDs, family:NAME, preset:NAME,
or legacy default/thesis/all. Expansion preserves first occurrence and removes duplicates.
Families are correctness, suspicious, modelling, performance and style.
Unknown/empty selectors and repeated --rules are errors.

Each root uses its nearest ancestor zincite.toml; ancestor files are not merged.
Its [lint] table accepts select, extend-select and ignore arrays of selectors.
Absent select uses default; select = [] selects no rules. Extend then ignore; ignore wins.
--rules replaces all configured selection, additions and exclusions after validating the file.
--config PATH applies one settings file to all roots; --isolated disables discovery.
--config and --isolated are mutually exclusive. Include closures use their root's settings.
Invalid settings for any root fail before analysis or partial settings output.
--show-settings prints root labels, settings paths and expanded IDs without reading models/stdin.
Personal preset members use existing selectors; they cannot include other personal presets.
Options merge defaults, the selected personal preset, then lint.options overrides.
--rules retains configured effective options; a named personal preset selects its own bundle.
--show-settings also prints exact ignored names and the positive candidate threshold.
[lint].fixable defaults to all rules; unfixable defaults to none and wins exclusions.
These rule/family/built-in preset selectors restrict fixes without selecting diagnostics.
--rules retains fix restrictions. --show-settings prints their resolved IDs.
--fix applies eligible fixes once to explicit regular user files, then reanalyses all roots.
--diff previews eligible fixes without writes and reports original diagnostics/status.
Safe fixes alone are eligible; --unsafe-fixes explicitly admits described unsafe fixes.
--fix and --diff are mutually exclusive and cannot be used with stdin, directories,
symlinks or --show-settings. --unsafe-fixes requires --fix or --diff.
Include-only and configured standard-library files are never rewritten.
Conflicting atomic groups are all omitted and reported; independent groups survive.
Every candidate must parse and its original file must still match before replacement.
Current rule producers supply no edits, so fix modes leave their diagnostics and bytes unchanged.
The thesis preset selects its fourteen rules; all includes every registered rule,
including unavailable rules that fail execution. Default remains two rules.
--list-rules lists rule IDs, families, availability, fix support and preset membership.
--explain RULE describes a rule's purpose, required facts, limitations and options.
These inspection commands run alone, write to stdout and do not read input.
Rule entries report availability, implemented fix support and current options.

-I DIR adds an ordered include directory for rules requiring model analysis.
--stdlib-dir DIR supplies the MiniZinc library root, overriding MZN_STDLIB_DIR.
Current default rules do not load includes or the standard library.
Stdin has no model context: selected semantic analysis reports a limitation;
standalone .dzn inputs mark semantic rules inapplicable.

Default rules are naming and missing-constraint-label; all rules remain warnings.
Use --explain for each rule's naming exemptions, label syntax or analysis limits.
Standalone suppressions apply throughout the next top-level item:
    % zincite-lint: ignore missing-constraint-label
    % zincite-lint: ignore naming
Consecutive comments can suppress both rules independently, including nested bindings.
Every registered rule ID can be suppressed even when disabled. Suppressions affect
only their own rule. Unknown, malformed, misplaced or dangling directives are errors.

The shared parser covers MiniZinc 2.10.1 model/data syntax; reserved
variant_record and case syntax is unsupported. Full semantic validity and solver
performance are not checked.
Syntax errors omit lint rules for that file. Independent files are processed.
Exit codes: 0 clean, 1 unsuppressed warnings, 2 input/parse/directive/configuration/usage/dependency errors.
Analysis limitations alone retain status 0 or 1.
-h, --help shows this help.
";

fn main() -> ExitCode {
    match run() {
        Ok(status) => ExitCode::from(status),
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8, String> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        io::stdout()
            .write_all(HELP.as_bytes())
            .map_err(|error| format!("stdout: {error}"))?;
        return Ok(0);
    }
    if arguments
        .first()
        .is_some_and(|argument| matches!(argument.to_str(), Some("--list-rules" | "--explain")))
    {
        return inspect_rules(&arguments);
    }
    let Arguments {
        paths,
        stdin_filepath,
        cli_selection,
        configuration,
        show_settings,
        model,
        mode,
        unsafe_fixes,
    } = parse_arguments(
        arguments,
        std::env::var_os("MZN_STDLIB_DIR").map(PathBuf::from),
    )?;
    if mode != Mode::Ordinary {
        let paths = explicit_fix_paths(&paths, &model)?;
        let anchors: Vec<_> = paths.iter().map(|path| Some(path.as_path())).collect();
        let mut settings =
            preflight_settings(&anchors, &configuration, cli_selection.as_deref(), true)?;
        for settings in &mut settings {
            settings.fixes.unsafe_fixes = unsafe_fixes;
        }
        return run_fixes(&paths, &settings, &model, mode);
    }
    let inputs = (!paths.is_empty()).then(|| zincite_syntax::inputs::discover_inputs(&paths));
    let anchors = match &inputs {
        Some(inputs) => inputs
            .files
            .iter()
            .map(|path| Some(path.as_path()))
            .collect(),
        None => vec![stdin_filepath.as_deref()],
    };
    let settings = preflight_settings(
        &anchors,
        &configuration,
        cli_selection.as_deref(),
        !show_settings,
    )?;
    if show_settings {
        let mut output = io::stdout().lock();
        for (anchor, settings) in anchors.iter().zip(&settings) {
            let label = anchor.map_or_else(|| "<stdin>".into(), |path| path.to_string_lossy());
            write_settings(&mut output, &label, settings)
                .map_err(|error| format!("stdout: {error}"))?;
        }
    }
    let Some(inputs) = inputs else {
        if show_settings {
            return Ok(0);
        }
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("<stdin>: {error}"))?;
        let label = stdin_filepath.as_ref().map_or_else(
            || "<stdin>".into(),
            |path| path.to_string_lossy().into_owned(),
        );
        let mode = stdin_filepath.map_or(FileMode::Model, FileMode::from_path);
        return check_source(&label, bytes, mode, &settings[0].options);
    };
    let mut status = if inputs.errors.is_empty() { 0 } else { 2 };
    for (path, error) in inputs.errors {
        let _ = writeln!(io::stderr(), "{}: {error}", path.display());
    }
    if show_settings {
        return Ok(status);
    }
    for (path, settings) in inputs.files.into_iter().zip(settings) {
        let options = settings.options;
        let label = path.to_string_lossy();
        let result = if options.requires_model() && FileMode::from_path(&path) == FileMode::Model {
            let context = load_model(&path, &model);
            write_analysis(&analyze_model(&context, &options), &mut io::stderr())
                .map_err(|error| format!("stderr: {error}"))
        } else {
            std::fs::read(&path)
                .map_err(|error| format!("{label}: {error}"))
                .and_then(|bytes| check_source(&label, bytes, FileMode::from_path(&path), &options))
        };
        match result {
            Ok(file_status) => status = status.max(file_status),
            Err(message) => {
                let _ = writeln!(io::stderr(), "{message}");
                status = 2;
            }
        }
    }
    Ok(status)
}

// Validate every written alias before deduplication. Ordinary discovery remains
// read-only and keeps its existing directory/symlink behavior.
fn explicit_fix_paths(paths: &[PathBuf], model: &ModelOptions) -> Result<Vec<PathBuf>, String> {
    let standard = model
        .stdlib_dir
        .as_ref()
        .map(|path| {
            path.canonicalize()
                .map_err(|error| format!("{}: {error}", path.display()))
        })
        .transpose()?;
    let mut seen = std::collections::BTreeSet::new();
    let mut files = Vec::new();
    for path in paths {
        let metadata = std::fs::symlink_metadata(path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if !metadata.file_type().is_file() {
            return Err(format!(
                "{}: --fix/--diff require regular non-symlink files",
                path.display()
            ));
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if standard
            .as_ref()
            .is_some_and(|standard| canonical.starts_with(standard))
        {
            return Err(format!(
                "{}: --fix/--diff refuse configured standard-library files",
                path.display()
            ));
        }
        if seen.insert(canonical) {
            files.push(path.clone());
        }
    }
    Ok(files)
}

// Retain original bytes only when a root finding supplies an edit. Semantic
// include contexts are dropped on return; include-only findings are diagnostic.
fn analyze_fix_root(
    path: &Path,
    options: &LintOptions,
    model: &ModelOptions,
    retain: bool,
) -> Result<(AnalysisResult, Option<SourceSnapshot>), String> {
    if options.requires_model() && FileMode::from_path(path) == FileMode::Model {
        let context = load_model(path, model);
        let result = analyze_model(&context, options);
        let snapshot = if retain
            && result
                .findings
                .iter()
                .any(|finding| finding.location.path == path && finding.fix.is_some())
        {
            context
                .root_file
                .map(|file| context.files[file].source_snapshot())
        } else {
            None
        };
        return Ok((result, snapshot));
    }
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let offset = source.len() - text.len();
    let parsed = parse_with_mode(text, FileMode::from_path(path));
    let result = analyze_file(&parsed, path, offset, options);
    let snapshot = (retain && result.findings.iter().any(|finding| finding.fix.is_some()))
        .then(|| SourceSnapshot::new(path, source));
    Ok((result, snapshot))
}

fn run_fixes(
    paths: &[PathBuf],
    settings: &[ResolvedSettings],
    model: &ModelOptions,
    mode: Mode,
) -> Result<u8, String> {
    let mut status = 0;
    let mut prepared: Vec<(SourceSnapshot, PreparedFixes)> = Vec::new();
    for (path, settings) in paths.iter().zip(settings) {
        let analyzed = analyze_fix_root(path, &settings.options, model, true);
        match analyzed {
            Err(error) => {
                writeln!(io::stderr(), "{error}").map_err(|error| format!("stderr: {error}"))?;
                status = 2;
            }
            Ok((result, snapshot)) => {
                if mode == Mode::Diff {
                    status = status.max(
                        write_analysis(&result, &mut io::stderr())
                            .map_err(|error| format!("stderr: {error}"))?,
                    );
                } else if !result.errors.is_empty() {
                    status = 2;
                }
                if result.errors.is_empty()
                    && let Some(snapshot) = snapshot
                {
                    match prepare_fixes(
                        &snapshot,
                        snapshot.source(),
                        &result.findings,
                        &settings.fixes,
                    ) {
                        Err(error) => {
                            writeln!(io::stderr(), "{}: {error}", path.display())
                                .map_err(|error| format!("stderr: {error}"))?;
                            status = 2;
                        }
                        Ok(candidate) => {
                            for conflict in &candidate.conflicts {
                                writeln!(io::stderr(), "{}: omitted fix '{}' (finding {}): conflicts with findings {:?}", path.display(), conflict.title, conflict.finding, conflict.conflicts_with)
                                    .map_err(|error| format!("stderr: {error}"))?;
                            }
                            if mode == Mode::Diff {
                                write_fix_diff(&snapshot, &candidate.candidate, &mut io::stdout())
                                    .map_err(|error| format!("stdout: {error}"))?;
                            } else if candidate.candidate != snapshot.source() {
                                prepared.push((snapshot, candidate));
                            }
                        }
                    }
                }
            }
        }
    }
    if mode == Mode::Diff {
        return Ok(status);
    }
    for (snapshot, candidate) in prepared {
        if let Err(error) = replace_fixed_file(&snapshot, &candidate.candidate) {
            writeln!(io::stderr(), "{}: {error}", snapshot.path().display())
                .map_err(|error| format!("stderr: {error}"))?;
            status = 2;
        }
    }
    // All explicit writes finish before any final include closure is loaded.
    // The final diagnostics never feed another preparation/application pass.
    for (path, settings) in paths.iter().zip(settings) {
        match analyze_fix_root(path, &settings.options, model, false) {
            Ok((result, _)) => {
                status = status.max(
                    write_analysis(&result, &mut io::stderr())
                        .map_err(|error| format!("stderr: {error}"))?,
                )
            }
            Err(error) => {
                writeln!(io::stderr(), "{error}").map_err(|error| format!("stderr: {error}"))?;
                status = 2;
            }
        }
    }
    Ok(status)
}

fn inspect_rules(arguments: &[std::ffi::OsString]) -> Result<u8, String> {
    let mut output = io::stdout().lock();
    let result = if arguments.len() == 1 && arguments[0] == "--list-rules" {
        write_rule_list(&mut output)
    } else if arguments.len() == 2 && arguments[0] == "--explain" {
        let id = arguments[1]
            .to_str()
            .ok_or("zincite-lint: rule ID must be UTF-8")?;
        let rule =
            Rule::from_id(id).ok_or_else(|| format!("zincite-lint: unknown rule ID '{id}'"))?;
        write_rule_explanation(rule, &mut output)
    } else {
        return Err("zincite-lint: use --list-rules or --explain RULE alone".into());
    };
    result.map_err(|error| format!("stdout: {error}"))?;
    Ok(0)
}

fn write_rule_list(output: &mut impl Write) -> io::Result<()> {
    writeln!(output, "ID\tFamily\tAvailability\tFixes\tPreset\tPurpose")?;
    for rule in Rule::all() {
        let metadata = rule.metadata();
        writeln!(
            output,
            "{}\t{}\t{}\t{}\t{}\t{}",
            metadata.id,
            metadata.family.as_str(),
            if metadata.available {
                "available"
            } else {
                "unavailable"
            },
            metadata.fix_support.as_str(),
            if Rule::DEFAULT.contains(&rule) {
                "default"
            } else if Rule::THESIS.contains(&rule) {
                "thesis"
            } else {
                "opt-in"
            },
            metadata.purpose,
        )?;
    }
    Ok(())
}

fn write_rule_explanation(rule: Rule, output: &mut impl Write) -> io::Result<()> {
    let metadata = rule.metadata();
    writeln!(output, "{}: {}", metadata.id, metadata.purpose)?;
    writeln!(output, "Family: {}", metadata.family.as_str())?;
    writeln!(
        output,
        "Families describe purpose, not severity or fix safety."
    )?;
    writeln!(
        output,
        "Availability: {}",
        if metadata.available {
            "available"
        } else {
            "unavailable"
        }
    )?;
    writeln!(
        output,
        "Preset: {}",
        if Rule::DEFAULT.contains(&rule) {
            "default"
        } else if Rule::THESIS.contains(&rule) {
            "thesis"
        } else {
            "opt-in"
        }
    )?;
    writeln!(
        output,
        "Requires model context: {}",
        if metadata.requires_model { "yes" } else { "no" }
    )?;
    writeln!(output, "Required facts: {}", metadata.requirements)?;
    writeln!(output, "Fix support: {}", metadata.fix_support.as_str())?;
    writeln!(
        output,
        "Fix support describes implemented edits; individual findings may have stricter conditions."
    )?;
    writeln!(
        output,
        "Options: {}",
        if metadata.options.is_empty() {
            "none".to_owned()
        } else {
            metadata.options.join(", ")
        }
    )?;
    writeln!(output, "Limitations: {}", metadata.limitations)
}

struct Arguments {
    paths: Vec<PathBuf>,
    stdin_filepath: Option<PathBuf>,
    cli_selection: Option<Vec<String>>,
    configuration: Configuration,
    show_settings: bool,
    model: ModelOptions,
    mode: Mode,
    unsafe_fixes: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Ordinary,
    Fix,
    Diff,
}

enum Configuration {
    Discover,
    Explicit(PathBuf),
    Isolated,
}

#[derive(Clone)]
struct ResolvedSettings {
    path: Option<PathBuf>,
    options: LintOptions,
    fixes: FixOptions,
}

// Validate every root before any input analysis or settings output. Explicit
// settings and CLI selectors are validated even when discovery finds no roots.
fn preflight_settings(
    anchors: &[Option<&Path>],
    configuration: &Configuration,
    cli_selection: Option<&[String]>,
    execute: bool,
) -> Result<Vec<ResolvedSettings>, String> {
    let explicit = match configuration {
        Configuration::Explicit(path) => Some(read_settings(path, cli_selection, execute)?),
        _ => None,
    };
    if anchors.is_empty() && explicit.is_none() {
        let defaults = LintSettings::default();
        let options = match cli_selection {
            Some(selection) => defaults.resolve_selection(selection),
            None => defaults.resolve(),
        }
        .map_err(|error| format!("zincite-lint: {error}"))?;
        if execute {
            options
                .check_available()
                .map_err(|error| format!("zincite-lint: {error}"))?;
        }
    }
    anchors
        .iter()
        .map(|anchor| {
            if let Some(settings) = &explicit {
                return Ok(settings.clone());
            }
            if matches!(configuration, Configuration::Discover)
                && let Some(anchor) = anchor
                && let Some(path) = nearest_settings(anchor)?
            {
                return read_settings(&path, cli_selection, execute);
            }
            let defaults = LintSettings::default();
            let options = match cli_selection {
                Some(selection) => defaults.resolve_selection(selection),
                None => defaults.resolve(),
            }
            .map_err(|error| format!("zincite-lint: {error}"))?;
            if execute {
                options
                    .check_available()
                    .map_err(|error| format!("zincite-lint: {error}"))?;
            }
            Ok(ResolvedSettings {
                path: None,
                options,
                fixes: defaults
                    .resolve_fixes()
                    .map_err(|error| format!("zincite-lint: {error}"))?,
            })
        })
        .collect()
}

fn read_settings(
    path: &Path,
    cli_selection: Option<&[String]>,
    execute: bool,
) -> Result<ResolvedSettings, String> {
    let source =
        std::fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let settings =
        LintSettings::from_toml(&source).map_err(|error| format!("{}: {error}", path.display()))?;
    let fixes = settings
        .resolve_fixes()
        .map_err(|error| format!("{}: {error}", path.display()))?;
    let options = match cli_selection {
        Some(selection) => settings.resolve_selection(selection),
        None => settings.resolve(),
    }
    .map_err(|error| format!("{}: {error}", path.display()))?;
    if execute {
        options
            .check_available()
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(ResolvedSettings {
        path: Some(path.to_path_buf()),
        options,
        fixes,
    })
}

fn nearest_settings(anchor: &Path) -> Result<Option<PathBuf>, String> {
    let absolute = if anchor.is_absolute() {
        anchor.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| format!("current directory: {error}"))?
            .join(anchor)
    };
    // Resolve the containing directory, not the file: stdin's filepath may be
    // unsaved. Start at the nearest existing directory if its parents are new.
    let mut directory = absolute.parent();
    let parent = loop {
        let Some(path) = directory else {
            return Ok(None);
        };
        match path.canonicalize() {
            Ok(parent) => break parent,
            Err(error) if error.kind() == io::ErrorKind::NotFound => directory = path.parent(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    };
    for directory in parent.ancestors() {
        let candidate = directory.join("zincite.toml");
        match std::fs::symlink_metadata(&candidate) {
            Ok(_) => return Ok(Some(candidate)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("{}: {error}", candidate.display())),
        }
    }
    Ok(None)
}

fn write_settings(
    output: &mut impl Write,
    label: &str,
    settings: &ResolvedSettings,
) -> io::Result<()> {
    writeln!(output, "Root: {label}")?;
    match &settings.path {
        Some(path) => writeln!(output, "Settings: {}", path.display())?,
        None => writeln!(output, "Settings: none")?,
    }
    writeln!(
        output,
        "Rules: {}",
        settings
            .options
            .rules
            .iter()
            .map(|rule| rule.id())
            .collect::<Vec<_>>()
            .join(",")
    )?;
    for (name, rules) in [
        ("fixable", &settings.fixes.fixable),
        ("unfixable", &settings.fixes.unfixable),
    ] {
        writeln!(
            output,
            "{name}: {}",
            rules
                .iter()
                .map(|rule| rule.id())
                .collect::<Vec<_>>()
                .join(",")
        )?;
    }
    writeln!(
        output,
        "suspicious-shadowing.ignore-names: {}",
        toml::Value::Array(
            settings
                .options
                .parameters
                .shadowing_ignore_names
                .iter()
                .cloned()
                .map(toml::Value::String)
                .collect()
        )
    )?;
    writeln!(
        output,
        "expensive-comprehension.max-candidates: {}",
        settings.options.parameters.comprehension_max_candidates
    )
}

fn parse_arguments(
    arguments: Vec<std::ffi::OsString>,
    stdlib_fallback: Option<PathBuf>,
) -> Result<Arguments, String> {
    let mut paths = Vec::new();
    let mut stdin_filepath = None;
    let mut rule_selection = None;
    let mut configuration = Configuration::Discover;
    let mut show_settings = false;
    let mut mode = Mode::Ordinary;
    let mut unsafe_fixes = false;
    let mut model = ModelOptions::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--fix" || argument == "--diff" {
            if mode != Mode::Ordinary {
                return Err("zincite-lint: use --fix or --diff once, not both".into());
            }
            mode = if argument == "--fix" {
                Mode::Fix
            } else {
                Mode::Diff
            };
        } else if argument == "--unsafe-fixes" {
            if unsafe_fixes {
                return Err("zincite-lint: repeated --unsafe-fixes".into());
            }
            unsafe_fixes = true;
        } else if argument == "--rules" {
            if rule_selection.is_some() {
                return Err("zincite-lint: repeated --rules".into());
            }
            let selection = arguments
                .next()
                .ok_or("zincite-lint: --rules requires a selection")?;
            let selection = selection
                .to_str()
                .ok_or("zincite-lint: rule selection must be UTF-8")?;
            rule_selection = Some(selection.split(',').map(str::to_owned).collect());
        } else if argument == "--config" {
            if !matches!(configuration, Configuration::Discover) {
                return Err("zincite-lint: use --config or --isolated once, not both".into());
            }
            configuration = Configuration::Explicit(PathBuf::from(
                arguments
                    .next()
                    .ok_or("zincite-lint: --config requires a path")?,
            ));
        } else if argument == "--isolated" {
            if !matches!(configuration, Configuration::Discover) {
                return Err("zincite-lint: use --config or --isolated once, not both".into());
            }
            configuration = Configuration::Isolated;
        } else if argument == "--show-settings" {
            show_settings = true;
        } else if argument == "-I" {
            model.include_dirs.push(PathBuf::from(
                arguments
                    .next()
                    .ok_or("zincite-lint: -I requires a directory")?,
            ));
        } else if argument == "--stdlib-dir" {
            if model.stdlib_dir.is_some() {
                return Err("zincite-lint: repeated --stdlib-dir".into());
            }
            model.stdlib_dir = Some(PathBuf::from(
                arguments
                    .next()
                    .ok_or("zincite-lint: --stdlib-dir requires a directory")?,
            ));
        } else if argument == "--stdin-filepath" {
            if stdin_filepath.is_some() {
                return Err("zincite-lint: repeated --stdin-filepath".into());
            }
            stdin_filepath = Some(PathBuf::from(
                arguments
                    .next()
                    .ok_or("zincite-lint: --stdin-filepath requires a path")?,
            ));
        } else {
            if argument != "-" && argument.to_string_lossy().starts_with('-') {
                return Err("zincite-lint: unsupported option; use --help for usage".into());
            }
            paths.push(PathBuf::from(argument));
        }
    }
    if paths.iter().any(|path| path == Path::new("-")) {
        if paths.len() != 1 {
            return Err("zincite-lint: stdin cannot be mixed with other inputs".into());
        }
        paths.clear();
    }
    if stdin_filepath.is_some() && !paths.is_empty() {
        return Err("zincite-lint: --stdin-filepath requires stdin".into());
    }
    if unsafe_fixes && mode == Mode::Ordinary {
        return Err("zincite-lint: --unsafe-fixes requires --fix or --diff".into());
    }
    if mode != Mode::Ordinary && (paths.is_empty() || show_settings) {
        return Err("zincite-lint: --fix/--diff require explicit files and cannot use stdin or --show-settings".into());
    }
    model.stdlib_dir = model.stdlib_dir.or(stdlib_fallback);
    Ok(Arguments {
        paths,
        stdin_filepath,
        cli_selection: rule_selection,
        configuration,
        show_settings,
        model,
        mode,
        unsafe_fixes,
    })
}

fn check_source(
    label: &str,
    bytes: Vec<u8>,
    mode: FileMode,
    options: &LintOptions,
) -> Result<u8, String> {
    let source = String::from_utf8(bytes).map_err(|error| {
        format!(
            "{label}: input is not UTF-8 at byte {}",
            error.utf8_error().valid_up_to()
        )
    })?;
    let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let offset = source.len() - text.len();
    let parsed = parse_with_mode(text, mode);
    write_analysis(
        &analyze_file(&parsed, label, offset, options),
        &mut io::stderr(),
    )
    .map_err(|error| format!("stderr: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_library_overrides_fallback_and_include_directories_remain_ordered() {
        let arguments = ["-I", "first", "-I", "second", "--stdlib-dir", "explicit"]
            .into_iter()
            .map(Into::into)
            .collect();
        let parsed = parse_arguments(arguments, Some("environment".into())).unwrap();
        assert_eq!(parsed.model.stdlib_dir, Some("explicit".into()));
        assert_eq!(
            parsed.model.include_dirs,
            [PathBuf::from("first"), PathBuf::from("second")]
        );
        let fallback = parse_arguments(Vec::new(), Some("environment".into())).unwrap();
        assert_eq!(fallback.model.stdlib_dir, Some("environment".into()));
    }
}
