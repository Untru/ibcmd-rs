#!/usr/bin/env python3
"""Read-only inventory and provenance audit for #361; never runs a database tool."""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import sys

RELEASES = ('8.3.24', '8.3.25', '8.3.26')
PHASES = ('native_export_source', 'candidate_export_source', 'native_import_empty',
          'candidate_import_empty', 'native_reexport_native_target',
          'native_reexport_candidate_target')
COMPARISONS = ('export', 'import')
BUILD = re.compile(r'8\.3\.(24|25|26)\.(0|[1-9][0-9]*)\Z')
SHA = re.compile(r'[0-9a-f]{64}\Z')


class EvidenceError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise EvidenceError(message)


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, f'duplicate JSON key: {key}')
        result[key] = value
    return result


def load(path):
    require(path.stat().st_size <= 128 * 1024**2, 'JSON exceeds 128 MiB')
    with path.open(encoding='utf-8') as stream:
        value = json.load(stream, object_pairs_hook=pairs)
    require(isinstance(value, dict), 'expected a JSON object')
    return value


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def text(value, label):
    require(isinstance(value, str) and value.strip(), f'missing {label}')
    return value


def sha(value):
    require(isinstance(value, str) and SHA.fullmatch(value), 'invalid SHA-256')
    return value


def artifact(root, ref):
    require(isinstance(ref, dict) and set(ref) == {'path', 'sha256'}, 'invalid artifact reference')
    name = text(ref['path'], 'artifact path')
    relative = PurePosixPath(name)
    require(not relative.is_absolute() and '\\' not in name and ':' not in name
            and all(part not in ('', '.', '..') for part in name.split('/')), 'unsafe artifact path')
    path = root
    for part in relative.parts:
        path = path / part
        require(not path.is_symlink(), 'symlink evidence is not allowed')
    require(path.is_file(), f'missing artifact: {name}')
    require(digest(path) == sha(ref['sha256']), f'artifact hash mismatch: {name}')
    return path



def source_inventory(root, ref):
    document = load(artifact(root, ref))
    files = document.get('files')
    require(isinstance(files, list), 'source inventory needs a complete files array')
    result = {}
    for entry in files:
        require(isinstance(entry, dict), 'invalid source inventory entry')
        name = text(entry.get('path'), 'source inventory path')
        require(not PurePosixPath(name).is_absolute() and '\\' not in name and ':' not in name
                and all(part not in ('', '.', '..') for part in name.split('/')),
                'unsafe source inventory path')
        require(name not in result, 'duplicate source inventory path')
        file_hash = sha(entry.get('sha256'))
        size = entry.get('size_bytes')
        require(type(size) is int and size >= 0, 'invalid source inventory size')
        result[name] = {'sha256': file_hash, 'size_bytes': size}
    return result


def inventory(root):
    """Directory names are hints, never a runtime version probe or acceptance."""
    require(root.is_dir(), 'platform root must be an existing directory')
    installed = []
    if root.is_dir():
        for directory in sorted(root.iterdir()):
            if not re.fullmatch(r'[0-9]+(?:\.[0-9]+){3}', directory.name):
                continue
            binary = directory / 'ibcmd'
            if not binary.is_file():
                binary = directory / 'ibcmd.exe'
            installed.append({'directory_build_hint': directory.name,
                              'ibcmd_present': binary.is_file(),
                              'ibcmd_sha256': digest(binary) if binary.is_file() else None,
                              'runtime_version_verified': False})
    return {'schema_version': 1, 'platform_root': str(root), 'installed': installed,
            'requested_releases': list(RELEASES), 'native_acceptance': 'not_measured'}


