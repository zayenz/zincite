//! Developer-only phase/allocation probe. Its counters add overhead; do not use
//! these timings as format-on-save latency or change the production allocator.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

struct CountingAllocator;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static REQUESTED: AtomicUsize = AtomicUsize::new(0);

fn allocated(bytes: usize) {
    ALLOCATIONS.fetch_add(1, Relaxed);
    REQUESTED.fetch_add(bytes, Relaxed);
    let live = LIVE.fetch_add(bytes, Relaxed) + bytes;
    PEAK.fetch_max(live, Relaxed);
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            allocated(layout.size());
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(pointer, layout) };
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let result = unsafe { System.realloc(pointer, layout, new_size) };
        if !result.is_null() {
            LIVE.fetch_sub(layout.size(), Relaxed);
            allocated(new_size);
        }
        result
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct PhaseMetrics {
    milliseconds: f64,
    allocations: usize,
    requested: usize,
    retained: usize,
    peak: usize,
}

fn measure<T>(work: impl FnOnce() -> T) -> (T, PhaseMetrics) {
    let live = LIVE.load(Relaxed);
    let allocations = ALLOCATIONS.load(Relaxed);
    let requested = REQUESTED.load(Relaxed);
    PEAK.store(live, Relaxed);
    let start = Instant::now();
    let result = work();
    let metrics = PhaseMetrics {
        milliseconds: start.elapsed().as_secs_f64() * 1000.0,
        allocations: ALLOCATIONS.load(Relaxed) - allocations,
        requested: REQUESTED.load(Relaxed) - requested,
        retained: LIVE.load(Relaxed).saturating_sub(live),
        peak: PEAK.load(Relaxed).saturating_sub(live),
    };
    (result, metrics)
}

fn phase<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let (result, metrics) = measure(work);
    println!(
        "{name}: ms={:.3} allocation_calls={} requested_bytes={} retained_delta_bytes={} peak_delta_bytes={}",
        metrics.milliseconds,
        metrics.allocations,
        metrics.requested,
        metrics.retained,
        metrics.peak
    );
    result
}

fn quoted(text: &str) -> String {
    let mut result = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\r' => result.push_str("\\r"),
            '\t' => result.push_str("\\t"),
            value if value.is_control() => result.push_str(&format!("\\u{:04x}", value as u32)),
            value => result.push(value),
        }
    }
    result.push('"');
    result
}

fn phase_json(name: &str, metrics: PhaseMetrics) -> String {
    format!(
        "{}:{{\"milliseconds\":{:.3},\"allocation_calls\":{},\"requested_bytes\":{},\"retained_delta_bytes\":{},\"peak_delta_bytes\":{}}}",
        quoted(name),
        metrics.milliseconds,
        metrics.allocations,
        metrics.requested,
        metrics.retained,
        metrics.peak
    )
}

