# Выгрузка .epf/.erf без платформы — план реализации (план 1 из 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ibcmd-rs cf export file.epf|file.erf out/` выдаёт ту же XML-выгрузку, что платформа 8.3.27.2214 (`/DumpExternalDataProcessorOrReportToFiles` в базе с конфигурацией), без 1С.

**Architecture:** адаптер на границе (spec §3). Главная запись внешнего объекта переписывается в запись `DataProcessor`/`Report` «как в .cf», `copyinfo` превращается в строку-индекс типов, штатный `mssql_dump` выгружает всё как обработку/отчёт конфигурации, затем дерево `DataProcessors/X/*` переносится в `X/*` и корневой XML превращается во внешний. Весь новый код — `src/external/`; в upstream-файлах меняются только две точки входа.

**Tech Stack:** Rust 2024 (cargo 1.96), crate `ibcmd-rs`, зависимости уже есть (`anyhow`, `flate2`, `sha1`, `quick-xml`). Регулярок нет (крейта `regex` в зависимостях нет — не добавлять).

**Spec:** `docs/superpowers/specs/2026-09-28-offline-external-epf-erf-design.md`

## Global Constraints

- XML-диалект выгрузки: 2.20 (`--source-version 2.20`), эталон — платформа 8.3.27.2214.
- Эталон сравнения: выгрузка платформы в базе с конфигурацией (`native-8.3.27.2214/ext-cfg/<X>`), для файлов без конфигурации — в пустой базе (`native-8.3.27.2214/<label>`).
- Fail closed: незнакомая форма записи → `anyhow::bail!` с понятным текстом, никаких догадок.
- Не менять поведение `cf export` для обычных `.cf/.cfe`: все существующие тесты upstream должны остаться зелёными.
- Новый код только в `src/external/`, кроме: `src/lib.rs` (1 строка `pub mod external;`), `src/mssql_dump/mod.rs` (новая pub-функция-обёртка), `src/commands/cf.rs` (вызов адаптера в `export()`).
- Корпус не коммитится (пользовательская конфигурация): путь в переменной `IBCMD_ONECDEC_CORPUS`, тест пропускается без неё. Коммитятся только чистые (clean-room) фикстуры, созданные в Task 1.
- Все файлы текста 1С: UTF-8 с BOM, переводы строк `\r\n` — сохранять как есть.
- Сборка/тесты: `cargo test --no-default-features <фильтр>`; release-бинарь для корпуса: `cargo build --release --no-default-features`.

## Review Focus

1. Обработка **без** модуля объекта / с пустым модулем (BOM-only `text`) — модуль должен выгрузиться 3-байтовым `Ext/ObjectModule.bsl`, как у платформы (Task 7).
2. `copyinfo` отсутствует или пуст (`{4,{0},{0},{0},{0,0},{0}}`) — выгрузка работает, типы остаются как есть (Task 4 тест `empty_copyinfo_has_no_types`).
3. Имя объекта — префикс другого имени в тексте (`Отчет.X` и `Отчет.X2`) — переименование ссылок не должно задеть `X2` (Task 5 тест `rename_respects_identifier_boundary`).
4. Файл `.cf/.cfe` не должен попадать в адаптер (`root` = `{2,<uuid>,<хеш base64>}` у Format16 .cf) — `export_if_external` обязан вернуть `None` (Task 6 тест `plain_configuration_is_not_external`).
5. Выгрузка в существующий каталог с `--overwrite` — перенос `DataProcessors/X` → `X` не должен падать на уже существующем `X/` (Task 6, `finish` удаляет целевой каталог перед переносом).

---

## Карта файлов

| Файл | Ответственность |
|---|---|
| `src/external/mod.rs` | `ExternalKind`, константы классов, `derived_uuid`, реэкспорт |
| `src/external/brace.rs` | разбор скобочных списков 1С (поля верхнего уровня) |
| `src/external/header.rs` | главная запись v4 ↔ запись DataProcessor v17 / Report v19 |
| `src/external/copyinfo.rs` | разбор `copyinfo`, таблица классов/порождённых типов, XML-строка для индекса типов |
| `src/external/versions.rs` | переписывание служебной записи `versions` |
| `src/external/rename.rs` | `DataProcessor.X` → `ExternalDataProcessor.X` с границами идентификатора |
| `src/external/root_xml.rs` | корневой XML внутреннего объекта → внешнего |
| `src/external/export.rs` | оркестровка выгрузки + перенос дерева + правка отчёта |
| `src/mssql_dump/mod.rs` | + `pub fn export_packed_entries_to_source` |
| `src/commands/cf.rs` | `export()` сначала пробует адаптер |
| `tests/external_export.rs` | интеграционные тесты на clean-room фикстурах + корпус (ignored) |
| `tests/fixtures/external/*` | clean-room .epf/.erf + эталонные выгрузки платформы |
| `_onecdec/make_fixtures.py` | воспроизводимая сборка фикстур платформой |

---

### Task 1: Clean-room фикстуры и падающий интеграционный тест

**Files:**
- Create: `_onecdec/fixture_src/ТестОбработка/**` (XML внешней обработки: реквизит строка, реквизит ссылка на нет — без конфигурации; табличная часть «Строки» с колонкой «Число»; управляемая форма «Форма» с полем реквизита; макет «Макет» (табличный документ); модуль объекта с одной процедурой; справка)
- Create: `_onecdec/fixture_src/ТестОтчет/**` (внешний отчёт: макет СКД «ОсновнаяСхемаКомпоновкиДанных» с одним набором-запросом `ВЫБРАТЬ 1 КАК Поле`, форма отчёта, модуль объекта)
- Create: `_onecdec/make_fixtures.py`
- Create: `tests/fixtures/external/<name>/input.<epf|erf>` и `tests/fixtures/external/<name>/expected/**`
- Create: `tests/external_export.rs`

**Interfaces:**
- Produces: фикстуры `tests/fixtures/external/test_processor/{input.epf,expected/}`, `tests/fixtures/external/test_report/{input.erf,expected/}`; хелпер `fn assert_tree_eq(expected: &Path, actual: &Path)` в `tests/external_export.rs`.

- [ ] **Step 1: Сгенерировать исходники фикстур.** Каркас — навык `unica:epf-init` / `unica:erf-init` (Designer XML, 2.20), затем дописать реквизиты/ТЧ/макет руками. Корневой XML обработки обязан содержать `InternalInfo` с `ContainedObject` (ClassId `c3831ec8-d8d5-4f93-8a22-f9bfae07327f`, отчёт — `e41aff26-25cf-4bb6-b6c1-3f478a75f374`) — без него платформа отказывает («Отсутствует внутренняя информация»).

- [ ] **Step 2: Написать `_onecdec/make_fixtures.py`**

```python
"""Build the clean-room external fixtures with the platform and capture the
native dumps the Rust tests compare against.

    python make_fixtures.py [8.3.27.2214]
"""
import os, shutil, sys
sys.path.insert(0, r'C:\Декомпилятор\1c-tools\onecdec')
import v8dump

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
FIXTURES = [('ТестОбработка', 'test_processor', '.epf'), ('ТестОтчет', 'test_report', '.erf')]


def main():
    ver = sys.argv[1] if len(sys.argv) > 1 else '8.3.27.2214'
    exe = r'C:\Program Files\1cv8\%s\bin\1cv8.exe' % ver
    work = os.path.join(HERE, '.fixture-work'); shutil.rmtree(work, ignore_errors=True); os.makedirs(work)
    ib, log = os.path.join(work, 'ib'), os.path.join(work, 'platform.log')
    v8dump._run(exe, ['CREATEINFOBASE', 'File="%s"' % ib], log)
    for src_name, label, ext in FIXTURES:
        dst = os.path.join(REPO, 'tests', 'fixtures', 'external', label)
        shutil.rmtree(dst, ignore_errors=True); os.makedirs(dst)
        binp = os.path.join(dst, 'input' + ext)
        src = os.path.join(HERE, 'fixture_src', src_name, src_name + '.xml')
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/LoadExternalDataProcessorOrReportFromFiles', src, binp], log)
        expected = os.path.join(dst, 'expected')
        os.makedirs(expected)
        v8dump._run(exe, ['DESIGNER', '/F', ib, '/DumpExternalDataProcessorOrReportToFiles',
                          os.path.join(expected, src_name + '.xml'), binp], log)
        print('ok', label)
    shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
```

Run: `python _onecdec/make_fixtures.py` → `ok test_processor`, `ok test_report`.

