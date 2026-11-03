"""Public malformed-input/normalization controls; never spawn product or peer."""
import contextlib
import io
import json
import os
from pathlib import Path
import unittest
from unittest.mock import patch

import readonly_probe
import rpc_probe
import verify_capture
from test_verify_capture import transcript


class ReviewRefusals(unittest.TestCase):
    def test_malformed_error_shapes_raise_defined_refusal(self):
        for value in (None, [], "public-shape", {"error": None}, {"error": []}, {"error": "public-shape"}):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "known error code required"):
                readonly_probe.public_error(value, b"")
    def test_nonobject_transcript_is_defined_refusal(self):
        for value in (None, [], "public-shape", 1):
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "JSON object"):
                verify_capture.verify(value)
    def test_bindings_names_in_list_do_not_pass_mapping_check(self):
        doc = transcript(); doc["requestBindings"] = list(doc["requestBindings"])
        with self.assertRaisesRegex(ValueError, "bindings.*object"):
            verify_capture.verify(doc)
    def test_cli_malformed_and_deep_json_refuses_without_traceback(self):
        nested = transcript(); nested["captures"][0]["response"] = []
        for raw in (b"null", json.dumps(nested).encode(), b"[" * 3000 + b"]" * 3000):
            output, errors = io.StringIO(), io.StringIO()
            with patch("sys.argv", ["verify_capture.py", "public-transcript.json"]), patch("verify_capture.Path.open", return_value=io.BytesIO(raw)), contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with self.assertRaises(SystemExit) as stop:
                    verify_capture.main()
            self.assertEqual(stop.exception.code, 1)
            self.assertEqual(output.getvalue(), "")
            self.assertTrue(errors.getvalue().startswith("status transcript refused:"))
            self.assertNotIn("Traceback", errors.getvalue())
    def test_equivalent_absolute_fixture_spelling_is_accepted(self):
        base = rpc_probe.FIXTURE_BASE
        value = str(base / ".." / base.name)
        with patch.dict(os.environ, {"STATUS_LAB_TEST_ROOT": value}):
            self.assertEqual(rpc_probe.explicit_fixture_base(), base)
    def test_missing_relative_and_other_fixture_roots_refuse(self):
        base = rpc_probe.FIXTURE_BASE
        for value in (None, "relative-root", str(base.parent)):
            env = {} if value is None else {"STATUS_LAB_TEST_ROOT": value}
            with patch.dict(os.environ, env, clear=True), self.assertRaises(rpc_probe.Refused):
                rpc_probe.explicit_fixture_base()
    def test_root_normalization_does_not_accept_symlink_alias(self):
        base = rpc_probe.FIXTURE_BASE
        with patch.dict(os.environ, {"STATUS_LAB_TEST_ROOT": str(base)}), patch("rpc_probe.Path.is_symlink", return_value=True):
            with self.assertRaises(rpc_probe.Refused):
                rpc_probe.explicit_fixture_base()


if __name__ == "__main__":
    unittest.main()
