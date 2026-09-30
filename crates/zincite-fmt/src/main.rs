use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use zincite_syntax::{Diagnostic, FileMode, parse_with_mode};

const HELP: &str = "Usage: zincite-fmt [--check|--write] [--stdin-filepath PATH] [FILE...|-]

Format UTF-8 MiniZinc model or data files. No input or '-' reads stdin.
Default: write one complete formatted input to stdout.
Multiple files require --check or --write; stdin cannot be mixed with files.
.dzn paths select assignment-only data syntax; other paths select model syntax.
Parsing checks syntax only; it does not resolve includes, names or types.

Options:
    --check                 Write nothing; exit 1 if any input would change
    --write                 Replace explicit files, retaining their permissions
                            Refuses symlinks; failing files remain unchanged
    --stdin-filepath PATH    Select stdin language mode and diagnostic path
    -h, --help               Show this help

Exit codes: 0 success, 1 check found changes, 2 input/usage/I/O errors.
Errors take precedence; independent files are processed even if another fails.
";

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Stdout,
    Check,
    Write,
}

struct Arguments {
    output: OutputMode,
    paths: Vec<PathBuf>,
    stdin_filepath: Option<PathBuf>,
}

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
    let arguments: Vec<_> = env::args_os().skip(1).collect();
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        io::stdout()
            .write_all(HELP.as_bytes())
            .map_err(|error| format!("stdout: {error}"))?;
        return Ok(0);
    }
    let arguments = parse_arguments(arguments)?;
    if arguments.paths.is_empty() {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("<stdin>: {error}"))?;
        let mode = arguments
            .stdin_filepath
            .as_ref()
            .map_or(FileMode::Model, FileMode::from_path);
        let label = arguments.stdin_filepath.map_or_else(
            || "<stdin>".to_owned(),
            |path| path.to_string_lossy().into_owned(),
        );
        let (formatted, changed) = format_source(&label, bytes, mode)?;
        if arguments.output == OutputMode::Stdout {
            io::stdout()
                .write_all(formatted.as_bytes())
                .map_err(|error| format!("stdout: {error}"))?;
        }
        return Ok(u8::from(arguments.output == OutputMode::Check && changed));
    }
    let mut status = 0;
    for path in arguments.paths {
        match process_file(&path, arguments.output) {
            Ok(changed) if arguments.output == OutputMode::Check && changed => {
                status = status.max(1)
            }
            Ok(_) => {}
            Err(message) => {
                let _ = writeln!(io::stderr(), "{message}");
                status = 2;
            }
        }
    }
    Ok(status)
}

fn parse_arguments(arguments: Vec<std::ffi::OsString>) -> Result<Arguments, String> {
    let mut output = OutputMode::Stdout;
    let mut inputs = Vec::new();
    let mut stdin_filepath = None;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if argument == "--check" || argument == "--write" {
            if output != OutputMode::Stdout {
                return Err(
                    "zincite-fmt: --check and --write are mutually exclusive; choose one mode once"
                        .into(),
                );
            }
            output = if argument == "--check" {
                OutputMode::Check
            } else {
                OutputMode::Write
            };
        } else if argument == "--stdin-filepath" {
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
            inputs.push(PathBuf::from(argument));
        }
    }
    let explicit_stdin = inputs.iter().any(|path| path == std::path::Path::new("-"));
    if explicit_stdin && inputs.len() > 1 {
        return Err("zincite-fmt: stdin cannot be mixed with other inputs".into());
    }
    if explicit_stdin {
        inputs.clear();
    }
    if output == OutputMode::Stdout && inputs.len() > 1 {
        return Err("zincite-fmt: multiple files require --check or --write".into());
    }
    if output == OutputMode::Write && inputs.is_empty() {
        return Err("zincite-fmt: --write requires explicit file paths".into());
    }
    if stdin_filepath.is_some() && !inputs.is_empty() {
        return Err("zincite-fmt: --stdin-filepath requires stdin input".into());
    }
    Ok(Arguments {
        output,
        paths: inputs,
        stdin_filepath,
    })
}

fn process_file(path: &std::path::Path, output: OutputMode) -> Result<bool, String> {
    let label = path.to_string_lossy();
    let permissions = if output == OutputMode::Write {
        let metadata = fs::symlink_metadata(path).map_err(|error| format!("{label}: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{label}: refusing --write through a symlink"));
        }
        Some(metadata.permissions())
    } else {
        None
    };
    let bytes = fs::read(path).map_err(|error| format!("{label}: {error}"))?;
    let (formatted, changed) = format_source(&label, bytes, FileMode::from_path(path))?;
    match output {
        OutputMode::Stdout => io::stdout()
            .write_all(formatted.as_bytes())
            .map_err(|error| format!("stdout: {error}"))?,
        OutputMode::Write if changed => {
            replace_file(path, formatted.as_bytes(), permissions.unwrap())
                .map_err(|error| format!("{label}: {error}"))?
        }
        OutputMode::Check | OutputMode::Write => {}
    }
    Ok(changed)
}

fn format_source(label: &str, bytes: Vec<u8>, mode: FileMode) -> Result<(String, bool), String> {
    let source = String::from_utf8(bytes).map_err(|error| {
        let offset = error.utf8_error().valid_up_to();
        format!("{label}: input is not UTF-8 at byte {offset}")
    })?;
    let parsed = parse_with_mode(source, mode);
    let formatted = zincite_fmt::format(&parsed).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| render_diagnostic(label, parsed.source(), diagnostic))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let changed = formatted != parsed.source();
    Ok((formatted, changed))
}

fn replace_file(
    path: &std::path::Path,
    bytes: &[u8],
    permissions: fs::Permissions,
) -> io::Result<()> {
    // A sibling temporary keeps replacement on the same filesystem and leaves
    // the original untouched until all output and permissions are ready.
    let mut attempt = 0;
    let (temporary, mut file) = loop {
        let temporary =
            path.with_file_name(format!(".zincite-fmt-{}-{attempt}.tmp", std::process::id()));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => attempt += 1,
            Err(error) => return Err(error),
        }
    };
    let result = (|| {
        file.write_all(bytes)?;
        file.set_permissions(permissions)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
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