- [ ] **Step 3: Написать падающий тест `tests/external_export.rs`**

```rust
//! `cf export` of external data processors (.epf) and reports (.erf):
//! the output tree must equal the platform's own dump byte for byte.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external").join(name)
}

fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root) {
        let entry = entry.expect("walk");
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            out.insert(rel, fs::read(entry.path()).unwrap());
        }
    }
    out
}

fn assert_tree_eq(expected: &Path, actual: &Path) {
    let (e, a) = (files(expected), files(actual));
    let missing: Vec<_> = e.keys().filter(|k| !a.contains_key(*k)).collect();
    let extra: Vec<_> = a.keys().filter(|k| !e.contains_key(*k)).collect();
    let differ: Vec<_> = e.keys().filter(|k| a.get(*k).is_some_and(|v| v != &e[*k])).collect();
    assert!(
        missing.is_empty() && extra.is_empty() && differ.is_empty(),
        "tree mismatch\n missing: {missing:?}\n extra: {extra:?}\n differ: {differ:?}"
    );
}

fn export(input: &Path, out: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "export"])
        .arg(input)
        .arg(out)
        .args(["--source-version", "2.20", "--overwrite"])
        .output()
        .expect("run ibcmd-rs")
}

#[test]
fn external_data_processor_matches_native_dump() {
    let dir = fixture("test_processor");
    let out = tempfile_dir("epf");
    let run = export(&dir.join("input.epf"), &out);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_tree_eq(&dir.join("expected"), &out);
}

#[test]
fn external_report_matches_native_dump() {
    let dir = fixture("test_report");
    let out = tempfile_dir("erf");
    let run = export(&dir.join("input.erf"), &out);
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert_tree_eq(&dir.join("expected"), &out);
}

fn tempfile_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ibcmd-external-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}
```

Проверить, что `walkdir` доступен тестам (он в `[dependencies]` — интеграционные тесты видят зависимости пакета).

- [ ] **Step 4: Убедиться, что тесты падают**

Run: `cargo test --no-default-features --test external_export`
Expected: FAIL оба (формы уходят в `CommonForms/`, нет `<X>.xml` и т.д.).

- [ ] **Step 5: Commit**

```bash
git add _onecdec/make_fixtures.py _onecdec/fixture_src tests/fixtures/external tests/external_export.rs
git commit -m "test(external): clean-room epf/erf fixtures and failing export parity tests"
```

---

### Task 2: Разбор скобочных списков и модуль `external`

**Files:**
- Create: `src/external/mod.rs`, `src/external/brace.rs`
- Modify: `src/lib.rs` (добавить `pub mod external;` после `pub mod dump_sources;`)

**Interfaces:**
- Produces: `external::brace::fields(text: &str, start: usize) -> Option<(Vec<&str>, usize)>` — поля верхнего уровня (обрезанные), индекс за закрывающей скобкой; `external::brace::strip_bom(&str) -> &str`; `external::ExternalKind` (`DataProcessor`, `Report`) с методами `from_class_id`, `class_id`, `internal_kind`, `internal_folder`, `external_kind`; `external::derived_uuid(seed: &str) -> String`.

- [ ] **Step 1: Тесты в `src/external/brace.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_top_level_fields_with_nesting_and_quotes() {
        let text = "{1,\r\n{2,\"a,}{\"\"b\"},x},\"q\",{}}";
        let (f, end) = fields(text, 0).unwrap();
        assert_eq!(f, vec!["1", "{2,\"a,}{\"\"b\"},x}", "\"q\"", "{}"]);
        assert_eq!(end, text.len());
    }

    #[test]
    fn unbalanced_list_is_none() {
        assert!(fields("{1,{2}", 0).is_none());
    }

    #[test]
    fn strips_utf8_bom() {
        assert_eq!(strip_bom("\u{feff}{1}"), "{1}");
        assert_eq!(strip_bom("{1}"), "{1}");
    }
}
```

- [ ] **Step 2: Реализация `src/external/brace.rs`**

```rust
//! Top-level fields of 1C brace lists (`{a,{b,c},"d"}`), for the few external
//! rows this adapter rewrites. Quotes are 1C strings with `""` escapes.

pub fn strip_bom(text: &str) -> &str {
    text.strip_prefix('\u{feff}').unwrap_or(text)
}

/// Fields of the brace list opening at `text[start]` (trimmed) and the byte
/// index just past its closing brace; `None` when unbalanced.
pub fn fields(text: &str, start: usize) -> Option<(Vec<&str>, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'{') {
        return None;
    }
    let (mut depth, mut quoted, mut field_start) = (0usize, false, start + 1);
    let mut out = Vec::new();
    let mut i = start;
    while i < bytes.len() {
        let c = bytes[i];
        if quoted {
            if c == b'"' {
                if bytes.get(i + 1) == Some(&b'"') {
                    i += 1;
                } else {
                    quoted = false;
                }
            }
        } else {
            match c {
                b'"' => quoted = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        out.push(text[field_start..i].trim());
                        return Some((out, i + 1));
                    }
                }
                b',' if depth == 1 => {
                    out.push(text[field_start..i].trim());
                    field_start = i + 1;
                }
                _ => {}
            }
        }
        i += 1;
    }
    None
}
```

- [ ] **Step 3: `src/external/mod.rs`**

```rust
//! External data processors (.epf) and reports (.erf).
//!
//! The configuration pipeline already knows every rule for `DataProcessor`
//! and `Report` objects; an external object is the same object in a
//! different wrapper. This module adapts it at the boundary: the main row is
//! rewritten into the internal row shape, the pipeline runs unchanged, and
//! the produced tree is moved and renamed into the external layout.

pub mod brace;
pub mod copyinfo;
pub mod export;
pub mod header;
pub mod rename;
pub mod root_xml;
pub mod versions;

use sha1::{Digest, Sha1};

pub const EXTERNAL_DATA_PROCESSOR_CLASS: &str = "c3831ec8-d8d5-4f93-8a22-f9bfae07327f";
pub const EXTERNAL_REPORT_CLASS: &str = "e41aff26-25cf-4bb6-b6c1-3f478a75f374";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalKind {
    DataProcessor,
    Report,
}

impl ExternalKind {
    pub fn from_class_id(class_id: &str) -> Option<Self> {
        match class_id.trim().to_ascii_lowercase().as_str() {
            EXTERNAL_DATA_PROCESSOR_CLASS => Some(Self::DataProcessor),
            EXTERNAL_REPORT_CLASS => Some(Self::Report),
            _ => None,
        }
    }
    pub const fn class_id(self) -> &'static str {
        match self {
            Self::DataProcessor => EXTERNAL_DATA_PROCESSOR_CLASS,
            Self::Report => EXTERNAL_REPORT_CLASS,
        }
    }
    pub const fn internal_kind(self) -> &'static str {
        match self {
            Self::DataProcessor => "DataProcessor",
            Self::Report => "Report",
        }
    }
    pub const fn internal_folder(self) -> &'static str {
        match self {
            Self::DataProcessor => "DataProcessors",
            Self::Report => "Reports",
        }
    }
    pub const fn external_kind(self) -> &'static str {
        match self {
            Self::DataProcessor => "ExternalDataProcessor",
            Self::Report => "ExternalReport",
        }
    }
}

/// Deterministic uuid-shaped id for rows the adapter has to invent (never
/// written to output: the fields that carry it are dropped again).
pub fn derived_uuid(seed: &str) -> String {
    let d = Sha1::digest(seed.as_bytes());
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-5{:x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        d[0], d[1], d[2], d[3], d[4], d[5], d[6] & 0x0f, d[7], (d[8] & 0x3f) | 0x80, d[9],
        d[10], d[11], d[12], d[13], d[14], d[15]
    )
}
```

Остальные модули на этом шаге — пустые файлы с `//!`-описанием (заполняются в следующих задачах), чтобы крейт собирался.

- [ ] **Step 4: Прогон**

Run: `cargo test --no-default-features --lib external::brace`
Expected: PASS (3 теста).

- [ ] **Step 5: Commit** — `git commit -m "feat(external): brace-list reader and external kind"`

---

### Task 3: Главная запись v4 → запись DataProcessor v17 / Report v19

**Files:**
- Create: `src/external/header.rs`

