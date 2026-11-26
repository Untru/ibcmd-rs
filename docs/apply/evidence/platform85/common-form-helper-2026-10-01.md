# Initial 8.5 CommonForm module-only helper (not publication admission)

The separate native control changes only the two exported marker literals of
existing CommonForm `_ДемоПримечание`, UUID
`a627e390-8fad-4a95-afe6-674f54813188`, on exact platform 8.5.1.1150. Native
partial import creates exactly five ConfigSave rows (descriptor, `.0`, root,
version, versions), all single-part/Attributes zero. Inflated descriptor and
version are unchanged; root is the previously measured opaque tag-2 restamp.
The form `.0` is complete raw DEFLATE yielding 8024 UTF-8 BOM bytes, tuple
container revision 4 and managed layout revision 59. It is not the CommonModule
V8 text/info container. All inflated bytes are equal except two `P85_FORM_A`
to `P85_FORM_B` literals inside the module string.

`dynamic_platform85::require_form_module_only_change` admits only this format
family on the exact profile. Complete DEFLATE StreamEnd, all-input consumption
and an 8 MiB inflated limit precede the existing form parser. The existing
module-only packer replaces its parsed module field in immutable active bytes;
the entire reconstructed inflated tuple must equal the staged bytes. This
binds every opaque byte outside canonical module quoting without a new codec.
The module must actually change. Three focused tests use native fixtures and
refuse another profile/revision/layout, changed non-module properties/trailer,
padding, missing BOM, every truncated prefix, trailing compressed bytes,
Sync-flushed complete content without final StreamEnd, invalid UTF-8 and size
overflow. This helper is not connected to dynamic apply in this checkpoint;
existing CommonForm admission stays closed. Physical stage/active headers,
initial history absence and full registration CAS remain caller requirements.
Actual focused results are three passed, zero failed. Mandatory debug quick
checks in `gates-form-helper` all pass (fmt, physical policy, clippy, root library
3691 passed/zero failed/ten ignored); `logs/form-helper-focused.log` retains the
native-fixture test output. No OWN form publication or matching product CLI
control is claimed by these helper tests.

Historical raw oracle under `F:/ibcmd/lab/05/wave3/platform85`:

- `snapshots/staged_form_b_{native,own}` are exact across six complete storage
  inventories/packs and eight auxiliary tables. Both are manifest-owned fresh
  clones; the OWN staged clone has not been applied.
- `logs/form-b-contract.json` records three NULL-message registrations and
  their three exact key-zero `.0` file lists. All eight auxiliary tables are
  unchanged by native force.
- `logs/form-b-warm-comparison.json` splits observer startup from publication.
  One opaque Files row grows 27 to 50 bytes during startup and remains exactly
  unchanged across force; `logs/form-startup-cache-headers.json` retains raw
  header/hash evidence. It is not classified as licensing or normalized away.
  Warm-to-force changes only measured Config aliases/root/version, Params
  marker and three licensing `.ui` rows, and Files.MobileVersions.dat. Raw
  licensing records are never decoded or written. OWN/native parity remains
  pending, so this list is an oracle, not an equality claim.
- Native force generation `018e9ec8-63cf-4aca-8e44-68e7fa5b60e9` is retained in
  `logs/native-form-b-force.log` and `snapshots/after_native_form_b_force`.
  `logs/form-b-native-session-assertions.json` validates every journal record:
  old SID1 has 298 A/A polls, new SID4 has 205 B/B polls, overlapping 206717 ms.
  This uses retained unopened form handles; it does not prove same-session
  refresh or a visible form's unsaved data preservation.
- `native-after-force-form-b` and `logs/form-b-native-source-diff.json` retain
  the actual native export. Input-to-export has 12334 byte-identical files and
  two differences: ConfigDumpInfo changes only two configVersion values, and
  native import inserts fourteen CR bytes before the input module's lone LF
  lines. Exact transformations are recorded in
  `logs/form-b-native-cdi-only-configVersion.json` and
  `logs/form-b-input-newline-delta.json`; raw module equality is not claimed.
  The eventual native/OWN final exports must still compare without module
  normalization.

The form observer was built with the exact native Designer from separate lab
source. EPF SHA256 is
`4b837d367da885c29e02ca35bae123335d118c83cc2f1820fe35c24b341bea7b`;
`logs/form-observer-build-identity.json` preserves compiler/source provenance.
BSL checks retain initial failures and a documented narrow GetFormMethod style
exception for the vendor-supported observation handle; final syntax checks have
zero errors. Before cleanup the registry/process census was saved; exact owned
clients were stopped, the registration removed, then the private kit verified
the retained process identity union and vacant listeners. No native/worker
lease remains. All five owned SQL clones are unregistered for subsequent
controls/default-60-minute cleanup. No reference database or other cluster was
modified. Native-only evidence does not enable forms, nested forms, extension
writes, repeated 8.5 overlays, LIVE or generalized worker support.
