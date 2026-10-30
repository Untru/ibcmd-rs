"""Synthetic protocol controls; no database, native process or filesystem fixture."""
import copy
import unittest
from verify_capture import verify


def transcript():
    h, source, target, binary, baseline = [str(i) * 64 for i in range(1, 6)]
    commit = "a" * 40
    bindings = {"infobase": "disposable-target", "sourceDir": "public-source", "versionBaselineDir": "public-reference"}
    def at(minute, second=0):
        return f"2026-10-24T00:{minute:02}:{second:02}Z"
    def capture(index, phase, minute, disposition="published"):
        evidence = {"basis": "native-config-version", "object": "CommonModule.Control", "objectVersion": "opaque-token", "baselineIdentity": baseline, "databaseSnapshot": h, "conclusive": True, "changed": False}
        response = {"id": index, "result": {"databaseSnapshot": h, "fullRequired": False, "objects": [{"object": "CommonModule.Control", "version": "opaque-token", "status": "same", "databaseVersionStatus": evidence}]}}
        if disposition == "cancelled":
            response = {"id": index, "error": {"code": -32800, "message": "cancelled"}}
        return {"phase": phase, "disposition": disposition, "startedAt": at(minute), "endedAt": at(minute, 1), "targetIdentity": target, "sourceBefore": source, "sourceAfter": source, "storageBefore": h, "storageAfter": h, "baselineIdentity": baseline, "view": "main", "treeRequest": {"method": "tree/children", "params": {"infobase": bindings["infobase"], "mainConfiguration": True}}, "currentGeneration": 2, "request": {"params": dict(bindings), "id": index, "generation": 1 if disposition == "discarded-stale" else 2, "method": "objects/status"}, "response": response}
    def consent(operation, authorized, completed):
        return {"operation": operation, "leaseId": "public-contract-lease", "targetIdentity": target, "sourceIdentity": source, "binarySha256": binary, "previewSha256": baseline, "explicit": True, "authorizedAt": at(authorized), "completedAt": at(completed)}
    return {"schema": 1, "fixture": {"kind": "public-clean-room", "sha256": h}, "product": {"commit": commit, "embeddedCommit": commit, "binarySha256": binary}, "lease": {"id": "public-contract-lease", "operator": "synthetic-operator", "scope": "disposable-sql-status", "exclusive": True, "validFrom": at(0), "expiresAt": at(30)}, "targetIdentity": target, "sourceIdentity": source, "baselineIdentity": baseline, "requestBindings": bindings, "consents": [consent("stage", 2, 3), consent("apply", 5, 6)], "captures": [capture(1, "before", 1), capture(2, "staged", 4), capture(3, "applied", 7), capture(4, "applied", 8, "cancelled"), capture(5, "applied", 9, "discarded-stale")]}


