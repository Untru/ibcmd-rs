"""Operator capture SQL source contract only; no SQL/provider execution."""
from pathlib import Path
import unittest


CAPTURE = Path(__file__).resolve().parents[2] / "scripts/sql-lab/snapshot-profile/capture.sql"


class CaptureNullGuardTests(unittest.TestCase):
    def test_sql_parameters_require_explicit_null_guards(self):
        # Source contract only: SQL UNKNOWN is not an IF-true refusal. Each
        # nullable typed parameter needs its own explicit NULL branch; this
        # does not execute SQL or establish server/provider behavior.
        capture = CAPTURE.read_text()
        limits = capture.split("THROW 57701,", 1)[0]
        for parameter, maximum in ((2, 1024), (3, 1048576), (4, 8388608),
                                   (5, 262144), (6, 128), (8, 32768)):
            with self.subTest(null_parameter=parameter):
                self.assertRegex(limits,
                    rf"@P{parameter} IS NULL OR @P{parameter} NOT BETWEEN 1 AND {maximum}\b")
        version = capture.split("THROW 57704,", 1)[0].rsplit("\n IF ", 1)[1]
        with self.subTest(null_parameter=9):
            self.assertTrue(version.startswith("@P9 IS NULL OR "))
            self.assertIn("CONVERT(nvarchar(128),SERVERPROPERTY('ProductVersion'))<>@P9", version)
        # A missing expected schema hash is deliberately first-measurement mode.
        self.assertIn("IF @P1 IS NOT NULL AND @SchemaHash<>@P1", capture)
        self.assertNotIn("@P1 IS NULL OR", capture)


if __name__ == "__main__":
    unittest.main()
