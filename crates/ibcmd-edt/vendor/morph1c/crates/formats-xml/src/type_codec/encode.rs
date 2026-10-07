//! ENCODE host-элемента из TypeSpec (byte-exact, оба диалекта).

use super::*;

// --- ENCODE -----------------------------------------------------------------

/// Восстановить host-элемент `<type>`/`<Type>` из канонического [`TypeSpec`] (§4.3).
/// Пустой набор → self-closing host; иначе — `<types>`/`<v8:Type>` в ПОРЯДКЕ, затем
/// квалификатор-блоки в порядке `number,string,date,binary` для квалифицированных
/// компонент. EDT sparse (дефолт-листы опущены) / Designer dense (все поля всегда).
///
/// Envelope хост-документа считается БЕЗ ns-объявлений из [`INLINE_NS_QNAMES`] (инлайн-ns
/// эмитятся всегда) — поведение Designer-ДЕСКРИПТОРОВ. Для документов, чей envelope
/// объявляет часть этих ns (Designer-ФОРМА объявляет `xmlns:dcsset` в корне), —
/// [`encode_scoped`].
pub fn encode(
    dialect: TypeDialect,
    host_prefix: &str,
    host_local: &str,
    spec: &TypeSpec,
) -> Result<OutElement, String> {
    encode_scoped(dialect, host_prefix, host_local, spec, &[])
}

/// [`encode`] с учётом ns-объявлений ENVELOPE хост-документа: инлайн-ns на `<v8:Type>`
/// (см. [`INLINE_NS_QNAMES`]) эмитится ТОЛЬКО если envelope НЕ объявляет ровно эту пару
/// `префикс→URI`. Модель — стандартная XML-областная видимость: 1С ставит инлайн-объявление
/// в точности когда префикс не в скоупе. Сверено: Designer-ФОРМА (корень объявляет
/// `xmlns:dcsset`) хостит `<v8:Type>dcsset:SettingsComposer` БЕЗ инлайна (SSL
/// ФормаНастроекОтчета; 0 инлайн-вхождений по Form.xml-корпусу), а `mxl:`/`fd:`/`d7p1:`
/// (НЕ в envelope формы) — С инлайном; Designer-ДЕСКРИПТОРЫ (envelope без `dcsset`) —
/// С инлайном (DataProcessor-корпус 2/2).
pub fn encode_scoped(
    dialect: TypeDialect,
    host_prefix: &str,
    host_local: &str,
    spec: &TypeSpec,
    envelope_ns: &[(&str, &str)],
) -> Result<OutElement, String> {
    encode_scoped_auto_cfg(dialect, host_prefix, host_local, spec, envelope_ns, None)
}

