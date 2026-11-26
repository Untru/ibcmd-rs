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