def check_row(row, root):
    release = row['release']
    if row.get('state') == 'blocked':
        blockers = row.get('blockers')
        require(isinstance(blockers, list) and blockers, 'blocked row needs reasons')
        for blocker in blockers:
            text(blocker, 'blocker')
        return {'release': release, 'state': 'blocked', 'blockers': blockers}
    require(row.get('state') == 'measured', 'row must be blocked or measured')
    build = text(row.get('platform_build'), 'exact platform build')
    require(BUILD.fullmatch(build) and build.rsplit('.', 1)[0] == release,
            'exact platform build does not match release')
    for key in ('native', 'candidate', 'corpus', 'database', 'observations', 'phases', 'comparisons'):
        require(isinstance(row.get(key), dict), f'invalid {key} object')
    native = row['native']
    candidate = row['candidate']
    native_hash = sha(native['binary_sha256'])
    candidate_hash = sha(candidate['binary_sha256'])
    commit = candidate['git_commit']
    require(isinstance(commit, str) and re.fullmatch(r'[0-9a-f]{40}', commit), 'invalid candidate commit')
    require(candidate.get('clean_worktree') is True, 'candidate must have a clean worktree')
    probe = load(artifact(root, native['version_probe']))
    require(probe.get('platform_build') == build and probe.get('binary_sha256') == native_hash
            and type(probe.get('exit_code')) is int and probe['exit_code'] == 0,
            'native version probe does not identify the exact binary/build')
    version_path = artifact(root, probe['stdout'])
    require(version_path.stat().st_size <= 1024**2, 'native version output exceeds 1 MiB')
    version_text = version_path.read_text(encoding='utf-8-sig')
    require(re.findall(r'(?<![\d.])8\.[0-9]+\.[0-9]+\.[0-9]+(?![\d.])', version_text) == [build],
            'native version output must identify one exact build')
    artifact(root, probe['stderr'])
    corpus = row['corpus']
    for field in ('id', 'version', 'origin', 'authorization'):
        text(corpus.get(field), f'corpus {field}')
    snapshot = corpus['snapshot']
    artifact(root, snapshot)
    database = row['database']
    require(database.get('engine') == 'mssql', 'SQL Server evidence is required')
    identifiers = [text(database.get(key), key) for key in ('source_id', 'native_target_id', 'candidate_target_id')]
    require(len(set(identifiers)) == 3, 'source and two empty targets must be distinct')
    before = database['source_before']
    after = database['source_after']
    artifact(root, before)
    artifact(root, after)
    require(before['sha256'] == after['sha256'], 'source fingerprint changed')
    empty = load(artifact(root, database['empty_targets_proof']))
    require(empty.get('engine') == 'mssql' and empty.get('platform_build') == build,
            'empty target proof platform/engine mismatch')
    require(empty.get('native_target_id') == identifiers[1]
            and empty.get('candidate_target_id') == identifiers[2]
            and empty.get('native_empty') is True and empty.get('candidate_empty') is True,
            'empty target proof must identify both fresh targets')
    for key in ('native_inventory', 'candidate_inventory'):
        artifact(root, empty[key])
    observations = row['observations']
    for key in ('unknown_layouts', 'write_limits'):
        require(isinstance(observations.get(key), list), f'missing explicit {key}')
        for value in observations[key]:
            text(value, key)
    artifact(root, observations['layout_census'])
    artifact(root, observations['write_policy'])
    require(set(row['phases']) == set(PHASES), 'incomplete native/candidate export/import phases')
    records = {}
    for phase in PHASES:
        record = load(artifact(root, row['phases'][phase]))
        records[phase] = record
        expected_database = (identifiers[0] if phase.endswith('_source') else
                             identifiers[1] if phase in ('native_import_empty', 'native_reexport_native_target')
                             else identifiers[2])
        require(record.get('database_id') == expected_database, f'phase database mismatch: {phase}')
        expected_hash = candidate_hash if phase.startswith('candidate_') else native_hash
        require(record.get('phase') == phase and record.get('platform_build') == build
                and record.get('binary_sha256') == expected_hash
                and record.get('candidate_commit') == commit
                and record.get('corpus_snapshot_sha256') == snapshot['sha256'],
                f'phase provenance mismatch: {phase}')
        require(type(record.get('exit_code')) is int and record['exit_code'] == 0,
                f'phase failed: {phase}')
        require(isinstance(record.get('argv_redacted'), list) and record['argv_redacted']
                and all(isinstance(arg, str) for arg in record['argv_redacted']), f'missing command: {phase}')
        for key in ('started_utc', 'ended_utc'):
            text(record.get(key), key)
        for key in ('stdout', 'stderr', 'output_inventory'):
            artifact(root, record[key])
    for phase in PHASES:
        if phase.endswith('_source'):
            expected_input = snapshot['sha256']
        elif phase.endswith('_empty'):
            expected_input = records['native_export_source']['output_inventory']['sha256']
        else:
            import_phase = ('native_import_empty' if phase == 'native_reexport_native_target'
                            else 'candidate_import_empty')
            expected_input = records[import_phase]['output_inventory']['sha256']
        require(records[phase].get('input_inventory_sha256') == expected_input,
                f'phase input mismatch: {phase}')
    require(set(row['comparisons']) == set(COMPARISONS), 'export and import comparisons required')
    differences = 0
    for comparison in COMPARISONS:
        report = load(artifact(root, row['comparisons'][comparison]))
        require(report.get('scope') == 'full' and report.get('platform_build') == build
                and report.get('candidate_commit') == commit
                and report.get('corpus_snapshot_sha256') == snapshot['sha256'],
                f'comparison provenance/scope mismatch: {comparison}')
        sides = (('native_export_source', 'candidate_export_source') if comparison == 'export' else
                 ('native_reexport_native_target', 'native_reexport_candidate_target'))
        require(report.get('left_inventory_sha256') == records[sides[0]]['output_inventory']['sha256']
                and report.get('right_inventory_sha256') == records[sides[1]]['output_inventory']['sha256'],
                f'comparison output binding mismatch: {comparison}')
        full_diff = load(artifact(root, report['full_diff']))
        require(full_diff.get('left_inventory_sha256') == report['left_inventory_sha256']
                and full_diff.get('right_inventory_sha256') == report['right_inventory_sha256'],
                f'full diff inventory binding mismatch: {comparison}')
        left_files = source_inventory(root, records[sides[0]]['output_inventory'])
        right_files = source_inventory(root, records[sides[1]]['output_inventory'])
        summary = report['summary']
        require(isinstance(summary, dict), 'invalid summary object')
        require(set(summary) == {'left_only', 'right_only', 'different', 'unchanged'},
                'unexpected comparison summary fields')
        require(full_diff.get('summary') == summary, 'full diff summary mismatch')
        entries = full_diff.get('differences')
        require(isinstance(entries, list), 'full per-file diff required')
        counts = dict.fromkeys(summary, 0)
        paths = set()
        for entry in entries:
            require(isinstance(entry, dict), 'invalid full diff entry')
            name = text(entry.get('path'), 'diff path')
            require(name not in paths, 'duplicate diff path')
            paths.add(name)
            left = left_files.get(name)
            right = right_files.get(name)
            require(left is not None or right is not None, 'diff path absent from compared inventories')
            for side, value in (('left', left), ('right', right)):
                for field in ('sha256', 'size_bytes'):
                    key = f'{side}_{field}'
                    expected = value[field] if value is not None else None
                    require(key in entry and entry[key] == expected
                            and (field != 'size_bytes' or expected is None or type(entry[key]) is int),
                            f'diff content does not match {side} inventory: {name}')
            expected_status = ('right_only' if left is None else 'left_only' if right is None else
                               'unchanged' if left['sha256'] == right['sha256'] else 'different')
            require(left is None or right is None or left['sha256'] != right['sha256']
                    or left['size_bytes'] == right['size_bytes'], 'equal hashes have inconsistent sizes')
            status = entry.get('status')
            require(status == expected_status, f'diff status does not match compared inventories: {name}')
            counts[status] += 1
        require(paths == set(left_files) | set(right_files), 'diff omits compared inventory paths')
        require(counts == summary, 'full diff entries do not match counts')
        for key in ('left_only', 'right_only', 'different', 'unchanged'):
            require(type(summary.get(key)) is int and summary[key] >= 0, 'invalid comparison count')
        require(sum(summary.values()) > 0, 'empty comparison is not evidence')
        differences += sum(summary[key] for key in ('left_only', 'right_only', 'different'))
    return {'release': release, 'platform_build': build, 'state': 'evidence_complete',
            'differences': differences, 'unknown_layouts': observations['unknown_layouts'],
            'write_limits': observations['write_limits']}


