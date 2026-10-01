//! DECODE + CLAIM host-элемента в канонический TypeSpec (оба диалекта через TypeDialect).

use super::*;

// --- DECODE -----------------------------------------------------------------

/// Разобрать host-элемент `<type>`/`<Type>` в канонический [`TypeSpec`] (§4.3).
///
/// Строго-тотальный разбор (стиль `decode_localized_v8`): пустой host → `parts:[]`;
/// `<types>`/`<v8:Type>` → канонизированный [`TypeRef`] (порядок сохраняется);
/// квалификатор-блок → распарсить + дроп дефолт-листов + привязать 1:1 к примитиву.
/// Любой неизвестный id/элемент/значение вне домена/orphan/dup → `Err`.
///
/// Контейнер уже claimed вызывающим. Возвращает либо значение, либо строку ошибки.
pub fn decode(dialect: TypeDialect, container: &Element) -> Result<PropertyValue, String> {
    let mut parts: Vec<TypeRef> = Vec::new();
    // Имя host-элемента и его ns не хардкодим — детей различаем по тегам.
    let (type_local, type_prefix) = match dialect {
        TypeDialect::Edt => ("types", ""),
        TypeDialect::Designer => ("Type", "v8"),
    };
    let (num_block, str_block, date_block) = qualifier_block_tags(dialect);

    for child in &container.children {
        if child.local == type_local && child.prefix == type_prefix {
            // Компонента набора: канонизируем id из текста.
            child.claim_with_text();
            // Инлайн-ns-объявление на `<v8:Type>` (напр. `xmlns:dcsset` у
            // `dcsset:SettingsComposer`) — claim, иначе тотальность (§1.0) поймает его.
            if let Some((ns_attr, _)) = inline_ns_for_qname(child.text.trim()) {
                if let Some(a) = child.attr(ns_attr) {
                    a.claimed.set(true);
                }
            }
            // Авто-префиксный тип (`d{N}p1:{local}`, см. [`AUTO_NS_TYPES`]): резолвим по
            // (local, uri ИНЛАЙН-атрибута) — префикс генерируем 1С по глубине, любое N
            // валидно; uri ОБЯЗАН совпасть (иначе это другой одноимённый тип — §1.0).
            if matches!(dialect, TypeDialect::Designer) {
                if let Some((canon, uri, prefix)) = auto_ns_qname_to_canon(child.text.trim()) {
                    let a = child.attr(&format!("xmlns:{prefix}")).ok_or_else(|| {
                        format!(
                            "Type: auto-prefixed {:?} without inline xmlns:{prefix} (§1.0)",
                            child.text
                        )
                    })?;
                    if a.value != uri {
                        return Err(format!(
                            "Type: {:?} inline ns {:?}, want {uri:?} (§1.0 — different type)",
                            child.text, a.value
                        ));
                    }
                    a.claimed.set(true);
                    parts.push(TypeRef {
                        id: canon.to_string(),
                        qualifier: None,
                    });
                    continue;
                }
                // АВТО-префиксный ССЫЛОЧНЫЙ тип текущей конфигурации (`d{N}p1:CatalogRef.Имя`
                // + инлайн `xmlns:d{N}p1=CURRENT_CONFIG_NS`) — так 1С пишет ref-тип в
                // документе, чей корень НЕ объявляет `cfg` (Designer-сайдкар
                // `Ext/Predefined.xml`). Канон — тот же bare id, что у `cfg:`-ветки (§1.6:
                // оба спеллинга → ОДИН канон).
                if let Some((canon, prefix)) = auto_cfg_qname_to_canon(child.text.trim()) {
                    let a = child.attr(&format!("xmlns:{prefix}")).ok_or_else(|| {
                        format!(
                            "Type: auto-prefixed ref {:?} without inline xmlns:{prefix} (§1.0)",
                            child.text
                        )
                    })?;
                    if a.value != CURRENT_CONFIG_NS {
                        return Err(format!(
                            "Type: {:?} inline ns {:?}, want {CURRENT_CONFIG_NS:?} (§1.0 — \
                             different type)",
                            child.text, a.value
                        ));
                    }
                    a.claimed.set(true);
                    parts.push(TypeRef {
                        id: canon,
                        qualifier: None,
                    });
                    continue;
                }
            }
            let id = canonicalize_type_id(dialect, &child.text)?;
            parts.push(TypeRef {
                id,
                qualifier: None,
            });
        } else if matches!(dialect, TypeDialect::Designer)
            && child.local == "TypeId"
            && child.prefix == "v8"
        {
            // Designer `<v8:TypeId>{guid}</v8:TypeId>` — ссылка на ОпределяемыйТип по
            // containerType-typeId (EDT-твин: `<types>{guid}</types>`; witness Файлы «Владелец»
            // 89432067-… = DefinedType ВладелецФайлов, ДоступныеАнкеты f0d7bb9f-… = Респондент).
            // Канон — сам guid (dialect-нейтрален, X by construction).
            child.claim_with_text();
            let id = child.text.trim().to_string();
            if !is_type_uuid(&id) {
                return Err(format!("Type: <v8:TypeId>={id:?}, want UUID (§1.0)"));
            }
            parts.push(TypeRef {
                id,
                qualifier: None,
            });
        } else if is_designer_typeset_host(dialect, child) {
            // Designer хостит TYPE-SET-ссылку как `<v8:TypeSet>` (вместо `<v8:Type>`).
            // Канонизируется ТЕМ ЖЕ путём (cfg:-таблица) → ОДИН и тот же canon id, что
            // EDT `<types>` для этого type-set'а (X by construction). §1.0: id обязан
            // быть type-set'ом (иначе TypeSet-хост для конкретного типа — ошибка).
            child.claim_with_text();
            let id = canonicalize_type_id(dialect, &child.text)?;
            if !is_type_set(&id) {
                return Err(format!(
                    "Type: <v8:TypeSet> hosts non-type-set id {id:?} (§1.0: only bare \
                     ref-kinds / DefinedType.* / Characteristic.* are type-sets)"
                ));
            }
            parts.push(TypeRef {
                id,
                qualifier: None,
            });
        } else if child.local == num_block && child.prefix == type_prefix {
            child.claim();
            let q = decode_number_qualifier(dialect, child)?;
            attach(&mut parts, q, "Number", &child.local)?;
        } else if child.local == str_block && child.prefix == type_prefix {
            child.claim();
            let q = decode_string_qualifier(dialect, child)?;
            attach(&mut parts, q, "String", &child.local)?;
        } else if child.local == date_block && child.prefix == type_prefix {
            child.claim();
            let q = decode_date_qualifier(dialect, child)?;
            attach(&mut parts, q, "Date", &child.local)?;
        } else {
            return Err(format!(
                "Type: unexpected child <{}> inside type host (§1.0: no silent skip)",
                qname(child)
            ));
        }
    }
    Ok(PropertyValue::Type(TypeSpec { parts }))
}

