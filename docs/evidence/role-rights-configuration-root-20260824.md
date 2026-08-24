# Role Rights: the Configuration root object, 20260824

Status: closes the largest single missing-file class measured on ERP
Управление холдингом 3.2.12.6 (`i2-uh.parity.json`, base commit `1645b1c`):
1,682 of 4,236 `Roles/<Name>/Ext/Rights.xml` files (~40% of the whole corpus'
missing set) were not emitted. All 1,682 are roles whose Rights blob carries
an entry for the Configuration root object itself (`Configuration.<Name>`,
administrative/client-launch rights such as `ThinClient`, `WebClient`,
`Administration`). No previously-exact Rights.xml (199 of them, measured
before this change) carries a Configuration-root entry, so this code path had
never been exercised against real data.

## Root cause

Two independent gaps, both in `src/mssql_dump/role_rights.rs`:

1. **Two right UUIDs with no name.** `3762abec-3836-446a-83ce-3e05001bca8b`
   and `4df6d046-3bf8-4dda-991c-53ba664296a5` appear on the Configuration-root
   object of essentially every role that has one (1,679/1,679 and 1,089/1,679
   respectively) but were absent from `ROLE_RIGHT_NAMES`. `role_right_name`
   returned `None`, which propagated through `parse_role_right_pairs` and
   `parse_role_rights_blob` via `?`, failing the whole Rights blob closed —
   this alone explains all 1,682 missing files.
2. **A right whose default value is `true` was read as absent, not
   default.** Six "launch mode" rights (`MainWindowModeNormal`,
   `MainWindowModeWorkplace`, `MainWindowModeEmbeddedWorkplace`,
   `MainWindowModeFullscreenWorkplace`, `MainWindowModeKiosk`,
   `AnalyticsSystemClient`) are omitted from the blob entirely, not written
   `false`, whenever a role leaves them at their default. Every other
   Configuration-root right the parser already recognized defaults to
   `false` when absent, so the existing reader had no way to represent "true
   because it was never mentioned."

Fixing only (1) would have moved most of these roles from `missing` to
`differing` (the six rights, and any plain-false right, would render wrong or
not at all); both gaps had to close together to reach `exact`.

## What the corpus proved

