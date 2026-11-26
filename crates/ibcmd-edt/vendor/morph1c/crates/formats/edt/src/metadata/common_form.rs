//! EDT-проекция вида `CommonForm` (зеркало `core/spec/metadata/common_form.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек дескриптора `.mdo` — канонику
//! держит `core/spec` (§1.6). Тело формы `Form.form` — вне этого дескриптора (под-IR L1f).
//!
//! Карта `FieldId → (XmlLocus, Codec)` (EDT SPARSE — дефолты опущены):
//! * `synonym` → `<synonym><key>/<value>` (`LocalizedKeyVal`);
//! * `comment` → `<comment>` (`PlainText`; дефолт `""` опущен);
//! * `formType` → `<formType>` (`EnumText`; дефолт `Managed` опущен — 0/104 в SSL);
//! * `includeHelpInContents` → `<includeHelpInContents>true</…>` (`BoolPresence`; дефолт опущен);
//! * `help` → `<help><pages><lang>ru</lang></pages></help>` (`HelpConst`; presence; X-исключён);
//! * `usePurposes` → сиблинги `<usePurposes>Литерал</usePurposes>` (`RefList(Edt)`; X-исключён);
//! * `useInInterfaceCompatibilityMode` → `<useInInterfaceCompatibilityMode>` (`EnumText`;
//!   дефолт `Any` опущен — 0/104);
//! * `useStandardCommands` → `<useStandardCommands>true</…>` (`BoolPresence`; дефолт опущен);
//! * `extendedPresentation` → `<extendedPresentation>` (`LocalizedKeyVal`; дефолт опущен);
//! * `explanation` → `<explanation>` (`LocalizedKeyVal`; дефолт опущен).
//!
//! Все плоские дети корня (`path` длины 1), без ns-префикса (`ns=""`). Порядок детей в
//! файле задаёт ПОРЯДОК СПЕКА (движок walk'ает спек), карта — лишь имена.

use formats_xml::ref_list::RefListDialect;
use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_form::{
    F_COMMENT, F_EXPLANATION, F_EXTENDED_PRESENTATION, F_FORM_TYPE, F_HELP,
    F_INCLUDE_HELP_IN_CONTENTS, F_SYNONYM, F_USE_IN_INTERFACE_COMPATIBILITY_MODE, F_USE_PURPOSES,
    F_USE_STANDARD_COMMANDS,
};

/// Плоский `PropElement{path:[tag], ns:""}` (хелпер таблицы).
const fn flat(tag: &'static [&'static str]) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns: "" }
}

const P_SYNONYM: &[&str] = &["synonym"];
const P_COMMENT: &[&str] = &["comment"];
const P_FORM_TYPE: &[&str] = &["formType"];
const P_INCLUDE_HELP: &[&str] = &["includeHelpInContents"];
const P_HELP: &[&str] = &["help"];
const P_USE_PURPOSES: &[&str] = &["usePurposes"];
const P_USE_IN_INTERFACE_COMPAT: &[&str] = &["useInInterfaceCompatibilityMode"];
const P_USE_STANDARD_COMMANDS: &[&str] = &["useStandardCommands"];
const P_EXTENDED_PRESENTATION: &[&str] = &["extendedPresentation"];
const P_EXPLANATION: &[&str] = &["explanation"];

/// Карта проекции EDT для `CommonForm`.
pub struct EdtCommonForm;

impl EdtCommonForm {
    /// Строка проекции для поля (или `None`, если поле не из этого вида).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(flat(P_SYNONYM), Codec::LocalizedKeyVal)
        } else if field == F_COMMENT {
            FieldProjection::new(flat(P_COMMENT), Codec::PlainText)
        } else if field == F_FORM_TYPE {
            FieldProjection::new(flat(P_FORM_TYPE), Codec::EnumText)
        } else if field == F_INCLUDE_HELP_IN_CONTENTS {
            FieldProjection::new(flat(P_INCLUDE_HELP), Codec::BoolPresence)
        } else if field == F_HELP {
            // EDT-only `<help><pages><lang>ru</lang></pages></help>` — фикс const-блок
            // (тот же `HelpConst`, что у Subsystem/Catalog/InformationRegister). X-исключён (спек).
            FieldProjection::new(flat(P_HELP), Codec::HelpConst)
        } else if field == F_USE_PURPOSES {
            // СПИСОК сиблингов `<usePurposes>Литерал</usePurposes>` (plain-text ref-list).
            // X-исключён (спек): EDT-литералы PersonalComputer/MobileDevice.
            FieldProjection::new(flat(P_USE_PURPOSES), Codec::RefList(RefListDialect::Edt))
        } else if field == F_USE_IN_INTERFACE_COMPATIBILITY_MODE {
            FieldProjection::new(flat(P_USE_IN_INTERFACE_COMPAT), Codec::EnumText)
        } else if field == F_USE_STANDARD_COMMANDS {
            FieldProjection::new(flat(P_USE_STANDARD_COMMANDS), Codec::BoolPresence)
        } else if field == F_EXTENDED_PRESENTATION {
            FieldProjection::new(flat(P_EXTENDED_PRESENTATION), Codec::LocalizedKeyVal)
        } else if field == F_EXPLANATION {
            FieldProjection::new(flat(P_EXPLANATION), Codec::LocalizedKeyVal)
        } else {
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for EdtCommonForm {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_form::common_form;

/// R-read для харнесса: `.mdo`-байты CommonForm → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonForm", common_form(), &EdtCommonForm, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonForm → `.mdo`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonForm", common_form(), &EdtCommonForm, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса EDT/CommonForm. Имя `HARNESS_ENTRY` — контракт `build.rs`.
/// Раскладка `DirPerObject` — `CommonForms/<Имя>/<Имя>.mdo` (тело формы `Form.form` —
/// отдельный файл, вне дескриптора).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Edt,
    kind: "CommonForm",
    read,
    write,
    corpus_subpath: "coverage/edt/s4_common/src/CommonForms",
    layout: formats_xml::CorpusLayout::DirPerObject { ext: "mdo" },
};
