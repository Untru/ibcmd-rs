//! Designer-проекция вида `CommonForm` (зеркало `core/spec/metadata/common_form.rs`,
//! ARCHITECTURE.md §5). ТОЛЬКО размещение/кодировка ячеек дескриптора `.xml` — канонику
//! держит `core/spec` (§1.6). Это ИНОЙ синтаксис того же спека, что и EDT, поэтому оба
//! формата дают РАВНЫЙ IR (§3.5). Designer DENSE — все serialized-поля present всегда
//! (дефолты эмитятся: `<Comment/>`, `<FormType>Managed</FormType>`, `<UseStandardCommands>
//! false</…>`, `<ExtendedPresentation/>` …). Тело формы `Form.form` — в спутнике
//! `<Имя>/Ext/`, вне этого дескриптора.
//!
//! Карта `FieldId → (XmlLocus, Codec)`, локусы — пути от КОРНЯ `<MetaDataObject>` через
//! `<CommonForm>/<Properties>`:
//! * `synonym` → `<Synonym>` (`LocalizedV8`);
//! * `comment` → `<Comment>` (`PlainText`; пустой → `<Comment/>`);
//! * `formType` → `<FormType>` (`EnumText`);
//! * `includeHelpInContents` → `<IncludeHelpInContents>true|false</…>` (`BoolText`);
//! * `help` — НЕ проецируется (Designer несёт справку в спутнике `Ext/`, не в дескрипторе);
//! * `usePurposes` → `<UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">Литерал
//!   </v8:Value>…` (`UsePurposesV8`; X-исключён — Designer-литералы Platform*/MobilePlatform*);
//! * `useInInterfaceCompatibilityMode` → `<UseInInterfaceCompatibilityMode>` (`EnumText`);
//! * `useStandardCommands` → `<UseStandardCommands>true|false</…>` (`BoolText`);
//! * `extendedPresentation` → `<ExtendedPresentation>` (`LocalizedV8`; пустой → `<…/>`);
//! * `explanation` → `<Explanation>` (`LocalizedV8`; пустой → `<Explanation/>`).
//!
//! Property-теги — UpperCamelCase БЕЗ префикса (дефолтный MDClasses-ns, `ns=""`);
//! внутренности (`v8:item`/`v8:Value`) несут v8/xsi — это разбирает кодек.

use formats_xml::{Codec, FieldProjection, LocusMap, XmlLocus};
use morph1c_core::ir::FieldId;
use morph1c_core::spec::metadata::common_form::{
    F_COMMENT, F_EXPLANATION, F_EXTENDED_PRESENTATION, F_FORM_TYPE, F_INCLUDE_HELP_IN_CONTENTS,
    F_SYNONYM, F_USE_IN_INTERFACE_COMPATIBILITY_MODE, F_USE_PURPOSES, F_USE_STANDARD_COMMANDS,
};

/// `PropElement{path, ns}` (хелпер таблицы).
const fn elem(tag: &'static [&'static str], ns: &'static str) -> XmlLocus {
    XmlLocus::PropElement { path: tag, ns }
}

const P_SYNONYM: &[&str] = &["CommonForm", "Properties", "Synonym"];
const P_COMMENT: &[&str] = &["CommonForm", "Properties", "Comment"];
const P_FORM_TYPE: &[&str] = &["CommonForm", "Properties", "FormType"];
const P_INCLUDE_HELP: &[&str] = &["CommonForm", "Properties", "IncludeHelpInContents"];
const P_USE_PURPOSES: &[&str] = &["CommonForm", "Properties", "UsePurposes"];
const P_USE_IN_INTERFACE_COMPAT: &[&str] = &["CommonForm", "Properties", "UseInInterfaceCompatibilityMode"];
const P_USE_STANDARD_COMMANDS: &[&str] = &["CommonForm", "Properties", "UseStandardCommands"];
const P_EXTENDED_PRESENTATION: &[&str] = &["CommonForm", "Properties", "ExtendedPresentation"];
const P_EXPLANATION: &[&str] = &["CommonForm", "Properties", "Explanation"];

