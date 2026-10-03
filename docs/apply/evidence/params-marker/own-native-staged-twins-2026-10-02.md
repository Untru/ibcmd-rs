# OWN Comment/Synonym Apply from native staging

The current OWN executable collects the 156-byte `Params.DynamicallyUpdated`
marker in both measured Comment-only and Synonym-only cases. Native Apply also
collects it in these cases. This establishes marker agreement for these two
staged inputs; #418 remains open for the remaining matrix and retention rule.

Both experiments ran on 2 October 2026 on SQL Server and platform 8.3.27.2214.
Each used one fresh clone of its exact already-native-STAGED backup, followed
by one OWN exclusive `infobase config apply --force --dynamic=disable`.
There was no import, stage compilation, native Apply, new backup, worker or
client cohort in either OWN experiment.

## Inputs and completed commands

The producer is the previously verified release built from integration source
`6d2ff0171466634a4b633d5aedae0e22b93ea3ce`: 46,396,416 bytes, SHA256
`A60C30A1D62B494F0078BC689E833F32C9D749482F70E5FF387675ECC6FB9930`.
It does not contain the later template physical-preservation repair.

| Case | Exact native-STAGED backup SHA256 | Backup bytes | Original OWN controller | Known-zero results |
| --- | --- | ---: | --- | ---: |
| Comment | `59A4CEE54B4D852CEFC9AD02D9072CE5CEDC2EC9E80EA1A0245A9AF248F8694A` | 254439424 | PID 90840, birth `2026-10-02T21:22:58.0093925Z` | 50 |
| Synonym | `CB388C83AB58C9A9ACEA27FC362440C839871856407D8C5EAF1E4AD7B18CC0D4` | 254439424 | PID 81060, birth `2026-10-02T21:23:36.2102097Z` | 50 |

Both original controllers returned 0. Their original stdout and stderr pipes
completed; terminal evidence was saved before the final closure check. Each
case acquired and released its exact native FIFO lease once. Final states
record no uncertain child, potential restore, potential native lease, held
lease or cleanup operation. The restored databases and raw evidence remain
on F under the laboratory retention policy.

The complete typed backup headers, immutable backup bytes, sole restore
lineage, fresh database GUID/family/fork/create-time and ownership ledger row
were checked. The new database GUID was measured rather than assumed to equal
the source GUID. Four complete pre-Apply images—initial, after VERIFYONLY,
before queueing, and after the native grant—equal their corresponding native
STAGED input in all stored headers and bytes across six storage tables and
ten auxiliary multisets. Only the declared outer database names differ.

## Measured outcomes

The five native-staged payloads were transferred into `Config`, and
`ConfigSave` became empty. The marker present before Apply had 156 bytes and
was absent afterward in both cases.

| Comparison | Comment | Synonym |
| --- | --- | --- |
| OWN `Params`, excluding the removed marker | 38 rows unchanged in all headers and bytes | 36 rows unchanged; one `.si` row and `siVersions` changed |
| OWN versus native `Config` | All 9841 physical rows equal | All 9841 physical rows equal |
| OWN versus native `ConfigCAS` | All 12797 physical rows equal | All 12797 physical rows equal |
| OWN versus native `ConfigSave` / `ConfigCASSave` | Both empty | Both empty |
| OWN versus native ten auxiliary tables | Complete multisets equal | Complete multisets equal |
| OWN versus native `Params` | 20 rows differ; 18 equal | 20 rows differ; 18 equal |
| OWN versus native `Files` | `MobileVersions.dat` differs; 352 rows equal | `MobileVersions.dat` differs; 352 rows equal |

The Synonym OWN changes are exactly
`1a621f0f-5568-4183-bd9f-f6ef670e7090.si` and `siVersions`. The `.si` change
includes payload size and bytes; it is not classified as a timestamp-only
effect. The complete OWN/native differences, including `MobileVersions.dat`
payload and timestamps and all Params SI/UI/service rows, remain in the
science reports. No normalization, blanket cache exclusion, full Params
physical equivalence, compiler parity or session behavior is claimed.

## Evidence and independent review

The evidence root is `F:/ibcmd/lab/05/wave3`.

Byte-identical review copies are included here: [Comment terminal certificate](own-native-staged-twins-v1/OWN-comment-native-staged-terminal-certificate-root-v1.json),
[Synonym terminal certificate](own-native-staged-twins-v1/OWN-synonym-native-staged-terminal-certificate-root-v1.json),
[independent scientific review](own-native-staged-twins-v1/review-OWN-native-staged-science-synonyms-v1.json),
[Comment science](own-native-staged-twins-v1/own-native-staged-comment-v1-science.json) and
[Synonym science](own-native-staged-twins-v1/own-native-staged-synonym-v1-science.json).
Large raw packs and original command receipts stay at their pinned laboratory
paths; the JSON copies make the measured differences and audit references
available to a repository reviewer.

| Artifact under that root | SHA256 |
| --- | --- |
| `coordinator/OWN-comment-native-staged-terminal-certificate-root-v1.json` | `4CB987AB31F5A4C3E9325360AA9B41B2DE29C570C8FC7E2DB39705525BCC2A0D` |
| `coordinator/OWN-synonym-native-staged-terminal-certificate-root-v1.json` | `2D996C3F762E53907B9644B5E5C3D0426C57381753163CE2E1F1724D6FC13B1B` |
| `coordinator/review-OWN-native-staged-science-synonyms-v1.json` | `F4B4D31824924DEE43EEFCA9B1DCB8A57DC8705E7340DA4DED41F9BD6B6640BE` |

Each root terminal certificate pins 153 actual evidence files, including all
five new full six-table/ten-auxiliary snapshots, command results and original
observer outputs. The separate 74-input controller closure and original
native Comment598/Synonym510 closures were also verified. Root and an
independent reviewer recomputed the science reports from every whole pack,
contiguous range, individual row SHA and full physical header. They checked
the original terminal proof, exact backup/restore identity, single dispatch,
five consumed stage payloads, lease release and all retained deltas.

The source rule was unchanged for these experiments. Their measured marker
agreement does not establish the retention rule for changed runtime bodies,
repeated generations, other description changes or the 20/21-row boundary.
Those cases require their own native/OWN twins before #418 can close.
