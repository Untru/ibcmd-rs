# Design

The drop-in parser owns native grammar and mutually exclusive base/sync. Export queries resolve the same connection and storage overlay as export. An inventory-only FileSink asks the common pipeline to emit ConfigDumpInfo before the body conversion batches. Native status compares version inventory, independently of the editor file-by-file status. Native objects selects descriptor/body rows with explicit owned-object recursion; custom object selection retains recursive semantics.

Evidence: 8.5.1.1592 on an unpacked repository tiny filedb. Native info stdout omits BOM, file retains BOM. Status stdout is text, --out writes BOM-prefixed text; unchanged file has BOM only. Reference removal and rename trigger `modified: all`. Native base+sync conflicts (rc2); unchanged --base creates no output. Full evidence is private external lab fixture issue362-preflight-001.

Additional real imports establish that both --base and --sync reject deleted/renamed objects (rc255, full export required); custom API fallback remains unchanged. Native --sync needs an existing export directory. Configuration --recursive includes top-level metadata, while shallow selection does not.

Native status and --base ignore a reference-only XML format header change: 2.20 reference with unchanged IDs/versions against 2.21 output gives empty status and a successful no-op export. The custom API still forces full conversion on dialect mismatch; its contract is independently regression-tested.

A mixed older XML header plus a changed stored module version also remains a two-file native delta (ConfigDumpInfo and module body). The drop-in export opts into version-based row selection; the custom API retains its existing XML dialect fallback. Read-only inventory rejects inflate/debug directories and runtime journals before entering the pipeline. Empty published CONFIG has a measured empty ConfigVersions inventory; nonempty malformed rows still fail.

The typed ConfigDumpInfo codec lives in ibcmd-xml: it owns streaming grammar validation, row/part XML types, attribute escaping and native BOM/CRLF/empty spelling. The physical adapter only extracts stored rows/versions and canonical routes, then supplies typed entries. Inventory capture belongs to the existing adapter inventory writer. No physical-policy baseline exemptions are added.