**Interfaces:**
- Consumes: `brace::fields`, `brace::strip_bom`, `ExternalKind`, `derived_uuid`.
- Produces:
```rust
pub struct ExternalMain {
    pub kind: ExternalKind,
    pub main_uuid: String,   // root → main entry name; XML uuid of the external object
    pub object_id: String,   // {1,0,<id>} in the header; ContainedObject ObjectId; module is <id>.0
    pub name: String,
    pub header: Vec<String>, // header fields, [0] == "4"
    pub collections: Vec<String>,
}
pub fn parse_main(main_uuid: &str, text: &str) -> anyhow::Result<Option<ExternalMain>>; // None: not external
pub fn internal_row_text(main: &ExternalMain) -> anyhow::Result<String>; // BOM-prefixed
```

Таблицы соответствия (установлены на корпусе 81 .epf + 22 .erf, spec §2):

| внешнее v4 (индекс) | DataProcessor v17 |
|---|---|
| 0 `4` | `17` |
| 1 TypeId, 2 ValueId, 3 имена | те же 1, 2, 3 |
| 4 DefaultForm | 4 |
| 5 `""` (всегда пусто в корпусе) | — (иначе fail closed) |
| 6 AuxiliaryForm | 9 |
| — | 5 `1`, 6 `0`, 7/8 derived manager ids, 10 `{0}`, 11 `{0}` |

| внешнее v4 (индекс) | Report v19 |
|---|---|
| 0 `4` | `19` |
| 1–6 | 1–6 |
| — | 7 `1` |
| 7, 8, 9 | 8, 9, 10 |
| 10 `""` | — (иначе fail closed) |
| — | 11 `0`, 12/13 derived manager ids |
| 11 | 14 |
| — | 15 `{0}`, 16 `{0}` |
| 12 | 17 |

Коллекции: обработке вставить `{45556acb-826a-4f73-898a-6025fc9536e1,0}` (команды) перед коллекцией форм `d5b0e5ed-…`; отчёту дописать в конец `{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,0}`.

- [ ] **Step 1: Тесты** (фрагменты — реальные строки из `tests/fixtures/external/test_processor/input.epf`, вынуть скриптом `python _onecdec/v8c.py tests/fixtures/external/test_processor/input.epf --get <uuid>` и вставить как `const`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const MAIN_DP: &str = "{1,\r\n{aaaaaaaa-0000-0000-0000-000000000001},1,\r\n{c3831ec8-d8d5-4f93-8a22-f9bfae07327f,\r\n{1,\r\n{4,11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222,\r\n{0,\r\n{3,\r\n{1,0,bbbbbbbb-0000-0000-0000-000000000002},\"Тест\",\r\n{1,\"ru\",\"Тест\"},\"\",0,0,00000000-0000-0000-0000-000000000000,0}\r\n},cccccccc-0000-0000-0000-000000000003,\"\",00000000-0000-0000-0000-000000000000},4,\r\n{2bcef0d1-0981-11d6-b9b8-0050bae0a95d,0},\r\n{3daea016-69b7-4ed4-9453-127911372fe6,0},\r\n{d5b0e5ed-256d-401c-9c36-f630cafd8a62,1,cccccccc-0000-0000-0000-000000000003},\r\n{ec6bb5e5-b7a8-4d75-bec9-658107a699cf,0}\r\n}\r\n}\r\n}";

    #[test]
    fn parses_external_data_processor_main_row() {
        let m = parse_main("aaaaaaaa-0000-0000-0000-000000000001", MAIN_DP).unwrap().unwrap();
        assert_eq!(m.kind, ExternalKind::DataProcessor);
        assert_eq!(m.object_id, "bbbbbbbb-0000-0000-0000-000000000002");
        assert_eq!(m.name, "Тест");
        assert_eq!(m.header.len(), 7);
        assert_eq!(m.collections.len(), 4);
    }

    #[test]
    fn rewrites_into_data_processor_row_the_pipeline_reads() {
        let m = parse_main("aaaaaaaa-0000-0000-0000-000000000001", MAIN_DP).unwrap().unwrap();
        let row = internal_row_text(&m).unwrap();
        assert!(row.starts_with("\u{feff}{1,\r\n{17,11111111-1111-1111-1111-111111111111,22222222-2222-2222-2222-222222222222,"));
        let (top, _) = brace::fields(brace::strip_bom(&row), 0).unwrap();
        assert_eq!(top[2], "5");
        let (h, _) = brace::fields(top[1], 0).unwrap();
        assert_eq!(h.len(), 12);
        assert_eq!((h[4], h[5], h[6]), ("cccccccc-0000-0000-0000-000000000003", "1", "0"));
        assert_eq!(h[9], "00000000-0000-0000-0000-000000000000");
        assert!(top[5].starts_with("{45556acb-826a-4f73-898a-6025fc9536e1,0}"));
        assert!(top[6].starts_with("{d5b0e5ed-"));
    }

    #[test]
    fn configuration_root_object_is_not_external() {
        let text = "{2,\r\n{30ffe4cc-eef2-4371-8b26-046597e37e22},6,\r\n{9cd510cd-abfc-11d4-9434-004095e12fc7,{1}}}";
        assert!(parse_main("30ffe4cc-eef2-4371-8b26-046597e37e22", text).unwrap().is_none());
    }

    #[test]
    fn unknown_filled_blank_field_fails_closed() {
        let text = MAIN_DP.replace(",\"\",00000000", ",\"x\",00000000");
        let m = parse_main("aaaaaaaa-0000-0000-0000-000000000001", &text).unwrap().unwrap();
        assert!(internal_row_text(&m).is_err());
    }
}
```

- [ ] **Step 2: Run** `cargo test --no-default-features --lib external::header` → FAIL (не определено).

- [ ] **Step 3: Реализация**

```rust
//! Main row of an external object (`{1,{<main>},1,{<class>,{1,{4,…},N,…}}}`)
//! and its rewrite into the `DataProcessor` (v17) / `Report` (v19) row that
//! the configuration pipeline decodes. Mapping tables: plan Task 3.

use anyhow::{Context, Result, bail};

use super::{ExternalKind, brace, derived_uuid};

const DATA_PROCESSOR_COMMANDS: &str = "{45556acb-826a-4f73-898a-6025fc9536e1,0}";
const REPORT_COMMANDS: &str = "{e7ff38c0-ec3c-47a0-ae90-20c73ca72246,0}";
const FORMS_COLLECTION: &str = "{d5b0e5ed-256d-401c-9c36-f630cafd8a62";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalMain {
    pub kind: ExternalKind,
    pub main_uuid: String,
    pub object_id: String,
    pub name: String,
    pub header: Vec<String>,
    pub collections: Vec<String>,
}

pub fn parse_main(main_uuid: &str, text: &str) -> Result<Option<ExternalMain>> {
    let text = brace::strip_bom(text).trim_start();
    let Some((top, _)) = brace::fields(text, 0) else { return Ok(None) };
    if top.len() != 4 || top[0] != "1" || top[2] != "1" {
        return Ok(None);
    }
    let Some((wrap, _)) = brace::fields(top[3], 0) else { return Ok(None) };
    let Some(kind) = wrap.first().and_then(|c| ExternalKind::from_class_id(c)) else {
        return Ok(None);
    };
    let inner = wrap.get(1).context("external object wrapper has no body")?;
    let (inner, _) = brace::fields(inner, 0).context("external object body is not a brace list")?;
    if inner.len() < 3 || inner[0] != "1" {
        bail!("unexpected external object body layout");
    }
    let (header, _) = brace::fields(inner[1], 0).context("external header is not a brace list")?;
    if header.first() != Some(&"4") {
        bail!("unsupported external header version `{}`", header.first().unwrap_or(&""));
    }
    let expected = match kind {
        ExternalKind::DataProcessor => 7,
        ExternalKind::Report => 13,
    };
    if header.len() != expected {
        bail!("external {} header has {} fields, expected {expected}", kind.external_kind(), header.len());
    }
    let count: usize = inner[2].parse().context("collection count")?;
    let collections: Vec<String> = inner[3..].iter().map(|s| (*s).to_owned()).collect();
    if collections.len() != count {
        bail!("external object declares {count} collections but has {}", collections.len());
    }
    let (names, _) = brace::fields(header[3], 0).context("header names block")?;
    let (named, _) = brace::fields(names.get(1).context("names block body")?, 0).context("names tuple")?;
    let (id_tuple, _) = brace::fields(named.get(1).context("object id tuple")?, 0).context("object id")?;
    let object_id = id_tuple.get(2).context("object id value")?.to_ascii_lowercase();
    let name = named.get(2).context("object name")?.trim_matches('"').replace("\"\"", "\"");
    Ok(Some(ExternalMain {
        kind,
        main_uuid: main_uuid.to_ascii_lowercase(),
        object_id,
        name,
        header: header.iter().map(|s| (*s).to_owned()).collect(),
        collections,
    }))
}

