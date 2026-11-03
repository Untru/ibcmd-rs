"""Producer-only source regression; live RED/GREEN is the retained direct SQL probe."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = ROOT / 'scripts/sql-lab/snapshot-profile'


def branch(sql, field):
    alias = 'ck' if field == 'checks' else 'tr'
    pattern = r'JSON_QUERY\(COALESCE\(\((SELECT ' + alias + r'\.name[\s\S]*?FOR JSON PATH,INCLUDE_NULL_VALUES)\),N\x27\[\]\x27\)\) AS ' + field
    result = re.search(pattern, sql)
    if result is None:
        raise AssertionError('empty-array producer fallback missing')
    return result[1]


class EmptyArrayProducerTests(unittest.TestCase):
    def test_canonical_and_capture_use_only_proven_empty_array_fallback(self):
        canonical = (ROOT / 'src/server/sql_import/snapshot.sql').read_text()
        capture = (PACKAGE / 'capture.sql').read_text()
        for field in ('checks', 'triggers'):
            self.assertEqual(branch(canonical, field), branch(capture, field))
        self.assertEqual(canonical.count("),N'[]')) AS"), 2)
        self.assertEqual(capture.count("),N'[]')) AS"), 2)

    def test_production_subqueries_match_live_public_candidate_projection(self):
        public = (PACKAGE / 'public_empty_arrays_probe.sql').read_text()
        canonical = (ROOT / 'src/server/sql_import/snapshot.sql').read_text()
        for field, columns in (
            ('checks', ('ck.name', 'ck.definition', 'ck.is_disabled', 'ck.is_not_trusted', 'ck.is_not_for_replication')),
            ('triggers', ('tr.name', 'tr.is_disabled', 'tr.is_instead_of_trigger', 'tr.is_not_for_replication', 'sm.definition'))):
            projection = lambda text: re.sub(r'\s+', '', text[text.index('SELECT ')+7:text.index('FROM ')])
            self.assertEqual(projection(branch(canonical, field)), ','.join(columns))
            self.assertIn(','.join(columns), re.sub(r'\s+', '', public))
        self.assertIn('LEFT JOIN sys.sql_modules sm ON sm.object_id=tr.object_id', canonical)
        self.assertIn('ORDER BY ck.name', canonical)
        self.assertIn('ORDER BY tr.name', canonical)
        self.assertNotIn('ISNULL(ck.', canonical)


if __name__ == '__main__':
    unittest.main()