/// [`encode_scoped`] для документа, чей корень-envelope НЕ объявляет `cfg` (ns ссылочных
/// типов текущей конфигурации). `auto_cfg_depth = Some(n)` ⇒ ССЫЛОЧНЫЕ типы пишутся
/// АВТО-префиксом `d{n}p1:` + инлайн `xmlns:d{n}p1=`[`CURRENT_CONFIG_NS`] на самом
/// `<v8:Type>`/`<v8:TypeSet>` (n = 1-based глубина host-элемента `<v8:Type>` в документе —
/// сериализатор 1С нумерует авто-префиксы позицией). `None` (все прочие вызовы) — прежнее
/// поведение (`cfg:` из envelope, без инлайна).
///
/// Витнесс: Designer-сайдкар `Ext/Predefined.xml` (CCT `ОбъектыАдресацииЗадач`) —
/// `<v8:Type xmlns:d4p1="…current-config">d4p1:ChartOfCharacteristicTypesRef.Имя</v8:Type>`
/// на глубине 4 (PredefinedData>Item>Type>v8:Type).
pub fn encode_scoped_auto_cfg(
    dialect: TypeDialect,
    host_prefix: &str,
    host_local: &str,
    spec: &TypeSpec,
    envelope_ns: &[(&str, &str)],
    auto_cfg_depth: Option<usize>,
) -> Result<OutElement, String> {
    if spec.parts.is_empty() {
        return Ok(OutElement::self_closing(host_prefix, host_local));
    }
    let mut host = OutElement::branch(host_prefix, host_local);

    // 1) компоненты в порядке. EDT хостит всё как `<types>`; Designer различает
    //    `<v8:Type>` (конкретный тип) и `<v8:TypeSet>` (type-set — bare ref / DefinedType
    //    / Characteristic). Решение детерминировано из canon id ([`is_type_set`]).
    for tr in &spec.parts {
        // Авто-префиксный Designer-тип ([`AUTO_NS_TYPES`]): префикс `d{N}p1`, N = глубина
        // host-элемента `<v8:Type>` в документе. ФОРМЫ хостят value-type реквизита на глубине 5
        // (Form>Attributes>Attribute>Type>v8:Type — envelope_ns непуст, DESIGNER_FORM_NS),
        // дескрипторы — 7 (envelope_ns пуст). Инлайн-ns эмитится ВСЕГДА (uri вне envelope).
        if matches!(dialect, TypeDialect::Designer) {
            if let Some((_, local, uri)) = AUTO_NS_TYPES.iter().find(|(c, _, _)| *c == tr.id) {
                let n = if envelope_ns.is_empty() { 7 } else { 5 };
                let prefix = format!("d{n}p1");
                let comp = OutElement::leaf("v8", "Type", format!("{prefix}:{local}"))
                    .attr(format!("xmlns:{prefix}"), *uri);
                host.push(comp);
                continue;
            }
            // Сырой guid (`v8:TypeId`) — ссылка на ОпределяемыйТип по containerType-typeId.
            if is_type_uuid(&tr.id) {
                host.push(OutElement::leaf("v8", "TypeId", tr.id.clone()));
                continue;
            }
            // Документ без `cfg` в envelope: ссылочный тип → авто-префикс + инлайн-ns.
            if let Some(n) = auto_cfg_depth {
                if is_known_ref(&tr.id) {
                    let prefix = format!("d{n}p1");
                    let local = if is_type_set(&tr.id) {
                        "TypeSet"
                    } else {
                        "Type"
                    };
                    let comp = OutElement::leaf("v8", local, format!("{prefix}:{}", tr.id))
                        .attr(format!("xmlns:{prefix}"), CURRENT_CONFIG_NS);
                    host.push(comp);
                    continue;
                }
            }
        }
        let (type_prefix, type_local, id_text) = match dialect {
            TypeDialect::Edt => {
                if !edt_id_known(&tr.id) {
                    return Err(format!(
                        "Type: unknown canon id {:?} for EDT emit (§1.0)",
                        tr.id
                    ));
                }
                ("", "types", tr.id.clone())
            }
            TypeDialect::Designer => {
                let qname = canon_to_qname(&tr.id).ok_or_else(|| {
                    format!(
                        "Type: unknown canon id {:?} for Designer emit (§1.0)",
                        tr.id
                    )
                })?;
                let local = if is_type_set(&tr.id) {
                    "TypeSet"
                } else {
                    "Type"
                };
                ("v8", local, qname)
            }
        };
        // Инлайн-ns на компоненте (Designer `<v8:Type xmlns:dcsset=…>dcsset:SettingsComposer`):
        // восстанавливаем объявление byte-exact — но ТОЛЬКО если envelope хост-документа
        // не объявляет этот префикс (XML-скоуп: объявленный в корне префикс инлайн не дублируется).
        let mut comp = OutElement::leaf(type_prefix, type_local, id_text.clone());
        if matches!(dialect, TypeDialect::Designer) {
            if let Some((ns_attr, ns_uri)) = inline_ns_for_qname(&id_text) {
                let declared = envelope_ns
                    .iter()
                    .any(|(a, u)| *a == ns_attr && *u == ns_uri);
                if !declared {
                    comp = comp.attr(ns_attr, ns_uri);
                }
            }
        }
        host.push(comp);
    }

    // 2) квалификатор-блоки ПОСЛЕ всех компонент, в ФИКСИРОВАННОМ порядке number,string,
    //    date (сверено корпусом: 1С эмитит блоки в этом порядке НЕЗАВИСИМО от порядка
    //    объявления типов в наборе — напр. набор [Boolean,String,Date,Number,AnyRef] даёт
    //    numberQualifiers,stringQualifiers,dateQualifiers). Сначала валидируем все, затем
    //    эмитим по канон-рангу вида квалификатора.
    let mut quals: Vec<(u8, &TypeQualifier)> = Vec::new();
    for tr in &spec.parts {
        if let Some(q) = &tr.qualifier {
            verify_qualifier_matches(&tr.id, q)?;
            let rank = match q {
                TypeQualifier::Number { .. } => 0u8,
                TypeQualifier::String { .. } => 1,
                TypeQualifier::Date { .. } => 2,
                TypeQualifier::Binary { .. } => 3,
            };
            quals.push((rank, q));
        }
    }
    quals.sort_by_key(|(r, _)| *r);
    for (_, q) in quals {
        host.push(encode_qualifier(dialect, q)?);
    }
    Ok(host)
}

/// §1.0: квалификатор обязан соответствовать примитиву своей компоненты.
fn verify_qualifier_matches(id: &str, q: &TypeQualifier) -> Result<(), String> {
    let ok = matches!(
        (id, q),
        ("String", TypeQualifier::String { .. })
            | ("Number", TypeQualifier::Number { .. })
            | ("Date", TypeQualifier::Date { .. })
    );
    if ok {
        Ok(())
    } else {
        Err(format!(
            "Type: qualifier {q:?} does not match component id {id:?} (§1.0)"
        ))
    }
}

