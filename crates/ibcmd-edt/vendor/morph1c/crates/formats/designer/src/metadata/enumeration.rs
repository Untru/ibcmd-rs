//! Designer-проекция вида `Enum` + его дочернего `Enum.EnumValue` (зеркало
//! `core/spec/metadata/{enumeration,enum_enum_value}.rs`, ARCHITECTURE.md §5;
//! child-objects substrate). ТОЛЬКО размещение/кодировка
//! ячеек — канонику держит `core/spec` (§1.6). Иной синтаксис ТОГО ЖЕ спека, что EDT →
//! оба формата дают РАВНЫЙ IR (§3.5), включая `children` рекурсивно.
//!
//! РОДИТЕЛЬ `Enum` (пути от корня `<MetaDataObject>` через `<Enum>/<Properties>`, DENSE):
//! * `synonym` → `<Synonym>` (`LocalizedV8`); `comment` → `<Comment>` (`PlainText`);
//! * `useStandardCommands`/`quickChoice` → `<Flag>true|false</Flag>` (`BoolText`);
//! * `standardAttributes` → const-блок (`EnumStandardAttributes(Designer)` — один
//!   `<StandardAttributes>` с `<xr:StandardAttribute>` Order+Ref);
//! * Designer-only пустые: `characteristics`/`default*Form` → `<Tag/>` (`PlainText`,
//!   дефолт `""` → self-closing); `*Presentation`/`explanation` → `<Tag/>`
//!   (`LocalizedV8`, дефолт `[]` → пустой self-closing; 1 explanation непуст);
//! * `choiceMode`/`choiceHistoryOnInput` → enum-литерал (`EnumText`).
//!
//! ДОЧЕРНЯЯ коллекция `EnumValue` → `<ChildObjects><EnumValue uuid><Properties>…`;
//! рекурсию ведёт `formats_xml::children`.
//!
//! РЕБЁНОК `Enum.EnumValue` ([`DesignerEnumValue`]) — БЕЗ `HARNESS_ENTRY` (покрыт
//! транзитивно): локусы относительны `<Properties>` ребёнка: `synonym`→`<Synonym>`,
//! `comment`→`<Comment>`, `color`→`<Color>` (`EnumText`, дефолт `auto`).

use formats_xml::children::ChildBinding;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, StdAttrsDialect, XmlLocus};
use formats_xml::std_attrs_ir;
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::enum_command as cmd;
use morph1c_core::spec::metadata::enum_command::enum_command;
use morph1c_core::spec::metadata::enum_enum_value::{
    enum_enum_value, F_COLOR as EV_COLOR, F_COMMENT as EV_COMMENT, F_SYNONYM as EV_SYNONYM,
};
use morph1c_core::spec::metadata::enum_form_ref::enum_form_ref;
use morph1c_core::spec::metadata::enum_template_ref::enum_template_ref;
use morph1c_core::spec::metadata::enumeration::{
    F_AUXILIARY_CHOICE_FORM, F_AUXILIARY_LIST_FORM, F_CHARACTERISTICS, F_CHOICE_HISTORY_ON_INPUT,
    F_CHOICE_MODE, F_COMMENT, F_DEFAULT_CHOICE_FORM, F_DEFAULT_LIST_FORM, F_EXPLANATION,
    F_EXTENDED_LIST_PRESENTATION, F_LIST_PRESENTATION, F_QUICK_CHOICE, F_STANDARD_ATTRIBUTES,
    F_SYNONYM, F_USE_STANDARD_COMMANDS,
};

/// Плоский `PropElement{path:[tag], ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

