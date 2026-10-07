# Native 8.5 signed root restamp

These 222-byte fixtures are the exact raw-deflate-inflated `root` bytes from
the fresh BSP85 pure-B module-only stage on platform 8.5.1.1150. They retain the
native BOM and CR/LF bytes. They are not decoded signatures or licensing data.

The active and staged rows use tag `2`, the same canonical root UUID, and a
128-byte opaque base64 payload. Its first 112 bytes are equal; only the final
16 bytes differ. The helper does not declare any write capability or accept
another payload size or platform build.

Historical lab provenance (2026-10-01):
`F:/ibcmd/lab/05/wave3/platform85/snapshots/staged_b_own/{Config,ConfigSave}.pack`
and the adjacent `inventory.json` retain the compressed bytes and complete
physical headers. `logs/pure-b-contract.json` records the native stage/twin
comparison. SHA-256 of these inflated fixtures:

- active: `886608f3fc22864506a0399ba623dc0d2d41c71f7a89e0a8ff352a15bc764f30`
- staged: `a827e300a3e396a77d19ad2f7e3e5aff4a751fcad02d56dcce2d752571534d47`

## Native CommonForm module-only stage

`form-{active,staged}.native.deflate` retain the complete native compressed
`.0` body of existing CommonForm `_ДемоПримечание`, UUID
`a627e390-8fad-4a95-afe6-674f54813188`, from the separate five-row native
stage on the same exact platform build. Both inflate to 8024 UTF-8 BOM bytes:
container revision 4, managed layout revision 59. Only the two module marker
literals change from `P85_FORM_A` to `P85_FORM_B`; every other inflated byte is
equal. These are form bodies, not licensing records or V8 text/info modules.

Raw provenance:
`F:/ibcmd/lab/05/wave3/platform85/snapshots/staged_form_b_native/{Config,ConfigSave}.pack`
and `inventory.json`; `logs/form-b-contract.json` and
`logs/form-b-warm-comparison.json` bind headers, registrations and native deltas.
The standalone helper does not admit forms into dynamic apply until separately
connected, tested against the owned twin and reviewed.

SHA-256 of the compressed native fixtures:

- active: `800758893dfb9cb11c1c14dda36d864aba2b804f9fba5a530f308c0e0add5abe`
- staged: `0f482784b8fca8365652d8aed9aaec5e342943db344cee1d92b9de211cb07032`
