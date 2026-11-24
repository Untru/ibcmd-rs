"""Regression cases for strict export-back evidence. Run with python -m unittest discover -s scripts/restructure-lab/s1k."""
import contextlib
import io
import os
from pathlib import Path
import runpy
import sys
import tempfile
import unittest
from unittest.mock import patch


class ExactExportTest(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.native, self.own = (Path(self.scratch.name) / name for name in ("native", "own"))
        for root in (self.native, self.own):
            root.mkdir()
            (root / "Configuration.xml").write_bytes(b"<configuration/>")
            (root / "ConfigDumpInfo.xml").write_bytes(b'<Metadata name="a" id="a" configVersion="00"/>')
            (root / "private.bin").write_bytes(b"\x00")

    def compare(self):
        tool = Path(__file__).with_name("check8.py")
        output = io.StringIO()
        with patch.object(sys, "argv", [str(tool), "--exact-summary", str(self.native), str(self.own)]):
            with contextlib.redirect_stdout(output), self.assertRaises(SystemExit) as exit_info:
                runpy.run_path(str(tool), run_name="__main__")
        return exit_info.exception.code, output.getvalue()

    def test_whole_exports(self):
        self.assertEqual(self.compare()[0], 0)
        info = self.own / "ConfigDumpInfo.xml"
        info.write_bytes(info.read_bytes().replace(b'="00"', b'="01"'))
        code, summary = self.compare()
        self.assertEqual(code, 1)
        self.assertIn("ConfigDumpInfo.xml differs 1", summary)
        (self.own / "private.bin").unlink()
        code, summary = self.compare()
        self.assertEqual(code, 1)
        self.assertIn("only native 1", summary)
        self.assertNotIn("private.bin", summary)

    def test_incomplete_or_unreadable_export(self):
        with patch.object(os, "walk", side_effect=PermissionError("private-path")):
            code, output = self.compare()
        self.assertEqual(code, "8. unable to read both exports completely")
        self.assertEqual(output, "")
        (self.native / "ConfigDumpInfo.xml").unlink()
        self.assertNotEqual(self.compare()[0], 0)
