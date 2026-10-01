//! EDT-проекция вида `Enum` + его дочернего `Enum.EnumValue` (зеркало
//! `core/spec/metadata/{enumeration,enum_enum_value}.rs`, ARCHITECTURE.md §5;
//! child-objects substrate). ТОЛЬКО размещение/кодировка
//! ячеек — канонику (id/порядок/дефолты) держит `core/spec` (§1.6).
//!
//! РОДИТЕЛЬ `Enum` (карта `FieldId → (XmlLocus, Codec)`, плоские дети корня `.mdo`):
//! * `synonym` → `<synonym>` (`LocalizedKeyVal`); `comment` → `<comment>` (`PlainText`);
//! * `useStandardCommands`/`quickChoice` → presence-bool (`BoolPresence`);
//! * `standardAttributes` → платформенный const-блок (`EnumStandardAttributes(Edt)` —
//!   эмитит 2 inline `<standardAttributes>`-блока Order+Ref);
//! * `choiceMode`/`choiceHistoryOnInput` → enum-литерал (`EnumText`);
//! * `defaultListForm`/`defaultChoiceForm` → PlainText-ссылки (`Enum.<Имя>.Form.<Форма>`,
//!   verbatim == Designer-значению). SSL их не витнессил (все пустые → «Designer-only»),
//!   ERP витнессит: `defaultChoiceForm` у 17 перечислений (напр.
//!   `ВариантыДействийПоРасхождениямВАктеПослеПриемки`), `defaultListForm` — у
//!   `СтатусыПриглашений`. Позиция = порядку спека (metamodel: quickChoice → choiceMode →
//!   defaultListForm → defaultChoiceForm; сверено `СтатусыПриглашений.mdo`, несущим оба);
//! * `listPresentation`/`extendedListPresentation`/`explanation` → `LocalizedKeyVal`
//!   (в SSL почти все пусты → EDT разрежён их опускает; 1 explanation непуст);
//! * auxiliaryListForm/auxiliaryChoiceForm are full-reference PlainText slots, witnessed
//!   in authentic UH Enum.УдалитьПредметыАренды; empty defaults stay sparse.
//! * Empty characteristics remains unprojected in EDT.
//!
//! ДОЧЕРНЯЯ коллекция `EnumValue` → inline `<enumValues uuid><name>…` (плоско, без
//! `<Properties>`-обёртки); рекурсию ведёт `formats_xml::children`.
//!
//! РЕБЁНОК `Enum.EnumValue` ([`EdtEnumValue`]) — БЕЗ `HARNESS_ENTRY` (покрыт
//! транзитивно через родителя): `synonym`→`<synonym>`, `comment`→`<comment>`. `color`
//! — Designer-only (EDT не несёт → `None`).

use formats_xml::children::ChildBinding;
use formats_xml::{ChildLocus, Codec, FieldProjection, LocusMap, StdAttrsDialect, XmlLocus};
use formats_xml::std_attrs_ir;
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::enum_command as cmd;
use morph1c_core::spec::metadata::enum_command::enum_command;
use morph1c_core::spec::metadata::enum_enum_value::{
    enum_enum_value, F_COMMENT as EV_COMMENT, F_SYNONYM as EV_SYNONYM,
};
use morph1c_core::spec::metadata::enum_form_ref as fref;
use morph1c_core::spec::metadata::enum_form_ref::enum_form_ref;
use morph1c_core::spec::metadata::enum_template_ref as tref;
use morph1c_core::spec::metadata::enum_template_ref::enum_template_ref;
use morph1c_core::spec::metadata::enumeration::{
    F_AUXILIARY_CHOICE_FORM, F_AUXILIARY_LIST_FORM,
    F_CHOICE_HISTORY_ON_INPUT, F_CHOICE_MODE, F_COMMENT, F_DEFAULT_CHOICE_FORM,
    F_DEFAULT_LIST_FORM, F_EXPLANATION, F_EXTENDED_LIST_PRESENTATION, F_LIST_PRESENTATION,
    F_QUICK_CHOICE, F_STANDARD_ATTRIBUTES, F_SYNONYM, F_USE_STANDARD_COMMANDS,
};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

