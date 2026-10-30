"""Reviewed finite public stdio fixture; never an actual ibcmd/SQL adapter."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

CASES = ("ok", "hang", "stdout-cap", "stderr-cap", "early-error", "wrong-id",
         "nonzero", "malformed", "mutate-input", "mutate-seal", "descendant-hang", "missing-cancel", "stale-snapshot", "trailing-frame", "trailing-error",
         "trailing-malformed", "trailing-flood", "trailing-progress")


def read_frame():
    headers = {}
    while True:
        line = sys.stdin.buffer.readline()
        if not line:
            return None
        if line == b"\r\n":
            break
        name, value = line.decode("ascii").split(":", 1)
        headers[name.lower()] = value.strip()
    return json.loads(sys.stdin.buffer.read(int(headers["content-length"])))


def send(value):
    body = json.dumps(value, ensure_ascii=True).encode()
    sys.stdout.buffer.write(f"Content-Length: {len(body)}\r\n\r\n".encode() + body)
    sys.stdout.buffer.flush()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=CASES, required=True)
    parser.add_argument("command", choices=["serve"])
    parser.add_argument("--stdio", action="store_true", required=True)
    case = parser.parse_args().case
    # No case behavior, filesystem operation or child spawn before containment.
    if sys.stdin.buffer.readline() != b"STATUS_LAB_FIXTURE_V1\n":
        return 31
    if case == "descendant-hang":
        child = subprocess.Popen([sys.executable, "-I", "-S", "-B", "-c",
            "import signal,time; from pathlib import Path; signal.signal(signal.SIGTERM,signal.SIG_IGN); Path('descendant.ready').write_text('ready'); time.sleep(60)"],
            stdin=subprocess.DEVNULL, stdout=sys.stdout.buffer, stderr=sys.stderr.buffer)
        Path("descendant.pid").write_text(str(child.pid))
        ready_deadline = time.monotonic() + 1
        while not Path("descendant.ready").is_file():
            if time.monotonic() >= ready_deadline:
                return 39
            time.sleep(0.005)
    while True:
        request = read_frame()
        if request is None:
            return 37 if case == "nonzero" else 0
        if request["method"] == "$/cancelRequest":
            continue
        ident = request["id"]
        if ident == 1 and case == "hang":
            time.sleep(60)
        if ident == 1 and case == "stdout-cap":
            sys.stdout.buffer.write(b"X" * (2 << 20)); sys.stdout.buffer.flush(); time.sleep(60)
        if ident == 1 and case == "stderr-cap":
            sys.stderr.buffer.write(b"synthetic-private" * (32 << 10)); sys.stderr.buffer.flush(); time.sleep(60)
        if ident == 1 and case == "malformed":
            sys.stdout.buffer.write(b"Content-Length: 99999999\r\n\r\n{}"); sys.stdout.buffer.flush(); continue
        if ident == 1 and case == "early-error":
            send({"jsonrpc": "2.0", "id": ident, "error": {"code": -32014, "message": "synthetic-secret-NEVER-PUBLISH", "data": {"sql": "private-fixture-query"}}})
            continue
        if ident == 1:
            result = {"protocolVersion": "1.4", "capabilities": {"methods": ["tree/children", "objects/status"], "cancelRequest": True}}
        elif ident in (2, 3):
            result = {"nodes": [{"id": "Configuration", "kind": "Configuration"}]}
        elif ident == 6 and case != "missing-cancel":
            cancel = read_frame()
            if not cancel or cancel.get("method") != "$/cancelRequest" or cancel["params"]["id"] != 6:
                return 38
            send({"jsonrpc": "2.0", "id": 6, "error": {"code": -32800, "message": "cancelled"}})
            continue
        else:
            params = request["params"]
            if ident == 5 and case == "mutate-input":
                Path(params["sourceDir"], "module").write_bytes(b"fixture mutation")
            if ident == 5 and case == "mutate-seal":
                Path("seal.json").write_text("{}")
            row = {"object": "CommonModule.Control", "version": "opaque-token", "status": "same", "changed": [], "baseOnly": [], "localOnly": []}
            result = {"objects": [row]}
            if ident in (5, 7):
                import hashlib
                baseline = hashlib.sha256(Path(params["versionBaselineDir"], "ConfigDumpInfo.xml").read_bytes()).hexdigest()
                snapshot = ("d" if ident == 7 and case == "stale-snapshot" else "c") * 64
                row["databaseVersionStatus"] = {"basis": "native-config-version", "object": row["object"], "objectVersion": row["version"], "baselineIdentity": baseline, "databaseSnapshot": snapshot, "conclusive": True, "changed": False}
                result.update(databaseSnapshot=snapshot, fullRequired=False)
        send({"jsonrpc": "2.0", "id": 99 if case == "wrong-id" else ident, "result": result})
        if ident == 7:
            if case == "trailing-frame": send({"jsonrpc": "2.0", "id": 88, "result": {}})
            if case == "trailing-error": send({"jsonrpc": "2.0", "id": 88, "error": {"code": -32014}})
            if case == "trailing-malformed":
                sys.stdout.buffer.write(b"malformed trailing bytes"); sys.stdout.buffer.flush()
            if case in {"trailing-flood", "trailing-progress"}:
                for _ in range(65 if case == "trailing-flood" else 1):
                    send({"jsonrpc": "2.0", "method": "progress", "params": {"done": True}})


if __name__ == "__main__":
    raise SystemExit(main())
