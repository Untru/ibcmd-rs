# `infobase config save`: a `.cf` straight from the rows (#352, #353)

`ibcmd infobase config save [--db] <file.cf>` and the research command
`ibcmd-rs mssql-save-config` write the configuration a database publishes as a
`.cf` without XML in between (`src/mssql_dump/config_save.rs`). The offline
half of `config load` (`ibcmd-rs mssql-load-config --script-only`,
`src/mssql/cf_load_stage.rs`) is the inverse: the elements of a `.cf` become
the rows a load stages in `ConfigSave`, written as the bulk stage's rows file
and scripts.

## What the container holds

| Rule | Evidence |
|---|---|
| One element per published row, its `BinaryData` as stored (raw deflate) | The platform keeps a form body byte for byte through `config load`, `config apply` and `config save --db` (`native-evidence/8.3.27.2214/dcs-form-attributes-conditional-appearance/manifest.json`, `reattested`); all 1 027 elements of the 102 platform-written containers in `tests/fixtures` (those whose element headers carry the save time) are one raw-deflate stream each. |
| A row stored in parts (`PartNo` 0..n) is one element, the parts concatenated | `Config` cuts a row over 10 000 000 bytes into parts that each carry the whole `DataSize` (`docs/import/patch-mode.md`, section 7). |
| The published rows: an online generation's aliases under their plain names; without `--db`, a completed stage of `ConfigSave` over `Config` | The view `config export` reads (`install_storage_overlay`). `DynamicallyUpdated` (the generation history, stored undeflated) is left out. |
| `deleted` only when the published rows have one | None of the 34 platform `.cf` files under `tests/fixtures/native-evidence` carries it (saved from applied infobases; the one whose steps are recorded used `config save --db`), nor any of the 80 `.cf`/`.cfe` files under `tests/fixtures/external`; the `config save` of an infobase whose `config import` was not applied carried the stage's empty removal list (4 bytes: BOM and `0`; `src/mssql_dump/config_dump_info.rs`, `OPTIONAL_SERVICE_NAME`, the procedure in `seed-configurations-method.md`). In `Config` the row marks an unfinished operation, which is refused with `config repair` (as the apply refuses it). |
| Elements in byte order of their names | 102 of 102 platform-written containers list them so. |
| Format15, 512-byte pages, the base-free bootstrap's writer | 8.3.27.2214 loads this writer's Format15 (`/LoadCfg`, commit 859d171) and refused its Format16. |
| Third file-header word = element count | 102 of 102 platform-written containers (12 elements: 12), and the Format15 preamble of all 60 Format16 ones (5 entries: 5). Of the 21 containers with zero header times (built by this project or by hand) only 4 say so; the bootstrap writes 5 by default. |
| Element headers without time | As the bootstrap writes them; the platform stamps the save time into every header. |

## Byte comparison with the platform's own files

Rows taken from a platform `.cf` (Designer `/DumpCfg` of 8.3.27.2214) and saved
back give the same elements in the same order with the same packed bytes and
the same UTF-16 names; `cf export` of both writes the same tree file for file
(`tests/cf_config_save.rs`). The files differ only in container layout.

`config_compat/c10_e27/input.cf` (Format15, 6 elements, 5 660 bytes; saved:
5 594 bytes):

| Field | Platform | Saved |
|---|---|---|
| File header (`next`, page, third word, reserved) | `7fffffff`, 512, 6, 0 | same |
| TOC order | names in byte order | same |
| Element header pages | exactly the header's length | same |
| TOC page | 72 bytes of data in a 512-byte page | a 72-byte page |
| Data over 512 bytes | one page of the data's length (1 767) | 512-byte pages chained (4) |
| Order the elements are laid out in | `…0001`, `versions`, `…0002.b`, `ba46…`, `root`, `version` | TOC order |
| Header times | `2026-09-28T06:42:22` (the save) | zero |

`choice_list_dates/input.cf` (Format16, 8 elements, 77 982 bytes; saved:
8 048 bytes) differs in the same three layout points (TOC page, data pages,
layout order) and the times, and in
addition is Format16 at offset `0x1359` behind the platform's Format15
preamble (5 entries: two of the platform's own, `root`, `version`,
`versions`; 64 KiB TOC page), where the saved file is Format15 from offset 0.
The third header word is 8 in both.

The layout order is the platform's own and not derivable from the rows (in
`role_rights/new_true_emt_false` it is `…0001`, `6a1b…`, `versions`,
`…0002.b`, `6a1b….0`, `ba46…`, `root`, `version`), and the times are the
moment of the save, so a byte-identical copy of a platform `.cf` is not a
goal. A file this writer saved does reproduce byte for byte: its rows saved
again give the same bytes, and the bytes equal an independent build with
the V8 writer from the same rules (`tests/cf_config_save.rs`).

## The offline half of `config load`

`mssql-load-config --script-only` refuses a file without `root`, `version` and
`versions` (an extension's `.cfe`) and one whose element is not one complete
raw-deflate stream, then writes one row per element in the container's order:
`Kind` 0, `PartNo` 0, `DataSize` the element's length, or several parts of
10 000 000 bytes each carrying the whole size. The apply script is the
base-free stage's (`ConfigSave` emptied, every row inserted with the
platform's dates and `Attributes` 0). A `.cf` that carries `deleted` stages it
as it is. `tests/cf_config_save.rs` reads the rows file back and finds the
container's records; saving those rows back gives the original configuration
(`cf export` file for file).

## Open, needs the platform or SQL Server

- `config load` of a saved file by 8.3.27.2214 / 8.5, and its export equal to
  the database's (the acceptance of #352); a save of a database with a
  completed stage, of one with an online generation, and of one with a row in
  parts against the platform's `config save` of the same database.
- Whether the platform's `config save` without `--db` writes `deleted` when
  nothing is staged (here it does not).
- The platform's lines and behaviour for `config save` (an existing file is
  replaced here; the title is the command's name in `ibcmd help infobase`).
- `config load` against a server: running the scripts, the `deleted` list for
  what the target's `Config` holds and the file does not, and the platform's
  `config apply` of such a stage (#353).
