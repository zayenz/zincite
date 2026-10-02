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
    if result.get('check_kind') == 'semantic':
        failed = []
        if result['token_coverage'] is False or result['tree_coverage'] is False:
            failed.append('lossless_coverage')
        if result['parse_count']:
            failed.append('parse_requires_assessment')
        if result['error_count']:
            failed.append('analysis_errors')
        if result['limitation_count']:
            failed.append('analysis_limitations')
        return failed
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


def semantic_report_valid(result, selection, selected):
    if result.get('input_status') in ('io_error', 'invalid_utf8'):
        return True
    if (result.get('input_status') != 'ok' or result.get('check_kind') != 'semantic'
            or result.get('selection') != selection or not isinstance(result.get('rules'), dict)
            or set(result['rules']) != set(selected)):
        return False
    counts = ('warning_count', 'error_count', 'limitation_count')
    if any(type(result.get(key)) is not int or result[key] < 0 for key in counts):
        return False
    state = result.get('root_state')
    if state != 'input_error' and (type(result.get('parse_count')) is not int or result['parse_count'] < 0):
        return False
    dependencies = result.get('dependencies')
    if state == 'complete' and (result.get('file_mode') != 'model' or result.get('loaded_solve_count') != 1
            or dependencies is not None and (not isinstance(dependencies, dict) or type(dependencies.get('resolved')) is not bool)):
        return False
    for rule in result['rules'].values():
        if (not isinstance(rule, dict)
                or rule.get('outcome') not in ('Completed', 'Inapplicable', 'Limited', 'NotRun')
                or type(rule.get('finding_count')) is not int or rule['finding_count'] < 0
                or not isinstance(rule.get('finding_samples'), list)
                or any(not isinstance(sample, dict) for sample in rule['finding_samples'])):
            return False
    status = 2 if result['error_count'] else int(bool(result['warning_count']))
    return (result.get('analysis_status') == status
            and sum(r['finding_count'] for r in result['rules'].values()) == result['warning_count']
            and result.get('root_state') in ('complete', 'fragment', 'multiple_solve', 'data', 'syntax_rejected', 'input_error')
            and (state == 'input_error' and result.get('token_coverage') is None and result.get('tree_coverage') is None
                 or type(result.get('token_coverage')) is bool and type(result.get('tree_coverage')) is bool))


def unobserved_rules(selected, reason):
    return {rule: {'outcome': 'Unobserved', 'reason': reason, 'finding_count': None}
            for rule in selected}