/// Карта проекции Designer для `CommonForm`.
pub struct DesignerCommonForm;

impl DesignerCommonForm {
    /// Строка проекции для поля (или `None`, если поле не проецируется этим форматом).
    fn projection_for(field: FieldId) -> Option<FieldProjection> {
        let fp = if field == F_SYNONYM {
            FieldProjection::new(elem(P_SYNONYM, ""), Codec::LocalizedV8)
        } else if field == F_COMMENT {
            FieldProjection::new(elem(P_COMMENT, ""), Codec::PlainText)
        } else if field == F_FORM_TYPE {
            FieldProjection::new(elem(P_FORM_TYPE, ""), Codec::EnumText)
        } else if field == F_INCLUDE_HELP_IN_CONTENTS {
            FieldProjection::new(elem(P_INCLUDE_HELP, ""), Codec::BoolText)
        } else if field == F_USE_PURPOSES {
            // `<UsePurposes><v8:Value xsi:type="app:ApplicationUsePurpose">…`. X-исключён (спек):
            // Designer-литералы PlatformApplication/MobilePlatformApplication.
            FieldProjection::new(elem(P_USE_PURPOSES, ""), Codec::UsePurposesV8)
        } else if field == F_USE_IN_INTERFACE_COMPATIBILITY_MODE {
            FieldProjection::new(elem(P_USE_IN_INTERFACE_COMPAT, ""), Codec::EnumText)
        } else if field == F_USE_STANDARD_COMMANDS {
            FieldProjection::new(elem(P_USE_STANDARD_COMMANDS, ""), Codec::BoolText)
        } else if field == F_EXTENDED_PRESENTATION {
            FieldProjection::new(elem(P_EXTENDED_PRESENTATION, ""), Codec::LocalizedV8)
        } else if field == F_EXPLANATION {
            FieldProjection::new(elem(P_EXPLANATION, ""), Codec::LocalizedV8)
        } else {
            // `help` (F_HELP) не проецируется Designer'ом — справка в спутнике `Ext/`.
            return None;
        };
        Some(fp)
    }
}

impl LocusMap for DesignerCommonForm {
    fn lookup(&self, field: FieldId) -> Option<FieldProjection> {
        Self::projection_for(field)
    }
}

// --- Строка R+X-харнесса вида (собирается build.rs'ом в `FORMAT_KINDS`) ---

use morph1c_core::ir::MetadataObject;
use morph1c_core::spec::metadata::common_form::common_form;

/// R-read для харнесса: `.xml`-байты CommonForm → IR (ошибка строкой).
fn read(bytes: &[u8]) -> Result<MetadataObject, String> {
    crate::read_descriptor("CommonForm", common_form(), &DesignerCommonForm, bytes)
        .map_err(|e| e.to_string())
}

/// R-write для харнесса: IR CommonForm → `.xml`-байты (byte-exact; ошибка строкой).
fn write(obj: &MetadataObject) -> Result<Vec<u8>, String> {
    crate::write_descriptor("CommonForm", common_form(), &DesignerCommonForm, obj)
        .map_err(|e| e.to_string())
}

/// Строка R+X-харнесса Designer/CommonForm. Имя `HARNESS_ENTRY` — контракт `build.rs`.
/// Раскладка `FilePerObject` — `CommonForms/<Имя>.xml` (тело формы — в `CommonForms/<Имя>/Ext/`,
/// вне дескриптора).
pub const HARNESS_ENTRY: formats_xml::FormatKind = formats_xml::FormatKind {
    format: formats_xml::Format::Designer,
    kind: "CommonForm",
    read,
    write,
    corpus_subpath: "coverage/designer/s4_common/CommonForms",
    layout: formats_xml::CorpusLayout::FilePerObject { ext: "xml" },
};
