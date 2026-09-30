use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use zincite_lint::{
    LintOptions, ModelOptions, analyze_file, analyze_model, load_model, write_analysis,
};
use zincite_syntax::{FileMode, parse_with_mode};

const HELP: &str = "Usage: zincite-lint [--rules SELECTION] [-I DIR] [--stdlib-dir DIR] [--stdin-filepath PATH] [FILE|DIR...|-]

Report MiniZinc modelling advice on stderr without rewriting source.
No input or '-' reads stdin; stdin cannot be mixed with files.
--stdin-filepath PATH selects the stdin language mode and diagnostic path.
Directories recursively discover .mzn/.dzn files, including hidden/ignored files.
Discovery skips .git, visits directory symlinks once and sorts directory entries.
Overlapping inputs use the first path to each file.
.dzn paths use assignment-only data syntax; other paths use model syntax.

--rules default|thesis|all or comma-separated rule IDs selects exactly that set.
Omitted --rules selects default; repeated --rules and unknown IDs are errors.
The thesis preset selects its fourteen catalogue rules; all includes the defaults.
global-variable-in-function and element-predicate are implemented; the other twelve thesis rules
remain unavailable and selecting any of them reports an error.
-I DIR adds an ordered include directory for rules requiring model analysis.
--stdlib-dir DIR supplies the MiniZinc library root, overriding MZN_STDLIB_DIR.
Current default rules do not load includes or the standard library.
global-variable-in-function advises passing captured global decisions as arguments.
It resolves lexical bindings and declared instantiation, without full type checking.
element-predicate recommends indexing equality for a resolved three-argument
standard predicate. Callable facts retain aliases, enums, optional and structured
types, parameter names/defaults and supported polymorphic coercions. Ambiguous,
unresolved or unsupported calls report limitations; no full type checking occurs.
Stdin has no model context: selected semantic analysis reports a limitation;
standalone .dzn inputs mark semantic rules inapplicable.

Default rules (both warnings):
    naming: snake_case for values, callable names, parameters, fields and bindings;
            UpperCamelCase for enum/type/constructor names and explicit parameter
            sets/domains. Decision sets use snake_case. No SHOUTY_CASE.
    missing-constraint-label: recommend a direct constraint-header string label.
            Nested strings and expression annotations do not count as labels.
Naming skips references, alias-dependent roles and data assignment targets.
Quoted names and anonymous '_' are exempt; a single leading underscore is
allowed, with the remaining spelling checked normally.
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
Exit codes: 0 clean, 1 unsuppressed warnings, 2 input/parse/directive/usage/dependency errors.
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
    let Arguments {
        paths,
        stdin_filepath,
        options,
        model,
    } = parse_arguments(
        arguments,
        std::env::var_os("MZN_STDLIB_DIR").map(PathBuf::from),
    )?;
    options
        .check_available()
        .map_err(|message| format!("zincite-lint: {message}"))?;
    if paths.is_empty() {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("<stdin>: {error}"))?;
        let label = stdin_filepath.as_ref().map_or_else(
            || "<stdin>".into(),
            |path| path.to_string_lossy().into_owned(),
        );
        let mode = stdin_filepath.map_or(FileMode::Model, FileMode::from_path);
        return check_source(&label, bytes, mode, &options);
    }
    let inputs = zincite_syntax::inputs::discover_inputs(&paths);
    let mut status = if inputs.errors.is_empty() { 0 } else { 2 };
    for (path, error) in inputs.errors {
        let _ = writeln!(io::stderr(), "{}: {error}", path.display());
    }
    for path in inputs.files {
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

struct Arguments {
    paths: Vec<PathBuf>,
    stdin_filepath: Option<PathBuf>,
    options: LintOptions,
    model: ModelOptions,
}

fn parse_arguments(
    arguments: Vec<std::ffi::OsString>,
    stdlib_fallback: Option<PathBuf>,
) -> Result<Arguments, String> {
    let mut paths = Vec::new();
    let mut stdin_filepath = None;
    let mut options = None;
    let mut model = ModelOptions::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--rules" {
            if options.is_some() {
                return Err("zincite-lint: repeated --rules".into());
            }
            let selection = arguments
                .next()
                .ok_or("zincite-lint: --rules requires a selection")?;
            let selection = selection
                .to_str()
                .ok_or("zincite-lint: rule selection must be UTF-8")?;
            options = Some(
                LintOptions::from_selection(selection)
                    .map_err(|message| format!("zincite-lint: {message}"))?,
            );
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
    model.stdlib_dir = model.stdlib_dir.or(stdlib_fallback);
    Ok(Arguments {
        paths,
        stdin_filepath,
        options: options.unwrap_or_default(),
        model,
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
