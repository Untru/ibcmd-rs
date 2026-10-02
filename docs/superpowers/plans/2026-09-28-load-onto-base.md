# Загрузка правок поверх исходного файла — план реализации (план 2 из 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `ibcmd-rs cf load <дерево> <выход> --base <исходный .cf|.epf|.erf>` собирает файл, в котором правки модулей и форм из дерева наложены на исходный контейнер, без платформы 1С.

**Architecture:** spec §«Загрузка v1». База выгружается во временный каталог штатным `cf export` (для .epf/.erf — адаптером плана 1), дерево пользователя сравнивается с ней пофайлово, изменённые файлы по отчёту выгрузки (ключ записи → файлы) превращаются в запросы штатного `overlay` (`SourcePayload::ModuleText`, `SourcePayload::NeedsBase` для тел форм), и `publish_overlay_new` пишет новый контейнер из **исходного** (неизменённые записи побайтно, `versions` обновляется штатно). Любые другие изменения — отказ со списком файлов.

**Tech Stack:** Rust 2024, crate `ibcmd-rs`; `ibcmd_cf::overlay`, `crate::compiler::{CompileAxes, CompileRequest, SourcePayload, overlay::compile_overlay_with_retained_budget}`, `crate::module_blob::{pack_form_body_blob_from_form_xml, pack_form_body_blob_from_module_text, patch_versions_blob_bytes_allowing_additions}`.

**Spec:** `docs/superpowers/specs/2026-09-28-offline-external-epf-erf-design.md` (раздел «Загрузка v1»)

## Global Constraints

- Не менять поведение существующих команд; новый код — `src/load.rs` (+ 1 строка в `src/lib.rs`), CLI — `src/cli.rs` (новая подкоманда `cf load`), `src/commands/cf.rs` (обработчик).
- Fail closed: изменения, которые v1 не умеет (метаданные, макеты, справка, картинки, добавление/удаление файлов кроме замены `.bin`→`.bsl`), — ошибка `unsupported_changes` со списком путей; файл не пишется.
- `.cfe` не поддерживается (нет `versions`) — понятная ошибка от overlay, не паника.
- Существующий выходной файл не перезаписывается (как у `cf overlay`).
- Сравнение деревьев — побайтно; переводы строк не нормализуются.

## Review Focus

1. Дерево без изменений → результат побайтно равен базе по содержимому записей (кроме `versions`? нет — при пустом наборе правок `load` должен отказать «нет изменений» или вернуть копию; решение: ошибка `no_changes`) — тест `load_without_changes_is_refused`.
2. Замена закрытого модуля: в дереве нет `Ext/Module.bin`, зато есть `Ext/Module.bsl` → правка модуля записи, владевшей `.bin` — тест `closed_module_bin_replaced_by_bsl_is_a_module_edit`.
3. Одновременная правка `Form.xml` и `Form/Module.bsl` одной формы → одна запись тела формы с обоими — тест `form_xml_and_module_edit_together`.
4. Правка файла, которого нет в отчёте выгрузки базы (не отображается в ключ) → отказ, а не тихий пропуск — тест `unmapped_change_is_refused`.
5. Путь дерева с кириллицей и пробелами (Windows) — все тесты используют кириллические имена фикстур.

---

## Карта файлов

| Файл | Ответственность |
|---|---|
| `src/load.rs` | сравнение деревьев, классификация правок, запросы overlay, кодек, `load_onto_base` |
| `src/cli.rs` | `CfLoadArgs`, `CfCommands::Load` |
| `src/commands/cf.rs` | `fn load(args)` → отчёт JSON |
| `tests/cf_load.rs` | интеграционные тесты (фикстуры плана 1 + .cf из `tests/fixtures/cf`) |
| `_onecdec/verify_load.py` | приёмка платформой на корпусе |

---

### Task 1: Сравнение деревьев и карта «файл → ключ записи»

**Files:** Create `src/load.rs` (часть), Modify `src/lib.rs`.