fn analysis_json(result: &zincite_lint::AnalysisResult) -> String {
    use zincite_lint::RuleOutcome;
    let rules = result
        .rules
        .iter()
        .map(|execution| {
            let (outcome, reason) = match &execution.outcome {
                RuleOutcome::Completed => ("Completed", ""),
                RuleOutcome::Inapplicable { reason } => ("Inapplicable", reason.as_str()),
                RuleOutcome::Limited { reason } => ("Limited", reason.as_str()),
                RuleOutcome::NotRun { reason } => ("NotRun", reason.as_str()),
            };
            let findings = result
                .findings
                .iter()
                .filter(|finding| finding.rule == execution.rule)
                .count();
            format!(
                "{}:{{\"outcome\":{},\"reason\":{},\"finding_count\":{findings}}}",
                quoted(execution.rule.id()),
                quoted(outcome),
                quoted(reason)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"status\":{},\"warnings\":{},\"errors\":{},\"limitations\":{},\"rules\":{{{rules}}}}}",
        result.status(),
        result.findings.len(),
        result.errors.len(),
        result.limitations.len()
    )
}

fn root_state(data: bool, parse_errors: usize, solves: usize) -> &'static str {
    if data {
        "data"
    } else if parse_errors > 0 {
        "syntax_rejected"
    } else if solves == 0 {
        "fragment"
    } else if solves == 1 {
        "complete"
    } else {
        "multiple_solve"
    }
}

fn solve_count(parsed: &zincite_syntax::ParsedFile) -> usize {
    parsed
        .tree()
        .child_nodes()
        .filter(|node| {
            matches!(
                node.kind(),
                zincite_syntax::NodeKind::Solve
                    | zincite_syntax::NodeKind::SolveMinimize
                    | zincite_syntax::NodeKind::SolveMaximize
            )
        })
        .count()
}

fn model_json(context: &zincite_lint::ModelContext) -> String {
    use zincite_lint::SourceKind;
    let parse_errors = context
        .root_file
        .map_or(0, |id| context.files[id].parsed.diagnostics().len());
    let solves = context
        .files
        .iter()
        .filter(|file| file.kind == SourceKind::User)
        .map(|file| solve_count(&file.parsed))
        .sum();
    let state = if context.root_file.is_none() {
        "input_error"
    } else {
        root_state(false, parse_errors, solves)
    };
    let unresolved = context
        .includes
        .iter()
        .filter(|edge| edge.target.is_none())
        .count();
    let resolved = context.errors.is_empty() && unresolved == 0 && context.implicit_core.is_some();
    let files = context.files.iter().map(|file| format!("{{\"canonical_path\":{},\"bytes\":{},\"kind\":{},\"implicit\":{},\"explicit\":{}}}", quoted(&file.canonical_path.to_string_lossy()), file.parsed.source().len() + file.byte_offset, quoted(if file.kind == SourceKind::User { "user" } else { "standard" }), file.implicit, file.explicit)).collect::<Vec<_>>().join(",");
    format!(
        "{{\"root_state\":{},\"root_parse_count\":{parse_errors},\"loaded_user_solve_count\":{solves},\"dependencies_resolved\":{resolved},\"include_edges\":{},\"unresolved_include_edges\":{unresolved},\"implicit_core_loaded\":{},\"load_errors\":{},\"load_limitations\":{},\"loaded_files\":[{files}]}}",
        quoted(state),
        context.includes.len(),
        context.implicit_core.is_some(),
        context.errors.len(),
        context.limitations.len()
    )
}

// Follow the native lint route, including model loading for rejected .mzn roots.
// Reporting runs outside phase snapshots and no context survives this function.
fn observe_root(
    path: &std::path::Path,
    options: &zincite_lint::LintOptions,
    model: &zincite_lint::ModelOptions,
) -> std::io::Result<u8> {
    use std::io::Write;
    use zincite_lint::{analyze_file, analyze_model, load_model, write_analysis};
    use zincite_syntax::{FileMode, parse_with_mode};
    let mode = FileMode::from_path(path);
    let label = path.to_string_lossy();
    let (row, status) = if options.requires_model() && mode == FileMode::Model {
        let (context, load) = measure(|| load_model(path, model));
        let (analysis, analyze) = measure(|| analyze_model(&context, options));
        let (status, render) = measure(|| write_analysis(&analysis, &mut std::io::stderr()));
        let row = format!(
            "{{\"kind\":\"root\",\"path\":{},\"source\":{},\"analysis\":{},\"phases\":{{{},{},{}}}}}",
            quoted(&label),
            model_json(&context),
            analysis_json(&analysis),
            phase_json("load", load),
            phase_json("analyze", analyze),
            phase_json("render", render)
        );
        (row, status?)
    } else {
        let (input, load) = measure(|| {
            let bytes = std::fs::read(path).map_err(|error| format!("{label}: {error}"))?;
            let source = String::from_utf8(bytes).map_err(|error| {
                format!(
                    "{label}: input is not UTF-8 at byte {}",
                    error.utf8_error().valid_up_to()
                )
            })?;
            let text = source.strip_prefix('\u{feff}').unwrap_or(&source);
            let offset = source.len() - text.len();
            let parsed = parse_with_mode(text, mode);
            Ok::<_, String>((source, parsed, offset))
        });
        match input {
            Ok((source, parsed, offset)) => {
                let (analysis, analyze) = measure(|| analyze_file(&parsed, path, offset, options));
                let (status, render) =
                    measure(|| write_analysis(&analysis, &mut std::io::stderr()));
                let metadata = format!(
                    "{{\"root_state\":{},\"root_parse_count\":{},\"loaded_user_solve_count\":{},\"dependencies_resolved\":null,\"loaded_files\":[],\"source_bytes\":{},\"bom_offset\":{offset}}}",
                    quoted(root_state(
                        mode == FileMode::Data,
                        parsed.diagnostics().len(),
                        solve_count(&parsed)
                    )),
                    parsed.diagnostics().len(),
                    solve_count(&parsed),
                    source.len()
                );
                let row = format!(
                    "{{\"kind\":\"root\",\"path\":{},\"source\":{metadata},\"analysis\":{},\"phases\":{{{},{},{}}}}}",
                    quoted(&label),
                    analysis_json(&analysis),
                    phase_json("read_parse", load),
                    phase_json("analyze", analyze),
                    phase_json("render", render)
                );
                (row, status?)
            }
            Err(message) => {
                writeln!(std::io::stderr(), "{message}")?;
                let rules = options.rules.iter().map(|rule| format!("{}:{{\"outcome\":\"Unobserved\",\"reason\":{},\"finding_count\":null}}", quoted(rule.id()), quoted(&message))).collect::<Vec<_>>().join(",");
                let row = format!(
                    "{{\"kind\":\"root\",\"path\":{},\"source\":{{\"root_state\":\"input_error\",\"loaded_files\":[]}},\"analysis\":{{\"status\":2,\"warnings\":0,\"errors\":1,\"limitations\":0,\"rules\":{{{rules}}}}},\"phases\":{{{}}}}}",
                    quoted(&label),
                    phase_json("read_parse", load)
                );
                (row, 2)
            }
        }
    };
    println!("{row}");
    std::io::stdout().flush()?;
    drop(row);
    Ok(status)
}

fn batch() -> Result<u8, String> {
    use std::io::Write;
    use std::path::PathBuf;
    let mut arguments = std::env::args_os().skip(2);
    let mut selection = None;
    let mut model = zincite_lint::ModelOptions::default();
    let mut paths = Vec::new();
    let mut manifest_only = false;
    while let Some(argument) = arguments.next() {
        if argument == "--rules" {
            if selection.is_some() {
                return Err("repeated --rules".into());
            }
            let value = arguments.next().ok_or("--rules requires a selection")?;
            let value = value.to_str().ok_or("selection must be UTF-8")?;
            if !matches!(value, "default" | "thesis" | "all") {
                return Err("batch rules must be default, thesis or all".into());
            }
            selection = Some(value.to_owned());
        } else if argument == "--stdlib-dir" {
            if model.stdlib_dir.is_some() {
                return Err("repeated --stdlib-dir".into());
            }
            model.stdlib_dir = Some(PathBuf::from(
                arguments
                    .next()
                    .ok_or("--stdlib-dir requires a directory")?,
            ));
        } else if argument == "-I" {
            model.include_dirs.push(PathBuf::from(
                arguments.next().ok_or("-I requires a directory")?,
            ));
        } else if argument == "--manifest-only" {
            manifest_only = true;
        } else if argument.to_string_lossy().starts_with('-') {
            return Err("unsupported batch option".into());
        } else {
            paths.push(PathBuf::from(argument));
        }
    }
    if paths.is_empty() {
        return Err("batch requires explicit files or directories".into());
    }
    model.stdlib_dir = model
        .stdlib_dir
        .or_else(|| std::env::var_os("MZN_STDLIB_DIR").map(PathBuf::from));
    let selection = selection.unwrap_or_else(|| "default".into());
    let options = zincite_lint::LintOptions::from_selection(&selection)?;
    options.check_available()?;
    let inputs = zincite_syntax::inputs::discover_inputs(&paths);
    let files = inputs
        .files
        .iter()
        .map(|path| {
            let canonical = path.canonicalize().map_err(|error| error.to_string())?;
            let bytes = std::fs::metadata(path)
                .map_err(|error| error.to_string())?
                .len();
            Ok::<_, String>(format!(
                "{{\"path\":{},\"canonical_path\":{},\"bytes\":{bytes}}}",
                quoted(&path.to_string_lossy()),
                quoted(&canonical.to_string_lossy())
            ))
        })
        .collect::<Result<Vec<_>, _>>()?
        .join(",");
    let errors = inputs
        .errors
        .iter()
        .map(|(path, error)| {
            format!(
                "{{\"path\":{},\"message\":{}}}",
                quoted(&path.to_string_lossy()),
                quoted(&error.to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let rules = options
        .rules
        .iter()
        .map(|rule| quoted(rule.id()))
        .collect::<Vec<_>>()
        .join(",");
    let presets = ["default", "thesis", "all"]
        .iter()
        .map(|selection| {
            let rules = zincite_lint::LintOptions::from_selection(selection)
                .expect("built-in preset")
                .rules
                .iter()
                .map(|rule| quoted(rule.id()))
                .collect::<Vec<_>>()
                .join(",");
            format!("{}:[{rules}]", quoted(selection))
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"kind\":\"manifest\",\"selection\":{},\"rules\":[{rules}],\"preset_rules\":{{{presets}}},\"positional_count\":{},\"files\":[{files}],\"discovery_errors\":[{errors}]}}",
        quoted(&selection),
        paths.len()
    );
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    drop(files);
    drop(errors);
    drop(rules);
    drop(presets);
    let mut status = if inputs.errors.is_empty() { 0 } else { 2 };
    for (path, error) in &inputs.errors {
        eprintln!("{}: {error}", path.display());
    }
    if manifest_only {
        return Ok(status);
    }
    // Initialize the process-wide stderr buffer before the per-root baseline.
    std::io::stderr()
        .write_all(&[])
        .map_err(|error| error.to_string())?;
    let baseline = LIVE.load(Relaxed);
    for path in &inputs.files {
        match observe_root(path, &options, &model) {
            Ok(root_status) => status = status.max(root_status),
            Err(error) => {
                eprintln!("stderr: {error}");
                status = 2;
            }
        }
        let live = LIVE.load(Relaxed);
        println!(
            "{{\"kind\":\"drop\",\"path\":{},\"baseline_live_bytes\":{baseline},\"after_drop_live_bytes\":{live}}}",
            quoted(&path.to_string_lossy())
        );
        std::io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
    }
    println!(
        "{{\"kind\":\"complete\",\"status\":{status},\"roots\":{}}}",
        inputs.files.len()
    );
    Ok(status)
}

fn main() {
    if std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "batch")
    {
        let status = batch().unwrap_or_else(|message| {
            eprintln!("profile-phases: {message}");
            2
        });
        std::process::exit(i32::from(status));
    }
    let mut arguments = std::env::args().skip(1);
    let path = arguments
        .next()
        .expect("usage: profile-phases FILE [all|lex|parse|format|drop] [seconds]");
    let selected = arguments.next().unwrap_or_else(|| "all".into());
    let seconds: f64 = arguments
        .next()
        .map_or(0.0, |value| value.parse().expect("seconds must be numeric"));
    assert!(matches!(
        selected.as_str(),
        "all" | "lex" | "parse" | "format" | "drop"
    ));
    assert!(seconds.is_finite() && seconds >= 0.0);
    let source = std::fs::read_to_string(&path).expect("read UTF-8 input");
    let mode = zincite_syntax::FileMode::from_path(&path);
    println!(
        "input_bytes={} phase={selected} repeat_seconds={seconds}",
        source.len()
    );
    if selected == "drop" {
        let baseline = LIVE.load(Relaxed);
        for iteration in 1..=3 {
            let parsed = zincite_syntax::parse_with_mode(source.clone(), mode);
            assert!(parsed.diagnostics().is_empty());
            let output = zincite_fmt::format(&parsed).expect("format input");
            // Formatting borrows syntax; its caller can still inspect and lint it.
            assert_eq!(parsed.source(), source);
            let warnings = zincite_lint::lint(&parsed).expect("lint retained syntax");
            drop(warnings);
            drop(output);
            drop(parsed);
            let live = LIVE.load(Relaxed);
            println!(
                "iteration={iteration} baseline_live_bytes={baseline} after_drop_live_bytes={live}"
            );
        }
        return;
    }
    if selected == "all" {
        let lexed = phase("lex (including owned source copy)", || {
            zincite_syntax::lex(source.clone())
        });
        assert!(lexed.diagnostics().is_empty(), "{:?}", lexed.diagnostics());
        println!("token_count={}", lexed.tokens().len());
        drop(lexed);
        let parsed = phase("parse (including lex/source copy)", || {
            zincite_syntax::parse_with_mode(source.clone(), mode)
        });
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        let output = phase("format (CST retained)", || {
            zincite_fmt::format(&parsed).expect("format input")
        });
        println!("output_bytes={}", output.len());
        return;
    }
    // Repetition makes native CPU sampling useful; no phase report in this loop.
    // Format-only retains one parsed CST; parse-only includes lexing and drops
    // each tree. Black boxes keep all public work observable to the optimizer.
    let parsed = if selected == "format" {
        let parsed = zincite_syntax::parse_with_mode(source.clone(), mode);
        assert!(
            parsed.diagnostics().is_empty(),
            "{:?}",
            parsed.diagnostics()
        );
        Some(parsed)
    } else {
        None
    };
    let start = Instant::now();
    let mut iterations = 0;
    loop {
        match selected.as_str() {
            "lex" => {
                std::hint::black_box(zincite_syntax::lex(source.clone()));
            }
            "parse" => {
                std::hint::black_box(zincite_syntax::parse_with_mode(source.clone(), mode));
            }
            "format" => {
                std::hint::black_box(
                    zincite_fmt::format(parsed.as_ref().unwrap()).expect("format input"),
                );
            }
            _ => unreachable!(),
        }
        iterations += 1;
        if start.elapsed().as_secs_f64() >= seconds {
            break;
        }
    }
    println!(
        "iterations={iterations} wall_ms={:.3}",
        start.elapsed().as_secs_f64() * 1000.0
    );
}
