"""Bounded execution of one reviewed public fake RPC peer, never actual ibcmd."""
from dataclasses import dataclass
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import tempfile
import threading
import time

from process_scope import ProcessScope
from readonly_probe import CANDIDATE, API_BASE, PROTOCOL, decode_frames, public_error

PEER = Path(__file__).with_name("fake_rpc_peer.py")
CODE = ("rpc_probe.py", "process_scope.py", "readonly_probe.py", "fake_rpc_peer.py", "rpc-runner-contract.md", "windows_file_identity.py")
CASES = {"ok", "hang", "stdout-cap", "stderr-cap", "early-error", "wrong-id", "nonzero", "malformed", "mutate-input", "mutate-seal", "descendant-hang", "missing-cancel", "stale-snapshot", "trailing-frame", "trailing-error", "trailing-malformed", "trailing-flood", "trailing-progress"}
PINNED_API = {
    "src/server/mod.rs": "e7f79e773e1875f75d48f2cb428e9ddcd22393ad8a88fdd416ab9418f01e953a",
    "src/server/rpc.rs": "ed603ed1deab59b750e20a949a85b28118e2da4aaa0fc0f1e55b85e956ac2942",
    "src/stored_objects.rs": "67d6f65921a4fd96527f35a03fcfd1379f5fb2fba64e34609e44a78c9f8ca915",
}


