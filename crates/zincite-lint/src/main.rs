use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use zincite_syntax::{FileMode, parse_with_mode};

const HELP: &str = "Usage: zincite-lint [--stdin-filepath PATH] [FILE|DIR...|-]

Report MiniZinc modelling advice on stderr without rewriting source.
No input or '-' reads stdin; stdin cannot be mixed with files.
--stdin-filepath PATH selects the stdin language mode and diagnostic path.
Directories recursively discover .mzn/.dzn files, including hidden/ignored files.
Discovery skips .git, visits directory symlinks once and sorts directory entries.
Overlapping inputs use the first path to each file.
.dzn paths use assignment-only data syntax; other paths use model syntax.

Rules (both warnings, enabled by default):
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
Unknown, malformed, misplaced or dangling suppressions are errors.

The shared parser covers MiniZinc 2.10.1 model/data syntax; reserved
variant_record and case syntax is unsupported. Includes are not loaded, and
name resolution, types, semantic validity and solver performance are not checked.
Syntax errors omit lint rules for that file. Independent files are processed.
Exit codes: 0 clean, 1 unsuppressed warnings, 2 input/parse/directive/usage errors.
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
    let (paths, stdin_filepath) = parse_arguments(arguments)?;
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
        return check_source(&label, bytes, mode);
    }
    let inputs = zincite_syntax::inputs::discover_inputs(&paths);
    let mut status = if inputs.errors.is_empty() { 0 } else { 2 };
    for (path, error) in inputs.errors {
        let _ = writeln!(io::stderr(), "{}: {error}", path.display());
    }
    for path in inputs.files {
        let label = path.to_string_lossy();
        let result = std::fs::read(&path)
            .map_err(|error| format!("{label}: {error}"))
            .and_then(|bytes| check_source(&label, bytes, FileMode::from_path(&path)));
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

fn parse_arguments(
    arguments: Vec<std::ffi::OsString>,
) -> Result<(Vec<PathBuf>, Option<PathBuf>), String> {
    let mut paths = Vec::new();
    let mut stdin_filepath = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--stdin-filepath" {
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
    Ok((paths, stdin_filepath))
}

fn check_source(label: &str, bytes: Vec<u8>, mode: FileMode) -> Result<u8, String> {
    let source = String::from_utf8(bytes).map_err(|error| {
        format!(
            "{label}: input is not UTF-8 at byte {}",
            error.utf8_error().valid_up_to()
        )
    })?;
    let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let offset = source.len() - text.len();
    let parsed = parse_with_mode(text, mode);
    match zincite_lint::lint(&parsed) {
        Ok(warnings) => {
            for warning in &warnings {
                let message = format!("warning [{}]: {}", warning.rule.id(), warning.message);
                writeln!(
                    io::stderr(),
                    "{}",
                    render_diagnostic(label, text, warning.range.clone(), offset, &message)
                )
                .map_err(|error| format!("stderr: {error}"))?;
            }
            Ok(u8::from(!warnings.is_empty()))
        }
        Err(errors) => Err(errors
            .iter()
            .map(|error| {
                render_diagnostic(
                    label,
                    text,
                    error.range.clone(),
                    offset,
                    &format!("error: {}", error.message),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")),
    }
}

fn render_diagnostic(
    label: &str,
    source: &str,
    range: std::ops::Range<usize>,
    offset: usize,
    message: &str,
) -> String {
    let mut line = 1;
    let mut column = 1;
    let mut characters = source[..range.start].chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            if characters.peek() == Some(&'\n') {
                characters.next();
            }
            line += 1;
            column = 1;
        } else if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    format!(
        "{label}:{line}:{column}: bytes {}..{}: {message}",
        range.start + offset,
        range.end + offset
    )
}