**Interfaces — Produces:**
```rust
pub struct TreeDiff { pub changed: Vec<String>, pub added: Vec<String>, pub removed: Vec<String> } // '/'-paths, sorted
pub fn diff_trees(base: &Path, edited: &Path) -> anyhow::Result<TreeDiff>;
pub fn keys_by_output(report: &StorageImageSourceExportReport) -> BTreeMap<String, String>; // output path → storage key
```

- [ ] **Step 1: Тесты** (`#[cfg(test)]` в `src/load.rs`, временные каталоги через `std::env::temp_dir()` + pid):

```rust
#[test]
fn diff_reports_changed_added_and_removed_files() {
    let (a, b) = (tmp("a"), tmp("b"));
    write(&a, "Обработка/Ext/ObjectModule.bsl", "old");
    write(&b, "Обработка/Ext/ObjectModule.bsl", "new");
    write(&a, "Обработка/Ext/Module.bin", "x");
    write(&b, "Обработка/Ext/Module.bsl", "y");
    write(&a, "Обработка.xml", "same");
    write(&b, "Обработка.xml", "same");
    let d = diff_trees(&a, &b).unwrap();
    assert_eq!(d.changed, vec!["Обработка/Ext/ObjectModule.bsl"]);
    assert_eq!(d.added, vec!["Обработка/Ext/Module.bsl"]);
    assert_eq!(d.removed, vec!["Обработка/Ext/Module.bin"]);
}
```

- [ ] **Step 2–4:** RED (не определено) → реализация (`walkdir`, пути через `strip_prefix` и `replace('\\', "/")`, `BTreeSet` для сортировки) → GREEN.
- [ ] **Step 5: Commit** `feat(load): tree diff and output-to-key map`

### Task 2: Классификация правок

**Interfaces — Produces:**
```rust
pub enum Edit {
    Module { key: String, text_path: String },                 // ObjectModule/Module/ManagerModule/.. .bsl
    FormBody { key: String, form_xml: Option<String>, module: Option<String> }, // paths
}
pub struct Plan { pub edits: Vec<Edit>, pub unsupported: Vec<String> }
pub fn classify(diff: &TreeDiff, keys: &BTreeMap<String, String>) -> Plan;
```

Правила (пути относительны корня дерева):
- `…/Ext/Form/Module.bsl` → тело формы: ключ записи, в выходах которой есть `…/Ext/Form.xml` (у тела формы это одна запись `<form>.0`); `module = Some(path)`.
- `…/Ext/Form.xml` → то же тело формы, `form_xml = Some(path)`; обе правки одной формы сливаются в один `FormBody`.
- `…/Ext/<X>.bsl`, изменённый → `Module { key: keys[path] }`.
- добавленный `…/Ext/<X>.bsl` при удалённом `…/Ext/<X>.bin` (тот же каталог и имя) → `Module { key: keys[".bin"-путь] }` (сценарий декомпилятора).
- всё остальное (включая изменённые/добавленные/удалённые `.xml`, `.html`, `Template.*`, картинки, и любой путь без ключа) → `unsupported`.

- [ ] **Step 1: Тесты** — `closed_module_bin_replaced_by_bsl_is_a_module_edit`, `form_xml_and_module_edit_together`, `unmapped_change_is_refused`, `metadata_xml_change_is_unsupported` (строятся из `TreeDiff` и карты ключей вручную):