Measured by decoding every Roles/*/Ext/Rights.xml-bearing element directly
from `1cv8.cf` in one pass (`ibcmd_v8::reader::StreamingReader`, the same
reader `cf inspect`/`cf extract` use — repeated single-element `cf extract`
calls cost ~30s each from re-verifying the whole container, so the corpus was
read once and each of the 1,881 roles' `<uuid>.0` element decoded from the
in-memory index) and comparing against the native
`Roles/<Name>/Ext/Rights.xml` files in `$S/cap/uh-r1/src`:

- **1,679/1,679** roles with a Configuration-root entry share one fixed
  19-member sequence of "always present" rights (`Administration`,
  `DataAdministration`, `UpdateDataBaseConfiguration`, `ExclusiveMode`,
  `ActiveUsers`, `EventLog`, `ThinClient`, `WebClient`, `MobileClient`,
  `ThickClient`, `ExternalConnection`, `Automation`,
  `TechnicalSpecialistMode`, the unnamed `3762abec…`, `SaveUserData`,
  `ConfigurationExtensionsAdministration`, `InteractiveOpenExtDataProcessors`,
  `InteractiveOpenExtReports`, `Output`), plus the six launch-mode rights
  either fully present (1,666/1,679) or fully absent (13/1,679) — never
  partial — plus the second unnamed right `4df6d046…` in 1,089/1,679.
- Whenever the six launch-mode rights are explicit, their value is `1`
  (true) in **every** occurrence (1,666/1,666, 0 counterexamples). Whenever
  absent, the native XML shows all six as `true`, always positioned
  immediately before `SaveUserData`.
- Both unnamed right UUIDs equal the role's own `setForNewObjects` flag in
  **every** occurrence (1,679 + 1,089 checks, 0 counterexamples), which is
  exactly the condition under which a Configuration-root right never
  prints (see next point) — they are structurally invisible regardless of
  which way the equality resolves.
- A Configuration-root right renders **iff its value differs from the
  role's own `setForNewObjects` flag** — not "iff true," as every other
  object kind's rights effectively behave once filtered. Only one role in
  the whole corpus sets `setForNewObjects: true` on a Configuration-root
  entry: the built-in `ПолныеПрава` (`fdb012c9-583a-42af-95da-070e75a58078`,
  "full access"). There, the convention inverts exactly as predicted: 9
  rights are explicit `false` and shown, the rest (including all six
  launch-mode rights, explicitly `true` in the blob) are hidden.
- A **Python re-implementation** of the parse+render rule above, run against
  the extracted plaintext of all 1,679 roles and compared field-by-field
  (name, value, order) to the real `Roles/<Name>/Ext/Rights.xml`, produced
  **1,679/1,679 exact matches, 0 mismatches, 0 fail-closed triggers** before
  a single line of Rust was written. This was the basis for the design in
  `parse_configuration_root_object_rights` and the Configuration branch of
  `role_rights_for_xml`.

Representative bytes (`1cv8.cf`, `cf extract`, `--compression raw-deflate`):

| Role | Element | unpacked bytes | unpacked SHA-256 |
| --- | --- | ---: | --- |
| `АдминистраторПроцесса` (Configuration-only, previously missing) | `1201f209-e625-41f9-9072-1eab727b7cf9.0` | 862 | `0cfeac45af1022d6b5c9520db567777b661618da6fe9d896f9aefc5fbfbc1a72` |
| `ВводИнформацииПоНоменклатуреБезКонтроля` (empty rights, previously exact) | `00799041-4f20-431d-ab5b-c39a864ce89a.0` | 45 | `73da4ddc5ed16b7ca8a30299dd680cc47c3e87f61f080efc2c274149d6a7b60f` |
| `ПолныеПрава` (`setForNewObjects: true`, previously missing) | `fdb012c9-583a-42af-95da-070e75a58078.0` | 2,356,643 | `81453d808b3791ab1db0556306df6a0f1e55824ae67a481e5b954f537e63b88f` |

The `45`-byte empty-rights blob's hash matches the "empty object table" row
already on record in `docs/evidence/rights-predefined-support-8.3.27.md` —
the same canonical zero-object blob is shared across every role that grants
no rights at all, Configuration-scoped or otherwise.

## The fix

`src/mssql_dump/role_rights.rs`:

- `parse_configuration_root_object_rights` — a dedicated parser for the
  Configuration-root object's plain-pairs rights list (restrictions are not
  a native concept here in any sampled role, so the restrictions blob shape
  `parse_role_object_rights` also accepts is refused as unproven). It
  tolerates the two unnamed right UUIDs only while their value equals the
  role's `setForNewObjects` flag (refuses/fails closed otherwise — the
  corpus proves the equality, not the name, so a divergent value is refused
  rather than rendered under a guessed name); it requires the six
  launch-mode rights to be either all present or all absent (refuses a
  partial set as an unproven shape); when absent, it synthesizes them as
  `true` and splices them into the vector immediately before `SaveUserData`,
  reproducing the platform's own canonical position.
- `role_rights_for_xml` now takes `&RoleRights` (not just the one object) and
  special-cases `is_configuration_root_rights_object` (`name.starts_with
  ("Configuration.")`) with a single rule — render iff
  `right.value != rights.set_for_new_objects` — replacing the former
  `should_omit_default_configuration_mode_rights`/`is_configuration_mode_right`
  heuristic pair, which had never matched a real Configuration-root object
  (0 of the 199 previously-exact Rights.xml files had one) and, per the
  corpus rewrite above, encoded the wrong rule: it hid true launch-mode
  rights only when *some other* right was plain-false, independent of
  `setForNewObjects`, rather than comparing every right to that flag
  directly.
- `parse_role_rights_blob` now parses `setForNewObjects` (and the other two
  role-level flags, and the restriction-template table) before the objects
  loop, since the Configuration-root parser needs the flag already known.

Two pre-existing tests assumed the old, disproven rule and were rewritten
rather than deleted, per this project's rule that a corpus rewrite decides
over standing reasoning:

- `format_role_rights_omits_default_configuration_mode_rights_when_only_admin_flags_remain`
  → split into
  `format_role_rights_configuration_root_shows_rights_that_differ_from_set_for_new_objects_false`
  and
  `format_role_rights_configuration_root_inverts_when_set_for_new_objects_true`,
  covering both directions of the corpus-proven rule directly.
- `writes_role_rights_to_source_layout`'s Configuration-object fixture used a
  single launch-mode right in isolation, a shape the corpus never produces
  (always 0 or 6 present together); it now uses two ordinary rights
  (`ThinClient`, `SaveUserData`) and asserts the six launch-mode rights are
  correctly synthesized-then-hidden end to end through the real pipeline.

Four new fixture tests exercise `parse_configuration_root_object_rights`
directly with literal blob text shaped after the real corpus samples above:
synthesis of all-six-absent, the `setForNewObjects: true` inversion, refusal
on a divergent unnamed right, and refusal on a partial launch-mode set.

## What is still refused, and why

**3 of the 1,682** (`БазовыеПраваБПУХ`, `ИспользованиеПлатежногоКалендаряУХ`,
`ЧтениеВекселей`) remain missing, unrelated to the Configuration-root defect:
their `.0` elements decode to 16,198,940 / 13,651,373 / 14,223,665 bytes of
plaintext respectively — roles with rights entries across many thousands of
objects — and exceed `MAX_NATIVE_NODES` (1,000,000) in
`src/compiler/families/native.rs`, a bound set from the largest node count
evidenced at the time (564,948, for a 2.27 MiB enterprise MXL). All three
roles here are confirmed by `ok`-parseable-but-oversized decode errors, not a
shape the parser gets wrong; raising a security-relevant bound needs its own
evidenced ceiling (how large a legitimate Rights blob gets, not just enough
headroom to silence these three), which this pass did not attempt. They fail
closed with `native Role Rights codec rejected data: native value exceeds
its node bound` rather than being force-fit under a raised limit chosen only
to match these three inputs.

## Verification

```text
cargo build --release
cargo test --lib role_rights            # 43 passed, 0 new failures
cargo test --lib                        # 2,210 passed / 33 failed; the 33 are
                                         # byte-identical (by name) to the
                                         # pre-existing $S/fail-base.txt set
```

Fast-gate parity (`$S/kit/run.sh <key> <worktree> <out>`), compared to
`$S/i2-<key>.parity.json` by exact-set difference, not counters — none of
these corpora contain a Configuration-root Rights.xml case, so they serve as
a no-regression check on the rest of the exporter, not as evidence for the
fix itself:

| key | exact (before → after) | broken |
| --- | --- | ---: |
| `ws` | 27 → 27 | 0 |
| `mdm` | 148 → 148 | 0 |
| `sslbase` | 9,569 → 9,569 | 0 |
| `ssl` | 12,634 → 12,634 | 0 |
| `wms` | 222 → 222 | 0 |

The full ERP УХ gate (140,411 files) is the decisive corpus for this fix and
is run separately; see the commit history for its exact-set result.
