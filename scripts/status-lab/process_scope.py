"""Contain only an owned isolated fake peer; no arbitrary-process controls."""
import os
import signal
import subprocess
import time


class ProcessScope:
    def __init__(self, argv, cwd, env):
        self.job = None
        self.process = None
        if os.name not in {"posix", "nt"}:
            raise ValueError("unsupported-containment")
        if os.name == "nt":
            self._windows_job()
        try:
            self.process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.PIPE,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, shell=False,
                start_new_session=os.name == "posix",
                creationflags=0x00000200 if os.name == "nt" else 0)
            if self.job is not None:
                if not self.kernel.AssignProcessToJobObject(self.job, int(self.process._handle)):
                    raise ValueError("job-assignment-refused")
        except BaseException:
            try:
                if self.process is not None:
                    self.process.kill()
                    self.process.wait(timeout=2)
            finally:
                self._close_job()
            raise

    def _windows_job(self):
        import ctypes as c
        from ctypes import wintypes as w
        size = c.c_size_t
        class Basic(c.Structure):
            _fields_ = [("process_time", c.c_longlong), ("job_time", c.c_longlong),
                ("flags", w.DWORD), ("min_ws", size), ("max_ws", size),
                ("active", w.DWORD), ("affinity", size), ("priority", w.DWORD), ("scheduling", w.DWORD)]
        class IO(c.Structure):
            _fields_ = [(name, c.c_ulonglong) for name in ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]
        class Extended(c.Structure):
            _fields_ = [("basic", Basic), ("io", IO), ("process_memory", size),
                ("job_memory", size), ("peak_process", size), ("peak_job", size)]
        k = c.WinDLL("kernel32", use_last_error=True)
        k.CreateJobObjectW.argtypes = [c.c_void_p, w.LPCWSTR]
        k.CreateJobObjectW.restype = w.HANDLE
        k.SetInformationJobObject.argtypes = [w.HANDLE, c.c_int, c.c_void_p, w.DWORD]
        k.SetInformationJobObject.restype = w.BOOL
        k.AssignProcessToJobObject.argtypes = [w.HANDLE, w.HANDLE]
        k.AssignProcessToJobObject.restype = w.BOOL
        k.CloseHandle.argtypes = [w.HANDLE]
        k.CloseHandle.restype = w.BOOL
        self.kernel = k
        self.job = k.CreateJobObjectW(None, None)
        if not self.job:
            raise ValueError("job-creation-refused")
        info = Extended()
        info.basic.flags = 0x00002000 | 0x00000008  # kill on close + active process limit
        info.basic.active = 4
        if not k.SetInformationJobObject(self.job, 9, c.byref(info), c.sizeof(info)):
            self._close_job()
            raise ValueError("job-limits-refused")

    def _close_job(self):
        if self.job is not None:
            job, self.job = self.job, None
            if not self.kernel.CloseHandle(job):
                raise ValueError("job-close-unconfirmed")

    def finish(self, stdout_eof, grace=0.3, allow_close_stdin=True):
        """Do not reap the leader until its owned group is signalled (PID custody)."""
        p = self.process
        if allow_close_stdin:
            try:
                p.stdin.close()
            except (OSError, ValueError):
                pass
        natural_eof = stdout_eof.wait(grace)
        if os.name == "posix" and not natural_eof:
            # An admitted live descendant retains both pipes. Do not signal an
            # already exited/empty group; reap it directly instead. Keep PID
            # custody until all required group signalling is complete.
            try:
                os.killpg(p.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            if not stdout_eof.wait(min(grace, 0.1)):
                try:
                    os.killpg(p.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        elif os.name == "nt":
            # Pipe EOF may precede interpreter termination. Give a normal peer
            # bounded time to exit before kill-on-close tears down the job.
            if natural_eof:
                try:
                    p.wait(timeout=grace)
                except subprocess.TimeoutExpired:
                    natural_eof = False
            self._close_job()  # kills the peer and all admitted inheriting descendants
        try:
            code = p.wait(timeout=2)
        except subprocess.TimeoutExpired:
            p.kill()
            code = p.wait(timeout=2)
            natural_eof = False
        return code, natural_eof
