# Native natural Params marker seed — 2026-10-02

One native-only B-force/C-force/D-exclusive sequence completed successfully on
the previously restored, manifest-owned BSP clone. This establishes an actual
natural baseline for #418; it does not establish the complete marker collection
rule, OWN parity, or completion of #418.

The writer was native 8.3.27.2214, SHA256
`11C77778927FAEF858FA4AB544ED627B9B6824A623EE7E5D6E6D5A0CF732D02B`.
Root retained original execution handle 86437 and confirmed controller 49788
known exit 0. All 117 numbered child receipts returned 0, including exactly
three native imports, three native applies and six exact native FIFO releases.
Final state has no native potential, bound lease or uncertainty. The sequence
did not restore again, register an infobase, start a worker/client, write Params
through the harness, or run our product CLI. Source checkpoint f1963df remains
the test-only semantic substrate; production marker collection is unchanged.

The six commands used the already measured sparse selected-files import, then
`config apply --force --dynamic=force` for B and C and
`config apply --force --dynamic=disable` for D. Each stage contained exactly
five native rows: root, version, versions, the existing CommonModule descriptor
and its .0 body. In all three stages, the descriptor's complete 289-byte decoded
content equals its effective preimage while the body genuinely changes. The
descriptor compression differs from the physical ordinary row (173 versus
174 bytes). No arbitrary normalization was used.

After D, ConfigSave is empty, Config is settled and Params has no generation
aliases. Params DynamicallyUpdated remains as a 156-byte naturally generated
row with four history UUIDs; B and C added the last two UUIDs. Its payload SHA256
is `a6a4becf0894197ff52642b07a26dafd9c9b5298e084a615a031a54b4906a1e6`.
The complete dates/attributes/size/part header is retained in seed-shape.json
and the raw snapshot index. This is evidence against using staged-descriptor
presence or compressed digest alone as the collection trigger. Genuine
metadata changes, repeated OWN application, effective pending aliases and the
20/21-row boundary still require their separate measured matrix.

The natural seed was preserved once with COPY_ONLY/CHECKSUM in
`F:/ibcmd/lab/05/wave3/params-marker/baselines/marker-seed-resumed-v2.bak`:
254169088 bytes, SHA256
`7DFA18301DC8EDB2E0BB70136ABE369E7AD370A45447DA61F2E8955977EB8D41`.
The retained typed HEADERONLY binds the exact owned DB, one full backup at
position 1, COPY_ONLY/checksum and not-damaged flags. VERIFYONLY returned 0.
All six storage table inventories, complete row headers/raw bytes and ten
auxiliary multisets are exactly equal before and after backup preservation.

The additive [terminal checkpoint](native-seed-resume-v2/terminal-checkpoint.json)
pins 562 original external files, including the unchanged 166-member executable
freeze, original handles, all 117 receipts, twenty new full six-table/ten-auxiliary
snapshots, backup and offline analyzers. The
[terminal analysis](native-seed-resume-v2/terminal-analysis.json) independently
checks every pack SHA/range/row SHA, multipart continuity and group DataSize,
known steps, exact native command target, queue preimage equality and backup
equality. The [physical transition report](native-seed-resume-v2/storage-transitions.json)
lists every added/removed/header-or-byte-changed row and auxiliary multiset
delta across all six import/apply transitions. Pack offsets are excluded only
from per-key presentation; the complete raw-offset proof is retained. There
are no evidence normalizations or blanket UI/cache exclusions. UI and derived
service/cache effects remain opaque measured deltas, not a full Params or
storage parity assertion.

Earlier failures remain retained and unaltered: the initial HEADERONLY Decimal
serialization refusal before restore; idle refusal after the one successful
restore (later transient cause unproved); native empty-stage apply returning
success without folding; and uppercase evidence-tag refusal before native
execution. The accepted resume used lower-case evidence tags, preserving the
upper-case fixture directories. The incorrect unconsumed launcher was preserved
and a separate correct manifest route was used. None of those attempts is
relabelled as a successful seed or hidden by a restore/retry.

The next native/OWN matrix is preparation only. The test-only descriptor reader
binds full inventoried physical headers and SHA before interpreting bytes,
strict complete streams, canonical own UUID, aggregate budgets and validated
history/alias closure. Enabling a production collection rule requires actual
native matrix outcomes, a locked semantic preimage guard, meaningful race/no-op
regressions, mandatory quick gates and a newly built matching OWN binary.
No broader 8.5, Params SI folding, licensing or global-clear capability follows
from this seed.

The later [OWN Comment/Synonym native-STAGED twins](own-native-staged-twins-2026-10-02.md)
record two completed exclusive Apply experiments and their independently
reviewed marker outcomes. The remaining retention matrix is still open.
