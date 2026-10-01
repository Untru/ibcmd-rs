//! Канонический спек вида объекта `Subsystem` (подсистема) — ARCHITECTURE.md §1.4/§1.6,
//! срез S1/S9 (picture-REF + childobjects-ref self-reference). ОДИН [`EntitySpec`] на вид:
//! движок ([`crate::engine`]) выводит read/write для КАЖДОГО XML-формата (EDT `.mdo`,
//! Designer `.xml`); формат хранит лишь ПРОЕКЦИЮ; здесь — канонический `id`, `value_kind`,
//! `default`, нормализация и ПОРЯДОК эмиссии (§1.6).
//!
//! ВНЕ спека (намеренно): `name`/`uuid` — идентичность объекта в
//! [`crate::ir::MetadataObject`], round-trip-ится каркасом коннектора.
//!
//! Поля (подтверждено корпусом SSL, 3 top-level объекта edt+designer; Designer DENSE — все
//! present всегда, EDT SPARSE — дефолты опущены):
//! * `Synonym` — локализованный синоним;
//! * `Comment` — свободный текст (default `""`);
//! * `IncludeHelpInContents` — bool включения справки в содержание (default `false`);
//! * `IncludeInCommandInterface` — bool включения в командный интерфейс (default `false`);
//! * `UseOneCommand` — bool «одна команда» (default `false`; в SSL всегда `false`);
//! * `Explanation` — локализованное пояснение (default пустой список);
//! * `Picture` — ссылка на общую/стандартную картинку (`PictureRef`; default `""`);
//! * `Content` — СОСТАВ подсистемы: список ссылок-объектов (`MdObject`). EDT — сиблинги
//!   `<content>Path</content>`; Designer — `<Content><xr:Item xsi:type="xr:MDObjectRef">
//!   Path</xr:Item>…`. Канонический IR — `List([Str(path)])`; пустой → `[]` (дефолт).
//!
//! Порядок = DENSE-порядок Designer `<Properties>`: Synonym, Comment, IncludeHelpInContents,
//! IncludeInCommandInterface, UseOneCommand, Explanation, Picture, Content (EDT эмитит то
//! же, опуская дефолтные comment/useOneCommand). Metamodel-only поля БЕЗ корпус-witness
//! (ObjectBelonging/ExtendedConfigurationObject/Help, occ=0 в SSL) намеренно НЕ включены
//! (§1.0: не спекулируем без оракула).
//!
//! # Дочерняя коллекция (childobjects-ref, self-reference)
//! `Subsystem.SubsystemRef` — ВЛОЖЕННЫЕ подсистемы (лишь ИМЯ-ссылки, bare-ref на обоих
//! форматах; тело — в отдельном файле). См. `subsystem_subsystem_ref`.

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{ChildSlot, EntitySpec, FieldSpec, Normalize, picture_ref_field};

