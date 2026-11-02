#!/usr/bin/env python3
"""Pure preparation for one bounded SQL status probe; never spawn or connect."""
import hashlib
import json
import math
import re
from pathlib import Path

CANDIDATE = "2aff32c79d0b9fbe45d513693b27e52fc47457de"
API_BASE = "6ca6ade04ceee151f056e29acb5cd6deaaa9fbc4"
PROTOCOL = "1.4"
STDOUT_CAP = 1 << 20
STDERR_CAP = 16 << 10
HEADER_CAP = 8 << 10
NOTIFICATION_CAP = 64
HEX64 = re.compile(r"[a-f0-9]{64}\Z")


def need(value, message):
    if not value:
        raise ValueError(message)


def first_operator_plan(binding):
    """An instruction/data package, not permission to execute the operator step."""
    need(set(binding) == {"kind", "commit", "binarySha256", "receiptSha256", "lockSha256", "sourceTreeSha256", "cleanSource", "toolchain", "command"}, "only public build receipt fields allowed")
    need(binding["commit"] == CANDIDATE, "candidate commit differs")
    need(binding["kind"] == "immutable-build-receipt", "build receipt required")
    for key in ("binarySha256", "receiptSha256", "lockSha256", "sourceTreeSha256"):
        need(isinstance(binding[key], str) and HEX64.fullmatch(binding[key]), "exact build digest required")
    need(binding["cleanSource"] is True and binding["toolchain"] == "1.95.0", "clean pinned build required")
    need(binding["command"] == ["cargo", "build", "--locked", "--profile", "iter"], "reviewed build command required")
    return {
        "kind": "read-only-operator-plan", "executed": False, "liveAcceptance": False,
        "candidateCommit": CANDIDATE, "apiBase": API_BASE, "protocolVersion": PROTOCOL,
        "binaryBinding": {**binding, "command": list(binding["command"])},
        "firstAction": "Review an exclusive disposable-public-tiny SQL lease, exact immutable build receipt, settings identity and unchanged source/reference/storage fingerprints; refuse before starting if any item is absent.",
        "requiredDbMetadata": ["hashed target identity", "disposable ownership", "SQL Server product/version", "collation", "platform build/profile", "public fixture digest", "CONFIG/CONFIGSAVE row counts and exact fingerprints", "completed/incomplete stage markers", "active/main inventory digests", "unrelated control object identity", "lease owner/id/start/expiry"],
        "process": {"executable": "<verified-absolute-binary>", "argv": ["serve", "--stdio"], "shell": False,
                    "wallSeconds": 180, "requestSeconds": 20, "maxRequests": 7,
                    "stdoutBytes": STDOUT_CAP, "stderrBytes": STDERR_CAP,
                    "headerBytes": HEADER_CAP, "maxNotifications": NOTIFICATION_CAP,
                    "timeoutAction": "stop; close stdin, terminate only the owned child, reap within 5 seconds, then kill/reap only that child if necessary; retain artifacts; never retry automatically"},
        "settings": {"file": "<lease-bound-private-settings>", "agentWrite": False,
                     "auth": "private operator environment only; never in arguments, source or public receipts"},
        "requests": [
            {"id": 1, "method": "initialize", "params": {"protocolVersion": PROTOCOL, "settingsFile": "<lease-bound-private-settings>", "workspaceFolder": "<owned-run-root>"}},
            {"id": 2, "method": "tree/children", "params": {"infobase": "<bound-target>", "mainConfiguration": False}},
            {"id": 3, "method": "tree/children", "params": {"infobase": "<bound-target>", "mainConfiguration": True}},
            {"id": 4, "method": "objects/status", "params": {"infobase": "<bound-target>", "sourceDir": "<immutable-public-source>"}},
            {"id": 5, "method": "objects/status", "params": {"infobase": "<bound-target>", "sourceDir": "<immutable-public-source>", "versionBaselineDir": "<immutable-public-reference>"}},
            {"id": 6, "method": "objects/status", "params": {"infobase": "<bound-target>", "sourceDir": "<immutable-public-source>", "versionBaselineDir": "<immutable-public-reference>"}, "control": "send $/cancelRequest for id 6; require -32800 and no published success"},
            {"id": 7, "method": "objects/status", "params": {"infobase": "<bound-target>", "sourceDir": "<immutable-public-source>", "versionBaselineDir": "<immutable-public-reference>"}, "control": "retire its client generation while pending; discard the late response, never publish it"},
        ],
        "postconditions": ["exact handshake 1.4 and required methods/cancel capability", "active status pairs with active tree; native baseline status with main tree", "native snapshot/baseline/object/cache tokens agree across the pair", "historical unchanged baseline is valid", "source/reference and storage unchanged within every read interval", "no writer method, preview, stage/apply, native command, database creation/drop or access change executed"],
        "laterMutationOwner": "SQL primary only, after a separate supported stage/apply preview and explicit consent; this first probe never stages/applies",
    }


