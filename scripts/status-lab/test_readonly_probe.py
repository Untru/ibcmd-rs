import copy
import json
import unittest
from readonly_probe import CANDIDATE, HEADER_CAP, STDOUT_CAP, STDERR_CAP, decode_frames, first_operator_plan, public_error, verify_recorded_transition


def binding():
    return {"kind": "immutable-build-receipt", "commit": CANDIDATE, "binarySha256": "1" * 64, "receiptSha256": "2" * 64, "lockSha256": "3" * 64, "sourceTreeSha256": "4" * 64, "cleanSource": True, "toolchain": "1.95.0", "command": ["cargo", "build", "--locked", "--profile", "iter"]}


def frame(message):
    body = json.dumps(message).encode()
    return f"Content-Length: {len(body)}\r\n\r\n".encode() + body


class ReadOnlyProbe(unittest.TestCase):
    def test_plan_is_bounded_readonly_and_not_executed(self):
        p = first_operator_plan(binding())
        self.assertFalse(p["executed"])
        self.assertFalse(p["liveAcceptance"])
        self.assertEqual(p["protocolVersion"], "1.4")
        self.assertEqual(p["process"]["argv"], ["serve", "--stdio"])
        self.assertFalse(p["process"]["shell"])
        self.assertEqual(len(p["requests"]), 7)
        self.assertLessEqual(len(p["requests"]) * p["process"]["requestSeconds"], p["process"]["wallSeconds"])
        self.assertTrue(all(r["method"] in {"initialize", "tree/children", "objects/status"} for r in p["requests"]))
    def test_candidate_binding_not_version_guess(self):
        for key, value in [("commit", "b" * 40), ("binarySha256", ""), ("cleanSource", False), ("kind", "version-string"), ("toolchain", "other")]:
            b = binding(); b[key] = value
            with self.assertRaises(ValueError): first_operator_plan(b)
    def test_private_receipt_field_is_never_echoed(self):
        b = binding(); b["password"] = "synthetic-NEVER-PUBLISH"
        with self.assertRaisesRegex(ValueError, "only public"):
            first_operator_plan(b)
    def test_binding_is_a_snapshot(self):
        b = binding(); plan = first_operator_plan(b)
        b["command"].append("synthetic-NEVER-PUBLISH")
        self.assertNotIn("NEVER-PUBLISH", json.dumps(plan))
    def test_current_active_and_native_main_pair(self):
        requests = first_operator_plan(binding())["requests"]
        self.assertFalse(requests[1]["params"]["mainConfiguration"])
        self.assertTrue(requests[2]["params"]["mainConfiguration"])
        self.assertNotIn("versionBaselineDir", requests[3]["params"])
        self.assertIn("versionBaselineDir", requests[4]["params"])
    def test_frames_preserve_request_identity(self):
        r = {"jsonrpc": "2.0", "id": 5, "result": {"objects": []}}
        self.assertEqual(decode_frames(frame(r)), [r])
    def test_output_cap(self):
        with self.assertRaisesRegex(ValueError, "stdout cap"): decode_frames(b"x" * (STDOUT_CAP + 1))
    def test_header_cap(self):
        with self.assertRaisesRegex(ValueError, "header"): decode_frames(b"x" * (HEADER_CAP + 1) + b"\r\n\r\n")
    def test_duplicate_or_negative_length(self):
        for raw in [b"Content-Length: -1\r\n\r\n", b"Content-Length: 1\r\nContent-Length: 1\r\n\r\n{}"]:
            with self.assertRaisesRegex(ValueError, "Content-Length"): decode_frames(raw)
    def test_truncated_or_oversized_body(self):
        for raw in [b"Content-Length: 4\r\n\r\n{}", f"Content-Length: {STDOUT_CAP + 1}\r\n\r\n".encode()]:
            with self.assertRaises(ValueError): decode_frames(raw)
    def test_duplicate_json_or_nonfinite_values_are_refused(self):
        for body in (b'{"jsonrpc":"2.0","id":99,"id":5}', b'{"jsonrpc":"2.0","id":NaN}'):
            raw = f"Content-Length: {len(body)}\r\n\r\n".encode() + body
            with self.assertRaises(ValueError): decode_frames(raw)
    def test_numeric_overflow_is_refused(self):
        for number in ("1e999", "-1e999"):
            body = ('{"jsonrpc":"2.0","id":5,"result":{"number":' + number + '}}').encode()
            raw = f"Content-Length: {len(body)}\r\n\r\n".encode() + body
            with self.assertRaisesRegex(ValueError, "non-finite JSON number"):
                decode_frames(raw)
    def test_notification_flood(self):
        raw = frame({"jsonrpc": "2.0", "method": "progress"}) * 65
        with self.assertRaisesRegex(ValueError, "notification cap"): decode_frames(raw)
    def test_error_privacy_and_cancel_not_success(self):
        for code in (-32800, -32014):
            result = public_error({"error": {"code": code, "message": "synthetic-password=NEVER-PUBLISH", "data": {"path": "private-fixture-label"}}}, b"raw private SQL error")
            self.assertNotIn("NEVER-PUBLISH", json.dumps(result))
            self.assertNotIn("private", json.dumps(result))
            self.assertFalse(result["publishedStatus"])
    def test_stderr_cap_and_unknown_code(self):
        with self.assertRaises(ValueError): public_error({"error": {"code": -32014}}, b"x" * (STDERR_CAP + 1))
        with self.assertRaises(ValueError): public_error({"error": {"code": True}}, b"")
    def test_recorded_added_object_transition(self):
        before = {"active": ["Configuration", "CommonModule.Control"], "main": ["Configuration", "CommonModule.Control"]}
        staged = {"active": before["active"], "main": before["main"] + ["CommonModule.Added"]}
        applied = {"active": staged["main"], "main": staged["main"]}
        self.assertFalse(verify_recorded_transition(before, staged, applied, expected_added=["CommonModule.Added"])["liveAcceptance"])
        broken = copy.deepcopy(staged); broken["main"] = before["main"]
        with self.assertRaisesRegex(ValueError, "added object"): verify_recorded_transition(before, broken, applied, expected_added=["CommonModule.Added"])
    def test_unexpected_added_object_is_not_known_fixture(self):
        before = {"active": ["Configuration"], "main": ["Configuration"]}
        staged = {"active": before["active"], "main": ["Configuration", "CommonModule.Unrelated"]}
        with self.assertRaisesRegex(ValueError, "inventory differs"):
            verify_recorded_transition(before, staged, {"active": staged["main"], "main": staged["main"]}, expected_added=["CommonModule.Added"])
    def test_unknown_stage_completeness_is_refused(self):
        before = {"active": ["Configuration"], "main": ["Configuration"]}
        with self.assertRaisesRegex(ValueError, "explicit boolean"):
            verify_recorded_transition(before, before, before, complete=None)
    def test_incomplete_recording_falls_back(self):
        before = {"active": ["Configuration"], "main": ["Configuration"]}
        self.assertTrue(verify_recorded_transition(before, before, before, complete=False)["recordingOnly"])
        with self.assertRaisesRegex(ValueError, "incomplete"): verify_recorded_transition(before, {"active": before["active"], "main": ["Configuration", "CommonModule.Unfinished"]}, before, complete=False)


if __name__ == "__main__": unittest.main()
