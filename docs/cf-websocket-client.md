# WebSocketClient CF conversion

The source implementation maps `WebSocketClients/<Name>.xml` to the existing
WebSocketClient configuration family. It supports the four authored booleans
`Predefined`, `AutoConnect`, `UseOSProxy`, `UseOSAuthentication`, the strings
`ServerURL`, `User`, `Password`, integer `Timeout`, and ordered string key/value
`Headers`. Header order and repeated keys are preserved. Headers use the readable
XML `ValueList` / `KeyAndValue` grammar; namespace prefixes may be renamed while
their expanded names stay valid. Unsupported types, check states, presentations,
unknown properties and malformed native counts are errors.

The optional owned module is `WebSocketClients/<Name>/Ext/Module.bsl`; it uses
the shared module container codec and the object's `<UUID>.0` storage entry.
The existing seven configuration groups are preserved, including the existing
WebSocketClient slot in the first group. The folder, object name, root reference
and module ownership participate in export and base-free reconstruction.

Build this checkout, then use:

```text
ibcmd-rs cf export input.cf source --source-version 2.20 --fail-on-opaque
ibcmd-rs cf bootstrap source rebuilt.cf --source-version 2.20 --base-free
ibcmd-rs cf export rebuilt.cf output --source-version 2.20 --fail-on-opaque
```

The same source route accepts XCF 2.21; cleanroom tests cover both source
profiles, current XML edits and exact module bytes. This feature is newer than
the published v0.4.0 archives. The older bootstrap route without `--base-free`
retains its separately declared service-family/profile admissions; this change
does not add a WebSocketClient admission to that route.

The measured native descriptor, scalar perturbations, populated Headers and
module container came from Windows platform 8.3.27.2214. Actual SDK equality of
the resulting product output, including a separate 8.5/XCF 2.21 measurement,
must be verified before declaring complete platform acceptance. Codec tests
alone do not establish native platform admission of arbitrary Timeout values
or whole-configuration parity.