pub fn internal_row_text(main: &ExternalMain) -> Result<String> {
    let h = &main.header;
    let m1 = derived_uuid(&format!("onecdec-external/manager-type/{}", h[1]));
    let m2 = derived_uuid(&format!("onecdec-external/manager-value/{}", h[2]));
    let mut collections = main.collections.clone();
    let header = match main.kind {
        ExternalKind::DataProcessor => {
            if h[5] != "\"\"" {
                bail!("external data processor header field 5 is `{}`, only \"\" is evidenced", h[5]);
            }
            let at = collections
                .iter()
                .position(|c| c.starts_with(FORMS_COLLECTION))
                .unwrap_or(collections.len());
            collections.insert(at, DATA_PROCESSOR_COMMANDS.to_owned());
            format!("{{17,{},{},\r\n{},{},1,0,{m1},{m2},{},\r\n{{0}},\r\n{{0}}\r\n}}", h[1], h[2], h[3], h[4], h[6])
        }
        ExternalKind::Report => {
            if h[10] != "\"\"" {
                bail!("external report header field 10 is `{}`, only \"\" is evidenced", h[10]);
            }
            collections.push(REPORT_COMMANDS.to_owned());
            format!(
                "{{19,{},{},\r\n{},{},{},{},1,{},{},{},0,{m1},{m2},{},\r\n{{0}},\r\n{{0}},{}}}",
                h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[11], h[12]
            )
        }
    };
    Ok(format!("\u{feff}{{1,\r\n{header},{},\r\n{}\r\n}}", collections.len(), collections.join(",\r\n")))
}
```

- [ ] **Step 4: Run** `cargo test --no-default-features --lib external::header` → PASS (4).
- [ ] **Step 5: Commit** — `feat(external): rewrite external main row into the internal object row`

---

### Task 4: copyinfo → индекс типов

**Files:** Create `src/external/copyinfo.rs`

**Interfaces:**
- Produces:
```rust
pub struct CopyInfoObject { pub id: String, pub path: Vec<(String, String)> } // (class id, name)
pub struct CopyInfoType { pub type_id: String, pub object_id: String, pub index: usize }
pub struct CopyInfo { pub objects: Vec<CopyInfoObject>, pub types: Vec<CopyInfoType> }
pub struct ResolvedType { pub type_id: String, pub name: String, pub category: &'static str } // name: "CatalogRef.Пользователи"
pub fn parse(text: &str) -> anyhow::Result<CopyInfo>;
pub fn resolve(info: &CopyInfo) -> Vec<ResolvedType>;
pub fn generated_types_xml(types: &[ResolvedType]) -> String; // BOM + XML read by mssql_dump::parse_indexed_generated_types_from_source_xml_text
```

Таблица классов (GUID из `src/compiler/root.rs`, порядок порождённых типов снят с `native-8.3.27.2214/src_1cv8`, InternalInfo; одинаков во всех объектах вида):

```rust
const CLASSES: &[(&str, &[(&str, &str)])] = &[
    ("cf4abea6-37b2-11d4-940f-008048da11f9", &[("CatalogObject", "Object"), ("CatalogRef", "Ref"), ("CatalogSelection", "Selection"), ("CatalogList", "List"), ("CatalogManager", "Manager")]),
    ("061d872a-5787-460e-95ac-ed74ea3a3e84", &[("DocumentObject", "Object"), ("DocumentRef", "Ref"), ("DocumentSelection", "Selection"), ("DocumentList", "List"), ("DocumentManager", "Manager")]),
    ("f6a80749-5ad7-400b-8519-39dc5dff2542", &[("EnumRef", "Ref"), ("EnumManager", "Manager"), ("EnumList", "List")]),
    ("13134201-f60b-11d5-a3c7-0050bae0a776", &[("InformationRegisterRecord", "Record"), ("InformationRegisterManager", "Manager"), ("InformationRegisterSelection", "Selection"), ("InformationRegisterList", "List"), ("InformationRegisterRecordSet", "RecordSet"), ("InformationRegisterRecordKey", "RecordKey"), ("InformationRegisterRecordManager", "RecordManager")]),
    ("b64d9a40-1642-11d6-a3c7-0050bae0a776", &[("AccumulationRegisterRecord", "Record"), ("AccumulationRegisterManager", "Manager"), ("AccumulationRegisterSelection", "Selection"), ("AccumulationRegisterList", "List"), ("AccumulationRegisterRecordSet", "RecordSet"), ("AccumulationRegisterRecordKey", "RecordKey")]),
    ("82a1b659-b220-4d94-a9bd-14d757b95a48", &[("ChartOfCharacteristicTypesObject", "Object"), ("ChartOfCharacteristicTypesRef", "Ref"), ("ChartOfCharacteristicTypesSelection", "Selection"), ("ChartOfCharacteristicTypesList", "List"), ("Characteristic", "Characteristic"), ("ChartOfCharacteristicTypesManager", "Manager")]),
    ("857c4a91-e5f4-4fac-86ec-787626f1c108", &[("ExchangePlanObject", "Object"), ("ExchangePlanRef", "Ref"), ("ExchangePlanSelection", "Selection"), ("ExchangePlanList", "List"), ("ExchangePlanManager", "Manager")]),
    ("3e63355c-1378-4953-be9b-1deb5fb6bec5", &[("TaskObject", "Object"), ("TaskRef", "Ref"), ("TaskSelection", "Selection"), ("TaskList", "List"), ("TaskManager", "Manager")]),
    ("fcd3404e-1523-48ce-9bc0-ecdb822684a1", &[("BusinessProcessObject", "Object"), ("BusinessProcessRef", "Ref"), ("BusinessProcessSelection", "Selection"), ("BusinessProcessList", "List"), ("BusinessProcessManager", "Manager"), ("BusinessProcessRoutePointRef", "RoutePointRef")]),
    ("bf845118-327b-4682-b5c6-285d2a0eb296", &[("DataProcessorObject", "Object"), ("DataProcessorManager", "Manager")]),
    ("631b75a0-29e2-11d6-a3c7-0050bae0a776", &[("ReportObject", "Object"), ("ReportManager", "Manager")]),
    ("0195e80c-b157-11d4-9435-004095e12fc7", &[("ConstantManager", "Manager"), ("ConstantValueManager", "ValueManager"), ("ConstantValueKey", "ValueKey")]),
    ("4612bd75-71b7-4a5c-8cc5-2b0b65f9fa0d", &[("DocumentJournalSelection", "Selection"), ("DocumentJournalList", "List"), ("DocumentJournalManager", "Manager")]),
    ("c045099e-13b9-4fb6-9d50-fca00202971e", &[("DefinedType", "DefinedType")]),
    ("3e7bfcc0-067d-11d6-a3c7-0050bae0a776", &[("FilterCriterionManager", "Manager"), ("FilterCriterionList", "List")]),
    ("46b4cd97-fd13-4eaa-aba2-3bddd7699218", &[("SettingsStorageManager", "Manager")]),
    ("c3831ec8-d8d5-4f93-8a22-f9bfae07327f", &[("ExternalDataProcessorObject", "Object")]),
    ("e41aff26-25cf-4bb6-b6c1-3f478a75f374", &[("ExternalReportObject", "Object")]),
];
```

Виды, которых в таблице нет (планы счетов/видов расчёта, регистры бухгалтерии/расчёта, и т.д.), не разрешаются — тип остаётся `TypeId` (как делает сам конвейер), а в отчёт попадает сообщение. Расширение таблицы — только по эталону.

- [ ] **Step 1: Тесты** (строка — реальный `copyinfo` обработки «КартаМаршрутаБизнесПроцесса» корпуса, сокращённая до 3 объектов/2 типов):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "{4,{3,{579baaa4-6493-4d99-8744-6399757295c7,579baaa4-6493-4d99-8744-6399757295c7,1,{cf4abea6-37b2-11d4-940f-008048da11f9,\"Пользователи\"}},{b85b3a58-95e4-4698-b1ae-037901ecc1b1,b85b3a58-95e4-4698-b1ae-037901ecc1b1,1,{f6a80749-5ad7-400b-8519-39dc5dff2542,\"СостоянияБизнесПроцессов\"}},{76389fa9-b9e0-492e-9c93-24b5f3bf094c,76389fa9-b9e0-492e-9c93-24b5f3bf094c,2,{3e63355c-1378-4953-be9b-1deb5fb6bec5,\"Задача\"},{3f58cbfb-4172-4e54-be49-561a579bb38b,\"Реквизит\"}}},{2,{645ed368-b47a-4aaa-9775-2a2cc3ab2ba4,b85b3a58-95e4-4698-b1ae-037901ecc1b1,0},{c54edff0-c3a1-44d7-9707-1fe05700b055,579baaa4-6493-4d99-8744-6399757295c7,1}},{0},{0,0},{0}}";

    #[test]
    fn parses_objects_and_types() {
        let info = parse(SAMPLE).unwrap();
        assert_eq!(info.objects.len(), 3);
        assert_eq!(info.objects[2].path.len(), 2);
        assert_eq!(info.types.len(), 2);
        assert_eq!(info.types[1].index, 1);
    }

    #[test]
    fn resolves_ref_types_by_generated_type_index() {
        let resolved = resolve(&parse(SAMPLE).unwrap());
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "EnumRef.СостоянияБизнесПроцессов");
        assert_eq!(resolved[1].name, "CatalogRef.Пользователи");
        assert_eq!(resolved[1].category, "Ref");
    }

    #[test]
    fn empty_copyinfo_has_no_types() {
        assert!(resolve(&parse("{4,{0},{0},{0},{0,0},{0}}").unwrap()).is_empty());
    }

    #[test]
    fn xml_lists_generated_types() {
        let xml = generated_types_xml(&resolve(&parse(SAMPLE).unwrap()));
        assert!(xml.contains("<xr:GeneratedType name=\"CatalogRef.Пользователи\" category=\"Ref\">"));
        assert!(xml.contains("<xr:TypeId>c54edff0-c3a1-44d7-9707-1fe05700b055</xr:TypeId>"));
    }
}
```

- [ ] **Step 2: Run** → FAIL. **Step 3: Реализация**

```rust
//! `copyinfo` of an external object: the configuration objects it references
//! (id → class + name path) and the type ids it uses (type id → object +
//! generated-type index). The platform uses it to show `cfg:CatalogRef.X`
//! without the configuration; so does this adapter. Layout (spec §4):
//! `{4,{N,{<id>,<id>,k,{<class>,"name"}…}…},{M,{<type>,<id>,<index>}…},…}`.

use anyhow::{Context, Result, bail};

use super::brace;

// CLASSES table from the plan goes here verbatim.

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyInfoObject { pub id: String, pub path: Vec<(String, String)> }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopyInfoType { pub type_id: String, pub object_id: String, pub index: usize }
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CopyInfo { pub objects: Vec<CopyInfoObject>, pub types: Vec<CopyInfoType> }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType { pub type_id: String, pub name: String, pub category: &'static str }

fn unquote(s: &str) -> String {
    s.trim().trim_matches('"').replace("\"\"", "\"")
}

pub fn parse(text: &str) -> Result<CopyInfo> {
    let text = brace::strip_bom(text).trim();
    let (top, _) = brace::fields(text, 0).context("copyinfo is not a brace list")?;
    if top.first() != Some(&"4") || top.len() < 3 {
        bail!("unsupported copyinfo layout");
    }
    let mut info = CopyInfo::default();
    let (objects, _) = brace::fields(top[1], 0).context("copyinfo objects")?;
    for object in objects.iter().skip(1) {
        let (f, _) = brace::fields(object, 0).context("copyinfo object")?;
        let depth: usize = f.get(2).context("copyinfo object depth")?.parse()?;
        let mut path = Vec::with_capacity(depth);
        for step in f.iter().skip(3).take(depth) {
            let (s, _) = brace::fields(step, 0).context("copyinfo path step")?;
            path.push((s[0].to_ascii_lowercase(), unquote(s.get(1).context("path name")?)));
        }
        info.objects.push(CopyInfoObject { id: f[0].to_ascii_lowercase(), path });
    }
    let (types, _) = brace::fields(top[2], 0).context("copyinfo types")?;
    for ty in types.iter().skip(1) {
        let (f, _) = brace::fields(ty, 0).context("copyinfo type")?;
        info.types.push(CopyInfoType {
            type_id: f.first().context("type id")?.to_ascii_lowercase(),
            object_id: f.get(1).context("type object")?.to_ascii_lowercase(),
            index: f.get(2).context("type index")?.parse()?,
        });
    }
    Ok(info)
}

pub fn resolve(info: &CopyInfo) -> Vec<ResolvedType> {
    let mut out = Vec::new();
    for ty in &info.types {
        let Some(object) = info.objects.iter().find(|o| o.id == ty.object_id) else { continue };
        let [(class, name)] = object.path.as_slice() else { continue };
        let Some((_, generated)) = CLASSES.iter().find(|(c, _)| *c == class) else { continue };
        let Some((prefix, category)) = generated.get(ty.index) else { continue };
        out.push(ResolvedType { type_id: ty.type_id.clone(), name: format!("{prefix}.{name}"), category });
    }
    out
}

pub fn generated_types_xml(types: &[ResolvedType]) -> String {
    let mut xml = String::from("\u{feff}<MetaDataObject xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\"><External><InternalInfo>\r\n");
    for t in types {
        xml.push_str(&format!(
            "<xr:GeneratedType name=\"{}\" category=\"{}\"><xr:TypeId>{}</xr:TypeId></xr:GeneratedType>\r\n",
            t.name, t.category, t.type_id
        ));
    }
    xml.push_str("</InternalInfo></External></MetaDataObject>\r\n");
    xml
}
```

Имена в XML экранировать не нужно: имена объектов 1С — идентификаторы (буквы/цифры/`_`).

- [ ] **Step 4: Run** → PASS (4). **Step 5: Commit** — `feat(external): copyinfo type references`

---

### Task 5: `versions` и переименование ссылок

**Files:** Create `src/external/versions.rs`, `src/external/rename.rs`

**Interfaces:**
- Produces: `versions::rewrite(text: &str, rename: &[(&str, &str)], drop: &[&str], add: &[(&str, &str)]) -> anyhow::Result<String>`; `rename::to_external_references(text: &str, kind: ExternalKind, name: &str) -> String`.

- [ ] **Step 1: Тесты**

```rust
// versions.rs
#[cfg(test)]
mod tests {
    use super::*;
    const V: &str = "\u{feff}{1,4,\"\",00000000-0000-0000-0000-00000000000a,\"main\",00000000-0000-0000-0000-00000000000b,\"copyinfo\",00000000-0000-0000-0000-00000000000c,\"root\",00000000-0000-0000-0000-00000000000d}";

    #[test]
    fn renames_drops_and_adds_entries_and_recounts() {
        let out = rewrite(V, &[("main", "obj")], &["copyinfo"], &[("types", "00000000-0000-0000-0000-00000000000e")]).unwrap();
        assert_eq!(out, "\u{feff}{1,4,\"\",00000000-0000-0000-0000-00000000000a,\"obj\",00000000-0000-0000-0000-00000000000b,\"root\",00000000-0000-0000-0000-00000000000d,\"types\",00000000-0000-0000-0000-00000000000e}");
    }
}

// rename.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renames_object_and_type_references() {
        let t = "cfg:DataProcessorObject.X|DataProcessor.X.Form.Ф|DataProcessorTabularSectionRow.X.Т";
        assert_eq!(
            to_external_references(t, ExternalKind::DataProcessor, "X"),
            "cfg:ExternalDataProcessorObject.X|ExternalDataProcessor.X.Form.Ф|ExternalDataProcessorTabularSectionRow.X.Т"
        );
    }

    #[test]
    fn rename_respects_identifier_boundary() {
        let t = "Report.X2 ExternalReport.X Report.X.Template.Т";
        assert_eq!(
            to_external_references(t, ExternalKind::Report, "X"),
            "Report.X2 ExternalReport.X ExternalReport.X.Template.Т"
        );
    }
}
```

- [ ] **Step 2: Run** → FAIL. **Step 3: Реализация**

```rust
// versions.rs
//! The `versions` service entry (`{1,N,"",<ver>,"<entry>",<ver>,…}`): a version
//! id per container entry. The pipeline checks it against the entry list, so
//! it follows the adapter's renames, drops and additions.

use anyhow::{Context, Result, bail};

use super::brace;

pub fn rewrite(text: &str, rename: &[(&str, &str)], drop: &[&str], add: &[(&str, &str)]) -> Result<String> {
    let (f, _) = brace::fields(brace::strip_bom(text).trim(), 0).context("versions is not a brace list")?;
    if f.first() != Some(&"1") || f.len() < 2 || (f.len() - 2) % 2 != 0 {
        bail!("unsupported versions layout");
    }
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in f[2..].chunks(2) {
        let name = pair[0].trim_matches('"');
        if drop.contains(&name) {
            continue;
        }
        let name = rename.iter().find(|(from, _)| *from == name).map_or(name, |(_, to)| to);
        pairs.push((name.to_owned(), pair[1].to_owned()));
    }
    pairs.extend(add.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())));
    let body: Vec<String> = pairs.iter().map(|(n, v)| format!("\"{n}\",{v}")).collect();
    Ok(format!("\u{feff}{{1,{},{}}}", pairs.len(), body.join(",")))
}

// rename.rs
//! `DataProcessor.X…` → `ExternalDataProcessor.X…` (and `Report` → `ExternalReport`)
//! in the produced XML/HTML: the pipeline names the object as a configuration
//! object; the platform names it as an external one. Only whole identifiers.

use super::ExternalKind;

const VARIANTS: [&str; 4] = ["TabularSectionRow", "TabularSection", "Object", ""];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

pub fn to_external_references(text: &str, kind: ExternalKind, name: &str) -> String {
    let internal = kind.internal_kind();
    let mut out = text.to_owned();
    for variant in VARIANTS {
        let needle = format!("{internal}{variant}.{name}");
        let mut result = String::with_capacity(out.len());
        let mut rest = out.as_str();
        while let Some(at) = rest.find(&needle) {
            let before = rest[..at].chars().next_back();
            let after = rest[at + needle.len()..].chars().next();
            let whole = before.is_none_or(|c| !is_ident(c) && c != '.') && after.is_none_or(|c| !is_ident(c));
            result.push_str(&rest[..at]);
            if whole {
                result.push_str("External");
            }
            result.push_str(&needle);
            rest = &rest[at + needle.len()..];
        }
        result.push_str(rest);
        out = result;
    }
    out
}
```

Замечание по порядку: `TabularSectionRow` идёт раньше `TabularSection`, иначе `DataProcessorTabularSection.X` не совпадёт с `…Row.X` (после `TabularSection` идёт `R`, а не `.`) — коллизий нет, но порядок фиксирован для ясности.

- [ ] **Step 4: Run** → PASS (3). **Step 5: Commit** — `feat(external): versions rewrite and external reference renaming`

---

### Task 6: Корневой XML, точка входа в mssql_dump и оркестровка выгрузки

**Files:**
- Create: `src/external/root_xml.rs`, `src/external/export.rs`
- Modify: `src/mssql_dump/mod.rs:2545-2583` (вынести тело в `export_packed_entries_to_source`)
- Modify: `src/commands/cf.rs:755-775` (`export()`)

**Interfaces:**
- Consumes: всё из Task 2–5.
- Produces:
```rust
// mssql_dump
pub fn export_packed_entries_to_source(
    source_profile: &str,
    entries: Vec<(String, Vec<u8>)>,   // name, packed (raw-deflate) payload
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport>;
// external
pub fn root_xml::to_external_root(xml: &str, main: &ExternalMain) -> anyhow::Result<String>;
pub fn export::export_if_external(archive: &PackedCfArchive, output_dir: &Path, overwrite: bool,
    source_version: InfobaseConfigSourceVersion) -> anyhow::Result<Option<StorageImageSourceExportReport>>;
pub const export::TYPES_ROW: &str = "onecdec-external-types";
```

- [ ] **Step 1: Тест root_xml** (фрагмент — начало реальной выгрузки опыта `spike/ИнформацияПриЗапуске/out/DataProcessors/ИнформацияПриЗапуске.xml`, сокращённый):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::external::{ExternalKind, header::ExternalMain};