// --- Родитель Enum ---
const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_USE_STANDARD_COMMANDS: &[&str] = &["useStandardCommands"];
const P_QUICK_CHOICE: &[&str] = &["quickChoice"];
const P_CHOICE_MODE: &[&str] = &["choiceMode"];
const P_CHOICE_HISTORY_ON_INPUT: &[&str] = &["choiceHistoryOnInput"];
const P_DEFAULT_LIST_FORM: &[&str] = &["defaultListForm"];
const P_DEFAULT_CHOICE_FORM: &[&str] = &["defaultChoiceForm"];
const P_LIST_PRESENTATION: &[&str] = &["listPresentation"];
const P_EXTENDED_LIST_PRESENTATION: &[&str] = &["extendedListPresentation"];
const P_EXPLANATION: &[&str] = &["explanation"];
// standardAttributes-локус не используется (const-блок навигирует от корня сам); даём
// нейтральный путь для единообразия таблицы.
const P_STANDARD_ATTRIBUTES: &[&str] = &["standardAttributes"];

/// Карта проекции EDT для родителя `Enum`.
pub struct EdtEnum;

impl EdtEnum {
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_USE_STANDARD_COMMANDS {
            FieldProjection::new(flat(P_USE_STANDARD_COMMANDS), Codec::BoolPresence)
        } else if field == F_STANDARD_ATTRIBUTES {
            FieldProjection::new(
                flat(P_STANDARD_ATTRIBUTES),
                Codec::IrStandardAttributes(StdAttrsDialect::Edt, &std_attrs_ir::ENUM),
            )
        } else if field == F_QUICK_CHOICE {
            FieldProjection::new(flat(P_QUICK_CHOICE), Codec::BoolPresence)
        } else if field == F_CHOICE_MODE {
            FieldProjection::new(flat(P_CHOICE_MODE), Codec::EnumText)
        } else if field == F_CHOICE_HISTORY_ON_INPUT {
            FieldProjection::new(flat(P_CHOICE_HISTORY_ON_INPUT), Codec::EnumText)
        } else if field == F_DEFAULT_LIST_FORM {
            // PlainText-ссылка `Enum.<Имя>.Form.<Форма>` — verbatim == Designer-значению
            // (witness ERP `СтатусыПриглашений`: <defaultListForm> между choiceMode и
            // defaultChoiceForm — позиция спека, ROOT_ORDER не нужен).
            FieldProjection::new(flat(P_DEFAULT_LIST_FORM), Codec::PlainText)
        } else if field == F_DEFAULT_CHOICE_FORM {
            // Witness ERP: 17 перечислений (напр. `ВариантыДействийПоРасхождениямВАкте
            // ПослеПриемки`) несут <defaultChoiceForm> после <choiceMode>.
            FieldProjection::new(flat(P_DEFAULT_CHOICE_FORM), Codec::PlainText)
        } else if field == F_AUXILIARY_LIST_FORM {
            FieldProjection::new(flat(&["auxiliaryListForm"]), Codec::PlainText)
        } else if field == F_AUXILIARY_CHOICE_FORM {
            FieldProjection::new(flat(&["auxiliaryChoiceForm"]), Codec::PlainText)
        } else if field == F_LIST_PRESENTATION {
            FieldProjection::new(flat(P_LIST_PRESENTATION), Codec::LocalizedKeyVal)
        } else if field == F_EXTENDED_LIST_PRESENTATION {
            FieldProjection::new(flat(P_EXTENDED_LIST_PRESENTATION), Codec::LocalizedKeyVal)
        } else if field == F_EXPLANATION {
            FieldProjection::new(flat(P_EXPLANATION), Codec::LocalizedKeyVal)
        } else {
            // Unwitnessed Designer-only characteristics remains unprojected.
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtEnum {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }

    fn child_collection(&self, collection: &str) -> Option<ChildLocus> {
        // EDT: inline-дети корня, плоская идентичность (`<enumValues uuid><name>…`;
        // Forms/Templates — inline-СТАБЫ, Command — полный sub-object).
        let tag = match collection {
            "EnumValue" => "enumValues",
            "Form" => "forms",
            "Template" => "templates",
            "Command" => "commands",
            _ => return None,
        };
        Some(ChildLocus {
            container: &[],
            child_tag: tag,
            props_wrapped: false,
            name_tag: "name",
            bare_ref: false,
        })
    }

    fn child_bindings(&self) -> &'static [ChildBinding] {
        enum_value_bindings()
    }

