//! DECODE host-элемента Value в канонический ValueSpec (оба диалекта) + ридер-хелперы.

use super::*;

pub(crate) fn decode_edt(host: &Element) -> Result<PropertyValue, String> {
    let xsi = claim_only_attr(host, XSI_TYPE)?;
    let spec = match xsi.as_str() {
        EDT_UNDEFINED => {
            ensure_no_children(host)?;
            ValueSpec { kind: ValueScalarKind::Undefined, scalar: None }
        }
        EDT_STRING => decode_edt_str_ref(ValueScalarKind::Str, host)?,
        EDT_REFERENCE => decode_edt_str_ref(ValueScalarKind::Reference, host)?,
        EDT_BOOLEAN => match edt_opt_value_child(host)? {
            // self-close ⇒ false; `<value>true</value>` ⇒ true (corpus: только эти формы).
            None => bool_spec(false),
            Some(t) if t == "true" => bool_spec(true),
            Some(t) => {
                return Err(format!(
                    "Value: EDT BooleanValue <value> must be \"true\" (false is self-close), got {t:?} (§1.0)"
                ))
            }
        },
        EDT_NUMBER => {
            let t = edt_require_value_child(host, EDT_NUMBER)?;
            str_spec(ValueScalarKind::Number, t)
        }
        EDT_DATE => {
            let t = edt_require_value_child(host, EDT_DATE)?;
            str_spec(ValueScalarKind::Date, t)
        }
        EDT_TYPE_DESCRIPTION => {
            // Пустое ОписаниеТипов: ВСЕГДА несёт пустой `<value/>`-ребёнок (сверено).
            match edt_opt_value_child(host)? {
                Some(t) if t.is_empty() => ValueSpec { kind: ValueScalarKind::TypeDescription, scalar: None },
                Some(t) => {
                    return Err(format!(
                        "Value: EDT TypeDescriptionValue <value> must be empty, got {t:?} (§1.0)"
                    ))
                }
                None => {
                    return Err("Value: EDT TypeDescriptionValue requires empty <value/> child (§1.0)".into())
                }
            }
        }
        EDT_ACCOUNT_TYPE => {
            // `<value>` ОПЦИОНАЛЕН: EDT опускает дефолт-литерал `Active` (self-close, cf-код 0),
            // эмитит явно `Passive`/`ActivePassive` (ERP-witness Хозрасчетный.ФормаСчета).
            let t = edt_opt_value_child(host)?.unwrap_or_else(|| ACCOUNT_TYPE_OMITTED.to_string());
            require_account_type_literal(&t)?;
            str_spec(ValueScalarKind::AccountType, t)
        }
        EDT_VALUE_LIST => {
            // Пустой типизированный список значений (self-close). Непустой layout НЕ витнессирован.
            ensure_no_children(host)?;
            ValueSpec { kind: ValueScalarKind::ValueList, scalar: None }
        }
        other => {
            return Err(format!(
                "Value: unknown EDT xsi:type {other:?} (witnessed: Undefined/String/Boolean/\
                 Number/Date/Reference/TypeDescription/AccountType/ValueList — §1.0: no guess/passthrough)"
            ))
        }
    };
    Ok(PropertyValue::Value(spec))
}

pub(crate) fn decode_designer(host: &Element) -> Result<PropertyValue, String> {
    let has_nil = host.attr(XSI_NIL).is_some();
    let has_type = host.attr(XSI_TYPE).is_some();
    let spec = match (has_nil, has_type) {
        (true, false) => {
            let nil = claim_only_attr(host, XSI_NIL)?;
            if nil != "true" {
                return Err(format!(
                    "Value: Designer xsi:nil must be \"true\", got {nil:?} (§1.0)"
                ));
            }
            ensure_no_children(host)?;
            ValueSpec {
                kind: ValueScalarKind::Undefined,
                scalar: None,
            }
        }
        (false, true) => {
            let xsi = claim_only_attr(host, XSI_TYPE)?;
            ensure_no_children(host)?;
            host.claim_text();
            let text = host.text.clone();
            match xsi.as_str() {
                DES_STRING => str_spec(ValueScalarKind::Str, text),
                DES_REFERENCE => str_spec(ValueScalarKind::Reference, text),
                DES_DECIMAL => str_spec(ValueScalarKind::Number, text),
                DES_DATETIME => str_spec(ValueScalarKind::Date, text),
                DES_BOOLEAN => match text.as_str() {
                    "false" => bool_spec(false),
                    "true" => bool_spec(true),
                    other => {
                        return Err(format!(
                            "Value: Designer xs:boolean must be \"true\"/\"false\", got {other:?} (§1.0)"
                        ))
                    }
                },
                DES_TYPE_DESCRIPTION => {
                    if !text.is_empty() {
                        return Err(format!(
                            "Value: Designer v8:TypeDescription must be empty (self-close), got {text:?} (§1.0)"
                        ));
                    }
                    ValueSpec { kind: ValueScalarKind::TypeDescription, scalar: None }
                }
                DES_ACCOUNT_TYPE => {
                    require_account_type_literal(&text)?;
                    str_spec(ValueScalarKind::AccountType, text)
                }
                DES_VALUE_LIST => {
                    if !text.is_empty() {
                        return Err(format!(
                            "Value: Designer xr:ValueList must be empty (self-close), got {text:?} (§1.0)"
                        ));
                    }
                    ValueSpec { kind: ValueScalarKind::ValueList, scalar: None }
                }
                other => {
                    return Err(format!(
                        "Value: unknown Designer xsi:type {other:?} (witnessed: xs:string/xs:boolean/\
                         xs:decimal/xs:dateTime/xr:DesignTimeRef/v8:TypeDescription/ent:AccountType/xr:ValueList — §1.0)"
                    ))
                }
            }
        }
        (true, true) => {
            return Err("Value: Designer host carries both xsi:nil and xsi:type (§1.0)".into())
        }
        (false, false) => {
            return Err("Value: Designer host missing xsi:nil/xsi:type marker (§1.0)".into())
        }
    };
    Ok(PropertyValue::Value(spec))
}