    fn main_dp() -> ExternalMain {
        ExternalMain {
            kind: ExternalKind::DataProcessor,
            main_uuid: "d9704b20-c29c-4e7f-b777-0ac9080a5631".into(),
            object_id: "3b58e713-b1af-4db7-84e7-e4e30aff1c21".into(),
            name: "Инфо".into(),
            header: vec![],
            collections: vec![],
        }
    }

    const INTERNAL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<MetaDataObject version=\"2.20\">\r\n\t<DataProcessor uuid=\"3b58e713-b1af-4db7-84e7-e4e30aff1c21\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:GeneratedType name=\"DataProcessorObject.Инфо\" category=\"Object\">\r\n\t\t\t\t<xr:TypeId>1</xr:TypeId>\r\n\t\t\t\t<xr:ValueId>2</xr:ValueId>\r\n\t\t\t</xr:GeneratedType>\r\n\t\t\t<xr:GeneratedType name=\"DataProcessorManager.Инфо\" category=\"Manager\">\r\n\t\t\t\t<xr:TypeId>3</xr:TypeId>\r\n\t\t\t\t<xr:ValueId>4</xr:ValueId>\r\n\t\t\t</xr:GeneratedType>\r\n\t\t</InternalInfo>\r\n\t\t<Properties>\r\n\t\t\t<Name>Инфо</Name>\r\n\t\t\t<Comment/>\r\n\t\t\t<UseStandardCommands>true</UseStandardCommands>\r\n\t\t\t<DefaultForm>DataProcessor.Инфо.Form.Форма</DefaultForm>\r\n\t\t\t<AuxiliaryForm/>\r\n\t\t\t<IncludeHelpInContents>false</IncludeHelpInContents>\r\n\t\t\t<ExtendedPresentation>\r\n\t\t\t\t<v8:item/>\r\n\t\t\t</ExtendedPresentation>\r\n\t\t\t<Explanation/>\r\n\t\t</Properties>\r\n\t\t<ChildObjects/>\r\n\t</DataProcessor>\r\n</MetaDataObject>";

