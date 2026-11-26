"""Synthetic only. Fixtures are retained under TMPDIR for Trash-only cleanup."""
import gzip
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location('check_capture', Path(__file__).resolve().parents[1] / 'check_capture.py')
CHECK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECK)


class CaptureGateTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(tempfile.mkdtemp(prefix='issue336-capture-', dir=os.environ.get('TMPDIR') or tempfile.gettempdir()))
        trace = dict(database='disposable', database_id=5, server='lab', events=10,
                     dropped_events=0, dropped_buffers=0, largest_dropped=0,
                     timed_out=False, exit_code=0, include_statements=True)
        self.write('trace/trace-meta.json', trace)
        for name in CHECK.TRACE_FILES:
            self.write('trace/' + name, 'synthetic artifact')
        for phase in ('before', 'after'):
            self.write(phase + '/meta.json', dict(database='disposable', database_id=5,
                       server='lab', service_rows={'Config': 0}, service_tables=['Config']))
            for name in CHECK.SNAPSHOT_FILES:
                self.write(phase + '/' + name, 'header\n')
            self.write(phase + '/rows/Config.tsv', 'header\n')
        self.write('diff/diff.json', {})
        self.write('diff/diff.md', 'synthetic diff')

    def write(self, name, data):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(data) if isinstance(data, dict) else data)

    def mutate(self, name, key, value):
        data = json.loads((self.root / name).read_text())
        data[key] = value
        self.write(name, data)

    def test_complete_is_only_artifact_gate(self):
        self.assertEqual(CHECK.check_capture(self.root), [])

    def test_loss_and_unknown_are_not_zero(self):
        for value in (None, False, '0', 1):
            self.mutate('trace/trace-meta.json', 'dropped_events', value)
            self.assertTrue(any('dropped_events' in e for e in CHECK.check_capture(self.root)))

    def test_incomplete_command(self):
        self.mutate('trace/trace-meta.json', 'exit_code', -1)
        self.mutate('trace/trace-meta.json', 'timed_out', True)
        self.assertGreaterEqual(len(CHECK.check_capture(self.root)), 2)

    def test_skipped_row_dump(self):
        self.mutate('before/meta.json', 'service_rows', {'Config': 'skipped (400000 rows)'})
        self.assertTrue(any('row dump incomplete' in e for e in CHECK.check_capture(self.root)))

    def test_identity_mismatch(self):
        self.mutate('after/meta.json', 'database', 'another_database')
        self.assertTrue(any('identity' in e for e in CHECK.check_capture(self.root)))

    def test_missing_raw_and_rows(self):
        # Rename preserves artifacts; no teardown removes files.
        (self.root / 'trace/events.xml.gz').rename(self.root / 'trace/retained.xml.gz')
        (self.root / 'before/rows/Config.tsv').rename(self.root / 'before/rows/retained.tsv')
        self.assertGreaterEqual(len(CHECK.check_capture(self.root)), 2)

    def test_bounded_metadata(self):
        self.write('trace/trace-meta.json', ' ' * (CHECK.MAX_META_BYTES + 1))
        self.assertTrue(any('exceeds 1 MiB' in e for e in CHECK.check_capture(self.root)))

    def test_symlink_escape(self):
        outside = self.root.parent / (self.root.name + '-outside')
        outside.write_text('{}')
        (self.root / 'diff/diff.json').rename(self.root / 'diff/retained.json')
        (self.root / 'diff/diff.json').symlink_to(outside)
        self.assertTrue(any('escapes' in e for e in CHECK.check_capture(self.root)))

    def test_report_retains_intermediate_when_requested(self):
        raw = self.root / 'synthetic-events.xml.gz'
        with gzip.open(raw, 'wt') as stream:
            stream.write('<events></events>')
        output = self.root / 'retained-report'
        result = subprocess.run([sys.executable, str(Path(CHECK.__file__).with_name('trace_report.py')),
                                 '--xml', str(raw), '--out', str(output), '--keep-intermediate'],
                                capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(raw.is_file())
        self.assertTrue((output / 'events.tsv').is_file())
        self.assertTrue((output / 'events.tsv.gz').is_file())

    def test_statement_level_required(self):
        self.mutate('trace/trace-meta.json', 'include_statements', False)
        self.assertTrue(any('statement-level' in e for e in CHECK.check_capture(self.root)))


if __name__ == '__main__':
    unittest.main()
