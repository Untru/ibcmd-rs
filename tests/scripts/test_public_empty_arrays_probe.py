"""Source envelope checks only, never SQL engine RED/GREEN evidence."""
import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
SQL = ROOT / 'scripts/sql-lab/snapshot-profile/public_empty_arrays_probe.sql'


class PublicProducerProbeTests(unittest.TestCase):
    def test_public_fixture_is_select_only_and_payload_free(self):
        code = re.sub(r'--[^\n]*', '', SQL.read_text())
        self.assertIsNone(re.search(r'\b(INSERT|UPDATE|DELETE|CREATE|ALTER|DROP|MERGE|EXEC|EXECUTE|TRUNCATE)\b', code, re.I))
        self.assertNotRegex(code, r'\bBinaryData\b|FROM\s+dbo\.|sys\.sql_modules')
        self.assertIn("HAS_PERMS_BY_NAME(DB_NAME(),'DATABASE','VIEW DEFINITION')", code)
        self.assertIn('database_guid', code)
        self.assertIn("FROM (VALUES(0),(1),(2)) p(ordinal)", code)

    def test_public_nonempty_oracles_keep_null_values_flags_and_order(self):
        source = SQL.read_text()
        declarations = re.findall(r"DECLARE @(checks|triggers)_(one|two) nvarchar\(max\)=N'([^']*)';", source)
        self.assertEqual(len(declarations), 4)
        for field, count, raw in declarations:
            rows = json.loads(raw)
            self.assertEqual(len(rows), 1 if count == 'one' else 2)
            self.assertIsNone(rows[0]['definition'])
            self.assertIs(rows[0]['is_disabled'], False)
            if count == 'two':
                self.assertIs(rows[1]['is_disabled'], True)
                self.assertIs(rows[1]['is_not_for_replication'], True)
            names = [row['name'] for row in rows]
            self.assertEqual(names, sorted(names))
            self.assertTrue(all(name.startswith('public-') for name in names))

    def test_default_openjson_observer_distinguishes_null_and_refuses_loss(self):
        class Pairs(list):
            pass
        def observe(text):
            pairs = json.loads(text, object_pairs_hook=Pairs)
            values = [value for key, value in pairs if key == 'v']
            if len(values) != 1:
                raise ValueError('refuse')
            value = values[0]
            if value is None:
                return 'NULL'
            if type(value) is list:
                return 'ARRAY_NONEMPTY' if value else 'ARRAY_EMPTY'
            raise ValueError('refuse')
        self.assertEqual(observe('{"v":null}'), 'NULL')
        self.assertEqual(observe('{"v":[]}'), 'ARRAY_EMPTY')
        self.assertEqual(observe('{"v":[1]}'), 'ARRAY_NONEMPTY')
        for text in ('{"v":"[]"}', '{}', '{"v":{}}', '{"v":0}',
                     '{"v":false}', '{"v":null,"v":[]}'):
            with self.assertRaises(ValueError):
                observe(text)
        source = SQL.read_text()
        executable = re.sub(r'--[^\n]*', '', source)
        self.assertNotIn(' AS JSON', executable)
        self.assertIn("property_count=1 AND property_type IN (0,4)", source)
        self.assertIn('THROW 57806', source)
        for name in ('original_checks', 'candidate_checks', 'original_triggers', 'candidate_triggers'):
            self.assertIn(name+'_count<>1 OR '+name+'_type NOT IN (0,4)', source)
            self.assertIn("CASE WHEN "+name+"_type=0 THEN N'NULL'", source)
            self.assertIn(name+'_type=4 AND CONVERT(varbinary(max)', source)

    def test_same_projection_and_nested_path_for_original_and_candidate(self):
        source = SQL.read_text()
        for field, alias in (('checks', 'ck'), ('triggers', 'tr')):
            original = re.search(r'JSON_QUERY\(\((SELECT '+alias+r'\.name[\s\S]*?)\)\) AS original_'+field, source)
            candidate = re.search(r'JSON_QUERY\(COALESCE\(\((SELECT '+alias+r'\.name[\s\S]*?)\),N\x27\[\]\x27\)\) AS candidate_'+field, source)
            self.assertIsNotNone(original)
            self.assertIsNotNone(candidate)
            self.assertEqual(original[1], candidate[1])
        self.assertIn(')) AS tables', source)
        self.assertIn("OPENJSON(@Public,'$.tables')", source)
        self.assertIn('LEFT JOIN (VALUES(2', source)
        for name in ('original_checks', 'candidate_checks', 'original_triggers', 'candidate_triggers'):
            self.assertIn('AS '+name+'_shape', source)
            self.assertIn('AS '+name+'_exact', source)
        self.assertIn('check_rows_zero', source)
        self.assertIn('trigger_rows_zero', source)


if __name__ == '__main__':
    unittest.main()