// --- Родитель Enum: пути от корня через <Enum>/<Properties> ---
const P_SYNONYM: &[&str] = &["Enum", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["Enum", "Properties", "Comment"];
const P_USE_STANDARD_COMMANDS: &[&str] = &["Enum", "Properties", "UseStandardCommands"];
const P_STANDARD_ATTRIBUTES: &[&str] = &["Enum", "Properties", "StandardAttributes"];
const P_CHARACTERISTICS: &[&str] = &["Enum", "Properties", "Characteristics"];
const P_QUICK_CHOICE: &[&str] = &["Enum", "Properties", "QuickChoice"];
const P_CHOICE_MODE: &[&str] = &["Enum", "Properties", "ChoiceMode"];
const P_DEFAULT_LIST_FORM: &[&str] = &["Enum", "Properties", "DefaultListForm"];
const P_DEFAULT_CHOICE_FORM: &[&str] = &["Enum", "Properties", "DefaultChoiceForm"];
const P_AUXILIARY_LIST_FORM: &[&str] = &["Enum", "Properties", "AuxiliaryListForm"];
const P_AUXILIARY_CHOICE_FORM: &[&str] = &["Enum", "Properties", "AuxiliaryChoiceForm"];
const P_LIST_PRESENTATION: &[&str] = &["Enum", "Properties", "ListPresentation"];
const P_EXTENDED_LIST_PRESENTATION: &[&str] = &["Enum", "Properties", "ExtendedListPresentation"];
const P_EXPLANATION: &[&str] = &["Enum", "Properties", "Explanation"];
const P_CHOICE_HISTORY_ON_INPUT: &[&str] = &["Enum", "Properties", "ChoiceHistoryOnInput"];

/// Карта проекции Designer для родителя `Enum`.
pub struct DesignerEnum;

impl DesignerEnum {
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_USE_STANDARD_COMMANDS {
            FieldProjection::new(elem(P_USE_STANDARD_COMMANDS, ""), Codec::BoolText)
        } else if field == F_STANDARD_ATTRIBUTES {
            FieldProjection::new(
                elem(P_STANDARD_ATTRIBUTES, ""),
                Codec::IrStandardAttributes(StdAttrsDialect::Designer, &std_attrs_ir::ENUM),
            )
        } else if field == F_CHARACTERISTICS {
            FieldProjection::new(elem(P_CHARACTERISTICS, ""), Codec::PlainText)
        } else if field == F_QUICK_CHOICE {
            FieldProjection::new(elem(P_QUICK_CHOICE, ""), Codec::BoolText)
        } else if field == F_CHOICE_MODE {
            FieldProjection::new(elem(P_CHOICE_MODE, ""), Codec::EnumText)
        } else if field == F_DEFAULT_LIST_FORM {
            FieldProjection::new(elem(P_DEFAULT_LIST_FORM, ""), Codec::PlainText)
        } else if field == F_DEFAULT_CHOICE_FORM {
            FieldProjection::new(elem(P_DEFAULT_CHOICE_FORM, ""), Codec::PlainText)
        } else if field == F_AUXILIARY_LIST_FORM {
            FieldProjection::new(elem(P_AUXILIARY_LIST_FORM, ""), Codec::PlainText)
        } else if field == F_AUXILIARY_CHOICE_FORM {
            FieldProjection::new(elem(P_AUXILIARY_CHOICE_FORM, ""), Codec::PlainText)
        } else if field == F_LIST_PRESENTATION {
            FieldProjection::new(elem(P_LIST_PRESENTATION, ""), Codec::LocalizedV8)
        } else if field == F_EXTENDED_LIST_PRESENTATION {
            FieldProjection::new(elem(P_EXTENDED_LIST_PRESENTATION, ""), Codec::LocalizedV8)
        } else if field == F_EXPLANATION {
            FieldProjection::new(elem(P_EXPLANATION, ""), Codec::LocalizedV8)
        } else if field == F_CHOICE_HISTORY_ON_INPUT {
            FieldProjection::new(elem(P_CHOICE_HISTORY_ON_INPUT, ""), Codec::EnumText)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerEnum {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        // Designer: `<Enum><ChildObjects>…`. EnumValue/Command — полные sub-object'ы
        // (`<X uuid><Properties><Name>…`), Form/Template — BARE-ссылки (`<Form>Имя</Form>`;
        // тело — отдельный файл).
        let cont: &'static [&'static str] = &["Enum", "ChildObjects"];
        let (tag, bare) = match collection {
            "EnumValue" => ("EnumValue", false),
            "Form" => ("Form", true),
            "Template" => ("Template", true),
            "Command" => ("Command", false),
            _ => return None,
        };
        Some(ChildLocus {
            container: cont,
            child_tag: tag,
            props_wrapped: !bare,
            name_tag: "Name",
            bare_ref: bare,
        })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        enum_value_bindings()
    }

    fn emit_empty_child_container(&self) -> bool {
        // ВСЕ Designer-Enum несут `<ChildObjects>` — при нуле EnumValue пустой
        // самозакрытый `<ChildObjects/>` после `</Properties>` (сверено корпусом:
        // `Пер_БыстрыйВыбор_Истина` и др. без значений). Только Designer (EDT не
        // оборачивает); при ≥1 значении обёртка несёт детей (пустой путь не берётся).
        true
    }
}

// --- Ребёнок Enum.EnumValue (БЕЗ HARNESS_ENTRY — транзитивное покрытие) ---

// Локусы относительны элемента `<Properties>` ребёнка (props_wrapped=true): движок
// получает `<Properties>` как `source.root`, поэтому путь — одиночный тег.
const EV_P_SYNONYM: &[&str] = &["Synonym"];
const EV_P_COMMENT: &[&str] = &["Comment"];
const EV_P_COLOR: &[&str] = &["Color"];

/// Карта проекции Designer для child-вида `Enum.EnumValue`.
pub struct DesignerEnumValue;

impl LocusMap for DesignerEnumValue {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == EV_SYNONYM {
            Some(FieldProjection::new(elem(EV_P_SYNONYM, ""), Codec::LocalizedV8))
        } else if field == EV_COMMENT {
            Some(FieldProjection::new(elem(EV_P_COMMENT, ""), Codec::PlainText))
        } else if field == EV_COLOR {
            Some(FieldProjection::new(elem(EV_P_COLOR, ""), Codec::EnumText))
        } else {
            None
        }
    }
}

