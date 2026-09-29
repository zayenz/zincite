use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::process::ExitCode;

use zincite_syntax::{Diagnostic, parse};

const HELP: &str = "Usage: zincite-fmt [FILE|-]

Format one UTF-8 scalar MiniZinc model to stdout. No input or '-' reads stdin.
Supports scalar declarations/assignments, atoms, constraints with optional string
labels, and solve satisfy. Other syntax is diagnosed as unsupported.

Options:
    -h, --help    Show this help
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
    if arguments.len() > 1 {
        return Err("zincite-fmt: expected one file or stdin; use --help for usage".into());
    }
    let input = arguments.first();
    if input.is_some_and(|arg| arg != "-" && arg.to_string_lossy().starts_with('-')) {
        return Err("zincite-fmt: unsupported option; use --help for usage".into());
    }
    let (label, bytes) = if let Some(path) = input.filter(|arg| *arg != "-") {
        let label = path.to_string_lossy().into_owned();
        let bytes = fs::read(path).map_err(|error| format!("{label}: {error}"))?;
        (label, bytes)
    } else {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("<stdin>: {error}"))?;
        ("<stdin>".to_owned(), bytes)
    };
    let source = String::from_utf8(bytes).map_err(|error| {
        let offset = error.utf8_error().valid_up_to();
        format!("{label}: input is not UTF-8 at byte {offset}")
    })?;
    let parsed = parse(source);
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
