#!/usr/bin/env python3
"""Validate a sanitized, bounded SQL-status transcript; never execute operations."""
import argparse
import datetime as dt
import json
from pathlib import Path
import re

MAX_BYTES = 1 << 20
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
HEX40 = re.compile(r"[0-9a-f]{40}\Z")


def require(ok, message):
    if not ok:
        raise ValueError(message)


def stamp(value):
    require(isinstance(value, str), "timestamp must be text")
    parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    require(parsed.tzinfo is not None, "timestamp requires timezone")
    return parsed


def digest(value):
    return isinstance(value, str) and HEX64.fullmatch(value) is not None


def verify(doc):
    require(isinstance(doc, dict), "transcript must be a JSON object")
    try:
        return _verify(doc)
    except (AttributeError, KeyError, TypeError, IndexError, RecursionError):
        raise ValueError("malformed transcript shape") from None


def _verify(doc):
    require(type(doc.get("schema")) is int and doc["schema"] == 1, "unknown transcript schema")
    require(doc["fixture"]["kind"] == "public-clean-room", "public fixture required")
    require(digest(doc["fixture"]["sha256"]), "fixture digest required")
    product = doc["product"]
    require(HEX40.fullmatch(product["commit"]) is not None, "exact product commit required")
    require(product["embeddedCommit"] == product["commit"], "binary commit mismatch")
    require(digest(product["binarySha256"]), "binary digest required")
    lease = doc["lease"]
    require(lease["scope"] == "disposable-sql-status" and lease["exclusive"] is True,
            "exclusive disposable SQL lease required")
    require(bool(lease["id"]) and bool(lease["operator"]), "lease identity required")
    start, expiry = stamp(lease["validFrom"]), stamp(lease["expiresAt"])
    require(0 < (expiry - start).total_seconds() <= 3600, "lease must be bounded to one hour")
    target, source = doc["targetIdentity"], doc["sourceIdentity"]
    require(digest(target) and digest(source), "sanitized target/source identity required")
    consents = doc["consents"]
    require(isinstance(consents, list) and len(consents) == 2, "exact stage and apply consents required")
    require({c["operation"] for c in consents} == {"stage", "apply"}, "consent operations differ")
    for c in consents:
        require(c["leaseId"] == lease["id"] and c["targetIdentity"] == target
                and c["sourceIdentity"] == source and c["binarySha256"] == product["binarySha256"],
                "consent identity mismatch")
        require(c["explicit"] is True and digest(c["previewSha256"]), "exact preview consent required")
        require(start <= stamp(c["authorizedAt"]) < stamp(c["completedAt"]) <= expiry,
                "consented operation outside lease")
    by_operation = {c["operation"]: c for c in consents}
    require(stamp(by_operation["stage"]["completedAt"]) < stamp(by_operation["apply"]["authorizedAt"]),
            "stage/apply consent order differs")
    bindings = doc["requestBindings"]
    require(isinstance(bindings, dict), "request bindings must be a JSON object")
    require(set(bindings) == {"infobase", "sourceDir", "versionBaselineDir"}, "native status bindings required")
    require(all(isinstance(v, str) and re.fullmatch(r"[A-Za-z0-9_.-]{1,64}", v) for v in bindings.values()),
            "sanitized logical request bindings required")
    require(digest(doc["baselineIdentity"]), "bound reference digest required")
    captures = doc["captures"]
    require(isinstance(captures, list) and 5 <= len(captures) <= 24, "bounded capture set required")
    ids, phases, dispositions = set(), {}, set()
    for capture in captures:
        phase, disposition = capture["phase"], capture["disposition"]
        require(phase in {"before", "staged", "applied"}, "unknown phase")
        require(disposition in {"published", "discarded-stale", "cancelled", "failed"}, "unknown disposition")
        began, ended = stamp(capture["startedAt"]), stamp(capture["endedAt"])
        require(start <= began <= ended <= expiry and (ended - began).total_seconds() <= 300,
                "status interval outside bounded lease")
        require(capture["targetIdentity"] == target, "status target changed")
        require(capture["sourceBefore"] == capture["sourceAfter"] == source, "status mutated source")
        require(digest(capture["storageBefore"]) and capture["storageBefore"] == capture["storageAfter"],
                "storage changed within read-only status interval")
        if phase == "before":
            require(ended < stamp(by_operation["stage"]["authorizedAt"]), "before capture follows stage consent")
        else:
            operation = "stage" if phase == "staged" else "apply"
            require(stamp(by_operation[operation]["completedAt"]) < began, "status precedes consented operation")
            if phase == "staged":
                require(ended < stamp(by_operation["apply"]["authorizedAt"]), "staged capture follows apply consent")
        request, response = capture["request"], capture["response"]
        require(request["method"] == "objects/status", "status transcript cannot authorize writes")
        require(request.get("params") == bindings, "status request binding mismatch")
        require(capture["baselineIdentity"] == doc["baselineIdentity"], "reference binding mismatch")
        require(capture["view"] == "main", "native status view mismatch")
        require(capture["treeRequest"] == {"method": "tree/children", "params": {"infobase": bindings["infobase"], "mainConfiguration": True}},
                "tree/status view or target mismatch")
        require(type(request["id"]) is int and request["id"] not in ids, "unique integer request id required")
        ids.add(request["id"])
        require(type(response["id"]) is int and response["id"] == request["id"], "response request id mismatch")
        generation, current = request["generation"], capture["currentGeneration"]
        require(type(generation) is int and type(current) is int and 0 <= generation <= current,
                "invalid generation")
        if generation < current:
            require(disposition == "discarded-stale", "stale response published")
        else:
            require(disposition != "discarded-stale", "stale control did not retire generation")
        dispositions.add(disposition)
        if "error" in response:
            require(disposition in {"cancelled", "failed"}, "error presented as successful status")
            require("result" not in response, "response has error and result")
            if disposition == "cancelled":
                require(response["error"]["code"] == -32800, "typed cancellation required")
            continue
        require(disposition in {"published", "discarded-stale"}, "cancelled/failed response has result")
        result = response["result"]
        require(digest(result["databaseSnapshot"]), "native snapshot required")
        require(digest(capture["baselineIdentity"]), "exact baseline identity required")
        require(type(result["fullRequired"]) is bool, "fullRequired must be boolean")
        require(isinstance(result["objects"], list) and 0 < len(result["objects"]) <= 64,
                "bounded nonempty object set required")
        objects = set()
        for row in result["objects"]:
            require(isinstance(row["object"], str) and row["object"] and row["object"] not in objects,
                    "unique named object required")
            objects.add(row["object"])
            require(row["status"] in {"same", "modified", "notExported", "notInBase"}, "unknown byte status")
            evidence = row["databaseVersionStatus"]
            require(evidence["basis"] == "native-config-version" and evidence["object"] == row["object"],
                    "native evidence object/basis mismatch")
            require(evidence.get("objectVersion") == row.get("version"), "native evidence cache-token mismatch")
            if evidence.get("conclusive") is True:
                require(isinstance(row.get("version"), str) and bool(row["version"])
                        and isinstance(evidence.get("objectVersion"), str) and bool(evidence["objectVersion"]),
                        "conclusive cache tokens must be nonempty strings")
            require(evidence["baselineIdentity"] == capture["baselineIdentity"]
                    and evidence["databaseSnapshot"] == result["databaseSnapshot"], "stale native evidence")
            require(type(evidence["conclusive"]) is bool, "conclusive must be boolean")
            if result["fullRequired"]:
                require(not evidence["conclusive"], "fullRequired presented as conclusive")
            if evidence["conclusive"]:
                require(type(evidence.get("changed")) is bool, "conclusive native change must be boolean")
            else:
                require("changed" not in evidence, "unknown evidence presented as clean/changed")
        if disposition == "published":
            require(phase not in phases, "ambiguous duplicate published phase")
            phases[phase] = capture
    require(set(phases) == {"before", "staged", "applied"}, "all three published phases required")
    require({"cancelled", "discarded-stale"} <= dispositions, "cancel/stale controls required")
    # A stage/apply owner must independently establish the intended changes.
    # This verifier binds evidence; it neither computes UI labels nor invents an oracle.
    return {"captures": len(captures), "phases": sorted(phases), "liveAcceptance": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("transcript", type=Path)
    args = parser.parse_args()
    try:
        with args.transcript.open("rb") as stream:
            raw = stream.read(MAX_BYTES + 1)
        require(len(raw) <= MAX_BYTES, "transcript exceeds 1 MiB")
        result = verify(json.loads(raw))
    except (OSError, ValueError, KeyError, TypeError, AttributeError, RecursionError) as error:
        parser.exit(1, f"status transcript refused: {type(error).__name__}: {error}\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
