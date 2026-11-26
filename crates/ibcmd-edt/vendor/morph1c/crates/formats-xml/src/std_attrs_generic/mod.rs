//! ОБОБЩЁННЫЙ (data-driven) кодек платформенного блока `standardAttributes`/
//! `StandardAttributes` (child-objects substrate §3.3). Читает `&'static StdAttrsDecl`
//! из канонического спека (`core/spec`) и проецирует его в ОБА XML-формата — заменяет
//! пер-видовые рукописные кодеки (Catalog/Document/DocumentJournal/Task/BusinessProcess/
//! ChartOfCharacteristicTypes/ExchangePlan) ОДНОЙ машинерией (§1.6: канон — в спеке,
//! формат — лишь проекция; будущий структурный вид добавляет ZERO shared-file правок).
//!
//! Вид отличается ТОЛЬКО данными декларации: набор+порядок атрибутов, name-определённые
//! константы, variable-слоты (типы/дефолты), EDT-разрежённая последовательность листьев,
//! Designer-DENSE 25-листовый регион и корневой тег-обёртка. Машинерия здесь —
//! ВЕРБАТИМ-порт бывшего `std_attrs_catalog` (тот же claim/decode/emit; §1.0 строго-
//! тотальна: любое отклонение → ОШИБКА).

use crate::choice_param_links;
use crate::choice_parameters;
use crate::descriptor::Element;
use crate::emit::OutElement;
use crate::link_by_type::{self, LinkByTypeDialect};
use crate::value_codec::{self, ValueDialect};
use morph1c_core::engine::Decoded;
use morph1c_core::ir::value::{Lang, PropertyValue, Token, ValueScalarKind, ValueSpec};
use morph1c_core::spec::common::{
    AttrDecl, DenseLeaf, DenseLeafRule, EdtLeaf, StdAttrsDecl, StdAttrsVariant, VarSlotKind,
};
use morph1c_core::version::{designer_field_available_in, FormatVersion};

/// Диалект блока (EDT inline-блоки vs Designer `<StandardAttributes>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GenDialect {
    /// EDT: подряд `<standardAttributes>`-блоки.
    Edt,
    /// Designer: `<StandardAttributes>` с `<xr:StandardAttribute>`'ами.
    Designer,
}

mod common;
mod edt;
mod designer;

pub(crate) use common::*;
pub(crate) use edt::*;
pub(crate) use designer::*;

// ---- public DECODE / CLAIM / EMIT ------------------------------------------

/// Декодировать блок (от source-root) в канонический IR-`List`. Строго-тотально.
pub fn decode(
    dialect: GenDialect,
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    root: &Element,
    version: FormatVersion,
) -> Decoded {
    let res = match dialect {
        GenDialect::Edt => decode_edt(decl, variant, root),
        GenDialect::Designer => decode_designer(decl, variant, root, version),
    };
    match res {
        Ok(v) => Decoded::Present(v),
        Err(e) => Decoded::Error(e),
    }
}

/// Claim-проход (для leftover): клеймит блок РОВНО если он совпал с ожидаемой структурой.
pub fn claim(
    dialect: GenDialect,
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    root: &Element,
    version: FormatVersion,
) {
    match dialect {
        GenDialect::Edt => {
            let _ = decode_edt(decl, variant, root);
        }
        GenDialect::Designer => {
            let _ = decode_designer(decl, variant, root, version);
        }
    }
}

/// Эмитировать блок byte-exact из IR-`List`.
pub fn emit(
    dialect: GenDialect,
    decl: &StdAttrsDecl,
    variant: StdAttrsVariant,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<Vec<OutElement>, String> {
    match dialect {
        GenDialect::Edt => emit_edt(decl, variant, value),
        GenDialect::Designer => emit_designer(decl, variant, value, version),
    }
}
