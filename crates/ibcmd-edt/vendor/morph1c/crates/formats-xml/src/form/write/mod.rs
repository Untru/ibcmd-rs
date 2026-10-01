//! WRITE: [`FormBody`] → байты тела формы (byte-exact, §3.2). EDT и Designer эмитят
//! РАЗНЫЙ физический порядок регионов (сверено по корпусу); каждый dialect собирает своё.

use super::fields::{
    designer_attr_value, emit_designer_auto_edit_mode, emit_field_designer, emit_field_edt,
    FieldProj,
};
use super::projection::{designer_attr_node, edt_attr_node};
use super::tables::{self, DesSlot};
use super::{
    designer_envelope, edt_envelope, form_profile_for, witnessed_form_versions, FormDialect,
    FormError, CORE_NS_URI, DESIGNER_FORM_NS, FORM_COMMAND_BAR_NAME, FORM_NS_URI, MXL_NS_URI,
    SCHEMA_NS_URI, SETTINGS_NS_URI, XSI_NS_URI,
};
use crate::emit::{render, OutElement};
use crate::value_codec::{self, ValueDialect};
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{
    DcsAvailableValue, DcsCalculatedField, DcsCorValue, DcsField, DcsItem, DcsListSettings,
    DcsOrderExpression, DcsParamValue,
    DcsParameter, DcsPresentation, DcsRightValue, DcsSettingsGroup, DcsSettingsParameterValue,
    DcsUseRestriction,
    DecoratorBody, DecoratorRef, DynamicListAttrExt, DynamicListExt, FieldId, FontRef, FormBody,
    FormCiItem, FormCommand, FormDataAttribute, FormItem, FormParameter, Lang, ReportFormInfo,
    TooltipBody, REPORT_FORM_AUTO,
};
// `MxlSpreadsheetSettings`/`MxlNode` — по полному пути модуля (не через re-export `ir::mod`, вне границ лейна).
use morph1c_core::ir::form::{MxlNode, MxlSpreadsheetSettings};
use morph1c_core::spec::forms::command as fc;
use morph1c_core::spec::forms::controls::button as bt;
use morph1c_core::spec::forms::controls::form_field as ff;
use morph1c_core::spec::forms::controls::form_group as fg;
use morph1c_core::spec::forms::controls::label_decoration as ld;
use morph1c_core::spec::forms::controls::radio_button as rb;
use morph1c_core::spec::forms::controls::table as tb;
use morph1c_core::spec::forms::form_root as fr;

mod edt;
mod edt_data;
mod edt_dcs;
mod edt_controls;
mod font;
mod tooltip;
mod designer;
mod designer_controls;
mod mxl;
mod designer_dcs;

pub(crate) use edt::*;
pub(crate) use edt_data::*;
pub(crate) use edt_dcs::*;
pub(crate) use edt_controls::*;
pub(crate) use font::*;
pub(crate) use tooltip::*;
pub(crate) use designer::*;
pub(crate) use designer_controls::*;
pub(crate) use mxl::*;
pub(crate) use designer_dcs::*;

/// Записать тело формы в байты заданного формата (byte-exact).
pub fn write_form(dialect: FormDialect, body: &FormBody) -> Result<Vec<u8>, FormError> {
    match dialect {
        FormDialect::Edt => write_edt(body),
        FormDialect::Designer => write_designer(body),
    }
}

// --- значение форм-атрибута: из bag, иначе канонический spec.default ---
pub(crate) fn attr_value(body: &FormBody, id: morph1c_core::ir::FieldId) -> Option<&PropertyValue> {
    body.attributes
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| v)
}

/// EDT `<id>` реквизита/таблицы: эмитим ЛИШЬ ненулевой (EDT ОПУСКАЕТ дефолт `0`; ⟺ Designer
/// эмитит `id="0"` всегда). Зеркало [`read::read_edt_id`]. См. ERP-witness id=0 (реквизит
/// ПоддержкаЭлектронногоАктированияВЕИС / главная autoTable).
pub(crate) fn push_edt_id(el: &mut OutElement, id: i64) {
    if id != 0 {
        el.push(OutElement::leaf("", "id", id.to_string()));
    }
}