/// Привязать квалификатор `q` к ЕДИНСТВЕННОЙ компоненте с id `prim_id`. orphan
/// (нет такой компоненты) / dup (уже квалифицирована) → ОШИБКА (§1.0).
fn attach(
    parts: &mut [TypeRef],
    q: TypeQualifier,
    prim_id: &str,
    block_name: &str,
) -> Result<(), String> {
    let target = parts.iter_mut().find(|p| p.id == prim_id);
    match target {
        Some(tr) if tr.qualifier.is_none() => {
            tr.qualifier = Some(q);
            Ok(())
        }
        Some(_) => Err(format!(
            "Type: duplicate <{block_name}> for {prim_id} component (§1.0)"
        )),
        None => Err(format!(
            "Type: orphan <{block_name}> — no {prim_id} component in the set (§1.0)"
        )),
    }
}

/// Канонизировать текст `<types>`/`<v8:Type>` в bare-id (§1.2). Неизвестный → ошибка.
fn canonicalize_type_id(dialect: TypeDialect, raw: &str) -> Result<String, String> {
    match dialect {
        TypeDialect::Edt => {
            if edt_id_known(raw) {
                Ok(raw.to_string())
            } else {
                Err(format!(
                    "Type: unknown EDT type-id {raw:?} (not in canon table — §1.0: no guess)"
                ))
            }
        }
        TypeDialect::Designer => qname_to_canon(raw).ok_or_else(|| {
            format!(
                "Type: unknown Designer QName {raw:?} (not in canon↔QName table — §1.0: no guess)"
            )
        }),
    }
}

