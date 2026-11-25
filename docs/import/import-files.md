# `infobase config import files`: what the platform does

Measured 2026-10-23 with `ibcmd` 8.5.1.1529 on copies of a БСП file infobase
taken off support (`config support disable --force` and `config apply`; on
support every edit is refused: «Загрузка невозможна: редактирование объекта
метаданных Catalog.Валюты запрещено!»). The directory is the platform's export
of that infobase with one edit: a comment line appended to
`Catalogs/Валюты/Ext/ObjectModule.bsl`. Each case starts from a fresh copy of
the infobase with an empty `CONFIGSAVE`; the staged rows were read from the
data file (`ibcmd-filedb`) and the staged state exported by the platform.

## The flags (`ibcmd help infobase`)

| flag | the platform's words |
|---|---|
| `--base-dir=<directory>` | Базовый каталог, содержащий XML файлы конфигурации |
| `--archive=<path>` | Путь к архиву, содержащему XML файлы конфигурации |
| `--no-check` | Отключить проверку метаданных после загрузки |
| `--partial` | Разрешить частичный набор файлов объектов метаданных |
| `--extension=<name>` | Имя расширения конфигурации |

## What is staged

`Валюты` below is the catalog `1d6b8425-…`: its descriptor, `.0` object
module, `.1` help, `.3` manager module, and four forms (descriptor, `.0` body,
`.1` help where it has one).

