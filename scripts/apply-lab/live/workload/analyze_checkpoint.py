"""Read-only wave3 checkpoint reconciliation, including unreturned COMMITs.

The owned SQL field/UUID layout is accepted only after exact agreement with
the independent, earlier COM readback of all final W2 documents.
"""
import json
import pathlib
import re
import sys

lab = pathlib.Path(sys.argv[1]).resolve()
assert lab.is_relative_to(pathlib.Path("F:/ibcmd/lab/05").resolve()), "analysis output must stay in the F 0.5 lab"
physical = {}
for line in (lab / "snapshots/f5-physical-documents.txt").read_text(encoding="utf-8-sig").splitlines():
    if not line.startswith("ibcmd-rs-load:"):
        continue
    mark, raw, posted = line.strip().split("|")
    assert re.fullmatch(r"[0-9A-F]{32}", raw), "invalid physical UUID"
    uuid = f"{raw[24:32]}-{raw[20:24]}-{raw[16:20]}-{raw[:4]}-{raw[4:16]}".lower()
    assert mark not in physical, "duplicate committed attempt"
    physical[mark] = {"uuid": uuid, "posted": posted}

com = {}
for line in (lab / "snapshots/committed-documents-v5.txt").read_text(encoding="utf-8-sig").splitlines():
    if line.startswith("ibcmd-rs-load:"):
        mark, uuid, posted = line.strip().split("|")
        assert posted == "True"
        com[mark] = uuid.lower()
base = {k: v for k, v in physical.items() if k.startswith("ibcmd-rs-load:base")}
assert len(com) == 1434 and set(com) == set(base), "physical mapping not validated against complete W2 COM proof"
assert all(v["posted"] == "01" and v["uuid"] == com[k] for k, v in base.items())

cohorts = []
observed = set()
for path in sorted((lab / "obs").glob("f5-*.log")):
    starts, returns, errors, operations = {}, {}, [], []
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        parts = line.split("|", 6)
        assert len(parts) == 7, "journal protocol"
        stamp, label, event, sid, client, server, detail = parts
        fields = dict(item.split("=", 1) for item in detail.split(";") if "=" in item)
        if event == "operation-start":
            assert fields["attempt"] not in starts
            starts[fields["attempt"]] = stamp
        if event == "operation":
            assert fields["attempt"] not in returns
            returns[fields["attempt"]] = fields
            if fields.get("committed") == "1":
                mark = f"ibcmd-rs-load:{label}:{fields['attempt']}"
                assert mark in physical and physical[mark]["posted"] == "01", "confirmed COMMIT absent/unposted"
                assert physical[mark]["uuid"] == fields["doc_uuid"].lower()
                assert mark not in observed
                observed.add(mark)
                operations.append({"utc_millis_year1": int(stamp), "client": client, "server": server,
                                   "report_ok": fields.get("report_ok"), "client_ms": fields.get("client_ms")})
        if "-error" in event or "-error=" in detail or fields.get("committed") == "unknown":
            errors.append(line)
    unresolved = []
    for attempt in sorted(set(starts) - set(returns), key=int):
        mark = f"ibcmd-rs-load:{path.stem}:{attempt}"
        unresolved.append({"attempt": attempt, "journal_result": "unknown", "physical": physical.get(mark)})
    cohorts.append({"label": path.stem, "confirmed": len(operations),
                    "reports": sum(x["report_ok"] == "1" for x in operations), "errors": errors,
                    "unreturned_attempts": unresolved, "operations": operations})
f5 = {k: v for k, v in physical.items() if k.startswith("ibcmd-rs-load:f5-")}
assert all(v["posted"] == "01" for v in f5.values())
result = {"physical_mapping_com_validated": len(com), "confirmed_journal_commits": len(observed),
          "posted_physical_commits": len(f5), "physical_commits_without_confirmed_journal":
          [{"mark": k, **v} for k, v in f5.items() if k not in observed], "cohorts": cohorts,
          "warm_readiness_established": False, "zero_error_claim": False}
(lab / "w3-checkpoint-summary.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
print(json.dumps({k: v for k, v in result.items() if k != "cohorts"}, ensure_ascii=False))
