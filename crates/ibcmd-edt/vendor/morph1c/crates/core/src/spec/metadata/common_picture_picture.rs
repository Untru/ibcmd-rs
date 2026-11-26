//! Каноническая child-структура КАРТИНКИ вида `CommonPicture` (metamodel `picture:
//! PictureDef`) — ARCHITECTURE.md §1.4/§1.6, срез S1.
//!
//! # Что это моделирует
//! Метамодель объявляет у `CommonPicture` child-поле `picture` типа `PictureDef`. В
//! ВИТНЕСС-корпусе (SSL, 600 объектов) эта картинка материализуется как БИНАРНЫЙ
//! файл-СПУТНИК `Picture.<ext>` рядом с дескриптором (Data-вариант; вариант «встроенный
//! Ref внутри `.mdo`» в корпусе НЕ встречен). Формат-нейтральный КАНОН картинки в IR
//! (§1.0 Blob): байты переносятся as-is через [`crate::ir::value::PropertyValue::Blob`]
//! ([`crate::ir::value::BlobRef`] = `{key, len, digest}`), где `key` = имя файла-спутника
//! (`Picture.png`/`Picture.svg`/`Picture.zip`/…). Оба формата читают ТЕ ЖЕ байты → тот же
//! `BlobRef` → X by construction (edt==designer, §1.6/§3.5).
//!
//! # Почему это child-спек, но НЕ движок-walked ChildSlot
//! Картинка — не inline-XML в дескрипторе (EDT `.mdo` её вовсе не упоминает; Designer
//! держит в ОТДЕЛЬНОМ sidecar `Ext/Picture.xml`+`Ext/Picture/Picture.<ext>`), а бинарный
//! спутник, фреймящийся КАРКАСОМ коннектора (как `Module.bsl`/`producedTypes`), а не
//! спек-движком. Поэтому этот `EntitySpec` НЕ подключён в `CommonPicture.children` как
//! `ChildSlot` — он ДОКУМЕНТИРУЕТ каноническую форму picture-поля (единый источник §1.6
//! для будущих picture-несущих видов: Bot/Subsystem/CommandGroup), а материализацию
//! Blob'а ведёт sidecar-aware путь (`formats-xml::picture_sidecar` + testkit).
//!
//! Файл присутствует в `spec/metadata`, поэтому обязан вернуть `EntitySpec` (контракт
//! `build.rs`); `entity = "CommonPicture.Picture"` (dotted child-код, зеркало
//! `Catalog.Command` и пр.).

use crate::ir::value::ValueKind;
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};

/// `data` — байты картинки как опаковый [`crate::ir::value::PropertyValue::Blob`] (§1.0).
/// Всегда present (у объекта есть файл-спутник); дефолта нет. `key` Blob'а = имя
/// файла-спутника (`Picture.<ext>`), `len`/`digest` — из байтов (cross-format равенство
/// без материализации потока).
pub const F_DATA: FieldId = FieldId(1);

/// Сконструировать [`FieldSpec`]'ы child-структуры картинки.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        // data: Blob, всегда present (нет дефолта — объект картинки без байтов не бывает).
        FieldSpec::required(F_DATA, "data", ValueKind::Blob),
    ]
}

/// Канонический [`EntitySpec`] child-структуры `CommonPicture.Picture` (кэш на процесс).
pub fn common_picture_picture() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonPicture.Picture",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
