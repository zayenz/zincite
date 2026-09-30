use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use zincite_syntax::{Diagnostic, FileMode, parse_with_mode};

const HELP: &str = "Usage: zincite-fmt [--stdin-filepath PATH] [FILE|-]

Format one UTF-8 MiniZinc model or data file to stdout. No input or '-' reads stdin.
.dzn paths select assignment-only data syntax; other paths select model syntax.
Parsing checks syntax only; it does not resolve includes, names or types.

Options:
    --stdin-filepath PATH    Select stdin language mode and diagnostic path
    -h, --help               Show this help
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<(), String> {
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        return io::stdout()
            .write_all(HELP.as_bytes())
            .map_err(|error| format!("stdout: {error}"));
    }
    let mut input = None;
    let mut stdin_filepath = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--stdin-filepath" {
            if stdin_filepath.is_some() {
                return Err("zincite-fmt: repeated --stdin-filepath".into());
            }
            stdin_filepath = Some(PathBuf::from(
                arguments
                    .next()
                    .ok_or("zincite-fmt: --stdin-filepath requires a path")?,
            ));
        } else {
            if argument != "-" && argument.to_string_lossy().starts_with('-') {
                return Err("zincite-fmt: unsupported option; use --help for usage".into());
            }
            if input.replace(argument).is_some() {
                return Err("zincite-fmt: expected one file or stdin; use --help for usage".into());
            }
        }
    }
    let (label, bytes, mode) = if let Some(path) = input.filter(|arg| arg != "-") {
        if stdin_filepath.is_some() {
            return Err("zincite-fmt: --stdin-filepath requires stdin input".into());
        }
        let label = path.to_string_lossy().into_owned();
        let bytes = fs::read(&path).map_err(|error| format!("{label}: {error}"))?;
        (label, bytes, FileMode::from_path(path))
    } else {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("<stdin>: {error}"))?;
        let mode = stdin_filepath
            .as_ref()
            .map_or(FileMode::Model, FileMode::from_path);
        let label = stdin_filepath.map_or_else(
            || "<stdin>".to_owned(),
            |path| path.to_string_lossy().into_owned(),
        );
        (label, bytes, mode)
    };
    let source = String::from_utf8(bytes).map_err(|error| {
        let offset = error.utf8_error().valid_up_to();
        format!("{label}: input is not UTF-8 at byte {offset}")
    })?;
    let parsed = parse_with_mode(source, mode);
    let formatted = zincite_fmt::format(&parsed).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| render_diagnostic(&label, parsed.source(), diagnostic))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    io::stdout()
        .write_all(formatted.as_bytes())
        .map_err(|error| format!("stdout: {error}"))
}

fn render_diagnostic(label: &str, source: &str, diagnostic: &Diagnostic) -> String {
    let prefix = &source[..diagnostic.range.start];
    let mut line = 1;
    let mut column = 1;
    let mut characters = prefix.chars().peekable();
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
        "{label}:{line}:{column}: bytes {}..{}: {}",
        diagnostic.range.start, diagnostic.range.end, diagnostic.message
    )
}