```rust
#[test]
fn closed_module_bin_replaced_by_bsl_is_a_module_edit() {
    let diff = TreeDiff { changed: vec![], added: vec!["CommonModules/М/Ext/Module.bsl".into()], removed: vec!["CommonModules/М/Ext/Module.bin".into()] };
    let keys = BTreeMap::from([("CommonModules/М/Ext/Module.bin".to_string(), "u1.0".to_string())]);
    let plan = classify(&diff, &keys);
    assert!(plan.unsupported.is_empty());
    assert!(matches!(&plan.edits[..], [Edit::Module { key, text_path }] if key == "u1.0" && text_path == "CommonModules/М/Ext/Module.bsl"));
}

#[test]
fn form_xml_and_module_edit_together() {
    let diff = TreeDiff { changed: vec!["О/Forms/Ф/Ext/Form.xml".into(), "О/Forms/Ф/Ext/Form/Module.bsl".into()], added: vec![], removed: vec![] };
    let keys = BTreeMap::from([
        ("О/Forms/Ф/Ext/Form.xml".to_string(), "f.0".to_string()),
        ("О/Forms/Ф/Ext/Form/Module.bsl".to_string(), "f.0".to_string()),
    ]);
    let plan = classify(&diff, &keys);
    assert!(matches!(&plan.edits[..], [Edit::FormBody { key, form_xml: Some(_), module: Some(_) }] if key == "f.0"));
}

#[test]
fn unmapped_change_is_refused() {
    let diff = TreeDiff { changed: vec!["О/Ext/ObjectModule.bsl".into()], added: vec![], removed: vec![] };
    let plan = classify(&diff, &BTreeMap::new());
    assert_eq!(plan.unsupported, vec!["О/Ext/ObjectModule.bsl"]);
}

#[test]
fn metadata_xml_change_is_unsupported() {
    let diff = TreeDiff { changed: vec!["О.xml".into()], added: vec![], removed: vec![] };
    let keys = BTreeMap::from([("О.xml".to_string(), "main".to_string())]);
    assert_eq!(classify(&diff, &keys).unsupported, vec!["О.xml"]);
}
```

- [ ] **Step 2–4:** RED → реализация → GREEN. **Step 5: Commit** `feat(load): classify tree edits into module and form-body patches`

### Task 3: Наложение на базу (`load_onto_base`)

**Interfaces — Produces:**
```rust
pub struct LoadReport { pub modules: usize, pub forms: usize, pub output: PathBuf }
pub fn load_onto_base(edited: &Path, base: &Path, output: &Path, source_version: InfobaseConfigSourceVersion) -> anyhow::Result<LoadReport>;
```

Шаги реализации:
1. `decode_packed_archive(base)` → `export_if_external` или `export_packed_cf_archive_to_source` во временный каталог (`std::env::temp_dir()/ibcmd-load-<pid>`, удалить в конце) → отчёт.
2. `diff_trees(tmp, edited)` → `classify`; `unsupported` не пуст → `bail!` со списком; `edits` пуст → `bail!("no changes")`.
3. Запросы: `Module` → `SourcePayload::ModuleText { text, info: None }`; `FormBody` → `SourcePayload::NeedsBase { required: key, reason }`; цели — `StoragePatchTarget::new(StorageKey, MultipartIdentity::single(), StorageProvenance::new("cf-load:…"))`; `CompileAxes::new(XmlDialect::parse(ver), None, None, StorageProfileId::parse("storage:cf-cli"), None)`.
4. Кодек `LoadCodec` (как `CliOverlayCodec`): `resolve_needs_base` → `pack_form_body_blob_from_form_xml(base.packed_payload(), xml_or_empty, module)` если есть `form_xml`, иначе `pack_form_body_blob_from_module_text(base.packed_payload(), module)`; `update_versions` → `patch_versions_blob_bytes_allowing_additions(base.packed_payload(), changed_keys, true)`.
5. `decode_archive_uniform(base, limits, profile, provenance, PayloadEncoding::RawDeflate)` → `publish_overlay_new(&archive, &patch, &mut codec, output, limits)`.

- [ ] **Step 1: Интеграционный тест** `tests/cf_load.rs` (фикстура плана 1 `test_processor`):

