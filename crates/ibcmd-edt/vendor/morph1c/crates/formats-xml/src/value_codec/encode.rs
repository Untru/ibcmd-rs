//! ENCODE host-элемента Value из ValueSpec (byte-exact, оба диалекта) + require-хелперы.

use super::*;

/// Encode an EDT Str/Reference host, honouring the presence-of-default split (see
/// [`decode_edt_str_ref`]): `scalar: None` re-emits the explicit `<value></value>` child;
/// `Some(Str(""))` re-emits the self-close host; non-empty re-emits `<value>t</value>`.
fn encode_edt_str_ref(
    prefix: &str,
    local: &str,
    xsi: &str,
    spec: &ValueSpec,
    kind_name: &str,
) -> Result<OutElement, String> {
    let with_value = |text: &str| -> OutElement {
        let mut h = OutElement::branch(prefix, local).attr(XSI_TYPE, xsi);
        h.push(OutElement::leaf("", "value", text.to_string()));
        h
    };
    Ok(match spec.scalar.as_deref() {
        // presence-of-default: explicit empty `<value></value>` child.
        None => with_value(""),
        Some(PropertyValue::Str(s)) if s.is_empty() => {
            OutElement::self_closing(prefix, local).attr(XSI_TYPE, xsi)
        }
        Some(PropertyValue::Str(s)) => with_value(s),
        other => {
            return Err(format!(
                "Value: {kind_name} requires Str scalar (or None for explicit-empty <value/>), \
                 got {:?} (§1.0)",
                other.map(|v| v.kind())
            ));
        }
    })
}

/// Encode a Designer Str/Reference host. Designer has ONE empty form (self-close), so BOTH
/// the canonical empty (`Some(Str(""))`) and the EDT-only explicit-empty marker
/// (`scalar: None`) collapse to a self-close `<v xsi:type="xs:string"/>`; non-empty → text.
fn encode_des_str_ref(
    prefix: &str,
    local: &str,
    xsi: &str,
    spec: &ValueSpec,
    kind_name: &str,
) -> Result<OutElement, String> {
    Ok(match spec.scalar.as_deref() {
        None => OutElement::self_closing(prefix, local).attr(XSI_TYPE, xsi),
        Some(PropertyValue::Str(s)) if s.is_empty() => {
            OutElement::self_closing(prefix, local).attr(XSI_TYPE, xsi)
        }
        Some(PropertyValue::Str(s)) => {
            OutElement::leaf(prefix, local, s.clone()).attr(XSI_TYPE, xsi)
        }
        other => {
            return Err(format!(
                "Value: {kind_name} requires Str scalar (or None for explicit-empty), got {:?} (§1.0)",
                other.map(|v| v.kind())
            ));
        }
    })
}

pub(crate) fn encode_edt(
    prefix: &str,
    local: &str,
    spec: &ValueSpec,
) -> Result<OutElement, String> {
    let sc = || -> OutElement { OutElement::self_closing(prefix, local) };
    let with_value = |xsi: &str, text: &str| -> OutElement {
        let mut h = OutElement::branch(prefix, local).attr(XSI_TYPE, xsi);
        h.push(OutElement::leaf("", "value", text.to_string()));
        h
    };
    Ok(match spec.kind {
        ValueScalarKind::SystemEnum => {
            let value = require_str(spec, "SystemEnum")?;
            comparison_member(value)?;
            with_value(EDT_SYSTEM_ENUM, value)
        }
        ValueScalarKind::Undefined => sc().attr(XSI_TYPE, EDT_UNDEFINED),
        ValueScalarKind::Str => encode_edt_str_ref(prefix, local, EDT_STRING, spec, "Str")?,
        ValueScalarKind::Reference => {
            encode_edt_str_ref(prefix, local, EDT_REFERENCE, spec, "Reference")?
        }
        ValueScalarKind::Bool => {
            // false ⇒ self-close; true ⇒ <value>true</value> (corpus convention).
            if require_bool(spec)? {
                with_value(EDT_BOOLEAN, "true")
            } else {
                sc().attr(XSI_TYPE, EDT_BOOLEAN)
            }
        }
        ValueScalarKind::Number => with_value(EDT_NUMBER, require_str(spec, "Number")?),
        ValueScalarKind::Date => with_value(EDT_DATE, require_str(spec, "Date")?),
        // Пустое ОписаниеТипов: `<host xsi:type="core:TypeDescriptionValue"><value/></host>`.
        ValueScalarKind::TypeDescription => {
            let mut h = OutElement::branch(prefix, local).attr(XSI_TYPE, EDT_TYPE_DESCRIPTION);
            h.push(OutElement::self_closing("", "value"));
            h
        }
        ValueScalarKind::AccountType => {
            let t = require_str(spec, "AccountType")?;
            require_account_type_literal(t)?;
            // Дефолт-литерал `Active` (cf-код 0) EDT ОПУСКАЕТ ⇒ self-close; иначе явный `<value>`.
            if t == ACCOUNT_TYPE_OMITTED {
                sc().attr(XSI_TYPE, EDT_ACCOUNT_TYPE)
            } else {
                with_value(EDT_ACCOUNT_TYPE, t)
            }
        }
        // Пустой список значений: self-close `<host xsi:type="core:ValueList"/>`.
        ValueScalarKind::ValueList => {
            require_empty_value_list(spec)?;
            sc().attr(XSI_TYPE, EDT_VALUE_LIST)
        }
    })
}