/// Является ли `child` Designer-хостом TYPE-SET'а (`<v8:TypeSet>`). Только для Designer;
/// EDT хостит type-set как обычный `<types>` (отдельного тега нет → всегда `false`).
fn is_designer_typeset_host(dialect: TypeDialect, child: &Element) -> bool {
    matches!(dialect, TypeDialect::Designer) && child.local == "TypeSet" && child.prefix == "v8"
}

/// Имена тегов трёх квалификатор-блоков по диалекту: `(number, string, date)`.
fn qualifier_block_tags(dialect: TypeDialect) -> (&'static str, &'static str, &'static str) {
    match dialect {
        TypeDialect::Edt => ("numberQualifiers", "stringQualifiers", "dateQualifiers"),
        TypeDialect::Designer => ("NumberQualifiers", "StringQualifiers", "DateQualifiers"),
    }
}

/// Распарсить `u32` из текста листа (§1.0: не-число → ошибка).
fn parse_u32(s: &str, what: &str) -> Result<u32, String> {
    s.parse::<u32>()
        .map_err(|_| format!("Type: {what} not a non-negative integer: {s:?}"))
}

/// Декод numberQualifiers. EDT sparse (present-only) / Designer dense (все 3 листа).
/// §1.0: лишний/неизвестный лист → ошибка; домен AllowedSign проверяется.
fn decode_number_qualifier(dialect: TypeDialect, block: &Element) -> Result<TypeQualifier, String> {
    let (prefix, precision_tag, scale_tag, sign_tag) = match dialect {
        TypeDialect::Edt => ("", "precision", "scale", "nonNegative"),
        TypeDialect::Designer => ("v8", "Digits", "FractionDigits", "AllowedSign"),
    };
    let mut precision = 0u32;
    let mut scale = 0u32;
    let mut nonnegative = false;
    for leaf in &block.children {
        if leaf.prefix != prefix {
            return Err(format!(
                "Type: numberQualifiers child <{}> wrong ns",
                qname(leaf)
            ));
        }
        if leaf.local == precision_tag {
            leaf.claim_with_text();
            precision = parse_u32(&leaf.text, "precision")?;
        } else if leaf.local == scale_tag {
            leaf.claim_with_text();
            scale = parse_u32(&leaf.text, "scale")?;
        } else if leaf.local == sign_tag {
            leaf.claim_with_text();
            nonnegative = match dialect {
                // EDT: только `<nonNegative>true</nonNegative>` (false опущен).
                TypeDialect::Edt => match leaf.text.as_str() {
                    "true" => true,
                    other => {
                        return Err(format!(
                        "Type: EDT nonNegative must be \"true\" (false is omitted), got {other:?}"
                    ))
                    }
                },
                // Designer: `Any` | `Nonnegative` (домен §3).
                TypeDialect::Designer => match leaf.text.as_str() {
                    "Nonnegative" => true,
                    "Any" => false,
                    other => {
                        return Err(format!(
                            "Type: AllowedSign domain is {{Any,Nonnegative}}, got {other:?} (§1.0)"
                        ))
                    }
                },
            };
        } else {
            return Err(format!(
                "Type: unexpected numberQualifiers child <{}> (§1.0)",
                qname(leaf)
            ));
        }
    }
    Ok(TypeQualifier::Number {
        precision,
        scale,
        nonnegative,
    })
}

/// Декод stringQualifiers. EDT sparse / Designer dense (Length+AllowedLength).
fn decode_string_qualifier(dialect: TypeDialect, block: &Element) -> Result<TypeQualifier, String> {
    let (prefix, length_tag, allowed_tag) = match dialect {
        TypeDialect::Edt => ("", "length", "fixed"),
        TypeDialect::Designer => ("v8", "Length", "AllowedLength"),
    };
    let mut length = 0u32;
    let mut fixed = false;
    for leaf in &block.children {
        if leaf.prefix != prefix {
            return Err(format!(
                "Type: stringQualifiers child <{}> wrong ns",
                qname(leaf)
            ));
        }
        if leaf.local == length_tag {
            leaf.claim_with_text();
            length = parse_u32(&leaf.text, "length")?;
        } else if leaf.local == allowed_tag {
            leaf.claim_with_text();
            fixed = match dialect {
                TypeDialect::Edt => match leaf.text.as_str() {
                    "true" => true,
                    other => {
                        return Err(format!(
                            "Type: EDT fixed must be \"true\" (false is omitted), got {other:?}"
                        ))
                    }
                },
                TypeDialect::Designer => match leaf.text.as_str() {
                    "Fixed" => true,
                    "Variable" => false,
                    other => {
                        return Err(format!(
                            "Type: AllowedLength domain is {{Variable,Fixed}}, got {other:?} (§1.0)"
                        ))
                    }
                },
            };
        } else {
            return Err(format!(
                "Type: unexpected stringQualifiers child <{}> (§1.0)",
                qname(leaf)
            ));
        }
    }
    Ok(TypeQualifier::String { length, fixed })
}