class Refused(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__(code)


def need(ok, code):
    if not ok:
        raise Refused(code)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(stat):
    return (stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns)


# This adapter is pinned to the selected external lab layout, not an arbitrary cwd.
FIXTURE_BASE = Path(__file__).resolve().parents[4] / "tmp" / "issue-380-rpc-runner"
_OWNED = {}


@dataclass(frozen=True)
class OwnedFixture:
    root: Path
    token: object


def explicit_fixture_base():
    value = os.environ.get("STATUS_LAB_TEST_ROOT")
    need(isinstance(value, str) and bool(value), "explicit-fixture-base-required")
    candidate = Path(value)
    need(candidate.is_absolute(), "explicit-fixture-base-required")
    for component in (candidate, *candidate.parents):
        need(not component.is_symlink() and not (hasattr(component, "is_junction") and component.is_junction()), "symlink-refused")
    need(candidate.resolve(strict=True) == FIXTURE_BASE, "explicit-fixture-base-required")
    return FIXTURE_BASE


def create_fixture_root():
    base = explicit_fixture_base()
    need(base.parents[1].name == "ibcmd-rs-lab" and base.is_dir(), "external-fixture-base-required")
    for component in (base, *base.parents):
        need(not component.is_symlink(), "symlink-refused")
    root = Path(tempfile.mkdtemp(prefix="rpc-contract-", dir=base))
    token = object()
    stamp = root.lstat()
    _OWNED[token] = (root, stamp.st_dev, stamp.st_ino)
    return OwnedFixture(root, token)


def owned_root(lease):
    need(type(lease) is OwnedFixture, "owned-fixture-required")
    known = _OWNED.pop(lease.token, None)
    need(known is not None and known[0] == lease.root and lease.root.parent == FIXTURE_BASE,
         "owned-fixture-required")
    stamp = lease.root.lstat()
    need(not lease.root.is_symlink() and lease.root.is_dir()
         and (stamp.st_dev, stamp.st_ino) == known[1:], "owned-fixture-rebound")
    return lease.root


def check_deadline(deadline):
    need(deadline is None or time.monotonic() <= deadline, "preparation-timeout")


def regular(path, cap, deadline=None, unique=False):
    check_deadline(deadline)
    path = Path(path)
    need(path.is_absolute(), "absolute-input-required")
    for component in (path, *path.parents):
        need(not component.is_symlink(), "symlink-refused")
    before = path.lstat()
    need(not unique or before.st_nlink == 1, "input-hardlink-refused")
    need(path.is_file() and before.st_size <= cap, "regular-input-cap")
    if os.name == "nt":
        from windows_file_identity import path_identity, fd_identity
        before_identity = path_identity(path)
        need(not unique or before_identity[-1] == 1, "input-hardlink-refused")
    else:
        before_identity = identity(before)
        fd_identity = lambda fd: identity(os.fstat(fd))
        path_identity = lambda name: identity(name.lstat())
    flags = os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_BINARY", 0)
    fd = os.open(path, flags)
    with os.fdopen(fd, "rb") as stream:
        need(fd_identity(stream.fileno()) == before_identity, "input-rebound")
        chunks, size = [], 0
        while True:
            check_deadline(deadline)
            chunk = stream.read(min(65536, cap + 1 - size))
            if not chunk:
                break
            chunks.append(chunk); size += len(chunk)
            need(size <= cap, "regular-input-cap")
        data = b"".join(chunks)
        need(len(data) <= cap and fd_identity(stream.fileno()) == before_identity, "input-changed")
    need(path_identity(path) == before_identity, "input-rebound")
    check_deadline(deadline)
    return data, before_identity


def inventory(root, deadline=None):
    root = Path(root)
    need(root.is_absolute() and root.is_dir() and not root.is_symlink(), "input-root-refused")
    files, total, keys = {}, 0, set()
    pending = [root]
    while pending:
        check_deadline(deadline)
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                check_deadline(deadline)
                need(len(keys) < 64, "inventory-cap")
                path = Path(entry.path)
                need(not path.is_symlink(), "symlink-refused")
                relative = path.relative_to(root).as_posix()
                key = relative.casefold()
                need(key not in keys, "input-alias-refused")
                keys.add(key)
                if path.is_dir():
                    pending.append(path)
                else:
                    data, stamp = regular(path, 1 << 20, deadline, unique=True)
                    total += len(data)
                    need(total <= 1 << 20 and len(files) < 32, "inventory-cap")
                    files[relative] = [sha(data), stamp]
    need(files, "empty-input")
    return sha(json.dumps(files, sort_keys=True).encode())


def inputs(root, deadline=None):
    settings = regular(root / "settings.toml", 16 << 10, deadline, unique=True)
    return {"source": inventory(root / "source", deadline), "reference": inventory(root / "reference", deadline),
            "settings": sha(json.dumps([sha(settings[0]), settings[1]]).encode())}


def code_digests(deadline=None):
    result = {}
    for name in CODE:
        data, stamp = regular(Path(__file__).with_name(name).absolute(), 1 << 20, deadline)
        result[name] = sha(json.dumps([sha(data), stamp]).encode())
    return result


def checked_seal(root, deadline=None):
    raw, stamp = regular(root / "seal.json", 16 << 10, deadline, unique=True)
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, "seal-schema")
            result[key] = value
        return result
    seal = json.loads(raw, object_pairs_hook=pairs)
    need(set(seal) == {"schema", "adapter", "candidateCommit", "apiBase", "protocol", "interpreterSha256", "codeDigests", "inputDigests"}, "seal-schema")
    need(type(seal["schema"]) is int and seal["schema"] == 1, "seal-schema")
    need(seal["adapter"] == "public-fake-stdio-v1", "adapter-refused")
    need(seal["candidateCommit"] == CANDIDATE and seal["apiBase"] == API_BASE and seal["protocol"] == PROTOCOL, "candidate-binding")
    interpreter = Path(sys.executable).resolve(strict=True)
    binary, binary_stamp = regular(interpreter, 64 << 20, deadline)
    need(seal["interpreterSha256"] == sha(binary), "interpreter-binding")
    need(seal["codeDigests"] == code_digests(deadline), "code-binding")
    actual = inputs(root, deadline)
    need(seal["inputDigests"] == actual, "input-binding")
    repo = Path(__file__).resolve().parents[2]
    api = {}
    for name, expected in PINNED_API.items():
        data, file_stamp = regular(repo / name, 1 << 20, deadline)
        need(sha(data) == expected, "api-binding")
        api[name] = file_stamp
    check_deadline(deadline)
    return {"receipt": sha(raw), "receiptIdentity": stamp, "interpreter": sha(binary),
            "interpreterIdentity": binary_stamp, "apiIdentities": api, "codes": seal["codeDigests"], "inputs": actual}


@dataclass(frozen=True)
class Limits:
    wall: float = 5.0
    preparation: float = 2.0
    request: float = 1.0
    stdout: int = 1 << 20
    stderr: int = 16 << 10
    def validate(self):
        need(all(type(v) in (int, float) and math.isfinite(v) for v in (self.wall, self.request, self.preparation)), "limit-schema")
        need(0.1 <= self.preparation <= 10, "limit-schema")
        need(0.1 <= self.request <= min(20, self.wall) and self.wall <= 180, "limit-schema")
        need(type(self.stdout) is int and 64 <= self.stdout <= 1 << 20, "limit-schema")
        need(type(self.stderr) is int and 16 <= self.stderr <= 16 << 10, "limit-schema")