class Contract(unittest.TestCase):
    def reject(self, mutate, text):
        doc = copy.deepcopy(transcript())
        mutate(doc)
        with self.assertRaisesRegex(ValueError, text):
            verify(doc)
    def test_valid_is_explicitly_not_live_acceptance(self):
        self.assertEqual(verify(transcript()), {"captures": 5, "phases": ["applied", "before", "staged"], "liveAcceptance": False})
    def test_stale_publication(self):
        self.reject(lambda d: d["captures"][0]["request"].update(generation=1), "stale response published")
    def test_response_identity(self):
        self.reject(lambda d: d["captures"][0]["response"].update(id=99), "request id mismatch")
    def test_boolean_response_id(self):
        self.reject(lambda d: d["captures"][0]["response"].update(id=True), "request id mismatch")
    def test_object_token(self):
        self.reject(lambda d: d["captures"][0]["response"]["result"]["objects"][0]["databaseVersionStatus"].update(objectVersion="late-token"), "cache-token mismatch")
    def test_reference_identity(self):
        self.reject(lambda d: d["captures"][0].update(baselineIdentity="f" * 64), "reference binding mismatch")
    def test_database_snapshot(self):
        self.reject(lambda d: d["captures"][0]["response"]["result"].update(databaseSnapshot="f" * 64), "stale native evidence")
    def test_source_immutability(self):
        self.reject(lambda d: d["captures"][0].update(sourceAfter="f" * 64), "mutated source")
    def test_status_storage_immutability(self):
        self.reject(lambda d: d["captures"][0].update(storageAfter="f" * 64), "storage changed")
    def test_write_consent_not_implied_by_status(self):
        self.reject(lambda d: d["consents"][0].update(explicit=False), "preview consent required")
    def test_cross_target_consent(self):
        self.reject(lambda d: d["consents"][1].update(targetIdentity="f" * 64), "consent identity mismatch")
    def test_binary_identity(self):
        self.reject(lambda d: d["product"].update(embeddedCommit="b" * 40), "binary commit mismatch")
    def test_cancel_not_clean(self):
        self.reject(lambda d: d["captures"][3].update(disposition="published"), "error presented as successful")
    def test_unknown_not_clean(self):
        self.reject(lambda d: d["captures"][0]["response"]["result"]["objects"][0]["databaseVersionStatus"].update(conclusive=False), "unknown evidence")
    def test_full_required_not_conclusive(self):
        self.reject(lambda d: d["captures"][0]["response"]["result"].update(fullRequired=True), "fullRequired")
    def test_lease_bound(self):
        self.reject(lambda d: d["captures"][0].update(endedAt="2026-10-24T00:31:00Z"), "bounded lease")
    def test_capture_deadline(self):
        self.reject(lambda d: d["captures"][2].update(endedAt="2026-10-24T00:14:00Z"), "bounded lease")
    def test_operation_order(self):
        self.reject(lambda d: d["consents"][1].update(authorizedAt="2026-10-24T00:02:30Z"), "consent order")
    def test_no_writer_method(self):
        self.reject(lambda d: d["captures"][0]["request"].update(method="config/apply"), "cannot authorize writes")

    def test_missing_tokens(self):
        def mutate(doc):
            row = doc["captures"][0]["response"]["result"]["objects"][0]
            del row["version"]
            del row["databaseVersionStatus"]["objectVersion"]
        self.reject(mutate, "nonempty strings")
    def test_nonstr_tokens(self):
        def mutate(doc):
            row = doc["captures"][0]["response"]["result"]["objects"][0]
            row["version"] = True
            row["databaseVersionStatus"]["objectVersion"] = True
        self.reject(mutate, "nonempty strings")
    def test_empty_tokens(self):
        def mutate(doc):
            row = doc["captures"][0]["response"]["result"]["objects"][0]
            row["version"] = ""
            row["databaseVersionStatus"]["objectVersion"] = ""
        self.reject(mutate, "nonempty strings")
    def test_missing_params(self):
        self.reject(lambda d: d["captures"][0]["request"].pop("params"), "request binding mismatch")
    def test_wrong_infobase_param(self):
        self.reject(lambda d: d["captures"][0]["request"]["params"].update(infobase="another-target"), "request binding mismatch")
    def test_wrong_source_param(self):
        self.reject(lambda d: d["captures"][0]["request"]["params"].update(sourceDir="another-source"), "request binding mismatch")
    def test_wrong_baseline_param(self):
        self.reject(lambda d: d["captures"][0]["request"]["params"].update(versionBaselineDir="another-reference"), "request binding mismatch")
    def test_wrong_active_tree(self):
        self.reject(lambda d: d["captures"][0]["treeRequest"]["params"].update(mainConfiguration=False), "tree/status view")
    def test_wrong_tree_target(self):
        self.reject(lambda d: d["captures"][0]["treeRequest"]["params"].update(infobase="another-target"), "tree/status view")
    def test_wrong_active_capture(self):
        self.reject(lambda d: d["captures"][0].update(view="active"), "native status view")


if __name__ == "__main__":
    unittest.main()
