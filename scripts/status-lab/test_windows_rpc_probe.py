"""Public Windows-only controls; no actual ibcmd, SQL, host or private data."""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import threading
import unittest
from unittest.mock import Mock, patch

import process_scope
import rpc_probe as probe
from test_rpc_probe import fixture, retain


class WindowsOrderingModel(unittest.TestCase):
    """Portable order model is NOT Windows kernel execution evidence."""
    def test_eof_waits_for_normal_exit_before_closing_job(self):
        events = []
        scope = process_scope.ProcessScope.__new__(process_scope.ProcessScope)
        scope.process = Mock()
        scope.process.wait.side_effect = lambda **kw: events.append("wait") or 0
        scope._close_job = lambda: events.append("close-job")
        eof = threading.Event(); eof.set()
        with patch("process_scope.os.name", "nt"):
            self.assertEqual(scope.finish(eof), (0, True))
        self.assertEqual(events, ["wait", "close-job", "wait"])

    def test_eof_does_not_turn_a_stalled_exit_into_success(self):
        scope = process_scope.ProcessScope.__new__(process_scope.ProcessScope)
        scope.process = Mock()
        scope.process.wait.side_effect = [subprocess.TimeoutExpired("fake", .3), 1]
        scope._close_job = Mock()
        eof = threading.Event(); eof.set()
        with patch("process_scope.os.name", "nt"):
            self.assertEqual(scope.finish(eof), (1, False))
        scope._close_job.assert_called_once_with()

    def test_stale_code_seal_refuses_before_process_creation(self):
        lease = fixture(); root = lease.root
        seal_path = root / "seal.json"
        seal = json.loads(seal_path.read_text())
        seal["codeDigests"]["process_scope.py"] = "0" * 64
        seal_path.write_text(json.dumps(seal))
        with patch("rpc_probe.ProcessScope") as spawn:
            report = retain(root, probe.run_probe(lease))
            spawn.assert_not_called()
        self.assertFalse(report["success"])
        self.assertFalse(report["spawned"])
        self.assertEqual(report["failure"], "code-binding")

    def test_failed_job_assignment_reaps_peer_without_bootstrap(self):
        scope_process = Mock()
        scope_process._handle = 17
        scope_process.wait.return_value = 1
        kernel = Mock()
        kernel.AssignProcessToJobObject.return_value = 0
        kernel.CloseHandle.return_value = 1
        def fake_job(scope):
            scope.kernel, scope.job = kernel, 19
        with patch("process_scope.os.name", "nt"), patch.object(process_scope.ProcessScope, "_windows_job", fake_job), patch("process_scope.subprocess.Popen", return_value=scope_process):
            with self.assertRaisesRegex(ValueError, "job-assignment-refused"):
                process_scope.ProcessScope(["finite-fake"], ".", {})
        scope_process.stdin.write.assert_not_called()
        scope_process.kill.assert_called_once_with()
        scope_process.wait.assert_called_once_with(timeout=2)
        kernel.CloseHandle.assert_called_once_with(19)


