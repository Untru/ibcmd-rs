#!/usr/bin/env python3
"""Capture a native XML reference from a fresh lab DB loaded with EDT's export.

Research only. No existing database, source tree or reference can be overwritten.
"""
import argparse
import contextlib
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

from oracle import OracleError, digest, no_links, require_xml, snapshot, write_json


def command(run, label, argv, timeout, allow_failed_probe=False):
    record = {"argv": [str(value) for value in argv], "timeout_seconds": timeout,
              "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())}
    started = time.monotonic()
    paths = [run / f"{label}.{suffix}" for suffix in ("stdout", "stderr")]
    with paths[0].open("xb") as stdout, paths[1].open("xb") as stderr:
        process = subprocess.Popen(record["argv"], cwd=run, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        while process.poll() is None:
            if time.monotonic() - started > timeout or any(
                    path.stat().st_size > 16 * 1024**2 for path in paths):
                subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"],
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                               check=False, creationflags=subprocess.CREATE_NO_WINDOW)
                process.wait(timeout=30)
                record["timeout_or_log_limit"] = True
                break
            time.sleep(0.5)
        record["exit_code"] = process.wait(timeout=30)
    record["duration_seconds"] = round(time.monotonic() - started, 3)
    write_json(run / f"{label}.command.json", record)
    if record.get("timeout_or_log_limit") or (record["exit_code"] and not allow_failed_probe):
        raise OracleError(f"{label} failed; see preserved logs in {run}")
    return paths[0].read_bytes()


@contextlib.contextmanager
def lock(args, label, name):
    prefix = ["pwsh", "-NoProfile", "-File", args.lock_script]
    command(args.run, f"{label}-acquire", prefix + ["acquire", "edt-native", "-Name", name,
            "-TimeoutMin", str(max(1, args.timeout // 60))], args.timeout)
    try:
        yield
    finally:
        command(args.run, f"{label}-release", prefix + ["release", "edt-native", "-Name", name], 60)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, required=True, help="Authentic installed-EDT XML export")
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--ibcmd", type=Path, required=True)
    parser.add_argument("--native-build", required=True)
    parser.add_argument("--restore-script", type=Path, required=True)
    parser.add_argument("--lock-script", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=21600)
    parser.add_argument("--probe-apply", action="store_true",
                        help="Record a separate activation probe; activation is not an import/export acceptance gate")
    args = parser.parse_args()
    created = False
    try:
        args.input, args.run = args.input.absolute(), args.run.absolute()
        if os.name != "nt" or args.run.drive.upper() != "F:" or args.input.drive.upper() != "F:":
            raise OracleError("Native references require disposable F: paths on Windows")
        for path in (args.input, args.run, args.ibcmd, args.restore_script, args.lock_script):
            no_links(path)
        if args.input.is_relative_to(args.run) or args.run.is_relative_to(args.input):
            raise OracleError("Input and run must not overlap")
        if not re.fullmatch(r"ibcmd_rs_04_edt07_[a-z0-9_]+", args.database) or args.timeout < 1:
            raise OracleError("Only fresh databases owned by this EDT lab are permitted")
        require_xml(args.input, require_dump_info=False)
        args.run.mkdir(parents=True, exist_ok=False)
        created = True
        source = Path(__file__).read_bytes()
        (args.run / "harness-source.py").write_bytes(source)
        write_json(args.run / "invocation.json", {
            **{key: str(value) if isinstance(value, Path) else value for key, value in vars(args).items()},
            "harness_sha256": hashlib.sha256(source).hexdigest()})
        before = snapshot(args.input)
        write_json(args.run / "input-before.json", before)
        binary_hash = digest(args.ibcmd)
        version = command(args.run, "native-version", [args.ibcmd, "--version"], 60)
        if args.native_build != version.decode("utf-8", errors="strict").strip():
            raise OracleError("Installed native build did not match the explicit profile")
        temporary = args.run / "tmp"
        temporary.mkdir()
        os.environ["TEMP"] = os.environ["TMP"] = str(temporary)
        data = args.run / "ibdata"
        common = ["--dbms=MSSQLServer", "--db-server=localhost",
                  f"--db-name={args.database}", f"--data={data}"]
        # The legacy helper enforces nonexistence, F: physical SQL files and the
        # shared ownership registry. It has no REPLACE path and never drops DBs.
        with lock(args, "fresh-database-heavy", "heavy"):
            with lock(args, "fresh-database", "native"):
                command(args.run, "fresh-database", ["pwsh", "-NoProfile", "-File", args.restore_script,
                        "-Corpus", "empty", "-Name", args.database, "-Track", "edt-native",
                        "-Purpose", "0.7 native reference from authentic installed-EDT export"], args.timeout)
        for label, operation, extra in (
                ("native-create", ["infobase", "create"], ["--locale=ru_RU"]),
                ("native-import", ["infobase", "config", "import"], [str(args.input)])):
            with lock(args, f"{label}-heavy", "heavy"):
                with lock(args, label, "native"):
                    command(args.run, label, [args.ibcmd, *operation, *common, *extra], args.timeout)
        activation = {"status": "NOT_REQUESTED"}
        if args.probe_apply:
            with lock(args, "native-apply-heavy", "heavy"):
                with lock(args, "native-apply", "native"):
                    command(args.run, "native-apply", [args.ibcmd, "infobase", "config", "apply",
                            *common, "--force", "--dynamic=disable"], args.timeout,
                            allow_failed_probe=True)
            probe = json.loads((args.run / "native-apply.command.json").read_text())
            activation = {"status": "PASS" if probe["exit_code"] == 0 else "FAIL",
                          "exit_code": probe["exit_code"], "command": "native-apply.command.json"}
        reference = args.run / "native-xml"
        with lock(args, "native-export-heavy", "heavy"):
            with lock(args, "native-export", "native"):
                command(args.run, "native-export", [args.ibcmd, "infobase", "config", "export", *common,
                        "--threads=4", reference], args.timeout)
        require_xml(reference)
        exported = snapshot(reference)
        write_json(args.run / "native-reference.json", exported)
        after = snapshot(args.input)
        write_json(args.run / "input-after.json", after)
        if after != before or digest(args.ibcmd) != binary_hash:
            raise OracleError("Immutable input or native executable changed")
        write_json(args.run / "result.json", {"status": "CAPTURED", "database": args.database,
            "native_build": args.native_build, "executable_sha256": binary_hash,
            "reference": str(reference), "reference_sha256": exported["tree_sha256"],
            "input_unchanged": True, "activation_probe": activation,
            "scope": "Fresh DB created and imported using authentic installed-EDT XML; loaded configuration exported by native ibcmd"})
        print(f"CAPTURED: {reference}", flush=True)
        return 0
    except (OracleError, OSError, ValueError, subprocess.SubprocessError) as error:
        if created:
            write_json(args.run / "failure.json", {"status": "FAIL", "error": str(error)})
        print(f"FAIL: {error}", file=sys.stderr, flush=True)
        return 1


if __name__ == "__main__":
    sys.exit(main())