fn str_spec(kind: ValueScalarKind, text: String) -> ValueSpec {
    ValueSpec {
        kind,
        scalar: Some(Box::new(PropertyValue::Str(text))),
    }
}

/// EDT Str/Reference scalar decode — models the presence-of-default byte-form split.
///
/// The EMPTY scalar has TWO distinct EDT byte-forms that both mean «empty string»:
/// * self-close host `<v xsi:type="core:StringValue"/>` — the SPARSE/default form
///   (canonical `scalar = Some(Str(""))`; X-equal to Designer self-close and cf `{"S",""}`);
/// * an explicit EMPTY `<value></value>` child — the PRESENCE-OF-DEFAULT form (witnessed
///   in ERP `fillValue`), which must NOT collapse to the self-close bytes on write.
///
/// The bijection «default ⟺ omission» breaks here: the same logical value (`""`) appears
/// in two byte-dresses within ONE format. We keep them distinct in the IR by mapping the
/// explicit-empty `<value></value>` form to `scalar: None` (a byte-form marker, valid ONLY
/// with a non-`Undefined` `kind`), leaving the self-close form as the canonical
/// `Some(Str(""))`. Rationale for reusing `None` (rather than a new `ValueSpec` field):
/// it is purely additive — cf/Designer/every other `ValueSpec` construction site stays
/// byte-identical, and `{Str/Reference, None}` is an EDT-only witnessed form that never
/// appears in the X-compared corpus (SSL routes `<value></value>` through `predefined`, not
/// this codec; ERP has no cf/Designer pair), so §1.6 cross-format equality is untouched.
fn decode_edt_str_ref(kind: ValueScalarKind, host: &Element) -> Result<ValueSpec, String> {
    Ok(match edt_opt_value_child(host)? {
        // Present but EMPTY `<value></value>` ⇒ presence-of-default marker (`scalar: None`).
        Some(t) if t.is_empty() => ValueSpec { kind, scalar: None },
        // Present non-empty `<value>t</value>` ⇒ the string itself.
        Some(t) => str_spec(kind, t),
        // Self-close host ⇒ canonical empty string (Some(Str(""))).
        None => str_spec(kind, String::new()),
    })
}

fn bool_spec(b: bool) -> ValueSpec {
    ValueSpec {
        kind: ValueScalarKind::Bool,
        scalar: Some(Box::new(PropertyValue::Bool(b))),
    }
}

/// Забрать единственный xsi-атрибут `name`, claim его, ошибиться на любом другом (§1.0).
fn claim_only_attr(host: &Element, name: &str) -> Result<String, String> {
    let a = host
        .attr(name)
        .ok_or_else(|| format!("Value: host <{}> missing @{name} (§1.0)", host.local))?;
    a.claimed.set(true);
    if let Some(extra) = host.attrs.iter().find(|x| !x.claimed.get()) {
        return Err(format!(
            "Value: host <{}> has unexpected attribute {:?} (§1.0)",
            host.local, extra.name
        ));
    }
    Ok(a.value.clone())
}

fn ensure_no_children(host: &Element) -> Result<(), String> {
    if let Some(child) = host.children.first() {
        return Err(format!(
            "Value: host <{}> must have no child elements, got <{}> (§1.0)",
            host.local, child.local
        ));
    }
    Ok(())
}

/// EDT: ОПЦИОНАЛЬНЫЙ `<value>текст</value>`-ребёнок (self-close ⇒ `None`). Любой иной/
/// лишний ребёнок → ошибка (§1.0). Claim'ит ребёнка+текст.
fn edt_opt_value_child(host: &Element) -> Result<Option<String>, String> {
    let mut it = host.children.iter();
    let value_el = match it.next() {
        None => return Ok(None), // self-close
        Some(e) if e.local == "value" && e.prefix.is_empty() => e,
        Some(e) => {
            return Err(format!(
                "Value: EDT expects <value> child, got <{}> (§1.0)",
                qname(e)
            ))
        }
    };
    if !value_el.attrs.is_empty() || !value_el.children.is_empty() {
        return Err("Value: EDT <value> must be a plain text leaf (§1.0)".into());
    }
    if let Some(extra) = it.next() {
        return Err(format!(
            "Value: EDT has unexpected extra child <{}> after <value> (§1.0)",
            qname(extra)
        ));
    }
    value_el.claim_with_text();
    Ok(Some(value_el.text.clone()))
}

/// EDT: ОБЯЗАТЕЛЬНЫЙ `<value>текст</value>`-ребёнок (Number/Date всегда несут value).
fn edt_require_value_child(host: &Element, xsi: &str) -> Result<String, String> {
    edt_opt_value_child(host)?
        .ok_or_else(|| format!("Value: EDT {xsi} requires <value> child (§1.0)"))
}

fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}
