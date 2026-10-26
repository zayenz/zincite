//! In-process release measurements; scripts/bench-checker.py owns files and runs.
use std::{env, error::Error, fs, hint::black_box, path::Path, time::Instant};
use zincite_check::{Assignment, PreparedModel, prepare};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        return Err("usage: measure-checking CASE_DIRECTORY SAMPLES WARMUP BATCH".into());
    }
    let directory = Path::new(&args[1]);
    let samples: usize = args[2].parse()?;
    let warmup: usize = args[3].parse()?;
    let batch: usize = args[4].parse()?;
    if samples == 0 || batch == 0 {
        return Err("samples and batch must be positive".into());
    }
    let source = fs::read_to_string(directory.join("model.fzn"))?;
    let assignments: Vec<Assignment> =
        serde_json::from_str(&fs::read_to_string(directory.join("assignments.json"))?)?;
    if assignments.is_empty() {
        return Err("expected at least one assignment".into());
    }
    let model = prepare("model.fzn", &source).map_err(|errors| format!("{errors:?}"))?;
    let serialized = model.to_json().map_err(|error| format!("{error:?}"))?;
    let loaded = PreparedModel::from_json(&serialized).map_err(|error| format!("{error:?}"))?;
    let reports = check_fresh_reports(&model, &loaded, &serialized, &assignments)?;
    fs::write(directory.join("prepared.json"), &serialized)?;
    fs::write(
        directory.join("reports.json"),
        serde_json::to_string(&reports)?,
    )?;

    let preparation = measure_samples(samples, warmup, |_| {
        prepare("model.fzn", black_box(&source)).map_err(|errors| format!("{errors:?}"))
    })?;
    let load = measure_samples(samples, warmup, |_| {
        PreparedModel::from_json(black_box(&serialized)).map_err(|error| format!("{error:?}"))
    })?;
    let check = measure_samples(samples, warmup, |index| {
        loaded
            .check(black_box(&assignments[index % assignments.len()]))
            .map_err(|error| format!("{error:?}"))
    })?;
    let check_batch = measure_samples(samples, warmup, |index| {
        for _ in 0..batch {
            // Batched costs include disposal; individual costs stop before disposal.
            black_box(loaded.check(black_box(&assignments[index % assignments.len()])))
                .map_err(|error| format!("{error:?}"))?;
        }
        Ok(())
    })?
    .into_iter()
    .map(|ms| ms / batch as f64)
    .collect::<Vec<_>>();
    println!(
        "{}",
        serde_json::json!({
            "prepare_ms": preparation, "load_ms": load, "check_ms": check, "check_batch_ms": check_batch,
            "correctness": "prepared, fresh-loaded and reused models agree after changing/removing assignments",
            "serialized_bytes": serialized.len(),
        })
    );
    Ok(())
}

fn check_fresh_reports(
    prepared: &PreparedModel,
    reused: &PreparedModel,
    serialized: &str,
    assignments: &[Assignment],
) -> Result<Vec<serde_json::Value>, String> {
    let mut reports = Vec::new();
    for assignment in assignments {
        let fresh = PreparedModel::from_json(serialized).map_err(|error| format!("{error:?}"))?;
        let expected = serde_json::to_value(fresh.check(assignment).map_err(|e| format!("{e:?}"))?)
            .map_err(|error| error.to_string())?;
        let original =
            serde_json::to_value(prepared.check(assignment).map_err(|e| format!("{e:?}"))?)
                .map_err(|error| error.to_string())?;
        let actual = serde_json::to_value(reused.check(assignment).map_err(|e| format!("{e:?}"))?)
            .map_err(|error| error.to_string())?;
        if expected != actual || expected != original {
            return Err("reused model changed a report compared with a fresh loader".into());
        }
        reports.push(expected);
    }
    Ok(reports)
}

fn measure_samples<T>(
    samples: usize,
    warmup: usize,
    mut operation: impl FnMut(usize) -> Result<T, String>,
) -> Result<Vec<f64>, String> {
    let mut timings = Vec::with_capacity(samples);
    for index in 0..warmup + samples {
        let start = Instant::now();
        let output = black_box(operation(index))?;
        let ms = start.elapsed().as_secs_f64() * 1000.;
        drop(output);
        if index >= warmup {
            timings.push(ms);
        }
    }
    Ok(timings)
}