def check(document, root):
    require(type(document.get('schema_version')) is int and document['schema_version'] == 1, 'unsupported schema version')
    rows = document.get('rows')
    require(isinstance(rows, list) and len(rows) == 3, 'exactly three release rows required')
    require(all(isinstance(row, dict) for row in rows), 'invalid release row')
    require(sorted(row.get('release', '') for row in rows) == list(RELEASES),
            'one row per 8.3.24, 8.3.25, 8.3.26 required')
    results = [check_row(row, root) for row in rows]
    blocked = any(row['state'] == 'blocked' for row in results)
    return {'schema_version': 1, 'state': 'blocked' if blocked else 'ready_for_review',
            'native_acceptance': 'not_determined', 'rows': results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest='command', required=True)
    inv = commands.add_parser('inventory')
    inv.add_argument('--platform-root', type=Path, required=True)
    audit = commands.add_parser('check')
    audit.add_argument('manifest', type=Path)
    audit.add_argument('--evidence-root', type=Path, required=True)
    args = parser.parse_args()
    try:
        if args.command == 'inventory':
            result = inventory(args.platform_root)
        else:
            result = check(load(args.manifest), args.evidence_root.resolve(strict=True))
        print(json.dumps(result, indent=2, ensure_ascii=False))
        return 2 if result.get('state') == 'blocked' else 0
    except (EvidenceError, OSError, ValueError, KeyError, TypeError) as error:
        print(f'evidence audit failed: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
