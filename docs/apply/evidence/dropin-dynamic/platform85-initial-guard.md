# Initial 8.5 dynamic cohort guard

This checkpoint adds a fail-closed guard; it does not enable the platform
capability. The independent platform track must prove an own/native twin before
declaring `mssql.config.apply.dynamic` supported for 8.5. Main/exclusive,
extensions, worker switching and LIVE capabilities are unchanged.

The native pure-B oracle is retained under
`F:/ibcmd/lab/05/wave3/platform85`. Its exact staged twin contains five rows:
one existing CommonModule descriptor (semantically unchanged), its changed `.0`
body, `root`, unchanged `version`, and changed `versions`. The root has tag 2,
the same canonical configuration UUID, and a 128-byte opaque payload whose first
112 bytes are unchanged. The profile helper recognizes only this root restamp.
The independent structural checker still runs without filtering root reasons.

The module is a raw-deflate v8 container containing exactly one `info` and one
`text` element. The measured info bytes are UTF-8 BOM plus `{3,1,0,"",0}`;
the UTF-8 BOM text changes from the old marker to the new one. Inflation is
bounded to 8 MiB before parsing and must reach DEFLATE `StreamEnd` with every
compressed input byte consumed. EOF alone is not a completed stream. Unknown
records, missing/duplicate elements,
invalid UTF-8, a missing BOM, oversized expansion and unchanged text refuse.

This initial cohort requires absent Config/Params history markers, no Config
aliases, no staged `deleted`, and no Params aliases. It deliberately avoids the
8.3 SI collector: no `.si` aliases or `siVersions` writes. A locked Params alias
absence assertion covers concurrent insertion. CommonForm, nested objects,
templates, help, other suffixes, new objects, changed descriptors or Version,
and repeated generations remain refused on 8.5 even if later 8.3 coverage grows.

The oracle has three existing eligible owner registrations, all with NULL
`_MessageNo`, each with its existing `.0` file-list row at key zero. Native B
preserves every registration and file-list value. The guard refuses additions,
missing registrations, extra objects, appended files, other list layouts and
non-NULL touched message numbers. It reuses the complete registration/list
preimage and node eligibility guards; its synthetic existing-file input must
produce zero additions and is never sent to parity writes. The touched-owner
count and NULL condition are rechecked under locks before any write.

The existing stage, ordinary Config rows, owner metadata, history and overlay
digest/header assertions remain in the transaction. MobileVersions uses the
existing parity helper, subject to the platform track's own/native measurement.
The three native runtime `.ui` row differences are retained as a known gap;
there is no licensing decoder/writer and no complete Params parity claim.

Four focused tests cover cohort refusals and bounded module decoding, including
a sync-flushed complete v8 container without its final DEFLATE block and every
truncated prefix of the corresponding completed stream. The
mandatory quick checks are formatting, physical adapter policy, CI's standalone
workspace clippy command, and the root library suite. The deliberately broader
root-inclusive clippy attempt reports inherited errors and is retained in the
lab; it is not reported as a passing gate. Raw guard/test logs are in
`F:/ibcmd/lab/05/wave3/metadata/logs/gates-initial85-*`.
