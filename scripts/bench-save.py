#!/usr/bin/env python3
"""Fresh-process save-path benchmark; Python standard library only."""
import argparse
import hashlib
import json
import math
import os
import sys
import platform
from pathlib import Path
import re
import statistics
import subprocess
import signal
import threading
import tempfile
import time
from datetime import datetime, timezone

try:
    import resource
except ImportError:
    resource = None

ROOT = Path(__file__).resolve().parents[1]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def invoke(command, data, timeout, stderr_limit=4096):
    before = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
    start = time.perf_counter()
    try:
        output = subprocess.run(command, input=data, capture_output=True, timeout=timeout)
        wall_ms = (time.perf_counter() - start) * 1000
        after = resource.getrusage(resource.RUSAGE_CHILDREN) if resource else None
        return {"wall_ms": wall_ms,
                "child_cpu_ms": ((after.ru_utime + after.ru_stime) - (before.ru_utime + before.ru_stime)) * 1000 if after else None,
                "returncode": output.returncode, "timeout": False,
                "stdout_bytes": len(output.stdout), "stdout_sha256": digest(output.stdout),
                "stderr": output.stderr.decode("utf-8", errors="replace")[:stderr_limit]}, output.stdout
    except subprocess.TimeoutExpired as error:
        return {"wall_ms": (time.perf_counter() - start) * 1000,
                "returncode": None, "timeout": True,
                "stdout_bytes": len(error.stdout or b""),
                "stderr": (error.stderr or b"").decode("utf-8", errors="replace")[:4096]}, b""


def accepted(result, expected):
    return (not result["timeout"] and result["returncode"] == expected
            and (result["stdout_bytes"] > 0 if expected == 0
                 else result["stdout_bytes"] == 0 and bool(result["stderr"])))


def peak_rss(command, data, timeout):
    system = platform.system()
    if system not in ("Darwin", "Linux") or not Path("/usr/bin/time").exists():
        return {"available": False, "reason": "native /usr/bin/time probe unavailable"}
    flag = "-l" if system == "Darwin" else "-v"
    result, _ = invoke(["/usr/bin/time", flag, *command], data, timeout, stderr_limit=None)
    pattern = (r"(\d+)\s+maximum resident set size" if system == "Darwin"
               else r"Maximum resident set size \(kbytes\):\s*(\d+)")
    match = re.search(pattern, result["stderr"])
    raw = int(match[1]) if match else None
    result["stderr"] = result["stderr"][:4096]
    return {"available": raw is not None, "raw": raw,
            "unit": "bytes" if system == "Darwin" else "KiB",
            "mib": raw / (1024 ** 2 if system == "Darwin" else 1024) if raw else None,
            "measurement": result, "command": ["/usr/bin/time", flag, *command]}


