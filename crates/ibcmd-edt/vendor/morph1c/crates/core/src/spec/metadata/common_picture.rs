//! Канонический спек вида объекта `CommonPicture` (общая картинка) — ARCHITECTURE.md
//! §1.4/§1.6, срез S1 (picture/Blob binary-sidecar).
//!
//! ОДИН [`EntitySpec`] на вид: движок ([`crate::engine`]) выводит read/write для КАЖДОГО
//! XML-формата (EDT `.mdo`, Designer `.xml`) из него — формат хранит лишь ПРОЕКЦИЮ (имя
//! тега/ns) в зеркальном модуле. Здесь — канонический `id`, `value_kind`, `default` и
//! ПОРЯДОК эмиссии (§1.6).
//!
//! # Что здесь (и чего НЕТ)
//! Дескриптор `.mdo`/`.xml` несёт только ТОНКИЕ поля объекта (synonym/comment +
//! transparentPixel + два availability-флага). Сама КАРТИНКА — БИНАРНЫЙ файл-СПУТНИК
//! `Picture.<ext>` рядом с
//! дескриптором (§1.0 Blob: переносится as-is, без интерпретации PNG/SVG/ZIP-внутренностей),
//! НЕ спек-свойство, разбираемое движком: спутник фреймится каркасом коннектора как Blob
//! (см. `formats-xml::picture_sidecar` + testkit sidecar-aware R/X), аналогично тому как
//! `Module.bsl`/`producedTypes` фреймятся вне спек-региона. Поэтому в `fields()` его нет.
//!
//! `name`/`uuid` — идентичность объекта ([`crate::ir::MetadataObject`]), round-trip-ится
//! каркасом, а не [`FieldSpec`] (как у всех видов).
//!
//! # `transparentPixel` — координата прозрачного пикселя (ERP-witnessed)
//! РЕАЛЬНОЕ задаваемое свойство (metamodel: `Point`), а НЕ константа: 30/2458
//! ERP-CommonPicture его несут. Носители расходятся ПО ДИАЛЕКТАМ:
//! * **EDT** — узел ДЕСКРИПТОРА `<transparentPixel><x>10</x><y>7</y></transparentPixel>`
//!   (SPARSE: нулевая координата опускается — witnessed `ВажностьНовостиОченьВажная`
//!   несёт только `<x>14</x>`, её designer-зеркало — `x="14" y="0"`);
//! * **Designer** — НЕ в дескрипторе, а в ОБЁРТКЕ сайдкара `Ext/Picture.xml`:
//!   `<xr:LoadTransparent>true</…>` + `<xr:TransparentPixel x="10" y="7"/>` (DENSE-атрибуты;
//!   `LoadTransparent=true` ⟺ пиксель есть — 30/30 и 2428/2428 ERP). Читает/пишет
//!   `pipeline::picture_read` В ЭТО ЖЕ спек-поле (зеркало `predefined`-паттерна);
//! * **cf** — envelope тела `<uuid>.0`: `{1,0,x,y}` при пикселе / `{0,0,-1,-1}` без
//!   (RE erp.cf: `ВажностиНовостей` `{1,0,10,7}`, `ВажностьНовостиОченьВажная` `{1,0,14,0}`;
//!   s4_common 4/4 `{0,0,-1,-1}`) — см. `formats_cf::picture_body`.
//!
//! Канонический IR — `List([Int(x), Int(y)])`; default = пустой `List` («пикселя нет»).
//! `x_ignore` — как у `predefined`: Designer-ДЕСКРИПТОР поля не несёт (сайдкар читается
//! отдельным pipeline-проходом), per-kind X по дескрипторам его сравнить не может; оба
//! диалекта дают РАВНЫЙ IR на whole-config пути.
//!
//! # Порядок полей (= порядок эмиссии Designer `<Properties>`, сверено корпусом SSL)
//! `Synonym` → `Comment` → `TransparentPixel`(только EDT) → `AvailabilityForChoice` →
//! `AvailabilityForAppearance` (позиция transparentPixel = metamodel: `comment → … →
//! transparentPixel → availabilityForChoice`; Designer-дескриптор его не эмитит вовсе).
//! Канонический IR РАЗРЕЖЕН: значения == `default` в bag НЕ хранятся (§1.1). EDT-`.mdo`
//! эмитит лишь non-default (в SSL: synonym у 600, comment у 5); Designer DENSE эмитит и
//! дефолты (`<Comment/>`, `<AvailabilityForChoice>false</…>`), но движок сводит их к тому
//! же РАЗРЕЖЕННОМУ IR → edt==designer (X by construction, §1.6/§3.5).