def semantic_totals(record, totals, rule_totals, states):
    totals['warnings'] += record.get('warning_count', 0)
    totals['errors'] += record.get('error_count', 0)
    totals['limitations'] += record.get('limitation_count', 0)
    totals['api_results_observed'] += record.get('check_kind') == 'semantic'
    totals['process_failures'] += record.get('process_status') != 'reported'
    state = record.get('root_state', 'unobserved')
    states[f'{record["source"]}/{state}'] += 1
    totals['structurally_complete_roots'] += state == 'complete'
    resolved = state == 'complete' and (record.get('dependencies') or {}).get('resolved', False)
    totals['complete_resolved_roots'] += resolved
    totals['complete_roots_all_selected_completed'] += resolved and all(
        r['outcome'] == 'Completed' for r in record['rules'].values())
    for rule_id, rule in record['rules'].items():
        entry = rule_totals[rule_id]
        entry['outcomes'][rule['outcome']] += 1
        entry['findings'] += rule.get('finding_count') or 0
        for sample in rule.get('finding_samples', []):
            if len(entry['finding_samples']) < 3:
                entry['finding_samples'].append({'input_source': record['source'], 'input_path': record['path'], **sample})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--challenge', type=Path, required=True)
    parser.add_argument('--local', type=Path, required=True)
    parser.add_argument('--supplement', type=Path)
    parser.add_argument('--output', type=Path, default=Path('target/corpus/latest'))
    parser.add_argument('--binary', type=Path, default=Path('target/release/examples/check-corpus-file'))
    parser.add_argument('--timeout', type=float, default=10)
    parser.add_argument('--rules', help='existing lint selectors for semantic-only API coverage; skips formatting')
    parser.add_argument('--campaign-timeout', type=float, default=3600, help='finite total child campaign seconds; remaining roots are Unobserved')
    parser.add_argument('-I', '--include-dir', action='append', type=Path, default=[])
    parser.add_argument('--stdlib-dir', type=Path)
    args = parser.parse_args()
    if any(not math.isfinite(v) or v <= 0 for v in (args.timeout, args.campaign_timeout)):
        parser.error('--timeout and --campaign-timeout must be finite and positive')
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
    if output.exists():
        parser.error(f'report already exists: {output}')
    output.mkdir(parents=True)
    (output / 'raw').mkdir()
    binary = args.binary.resolve()
    selected = []
    if args.rules:
        command = [str(binary), '--selected-rules', args.rules]
        with (output / 'selection.stdout').open('xb') as out, (output / 'selection.stderr').open('xb') as err:
            selection_run = subprocess.run(command, stdout=out, stderr=err, timeout=args.timeout)
        write_json(output / 'selection-process.json', {'command': command, 'status': selection_run.returncode})
        if selection_run.returncode:
            parser.error('selection resolution failed; raw evidence retained')
        selection = json.loads((output / 'selection.stdout').read_bytes())
        selected = selection['rules']
        if selection['selection'] != args.rules or len(selected) != len(set(selected)):
            parser.error('catalogue selection report does not match request')
    errors, files = [], []
    counts = collections.Counter()
    for name, root in roots.items():
        for path in inventory(root, errors):
            relative = path.relative_to(root).as_posix()
            files.append((name, relative, path))
            counts[f'{name}/{relative.split("/")[0]}/{path.suffix.lower()}'] += 1
    seen_files, canonical_files = set(), []
    for name, relative, path in files:
        canonical = path.resolve()
        if canonical in seen_files:
            errors.append({'path': str(path), 'classification': 'file_alias'})
            continue
        seen_files.add(canonical); canonical_files.append((name, relative, path))
    files = canonical_files
    counts = collections.Counter(f'{name}/{relative.split("/")[0]}/{path.suffix.lower()}' for name, relative, path in files)
    revision = subprocess.run(['git', '-C', str(roots['challenge']), 'rev-parse', 'HEAD'], capture_output=True, text=True)
    tracked = subprocess.run(['git', '-C', str(roots['challenge']), 'ls-files'], capture_output=True, text=True)
    tracked_inputs = [p for p in tracked.stdout.splitlines() if Path(p).suffix.lower() in ('.mzn', '.dzn')]
    missing_tracked = [p for p in tracked_inputs if not (roots['challenge'] / p).is_file()]
    write_json(output / 'inventory.json', {'roots': {k: str(v) for k,v in roots.items()}, 'revision': revision.stdout.strip(), 'revision_status': revision.returncode, 'tracked_status': tracked.returncode, 'tracked_inputs': len(tracked_inputs), 'missing_tracked': missing_tracked, 'counts': dict(counts), 'inventory_notes': errors, 'files': [{'source': n, 'path': r} for n,r,_ in files]})
    failures, totals, duplicates = collections.Counter(), collections.Counter(), {}
    rule_totals = {rule: {'outcomes': collections.Counter(), 'findings': 0, 'finding_samples': []}
                   for rule in selected} if args.rules else {}
    states = collections.Counter()
    original_hashes, input_pins, dependency_hashes = {}, {}, {}
    for name, relative, path in files:
        try:
            data = path.read_bytes(); value = hashlib.sha256(data).hexdigest()
            original_hashes[path] = value; input_pins[path] = {'sha256': value, 'bytes': len(data)}
        except OSError:
            pass
    for directory in [*args.include_dir, *([args.stdlib_dir] if args.stdlib_dir else [])]:
        for path in inventory(directory.resolve(), []):
            original_hashes[path] = hashlib.sha256(path.read_bytes()).hexdigest()
    write_json(output / 'original-sha256.json', {str(path): value for path,value in original_hashes.items()})
    checker_args = ['--rules', args.rules] if args.rules else []
    if args.rules and args.stdlib_dir:
        checker_args += ['--stdlib-dir', str(args.stdlib_dir.resolve())]
    for directory in args.include_dir:
        checker_args += ['-I', str(directory.resolve())]
    binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    source_head = subprocess.run(['git', 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip()
    started = time.monotonic()
    with (output / 'results.jsonl').open('w') as report:
        for index, (name, relative, path) in enumerate(files, 1):
            record = {'source': name, 'path': relative, 'selection': args.rules or 'default', 'process_status': 'unobserved', **input_pins.get(path, {})}
            remaining = args.campaign_timeout - (time.monotonic() - started)
            child_started = time.monotonic()
            if remaining <= 0:
                record.update(input_status='campaign_deadline', process_status='unobserved', causes=['campaign_deadline'])
            else:
                try:
                    before = path.read_bytes()
                    digest = hashlib.sha256(before).hexdigest()
                    record['observed_input_sha256'] = digest
                    record.setdefault('sha256', digest)
                    original_hashes.setdefault(path, digest)
                    record['bytes'] = len(before)
                    record['duplicate_of'] = duplicates.get(digest)
                    duplicates.setdefault(digest, f'{name}/{relative}')
                    # Compiler test metadata is a hint, never proof that syntax is invalid.
                    opening = before[:8192].lstrip()
                    metadata = opening.split(b'***/', 1)[0] if opening.startswith(b'/***') else b''
                    record['compiler_negative_hint'] = b'expected:' in metadata and b'!Error' in metadata
                    command = [str(binary), str(path), *checker_args]
                    out = output / 'raw' / f'{index:06}.stdout'; err = output / 'raw' / f'{index:06}.stderr'
                    record.update(checker_command=command, checker_stdout=str(out), checker_stderr=str(err), deadline_seconds=min(args.timeout, remaining))
                    with out.open('xb') as stdout, err.open('xb') as stderr:
                        run = subprocess.run(command, stdout=stdout, stderr=stderr, timeout=min(args.timeout, remaining))
                    record['checker_exit_status'] = run.returncode
                    record['process_status'] = 'reported' if run.returncode == 0 else 'crash'
                    if run.returncode:
                        result = {'input_status': 'checker_crash', 'exit_status': run.returncode, 'stderr': err.read_bytes().decode(errors='replace')[:4096]}
                    else:
                        try:
                            result = json.loads(out.read_bytes())
                            if not isinstance(result, dict) or args.rules and not semantic_report_valid(result, args.rules, selected):
                                raise ValueError('checker protocol does not match the selected analysis')
                        except (ValueError, UnicodeError):
                            record['process_status'] = 'invalid_report'
                            result = {'input_status': 'invalid_checker_report', 'stdout': out.read_bytes().decode(errors='replace')[:4096]}
                    record.update(result)
                    record['causes'] = causes(result)
                except subprocess.TimeoutExpired as error:
                    record.update(input_status='timeout', process_status='timeout', causes=['timeout'],
                                  stdout=out.read_bytes().decode(errors='replace')[:4096],
                                  stderr=err.read_bytes().decode(errors='replace')[:4096])
                except OSError as error:
                    record.update(input_status='io_error', process_status='io_error', message=str(error), causes=['io_error'])
            record['elapsed_seconds'] = time.monotonic() - child_started
            if args.rules:
                if record.get('check_kind') != 'semantic':
                    record['rules'] = unobserved_rules(selected, record['input_status'])
                semantic_totals(record, totals, rule_totals, states)
            for dependency in (record.get('dependencies') or {}).get('files', []):
                loaded = Path(dependency['canonical_path'])
                if loaded not in dependency_hashes:
                    try:
                        dependency_hashes[loaded] = hashlib.sha256(loaded.read_bytes()).hexdigest()
                    except OSError as error:
                        record['causes'].append('dependency_hash_error')
                        record.setdefault('dependency_hash_errors', []).append({'path': str(loaded), 'message': str(error)})
            if record.get('sha256'):
                try:
                    record['original_unchanged'] = hashlib.sha256(path.read_bytes()).hexdigest() == record['sha256']
                    if not record['original_unchanged']:
                        record['causes'].append('source_changed_during_check')
                except OSError as error:
                    record['causes'].append('source_recheck_error')
                    record['source_recheck_error'] = str(error)
            if record.get('observed_input_sha256') and record['observed_input_sha256'] != record['sha256']:
                record['causes'].append('source_changed_before_check')
            failures.update(record['causes'])
            totals['checked'] += 1
            totals['attempted'] += 'checker_command' in record
            totals['unobserved'] += record['process_status'] == 'unobserved'
            totals['timeouts'] += record['process_status'] == 'timeout'
            totals['failed_files'] += bool(record['causes'])
            totals['duplicate_files'] += bool(record.get('duplicate_of'))
            totals['compiler_negative_hints'] += bool(record.get('compiler_negative_hint'))
            if not args.rules:
                totals['lint_completed'] += record.get('lint_status') == 'ok'
                totals['warnings'] += record.get('warnings', 0)
            totals['parse_clean'] += record.get('input_status') == 'ok' and record.get('parse_count') == 0
            report.write(json.dumps(record, sort_keys=True) + '\n')
            report.flush()
            if index % 100 == 0:
                print(f'{index}/{len(files)} files checked; {totals["failed_files"]} with failures', flush=True)
    years = {year: {ext: sum(value for key,value in counts.items() if key.startswith(f'challenge/{year}/') and key.endswith(ext)) for ext in ('.mzn','.dzn')} for year in range(2008,2027)}
    summary = {'enumerated': len(files), 'reconciled': totals['checked'] == len(files), 'totals': dict(totals), 'failure_causes': dict(failures), 'elapsed_seconds': time.monotonic()-started, 'timeout_seconds': args.timeout, 'campaign_timeout_seconds': args.campaign_timeout, 'campaign_deadline_reached': any(r for r in failures if r=='campaign_deadline'), 'years': years, 'years_without_archive_data': [year for year,data in years.items() if data['.dzn']==0], 'supplement_inputs': sum(name=='supplement' for name,_,_ in files), 'inventory_errors': [e for e in errors if e['classification']=='inventory_error'], 'missing_tracked': missing_tracked, 'binary': str(binary)}
    after_failures = []
    # A later dependency observation must not replace the before-campaign digest.
    for path, digest in {**dependency_hashes, **original_hashes}.items():
        try:
            if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
                after_failures.append({'path': str(path), 'cause': 'source_changed'})
        except OSError as error:
            after_failures.append({'path': str(path), 'cause': 'source_recheck_error', 'message': str(error)})
    summary.update(selection=args.rules or 'default', check_kind='semantic' if args.rules else 'syntax_format',
                   source_head=source_head, binary_sha256=binary_hash, selected_rules=selected,
                   observed_dependency_hashes={str(p):h for p,h in dependency_hashes.items()},
                   dependency_hash_capture='Before campaign for inventoried roots/include/stdlib source files; otherwise first observed after root analysis.',
                   binary_unchanged=hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash,
                   include_dirs=[str(p.resolve()) for p in args.include_dir],
                   stdlib_dir=str(args.stdlib_dir.resolve()) if args.stdlib_dir else None,
                   original_hashes_rechecked_after_run=len(original_hashes), observed_dependencies_rechecked=len(dependency_hashes), original_recheck_failures=after_failures)
    if args.rules:
        summary.update(per_rule=rule_totals, root_states=dict(states))
        summary['rule_results_reconciled'] = all(sum(r['outcomes'].values()) == len(files) for r in rule_totals.values())
    write_json(output / 'summary.json', summary)
    print(json.dumps(summary, indent=2))
    return int(bool(totals['failed_files'] or summary['inventory_errors'] or missing_tracked or revision.returncode or tracked.returncode or after_failures or not summary['binary_unchanged']))


if __name__ == '__main__':
    raise SystemExit(main())