pub(crate) fn encode_designer(
    prefix: &str,
    local: &str,
    spec: &ValueSpec,
) -> Result<OutElement, String> {
    let leaf = |xsi: &str, text: &str| -> OutElement {
        OutElement::leaf(prefix, local, text.to_string()).attr(XSI_TYPE, xsi)
    };
    Ok(match spec.kind {
        ValueScalarKind::SystemEnum => leaf(
            DES_COMPARISON_ENUM,
            comparison_member(require_str(spec, "SystemEnum")?)?,
        ),
        ValueScalarKind::Undefined => OutElement::self_closing(prefix, local).attr(XSI_NIL, "true"),
        ValueScalarKind::Str => encode_des_str_ref(prefix, local, DES_STRING, spec, "Str")?,
        ValueScalarKind::Reference => {
            encode_des_str_ref(prefix, local, DES_REFERENCE, spec, "Reference")?
        }
        ValueScalarKind::Bool => leaf(
            DES_BOOLEAN,
            if require_bool(spec)? { "true" } else { "false" },
        ),
        ValueScalarKind::Number => leaf(DES_DECIMAL, require_str(spec, "Number")?),
        ValueScalarKind::Date => leaf(DES_DATETIME, require_str(spec, "Date")?),
        // Пустое ОписаниеТипов: self-close `<host xsi:type="v8:TypeDescription"/>`.
        ValueScalarKind::TypeDescription => {
            OutElement::self_closing(prefix, local).attr(XSI_TYPE, DES_TYPE_DESCRIPTION)
        }
        ValueScalarKind::AccountType => {
            let t = require_str(spec, "AccountType")?;
            require_account_type_literal(t)?;
            leaf(DES_ACCOUNT_TYPE, t)
        }
        // Пустой список значений: self-close `<host xsi:type="xr:ValueList"/>`.
        ValueScalarKind::ValueList => {
            require_empty_value_list(spec)?;
            OutElement::self_closing(prefix, local).attr(XSI_TYPE, DES_VALUE_LIST)
        }
    })
}

/// §1.0: `ValueList` в корпусе только ПУСТОЙ (`scalar==None`). Непустой — не витнессирован.
fn require_empty_value_list(spec: &ValueSpec) -> Result<(), String> {
    if spec.scalar.is_some() {
        return Err(format!(
            "Value: non-empty ValueList is UNWITNESSED (scalar={:?}); only the empty typed marker \
             is modelled (§1.0)",
            spec.scalar.as_deref().map(|v| v.kind())
        ));
    }
    Ok(())
}

fn require_str<'a>(spec: &'a ValueSpec, kind: &str) -> Result<&'a str, String> {
    match spec.scalar.as_deref() {
        Some(PropertyValue::Str(s)) => Ok(s),
        other => Err(format!(
            "Value: {kind} requires Str scalar, got {:?} (§1.0)",
            other.map(|v| v.kind())
        )),
    }
}
fn require_bool(spec: &ValueSpec) -> Result<bool, String> {
    match spec.scalar.as_deref() {
        Some(PropertyValue::Bool(b)) => Ok(*b),
        other => Err(format!(
            "Value: Bool requires Bool scalar, got {:?} (§1.0)",
            other.map(|v| v.kind())
        )),
    }
}