| listed | `--partial` | staged in `CONFIGSAVE` (besides `root`, `version`, `versions`) |
|---|---|---|
| `Catalogs/Валюты/Ext/ObjectModule.bsl` | yes / no | the descriptor (bytes unchanged) and `.0` |
| the same, from a directory holding that one file | yes / no | the same |
| `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl` | yes | the form's descriptor and `.0` |
| `Catalogs/Валюты/Forms/ФормаСписка.xml` | yes | the form's descriptor and `.0` |
| `Catalogs/Валюты.xml` | yes | the descriptor, `.0`, `.1`, `.3` and the four forms with all their rows (17 rows): the object is loaded from the directory, the unlisted edit of the object module included (the platform's export of the staged state has it) |
| `Catalogs/Валюты.xml` | no | the same, and a `deleted` row: `.1c`, `.1d` of the catalog and `.1` (help) of `ФормаСписка` -- roles the directory has no file for (the infobase has none of these rows either) |
| `Catalogs/Валюты.xml` from a directory holding only that file | yes | the descriptor; the bodies stay the infobase's |
| the same | no | the descriptor and `deleted`: `.0`, `.1`, `.3`, `.1c`, `.1d` -- the bodies are removed |
| `Catalogs/Валюты/Ext/ManagerModule.bsl`, absent from the directory | yes | nothing (`CONFIGSAVE` stays as it was, exit 0) |
| the same | no | the descriptor and `deleted`: `.3` |
| the five files of the catalog (descriptor, help, page, two modules) | no | as `Catalogs/Валюты.xml` without `--partial` |
| a new common module (its two files) | yes | refused, -1: «Ошибка добавления объекта CommonModule uuid="…" : нельзя добавлять объекты метаданных без загрузки родительского объекта.» |
| the same and `Configuration.xml` | yes / no | the whole configuration (7 214 rows; 7 215 with `deleted` without `--partial`) |
| a file outside the directory | yes | refused, -1: «Ошибка загрузки: нельзя загрузить файл file://…, т.к. он не находится в папке загрузки file://….» |
| `Catalogs/Валюты/Ext/Нет.bsl` (no such property) | yes | refused, -1: «Неверное свойство объекта метаданных. Свойство Нет не входит в состав объекта метаданных Catalog.» |
| the object module, with `--no-check` | yes | as without it |
| the object module as `Catalogs\Валюты\Ext\ObjectModule.bsl` or as an absolute path inside the directory | yes | as the relative path |

So `--partial` decides one thing: whether the files of an object the
directory lacks are kept (`--partial`) or removed. A listed metadata file
loads its object whole, nested objects included; a listed body file stages
that body (and re-saves the descriptor).

## A stage already in `CONFIGSAVE`

Two imports one after the other add up: the form module, then the object
module left the rows of both (seven: two descriptors, two bodies, `root`,
`version`, `versions`). A failed import (an unknown property) and an import of
a missing file under `--partial` leave the earlier stage as it was.

## Against ibcmd-rs on SQL Server

Two databases created by the platform from the same `.cf` (the БСП above, off
support) on SQL Server 17.0; each case starts with an empty `ConfigSave` on
both, then the platform stages on one and ibcmd-rs on the other, and the
platform exports the staged state of both:

| listed | `--partial` | rows staged: platform / ibcmd-rs | export of the staged states |
|---|---|---|---|
| `Catalogs/Валюты/Ext/ObjectModule.bsl` | yes | 5 / 4: ibcmd-rs leaves the descriptor the target's (the platform re-saves it unchanged) | equal, 9 156 of 9 156 files (`ConfigDumpInfo.xml` aside), the edit in both |
| `Catalogs/Валюты.xml` | yes | the same 18 row names (the platform staged 14 in a later run) | equal, 9 156 of 9 156, the unlisted edit of the module in both |
| `Catalogs/Валюты.xml` | no | 19 / 18: `deleted` (rows the target does not hold) | -- |
| `Catalogs/Валюты/Forms/ФормаСписка/Ext/Form/Module.bsl` | yes | 5 / 4, as for the module | -- |
| a missing file, from a sparse directory | yes | 0 / 0, both succeed | -- |

ibcmd-rs refused, as below: a second import onto the first one's stage, and
`Catalogs/Валюты.xml` without `--partial` from a directory holding only that
file (the platform removes `.0`, `.1`, `.3`; ibcmd-rs names them).

## What ibcmd-rs does with it

`src/mssql/files_stage.rs` (`select_native`), on SQL Server:

- a listed metadata file brings the files of its object the directory holds,
  nested objects' too, as the platform loads it;
- a listed file the directory lacks is skipped under `--partial`; with nothing
  left the command succeeds and writes nothing, as the platform does. Without
  `--partial` it is refused (exit 1): the platform would remove its row;
- an object loaded whole without `--partial` is refused (exit 1) when the
  infobase holds a body of it that the directory lacks: the platform would
  remove that body, the stage of ibcmd-rs would keep it. Bodies the infobase
  does not hold (the `.1c`, `.1d` the platform lists) do not count;
- a directory without `Configuration.xml` is taken without `--partial` too;
- `CONFIGSAVE` that already holds a stage is refused (exit 1): the platform
  adds to it, the stage of ibcmd-rs replaces `CONFIGSAVE`;
- unchanged: a new object, `Configuration.xml`, a body file whose object's
  metadata file the directory lacks (the platform loads such a body; ibcmd-rs
  needs the descriptor to compile it), `--archive` and `--extension` are
  refused.
## SQL twins, 2026-11-24

The public CLI on master `54b796f6` failed to import a changed common form
descriptor from a sparse directory. It attempted to compile the descriptor
using an absent `Configuration.xml` and exited −1. Full import succeeded.
With the sparse patch fix, both imports succeeded, native apply accepted
both stages, and native export reproduced both edits.

### Inputs and reproduction

- SQL Server Developer 17.0.1155.3 on Windows, integrated authentication.
- Native `ibcmd.exe` 8.3.27.2342, SHA-256
  `38f37715535cea530e58d5ba2e7e072c184b61b12f3e903ac290b299cf39fd38`.
- ibcmd-rs 0.4.1, Windows MSVC dev build with Rust 1.98.0 and debug info off,
  SHA-256 `ae04c5c67d34654a5ec4824c3ed9f602e0f693cd44f4e1122eab58ee56ab4f24`.
- Fixture `tests/fixtures/external/home_page/two_variable/input.cf` (80,862
  bytes, SHA-256 `f2a236f7babbc194655e903e24f7686d25dc368ec6c4d0dd61af047b8db8b86a`).

1. Use native `infobase create --load=<fixture> --apply --locale=ru_RU` on
   a fresh SQL database. Take a SQL backup with CHECKSUM. Restore it into a
   second disposable database with its own data and log files. Use
   `--dbms=MSSQLServer --db-server=localhost --db-name=<twin>` for each
   command and a separate `--data=<directory>` for each native invocation
   that can run concurrently. The initial native exports matched in all 11 files.
2. Copy the native export into a full input directory. In
   `CommonForms/ФормаСлева/Ext/Form/Module.bsl`, replace `Номер = 1;` with
   `Номер = 2;`. In `CommonForms/ФормаСправа.xml`, replace `<Comment/>` with
   `<Comment>Изменено частичной загрузкой</Comment>`. Preserve UTF-8 BOM
   and line endings. Copy those two files and `CommonForms/ФормаСлева.xml`
   (module owner) into a separate sparse tree at the same relative paths.
3. On the first twin run `infobase config import files --base-dir=<sparse>
   --partial CommonForms/ФормаСлева/Ext/Form/Module.bsl CommonForms/ФормаСправа.xml`.
   On the other run `infobase config import <full>`. Inspect
   `SELECT FileName FROM ConfigSave ORDER BY FileName` before apply.
4. Run native `infobase config apply --force` on each twin, then native
   `infobase config export <fresh-dir>`. Run the pair first with native
   import as the oracle, then restore both from the same backup and repeat
   with ibcmd-rs. Run the checker on the four resulting native exports.

```text
python scripts/apply-trace/lab/compare_import_files_twins.py baseline native-full native-sparse product-full product-sparse
```

The sparse regression can also be run on its own. Start from the restored
fixture with empty ConfigSave. It creates the sparse input from a native
export, checks the unsupported property with `--no-check`, verifies that
ConfigSave is still empty, then stages the supported edits and checks the
native apply and export.

```text
python scripts/apply-trace/lab/test_sparse_descriptor.py --product <ibcmd-rs.exe> --native <ibcmd.exe> --database ibcmd_rs_363_<disposable> <new-work-directory>
```

All four imports, applies and exports exited 0. The checker compares the
path set and every file against the initial export with exactly the two
edits above. In `ConfigDumpInfo.xml` only the 40-hex-digit object version
values can vary. The object names, IDs, order and other bytes matched.

| Import | Files checked | Raw SHA-256 of ConfigDumpInfo.xml |
| --- | ---: | --- |
| Native full | 11 | `cbd7c2b53d73d519a8d3f79c79761ab8a4b0125f7662639bf81505a081d3eb01` |
| Native sparse | 11 | `36a56e9d638244455cc7c23baeedb41e7043e5415f25ed5f934da978d2f92451` |
| ibcmd-rs full | 11 | `3506febcfdc83cb3a19c342e659e5771c11073af76d02dd3014afb499f1943ad` |
| ibcmd-rs sparse | 11 | `a6370fe532d59256b65a80277711fdc9be939054a7fbb1ed008913647ac82140` |

### Descriptor and flag boundaries

- Native sparse staged six rows: the left form descriptor and body, the
  right form descriptor, `root`, `version` and `versions`. Both product
  imports staged five, reusing the target's unchanged left descriptor.
- When the sparse tree held only the module, native `--partial --no-check`
  staged the owner, body and three service rows. ibcmd-rs refused (exit 1),
  named the missing owner XML and left ConfigSave empty.
- Listing just the right descriptor from the same sparse tree without
  `--partial` made native stage a `deleted` row. ibcmd-rs refused (exit 1)
  with the missing body's row name and left ConfigSave empty.
- Changing `UseStandardCommands` from false to true as well as the comment
  in the sparse right descriptor requires a different descriptor row. With
  `--partial --no-check` ibcmd-rs reported the mismatched property (exit −1)
  and left ConfigSave empty. When compiling requires the configuration root,
  supply the full tree as `--base-dir` and list the desired files.

This records existing common forms on SQL Server with platform 8.3.27.2342.
Other object families, nested dependency descriptors and merging into a
previous stage remain outside this measurement.

## Nested subsystem descriptors on SQL Server, 2026-11-25

The second fixture is
`tests/fixtures/external/nested_subsystems/input.cf` (SHA-256
`d31fa5104dbb979cd478f94b732763eabdf998f833ba6ba26332f266c9cc3a25`).
It requires platform 8.5.1. Use native ibcmd 8.5.1.1529 (SHA-256
`4dde71c1d061cc14c619255ec524b4ca8aca1f9bc38a92d1d766eb7d57721d20`)
and SQL Server Developer 17.0.1155.3. Create and apply the fixture, then
restore a copy-only checksum backup into a second disposable SQL database
as above. The two initial native exports were byte-identical.

In the full native tree change the empty comment of
`Subsystems/Верх/Subsystems/Низ.xml` to `<Comment>SQL twin 363</Comment>`.
The parent lists the child in `ChildObjects`. The child references
`CommonModule.ФайловаяБазаПроверка` in `Content`. Leave both relationships
unchanged. Keep the native encoding and line endings.

| Native import | XML directory and file list | Rows in ConfigSave before apply |
| --- | --- | --- |
| Full | Whole edited tree | Full import |
| Partial, `--no-check` | The parent and child descriptors, with only `Subsystems/Верх.xml` on the command line | Parent, child, `root`, `version`, `versions` |
| Partial | Only the edited child descriptor, listed by its relative path | Child, `root`, `version`, `versions` |

Both partial cases ran on separate restores of the same backup. Native
`config apply --force` and `config export` succeeded in each case. The
seven exported paths and their contents matched the edited input with
only `ConfigDumpInfo.xml` version values normalized. The child-only case
required neither the parent descriptor nor the referenced common module
descriptor in the source directory. The parent-selected case imported
the unlisted child that was present beneath its directory.

```text
python scripts/apply-trace/lab/compare_import_files_twins.py --edited edited native-full native-parent native-child
```

Restore both databases from the original backup and repeat the three
imports using ibcmd-rs, restoring the sparse twin between partial runs.
Use native apply and export throughout. Measured on `dc177253`, Windows
MSVC dev build with Rust 1.95.0, SHA-256
`cf9216dbb0cf06ff0c8e5262dc4eb23e7d2ff4fdd41a9783d540c4b43741430e`.

| ibcmd-rs import | Rows in ConfigSave before apply | Raw ConfigDumpInfo.xml SHA-256 after apply |
| --- | --- | --- |
| Full tree | Child, `root`, `version`, `versions` | `47ed2dec8f8451264e8d97da95f300eaf0da50394984330b75261a444b98d1cb` |
| Only parent listed, parent and child supplied, `--partial --no-check` | Parent, child, `root`, `version`, `versions` | `026dd0e159aef6e9e613c42832289d6ca3cc218f504324e5c72358d980511b15` |
| Only child listed and supplied, `--partial` | Child, `root`, `version`, `versions` | `167d1f5ce419b26ea4db54b8cce3d5ecc6064aee02a05901d6978cb81ede4c38` |

All imports, applies and exports returned 0. All six exports matched all
seven files in the edited input after normalizing only `configVersion`
values. The existing parent-child relationship and common-module reference
survived all three product imports. Unserved operations are documented in
“What ibcmd-rs does with it” above, along with their refusal behavior.
