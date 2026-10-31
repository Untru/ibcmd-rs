# Empty checks/triggers arrays: producer correction

Refs #359, #382. A measured public SQL 16.0.1000.6 source produced JSON null for
zero-row correlated checks/triggers projections. The strict metadata validator
correctly refused these values. Both producers now use
`JSON_QUERY(COALESCE((SELECT ... FOR JSON PATH,INCLUDE_NULL_VALUES),N'[]'))`.
Only those two wrappers change; populated columns, ordering, nullable definitions,
flags, visibility/identity checks, bounds and transaction behavior are preserved.
No validator or retained receipt normalization is involved.

## Actual public SQL evidence

Ordinary SSH/PowerShell with an existing dedicated nonpooled SqlClient connection:

- Public zero/one/two-row reproduction completed in 1.684 s. Original zero-row
  checks/triggers were literal JSON null; corrected values were exact empty
  arrays. Nonempty JSON contents/order/nulls/flags matched exactly. Independent
  metadata queries found zero check/trigger rows on both public fixture tables.
  Full-release SHA256:
  `a227096480191ded6f21c845fe31cf63b94566a8fb897f399a6a4fc87423c372`.
- Corrected snapshot completed in 1.956 s, ONE ExecuteReader, unchanged strict
  validator PASS. Config 8 parts / 3313 payload bytes; ConfigSave 0 parts / 0 bytes.
  Schema SHA256:
  `ada21b513de55c543ded7de724d13441523178d078d5f858c91dce0cac10b626`.
  New VM-retained receipt 21425 bytes SHA256:
  `4ee1c14f566d7a1eb41343748c8d77e1f9131c23e0a4c4571ce190637f0c2974`.
  Full-release SHA256:
  `6285d53ce3ff723eba8552f4ed6e8c96b9cbe2d1073d01d8eb489952a824e154`.
- Old 21433-byte refused receipt remains unchanged, SHA256:
  `7dc2f9c233eb07b43bb0ab5153175aa90c9512b5dc3c3a9aead3c2e33e59c3d1`.
  No raw receipt/payload/application source transferred, no DB writes, no native
  rerun, no automatic retry, connections cleaned up and full release confirmed.

Canonical snapshot SQL at this measurement checkpoint:
`e6d1ecbfe06cd2df605cf53066a425dd98058056a701b661786567c057056da6`.
Executed corrected capture SQL:
`42ecd9455b93ef67c80fe7a551e1ed1ccb2f7b30a612ca78a1724b376c3b8739`.
Historical observer contract reference `ae96e854...` is not the current canonical
source. The actual result reports both separately. Validators/wrappers remain
byte-identical to their reviewed versions.

## Regression and review

Two producer tests failed before the correction for missing empty-array fallback,
then passed. Four public fixture source tests cover exact populated oracles,
SELECT-only scope and default OPENJSON key/type observation (missing, duplicate,
string, scalar and object cannot masquerade as literal null).

Run the six self-contained committed source controls with:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/scripts -p test_snapshot_empty_arrays_producer.py
PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover -s tests/scripts -p test_public_empty_arrays_probe.py
```

Both pass. Existing local metadata harness controls plus these regressions total
42 PASS on Python 3.14.2 and 3.9.6; primary #382 independently repeated 42 PASS and
approved the exact two-wrapper diff, reconstructed capture equality and unchanged
validators. Fixtures retained externally, no teardown. SQL sources were actually
executed as above; Python source checks alone are not SQL-engine evidence.

## Acceptance limit

This is corrected public SQL metadata measurement. Product executed=false,
accepted profile=false and SQL write authority=false. It does not establish a
product reader/TLS/auth tuple, import/apply parity, native/our comparison or UT11
acceptance. Existing SQL comparison work continues separately under #382.