    #[test]
    fn converts_internal_root_into_external_root() {
        let xml = to_external_root(INTERNAL, &main_dp()).unwrap();
        assert!(xml.contains("\t<ExternalDataProcessor uuid=\"d9704b20-c29c-4e7f-b777-0ac9080a5631\">\r\n\t\t<InternalInfo>\r\n\t\t\t<xr:ContainedObject>\r\n\t\t\t\t<xr:ClassId>c3831ec8-d8d5-4f93-8a22-f9bfae07327f</xr:ClassId>\r\n\t\t\t\t<xr:ObjectId>3b58e713-b1af-4db7-84e7-e4e30aff1c21</xr:ObjectId>\r\n\t\t\t</xr:ContainedObject>\r\n\t\t\t<xr:GeneratedType name=\"ExternalDataProcessorObject.Инфо\""));
        assert!(!xml.contains("Manager"));
        for gone in ["UseStandardCommands", "IncludeHelpInContents", "ExtendedPresentation", "Explanation"] {
            assert!(!xml.contains(gone), "{gone} left");
        }
        assert!(xml.contains("<DefaultForm>ExternalDataProcessor.Инфо.Form.Форма</DefaultForm>"));
        assert!(xml.ends_with("\t</ExternalDataProcessor>\r\n</MetaDataObject>"));
    }
}
```

- [ ] **Step 2: Run** → FAIL. **Step 3: Реализация `root_xml.rs`**

```rust
//! The root `<DataProcessor>`/`<Report>` XML the pipeline writes → the
//! platform's `<ExternalDataProcessor>`/`<ExternalReport>`: main uuid,
//! ContainedObject, no manager type, only the external property set.

use anyhow::{Result, bail};

use super::{header::ExternalMain, rename::to_external_references};

const DROPPED_PROPERTIES: [&str; 4] = ["UseStandardCommands", "IncludeHelpInContents", "ExtendedPresentation", "Explanation"];

fn remove_root_property(xml: &str, tag: &str) -> String {
    let empty = format!("\r\n\t\t\t<{tag}/>");
    if let Some(at) = xml.find(&empty) {
        return format!("{}{}", &xml[..at], &xml[at + empty.len()..]);
    }
    let open = format!("\r\n\t\t\t<{tag}>");
    let close = format!("</{tag}>");
    if let Some(at) = xml.find(&open)
        && let Some(end) = xml[at..].find(&close)
    {
        return format!("{}{}", &xml[..at], &xml[at + end + close.len()..]);
    }
    xml.to_owned()
}

