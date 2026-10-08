# Form-root command-interface presence

The canonical `FormBody.command_interface` boolean is independent of the form's
automatic command bar. EDT can express an automatic bar with no command-interface
block, and an explicit empty command-interface block with no automatic bar.
Native XML's reader convention maps an automatic bar to `command_interface=true`;
an empty interface without a bar maps to false. Native XML alone cannot recover
which of these states was authored.

The form presence companion transports this boolean as the closed canonical
`FormRootPresence` type. It binds to the declared form UUID, exact XML profile and
SHA256 of the actual forward root (automatic bar including descendants, interface
boolean and ordered panels). It contains no XML text or retained input values.
Writing regenerates it from CURRENT typed data; editing the interface or bar
regenerates or removes the record as appropriate.

`prepare_form_presence` returns freshly emitted bytes and their companion together.
Plain `write_form` / `write_form_with_context` report a loss error when native's
root convention cannot retain the current boolean. Their supported root combinations
continue to emit ordinary XML. EDT's descriptor already preserves the boolean.
Populated panels require their canonical interface boolean to be true: contradictory
edited IR receives an error before panels can be silently dropped by EDT.

Whole-configuration native writing prepares an immutable `NativeFormWritePlan`
before publishing descriptors. The same plan supplies native form bytes, chart
companions and the form-root records in `ConfigDumpInfo.xml`. The manifest uses
`ibcmd-configuration-semantics:2:` and schema
`urn:ibcmd:source-extension:configuration-semantics:2` only when a new root record
is required. Its exact collections are `data_paths` (the existing closed v1
resources) and nonempty `form_presence`. Unaffected configurations retain the v1
manifest schema, field order and acceptance, or no annotation when none is needed.
Unknown versions, fields, duplicate roots, foreign profiles and empty v2 families
are rejected. V1 never accepts a new v2 key.

Standalone object-sidecar writers use `Ext/ibcmd-form-presence.v2.json` because
they have no configuration manifest owner. The same root cannot be declared in
both the manifest and sidecar. The public conversion reports extension use as
`ibcmd-form-presence/2`.

Reading validates the record against the original native artifact, then restores
other typed descendant resources. It applies the root boolean to an unpublished
clone, checks its actual CURRENT forward root, and publishes only after validation.
Whole-configuration completion verifies UUID ownership and CURRENT metadata again.
The canonical boolean survives ordinary IR serialization as well as both formats.

The original synthetic ACB-present / CI-absent failure is retained as
`availability-roundtrip-differences-3a9.json` in the laboratory. Its fixture
calibration did not repair this loss. The nonignored `form_root_presence` tests
cover that counterexample, the inverse explicit-empty case, both profiles 2.20
and 2.21, complete IR serde, bar/panel/interface edits, stale and malformed atomic
rejection, and whole-configuration conversion with rich DataPaths. The native
annotation owner additionally tests unchanged v1 bytes and strict mixed v2 sections.
These are candidate tests, not a claim that they have passed.

This source slice does not activate UseAlways deduplication, chart presence
projection or Gantt defaults. Their separate lossless contracts remain required.
Installed SDK import/export and CF survival of the typed companion are separate
acceptance gates; SDK-produced XML without a companion cannot reconstruct erased
authored history.