@unittest.skipUnless(os.name == "nt", "requires real Windows Job Object execution")
class WindowsKernelControls(unittest.TestCase):
    def setUp(self):
        self.k = ctypes.WinDLL("kernel32", use_last_error=True)
        self.k.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
        self.k.OpenProcess.restype = wintypes.HANDLE
        self.k.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
        self.k.WaitForSingleObject.restype = wintypes.DWORD
        self.k.CloseHandle.argtypes = [wintypes.HANDLE]
        self.k.CloseHandle.restype = wintypes.BOOL
        self.k.IsProcessInJob.argtypes = [wintypes.HANDLE, wintypes.HANDLE, ctypes.POINTER(wintypes.BOOL)]
        self.k.IsProcessInJob.restype = wintypes.BOOL
        self.k.QueryInformationJobObject.argtypes = [wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD, ctypes.c_void_p]
        self.k.QueryInformationJobObject.restype = wintypes.BOOL

    def test_fixed_argv_and_real_job_membership_before_bootstrap(self):
        lease = fixture(); root = lease.root
        original = probe.ProcessScope
        observed = []
        def admitted(argv, cwd, env):
            self.assertEqual(argv[1:4], ["-I", "-S", "-B"])
            self.assertEqual(argv[5:], ["--case", "ok", "serve", "--stdio"])
            self.assertEqual(Path(argv[4]).name, "peer_snapshot.py")
            self.assertEqual(cwd, root)
            self.assertEqual(set(env), {"PATH", "TMPDIR", "TMP", "TEMP", "LANG", "SystemRoot"})
            self.assertEqual(env["PATH"], "")
            scope = original(argv, cwd, env)
            try:
                inside = wintypes.BOOL()
                self.assertTrue(self.k.IsProcessInJob(int(scope.process._handle), scope.job, ctypes.byref(inside)))
                self.assertTrue(inside.value)
                # Basic accounting: current active member count must be exactly one.
                class Accounting(ctypes.Structure):
                    _fields_ = [(name, ctypes.c_longlong) for name in ("user", "kernel", "period_user", "period_kernel")] + [(name, wintypes.DWORD) for name in ("faults", "total", "active", "terminated")]
                info = Accounting()
                self.assertTrue(self.k.QueryInformationJobObject(scope.job, 1, ctypes.byref(info), ctypes.sizeof(info), None))
                self.assertEqual(info.active, 1)
                class BasicLimits(ctypes.Structure):
                    _fields_ = [("process_time", ctypes.c_longlong), ("job_time", ctypes.c_longlong),
                        ("flags", wintypes.DWORD), ("min_ws", ctypes.c_size_t), ("max_ws", ctypes.c_size_t),
                        ("active", wintypes.DWORD), ("affinity", ctypes.c_size_t),
                        ("priority", wintypes.DWORD), ("scheduling", wintypes.DWORD)]
                limits = BasicLimits()
                self.assertTrue(self.k.QueryInformationJobObject(scope.job, 2, ctypes.byref(limits), ctypes.sizeof(limits), None))
                self.assertEqual(limits.flags & (0x2000 | 0x8), 0x2000 | 0x8)
                self.assertEqual(limits.active, 4)
                observed.append(True)
            except BaseException:
                scope.finish(threading.Event(), grace=.1)
                raise
            return scope
        with patch("rpc_probe.ProcessScope", side_effect=admitted):
            report = retain(root, probe.run_probe(lease, limits=probe.Limits(wall=15, request=3, preparation=10)))
        self.assertEqual(observed, [True])
        self.assertTrue(report["success"], report)
        self.assertEqual(report["requests"], 7)
        self.assertTrue(report["bindingsUnchanged"])
        self.assertFalse(report["productExecuted"])

    def test_descendant_handle_is_signalled_after_job_close(self):
        lease = fixture(); root = lease.root
        original = probe.ProcessScope
        handles = []
        checks = []
        def admitted(*args):
            scope = original(*args)
            finish = scope.finish
            def watched(*a, **kw):
                try:
                    self.assertEqual((root / "descendant.ready").read_text(), "ready")
                    pid = int((root / "descendant.pid").read_text())
                    # Hold identity BEFORE cleanup; never infer death from a missing PID.
                    handle = self.k.OpenProcess(0x00100000, False, pid)
                    self.assertTrue(handle, "owned descendant must be alive before cleanup")
                    handles.append(handle)
                    self.assertEqual(self.k.WaitForSingleObject(handle, 0), 258)
                finally:
                    result = finish(*a, **kw)
                checks.append(self.k.WaitForSingleObject(handle, 2000))
                return result
            scope.finish = watched
            return scope
        try:
            with patch("rpc_probe.ProcessScope", side_effect=admitted):
                report = retain(root, probe.run_probe(lease, "descendant-hang", probe.Limits(wall=15, request=3, preparation=10)))
            self.assertEqual(len(handles), 1, report)
            self.assertEqual(checks, [0], "retained descendant handle must be signalled")
            self.assertTrue(report["cleanupConfirmed"], report)
            self.assertFalse(report["success"], report)
            self.assertEqual(report["failure"], "process-not-clean")
        finally:
            for handle in handles:
                self.assertTrue(self.k.CloseHandle(handle))

    def test_windows_hang_flood_and_drift_never_succeed(self):
        cases = (("hang", "timeout"), ("stdout-cap", "stdout-cap"), ("stderr-cap", "stderr-cap"),
                 ("mutate-input", "cleanup-or-binding-unconfirmed"), ("mutate-seal", "cleanup-or-binding-unconfirmed"))
        for case, failure in cases:
            with self.subTest(case=case):
                lease = fixture(); root = lease.root
                limits = probe.Limits(wall=8, request=2, preparation=10, stdout=4096, stderr=64)
                report = retain(root, probe.run_probe(lease, case, limits))
                self.assertFalse(report["success"], report)
                self.assertEqual(report["failure"], failure, report)
                self.assertTrue(report["cleanupConfirmed"], report)
                self.assertFalse(report["liveAcceptance"])
                self.assertFalse(report["productExecuted"])
                self.assertNotIn(str(root), json.dumps(report))


if __name__ == "__main__":
    unittest.main()
