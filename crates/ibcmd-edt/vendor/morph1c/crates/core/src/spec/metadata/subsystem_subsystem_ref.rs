//! Канонический спек ДОЧЕРНЕГО вида `Subsystem.SubsystemRef` — self-referential
//! childobjects-ref коллекция подсистемы (child-objects substrate §3.2). Подсистема
//! содержит список ВЛОЖЕННЫХ подсистем, но в дескрипторе родителя это лишь ИМЯ-ссылка
//! (членство+порядок), а тело вложенной подсистемы — в отдельном файле (SSL раскладка
//! `Subsystems/<Parent>/Subsystems/<Child>/<Child>.mdo`). Лист-вид, БЕЗ полей.
//!
//! BARE-REF на ОБОИХ форматах (в отличие от Catalog.FormRef, где EDT несёт inline-стаб):
//! * EDT: сиблинг `<subsystems>Имя</subsystems>` под корнем `<mdclass:Subsystem>`;
//! * Designer: `<ChildObjects><Subsystem>Имя</Subsystem>…` под `<Subsystem>`.
//!
//! Оба читаются каркасом как `uuid=zero`, `properties=[]` (см. `formats_xml::children`
//! ветка `bare_ref`) → РАВНЫЙ IR (имя+порядок) → X by construction без спец-исключения.
//!
//! `name` — единственная информация ссылки (идентичность объекта в
//! [`crate::ir::MetadataObject`]), round-trip-ится каркасом; полей-`FieldSpec` нет.

use crate::spec::common::EntitySpec;

/// `&'static EntitySpec` вида `Subsystem.SubsystemRef` (кэш на процесс). Лист-вид без
/// полей и без собственных детей: элемент коллекции — лишь имя-ссылка.
pub fn subsystem_subsystem_ref() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Subsystem.SubsystemRef",
        fields: &[],
        children: &[],
    })
}