def cases(external):
    result = [("ordinary-changed", (ROOT / "tests/fixtures/integration.mzn").read_bytes(), ".mzn", 0),
              ("invalid-edited", b"constraint true; int: edited = ;\n", ".mzn", 2),
              ("matrix", ("values=[|" + "|".join(",".join(str((row + col) % 100) for col in range(12))
                                                  for row in range(20)) + "|];").encode(), ".dzn", 0),
              ("nested", ("int: value=" + "if true then (" * 40 + "1" + ") else 0 endif" * 40 + ";").encode(), ".mzn", 0)]
    result.append(("matrix-grown", ("values=[|" + "|".join(",".join(str((row + col) % 100) for col in range(12))
                                                         for row in range(200)) + "|];").encode(), ".dzn", 0))
    result.append(("nested-grown", ("int: value=" + "if true then (" * 160 + "1" + ") else 0 endif" * 160 + ";").encode(), ".mzn", 0))
    for scale in (10_000, 100_000, 1_000_000, 2_000_000):
        data = ("values=[" + ",".join(str(i % 100) for i in range(scale // 3)) + "];\n").encode()
        result.append((f"dense-{scale}", data, ".dzn", 0))
    for index, path in enumerate(external):
        result.append((f"external-{index}-{path.name}", path.read_bytes(), path.suffix, 0))
    return result


def file_digest(path):
    value = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def file_invoke(command, directory, name, timeout):
    """Keep complete streams; wait4's RSS belongs to this exact child."""
    stdout = directory / (name + ".stdout")
    stderr = directory / (name + ".stderr")
    timed_out = False
    usage = None
    start = time.perf_counter()
    with stdout.open("xb") as output, stderr.open("xb") as errors:
        child = subprocess.Popen(command, stdout=output, stderr=errors, cwd=ROOT)
        if all(hasattr(os, name) for name in ("wait4", "waitid", "WNOWAIT")):
            expired = threading.Event()
            def deadline():
                # WNOWAIT leaves the PID reserved until the sole wait4 reaper runs.
                if os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
                    expired.set()
                    os.kill(child.pid, signal.SIGKILL)
            watchdog = threading.Timer(max(0, timeout - (time.perf_counter() - start)), deadline)
            watchdog.start()
            os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOWAIT)
            elapsed = time.perf_counter() - start
            watchdog.cancel()
            watchdog.join()
            _, status, usage = os.wait4(child.pid, 0)
            child.returncode = os.waitstatus_to_exitcode(status)
            timed_out = expired.is_set() and child.returncode == -signal.SIGKILL
        else:
            try:
                child.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                timed_out = True
                child.kill()
                child.wait()
            elapsed = time.perf_counter() - start
    system = platform.system()
    rss = usage.ru_maxrss if usage else None
    return {"command": command, "wall_seconds": elapsed,
            "child_cpu_seconds": usage.ru_utime + usage.ru_stime if usage else None,
            "returncode": child.returncode, "timeout": timed_out,
            "deadline_seconds": timeout,
            "peak_child_rss_raw": rss,
            "peak_child_rss_unit": "bytes" if system == "Darwin" else "KiB" if system == "Linux" else None,
            "peak_child_rss_mib": rss / (1024 ** 2 if system == "Darwin" else 1024) if rss is not None and system in ("Darwin", "Linux") else None,
            "rss_method": "per-child os.wait4 high-water value" if usage else "unavailable",
            "stdout": str(stdout), "stderr": str(stderr),
            "stdout_bytes": stdout.stat().st_size, "stderr_bytes": stderr.stat().st_size,
            "stdout_sha256": file_digest(stdout), "stderr_sha256": file_digest(stderr)}


def load_probe(path, manifest, process, selected):
    """Aggregate current outcomes; leave unfinished roots explicitly unobserved."""
    observed = set()
    totals = {"observed_roots": 0, "warnings": 0, "errors": 0, "limitations": 0,
              "structurally_complete_roots": 0, "complete_resolved_roots": 0,
              "complete_roots_all_selected_completed": 0, "repeated_loaded_files": 0,
              "repeated_loaded_bytes": 0}
    partitions = {rule: {} for rule in selected}
    states, dependencies, drops, phases = {}, {}, [], {}
    complete = None
    invalid_lines = []
    with path.open() as stream:
        for index, line in enumerate(stream, 1):
            try:
                row = json.loads(line)
            except ValueError:
                invalid_lines.append(index)
                continue
            if row["kind"] == "manifest":
                assert selected == row["rules"]
                assert row["files"] == manifest["files"]
            elif row["kind"] == "root":
                assert row["path"] not in observed
                observed.add(row["path"])
                analysis, source = row["analysis"], row["source"]
                assert set(analysis["rules"]) == set(selected)
                totals["observed_roots"] += 1
                for key in ("warnings", "errors", "limitations"):
                    totals[key] += analysis[key]
                state = source["root_state"]
                states[state] = states.get(state, 0) + 1
                structural = state == "complete"
                resolved = structural and source.get("dependencies_resolved", False)
                totals["structurally_complete_roots"] += structural
                totals["complete_resolved_roots"] += bool(resolved)
                totals["complete_roots_all_selected_completed"] += bool(resolved and all(rule["outcome"] == "Completed" for rule in analysis["rules"].values()))
                for rule, execution in analysis["rules"].items():
                    outcome = execution["outcome"]
                    partitions[rule][outcome] = partitions[rule].get(outcome, 0) + 1
                for dependency in source["loaded_files"]:
                    canonical = dependency["canonical_path"]
                    totals["repeated_loaded_files"] += 1
                    totals["repeated_loaded_bytes"] += dependency["bytes"]
                    if canonical in dependencies:
                        assert dependencies[canonical]["bytes"] == dependency["bytes"]
                    else:
                        dependencies[canonical] = dependency
                for phase, values in row["phases"].items():
                    entry = phases.setdefault(phase, {key: 0 for key in values})
                    for key, value in values.items():
                        entry[key] = max(entry[key], value) if key in ("peak_delta_bytes", "retained_delta_bytes") else entry[key] + value
            elif row["kind"] == "drop":
                drops.append({"path": row["path"], "baseline": row["baseline_live_bytes"], "after_drop": row["after_drop_live_bytes"]})
            elif row["kind"] == "complete":
                complete = row
    expected = {row["path"] for row in manifest["files"]}
    assert observed <= expected
    reason = "probe deadline" if process["timeout"] else "probe did not report this root"
    unobserved = [{"path": row["path"], "outcome": "Unobserved", "reason": reason} for row in manifest["files"] if row["path"] not in observed]
    for counts in partitions.values():
        counts["Unobserved"] = counts.get("Unobserved", 0) + len(unobserved)
        assert sum(counts.values()) == len(expected)
    return {"process": process, "selected_rules": selected, "totals": totals,
            "rule_outcome_partitions": partitions, "root_states": states,
            "unobserved_roots": unobserved, "unobserved_applies_to_all_selected_rules": True,
            "invalid_json_lines": invalid_lines, "complete_record": complete,
            "dependency_union": list(dependencies.values()),
            "unique_loaded_files": len(dependencies),
            "unique_loaded_bytes": sum(row["bytes"] for row in dependencies.values()),
            "phase_attribution": phases, "drop_snapshots": drops,
            "all_reported_scopes_return_to_baseline": all(row["baseline"] == row["after_drop"] for row in drops)}


def batch_main(arguments):
    parser = argparse.ArgumentParser(description="Native sequential batch timing with current structured coverage; no source writes.")
    parser.add_argument("paths", type=Path, nargs="+", help="explicit file or directory roots; canonical discovery comes from the Rust probe")
    parser.add_argument("--formatter", type=Path, default=ROOT / "target/release/zincite-fmt")
    parser.add_argument("--linter", type=Path, default=ROOT / "target/release/zincite-lint")
    parser.add_argument("--probe", type=Path, default=ROOT / "target/release/examples/profile-phases")
    parser.add_argument("--selection", action="append", help="format or any existing lint selector expression")
    parser.add_argument("--native-only", action="store_true", help="omit instrumented companion coverage, for retained native binary comparisons")
    parser.add_argument("--repeat", type=int, default=2)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--probe-timeout", type=float, default=300)
    parser.add_argument("-I", "--include-dir", type=Path, action="append", default=[])
    parser.add_argument("--stdlib-dir", type=Path)
    parser.add_argument("--output", type=Path, required=True, help="new ignored report directory outside inputs")
    args = parser.parse_args(arguments)
    if args.repeat < 1 or any(not math.isfinite(value) or value <= 0 for value in (args.timeout, args.probe_timeout)):
        parser.error("repeat and finite deadlines must be positive")
    output = args.output.resolve()
    inputs = [path.absolute() for path in args.paths]
    if output.exists():
        parser.error(f"report already exists: {output}")
    if any(output == path.resolve() or path.is_dir() and output.is_relative_to(path.resolve()) for path in inputs):
        parser.error("report output must be outside every input root")
    binaries = {name: path.resolve() for name, path in (("formatter", args.formatter), ("linter", args.linter), ("probe", args.probe))}
    if any(not path.is_file() for path in binaries.values()):
        parser.error("build or supply all three release binaries")
    output.mkdir(parents=True)
    model = []
    if args.stdlib_dir:
        model += ["--stdlib-dir", str(args.stdlib_dir.resolve())]
    for path in args.include_dir:
        model += ["-I", str(path.resolve())]
    positional = [str(path) for path in inputs]
    probe = str(binaries["probe"])
    manifest_run = file_invoke([probe, "batch", "--manifest-only", *positional], output, "manifest", args.probe_timeout)
    if manifest_run["timeout"] or manifest_run["returncode"] not in (0, 2):
        parser.error("manifest probe failed; raw evidence retained")
    with Path(manifest_run["stdout"]).open() as source:
        manifest = json.loads(source.readline())
    assert manifest["kind"] == "manifest"
    originals = {row["canonical_path"]: file_digest(Path(row["canonical_path"])) for row in manifest["files"]}
    dependency_dirs = [*args.include_dir]
    if args.stdlib_dir or os.environ.get("MZN_STDLIB_DIR"):
        dependency_dirs.append(args.stdlib_dir or Path(os.environ["MZN_STDLIB_DIR"]))
    dependency_manifest_run = None
    if dependency_dirs:
        dependency_manifest_run = file_invoke([probe, "batch", "--manifest-only", *[str(path.resolve()) for path in dependency_dirs]], output, "dependency-manifest", args.probe_timeout)
        if dependency_manifest_run["timeout"] or dependency_manifest_run["returncode"] not in (0, 2):
            parser.error("dependency manifest failed; raw evidence retained")
        with Path(dependency_manifest_run["stdout"]).open() as source:
            dependency_manifest = json.loads(source.readline())
        originals.update({row["canonical_path"]: file_digest(Path(row["canonical_path"])) for row in dependency_manifest["files"]})
    selections = args.selection or ("format", "default", "thesis", "all")
    if len(set(selections)) != len(selections):
        parser.error("repeated selection; use --repeat for trials")
    selected_rules, labels = {}, {}
    for index, selection in enumerate(selections):
        labels[selection] = f"s{index}"
        if selection == "format":
            continue
        process = file_invoke([probe, "batch", "--manifest-only", "--rules", selection, *positional], output, f"s{index}-manifest", args.probe_timeout)
        if process["timeout"] or process["returncode"] not in (0, 2):
            parser.error("selection manifest failed; raw evidence retained")
        try:
            with Path(process["stdout"]).open() as source:
                selected = json.loads(source.readline())
        except (ValueError, OSError):
            parser.error("selection manifest failed; raw evidence retained")
        if selected.get("kind") != "manifest" or selected["files"] != manifest["files"] or selected["selection"] != selection:
            parser.error("selection manifest does not match input discovery")
        selected_rules[selection] = selected["rules"]
    count = len(manifest["files"])
    input_bytes = sum(row["bytes"] for row in manifest["files"])
    report = {"revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "platform": platform.platform(), "machine": platform.machine(), "python": platform.python_version(),
              "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
              "binaries": {name: {"path": str(path), "sha256": file_digest(path)} for name, path in binaries.items()},
              "positional_paths": positional, "model_options": model, "MZN_STDLIB_DIR": os.environ.get("MZN_STDLIB_DIR"),
              "cache_label": "fresh processes; ordinary warm filesystem caches; first use is not controlled cold I/O",
              "manifest": manifest, "manifest_process": manifest_run, "dependency_manifest_process": dependency_manifest_run, "input_count": count, "input_bytes": input_bytes,
              "original_sha256": originals, "runs": [], "coverage": {},
              "selection_rules": selected_rules, "selection_labels": labels,
              "coverage_collected": not args.native_only,
              "manifest_binary": str(binaries["probe"]),
              "timed_native_binary": str(binaries["linter"])}
    report_path = output / "report.json"
    def save():
        report_path.write_text(json.dumps(report, indent=2) + "\n")
    save()
    with (output / "runs.jsonl").open("x") as log:
        for selection in selections:
            command = ([str(binaries["formatter"]), "--check", *positional] if selection == "format" else
                       [str(binaries["linter"]), "--rules", selection, *model, *positional])
            for trial in range(args.repeat):
                row = file_invoke(command, output, labels[selection] + "-" + str(trial), args.timeout)
                row.update(selection=selection, trial=trial, first_use=trial == 0,
                           discovered_input_count=count, discovered_input_bytes=input_bytes,
                           completed_root_count=count if not row["timeout"] and row["returncode"] in (0, 1, 2) else None)
                row["attempted_input_mib_per_second"] = input_bytes / 1024 ** 2 / row["wall_seconds"] if row["completed_root_count"] is not None else None
                report["runs"].append(row)
                log.write(json.dumps(row) + "\n"); log.flush(); save()
                print(f"{selection} trial={trial} wall={row['wall_seconds']:.3f}s status={row['returncode']} timeout={row['timeout']} RSS={row['peak_child_rss_mib']}MiB", flush=True)
            if selection != "format" and not args.native_only:
                process = file_invoke([probe, "batch", "--rules", selection, *model, *positional], output, labels[selection] + "-coverage", args.probe_timeout)
                report["coverage"][selection] = load_probe(Path(process["stdout"]), manifest, process, selected_rules[selection])
                save()
    changed = [path for path, value in originals.items() if file_digest(Path(path)) != value]
    report["originals_rehashed"] = len(originals)
    report["original_changes"] = changed
    report["binaries_unchanged"] = all(file_digest(path) == report["binaries"][name]["sha256"] for name, path in binaries.items())
    save()
    return int(bool(changed or not report["binaries_unchanged"] or any(row["timeout"] or row["returncode"] not in (0, 1, 2) for row in report["runs"])))


def main():
    if sys.argv[1:2] == ["batch"]:
        return batch_main(sys.argv[2:])
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, default=ROOT / "target/release/zincite-fmt")
    parser.add_argument("--samples", type=int, default=50, help="fresh processes per case/settings; below 50 is smoke-only")
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--external", type=Path, action="append", default=[], help="read-only additional explicit input")
    parser.add_argument("--case", action="append", help="run only named cases (ordinary-formatted derives from ordinary-changed)")
    parser.add_argument("--output", type=Path, default=ROOT / "target/benchmarks" / (datetime.now(timezone.utc).strftime("save-%Y%m%dT%H%M%S.json")))
    args = parser.parse_args()
    if args.samples < 1 or not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error("samples and timeout must be positive")
    if args.output.exists():
        parser.error(f"report already exists: {args.output}")
    binary = str(args.binary.resolve())
    report = {"revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "platform": platform.platform(), "machine": platform.machine(),
              "python": platform.python_version(), "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
              "binary": binary, "binary_sha256": digest(Path(binary).read_bytes()), "samples": args.samples, "timeout_seconds": args.timeout,
              "cache_label": "ordinary warm filesystem caches; first-use is not controlled cold I/O",
              "results": []}
    inputs = cases(args.external)
    failed = False
    with tempfile.TemporaryDirectory(prefix="zincite-save-bench-") as directory:
        base = Path(directory)
        default = base / "defaults"
        configured = base / "configured"
        for root in (default, configured):
            (root / "project/models").mkdir(parents=True)
            (root / ".editorconfig").write_text("root = true\n")
        (configured / ".editorconfig").write_text("root = true\n[*]\nindent_style = space\nindent_size = 4\ntab_width = 4\nend_of_line = lf\ninsert_final_newline = true\ntrim_trailing_whitespace = true\ncharset = utf-8\nmax_line_length = 120\n")
        (configured / "project/.editorconfig").write_text("[*.{mzn,dzn}]\nindent_size = 4\n")
        (configured / "project/models/.editorconfig").write_text("[*.mzn]\nmax_line_length = 120\n")
        command = [binary, "--stdin-filepath", str(default / "project/models/ordinary.mzn")]
        first, formatted = invoke(command, inputs[0][1], args.timeout)
        first["expected_status"] = 0
        first["accepted"] = accepted(first, 0)
        report["first_use"] = {"case": "ordinary-changed", "command": command, "measurement": first}
        failed |= not first["accepted"]
        if first["accepted"]:
            inputs.insert(1, ("ordinary-formatted", formatted, ".mzn", 0))
        available = {case[0] for case in inputs} | {"ordinary-formatted"}
        if args.case and not set(args.case) <= available:
            parser.error("unknown or unavailable case: " + ", ".join(set(args.case) - available))
        for name, data, suffix, expected in inputs:
            if args.case and name not in args.case:
                continue
            variants = []
            for label, root in [("default-values", default), ("nested-editorconfig", configured)]:
                command = [binary, "--stdin-filepath", str(root / "project/models" / (name + suffix))]
                variants.append({"case": name, "settings": label, "input_bytes": len(data),
                                 "input_sha256": digest(data), "expected_status": expected,
                                 "command": command, "samples": [], "_data": data})
            batch_start = time.perf_counter()
            for _ in range(args.samples):
                for variant in variants:
                    result, _ = invoke(variant["command"], data, args.timeout)
                    result["accepted"] = accepted(result, expected)
                    if name == "ordinary-formatted" and expected == 0:
                        result["accepted"] &= result.get("stdout_sha256") == digest(data)
                    failed |= not result["accepted"]
                    variant["samples"].append(result)
            batch_ms = (time.perf_counter() - batch_start) * 1000
            for variant in variants:
                del variant["_data"]
                times = sorted(sample["wall_ms"] for sample in variant["samples"])
                variant["summary"] = {"min_ms": times[0], "p50_ms": statistics.median(times),
                                      "p95_ms": times[math.ceil(len(times) * .95) - 1], "max_ms": times[-1],
                                      "failures": sum(not sample["accepted"] for sample in variant["samples"]),
                                      "timed_wall_ms": sum(times), "mib_per_second": len(data) * len(times) / (sum(times) / 1000) / 1024 ** 2}
                variant["interleaved_pair_batch_wall_ms"] = batch_ms
                variant["peak_child_rss"] = peak_rss(variant["command"], data, args.timeout)
                print(f"{name:26} {variant['settings']:20} {len(data):8} bytes  p50 {variant['summary']['p50_ms']:8.2f}  p95 {variant['summary']['p95_ms']:8.2f} ms  RSS {variant['peak_child_rss'].get('mib')} MiB  failures {variant['summary']['failures']}", flush=True)
                report["results"].append(variant)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(f"Report: {args.output}")
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