/// Эмитить один квалификатор-блок (sparse для EDT, dense для Designer).
fn encode_qualifier(dialect: TypeDialect, q: &TypeQualifier) -> Result<OutElement, String> {
    match dialect {
        TypeDialect::Edt => encode_qualifier_edt(q),
        TypeDialect::Designer => encode_qualifier_designer(q),
    }
}

/// EDT SPARSE: дефолтные листы опущены; пустой блок самозакрывается.
fn encode_qualifier_edt(q: &TypeQualifier) -> Result<OutElement, String> {
    match q {
        TypeQualifier::Number {
            precision,
            scale,
            nonnegative,
        } => {
            let mut leaves = Vec::new();
            if *precision != 0 {
                leaves.push(OutElement::leaf("", "precision", precision.to_string()));
            }
            if *scale != 0 {
                leaves.push(OutElement::leaf("", "scale", scale.to_string()));
            }
            if *nonnegative {
                leaves.push(OutElement::leaf("", "nonNegative", "true"));
            }
            Ok(block_or_self_closing("", "numberQualifiers", leaves))
        }
        TypeQualifier::String { length, fixed } => {
            let mut leaves = Vec::new();
            if *length != 0 {
                leaves.push(OutElement::leaf("", "length", length.to_string()));
            }
            if *fixed {
                leaves.push(OutElement::leaf("", "fixed", "true"));
            }
            Ok(block_or_self_closing("", "stringQualifiers", leaves))
        }
        TypeQualifier::Date { fractions } => {
            let mut leaves = Vec::new();
            if let Some(lit) = date_fractions_non_default(*fractions) {
                leaves.push(OutElement::leaf("", "dateFractions", lit));
            }
            Ok(block_or_self_closing("", "dateQualifiers", leaves))
        }
        TypeQualifier::Binary { .. } => Err(
            "Type: BinaryDataQualifiers EDT encoding is UNKNOWN (not exercised by SSL — §6 \
             UNKNOWNS); refusing to guess (§1.0)"
                .into(),
        ),
    }
}

/// Designer DENSE: ВСЕ поля квалификатора эмитятся всегда (даже на дефолте).
fn encode_qualifier_designer(q: &TypeQualifier) -> Result<OutElement, String> {
    match q {
        TypeQualifier::Number {
            precision,
            scale,
            nonnegative,
        } => {
            let mut b = OutElement::branch("v8", "NumberQualifiers");
            b.push(OutElement::leaf("v8", "Digits", precision.to_string()));
            b.push(OutElement::leaf("v8", "FractionDigits", scale.to_string()));
            b.push(OutElement::leaf(
                "v8",
                "AllowedSign",
                if *nonnegative { "Nonnegative" } else { "Any" },
            ));
            Ok(b)
        }
        TypeQualifier::String { length, fixed } => {
            let mut b = OutElement::branch("v8", "StringQualifiers");
            b.push(OutElement::leaf("v8", "Length", length.to_string()));
            b.push(OutElement::leaf(
                "v8",
                "AllowedLength",
                if *fixed { "Fixed" } else { "Variable" },
            ));
            Ok(b)
        }
        TypeQualifier::Date { fractions } => {
            let mut b = OutElement::branch("v8", "DateQualifiers");
            b.push(OutElement::leaf(
                "v8",
                "DateFractions",
                date_fractions_literal(*fractions),
            ));
            Ok(b)
        }
        TypeQualifier::Binary { .. } => Err(
            "Type: BinaryDataQualifiers Designer encoding is UNKNOWN (not exercised by SSL — §6 \
             UNKNOWNS); refusing to guess (§1.0)"
                .into(),
        ),
    }
}

/// Непустой блок → branch с листьями; пустой (все листы дефолтны) → self-closing.
fn block_or_self_closing(prefix: &str, local: &str, leaves: Vec<OutElement>) -> OutElement {
    if leaves.is_empty() {
        OutElement::self_closing(prefix, local)
    } else {
        let mut b = OutElement::branch(prefix, local);
        for l in leaves {
            b.push(l);
        }
        b
    }
}

/// Литерал DateFractions для dense-эмиссии.
fn date_fractions_literal(f: DateFractions) -> &'static str {
    match f {
        DateFractions::DateTime => "DateTime",
        DateFractions::Date => "Date",
        DateFractions::Time => "Time",
    }
}

/// Не-дефолтный литерал DateFractions (`None` для DateTime — EDT его опускает).
fn date_fractions_non_default(f: DateFractions) -> Option<&'static str> {
    match f {
        DateFractions::DateTime => None,
        other => Some(date_fractions_literal(other)),
    }
}