pub fn to_external_root(xml: &str, main: &ExternalMain) -> Result<String> {
    let internal = main.kind.internal_kind();
    let external = main.kind.external_kind();
    let open = format!("\t<{internal} uuid=\"{}\">", main.object_id);
    if !xml.contains(&open) {
        bail!("root XML does not open with `{open}`");
    }
    let mut out = xml.replacen(&open, &format!("\t<{external} uuid=\"{}\">", main.main_uuid), 1);
    out = out.replacen(&format!("\t</{internal}>"), &format!("\t</{external}>"), 1);
    let contained = format!(
        "\t\t<InternalInfo>\r\n\t\t\t<xr:ContainedObject>\r\n\t\t\t\t<xr:ClassId>{}</xr:ClassId>\r\n\t\t\t\t<xr:ObjectId>{}</xr:ObjectId>\r\n\t\t\t</xr:ContainedObject>\r\n",
        main.kind.class_id(),
        main.object_id
    );
    if !out.contains("\t\t<InternalInfo>\r\n") {
        bail!("root XML has no InternalInfo");
    }
    out = out.replacen("\t\t<InternalInfo>\r\n", &contained, 1);
    let manager = format!("\t\t\t<xr:GeneratedType name=\"{internal}Manager.");
    if let Some(at) = out.find(&manager) {
        let end = out[at..].find("</xr:GeneratedType>\r\n").map(|e| at + e + "</xr:GeneratedType>\r\n".len());
        let Some(end) = end else { bail!("unterminated manager GeneratedType") };
        out.replace_range(at..end, "");
    }
    for tag in DROPPED_PROPERTIES {
        out = remove_root_property(&out, tag);
    }
    Ok(to_external_references(&out, main.kind, &main.name))
}
```

- [ ] **Step 4: Точка входа в `mssql_dump`**: тело `export_packed_cf_archive_to_source` перенести в новую функцию, старую сделать обёрткой:

```rust
pub fn export_packed_cf_archive_to_source(
    archive: PackedCfArchive,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    let (_, source_profile, entries) = archive.into_parts();
    let entries = entries.into_iter().map(|entry| entry.into_parts()).collect();
    export_packed_entries_to_source(source_profile.as_str(), entries, output_dir, overwrite, source_version)
}

/// Same as [`export_packed_cf_archive_to_source`] for entries already taken
/// out of a container (the external-object adapter rewrites a few of them).
pub fn export_packed_entries_to_source(
    source_profile: &str,
    entries: Vec<(String, Vec<u8>)>,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<StorageImageSourceExportReport> {
    let physical_entries = entries.len();
    let mut rows = Vec::with_capacity(physical_entries);
    let mut records = Vec::with_capacity(physical_entries);
    for (name, payload) in entries {
        // unchanged loop body from the old function
    }
    export_direct_storage_rows_to_source(rows, records, Some(source_profile.to_owned()), physical_entries, output_dir, overwrite, source_version)
}
```

Run: `cargo test --no-default-features --test cf_export` → PASS (регрессии нет).

- [ ] **Step 5: `export.rs`**

```rust
//! `cf export` of an external object: adapt the entries, run the
//! configuration pipeline, then move and rename its output (plan Task 6).

use std::{fs, path::{Path, PathBuf}};

use anyhow::{Context, Result};
use ibcmd_cf::archive::PackedCfArchive;
use ibcmd_cf::export::{StorageExportDisposition, StorageExportEntryReport};

use crate::legacy_version::InfobaseConfigSourceVersion;
use crate::module_blob::{deflate_raw, inflate_raw};
use crate::mssql_dump::{self, StorageImageSourceExportReport};

use super::{brace, copyinfo, derived_uuid, header::{self, ExternalMain}, rename, root_xml, versions};

pub const TYPES_ROW: &str = "onecdec-external-types";

fn text_of(payload: &[u8]) -> Result<String> {
    Ok(String::from_utf8(inflate_raw(payload)?)?)
}

/// `Some(main)` when `root` names a main row wrapped as an external object.
pub fn detect(entries: &[(String, Vec<u8>)]) -> Result<Option<ExternalMain>> {
    let Some((_, root)) = entries.iter().find(|(n, _)| n == "root") else { return Ok(None) };
    let root = text_of(root)?;
    let Some((f, _)) = brace::fields(brace::strip_bom(&root).trim(), 0) else { return Ok(None) };
    if f.first() != Some(&"2") || f.get(2).is_some_and(|h| !h.is_empty()) {
        return Ok(None); // a configuration root carries a hash in the third field
    }
    let Some(main_uuid) = f.get(1) else { return Ok(None) };
    let Some((_, main)) = entries.iter().find(|(n, _)| n.eq_ignore_ascii_case(main_uuid)) else { return Ok(None) };
    header::parse_main(main_uuid, &text_of(main)?)
}

pub fn export_if_external(
    archive: &PackedCfArchive,
    output_dir: &Path,
    overwrite: bool,
    source_version: InfobaseConfigSourceVersion,
) -> Result<Option<StorageImageSourceExportReport>> {
    let entries: Vec<(String, Vec<u8>)> =
        archive.entries().iter().map(|e| (e.name().to_owned(), e.payload().to_vec())).collect();
    let Some(main) = detect(&entries)? else { return Ok(None) };

    let mut adapted = Vec::with_capacity(entries.len() + 1);
    let mut types_added = false;
    for (name, payload) in &entries {
        if name.eq_ignore_ascii_case(&main.main_uuid) {
            adapted.push((main.object_id.clone(), deflate_raw(header::internal_row_text(&main)?.as_bytes())?));
        } else if name == "copyinfo" {
            let resolved = copyinfo::resolve(&copyinfo::parse(&text_of(payload)?)?);
            if !resolved.is_empty() {
                adapted.push((TYPES_ROW.to_owned(), deflate_raw(copyinfo::generated_types_xml(&resolved).as_bytes())?));
                types_added = true;
            }
        } else {
            adapted.push((name.clone(), payload.clone()));
        }
    }
    let types_version = derived_uuid("onecdec-external/types-row-version");
    for (name, payload) in adapted.iter_mut() {
        if name == "versions" {
            let add = if types_added { vec![(TYPES_ROW, types_version.as_str())] } else { vec![] };
            let text = versions::rewrite(&text_of(payload)?, &[(main.main_uuid.as_str(), main.object_id.as_str())], &["copyinfo"], &add)?;
            *payload = deflate_raw(text.as_bytes())?;
        }
    }

    let mut report = mssql_dump::export_packed_entries_to_source(
        archive.source_profile().as_str(), adapted, output_dir, overwrite, source_version,
    )?;
    finish(output_dir, &main, &mut report)?;
    Ok(Some(report))
}

fn finish(output_dir: &Path, main: &ExternalMain, report: &mut StorageImageSourceExportReport) -> Result<()> {
    let folder = output_dir.join(main.kind.internal_folder());
    let name = &main.name;
    let (from_dir, to_dir) = (folder.join(name), output_dir.join(name));
    let (from_xml, to_xml) = (folder.join(format!("{name}.xml")), output_dir.join(format!("{name}.xml")));
    if to_dir.exists() {
        fs::remove_dir_all(&to_dir).with_context(|| format!("failed to clear {}", to_dir.display()))?;
    }
    if from_dir.exists() {
        fs::rename(&from_dir, &to_dir).context("move object folder")?;
    }
    let root = fs::read_to_string(&from_xml).with_context(|| format!("no root XML {}", from_xml.display()))?;
    fs::write(&to_xml, root_xml::to_external_root(&root, main)?)?;
    fs::remove_file(&from_xml)?;
    if folder.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false) {
        fs::remove_dir(&folder)?;
    }
    let dump_info = output_dir.join("ConfigDumpInfo.xml");
    if dump_info.exists() {
        fs::remove_file(dump_info)?; // native .epf/.erf dumps have none
    }
    if to_dir.exists() {
        for entry in walkdir::WalkDir::new(&to_dir) {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type().is_file() && path.extension().is_some_and(|e| e == "xml" || e == "html") {
                let text = fs::read_to_string(path)?;
                let renamed = rename::to_external_references(&text, main.kind, name);
                if renamed != text {
                    fs::write(path, renamed)?;
                }
            }
        }
    }
    rewrite_report(report, main);
    Ok(())
}

