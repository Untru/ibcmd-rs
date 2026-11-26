# Bind dynamic judgment to complete physical headers

The dynamic admission inventory judges ConfigSave and ordinary Config rows before the online engine captures their transactional image. Comparing only key, size and SHA256 permitted an Attributes, Creation or Modified change between those reads: the engine would lock the later header, after admission had accepted the earlier one.

`require_judged_image` now compares all three physical header fields exactly, as well as the existing complete key set, DataSize, payload length and SHA256. Both inventories use SQL style121 date strings. The comparison runs immediately after capture and before engine preparation, recovery artifacts or publication SQL. Full-row transactional CAS remains in place; mismatch refuses without a publication attempt. No runtime scope, body kind, default activation policy or platform capability is widened.

Dependency: the separately reviewed complete activation row capture (upstream0197b39a) preserves actual headers in MainStorageRow; synthetic empty dates are not a supported substitute.

The focused regression covers ConfigSave and Config independently, with deleted, root and a module body, and each header changed alone while bytes and sizes stay identical. It fails against the prior guard and passes with this change. The existing deleted-content drift regression continues to cover payload and inventory changes. Raw logs: F:/ibcmd/lab/05/wave3/metadata/logs/test-header-drift-{red,green}.log. Final quick gates and immutable source are recorded in the laboratory checkpoint; no database/client operation is required or claimed by this repair.
