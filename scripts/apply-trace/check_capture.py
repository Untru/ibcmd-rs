#!/usr/bin/env python3
"""Offline prerequisite gate for #336; passing is not semantic acceptance.

Reads at most 1 MiB per metadata file. Does not decompress events or row blobs,
connect to SQL, execute commands, mutate captures, or certify their provenance.
"""
import argparse
import json
from pathlib import Path

MAX_META_BYTES = 1024 * 1024
SNAPSHOT_FILES = ('tables.tsv', 'columns.tsv', 'indexes.tsv', 'constraints.tsv',
                  'objects.tsv', 'dbsettings.tsv')
TRACE_FILES = ('events.xml.gz', 'events.tsv.gz', 'session.sql', 'command.log',
               'summary.md', 'write-phases.md', 'timeline.md', 'groups.md',
               'service-writes.tsv', 'data-writes.tsv', 'ddl.sql', 'transactions.tsv')


def check_capture(root):
    root = Path(root).resolve()
    errors = []

    def artifact(relative, nonempty=False):
        path = root / relative
        if not path.resolve().is_relative_to(root):
            errors.append(f'{relative}: escapes capture directory')
            return None
        if not path.is_file() or (nonempty and path.stat().st_size == 0):
            errors.append(f'{relative}: missing or empty required artifact')
            return None
        return path

    def metadata(relative):
        path = artifact(relative, nonempty=True)
        if path is None:
            return {}
        try:
            with path.open('rb') as stream:
                raw = stream.read(MAX_META_BYTES + 1)
            if len(raw) > MAX_META_BYTES:
                raise ValueError('metadata exceeds 1 MiB')
            value = json.loads(raw)
            if not isinstance(value, dict):
                raise ValueError('metadata must be an object')
            return value
        except (ValueError, OSError) as exc:
            errors.append(f'{relative}: {exc}')
            return {}

    trace = metadata('trace/trace-meta.json')
    for key in ('dropped_events', 'dropped_buffers', 'largest_dropped'):
        # Unknown, false, and string zero are not measured numeric zero.
        if type(trace.get(key)) is not int or trace[key] != 0:
            errors.append(f'trace: {key} must be measured integer zero')
    if type(trace.get('events')) is not int or trace['events'] <= 0:
        errors.append('trace: positive event count required')
    if trace.get('timed_out') is not False:
        errors.append('trace: timeout status must be false')
    if type(trace.get('exit_code')) is not int or trace['exit_code'] != 0:
        errors.append('trace: command did not complete successfully; retain failed capture separately')
    if trace.get('include_statements') is not True:
        errors.append('trace: statement-level capture required')
    for name in TRACE_FILES:
        artifact('trace/' + name, nonempty=name in ('events.xml.gz', 'events.tsv.gz', 'session.sql'))
    for phase in ('before', 'after'):
        meta = metadata(phase + '/meta.json')
        for key in ('database', 'database_id', 'server'):
            if not meta.get(key) or meta.get(key) != trace.get(key):
                errors.append(f'{phase}: {key} must match trace identity')
        for name in SNAPSHOT_FILES:
            artifact(phase + '/' + name, nonempty=True)
        rows = meta.get('service_rows')
        if not isinstance(rows, dict) or not rows or len(rows) > 10000:
            errors.append(f'{phase}: bounded, nonempty service_rows inventory required')
            continue
        tables = meta.get('service_tables')
        if not isinstance(tables, list) or set(rows) != set(tables):
            errors.append(f'{phase}: service table inventories disagree')
        for table, count in rows.items():
            if '/' in table or '\\' in table or table in ('.', '..'):
                errors.append(f'{phase}: invalid service table name')
                continue
            if type(count) is not int or count < 0:
                errors.append(f'{phase}: {table} row dump incomplete ({count!r})')
            artifact(f'{phase}/rows/{table}.tsv', nonempty=True)
    artifact('diff/diff.json', nonempty=True)
    artifact('diff/diff.md', nonempty=True)
    return errors


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture', type=Path)
    args = parser.parse_args(argv)
    try:
        errors = check_capture(args.capture)
    except (OSError, ValueError, TypeError, RuntimeError) as exc:
        errors = [f'capture cannot be inspected: {exc}']
    print(json.dumps({'artifact_gate': 'incomplete' if errors else 'complete',
                      'semantic_acceptance': 'not_evaluated', 'errors': errors}, indent=2))
    return 1 if errors else 0


if __name__ == '__main__':
    raise SystemExit(main())