/// Декод dateQualifiers. EDT: пустой блок = DateTime; `<dateFractions>Date|Time`.
/// Designer: всегда `<v8:DateFractions>DateTime|Date|Time`.
fn decode_date_qualifier(dialect: TypeDialect, block: &Element) -> Result<TypeQualifier, String> {
    let (prefix, frac_tag) = match dialect {
        TypeDialect::Edt => ("", "dateFractions"),
        TypeDialect::Designer => ("v8", "DateFractions"),
    };
    let mut fractions = DateFractions::DateTime;
    let mut seen = false;
    for leaf in &block.children {
        if leaf.prefix != prefix || leaf.local != frac_tag {
            return Err(format!(
                "Type: unexpected dateQualifiers child <{}> (§1.0)",
                qname(leaf)
            ));
        }
        if seen {
            return Err("Type: duplicate dateFractions (§1.0)".into());
        }
        leaf.claim_with_text();
        fractions = parse_date_fractions(&leaf.text)?;
        seen = true;
    }
    Ok(TypeQualifier::Date { fractions })
}

/// Распарсить домен dateFractions (§3): `DateTime|Date|Time`, иначе ошибка.
fn parse_date_fractions(s: &str) -> Result<DateFractions, String> {
    match s {
        "DateTime" => Ok(DateFractions::DateTime),
        "Date" => Ok(DateFractions::Date),
        "Time" => Ok(DateFractions::Time),
        other => Err(format!(
            "Type: DateFractions domain is {{DateTime,Date,Time}}, got {other:?} (§1.0)"
        )),
    }
}

// --- CLAIM (согласован с decode, §4.2 B1) -----------------------------------

/// Claim РОВНО те узлы, что прочитает [`decode`] (host уже claimed вызывающим):
/// каждый `<types>`/`<v8:Type>` (узел+текст), каждый квалификатор-блок (узел) и его
/// листья (узел+текст). Лишнее остаётся неклеймнутым → leftover поймает (§1.0).
pub fn claim(dialect: TypeDialect, container: &Element) {
    let (type_local, type_prefix) = match dialect {
        TypeDialect::Edt => ("types", ""),
        TypeDialect::Designer => ("Type", "v8"),
    };
    let (num_block, str_block, date_block) = qualifier_block_tags(dialect);
    for child in &container.children {
        if (child.local == type_local && child.prefix == type_prefix)
            || is_designer_typeset_host(dialect, child)
            || (matches!(dialect, TypeDialect::Designer)
                && child.local == "TypeId"
                && child.prefix == "v8")
        {
            // `<types>`/`<v8:Type>`/`<v8:TypeSet>`/`<v8:TypeId>` — компонента (узел+текст).
            child.claim_with_text();
            // Инлайн-ns на `<v8:Type>` (напр. `xmlns:dcsset` / авто-`d{N}p1`) — claim в тон decode.
            if let Some((ns_attr, _)) = inline_ns_for_qname(child.text.trim()) {
                if let Some(a) = child.attr(ns_attr) {
                    a.claimed.set(true);
                }
            }
            if let Some((_, _, prefix)) = auto_ns_qname_to_canon(child.text.trim()) {
                if let Some(a) = child.attr(&format!("xmlns:{prefix}")) {
                    a.claimed.set(true);
                }
            }
        } else if (child.local == num_block
            || child.local == str_block
            || child.local == date_block)
            && child.prefix == type_prefix
        {
            child.claim();
            for leaf in &child.children {
                leaf.claim_with_text();
            }
        }
        // Иное — НЕ клеймим (decode вернёт ошибку, leftover не вырастет лишним).
    }
}

/// Полное имя тега (`prefix:local` или `local`) — диагностика.
fn qname(el: &Element) -> String {
    if el.prefix.is_empty() {
        el.local.clone()
    } else {
        format!("{}:{}", el.prefix, el.local)
    }
}
