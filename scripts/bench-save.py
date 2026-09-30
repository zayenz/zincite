#!/usr/bin/env python3
"""Fresh-process save-path benchmark; Python standard library only."""
import argparse
import hashlib
import json
import math
import platform
from pathlib import Path
import re
import statistics
import subprocess
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


def main():
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