    fn root_extra_namespaces(&self) -> &'static [(&'static str, &'static str)] {
        // EDT Enum-корень несёт xmlns:xsi+xmlns:core (нужны const-блоку
        // standardAttributes: `xsi:type="core:UndefinedValue"`). Порядок = эталона
        // (xsi, затем core; оба ДО xmlns:mdclass).
        &[
            ("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"),
            ("xmlns:core", "http://g5.1c.ru/v8/dt/mcore"),
        ]
    }
}

// --- Ребёнок Enum.EnumValue (БЕЗ HARNESS_ENTRY — транзитивное покрытие) ---

const EV_P_SYNONYM: &[&str] = &["synonym"];
const EV_P_COMMENT: &[&str] = &["comment"];
const EV_P_COLOR: &[&str] = &["color"];

/// Карта проекции EDT для child-вида `Enum.EnumValue`. Локусы относительны элемента
/// `<enumValues>` (props_wrapped=false).
pub struct EdtEnumValue;

impl LocusMap for EdtEnumValue {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        if field == EV_SYNONYM {
            Some(FieldProjection::new(flat(EV_P_SYNONYM), Codec::LocalizedKeyVal))
        } else if field == EV_COMMENT {
            Some(FieldProjection::new(flat(EV_P_COMMENT), Codec::PlainText))
        } else if field == morph1c_core::spec::metadata::enum_enum_value::F_COLOR {
            Some(FieldProjection::new(flat(EV_P_COLOR), Codec::MetadataColor(formats_xml::metadata_color::Dialect::Edt)))
        } else {
            // `color` — Designer-only (EDT EnumValue его не несёт).
            None
        }
    }
}

// ===== Дети FormRef / TemplateRef (EDT — полный inline-стаб) =====
pub struct EdtFormRef;
impl LocusMap for EdtFormRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            fref::F_FORM_TYPE => FieldProjection::new(flat(&["formType"]), Codec::EnumText),
            fref::F_SYNONYM => FieldProjection::new(flat(&["synonym"]), Codec::LocalizedKeyVal),
            fref::F_COMMENT => FieldProjection::new(flat(&["comment"]), Codec::PlainText),
            fref::F_INCLUDE_HELP_IN_CONTENTS => {
                FieldProjection::new(flat(&["includeHelpInContents"]), Codec::BoolPresence)
            }
            fref::F_HELP => FieldProjection::new(flat(&["help"]), Codec::HelpConst),
            fref::F_USE_PURPOSES => {
                FieldProjection::new(flat(&["usePurposes"]), Codec::RefList(formats_xml::ref_list::RefListDialect::Edt))
            }
            _ => return None,
        })
    }
}

pub struct EdtTemplateRef;
impl LocusMap for EdtTemplateRef {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            tref::F_SYNONYM => FieldProjection::new(flat(&["synonym"]), Codec::LocalizedKeyVal),
            tref::F_COMMENT => FieldProjection::new(flat(&["comment"]), Codec::PlainText),
            tref::F_TEMPLATE_TYPE => FieldProjection::new(flat(&["templateType"]), Codec::EnumText),
            _ => return None,
        })
    }
}

