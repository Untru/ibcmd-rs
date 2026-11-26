//! Канонический спек ДОЧЕРНЕГО вида `AccountingRegister.Resource` (ресурс регистра
//! бухгалтерии) — child-objects substrate. Лист-вид
//! (`children: &[]`), БЕЗ `HARNESS_ENTRY` (покрыт ТРАНЗИТИВНО через родителя).
//!
//! Поля = базовый набор leaf-детей [`crate::spec::ir_child::base_child_fields`] с ТЕМИ ЖЕ
//! регистро-поправками, что и у регистра накопления ([`ac_base_child_fields`]: fillValue
//! DEFAULTED (Undefined), choiceParameters — `List`), ПЛЮС три поля регистра бухгалтерии:
//! `balance` (bool), `accountingFlag` (ref → AccountingFlag), `extDimensionAccountingFlag`
//! (ref → ExtDimensionAccountingFlag). Сверено designer/edt/cf-фикстурами
//! `coverage/*/s2_registers/AccountingRegisters` (6 ресурсов на объект).

use crate::ir::value::{PropertyValue, ValueKind, ValueScalarKind, ValueSpec};
use crate::ir::FieldId;
use crate::spec::common::{EntitySpec, FieldSpec};
use crate::spec::ir_child::{base_child_fields, F_CHOICE_PARAMETERS, F_FILL_VALUE};

/// `balance` (признак «остаток») — СОБСТВЕННЫЙ id вида. Default false (designer DENSE
/// эмитит `<Balance>true</Balance>`; cf INNER10-слот; edt `<balance>`). Общий для
/// Resource/Dimension регистра бухгалтерии.
pub const F_BALANCE: FieldId = FieldId(40);
/// `accountingFlag` (ref → ChartOfAccounts.AccountingFlag) — СОБСТВЕННЫЙ id. Default ""
/// (в корпусе покрытия ПУСТ во всех объектах; cf → zeroGuid). Общий для Resource/Dimension.
pub const F_ACCOUNTING_FLAG: FieldId = FieldId(41);
/// `extDimensionAccountingFlag` (ref → ExtDimensionAccountingFlag) — Resource-only id.
/// Default "" (в корпусе покрытия ПУСТ; cf → zeroGuid).
pub const F_EXT_DIMENSION_ACCOUNTING_FLAG: FieldId = FieldId(42);

fn bool_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Bool, PropertyValue::Bool(false))
}
fn str_f(id: FieldId, name: &'static str) -> FieldSpec {
    FieldSpec::with_default(id, name, ValueKind::Str, PropertyValue::Str(String::new()))
}

/// Базовый набор leaf-детей регистра бухгалтерии: как [`base_child_fields`], но с ДВУМЯ
/// регистро-поправками (зеркало `accumulation_register_resource::ar_base_child_fields`),
/// т.к. `ir_child` заточён под InformationRegister:
/// - `fillValue` — DEFAULTED (Undefined): дети регистра бухгалтерии его НЕ несут;
/// - `choiceParameters` — `List` (choiceParameters-кодек), а НЕ Designer-only `Str`.
///
/// Общий для Resource/Attribute/Dimension этого вида.
pub fn ac_base_child_fields() -> Vec<FieldSpec> {
    let undefined = PropertyValue::Value(ValueSpec { kind: ValueScalarKind::Undefined, scalar: None });
    base_child_fields()
        .into_iter()
        .map(|f| {
            if f.id == F_FILL_VALUE {
                FieldSpec::with_default(f.id, f.name, f.value_kind, undefined.clone())
            } else if f.id == F_CHOICE_PARAMETERS {
                FieldSpec::with_default(f.id, f.name, ValueKind::List, PropertyValue::List(Vec::new()))
            } else {
                f
            }
        })
        .collect()
}

fn build_fields() -> Vec<FieldSpec> {
    let mut v = ac_base_child_fields();
    // Три поля регистра бухгалтерии В КОНЕЦ канонического набора (физ.порядок форматов —
    // через field_emit_order проекций): balance, accountingFlag, extDimensionAccountingFlag.
    v.push(bool_f(F_BALANCE, "balance"));
    v.push(str_f(F_ACCOUNTING_FLAG, "accountingFlag"));
    v.push(str_f(F_EXT_DIMENSION_ACCOUNTING_FLAG, "extDimensionAccountingFlag"));
    v
}

/// `&'static EntitySpec` вида `AccountingRegister.Resource` (кэш на процесс).
pub fn accounting_register_resource() -> &'static EntitySpec {
    use std::sync::OnceLock;
    static SPEC: OnceLock<EntitySpec> = OnceLock::new();
    SPEC.get_or_init(|| EntitySpec {
        entity: "AccountingRegister.Resource",
        fields: Box::leak(build_fields().into_boxed_slice()),
        children: &[],
    })
}
