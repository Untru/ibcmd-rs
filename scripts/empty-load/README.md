# Empty-database load: lab harness

The scripts that produced the empty-database results recorded in
`openspec/changes/complete-native-source-load-parity/evidence/empty-database-load-20260928.md`.
They carry the lab's paths (`F:\ibcmd\lab\...`, reference trees, row caches)
and talk to the local SQL Server with Windows authentication.

| Script | What it does |
|---|---|
| `ve.sh <corpus> <run> [binary]` | Offline cycle, no SQL: `audit-empty-stage --rows-out` writes every row a load into an empty database would write, `mssql-dump-config --rows-dir` exports those rows, `source-diff` compares with the reference tree. Run it before a real load. |
| `run_empty.ps1 -Corpus <c> -Mode ours\|native [-Tag t] [-Exe path]` | Real run on a fresh disposable database `ibcmd_rs_empty_<corpus>_<mode>_<date>_<tag>` (files on F:): native `infobase create`, our `mssql-stage-source-objects --base-free`, native `config apply`, native `config export`, `source-diff`. Appends each step's time to `runs.tsv`. |
| `run_dropin_import.ps1 -Corpus <c> -Exe <our ibcmd.exe> [-Tag t] [-Step all\|reimport]` | The same through the drop-in command line (0.3): a fresh `ibcmd_rs_03_<corpus>_import_<date>_<tag>`, native `infobase create`, **our `ibcmd infobase config import`** in the platform's syntax (an empty target stages base-free by itself), native `config apply` (retried once for the БСП 8.3.27 SDBL quirk), native `config export`, `source-diff`. `-Step reimport` imports again into the loaded infobase (the patch path). |
| `native_cf_oracle.ps1` | The native control: `config save` of the source database to a `.cf`, then `infobase create --load=<cf> --apply` into a fresh database. |
| `xcheck.sh <corpus> <old\|model> <run> <binary>` | Offline export check of a build: export a stored row set (`--legacy-export` for `old`) and diff it against the reference tree. |

Corpora: `bsp8327`, `uha8327`, `bsp85`, `uha85` (`xcheck.sh`: `bsp`, `uha`,
`bsp85`, `uha85`).

The timing harness that measured ibcmd-rs against native ibcmd is in
`scripts/timing/` (`run_timing.ps1 -Phase export|load`, `summarize_final.py`).
