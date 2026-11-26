//! Designer-проекции видов метаданных (зеркало `core/spec/metadata`, ARCHITECTURE.md §5).
//! Модуль-на-вид: каждый `<вид>.rs` несёт [`formats_xml::LocusMap`]-проекцию и
//! `pub const HARNESS_ENTRY: formats_xml::FormatKind` (строку R+X-харнесса) — добавление
//! вида не правит общих файлов.
//!
//! Список `pub mod …` и массив [`FORMAT_KINDS`] ГЕНЕРИРУЮТСЯ `build.rs` из файлов
//! каталога (parallel-safety, §5; ORCHESTRATION). `FORMAT_KINDS` — ссылаемый `const`,
//! держащий модули видов живыми (DCE-safe; см. `build.rs` «почему codegen, а не inventory»).

// Сгенерировано build.rs: `#[path] pub mod <вид>;` для каждого `metadata/*.rs`.
include!(concat!(env!("OUT_DIR"), "/metadata_mods.rs"));
// Сгенерировано build.rs: `pub static FORMAT_KINDS: &[FormatKind]`.
include!(concat!(env!("OUT_DIR"), "/metadata_kinds.rs"));
