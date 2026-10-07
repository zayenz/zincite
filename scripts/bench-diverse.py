#!/usr/bin/env python3
"""Run bench-save batch with model directories interleaved.

Usage: python3 scripts/bench-diverse.py ROOT... -- BENCHMARK_OPTIONS...
The options after -- are the existing bench-save.py batch options.
Each directory contributes one file per round, with smaller .mzn files first.
All discovered files remain in the sweep. Native and companion runs receive
the same explicit order; trailing roots retain discovery errors and new files.
"""
import argparse
from collections import defaultdict, deque
import importlib.util
import json
import math
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def interleave(files):
    groups = defaultdict(list)
    for row in files:
        groups[str(Path(row["canonical_path"]).parent)].append(row)
    pending = [deque(sorted(groups[key], key=lambda row: (
        Path(row["path"]).suffix.lower() != ".mzn", row["bytes"], row["path"]
    ))) for key in sorted(groups)]
    ordered = []
    while pending:
        ordered.extend(group.popleft()["path"] for group in pending)
        pending = [group for group in pending if group]
    return ordered


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("roots", type=Path, nargs="+")
    if "--" not in sys.argv[1:]:
        parser.error("separate roots and bench-save batch options with --")
    split = sys.argv.index("--")
    args = parser.parse_args(sys.argv[1:split])
    options = sys.argv[split + 1:]
    probe_options = argparse.ArgumentParser(add_help=False)
    probe_options.add_argument("--probe", type=Path,
                               default=ROOT / "target/release/examples/profile-phases")
    probe_options.add_argument("--probe-timeout", type=float, default=300)
    probe_args, _ = probe_options.parse_known_args(options)
    if not math.isfinite(probe_args.probe_timeout) or probe_args.probe_timeout <= 0:
        parser.error("probe timeout must be finite and positive")
    roots = [str(path.absolute()) for path in args.roots]
    try:
        process = subprocess.run(
            [str(probe_args.probe), "batch", "--manifest-only", *roots],
            capture_output=True, timeout=probe_args.probe_timeout, check=False,
        )
        if process.returncode not in (0, 2):
            parser.error("input discovery failed")
        manifest = json.loads(process.stdout.splitlines()[0])
        if manifest["kind"] != "manifest":
            parser.error("input discovery did not return a manifest")
        ordered = interleave(manifest["files"])
    except (OSError, subprocess.TimeoutExpired, ValueError, KeyError, IndexError) as error:
        parser.error(str(error))
    # The existing runner captures discovery errors, pins inputs, and accounts
    # for unfinished files. Explicit files retain order in Rust discovery.
    spec = importlib.util.spec_from_file_location("bench_save", ROOT / "scripts/bench-save.py")
    bench = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(bench)
    return bench.batch_main([*ordered, *roots, *options])


if __name__ == "__main__":
    raise SystemExit(main())