class Streams:
    def __init__(self, process, limits):
        self.condition = threading.Condition()
        self.data = {"stdout": bytearray(), "stderr": bytearray()}
        self.failure = None
        self.eof = threading.Event()
        self.process_eof = threading.Event()
        self.threads = []
        self.offset, self.notifications = 0, 0
        self.limits = limits
        for name, stream, cap in (("stdout", process.stdout, limits.stdout), ("stderr", process.stderr, limits.stderr)):
            thread = threading.Thread(target=self._read, args=(name, stream, cap), daemon=True)
            thread.start(); self.threads.append(thread)
    def _read(self, name, stream, cap):
        try:
            while True:
                part = os.read(stream.fileno(), 4096)
                with self.condition:
                    if not part:
                        if name == "stdout": self.eof.set()
                        self.process_eof.set()
                        self.condition.notify_all()
                        return
                    room = cap - len(self.data[name])
                    self.data[name].extend(part[:max(0, room)])
                    if len(part) > room:
                        self.failure = f"{name}-cap"
                        self.condition.notify_all()
                        return
                    self.condition.notify_all()
        except OSError:
            with self.condition:
                self.failure = self.failure or "pipe-read"
                self.condition.notify_all()
    def next(self, deadline):
        while True:
            need(time.monotonic() <= deadline, "timeout")
            with self.condition:
                need(self.failure is None, self.failure or "pipe-read")
                available = self.data["stdout"]
                cut = available.find(b"\r\n\r\n", self.offset)
                if cut < 0:
                    need(len(available) - self.offset <= 8 << 10, "header-cap")
                else:
                    need(cut - self.offset <= 8 << 10, "header-cap")
                    header = bytes(available[self.offset:cut])
                    lengths = [line.split(b":", 1)[1].strip() for line in header.split(b"\r\n") if line.lower().startswith(b"content-length:")]
                    need(len(lengths) == 1 and lengths[0].isdigit(), "frame-header")
                    size = int(lengths[0])
                    need(0 < size <= self.limits.stdout, "frame-cap")
                    end = cut + 4 + size
                    if len(available) >= end:
                        packet = bytes(available[self.offset:end]); self.offset = end
                        value = decode_frames(packet)[0]
                        if "id" not in value:
                            self.notifications += 1
                            need(self.notifications <= 64 and set(value) == {"jsonrpc", "method", "params"}
                                 and value.get("method") in {"progress", "log"} and isinstance(value["params"], dict), "notification-cap")
                            continue
                        return value
                need(not self.eof.is_set(), "early-exit")
                remaining = deadline - time.monotonic()
                need(remaining > 0, "timeout")
                self.condition.wait(min(remaining, 0.1))
    def joined(self):
        for thread in self.threads: thread.join(timeout=2)
        return not any(thread.is_alive() for thread in self.threads)


def write_bounded(process, packet, deadline, writers):
    done, failed = threading.Event(), []
    def write():
        try:
            process.stdin.write(packet)
            process.stdin.flush()
        except (OSError, ValueError):
            failed.append(True)
        finally:
            done.set()
    thread = threading.Thread(target=write, daemon=True)
    writers.append(thread); thread.start()
    need(done.wait(max(0, deadline - time.monotonic())), "timeout")
    need(not failed, "pipe-write")


def send(process, message, deadline, writers):
    body = json.dumps(message, ensure_ascii=True).encode()
    need(len(body) <= 16 << 10, "request-cap")
    write_bounded(process, f"Content-Length: {len(body)}\r\n\r\n".encode() + body, deadline, writers)


def comparison(result):
    rows = result.get("objects")
    need(isinstance(rows, list) and len(rows) == 1 and isinstance(rows[0], dict), "status-schema")
    row = rows[0]
    need(row.get("object") == "CommonModule.Control" and row.get("status") == "same", "status-schema")
    need(isinstance(row.get("version"), str) and row["version"], "status-schema")
    for field in ("changed", "baseOnly", "localOnly"):
        need(isinstance(row.get(field), list) and not row[field], "status-schema")
    return row


