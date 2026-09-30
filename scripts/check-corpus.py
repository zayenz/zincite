#!/usr/bin/env python3
"""Read every model/data file, invoke the bounded developer checker, save reports."""
import argparse
import collections
import hashlib
import json
import math
from pathlib import Path
import subprocess
import time


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def inventory(root, errors):
    seen = set()
    found = []
    def visit(directory):
        try:
            physical = directory.resolve(strict=True)
            if physical in seen:
                errors.append({'path': str(directory), 'classification': 'directory_alias_or_cycle'})
                return
            seen.add(physical)
            entries = sorted(directory.iterdir())
            for entry in entries:
                if entry.name == '.git':
                    continue
                if entry.is_dir():
                    visit(entry)
                elif entry.suffix.lower() in ('.mzn', '.dzn') and entry.is_file():
                    found.append(entry)
        except OSError as error:
            errors.append({'path': str(directory), 'classification': 'inventory_error', 'message': str(error)})
    visit(root)
    return found


def causes(result):
    if result.get('input_status') != 'ok':
        return [result.get('input_status', 'checker_error')]
    failed = []
    if not result['token_coverage'] or not result['tree_coverage']:
        failed.append('lossless_coverage')
    if result['parse_count']:
        failed.append('data_contains_model_items' if result['data_model_items'] else 'parse_requires_assessment')
    if result['format_status'] == 'directive_error' or result['lint_status'] == 'directive_error':
        failed.append('directive_error')
    if result['reparse_count']:
        failed.append('formatted_parse_error')
    if result['spellings'] == 'mismatch' or result['structure'] == 'mismatch' or result.get('protected_bytes') == 'mismatch':
        failed.append('format_preservation')
    if result['idempotence'] in ('mismatch', 'error'):
        failed.append('format_second_pass')
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--challenge', type=Path, required=True)
    parser.add_argument('--local', type=Path, required=True)
    parser.add_argument('--supplement', type=Path)
    parser.add_argument('--output', type=Path, default=Path('target/corpus/latest'))
    parser.add_argument('--binary', type=Path, default=Path('target/release/examples/check-corpus-file'))
    parser.add_argument('--timeout', type=float, default=10)
    args = parser.parse_args()
    if not math.isfinite(args.timeout) or args.timeout <= 0:
        parser.error('--timeout must be finite and positive')
    if not args.binary.is_file():
        parser.error('build the developer checker first, or pass --binary PATH')
    roots = {'challenge': args.challenge.resolve(), 'local': args.local.resolve()}
    if args.supplement:
        roots['supplement'] = args.supplement.resolve()
    if any(not path.is_dir() for path in roots.values()):
        parser.error('all corpus roots must be existing directories')
    output = args.output.resolve()
    if any(output.is_relative_to(root) for root in roots.values()):
        parser.error('report output must be outside corpus roots')
    output.mkdir(parents=True, exist_ok=True)
    binary = args.binary.resolve()
    errors, files = [], []
    counts = collections.Counter()
    for name, root in roots.items():
        for path in inventory(root, errors):
            relative = path.relative_to(root).as_posix()
            files.append((name, relative, path))
            counts[f'{name}/{relative.split("/")[0]}/{path.suffix.lower()}'] += 1
    revision = subprocess.run(['git', '-C', str(roots['challenge']), 'rev-parse', 'HEAD'], capture_output=True, text=True)
    tracked = subprocess.run(['git', '-C', str(roots['challenge']), 'ls-files'], capture_output=True, text=True)
    tracked_inputs = [p for p in tracked.stdout.splitlines() if Path(p).suffix.lower() in ('.mzn', '.dzn')]
    missing_tracked = [p for p in tracked_inputs if not (roots['challenge'] / p).is_file()]
    write_json(output / 'inventory.json', {'roots': {k: str(v) for k,v in roots.items()}, 'revision': revision.stdout.strip(), 'revision_status': revision.returncode, 'tracked_status': tracked.returncode, 'tracked_inputs': len(tracked_inputs), 'missing_tracked': missing_tracked, 'counts': dict(counts), 'inventory_notes': errors, 'files': [{'source': n, 'path': r} for n,r,_ in files]})
    failures, totals, duplicates = collections.Counter(), collections.Counter(), {}
    started = time.monotonic()
    with (output / 'results.jsonl').open('w') as report:
        for index, (name, relative, path) in enumerate(files, 1):
            record = {'source': name, 'path': relative}
            try:
                before = path.read_bytes()
                digest = hashlib.sha256(before).hexdigest()
                record['sha256'] = digest
                record['bytes'] = len(before)
                record['duplicate_of'] = duplicates.get(digest)
                duplicates.setdefault(digest, f'{name}/{relative}')
                # Compiler test metadata is a hint, never proof that syntax is invalid.
                opening = before[:8192].lstrip()
                metadata = opening.split(b'***/', 1)[0] if opening.startswith(b'/***') else b''
                record['compiler_negative_hint'] = b'expected:' in metadata and b'!Error' in metadata
                run = subprocess.run([str(binary), str(path)], capture_output=True, timeout=args.timeout)
                if run.returncode:
                    result = {'input_status': 'checker_crash', 'exit_status': run.returncode, 'stderr': run.stderr.decode(errors='replace')[:4096]}
                else:
                    try:
                        result = json.loads(run.stdout)
                    except (ValueError, UnicodeError):
                        result = {'input_status': 'invalid_checker_report', 'stdout': run.stdout.decode(errors='replace')[:4096]}
                record.update(result)
                record['causes'] = causes(result)
            except subprocess.TimeoutExpired:
                record.update(input_status='timeout', causes=['timeout'])
            except OSError as error:
                record.update(input_status='io_error', message=str(error), causes=['io_error'])
            if record.get('sha256'):
                try:
                    record['original_unchanged'] = hashlib.sha256(path.read_bytes()).hexdigest() == record['sha256']
                    if not record['original_unchanged']:
                        record['causes'].append('source_changed_during_check')
                except OSError as error:
                    record['causes'].append('source_recheck_error')
                    record['source_recheck_error'] = str(error)
            failures.update(record['causes'])
            totals['checked'] += 1
            totals['failed_files'] += bool(record['causes'])
            totals['duplicate_files'] += bool(record.get('duplicate_of'))
            totals['compiler_negative_hints'] += bool(record.get('compiler_negative_hint'))
            totals['lint_completed'] += record.get('lint_status') == 'ok'
            totals['warnings'] += record.get('warnings', 0)
            totals['parse_clean'] += record.get('input_status') == 'ok' and record.get('parse_count') == 0
            report.write(json.dumps(record, sort_keys=True) + '\n')
            report.flush()
            if index % 100 == 0:
                print(f'{index}/{len(files)} files checked; {totals["failed_files"]} with failures', flush=True)
    years = {year: {ext: sum(value for key,value in counts.items() if key.startswith(f'challenge/{year}/') and key.endswith(ext)) for ext in ('.mzn','.dzn')} for year in range(2008,2027)}
    summary = {'enumerated': len(files), 'reconciled': totals['checked'] == len(files), 'totals': dict(totals), 'failure_causes': dict(failures), 'elapsed_seconds': time.monotonic()-started, 'timeout_seconds': args.timeout, 'years': years, 'years_without_archive_data': [year for year,data in years.items() if data['.dzn']==0], 'supplement_inputs': sum(name=='supplement' for name,_,_ in files), 'inventory_errors': [e for e in errors if e['classification']=='inventory_error'], 'missing_tracked': missing_tracked, 'binary': str(binary)}
    write_json(output / 'summary.json', summary)
    print(json.dumps(summary, indent=2))
    return int(bool(totals['failed_files'] or summary['inventory_errors'] or missing_tracked or revision.returncode or tracked.returncode))


if __name__ == '__main__':
    raise SystemExit(main())
