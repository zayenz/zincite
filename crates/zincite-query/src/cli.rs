use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zincite_syntax::{FileMode, byte_line_column};

use crate::{ErrorLocation, Input, Limits, Query, QueryError};

const HELP: &str = "Usage: zincite-query [--stdin-filepath PATH] QUERY [FILE|-]

Select MiniZinc model items or data assignments in written order.
No input or '-' reads stdin; .dzn paths select assignment-only data syntax.
Syntax queries do not load includes or require a MiniZinc library or solver.
QUERY is one fixed pipeline separated by '|'; an empty query selects all items.

Stages:
    items                   Select all top-level items (initial selection)
    filter(PREDICATE)       Keep matching items; not > and > or, with parentheses
    head(N)                 Keep the first N items of the computed selection
    count                   Emit a decimal count and LF; terminal stage
    emit                    Emit original selected source; terminal stage
Source emission is implicit when count/emit is omitted.
Predicates: kind(\"assignment\"), name(\"capacity\"); names compare identifier identity.
Kinds: assignment, declaration, enum, type_alias, function, predicate, test,
       annotation, constraint, include, output, solve.
Untouched items emit the whole document; filter/head emit item fragments with
attached comments. Separated section comments and a BOM stay in document output.
Fragment selections that unbalance paired formatter markers fail.

Options:
    --stdin-filepath PATH   Select stdin language mode and diagnostic path
    --                      End option processing
    -h, --help              Show this help

Limits: 1048576 query bytes, 64 nested parentheses/not operators,
        100000 collected items and 1000000 evaluation visits.
Exit codes: 0 success, 2 query/input/usage/I/O errors.
Query, input and usage errors leave stdout empty.
";

struct Arguments {
    query: String,
    path: Option<PathBuf>,
    stdin_filepath: Option<PathBuf>,
}

/// Shared entry point for both command spellings. Arguments exclude the binary
/// or subcommand name. The complete result is buffered before writing stdout.
pub fn run(arguments: Vec<OsString>, invocation: &str) -> ExitCode {
    match execute(arguments, invocation) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::from(2)
        }
    }
}

fn execute(arguments: Vec<OsString>, invocation: &str) -> Result<(), String> {
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        return io::stdout()
            .write_all(HELP.replacen("zincite-query", invocation, 1).as_bytes())
            .map_err(|error| format!("stdout: {error}"));
    }
    let arguments =
        parse_arguments(arguments).map_err(|message| format!("{invocation}: {message}"))?;
    let limits = Limits::default();
    let query = Query::parse(&arguments.query, limits)
        .map_err(|error| query_diagnostic(&arguments.query, &error))?;
    let path = arguments
        .path
        .as_ref()
        .or(arguments.stdin_filepath.as_ref());
    let label = path.map_or_else(
        || "<stdin>".to_owned(),
        |path| path.to_string_lossy().into_owned(),
    );
    let mode = path.map_or(FileMode::Model, FileMode::from_path);
    let bytes = if let Some(path) = &arguments.path {
        if path.is_dir() {
            return Err(format!("{label}: directory inputs are not supported"));
        }
        fs::read(path).map_err(|error| format!("{label}: {error}"))?
    } else {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(|error| format!("{label}: {error}"))?;
        bytes
    };
    let input = Input::parse(bytes.clone(), mode).map_err(|error| {
        error
            .diagnostics
            .iter()
            .map(|diagnostic| located(&label, &bytes, &diagnostic.range, &diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let result = query
        .evaluate(&input, limits)
        .map_err(|error| match error.location {
            ErrorLocation::Query => query_diagnostic(&arguments.query, &error),
            ErrorLocation::Input => {
                located(&label, input.source_bytes(), &error.range, &error.message)
            }
        })?;
    io::stdout()
        .write_all(&result.render())
        .map_err(|error| format!("stdout: {error}"))
}

fn parse_arguments(arguments: Vec<OsString>) -> Result<Arguments, String> {
    let mut positional = Vec::new();
    let mut stdin_filepath = None;
    let mut options = true;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if options && argument == "--" {
            options = false;
        } else if options && argument == "--stdin-filepath" {
            if stdin_filepath.is_some() {
                return Err("repeated --stdin-filepath".into());
            }
            stdin_filepath = Some(PathBuf::from(
                arguments.next().ok_or("--stdin-filepath requires a path")?,
            ));
        } else {
            if options && argument != "-" && argument.to_string_lossy().starts_with('-') {
                return Err("unsupported option; use --help for usage".into());
            }
            positional.push(argument);
        }
    }
    if positional.is_empty() || positional.len() > 2 {
        return Err("expected one query and at most one input file or '-'".into());
    }
    let path = if positional.len() == 2 {
        positional
            .pop()
            .map(PathBuf::from)
            .filter(|path| path != Path::new("-"))
    } else {
        None
    };
    let query = positional.pop().ok_or("expected a query argument")?;
    let query = query
        .into_string()
        .map_err(|_| "query argument must be UTF-8")?;
    if path.is_some() && stdin_filepath.is_some() {
        return Err("--stdin-filepath requires stdin input".into());
    }
    Ok(Arguments {
        query,
        path,
        stdin_filepath,
    })
}

fn query_diagnostic(query: &str, error: &QueryError) -> String {
    located("<query>", query.as_bytes(), &error.range, &error.message)
}

fn located(label: &str, bytes: &[u8], range: &std::ops::Range<usize>, message: &str) -> String {
    let (line, column) = byte_line_column(bytes, range.start);
    format!(
        "{label}:{line}:{column}: error: {message} (bytes {}..{})",
        range.start, range.end
    )
}