def decode_frames(raw):
    """Bounded synthetic decoder controls for a future collector, not that collector."""
    need(isinstance(raw, bytes) and len(raw) <= STDOUT_CAP, "stdout cap exceeded")
    messages, notifications = [], 0
    while raw:
        cut = raw.find(b"\r\n\r\n")
        need(0 <= cut <= HEADER_CAP, "header missing or exceeds cap")
        fields = raw[:cut].split(b"\r\n")
        lengths = [f.split(b":", 1)[1].strip() for f in fields if f.lower().startswith(b"content-length:")]
        need(len(lengths) == 1 and lengths[0].isdigit(), "one numeric Content-Length required")
        size = int(lengths[0])
        need(0 < size <= STDOUT_CAP, "body cap exceeded")
        raw = raw[cut + 4:]
        need(len(raw) >= size, "truncated frame")
        def object_members(pairs):
            result = {}
            for key, value in pairs:
                need(key not in result, "duplicate JSON member")
                result[key] = value
            return result
        def finite_json(_):
            raise ValueError("non-finite JSON value")
        def finite_float(text):
            value = float(text)
            need(math.isfinite(value), "non-finite JSON number")
            return value
        message = json.loads(raw[:size], object_pairs_hook=object_members, parse_constant=finite_json, parse_float=finite_float)
        need(isinstance(message, dict) and message.get("jsonrpc") == "2.0", "JSON-RPC object required")
        raw = raw[size:]
        if "id" not in message:
            notifications += 1
            need(notifications <= NOTIFICATION_CAP, "notification cap exceeded")
        messages.append(message)
        need(len(messages) <= 7 + NOTIFICATION_CAP, "message cap exceeded")
    return messages


def public_error(response, stderr):
    need(isinstance(stderr, bytes) and len(stderr) <= STDERR_CAP, "stderr cap exceeded")
    error = response.get("error") if isinstance(response, dict) else None
    code = error.get("code") if isinstance(error, dict) else None
    need(type(code) is int and code in {-32800, -32010, -32011, -32012, -32014, -32602, -32603}, "known error code required")
    # Never echo raw SQL/client messages, data, stderr, paths or identifiers.
    return {"errorCode": code, "category": "cancelled" if code == -32800 else "refused", "publishedStatus": False}


def verify_recorded_transition(before, staged, applied, *, complete=True, expected_added=()):
    """Check claimed inventories; does not manufacture or execute a SQL overlay."""
    need(type(complete) is bool, "stage completeness must be explicit boolean")
    for view in (before, staged, applied):
        need(set(view) == {"active", "main"}, "both views required")
        for names in view.values():
            need(isinstance(names, list) and len(names) == len(set(names)), "unique inventory required")
    need(staged["active"] == before["active"], "stage changed active inventory")
    if complete:
        need(expected_added and len(expected_added) == len(set(expected_added)), "exact expected added objects required")
        need(not (set(expected_added) & set(before["main"])), "expected addition already present")
        need(set(staged["main"]) == set(before["main"]) | set(expected_added), "known added object inventory differs")
        need(applied["active"] == applied["main"] == staged["main"], "apply did not preserve main inventory")
    else:
        need(staged["main"] == staged["active"], "incomplete stage was published")
    return {"recordingOnly": True, "liveAcceptance": False}


if __name__ == "__main__":
    # No CLI accepts arbitrary commands or private input. Output is a clearly
    # synthetic public plan until an operator supplies a reviewed build receipt.
    example = {"kind": "immutable-build-receipt", "commit": CANDIDATE, "binarySha256": "1" * 64,
               "receiptSha256": "2" * 64, "lockSha256": "3" * 64, "sourceTreeSha256": "4" * 64,
               "cleanSource": True, "toolchain": "1.95.0", "command": ["cargo", "build", "--locked", "--profile", "iter"]}
    plan = first_operator_plan(example)
    plan["syntheticExample"] = True
    root = Path(__file__).resolve().parents[2]
    plan["reviewedApiSha256"] = {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in ("src/server/mod.rs", "src/server/rpc.rs", "src/stored_objects.rs")}
    print(json.dumps(plan, indent=2))