def native(result, baseline):
    need(type(result.get("fullRequired")) is bool and result["fullRequired"] is False, "native-unknown")
    snapshot = result.get("databaseSnapshot")
    need(isinstance(snapshot, str) and re.fullmatch(r"[a-f0-9]{64}", snapshot), "native-snapshot")
    rows = result.get("objects")
    need(isinstance(rows, list) and len(rows) == 1, "native-objects")
    row = comparison(result); evidence = row.get("databaseVersionStatus", {})
    need(row.get("object") == "CommonModule.Control" and row.get("status") == "same", "native-object")
    need(isinstance(row.get("version"), str) and row["version"], "native-token")
    need(evidence.get("basis") == "native-config-version" and evidence.get("object") == row["object"] and evidence.get("objectVersion") == row["version"], "native-token")
    need(evidence.get("baselineIdentity") == baseline and evidence.get("databaseSnapshot") == snapshot, "native-snapshot")
    need(evidence.get("conclusive") is True and evidence.get("changed") is False, "native-unknown")
    return snapshot


def run_probe(fixture, case="ok", limits=Limits()):
    """Only admitted fake execution. Every report explicitly disclaims product/live proof."""
    report = {"schema": 1, "fixtureOnly": True, "liveAcceptance": False, "productExecuted": False,
              "publishedStatus": False, "success": False, "spawned": False}
    scope = streams = None
    writers = []
    snapshot = snapshot_before = None
    root = None
    before = None
    try:
        limits.validate(); need(case in CASES, "case-refused")
        report["case"] = case  # validated finite public label, never arbitrary caller text
        root = owned_root(fixture)
        preparation = time.monotonic() + limits.preparation
        before = checked_seal(root, preparation)
        run = Path(tempfile.mkdtemp(prefix="rpc-probe-", dir=root))
        peer_bytes = regular(PEER.absolute(), 1 << 20, preparation)[0]
        snapshot = run / "peer_snapshot.py"
        with snapshot.open("xb") as target: target.write(peer_bytes)
        os.chmod(snapshot, 0o400)
        snapshot_before = regular(snapshot, 1 << 20, preparation)
        need(sha(snapshot_before[0]) == sha(peer_bytes), "snapshot-binding")
        env = {"PATH": "", "TMPDIR": str(run), "TMP": str(run), "TEMP": str(run), "LANG": "C.UTF-8"}
        if os.name == "nt":
            need(bool(os.environ.get("SystemRoot")), "system-root-required")
            env["SystemRoot"] = os.environ["SystemRoot"]
        argv = [str(Path(sys.executable).resolve()), "-I", "-S", "-B", str(snapshot), "--case", case, "serve", "--stdio"]
        check_deadline(preparation)
        wall = time.monotonic() + limits.wall
        scope = ProcessScope(argv, root, env)
        report["spawned"] = True
        streams = Streams(scope.process, limits)
        report["binding"] = {"candidateCommit": CANDIDATE, "apiBase": API_BASE, "protocol": PROTOCOL,
            "receiptSha256": before["receipt"], "interpreterSha256": before["interpreter"],
            "peerSnapshotSha256": sha(peer_bytes), "sourceInputsSha256": before["inputs"]}
        write_bounded(scope.process, b"STATUS_LAB_FIXTURE_V1\n", min(wall, time.monotonic() + limits.request), writers)
        source, reference = str(root / "source"), str(root / "reference")
        base_params = {"infobase": "public-fake-only", "sourceDir": source}
        calls = [
            ("initialize", {"protocolVersion": PROTOCOL, "settingsFile": str(root / "settings.toml"), "workspaceFolder": str(root)}),
            ("tree/children", {"infobase": "public-fake-only", "mainConfiguration": False}),
            ("tree/children", {"infobase": "public-fake-only", "mainConfiguration": True}),
            ("objects/status", base_params),
            *[("objects/status", {**base_params, "versionBaselineDir": reference}) for _ in range(3)],
        ]
        baseline = sha(regular(root / "reference" / "ConfigDumpInfo.xml", 1 << 20)[0])
        for ident, (method, params) in enumerate(calls, 1):
            need(time.monotonic() < wall, "timeout")
            deadline = min(wall, time.monotonic() + limits.request)
            send(scope.process, {"jsonrpc": "2.0", "id": ident, "method": method, "params": params}, deadline, writers)
            if ident == 6:
                send(scope.process, {"jsonrpc": "2.0", "method": "$/cancelRequest", "params": {"id": 6}}, deadline, writers)
            response = streams.next(deadline)
            need(type(response.get("id")) is int and response["id"] == ident, "response-id")
            need(not ("error" in response and "result" in response), "response-schema")
            if ident == 6:
                need(response.get("error", {}).get("code") == -32800 and "result" not in response, "missing-cancel")
                report["cancelledRequest"] = 6
                continue
            if "error" in response:
                report["publicError"] = public_error(response, b"")
                raise Refused("rpc-error")
            result = response.get("result")
            need(isinstance(result, dict), "response-schema")
            if ident == 1:
                need(result.get("protocolVersion") == PROTOCOL, "protocol-mismatch")
                caps = result.get("capabilities", {})
                need(caps.get("cancelRequest") is True and {"objects/status", "tree/children"} <= set(caps.get("methods", [])), "capabilities")
            elif ident in (2, 3):
                need(isinstance(result.get("nodes"), list) and len(result["nodes"]) == 1 and isinstance(result["nodes"][0], dict) and result["nodes"][0].get("id") == "Configuration" and result["nodes"][0].get("kind") == "Configuration", "tree-schema")
            elif ident == 4:
                comparison(result)
            elif ident == 5:
                report["validatedNativeSnapshot"] = native(result, baseline)
            elif ident == 7:
                # Validate shape only; a retired generation never publishes its snapshot.
                native(result, baseline)
                report["discardedStaleRequest"] = 7
        report["requests"] = 7
        report["success"] = True
    except Refused as error:
        report["failure"] = error.code
    except Exception:
        report["failure"] = "invalid-or-unavailable"
    finally:
        if scope is not None:
            try:
                code, natural = scope.finish(streams.process_eof if streams else threading.Event(), allow_close_stdin=not any(w.is_alive() for w in writers))
                report["returnCode"] = code
                joined = streams.joined() if streams else True
                for writer in writers: writer.join(timeout=2)
                joined = joined and not any(writer.is_alive() for writer in writers)
                report["cleanupConfirmed"] = joined
                if code != 0 or not natural or not joined:
                    report.update(success=False, failure=report.get("failure", "process-not-clean"))
                if streams:
                    report["stdoutBytes"], report["stderrBytes"] = (len(streams.data[n]) for n in ("stdout", "stderr"))
                    if streams.failure:
                        report.update(success=False, failure=streams.failure)
                    else:
                        trailing = bytes(streams.data["stdout"][streams.offset:])
                        for message in decode_frames(trailing) if trailing else []:
                            need("id" not in message and set(message) == {"jsonrpc", "method", "params"}
                                 and message["method"] in {"progress", "log"}
                                 and isinstance(message["params"], dict), "unexpected-trailing-output")
                            streams.notifications += 1
                            need(streams.notifications <= 64, "notification-cap")
                        report["notifications"] = streams.notifications
                if before is not None:
                    verification = time.monotonic() + limits.preparation
                    need(checked_seal(root, verification) == before, "seal-or-input-drift")
                    need(regular(snapshot, 1 << 20, verification) == snapshot_before, "snapshot-drift")
                    report["bindingsUnchanged"] = True
            except Exception as error:
                report.update(success=False, failure="cleanup-or-binding-unconfirmed")
                allowed = {"OSError", "PermissionError", "ProcessLookupError", "ValueError", "TypeError", "AttributeError", "TimeoutExpired", "BrokenPipeError", "Refused"}
                kind = type(error).__name__
                code = getattr(error, "errno", None)
                report["cleanupError"] = {"kind": kind if kind in allowed else "unknown", "errno": code if type(code) is int and 0 <= code <= 4096 else None}
            # Never take a buffered-pipe lock from a still blocked writer/reader.
            if not any(w.is_alive() for w in writers) and (streams is None or not any(t.is_alive() for t in streams.threads)):
                for pipe in (scope.process.stdin, scope.process.stdout, scope.process.stderr):
                    try: pipe.close()
                    except (OSError, ValueError): pass
    report["success"] = report["success"] and "failure" not in report
    return report
