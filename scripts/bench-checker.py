#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Named in-process checker runs. One writer per name; raw files stay in target/."""

import argparse
import hashlib
import json
import math
import os
import platform
import re
import shutil
import signal
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_RESULTS = ROOT / "target/checker-browser"
STATES = ("feasible", "infeasible", "omitted", "incomplete-invalid", "feasible")
METRICS = ("prepare_ms", "load_ms", "check_ms", "check_batch_ms")
BROWSER_METRICS = (
    "load_ms",
    "boundary_ms",
    "report_parse_ms",
    "assignment_json_ms",
    "edit_ms",
)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("run", "analyze", "import-browser"):
        sub = commands.add_parser(command)
        sub.add_argument("--name", required=True)
        sub.add_argument("--results-root", type=Path, default=DEFAULT_RESULTS)
        if command == "run":
            sub.add_argument("--samples", type=positive, default=60)
            sub.add_argument("--warmup", type=positive, default=10)
            sub.add_argument("--batch", type=positive, default=10)
            sub.add_argument("--copies", type=positive, nargs="+", default=[1, 10, 100])
            sub.add_argument("--timeout", type=positive, default=120)
            sub.add_argument(
                "--binary",
                type=Path,
                default=ROOT / "target/release/examples/measure-checking",
            )
            sub.add_argument("--dry-run", action="store_true")
            sub.add_argument(
                "--limit",
                type=positive,
                help="Collect at most this many unfinished native cases",
            )
        if command == "import-browser":
            sub.add_argument("path", type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", args.name):
        parser.error("name must be one safe path component")
    directory = args.results_root.resolve() / args.name
    try:
        if args.command == "run":
            collect(args, directory)
        elif args.command == "import-browser":
            import_browser(directory, args.path)
        else:
            analyze(directory)
    except (OSError, ValueError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"{error}\n")


def positive(text: str) -> int:
    number = int(text)
    if number < 1:
        raise argparse.ArgumentTypeError("must be positive")
    return number


def read_json(path: Path) -> object:
    return json.loads(path.read_text())


def save_json(path: Path, value: object) -> None:
    temporary = path.with_suffix(".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rename_variables(text: str, names: set[str], suffix: str) -> str:
    # Strings include paths/labels and must not be rewritten as variable names.
    return re.sub(
        r'"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*',
        lambda match: match[0] + suffix if match[0] in names else match[0],
        text,
    )


def workload(copies: int) -> tuple[str, list[dict]]:
    original = (ROOT / "examples/checker-compilation/model.fzn").read_text()
    names = set(
        re.findall(
            r"^(?:array.*?|var .*?):\s*([A-Za-z_][A-Za-z_0-9]*)", original, re.MULTILINE
        )
    )
    candidates = read_json(ROOT / "examples/checker-wasm/candidates.json")
    if not isinstance(candidates, list):
        raise TypeError("candidate file must contain a list")
    base = {
        case["name"]: case["assignment"]
        for case in candidates
        if case["model"] == "schedule"
    }
    base["feasible"]["entries"] = [
        entry for entry in base["feasible"]["entries"] if entry["name"] == "start"
    ]
    base["omitted"] = {"entries": []}
    assignments = []
    for state in STATES:
        entries = []
        for copy in range(copies):
            for entry in base[state]["entries"]:
                entries.append(
                    {
                        **entry,
                        "name": entry["name"] + (f"_copy{copy}" if copies > 1 else ""),
                    }
                )
        assignments.append({"entries": entries})
    if copies == 1:
        return original, assignments
    lines = [
        line
        for line in original.splitlines()
        if line and not line.startswith(("%", "solve"))
    ]
    # Replication must preserve FlatZinc's parameter/variable/constraint item order.
    parameters = [
        line for line in lines if line.startswith("array") and "of var " not in line
    ]
    variables = [line for line in lines if line.startswith("var ") or "of var " in line]
    constraints = [line for line in lines if line.startswith("constraint ")]
    if len(parameters) + len(variables) + len(constraints) != len(lines):
        raise ValueError("unexpected item in workshop template")
    source = "\n".join(
        rename_variables("\n".join(group), names, f"_copy{copy}")
        for group in (parameters, variables, constraints)
        for copy in range(copies)
    )
    return source + "\nsolve satisfy;\n", assignments


def collect(args: argparse.Namespace, directory: Path) -> None:
    if len(set(args.copies)) != len(args.copies):
        raise ValueError("copies must be unique")
    binary = args.binary.resolve()
    files = [
        Path(__file__).resolve(),
        ROOT / "crates/zincite-check/examples/measure-checking.rs",
        ROOT / "scripts/checker-bench/measure.js",
        ROOT / "scripts/checker-bench/index.html",
        ROOT / "examples/checker-compilation/model.fzn",
        ROOT / "examples/checker-wasm/candidates.json",
        binary,
        args.results_root.resolve() / "web/zincite_check.js",
        args.results_root.resolve() / "web/zincite_check_bg.wasm",
    ]
    config = {
        "method_version": 1,
        "copies": args.copies,
        "states": STATES,
        "samples": args.samples,
        "warmup": args.warmup,
        "batch": args.batch,
        "timeout_seconds": args.timeout,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "hardware": subprocess.check_output(
            ["sysctl", "-n", "machdep.cpu.brand_string", "hw.memsize"], text=True
        ).strip(),
        "macos": subprocess.check_output(["sw_vers"], text=True).strip(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "targets": {
            str(path.relative_to(ROOT)) if path.is_relative_to(ROOT) else str(path): {
                "sha256": digest(path),
                "bytes": path.stat().st_size,
            }
            for path in files
        },
    }
    # Normalize tuples exactly as JSON will store them.
    config = json.loads(json.dumps(config))
    if args.dry_run:
        print(
            json.dumps(
                {
                    "directory": str(directory),
                    "config": config,
                    "boundary": "native API timers; process launch/I/O excluded; separate whole-process peak RSS",
                    "expected_records": [f"native-{n}.json" for n in args.copies]
                    + ["browser.json"],
                },
                indent=2,
            )
        )
        return
    if (directory / "config.json").exists() and read_json(
        directory / "config.json"
    ) != config:
        raise ValueError("configuration/target changed; choose a new run name")
    directory.mkdir(parents=True, exist_ok=True)
    if not (directory / "config.json").exists():
        save_json(directory / "config.json", config)
    for name in ("index.html", "measure.js"):
        shutil.copyfile(ROOT / "scripts/checker-bench" / name, directory / name)
    collected = 0
    for copies in args.copies:
        record_path = directory / f"native-{copies}.json"
        if native_record(record_path, args.samples, copies) is not None:
            print(f"skip completed {record_path.name}")
            continue
        print(f"collect unfinished {record_path.name}", flush=True)
        record = collect_native(args, binary, directory, copies)
        save_json(record_path, record)
        print(f"{record_path.name}: {record['status']}")
        collected += 1
        if args.limit and collected >= args.limit:
            break
    print(
        f"Browser page: {directory / 'index.html'}; use existing static server, download browser.json, import-browser --name {args.name} PATH"
    )


def collect_native(
    args: argparse.Namespace, binary: Path, directory: Path, copies: int
) -> dict:
    case = directory / f"case-{copies}"
    case.mkdir(exist_ok=True)
    source, assignments = workload(copies)
    (case / "model.fzn").write_text(source)
    save_json(case / "assignments.json", assignments)
    command = [
        "/usr/bin/time",
        "-l",
        str(binary),
        str(case),
        str(args.samples),
        str(args.warmup),
        str(args.batch),
    ]
    try:
        output = invoke_native(command, args.timeout)
        (case / "stdout.txt").write_text(output.stdout)
        (case / "stderr.txt").write_text(output.stderr)
        if output.returncode != 0:
            record = {
                "status": "failure",
                "exit_code": output.returncode,
                "diagnostic": output.stderr[-4000:],
            }
        else:
            timings = json.loads(output.stdout)
            validate_metrics(timings, METRICS, args.samples)
            reports = validate_reports(read_json(case / "reports.json"), copies)
            rss = re.search(r"(\d+)\s+maximum resident set size", output.stderr)
            record = {
                "status": "success",
                "timings": timings,
                "artifacts": {
                    name: {
                        "sha256": digest(case / name),
                        "bytes": (case / name).stat().st_size,
                    }
                    for name in (
                        "model.fzn",
                        "assignments.json",
                        "prepared.json",
                        "reports.json",
                    )
                },
                "source_bytes": len(source.encode()),
                "assignment_bytes": (case / "assignments.json").stat().st_size,
                "report_bytes": [
                    len(json.dumps(report, separators=(",", ":")).encode())
                    for report in reports
                ],
                "peak_process_rss_bytes": int(rss[1]) if rss else None,
                "rss_scope": "macOS whole helper process including preparation, load, checking and report serialization",
            }
    except subprocess.TimeoutExpired as error:
        (case / "stdout.txt").write_bytes(error.stdout or b"")
        (case / "stderr.txt").write_bytes(error.stderr or b"")
        record = {"status": "timeout", "diagnostic": f"exceeded {args.timeout}s"}
    except (ValueError, OSError) as error:
        record = {"status": "failure", "diagnostic": str(error)}
    return {"copies": copies, **record}


def invoke_native(command: list[str], timeout: int) -> subprocess.CompletedProcess[str]:
    with subprocess.Popen(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    ) as child:
        try:
            stdout, stderr = child.communicate(timeout=timeout)
        except (subprocess.TimeoutExpired, KeyboardInterrupt) as error:
            os.killpg(child.pid, signal.SIGKILL)
            stdout, stderr = child.communicate()
            if isinstance(error, KeyboardInterrupt):
                raise
            raise subprocess.TimeoutExpired(
                command, timeout, stdout.encode(), stderr.encode()
            ) from None
        return subprocess.CompletedProcess(command, child.returncode, stdout, stderr)


def validate_metrics(record: object, metrics: tuple[str, ...], samples: int) -> None:
    if not isinstance(record, dict):
        raise TypeError("metrics must be an object")
    for metric in metrics:
        values = record.get(metric)
        if (
            not isinstance(values, list)
            or len(values) != samples
            or not all(
                isinstance(value, (int, float))
                and not isinstance(value, bool)
                and math.isfinite(value)
                and value >= 0
                for value in values
            )
        ):
            raise ValueError(f"invalid or unfinished {metric}")


def validate_reports(reports: object, copies: int) -> list[dict]:
    if not isinstance(reports, list) or len(reports) != len(STATES):
        raise ValueError("missing correctness reports")
    for report, (status, complete, violations) in zip(
        reports,
        [
            ("valid", True, 0),
            ("invalid", True, 3),
            ("incomplete", False, 0),
            ("invalid", False, 1),
            ("valid", True, 0),
        ],
        strict=True,
    ):
        if (
            report["status"],
            report["completeness"]["complete"],
            len(report["violations"]),
        ) != (status, complete, copies * violations):
            raise ValueError(
                "known candidate report differs from expected status/completeness/findings"
            )

    return reports


def native_record(path: Path, samples: int, copies: int) -> dict | None:
    try:
        record = read_json(path)
        if not isinstance(record, dict) or record.get("status") not in (
            "success",
            "failure",
            "timeout",
        ):
            return None
        if record.get("copies") != copies:
            return None
        if record["status"] == "success":
            if not all(
                key in record
                for key in (
                    "source_bytes",
                    "assignment_bytes",
                    "report_bytes",
                    "peak_process_rss_bytes",
                )
            ):
                return None
            if not isinstance(record["timings"], dict) or not isinstance(
                record["timings"].get("serialized_bytes"), int
            ):
                return None
            validate_metrics(record["timings"], METRICS, samples)
        elif not isinstance(record.get("diagnostic"), str):
            return None
        return record
    except (OSError, ValueError, KeyError, TypeError):
        return None


def validate_browser(record: object, config: dict) -> dict:
    if (
        not isinstance(record, dict)
        or record.get("status") != "success"
        or record.get("config") != config
    ):
        raise ValueError("browser record does not match run configuration")
    cases = record.get("cases")
    if (
        not isinstance(cases, list)
        or not all(isinstance(case, dict) for case in cases)
        or [case.get("copies") for case in cases] != config["copies"]
    ):
        raise ValueError("browser cases do not match run")
    for case in cases:
        validate_metrics(case, BROWSER_METRICS, config["samples"])
        for key in ("memory_before_bytes", "memory_after_bytes"):
            if not isinstance(case.get(key), int) or case[key] <= 0:
                raise ValueError(f"invalid {key}")
    if not isinstance(record.get("user_agent"), str):
        raise TypeError("browser user agent must be a string")
    return record


def read_config(directory: Path) -> dict:
    config = read_json(directory / "config.json")
    if (
        not isinstance(config, dict)
        or not isinstance(config.get("copies"), list)
        or not isinstance(config.get("samples"), int)
        or isinstance(config["samples"], bool)
        or config["samples"] < 1
        or not all(
            isinstance(copies, int) and not isinstance(copies, bool) and copies > 0
            for copies in config["copies"]
        )
    ):
        raise TypeError("invalid saved configuration")
    return config


def import_browser(directory: Path, path: Path) -> None:
    config = read_config(directory)
    record = validate_browser(read_json(path), config)
    destination = directory / "browser.json"
    if destination.exists():
        if read_json(destination) != record:
            raise ValueError(
                "browser record already exists; choose a new name for recollection"
            )
        print("skip completed browser.json")
        return
    save_json(destination, record)
    print(f"imported {destination}")


def summary(values: list[float]) -> dict:
    ordered = sorted(values)
    return {
        "n": len(values),
        "p50_ms": ordered[math.ceil(0.5 * len(values)) - 1],
        "p95_ms": ordered[math.ceil(0.95 * len(values)) - 1],
        "min_ms": ordered[0],
        "max_ms": ordered[-1],
    }


def analyze(directory: Path) -> None:
    config = read_config(directory)
    completed = failed = unfinished = 0
    for copies in config["copies"]:
        record = native_record(
            directory / f"native-{copies}.json", config["samples"], copies
        )
        if record is None:
            unfinished += 1
            print(f"native-{copies}: unfinished (missing/malformed record)")
            continue
        completed += 1
        if record["status"] != "success":
            failed += 1
            print(f"native-{copies}: {record['status']}: {record.get('diagnostic')}")
            continue
        print(
            f"native-{copies}: RSS={record['peak_process_rss_bytes']} bytes; source={record['source_bytes']} serialized={record['timings']['serialized_bytes']}"
        )
        for metric in METRICS:
            print(f"  {metric}: {json.dumps(summary(record['timings'][metric]))}")
    try:
        browser = validate_browser(read_json(directory / "browser.json"), config)
        completed += 1
        print(f"browser: {browser['user_agent']}")
        for case in browser["cases"]:
            print(
                f"browser-{case['copies']}: linear memory={case['memory_before_bytes']} -> {case['memory_after_bytes']} bytes"
            )
            for metric in BROWSER_METRICS:
                print(f"  {metric}: {json.dumps(summary(case[metric]))}")
    except (OSError, ValueError, KeyError, TypeError) as error:
        unfinished += 1
        print(f"browser: unfinished ({error})")
    print(
        f"expected={len(config['copies']) + 1} completed={completed} failed={failed} unfinished={unfinished}"
    )


if __name__ == "__main__":
    main()
