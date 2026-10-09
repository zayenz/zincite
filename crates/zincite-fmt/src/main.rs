use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use zincite_syntax::{Diagnostic, FileMode, byte_line_column, parse_bytes_with_mode};

mod config;

const UTF8_BOM: &[u8] = b"\xef\xbb\xbf";

const HELP: &str = "Usage: zincite-fmt [--check|--write] [--stdin-filepath PATH] [FILE|DIR...|-]

Format MiniZinc model or data files. No input or '-' reads stdin.
Code and literals use UTF-8; opaque comment bytes are preserved.
Default: write one complete formatted input to stdout.
Multiple files require --check or --write; stdin cannot be mixed with files.
Directories require --check and recursively discover .mzn/.dzn files.
Discovery includes hidden/ignored files, skips .git and visits directory symlinks once.
Overlapping read-only inputs use the first path to each file; directory entries are sorted.
.dzn paths select assignment-only data syntax; other paths select model syntax.
Parsing checks syntax only; it does not resolve includes, names or types.

Options:
    --check                 Write nothing; exit 1 if any input would change
    --write                 Replace explicit files, retaining their permissions
                            Refuses symlinks; failing files remain unchanged
    --stdin-filepath PATH    Select stdin language mode and diagnostic path
    --indent-style STYLE    Override indentation: space or tab
    --indent-size N         Override indentation columns: positive integer
    --tab-width N           Override tab stops: positive integer
    --end-of-line EOL       Override editable line endings: lf, crlf or cr
    --max-line-length N     Override width: positive integer or off
    -h, --help               Show this help

Exit codes: 0 success, 1 check found changes, 2 input/usage/I/O errors.
Errors take precedence; independent files are processed even if another fails.
EditorConfig lookup uses input paths or --stdin-filepath; plain stdin uses defaults.
Supported properties: indent_style, indent_size (integer or tab), tab_width,
end_of_line, max_line_length (integer or off), insert_final_newline,
trim_trailing_whitespace and charset (utf-8 or utf-8-bom). CLI overrides win.
Protected text keeps its bytes. Formatting may change before a stable release.
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
    overrides: ec4rs::Properties,
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
        let label = arguments.stdin_filepath.as_ref().map_or_else(
            || "<stdin>".to_owned(),
            |path| path.to_string_lossy().into_owned(),
        );
        let settings = config::resolve(arguments.stdin_filepath.as_deref(), &arguments.overrides)
            .map_err(|error| format!("{label}: {error}"))?;
        let mode = arguments
            .stdin_filepath
            .as_ref()
            .map_or(FileMode::Model, FileMode::from_path);
        let (formatted, changed) = format_source(&label, bytes, mode, &settings)?;
        if arguments.output == OutputMode::Stdout {
            io::stdout()
                .write_all(&formatted)
                .map_err(|error| format!("stdout: {error}"))?;
        }
        return Ok(u8::from(arguments.output == OutputMode::Check && changed));
    }
    if arguments.output != OutputMode::Check && arguments.paths.iter().any(|path| path.is_dir()) {
        return Err("zincite-fmt: directory inputs require --check".into());
    }
    let (paths, mut status) = if arguments.output == OutputMode::Check {
        let inputs = zincite_syntax::inputs::discover_inputs(&arguments.paths);
        let status = if inputs.errors.is_empty() { 0 } else { 2 };
        for (path, error) in inputs.errors {
            let _ = writeln!(io::stderr(), "{}: {error}", path.display());
        }
        (inputs.files, status)
    } else {
        // Explicit writes must still reject every symlink, even an alias of
        // another input. Discovery's canonical-file deduplication is read-only.
        (arguments.paths, 0)
    };
    for path in paths {
        match process_file(&path, arguments.output, &arguments.overrides) {
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
    let mut overrides = ec4rs::Properties::new();
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
        } else if let Some(key) = config::override_key(&argument) {
            let value = arguments.next().ok_or_else(|| {
                format!(
                    "zincite-fmt: {} requires a value",
                    argument.to_string_lossy()
                )
            })?;
            let value = value
                .to_str()
                .ok_or("zincite-fmt: override values must be UTF-8")?;
            config::set_override(&mut overrides, key, value)
                .map_err(|error| format!("zincite-fmt: {error}"))?;
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
        overrides,
    })
}

fn process_file(
    path: &std::path::Path,
    output: OutputMode,
    overrides: &ec4rs::Properties,
) -> Result<bool, String> {
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
    let settings =
        config::resolve(Some(path), overrides).map_err(|error| format!("{label}: {error}"))?;
    let (formatted, changed) = format_source(&label, bytes, FileMode::from_path(path), &settings)?;
    match output {
        OutputMode::Stdout => io::stdout()
            .write_all(&formatted)
            .map_err(|error| format!("stdout: {error}"))?,
        OutputMode::Write if changed => replace_file(path, &formatted, permissions.unwrap())
            .map_err(|error| format!("{label}: {error}"))?,
        OutputMode::Check | OutputMode::Write => {}
    }
    Ok(changed)
}

fn format_source(
    label: &str,
    mut bytes: Vec<u8>,
    mode: FileMode,
    settings: &config::Settings,
) -> Result<(Vec<u8>, bool), String> {
    let had_bom = bytes.starts_with(UTF8_BOM);
    let bom_offset = if had_bom { UTF8_BOM.len() } else { 0 };
    if had_bom {
        drop(bytes.drain(..UTF8_BOM.len()));
    }
    // Encoding errors return no parsed source. Retain bytes only for their
    // coordinates; ordinary UTF-8 input moves directly into the parser.
    let encoding_error_source = std::str::from_utf8(&bytes).is_err().then(|| bytes.clone());
    let parsed = parse_bytes_with_mode(bytes, mode).map_err(|diagnostic| {
        let source = encoding_error_source
            .as_deref()
            .expect("UTF-8 input cannot have an encoding error");
        render_diagnostic(
            label,
            byte_line_column(source, diagnostic.range.start),
            bom_offset,
            &diagnostic,
        )
    })?;
    drop(encoding_error_source);
    let mut formatted = zincite_fmt::format_bytes_with_options(&parsed, &settings.format).map_err(
        |diagnostics| {
            diagnostics
                .iter()
                .map(|diagnostic| {
                    render_diagnostic(
                        label,
                        parsed.line_column(diagnostic.range.start),
                        bom_offset,
                        diagnostic,
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        },
    )?;
    let changed = formatted.as_slice() != parsed.source_bytes() || settings.bom != had_bom;
    if settings.bom {
        drop(formatted.splice(..0, UTF8_BOM.iter().copied()));
    }
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

fn render_diagnostic(
    label: &str,
    (line, column): (usize, usize),
    bom_offset: usize,
    diagnostic: &Diagnostic,
) -> String {
    format!(
        "{label}:{line}:{column}: bytes {}..{}: {}",
        diagnostic.range.start + bom_offset,
        diagnostic.range.end + bom_offset,
        diagnostic.message
    )
}