```rust
mod common;
use std::fs;

#[test]
fn edited_modules_and_form_round_trip_through_export() {
    let dir = common::fixture("test_processor");
    let tree = common::temp_dir("load-tree");
    assert!(common::export(&dir.join("input.epf"), &tree).status.success());
    let module = tree.join("ТестОбработка/Ext/ObjectModule.bsl");
    let text = fs::read_to_string(&module).unwrap().replace("Сумма = 0;", "Сумма = 1;");
    fs::write(&module, &text).unwrap();
    let form_module = tree.join("ТестОбработка/Forms/Форма/Ext/Form/Module.bsl");
    fs::write(&form_module, "\u{feff}&НаКлиенте\r\nПроцедура Тест()\r\nКонецПроцедуры\r\n").unwrap();
    let out = common::temp_dir("load-out").with_extension("epf");
    let run = std::process::Command::new(env!("CARGO_BIN_EXE_ibcmd-rs"))
        .args(["cf", "load"]).arg(&tree).arg(&out)
        .arg("--base").arg(dir.join("input.epf"))
        .output().unwrap();
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    let back = common::temp_dir("load-back");
    assert!(common::export(&out, &back).status.success());
    common::assert_tree_eq(&tree, &back);
}

#[test]
fn load_without_changes_is_refused() { /* export → load same tree → non-zero exit, stderr JSON code "no_changes" */ }

#[test]
fn metadata_change_is_refused_with_the_path() { /* edit ТестОбработка.xml comment → exit != 0, stderr contains "ТестОбработка.xml" */ }
```

(Последние два теста — полный код по образцу первого: `export`, правка/без правки, `cf load`, проверка `!status.success()` и текста stderr.)

- [ ] **Step 2–4:** RED (нет подкоманды) → реализация Task 3 + Task 4 (CLI) → GREEN.
- [ ] **Step 5: Commit** `feat(load): cf load overlays module and form edits onto the base file`

### Task 4: CLI `cf load`

**Files:** Modify `src/cli.rs` (рядом с `CfOverlayArgs`), `src/commands/cf.rs` (`run` dispatch + `fn load`).

```rust
#[derive(Debug, Args)]
pub struct CfLoadArgs {
    /// Edited hierarchical XML source tree (a `cf export` of the base, changed).
    pub source_dir: PathBuf,
    /// New file destination. Existing files are never overwritten.
    pub output: PathBuf,
    /// The .cf/.epf/.erf the tree was exported from.
    #[arg(long)]
    pub base: PathBuf,
    /// Source XML dialect of the tree.
    #[arg(long, value_enum, default_value_t = InfobaseConfigSourceVersion::V2_20)]
    pub source_version: InfobaseConfigSourceVersion,
}
```

Отчёт `CfCommandReport::Load(CfLoadReport { schema_version, command: "load", ok, source_dir, output, base, modules, forms, errors })`; ошибки: `no_changes`, `unsupported_changes` (сообщение со списком путей), `load_failed`.

- [ ] Тесты — Task 3 Step 1. **Commit** вместе с Task 3.

### Task 5: Приёмка платформой (скрипт, не в CI)

`_onecdec/verify_load.py <corpus>`: для каждого `ext-bin/*` — `cf export` → правка первого `*.bsl` (добавить комментарий в конец) → `cf load` → выгрузка результата платформой 8.3.27.2214 в базе с конфигурацией → сравнение с деревом пользователя (все файлы, кроме изменённого модуля, должны совпасть с эталоном `ext-cfg`, изменённый — с правкой). Отчёт: сколько объектов прошло.

- [ ] Написать, прогнать, результат записать в spec. **Commit** `test(load): platform acceptance script`

## Self-review

- Spec «Загрузка v1» пп.1–5 → Task 3 (1,2,4), Task 2 (3,5), Task 5 (приёмка), `.cfe` → ошибка overlay (Global Constraints).
- Review Focus 1–5 → тесты Task 3/Task 2; кириллица — все фикстуры.
- Имена: `TreeDiff`, `diff_trees`, `keys_by_output`, `Edit`, `Plan`, `classify`, `load_onto_base`, `CfLoadArgs` — согласованы.
