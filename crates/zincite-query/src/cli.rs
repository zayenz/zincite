use std::ffi::OsString;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use zincite_lint::{
    ModelOptions, SourceLocation, SourceSnapshot, load_model, replace_source_file, write_fix_diff,
};
use zincite_syntax::{FileMode, byte_line_column};

use crate::{ErrorLocation, Input, Limits, Query, QueryError, QueryResult};

const HELP: &str = "Usage: zincite-query [--stdin-filepath PATH] [--model PATH] [-I DIR] [--stdlib-dir DIR] [--diff|--write] QUERY [FILE|-]

Inspect MiniZinc source or change data assignments and enum membership.
No input or '-' reads stdin; .dzn paths select assignment-only data syntax.
Syntax queries do not load includes or require a MiniZinc library or solver.
QUERY is one fixed pipeline separated by '|'; an empty query selects all items.

Stages:
    items                   Select all top-level items (initial selection)
    filter(PREDICATE)       Keep matching items; not > and > or, with parentheses
    head(N)                 Keep the first N nodes, literal values or strings
    count                   Return a count; only json may follow
    emit                    Emit original selected source; terminal stage
    expressions             Select expression nodes in each selected subtree
    children                Select immediate CST child nodes
    subtree                 Select each node and its descendants, with duplicates
    range(START,END)         Keep nodes contained in an original-input byte range
    references              Select written references inside selected regions
    declarations            Select owned declarations or proven reference targets
    uses                    Select occurrences referring to selected declarations
    transitive_references(N) Follow written declaration references for N waves
    types                   Inspect native declaration/expression type facts
    instantiations          Inspect available par/var facts and unknown reasons
    names                   Project direct identifier identities
    call_names              Project selected call heads
    annotation_names        Project selected annotation heads
    text                    Project original node text (requires UTF-8)
    unique                  Keep first occurrences of nodes or strings
    tally                   Count projected strings; only json may follow
    values                  Inspect recursive literal values with original ranges
    fields                  Select record field nodes
    elements                Select collection values; matrix cells in row order
    keys                    Select written array and matrix axis keys
    json                    Emit inspection JSON; terminal stage
    set_value(\"EXPRESSION\") Replace selected data assignment right-hand sides
    filter_elements(CMP)    Filter literal set/array/matrix elements in data mode
    remove                  Remove selected data assignments and attached comments
    reduce_enum(NAME,KEEP)   Reduce an explicit enum and supported dependent data
    emit_document           Emit the reparsed edit candidate
Source/string emission is implicit; edits implicitly emit_document.
Projections inspect selected nodes; use subtree to include descendants.
Range bounds are half-open original byte coordinates, including a BOM.
JSON includes node kind, names, file, range and exact UTF-8 text.
Node navigation preserves per-root written order and overlapping duplicates.
Counts describe written occurrences, not execution frequency.
Only emit_document may follow one transformation stage. Edits require data mode.
Comments inside a replaced expression cause a located placement error.
Element comparisons: eq(VALUE), lt(NUMBER), le(NUMBER), gt(NUMBER), ge(NUMBER).
Equality accepts numbers, Booleans, strings and explicit member(\"A\") identities.
Filtering preserves written keys and retained comments; positional lists compact.
Matrices require a common column mask; starting-key bare tails cannot be changed.
Enum retention: keep(\"A\", \"B\") selects identities in original order;
keep_first(N) keeps an original prefix. Empty retention is allowed.
reduce_enum inspects the entire data document, including after filter/head.
Explicit enum keys establish identity; positional axes require model domains.
Supported sets prune removed members; array[int] groups drop newly empty entries.
Literal group accesses use original-to-retained indices; required empty cells stay.
Remaining scalar/computed dependence makes the candidate incomplete.
values accepts head/count/json/emit; other projections continue as node streams.
Predicates: kind(\"assignment\"), name(\"capacity\"); names compare identifier identity.
Semantic predicates: type(\"int\"), instantiation(\"par\"|\"var\"),
    source_kind(\"user\"|\"standard_library\"). Unknown stays unknown under not.
Type kinds: bool, int, float, string, annotation, enum, set, array, tuple, record,
    bottom. Nested unknown types/type variables cannot match a type predicate.
Semantic streams accept filter/head/count/names/text/unique/json/emit;
    names/text yield strings and may use tally. Unrelated CST/edit stages fail.
Semantic source uses actual retained file owners and original BOM coordinates.
references/declarations/uses preserve root order and duplicates; unique uses IDs.
transitive_references starts at declarations or proven reference targets. N=0
    emits none. Expand N waves, visiting roots then file/range/kind order; cycles
    stop by declaration ID, emitted references deduplicate file/range/kind.
Traversal includes signatures, defaults, initializers and nested written regions.
Semantic queries load an explicit model-mode file as root, or use --model.
Semantic stdin requires --model and exact original root bytes, including BOM.
--stdin-filepath is a label and mode, never a model filesystem identity.
Semantic JSON is {complete, limitations, result}, including counts/empty streams.
Located unknown facts and model limitations survive every projection.
These inspect static written relationships; they do not run lint policy or solve.
Item kinds: assignment, declaration, enum, type_alias, function, predicate, test,
       annotation, constraint, include, output, solve.
Navigated node kinds use CST names in snake_case, such as call_expression.
Untouched items emit the whole document; filter/head emit item fragments with
attached comments. Separated section comments and a BOM stay in document output.
Fragment selections that unbalance paired formatter markers fail.

Options:
    --stdin-filepath PATH   Select stdin language mode and diagnostic path
    --model PATH            Retain a root for semantic queries or data reduction
    -I DIR                  Add an include directory in supplied order
    --stdlib-dir DIR        MiniZinc library root; overrides MZN_STDLIB_DIR
    --diff                  Preview an editing pipeline as a whole-file diff
    --write                 Write a complete candidate; rejects stdin/symlinks
    --                      End option processing
    -h, --help              Show this help

Limits: 1048576 query bytes, 64 nested parentheses/not operators,
        100000 collected entries and 1000000 evaluation work units.
Exit codes: 0 complete, 1 incomplete semantic report/candidate, 2 query/input/model/usage/I/O errors.
Incomplete previews emit source/diff with stderr dependencies; --write refuses them.
Query, input and usage errors leave stdout empty.
";

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutputMode {
    Source,
    Diff,
    Write,
}

struct Arguments {
    query: String,
    path: Option<PathBuf>,
    stdin_filepath: Option<PathBuf>,
    output: OutputMode,
    model_path: Option<PathBuf>,
    model_options: ModelOptions,
}

/// Shared entry point for both command spellings. Arguments exclude the binary
/// or subcommand name. The complete result is buffered before writing stdout.
pub fn run(arguments: Vec<OsString>, invocation: &str) -> ExitCode {
    match execute(arguments, invocation) {
        Ok(status) => ExitCode::from(status),
        Err(message) => {
            let _ = writeln!(io::stderr(), "{message}");
            ExitCode::from(2)
        }
    }
}

fn execute(arguments: Vec<OsString>, invocation: &str) -> Result<u8, String> {
    if arguments.len() == 1 && matches!(arguments[0].to_str(), Some("-h" | "--help")) {
        return io::stdout()
            .write_all(HELP.replacen("zincite-query", invocation, 1).as_bytes())
            .map(|()| 0)
            .map_err(|error| format!("stdout: {error}"));
    }
    let arguments =
        parse_arguments(arguments).map_err(|message| format!("{invocation}: {message}"))?;
    let limits = Limits::default();
    let query = Query::parse(&arguments.query, limits)
        .map_err(|error| query_diagnostic(&arguments.query, &error))?;
    if arguments.output != OutputMode::Source && !query.is_editing() {
        return Err(format!(
            "{invocation}: --diff and --write require an editing pipeline"
        ));
    }
    if arguments.output == OutputMode::Write && arguments.path.is_none() {
        return Err(format!(
            "{invocation}: --write requires an explicit regular file; stdin is not supported"
        ));
    }
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
        if arguments.output == OutputMode::Write {
            let metadata =
                fs::symlink_metadata(path).map_err(|error| format!("{label}: {error}"))?;
            if !metadata.file_type().is_file() {
                return Err(format!(
                    "{label}: --write requires a regular non-symlink file"
                ));
            }
        }
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
    let input = Input::parse_named(bytes.clone(), mode, label.clone()).map_err(|error| {
        error
            .diagnostics
            .iter()
            .map(|diagnostic| located(&label, &bytes, &diagnostic.range, &diagnostic.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let model_path = arguments.model_path.as_ref().or_else(|| {
        (mode == FileMode::Model)
            .then_some(arguments.path.as_ref())
            .flatten()
    });
    let model = model_path
        .filter(|_| query.requires_model_facts())
        .map(|path| load_model(path, &arguments.model_options));
    if let Some(model) = &model
        && !model.errors.is_empty()
    {
        return Err(model
            .errors
            .iter()
            .map(|error| model_diagnostic(&error.location, &error.message, "error"))
            .collect::<Vec<_>>()
            .join("\n"));
    }
    let result = query
        .evaluate_with_model(&input, model.as_ref(), limits)
        .map_err(|error| match error.location {
            ErrorLocation::Query => query_diagnostic(&arguments.query, &error),
            ErrorLocation::Input => {
                located(&label, input.source_bytes(), &error.range, &error.message)
            }
            ErrorLocation::Model(ref location) => {
                model_diagnostic(location, &error.message, "error")
            }
        })?;
    let mut status = 0;
    if let QueryResult::Reduction(reduction) = &result
        && !reduction.is_complete()
    {
        status = 1;
        for dependency in reduction.unresolved() {
            writeln!(
                io::stderr(),
                "{}",
                model_diagnostic(&dependency.location, &dependency.message, "incomplete")
            )
            .map_err(|error| format!("stderr: {error}"))?;
        }
        if arguments.output == OutputMode::Write {
            return Ok(1);
        }
    }
    if let QueryResult::Semantic(semantic) = &result
        && !semantic.is_complete()
    {
        status = 1;
        for limitation in semantic.limitations() {
            writeln!(
                io::stderr(),
                "{}",
                model_diagnostic(&limitation.location, &limitation.message, "incomplete")
            )
            .map_err(|error| format!("stderr: {error}"))?;
        }
    }
    let candidate = result.render();
    let output = match arguments.output {
        OutputMode::Source => candidate,
        OutputMode::Diff => {
            let snapshot = SourceSnapshot::new(
                path.cloned().unwrap_or_else(|| PathBuf::from("<stdin>")),
                &bytes,
            );
            let mut diff = Vec::new();
            write_fix_diff(&snapshot, &candidate, &mut diff)
                .map_err(|error| format!("diff: {error}"))?;
            diff
        }
        OutputMode::Write => {
            let snapshot = SourceSnapshot::new(arguments.path.as_ref().unwrap(), &bytes);
            replace_source_file(&snapshot, &candidate)
                .map_err(|error| format!("{label}: {error}"))?;
            return Ok(0);
        }
    };
    io::stdout()
        .write_all(&output)
        .map(|()| status)
        .map_err(|error| format!("stdout: {error}"))
}

fn parse_arguments(arguments: Vec<OsString>) -> Result<Arguments, String> {
    let mut positional = Vec::new();
    let mut stdin_filepath = None;
    let mut output = OutputMode::Source;
    let mut model_path = None;
    let mut model_options = ModelOptions {
        include_dirs: Vec::new(),
        stdlib_dir: std::env::var_os("MZN_STDLIB_DIR").map(PathBuf::from),
    };
    let mut explicit_stdlib = false;
    let mut options = true;
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        if options && argument == "--" {
            options = false;
        } else if options && matches!(argument.to_str(), Some("--diff" | "--write")) {
            if output != OutputMode::Source {
                return Err(
                    "--diff and --write are mutually exclusive and may be supplied only once"
                        .into(),
                );
            }
            output = if argument == "--diff" {
                OutputMode::Diff
            } else {
                OutputMode::Write
            };
        } else if options && argument == "--model" {
            if model_path.is_some() {
                return Err("repeated --model".into());
            }
            model_path = Some(PathBuf::from(
                arguments.next().ok_or("--model requires a path")?,
            ));
        } else if options && argument == "-I" {
            model_options.include_dirs.push(PathBuf::from(
                arguments.next().ok_or("-I requires a directory")?,
            ));
        } else if options && argument == "--stdlib-dir" {
            if explicit_stdlib {
                return Err("repeated --stdlib-dir".into());
            }
            explicit_stdlib = true;
            model_options.stdlib_dir = Some(PathBuf::from(
                arguments
                    .next()
                    .ok_or("--stdlib-dir requires a directory")?,
            ));
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
        output,
        model_path,
        model_options,
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

fn model_diagnostic(location: &SourceLocation, message: &str, level: &str) -> String {
    format!(
        "{}:{}:{}: {level}: {message} (bytes {}..{})",
        location.path.display(),
        location.line,
        location.column,
        location.range.start,
        location.range.end
    )
}
