# Direct SQL apply with a staged CommonModule body (2026-11-24)

The oracle was Windows x64 ibcmd 8.3.27.2342. The replacement was built from
upstream `54b796f6` and used its default 8.3.27 storage profile (2214).
This comparison does not add 2342 as an exact supported profile.

## Setup and readback

1. Restore a disposable БСП database and append one comment to
   `CommonModules/GoogleПереводчик/Ext/Module.bsl` in its XML export.
2. Stage the body with native `infobase config import files --base-dir=<tree> <body>`.
   Take a copy-only SQL backup. Each apply below restores it to a fresh database.
   All sixteen preimages matched across the six tables measured.
3. Run `infobase config apply --dbms=MSSQLServer --db-server=localhost
   --db-name=<clone> --data=<private-directory> --force --dynamic=<mode>`.
   Redirect stdin from the null device. In held cases open one idle ODBC
   connection on the clone and query its original SPID again after the command.
4. Read every (FileName, PartNo, SHA-256 of stored BinaryData) in Config,
   ConfigSave, Params, Files, ConfigCAS and ConfigCASSave before and after.
   Row counts alone cannot distinguish a new body from the old body.
   [Per-run exits, payload map hashes, markers and changed-row counts](direct-2342-results.json)

## Apply results

| Mode | Native, alone | Native, held | Replacement, alone | Replacement, held |
|---|---|---|---|---|
| auto | in place | in place | in place | lock refusal with dynamic hint |
| disable | in place | in place | in place | lock refusal |
| prompt, closed stdin | in place | in place | in place | lock refusal with dynamic hint |
| force | overlay | overlay | overlay | overlay |

Every successful apply returned 0 and consumed five staged rows. In-place
apply changed the ordinary module body and `versions`; neither marker was
present. Force kept the ordinary body and added three aliases and one Config
marker. The Params marker payload matched native. Across successful twins
the entire Config payload map matched, including the alias names. Each held
connection survived. The three replacement lock refusals returned -1, with
all six payload maps intact. The stricter SQL census is deliberate.

With the held connection, direct native `disable` plus either
`--session-terminate=prompt` or `force` applied in place; the replacement
returned 1 and preserved the stage. Dynamic force plus termination force
published the overlay in both tools and left the held connection alive.

In a Windows pseudoconsole, the replacement offered its SQL-guard menu only
after the lock refusal. Choice 1 returned 0 with all five rows still staged.
Choice 2 returned -1 with the connection held and applied in place after the
test released it at the prompt. Choice 3 published an overlay with both markers
and kept the connection. These are SQL-guard sessions, not 1C client sessions.

## Other writes and remaining acceptance

Native also changed `Params.ibparams.inf` and `locale.inf` and added two
`.ui` rows; the replacement kept those preimages. Both rewrote
`Files.MobileVersions.dat`; stored bytes differed. The two extension tables
were empty throughout. This first generation does not exercise a pending
marker fold (#418), extension activation (#348), or help/search index rebuilds.
No conclusion about native standalone session termination follows from a SQL SPID.

## Help route

On native, `help`, `infobase config apply --help` and
`help infobase config apply` returned identical overview bytes and exit 0.
`help infobase` returned the mode help listing the four dynamic values.
The replacement previously routed all three infobase spellings to its detailed
mode page. The CLI regression checks the destination independently of the
database runs.
