# Extension root Version: native and own twins, 2026-10-01

Base: `13d9c6dd08806d38ffc4386883b53db6f2e985a4`; branch
`feat/0.5-extension-version`. Platform **8.3.27.2214**, fresh BSP clones only.
The former loader refused a Version-only root descriptor before staging.
This adapter accepts an existing numeric dotted Version with the same number
of components, retaining every other root descriptor byte. Other supported
body edits can accompany it; the structural/data-bearing refusal is unchanged.

## Source versions and native shape

Actual source increments are **1.7.0.0 → 1.7.0.1 → 1.7.0.2**. The first edit
changes only Version; the second also changes the adopted no-base common-form
handler to After. BSL and data metadata are unchanged. `meta-edit` performs the
fixture Version edits. Both trees pass `cfe-validate` (13 checks each); its
companion automatic `meta-validate` does not recognize Configuration and is
recorded as a tool limitation, not a pass. No CFE was built or delivered.

The root has 11 fields, seven contained sections, a first section with two
fields, a 28-field payload declaring 25 family lists, and a 61-field properties
tuple. **68 is its layout discriminator, not the member count**: 60 members
follow it. Version occupies member 15. The storage identity must match the
logical root key. The complete active reader validates the class/adoption
layout before the adapter runs; a second complete export checks the proposed
source before any write. Unknown layouts, old-version mismatch, malformed
XML, additional root changes and a table-bearing descriptor change refuse.

Native import itself also reorders six adopted-property pairs and refreshes
an opaque footer. The adapter preserves those original bytes and changes only
Version. The native and own packed roots are therefore not claimed equal.
Native activation accepts each own proposal and reports its exact proposed CAS
root. Native row fixtures, hashes, measured shape and an honest native delta
(`equal_after_version_only: false`) accompany this proof.

## Measured acceptance

Every row below compares **all 464 source files**, with inventories equal and
no changed/missing/extra files. ConfigDumpInfo is checked separately: blanking
**only configVersion attribute values** makes the entire raw CDI byte-equal.
Each inventory has 430 such attributes; there are no other CDI differences.

| Route | Source Version | Staged / active native exports | Different CDI configVersion values |
| --- | --- | --- | --- |
| Native Version-only twin | 1.7.0.1 | 464/464 each | 176 each |
| Native Version + After twin | 1.7.0.2 | 464/464 each | 177 each |
| Own Version-only load + native activation | 1.7.0.1 | 464/464 each | 1 each |
| Own Version + After load + native activation | 1.7.0.2 | 464/464 each | 2 each |

Our active reader independently reproduces all 464 source files in both own
cases, with the same narrow CDI differences. Configuration.xml source hashes
and parsed Versions match on both sides of every report.

Own Version-only load executes with one compiled target, Configuration.xml,
431 staged rows, 569942 bytes, proposed root
`9cded519f8a07c2c868d0e776c18c928a163fd55`; native activation reports exactly
that generation (exit 0). Own Version + After executes with two targets,
Configuration.xml and `CommonForms/СвязанныеДокументы/Ext/Form.xml`, 431 rows,
569809 bytes, proposed root `6d2d205735de9804f0ce76a19081c46ead50a92d`;
native activation reports exactly that generation (exit 0).

All four extension baseline exports before the edits match 733 source files
and four raw CDI files exactly (the initial binary's reader is unchanged by
the later adapter shape correction). The final acceptance binary additionally
exports all four active extensions: all 733 source files match; the three
untouched extensions retain raw CDI equality and ServiceDesk retains only the
two expected configVersion value differences, Version 1.7.0.2.

A real Version + unsupported root Comment change refuses before staging,
ConfigCASSave count zero. An identical-tree reload explicitly refuses as
"no changes ... nothing to load", also with zero staged rows. The first own
attempt safely refused an incorrectly inferred layout field count before
writing; its error and zero-stage proof are preserved. The adapter was then
corrected to the measured shape above, rebuilt, and all positive own proofs
ran on the corrected binary. A separate initial TLS refusal without trust-cert
was infrastructure-only; the trust-cert retry proved the former root guard.

## Verification and evidence map

Final quick gates: **fmt, guard, clippy 1.95 and root library tests PASS**;
**3650 passed, 0 failed, 10 ignored**. Two focused regression tests exercise the
native root fixture/byte preservation and Version descriptor/refusal boundary.
The policy guard passes without a baseline adjustment. No full/release gate ran.

Compact evidence is in [extension-version-20261001](extension-version-20261001/):

* `compare_native_*`: native Version-only and Version + After staged/active twins.
* `compare_own_*`: own native staged/active and own-reader active source/CDI proof.
* `compare_baseline_verified_*` and `compare_baseline_final_*`: all four extensions.
* `own-load-version1.json`, `own-load-version2-after.json`: executed staging reports.
* `native_apply_*`: four actual native activation logs, including own CAS generations.
* `native-root-*`, `native-row-hashes.json`: measured layout, delta, original row hashes.
* `negative-root-*`, `before-fix-refusal.txt`, `own_load_version1_shape_refusal.log`,
  `shape_refusal_staged_count.txt`, `own_load_version2_after_noop.json`,
  `noop-staged-count.txt`: honest refusals and zero-write evidence.
* `cfe_validate_*`, `quick-gates*`, `binary-hash.json`, `cleanup-final.log`: checks,
  acceptance binary SHA256, and guarded cleanup result.

Full raw trees, SQL row dumps, commands, scripts, gate logs and preserved binary:
`F:\ibcmd\lab\05\wave2\extensions`. Corrected acceptance executable is
`ibcmd-rs-version.exe`, SHA256
`F6A3E1DE2A3AA747D6842607EC430D982B1494D6A9E189AA2025031630597C35`.
Its production source is the final adapter; no later production edit occurred.

Only owned clones `ibcmd_rs_05_ext_wave2_native20261001` and
`ibcmd_rs_05_ext_wave2_own20261001` were written. Own registration is removed;
native was never registered. All native/heavy tickets are released. The shared
60-minute cleanup guard currently skips both recent writes; their manifest and
cleanup log remain for coordinator cleanup. Original corpora and worktrees are
untouched; build cache remains for coordinator dependency reuse.

## Limits

This proves the measured 8.3.27 layout and existing numeric dotted Version
format, including supported body edits. It does not enable arbitrary root
properties, Version plus root child-list edits, unknown layouts/formats,
data-bearing metadata/schema changes, 8.5 extension writes or new drop-in
capabilities. It does not claim complete closure of #348 or a delivered CFE.