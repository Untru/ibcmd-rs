# Settled 8.5 extension baseline preserved for a future control

One owned COPY_ONLY backup preserves the settled ServiceDesk baseline after the
[failed Version control and exact cleanup](extension-version-failed-2026-10-02.md).
This is baseline preservation, not another import or an installed Version change.
No restore, registration, native command, worker lifecycle or product CLI ran.
Product extension writes remain unsupported; no CFE was built. Issue #346 remains
incomplete.

The source was `ibcmd_rs_05_p85_w3_ext_version_native_20261001`, created locally
on 2026-10-01 at 21:34:46 and last unregistered on 2026-10-02 at 00:47:14.
Before every bounded child, the driver checked the exact ownership record,
unregistered state, stopped private cluster, absent saved producers and processes,
ports, child-completion authority, immutable dependencies and effective executable
resolution. SQL checks required an idle database with no target transactions or
other sessions, ConfigSave=0 and ConfigCASSave=0. The default 60-minute cleanup
policy was unchanged; this preservation grants no source retention exemption.

## Actual backup and physical comparison

The single [backup command](extension-baseline-preservation/one-copy-only-backup.command.json)
used COPY_ONLY, CHECKSUM, COMPRESSION, one destination, BUFFERCOUNT=4 and
MAXTRANSFERSIZE=65536. It refused an existing destination and completed with
exit code zero in 9.599 seconds, under a 90-second deadline. The file remains at
`F:/ibcmd/lab/05/wave3/platform85/baselines/extension-settled-A-20261002.bak`:
512,516,096 bytes, SHA256
`b60985a24206b9146fcd8c45a0c89b999ece63135cc3bee28450b31344715690`.
The separately bounded [VERIFYONLY receipt](extension-baseline-preservation/backup-verifyonly.json)
also reports exit code zero with CHECKSUM. Neither check proves a future restore.

The [actual HEADERONLY rows](extension-baseline-preservation/backup-header-rows-readback-v5.json)
contain exactly one set: the exact source database, BackupType=1, Position=1,
IsCopyOnly=true, HasBackupChecksums=true and IsDamaged=false. SQL records backup
start/finish as 2026-10-02 01:23:30/01:23:40 and BackupSetGUID
`964d9d0b-d53e-46e8-8831-723fc736f9bd`.

Full captures immediately before and after preservation both equal the accepted
postfailure baseline across all six configuration-storage tables and eight
auxiliary row multisets. Offline checks bind whole-pack and per-row range SHA256,
multipart group totals, raw bytes and all captured headers, including dates,
Attributes and DataSize. No normalization is applied. The 29,721 storage rows
contain 244,366,228 raw bytes. This comparison covers configuration storage and
the selected auxiliary tables, not every business table. Original captures are
`snapshots/ext_version_settled_backup_before_v5` and
`snapshots/ext_version_settled_backup_after_v5` under the lab root; the repository
keeps the corresponding bounded comparison receipts.

## Retained evidence error and correction

The original header reader validated the backup and wrote its rows to
`backup-header.json`. The generic child wrapper then overwrote that same filename
with its exit-zero command receipt. The [original receipt](extension-baseline-preservation/backup-header.json)
and frozen driver are retained unchanged: that receipt is not raw header evidence.
An actual-wrapper [RED/GREEN regression](extension-baseline-preservation/header-label-RED-GREEN.log)
reproduces the collision and proves distinct output/result labels preserve both.

A separate bounded, read-only HEADERONLY extraction then wrote fresh
`backup-header-rows-readback-v5.json`, with a distinct
[command receipt](extension-baseline-preservation/backup-header-readback-v5.json).
It validates the same backup; there was no second backup or source mutation.
The original sqlcmd informational Russian stdout contains replacement characters
from the accepted helper's decoding. Those receipts are unchanged; ASCII identity
fields, structured header rows, checksum verification and full physical captures
provide the stated evidence.

## Immutable checkpoint and remaining work

The [51-member checkpoint](extension-baseline-preservation/checkpoint.json), SHA256
`a88ec100aa9527b288c6672f27959f0ce752c6bb5b4a9152f44e68f36909a828`,
binds the backup, commands, results, both full captures, frozen preparation and
the evidence correction. Selected repository copies retain exact original bytes;
[copies.json](extension-baseline-preservation/copies.json) maps their paths and
hashes. Large packs and the backup remain in the lab. Copied scripts are evidence,
not new runnable entry points. The original driver/header paths and frozen lab
context remain authoritative.

Root and an independent peer recomputed all selected raw bytes and headers and
accepted the checkpoint. Their records are
`F:/ibcmd/lab/05/wave3/coordinator/review-resource-and-preservation.json` and
`review-p85-P0-A88EC100.json`. The execution used source reference
`7688a9feb2f2e3e7a4b752f67d4606baddce8940`; this later documentation commit does
not claim another execution.

A future full-tree native control is preparation only: a fresh owned clone,
the same 633-file baseline and a fixture changing only Version 1.8.3.0 to
1.8.3.1. It needs a new complete closure, independently reviewed cleanup and
fresh execution authorization. There is no automatic retry, reference bypass,
new metadata, product extension capability or CFE acceptance from this backup.