// ===== Дитя Command (полный sub-object, зеркало InformationRegister.Command) =====
pub struct EdtCommand;
impl LocusMap for EdtCommand {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Some(match field {
            cmd::F_SYNONYM => FieldProjection::new(flat(&["synonym"]), Codec::LocalizedKeyVal),
            cmd::F_COMMENT => FieldProjection::new(flat(&["comment"]), Codec::PlainText),
            cmd::F_GROUP => FieldProjection::new(flat(&["group"]), Codec::EnumText),
            cmd::F_COMMAND_PARAMETER_TYPE => FieldProjection::new(
                flat(&["commandParameterType"]),
                Codec::Type(formats_xml::TypeDialect::Edt),
            ),
            cmd::F_MODIFIES_DATA => {
                FieldProjection::new(flat(&["modifiesData"]), Codec::BoolPresence)
            }
            cmd::F_REPRESENTATION => FieldProjection::new(flat(&["representation"]), Codec::EnumText),
            cmd::F_TOOL_TIP => FieldProjection::new(flat(&["toolTip"]), Codec::LocalizedKeyVal),
            // parameterUseMode/picture/shortcut платформенный EDT НЕСЁТ — «Designer-only»
            // было ошибкой (зеркало `information_register.rs::EdtCommand`, ERP-witnessed).
            // Без этих трёх проекций ir→edt МОЛЧА терял бы поля (`encode_field` на
            // `lookup()==None` возвращает Ok, §1.0-потеря).
            cmd::F_PARAMETER_USE_MODE => {
                FieldProjection::new(flat(&["parameterUseMode"]), Codec::EnumText)
            }
            cmd::F_PICTURE => FieldProjection::new(
                flat(&["picture"]),
                Codec::PictureRef(formats_xml::picture::PictureDialect::Edt),
            ),
            cmd::F_SHORTCUT => FieldProjection::new(flat(&["shortcut"]), Codec::Shortcut),
            // onMainServer* — Designer-only.
            _ => return None,
        })
    }
    fn field_emit_order(&self) -> Option<&'static [FieldId]> {
        Some(COMMAND_ORDER)
    }
}

/// EDT физический порядок Command — ПОБАЙТНОЕ зеркало
/// `information_register.rs::COMMAND_ORDER` (ERP-witnessed: parameterUseMode между
/// commandParameterType и modifiesData, picture/shortcut в хвосте; `comment` вне списка ⇒
/// уходит в конец — как у зеркала).
static COMMAND_ORDER: &[FieldId] = &[
    cmd::F_SYNONYM,
    cmd::F_GROUP,
    cmd::F_COMMAND_PARAMETER_TYPE,
    cmd::F_PARAMETER_USE_MODE,
    cmd::F_MODIFIES_DATA,
    cmd::F_REPRESENTATION,
    cmd::F_TOOL_TIP,
    cmd::F_PICTURE,
    cmd::F_SHORTCUT,
];

/// `&'static` бинды child-видов родителя `Enum` (кэш на процесс).
fn enum_value_bindings() -> &'static [ChildBinding] {
    use std::sync::OnceLock;
    static BINDINGS: OnceLock<Vec<ChildBinding>> = OnceLock::new();
    BINDINGS.get_or_init(|| {
        vec![
            ChildBinding {
                collection: "EnumValue",
                child_spec: enum_enum_value(),
                child_map: &EdtEnumValue,
            },
            ChildBinding { collection: "Form", child_spec: enum_form_ref(), child_map: &EdtFormRef },
            ChildBinding {
                collection: "Template",
                child_spec: enum_template_ref(),
                child_map: &EdtTemplateRef,
            },
            ChildBinding { collection: "Command", child_spec: enum_command(), child_map: &EdtCommand },
        ]
    })
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::enumeration::enumeration;

/// R-read для харнесса: `.mdo`-байты Enum → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("Enum", enumeration(), &EdtEnum, bytes).map_err(|e| e.to_string())
}

/// R-write для харнесса: IR Enum → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("Enum", enumeration(), &EdtEnum, obj).map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/Enum. Имя `HARNESS_ENTRY` — контракт `build.rs`. Дети
/// (`Enum.EnumValue`) покрыты ТРАНЗИТИВНО (R byte-exact ⇒ inline-дети byte-exact; X
/// IR-равен ⇒ `children` рекурсивно равны) — отдельной записи у них нет.
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "Enum",
    read,
    write,
    corpus_subpath: "coverage/edt/s1_core/src/Enums",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
