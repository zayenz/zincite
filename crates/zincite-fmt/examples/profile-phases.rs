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

fn phase<T>(name: &str, work: impl FnOnce() -> T) -> T {
    let live = LIVE.load(Relaxed);
    let allocations = ALLOCATIONS.load(Relaxed);
    let requested = REQUESTED.load(Relaxed);
    PEAK.store(live, Relaxed);
    let start = Instant::now();
    let result = work();
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    // Snapshot before printing: reporting allocations do not belong to the phase.
    let count = ALLOCATIONS.load(Relaxed) - allocations;
    let bytes = REQUESTED.load(Relaxed) - requested;
    let retained = LIVE.load(Relaxed).saturating_sub(live);
    let peak = PEAK.load(Relaxed).saturating_sub(live);
    println!(
        "{name}: ms={elapsed:.3} allocation_calls={count} requested_bytes={bytes} retained_delta_bytes={retained} peak_delta_bytes={peak}"
    );
    result
}

fn main() {
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