/// `synonym` — локализованный синоним. Default = пустой список; сорт по коду языка.
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `includeHelpInContents` — включить справку в содержание (bool). Default = `false`.
pub const F_INCLUDE_HELP_IN_CONTENTS: FieldId = FieldId(3);
/// `includeInCommandInterface` — включить в командный интерфейс (bool). Default = `false`.
pub const F_INCLUDE_IN_COMMAND_INTERFACE: FieldId = FieldId(4);
/// `useOneCommand` — использовать одну команду (bool). Default = `false`.
pub const F_USE_ONE_COMMAND: FieldId = FieldId(5);
/// `explanation` — локализованное пояснение. Default = пустой список; сорт по коду языка.
pub const F_EXPLANATION: FieldId = FieldId(6);
/// `picture` — ссылка на картинку (`PictureRef`). Default = `""` (пусто).
pub const F_PICTURE: FieldId = FieldId(7);
/// `content` — состав подсистемы (СПИСОК ссылок-объектов). Default = пустой список.
pub const F_CONTENT: FieldId = FieldId(8);
/// `help` — EDT-only const-блок `<help><pages><lang>ru</lang></pages></help>` (presence).
/// Метамодель: `serialized:true` для EDT-`.mdo`, но Designer несёт справку в спутнике
/// `Ext/Help.xml`, а НЕ в дескрипторе → в per-file X это EDT-локальная данность.
/// Default = `false`; X-исключён (`x_ignore`); участвует в R EDT (byte-exact). Зеркало
/// EDT-only `<help>` у Catalog/InformationRegister (тот же `HelpConst`-кодек). Появляется
/// у вложенных подсистем (24 в SSL); top-level 3 его не несут.
pub const F_HELP: FieldId = FieldId(9);
/// `parentSubsystem` — EDT-only обратная ссылка на РОДИТЕЛЬСКУЮ подсистему
/// (`<parentSubsystem>Subsystem.Имя</parentSubsystem>`). Метамодель: `serialized:false,
/// is_md_property:false` — ДЕРИВИРУЕМАЯ (инверсия `subsystems`-containment родителя), НЕ
/// каноническое метаданное. EDT материализует её в per-file `.mdo`; Designer кодирует
/// родителя СТРУКТУРНО (позиция файла под `<parent>/Subsystems/`) и в дескрипторе не несёт.
/// Поэтому: участвует в R EDT (byte-exact — иначе 84 вложенных `.mdo` не свернутся), но
/// X-исключён (`x_ignore`) — как EDT-only `<help>`. Физически ПОСЛЕДНИЙ элемент (после
/// `content` И после вложенных `<subsystems>`), поэтому EDT-проекция помечает его
/// `trailing_fields` (эмиссия ПОСЛЕ inline-детей). Default = `""`.
pub const F_PARENT_SUBSYSTEM: FieldId = FieldId(10);

/// Сконструировать [`FieldSpec`] вида `Subsystem` в каноническом порядке.
fn build_fields() -> Vec<FieldSpec> {
    vec![
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        FieldSpec::with_default(F_COMMENT, "comment", ValueKind::Str, PropertyValue::Str(String::new())),
        FieldSpec::with_default(
            F_INCLUDE_HELP_IN_CONTENTS,
            "includeHelpInContents",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        // help — EDT-only presence-блок (после includeHelpInContents, метамодель-порядок).
        // Default false; X-исключён (Designer несёт справку в спутнике, не в дескрипторе).
        FieldSpec::with_default(F_HELP, "help", ValueKind::Bool, PropertyValue::Bool(false))
            .x_ignored(),
        FieldSpec::with_default(
            F_INCLUDE_IN_COMMAND_INTERFACE,
            "includeInCommandInterface",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_USE_ONE_COMMAND,
            "useOneCommand",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_EXPLANATION,
            "explanation",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // Picture — ссылка на картинку; кодек PictureRef. Default = "" (пусто).
        picture_ref_field(F_PICTURE),
        // Content — список ссылок-объектов; default = пустой список.
        FieldSpec::with_default(F_CONTENT, "content", ValueKind::List, PropertyValue::List(Vec::new())),
        // parentSubsystem — EDT-only деривируемая обратная ссылка (метамодель serialized:false).
        // ПОСЛЕДНЯЯ в каноническом порядке (физически после content И вложенных <subsystems>;
        // EDT-проекция помечает `trailing_fields`). Default ""; X-исключён (Designer не несёт).
        FieldSpec::with_default(
            F_PARENT_SUBSYSTEM,
            "parentSubsystem",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        )
        .x_ignored(),
    ]
}

/// Слоты дочерних коллекций `Subsystem`: вложенные подсистемы (self-reference, bare-ref).
static CHILDREN: &[ChildSlot] =
    &[ChildSlot { collection: "Subsystem", child_kind: "Subsystem.SubsystemRef" }];

/// Канонический [`EntitySpec`] вида `Subsystem` (кэш на процесс).
pub fn subsystem() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "Subsystem",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: CHILDREN,
    })
}
