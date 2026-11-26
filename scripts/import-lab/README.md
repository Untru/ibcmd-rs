# import-lab: research kit of the "import" track (issue #388)

Tools that measure what `ibcmd infobase config import` stages into an existing database, in each mode,
against the platform's own import. Findings: `docs/import/patch-mode.md`.

**Lab only.** Databases `ibcmd_rs_04_import_*` (the scripts check the prefix); every native write runs inside
the lab's native lock (`F:\ibcmd\lab\04\tools\heavy-lock.ps1 ... -Name native`). Python 3.13 with `pyodbc` and
the "ODBC Driver 18 for SQL Server", PowerShell 7, SQL Server with Windows authentication, native
`ibcmd.exe` 8.3.27.2214. The lab folder defaults to `F:\ibcmd\lab\04\import` (`IMPORT_LAB`).

| script | what |
|---|---|
| `edits.py` | named edits of an exported XML tree (`list`, `apply <name>[,...]`, `reset`): a new attribute, tabular section, catalog, form, template, predefined item, enum value, role rights, subsystem content, command interface, module, configuration version, and removals. Each on its own object; BOM, CRLF and the bare LF inside text values are kept. The base tree is never written; a work tree is reset from it |
| `rowdiff.py` | row by row comparison of two `Config`-like tables or `<name>__part<N>.bin` folders (parts joined, inflated; classes identical / same text / layout only / v8 container headers only / different / in one side only); `--dump-left` writes a table as such a folder |
| `native.ps1` | `Invoke-NativeImport`, `Invoke-NativeApply`, `Invoke-NativeExport`, `Invoke-OursImport` (the drop-in, patch or `--base-free`), the re-entrant native lock |
| `run_matrix.ps1` | every change x mode staged into a clone that is never applied, compared with the stage of the unchanged tree in the same mode; results in `out\matrix\<tag>` |
| `summarize.py` | the matrix as a table and per-change row lists |
| `run_survival.ps1` | stage in a mode, native `config apply`, native `config export`, `source-diff` against the tree |
| `equiv_cmp.py` | two applied databases: table structure, `DBNames`, `DBSchema`, `Params`, Config rows |
| `ve_tree.sh` | offline round trip of a tree: the rows `--base-free` would stage, exported again from those rows, diffed against the tree |
| `guard_probe.ps1` | the guard of the proposal as an experiment: stage a change (patch or `--base-free`), model the state an apply produces (`overlay_rows.py`), export it offline with the model, `source-diff` against the edited tree; a differing file is a change the stage did not carry |
| `overlay_rows.py` | ConfigSave over Config as a rows folder, without the `deleted` names and the dynamic-update rows (the state the native apply produces) |
| `guard_table.py` | the guard probe results next to the matrix verdict of each change |
| `native_complete.py` | the lab's completeness test of a native `config import` stage (has `versions`, no `commit` / `*.new` row, every changed entry of `versions` has its row) |
| `native_subset.ps1` | native `config import` of a tree with only the named edits, to find which edit the platform refuses |
| `run_guard_acceptance.ps1` | the guard's acceptance on a БСП clone: every edit of `edits.py` through the drop-in `config import` with its default flags; refusals must leave ConfigSave byte-identical (fingerprint before and after), carried edits must stage; results in `out\guard-acceptance\<tag>.json` |
| `run_uha_guard.ps1` | what the guard costs on ERP УХ, offline (`IBCMD_RS_BASE_ROWS_DIR`, `--script-only`) inside the heavy lock: wall and CPU seconds, peak memory, the stage's timing lines; results in `out\uha-guard\<tag>.json` |
| `run_override_acceptance.ps1` | step 2: a tree through the drop-in import into a БСП clone (default flags), what it left in ConfigSave, the platform's native `config apply` and `config export`, `source-diff` of the tree against the export; results in `out\override-acceptance\<tag>.json` |
| `configsave_summary.py` | what a stage left in ConfigSave: row count, rows in parts, the years of the dates, the `deleted` row's names, the `versions` counts (read-only) |
| `run_uha_import.ps1` | the database-backed import on an ERP УХ clone inside the heavy lock: wall and CPU seconds, peak memory, the stage's timing lines (the comparison of the tree, the base rows, the guard, the move into ConfigSave); results in `out\uha-import\<tag>.json` |

Typical run:

```powershell
pwsh -NoProfile -File F:\ibcmd\lab\04\tools\restore-clone.ps1 -Corpus bsp8327 -Name ibcmd_rs_04_import_bsp_s1 -Track import -Purpose "..."
# baselines of the unchanged tree, one per mode (out\rows_<patch|bf|native>_pristine)
python scripts\import-lab\rowdiff.py ibcmd_rs_04_import_bsp_s1:ConfigSave x --dump-left F:\ibcmd\lab\04\import\out\rows_patch_pristine
pwsh -NoProfile -File scripts\import-lab\run_matrix.ps1 -Changes attr,ts,newcat -Modes patch,bf,native
python scripts\import-lab\summarize.py F:\ibcmd\lab\04\import\out\matrix\v0 --detail
```

Run one native `ibcmd` of your own at a time, one command per hold of the native lock, and check a native import
with `native_complete.py` (a partial stage makes the apply fail). A native import that fails leaves a few rows in
`ConfigSave` (`*.1c.new`); the next import replaces them.