fn rewrite_report(report: &mut StorageImageSourceExportReport, main: &ExternalMain) {
    let prefix = format!("{}/", main.kind.internal_folder());
    let storage = &mut report.storage;
    storage.entries.retain(|e| e.logical_name != TYPES_ROW);
    for e in &mut storage.entries {
        if e.logical_name == main.object_id {
            e.logical_name = main.main_uuid.clone();
            e.logical_key = main.main_uuid.clone();
        }
        for o in &mut e.outputs {
            if let Some(rest) = o.strip_prefix(&prefix) {
                *o = rest.to_owned();
            }
        }
        e.outputs.retain(|o| o != "ConfigDumpInfo.xml");
    }
    storage.entries.push(StorageExportEntryReport {
        logical_name: "copyinfo".into(),
        logical_key: "copyinfo".into(),
        part_count: 1,
        packed_bytes: 0,
        disposition: StorageExportDisposition::Supported,
        outputs: Vec::new(),
        message: Some("configuration references used to resolve types".into()),
    });
    storage.supported = storage.entries.iter().filter(|e| e.disposition == StorageExportDisposition::Supported).count();
    storage.opaque = storage.entries.iter().filter(|e| e.disposition == StorageExportDisposition::Opaque).count();
    storage.failed = storage.entries.iter().filter(|e| e.disposition == StorageExportDisposition::Failed).count();
    storage.logical_entries = storage.entries.len();
}
```

(`versions` проходит в цикле как обычная запись и переписывается вторым проходом, когда уже известно, добавлена ли строка типов.)

- [ ] **Step 6: Точка входа в `cf.rs` `export()`** — заменить вызов `mssql_dump::export_packed_cf_archive_to_source(...)`:

```rust
    let external = crate::external::export::export_if_external(
        &archive,
        &args.output_dir,
        args.overwrite,
        args.source_version,
    )
    .map_err(|source| {
        export_failure(&args, profile.clone(), "export_failed", format!("failed to export external object: {source:#}"))
    })?;
    let export = match external {
        Some(report) => report,
        None => mssql_dump::export_packed_cf_archive_to_source(
            archive,
            &args.output_dir,
            args.overwrite,
            args.source_version,
        )
        .map_err(|source| {
            export_failure(&args, profile.clone(), "export_failed", format!("failed to export CF storage image: {source:#}"))
        })?,
    };
```

- [ ] **Step 7: Тест «конфигурация — не внешний объект»** (в `export.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_configuration_is_not_external() {
        let root = deflate_raw("\u{feff}{2,30ffe4cc-eef2-4371-8b26-046597e37e22,h55/Mge2fMFz==}".as_bytes()).unwrap();
        let entries = vec![("root".to_owned(), root)];
        assert!(detect(&entries).unwrap().is_none());
    }
}
```

- [ ] **Step 8: Run** `cargo test --no-default-features --lib external` → PASS; `cargo test --no-default-features --test external_export` → ожидаемо ещё FAIL только на модуле объекта/справке (Task 7) — зафиксировать фактический список расхождений в сообщении коммита.

- [ ] **Step 9: Commit** — `feat(external): cf export of .epf/.erf through the configuration pipeline`

---

### Task 7: Модуль объекта и справка внешнего объекта

**Files:** Modify по результату диагностики — `src/module_blob.rs` (`read_element_from_blob`) или `src/external/export.rs`.

**Interfaces:** без новых публичных имён.

Проба показала: `<ObjectId>.0` (модуль, в т.ч. пустой BOM-only `text`) уходит в `opaque`, хотя тот же модуль в `.cf` выгружается. Единственная видимая разница байтов: у контейнера модуля во внешнем файле `storage_version = 1` (байты 8..12), в `.cf` — `2`.

- [ ] **Step 1: Диагностический тест** в `src/module_blob.rs` (тесты модуля):

```rust
#[test]
fn reads_text_of_module_container_with_storage_version_one() {
    // real `<ObjectId>.0` of tests/fixtures/external/test_processor/input.epf (inflated)
    let blob = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/external/test_processor/object_module.bin")).unwrap();
    let text = unpack_module_blob_text(&blob).expect("module text");
    assert!(text.starts_with(b"\xef\xbb\xbf"));
}
```

`object_module.bin` вынуть: `python _onecdec/v8c.py tests/fixtures/external/test_processor/input.epf --get <ObjectId>.0 --out tests/fixtures/external/test_processor/` и переименовать.

- [ ] **Step 2: Run** → если FAIL: причина в `read_element_from_blob`/`ibcmd-v8` (версия хранения) — ослабить проверку версии до `{1,2}` с комментарием-свидетельством (корпус: 81 .epf, все `1`). Если PASS: причина в индексе путей — добавить в `export.rs` после экспорта явную запись `Ext/ObjectModule.bsl` из `<ObjectId>.0` (`crate::module_blob::unpack_module_blob_text`) и справку `<ObjectId>.1` через `SourceAssetKind::Help`-путь; тест `external_data_processor_matches_native_dump` должен пройти.

- [ ] **Step 3: Run** `cargo test --no-default-features --test external_export` → PASS оба теста.
- [ ] **Step 4: Commit** — `fix(external): object module and help of external objects`

---

### Task 8: Корпус-храповик (ratchet) и хвост расхождений

**Files:**
- Create: `tests/external_corpus.rs`
- Create: `tests/fixtures/external/corpus-baseline.txt` (список относительных путей «совпадает с эталоном»)

**Interfaces:** Consumes: бинарь `ibcmd-rs`.

- [ ] **Step 1: Тест (ignored, env-gated)**

```rust
//! Ratchet over the local .epf/.erf corpus (not in git): every file listed
//! in corpus-baseline.txt must stay byte-identical to the platform's dump.
//! IBCMD_ONECDEC_CORPUS=<root> cargo test --release --no-default-features --test external_corpus -- --ignored
//! IBCMD_UPDATE_BASELINE=1 additionally rewrites the baseline with every file that matches now.

mod common;

use std::{collections::BTreeSet, fs, path::Path};

#[test]
#[ignore = "needs IBCMD_ONECDEC_CORPUS"]
fn external_corpus_does_not_regress() {
    let Ok(root) = std::env::var("IBCMD_ONECDEC_CORPUS") else { return };
    let root = Path::new(&root);
    let work = Path::new(env!("CARGO_TARGET_TMPDIR")).join("external-corpus");
    let mut identical = BTreeSet::new();
    let (mut total, mut same) = (0usize, 0usize);
    for entry in fs::read_dir(root.join("ext-bin")).unwrap() {
        let path = entry.unwrap().path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else { continue };
        let out = work.join(&stem);
        let _ = common::export(&path, &out);
        let native = common::files(&root.join("native-8.3.27.2214/ext-cfg").join(&stem));
        let ours = common::files(&out);
        for (rel, bytes) in &native {
            if rel == ".complete" { continue; }
            total += 1;
            if ours.get(rel) == Some(bytes) {
                same += 1;
                identical.insert(format!("{stem}/{rel}"));
            }
        }
    }
    eprintln!("external corpus: {same}/{total} files identical");
    let baseline_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/external/corpus-baseline.txt");
    if std::env::var_os("IBCMD_UPDATE_BASELINE").is_some() {
        let text: Vec<_> = identical.iter().cloned().collect();
        fs::write(&baseline_path, text.join("\n") + "\n").unwrap();
    }
    let baseline = fs::read_to_string(&baseline_path).unwrap_or_default();
    let regressed: Vec<_> = baseline.lines().filter(|l| !l.is_empty() && !identical.contains(*l)).collect();
    assert!(regressed.is_empty(), "regressed vs baseline: {regressed:?}");
}
```

`tests/common/mod.rs` — перенести туда `files`, `assert_tree_eq`, `export` из `tests/external_export.rs` (сделать `pub fn`), а в `external_export.rs` заменить их на `mod common; use common::*;`.

- [ ] **Step 2:** Первый прогон с `IBCMD_UPDATE_BASELINE=1` → закоммитить baseline.
- [ ] **Step 3:** Разобрать оставшиеся расхождения по группам (скрипт `_onecdec/spike_compare.py` как образец), на каждую группу — отдельный коммит «тест + исправление + обновлённый baseline». Известные группы из пробы: ссылки справки `../id<main>/…` на собственный объект; картинки общей конфигурации в формах; формы 8.5 (не наше — только задокументировать).
- [ ] **Step 4: Commit** после каждой группы.

---

## Self-review (выполнен при написании)

- Покрытие spec: выгрузка §4 «Поток выгрузки» — Task 2–7; критерии §5 (эталон ext-cfg, корпус, fail closed, upstream-тесты) — Task 1, 6 (cf_export), 8; загрузка — **план 2** (пишется после этого плана, т.к. зависит от найденного здесь).
- Типы/имена: `ExternalMain`, `internal_row_text`, `to_external_references`, `to_external_root`, `export_packed_entries_to_source`, `TYPES_ROW` — согласованы между задачами.
- Review Focus 1–5 закреплены тестами в Task 7, 4, 5, 6, 6.