// ===== Дитя Command (полный sub-object, зеркало InformationRegister.Command) =====
pub struct DesignerCommand;
impl LocusMap for DesignerCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cmd::F_SYNONYM => FieldProjection::new(elem(&["Synonym"], ""), Codec::LocalizedV8),
            cmd::F_COMMENT => FieldProjection::new(elem(&["Comment"], ""), Codec::PlainText),
            cmd::F_GROUP => FieldProjection::new(elem(&["Group"], ""), Codec::EnumText),
            cmd::F_COMMAND_PARAMETER_TYPE => FieldProjection::new(
                elem(&["CommandParameterType"], ""),
                Codec::Type(formats_xml::TypeDialect::Designer),
            ),
            cmd::F_PARAMETER_USE_MODE => {
                FieldProjection::new(elem(&["ParameterUseMode"], ""), Codec::EnumText)
            }
            cmd::F_MODIFIES_DATA => FieldProjection::new(elem(&["ModifiesData"], ""), Codec::BoolText),
            cmd::F_REPRESENTATION => {
                FieldProjection::new(elem(&["Representation"], ""), Codec::EnumText)
            }
            cmd::F_TOOL_TIP => FieldProjection::new(elem(&["ToolTip"], ""), Codec::LocalizedV8),
            // picture — спек `picture_ref_field` ⇒ IR `List([Str(ref), Bool(loadTransparent)])`,
            // кодек PictureRef, как у ВСЕХ 16 остальных `*.Command` (catalog/document/…).
            // РАНЬШЕ стоял PlainText → designer-чтение ЛЮБОЙ команды перечисления падало
            // «spec says List, format produced Str» (тот же класс, что вылечен у
            // `information_register.rs`). Пустой `<Picture/>` даёт РОВНО `picture_ref_default()`
            // (`decode_designer`: pack("", default_load_transparent("")) = [Str(""), Bool(true)]) —
            // тот же канон IR, что даёт EDT-ридер на отсутствующем `<picture>`.
            cmd::F_PICTURE => FieldProjection::new(
                elem(&["Picture"], ""),
                Codec::PictureRef(formats_xml::picture::PictureDialect::Designer),
            ),
            // shortcut — самостоятельный datatype метамодели (`Codec::Shortcut`), как у всех
            // 16 сиблингов. IR-эквивалентен PlainText (`Str("")` на `<Shortcut/>`), но даёт
            // полю его собственный дом (§1.6) вместо анонимного текста.
            cmd::F_SHORTCUT => FieldProjection::new(elem(&["Shortcut"], ""), Codec::Shortcut),
            cmd::F_ON_MAIN_SERVER_UNAVAILABLE => FieldProjection::new(
                elem(&["OnMainServerUnavalableBehavior"], ""),
                Codec::EnumText,
            ),
            _ => return None,
        })
    }
}

// FormRef/TemplateRef — bare-ссылки (LocusMap не проецирует свойств).
pub struct DesignerFormRef;
impl LocusMap for DesignerFormRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}
pub struct DesignerTemplateRef;
impl LocusMap for DesignerTemplateRef {
    fn lookup(&self, _f: FieldId) -> Option<FieldProjection> {
        None
    }
}

/// `&'static` бинды child-видов родителя `Enum` (кэш на процесс).
fn enum_value_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static BINDINGS: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    BINDINGS.get_or_init(|| {
        vec![
            ChildBinding {
                collection: "EnumValue",
                child_spec: enum_enum_value(),
                child_map: &DesignerEnumValue,
            },
            ChildBinding { collection: "Form", child_spec: enum_form_ref(), child_map: &DesignerFormRef },
            ChildBinding {
                collection: "Template",
                child_spec: enum_template_ref(),
                child_map: &DesignerTemplateRef,
            },
            ChildBinding { collection: "Command", child_spec: enum_command(), child_map: &DesignerCommand },
        ]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::enumeration::enumeration;

/// R-read для харнесса: `.xml`-байты Enum → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Enum", enumeration(), &DesignerEnum, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Enum → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Enum", enumeration(), &DesignerEnum, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/Enum. Дети (`Enum.EnumValue`) покрыты ТРАНЗИТИВНО.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "Enum",
    read,
    write,
    corpus_subpath: "coverage/designer/s1_core/Enums",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
