//! EDT-проекция вида `Constant` (зеркало `core/spec/metadata/constant.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек — канонику держит `core/spec`.
//!
//! Карта `FieldId → (XmlLocus, Codec)` ВЫВОДИТСЯ из канонического спека
//! (`docs/APPROACH.md` §2.2): плоские дети корня `<mdclass:Constant>`, тег = имя поля
//! verbatim, кодек = по value_kind (`localized→LocalizedKeyVal`, `bool→BoolPresence`,
//! `enum→EnumText`, `str→PlainText`, `type→Type(Edt)`, `value→Value(Edt)`). EDT эмитит
//! РАЗРЕЖЁННО (дефолты опущены) — это решает движок+спек, не карта.
//!
//! Платформенный `<producedTypes>` (Manager/ValueManager/ValueKey) — НЕ в этой карте:
//! лежит структурно ДО `<name>`, фреймится каркасом коннектора в
//! `MetadataObject.internal_info` (§3.5). Корень несёт `xmlns:xsi`+`xmlns:core` (нужны
//! Value-кодеку: `xsi:type="core:UndefinedValue"`) — переопределение
//! `root_extra_namespaces` ниже (вне деривации проекции).

use formats_xml::derive::{projection, DeriveDialect};
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::constant as c;

/// Карта проекции EDT для `Constant` — ВЫВОДИТСЯ из спека (`formats_xml::derive`),
/// заменяет рукописную таблицу `FieldId → (тег, кодек)`. Корневые доп. ns — ниже.
pub struct EdtConstant;

impl LocusMap for EdtConstant {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        // Структурные List-поля не деривятся по value_kind (§2.2) — ручные ветки.
        // ERP несёт их на КОРНЕ Constant (witnessed ВидЦеныПлановойСтоимостиМатериаловРабот).
        match field {
            c::F_CHOICE_PARAMETER_LINKS => Some(FieldProjection {
                locus: XmlLocus::PropElement { path: &["choiceParameterLinks"], ns: "" },
                codec: Codec::ChoiceParameterLinks(
                    formats_xml::choice_param_links::LinksDialect::Edt,
                ),
            }),
            c::F_CHOICE_PARAMETERS => Some(FieldProjection {
                locus: XmlLocus::PropElement { path: &["choiceParameters"], ns: "" },
                codec: Codec::ChoiceParameters(
                    formats_xml::choice_parameters::ChoiceParametersDialect::Edt,
                ),
            }),
            _ => projection(DeriveDialect::Edt, constant(), field),
        }
    }

    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        // EDT физический порядок совпадает с DENSE-порядком спека ВЕЗДЕ, кроме `dataHistory`:
        // в EDT `.mdo` он стоит сразу ПОСЛЕ `comment` (перед `type`), а не в конце (сверено
        // фикстурой Конст_ИсторияДанных_Использовать). Companions
        // `updateDataHistoryImmediatelyAfterWrite`/`executeAfterWriteDataHistoryVersionProcessing`
        // остаются В КОНЦЕ, как в спеке (сверено Конст_Обновлять…/Конст_Выполнять…). Поля,
        // не перечисленные здесь, сохраняют спек-порядок (стабильный хвост `XmlSink::ordered`),
        // поэтому достаточно закрепить голову: synonym, comment, dataHistory.
        Some(EDT_ORDER)
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // EDT Constant-корень несёт xmlns:xsi+xmlns:core (нужны Value-кодеку:
        // `<minValue xsi:type="core:UndefinedValue"/>`). Порядок = эталона (xsi,
        // затем core; оба ДО xmlns:mdclass).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

/// EDT-голова порядка эмиссии Constant: закрепляет `dataHistory` сразу после `comment`;
/// прочие поля идут спек-порядком (стабильный хвост). См. [`EdtConstant::field_emit_order`].
static EDT_ORDER: &[FieldId] = &[c::F_SYNONYM, c::F_COMMENT, c::F_DATA_HISTORY];

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::constant::constant;

/// R-read для харнесса: `.mdo`-байты Constant → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Constant", constant(), &EdtConstant, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Constant → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Constant", constant(), &EdtConstant, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Constant. Имя `HARNESS_ENTRY` — контракт `build.rs`.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Constant",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/Constants",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