use crate::ir::value::{PropertyValue, ValueKind};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec, Normalize};

/// `synonym` — локализованный синоним. Default = пустой список; маркер сортировки по
/// коду языка для X (порядок языков хранится из источника — byte-exact R, §1.6).
pub const F_SYNONYM: FieldId = FieldId(1);
/// `comment` — комментарий (свободный текст). Default = `""`.
pub const F_COMMENT: FieldId = FieldId(2);
/// `availabilityForChoice` — доступность для выбора (bool). Default = `false`.
pub const F_AVAILABILITY_FOR_CHOICE: FieldId = FieldId(3);
/// `availabilityForAppearance` — доступность для оформления (bool). Default = `false`.
pub const F_AVAILABILITY_FOR_APPEARANCE: FieldId = FieldId(4);
/// `transparentPixel` — координата прозрачного пикселя картинки (metamodel `Point`).
/// IR — `List([Int(x), Int(y)])`; default = пустой `List` («пикселя нет»). ERP-witnessed
/// 30/2458 (см. модульный doc-comment: EDT-узел дескриптора / Designer `Ext/Picture.xml`
/// `LoadTransparent=true`+`TransparentPixel` / cf-envelope `{1,0,x,y}`).
pub const F_TRANSPARENT_PIXEL: FieldId = FieldId(5);

/// Сконструировать [`FieldSpec`]'ы вида `CommonPicture` в каноническом порядке.
///
/// Не `static`: `default`-значения ([`PropertyValue`]) владеют `String`/`Vec` (не
/// `const`-конструируемы). Кэшируется на первый вызов ([`common_picture`]).
fn build_fields() -> Vec<FieldSpec> {
    vec![
        // synonym: Localized, default пустой список, маркер сорт-по-lang для X.
        FieldSpec::with_default(
            F_SYNONYM,
            "synonym",
            ValueKind::Localized,
            PropertyValue::Localized(Vec::new()),
        )
        .normalized(Normalize::LocalizedSortByLang),
        // comment: Str, default "".
        FieldSpec::with_default(
            F_COMMENT,
            "comment",
            ValueKind::Str,
            PropertyValue::Str(String::new()),
        ),
        // transparentPixel: List([Int(x),Int(y)]), default [] («пикселя нет»). Позиция =
        // metamodel (comment → transparentPixel → availabilityForChoice). ValueKind::List
        // намеренно: derive-проекции List не деривируют → Designer-дескриптор поле НЕ
        // проецирует (носитель — сайдкар-обёртка, pipeline::picture_read), EDT даёт
        // явный Codec::TransparentPixel. x_ignore — зеркало `predefined` (сайдкар-носитель
        // невидим per-kind-дескрипторному X; whole-config IR обоих диалектов равен).
        FieldSpec::with_default(
            F_TRANSPARENT_PIXEL,
            "transparentPixel",
            ValueKind::List,
            PropertyValue::List(Vec::new()),
        )
        .x_ignored(),
        // availabilityForChoice / availabilityForAppearance: Bool, default false.
        FieldSpec::with_default(
            F_AVAILABILITY_FOR_CHOICE,
            "availabilityForChoice",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
        FieldSpec::with_default(
            F_AVAILABILITY_FOR_APPEARANCE,
            "availabilityForAppearance",
            ValueKind::Bool,
            PropertyValue::Bool(false),
        ),
    ]
}

/// Канонический [`EntitySpec`] вида `CommonPicture` (кэш на процесс).
///
/// 5 тонких свойств дескриптора в каноническом порядке эмиссии (`name`/`uuid` —
/// идентичность объекта, тут НЕ перечислены; картинка — Blob-спутник, вне спека;
/// `transparentPixel` в Designer живёт в обёртке спутника, не в дескрипторе).
pub fn common_picture() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "CommonPicture",
        // Утечка единожды на процесс → владеемый Vec в `&'static [_]` без unsafe.
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[], // картинка — Blob-спутник (каркас), не child-коллекция.
    })
}

// Реестр (`crate::spec::registry`) собирается build.rs'ом, который ИМЕНУЕТ
// `common_picture::common_picture` — отдельной саморегистрации не нужно. Канонический
// код вида берётся из `EntitySpec::entity` (== "CommonPicture").
