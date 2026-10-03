//! usedMobileApplicationFunctionalities — nested список функциональностей мобильного приложения.

use super::*;

// ============================================================================
// usedMobileApplicationFunctionalities — nested список. EDT (sparse) и Designer (dense)
//   РАСХОДЯТСЯ по КОДИРОВАНИЮ одного и того же значения:
// EDT:      <usedMobileApplicationFunctionalities><functionality>[<functionality>F]
//             <use>true|false</use></functionality>…</usedMobileApplicationFunctionalities>
//           — ТОЛЬКО use=true строки; имя ОПУЩЕНО для первого литерала перечисления
//             (Biometrics — EMF-дефолт).
// Designer: <UsedMobileApplicationFunctionalities><app:functionality>[<app:functionality>F]
//             <app:use>…</app:use></app:functionality>…</UsedMobileApplicationFunctionalities>
//           — ПОЛНЫЙ dense-список ВСЕХ функциональностей платформы в фиксированном
//             порядке [MOBILE_FUNCTIONALITY_TABLE] с явными use-флагами.
// КАНОН IR = EDT-sparse: List([ List([Str(имя|""=Biometrics), Bool(true)]) ]).
// Designer-decode РЕДУЦИРУЕТ dense→sparse с §1.0-самопроверкой (re-expand обязан
// воспроизвести исходные строки — иначе незасвидетельствованная форма, громкий отказ);
// designer-emit РАЗВОРАЧИВАЕТ sparse→dense по таблице. (host claimed locate'ом.)
// ============================================================================

/// Полный dense-список функциональностей мобильного приложения в порядке Designer-дампа
/// платформы (WITNESSED: 38 имён, идентичны в coverage s1..s15 (8.3.27), SSL 8.5.1 и
/// 24 factory-пробах — порядок стабилен между версиями формата 2.21).
const MOBILE_FUNCTIONALITY_TABLE: &[&str] = &[
    "Biometrics",
    "Location",
    "BackgroundLocation",
    "BluetoothPrinters",
    "WiFiPrinters",
    "Contacts",
    "Calendars",
    "PushNotifications",
    "LocalNotifications",
    "InAppPurchases",
    "PersonalComputerFileExchange",
    "Ads",
    "NumberDialing",
    "CallProcessing",
    "CallLog",
    "AutoSendSMS",
    "ReceiveSMS",
    "SMSLog",
    "Camera",
    "Microphone",
    "MusicLibrary",
    "PictureAndVideoLibraries",
    "AudioPlaybackAndVibration",
    "BackgroundAudioPlaybackAndVibration",
    "InstallPackages",
    "OSBackup",
    "ApplicationUsageStatistics",
    "BarcodeScanning",
    "BackgroundAudioRecording",
    "AllFilesAccess",
    "Videoconferences",
    "NFC",
    "DocumentScanning",
    "SpeechToText",
    "Geofences",
    "IncomingShareRequests",
    "AllIncomingShareRequestsTypesProcessing",
    "TextToSpeech",
];

/// Имя перечисления мобильных функциональностей в EDT-метамодели — ключ since-таблицы
/// литералов (`inventory/enums.jsonl`).
const MOBILE_FUNCTIONALITY_ENUM: &str = "MobileApplicationFunctionalities";

/// Таблица функциональностей, СУЩЕСТВУЮЩИХ в версии формата `version` (§1.5).
///
/// Плотная таблица — это полный набор литералов перечисления СВОЕЙ версии, а набор растёт:
/// WITNESSED (`.fixtures/versions/probes/configuration_mobile_func/`) дамп 2.17 несёт 37
/// строк, 2.20 — 38 (добавился `TextToSpeech`, since 8.3.25). Читать дамп 2.17 таблицей
/// 2.21 нельзя: лишняя строка → отказ «dense shape mismatch».
///
/// Порядок берётся из ВИТНЕСС-таблицы (порядок Designer-дампа), а гейт — из инвентаря;
/// сверка «состав таблицы == литералы перечисления» — тест `dense_table_matches_inventory`.
fn mobile_functionality_table(version: FormatVersion) -> Vec<&'static str> {
    MOBILE_FUNCTIONALITY_TABLE
        .iter()
        .copied()
        .filter(|name| literal_available_in(MOBILE_FUNCTIONALITY_ENUM, name, version))
        .collect()
}

/// Dense-строки Designer → канонический sparse (только use=true; имя первого литерала
/// таблицы канонизируется в `""` — EDT-спеллинг EMF-дефолта).
fn reduce_mobile_functionalities(rows: &[PropertyValue]) -> Result<Vec<PropertyValue>, String> {
    let mut sparse = Vec::new();
    for row in rows {
        let cells = match row {
            PropertyValue::List(c) if c.len() == 2 => c,
            _ => return Err("mobile functionality row must be List([name, use])".into()),
        };
        let name = match &cells[0] {
            PropertyValue::Str(s) => s.as_str(),
            other => {
                return Err(format!(
                    "functionality name must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        let used = match &cells[1] {
            PropertyValue::Bool(b) => *b,
            other => {
                return Err(format!(
                    "functionality use must be Bool, got {:?}",
                    other.kind()
                ))
            }
        };
        if used {
            let canon = if name == MOBILE_FUNCTIONALITY_TABLE[0] {
                ""
            } else {
                name
            };
            sparse.push(PropertyValue::List(vec![
                PropertyValue::Str(canon.to_string()),
                PropertyValue::Bool(true),
            ]));
        }
    }
    Ok(sparse)
}

/// Канонический sparse → dense-строки Designer: ВСЯ таблица ВЕРСИИ в фиксированном
/// порядке, use=true у перечисленных. Имя вне таблицы — громкий отказ (§1.0 — не тихий
/// дроп); в т.ч. функциональность, которой в ТАРГЕТ-версии ещё НЕ СУЩЕСТВУЕТ: её нельзя
/// ни эмитить, ни молча выбросить — это потеря при downgrade (см. `--v8version`).
fn expand_mobile_functionalities(
    sparse: &[PropertyValue],
    version: FormatVersion,
) -> Result<Vec<PropertyValue>, String> {
    let table = mobile_functionality_table(version);
    let mut used: Vec<bool> = vec![false; table.len()];
    for row in sparse {
        let cells = match row {
            PropertyValue::List(c) if c.len() == 2 => c,
            _ => return Err("mobile functionality row must be List([name, use])".into()),
        };
        let name = match &cells[0] {
            PropertyValue::Str(s) => s.as_str(),
            other => {
                return Err(format!(
                    "functionality name must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        let flag = match &cells[1] {
            PropertyValue::Bool(b) => *b,
            other => {
                return Err(format!(
                    "functionality use must be Bool, got {:?}",
                    other.kind()
                ))
            }
        };
        let resolved = if name.is_empty() {
            MOBILE_FUNCTIONALITY_TABLE[0]
        } else {
            name
        };
        let idx = table.iter().position(|n| *n == resolved).ok_or_else(|| {
            if MOBILE_FUNCTIONALITY_TABLE.contains(&resolved) {
                // Литерал платформе ИЗВЕСТЕН, но введён ПОЗЖЕ таргета: тихо выбросить его
                // из плотной таблицы значило бы потерять свойство при downgrade (§1.0).
                format!(
                    "mobile functionality {resolved:?} does not exist in format {version} \
                     (introduced later) — refusing to silently drop it (§1.0)"
                )
            } else {
                format!(
                    "mobile functionality {resolved:?} is not in the witnessed platform table \
                     (§1.0 — refusing to emit a Designer dense list that silently drops it)"
                )
            }
        })?;
        used[idx] = flag;
    }
    Ok(table
        .iter()
        .zip(used)
        .map(|(name, flag)| {
            PropertyValue::List(vec![
                PropertyValue::Str((*name).to_string()),
                PropertyValue::Bool(flag),
            ])
        })
        .collect())
}

/// Декодировать `<…MobileApplicationFunctionalities>` (host claimed). `app_ns` — префикс
/// внутренних тегов (`""` EDT / `"app"` Designer).
pub fn decode_mobile_functionalities(
    host: &Element,
    app_ns: &str,
    version: FormatVersion,
) -> Decoded {
    if !host.attrs.is_empty() || !host.text.is_empty() {
        return Decoded::Error(
            "MobileApplicationFunctionalities host must be attribute-less, no text".into(),
        );
    }
    let mut rows: Vec<PropertyValue> = Vec::new();
    for f in &host.children {
        if f.local != "functionality" || f.prefix != app_ns {
            return Decoded::Error(format!(
                "MobileFunctionalities: expected <functionality>, got <{}>",
                qname(f)
            ));
        }
        if !f.attrs.is_empty() || !f.text.is_empty() {
            return Decoded::Error("<functionality> must be attribute-less, no text".into());
        }
        f.claim();
        let mut name = String::new();
        let mut use_val: Option<bool> = None;
        for c in &f.children {
            if c.prefix != app_ns {
                return Decoded::Error(format!("<functionality> child <{}> wrong ns", qname(c)));
            }
            match c.local.as_str() {
                "functionality" => {
                    if !c.attrs.is_empty() || !c.children.is_empty() {
                        return Decoded::Error(
                            "<functionality>/<functionality> must be text leaf".into(),
                        );
                    }
                    c.claim_with_text();
                    name = c.text.clone();
                }
                "use" => {
                    if !c.attrs.is_empty() || !c.children.is_empty() {
                        return Decoded::Error("<functionality>/<use> must be text leaf".into());
                    }
                    c.claim_with_text();
                    use_val = Some(match c.text.as_str() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Decoded::Error(format!(
                                "<use> must be true/false, got {other:?}"
                            ))
                        }
                    });
                }
                other => {
                    return Decoded::Error(format!("<functionality>: unexpected child <{other}>"))
                }
            }
        }
        let u = match use_val {
            Some(u) => u,
            None => return Decoded::Error("<functionality> missing <use> (§1.0)".into()),
        };
        rows.push(PropertyValue::List(vec![
            PropertyValue::Str(name),
            PropertyValue::Bool(u),
        ]));
    }
    // Designer (app_ns="app"): dense → канонический sparse, с §1.0-самопроверкой —
    // обратная экспансия ОБЯЗАНА воспроизвести исходные dense-строки (полная таблица в
    // фиксированном порядке, явные имена). Иное — незасвидетельствованная форма, громко.
    if app_ns == "app" && !rows.is_empty() {
        let sparse = match reduce_mobile_functionalities(&rows) {
            Ok(s) => s,
            Err(e) => return Decoded::Error(e),
        };
        match expand_mobile_functionalities(&sparse, version) {
            Ok(re) if re == rows => return Decoded::Present(PropertyValue::List(sparse)),
            Ok(_) => {
                return Decoded::Error(
                    "Designer mobile-functionality list does not match the witnessed dense \
                     shape (full platform table, fixed order, explicit names) — refusing a \
                     lossy sparse reduction (§1.0)"
                        .into(),
                )
            }
            Err(e) => return Decoded::Error(e),
        }
    }
    Decoded::Present(PropertyValue::List(rows))
}

/// Claim `<…MobileApplicationFunctionalities>` (host claimed; claim внутренности).
pub fn claim_mobile_functionalities(host: &Element, app_ns: &str, version: FormatVersion) {
    let _ = decode_mobile_functionalities(host, app_ns, version);
}

/// Эмитировать `<…MobileApplicationFunctionalities>` (host-тег/ns — из локуса; внутренний
/// `app_ns` — `""` EDT / `"app"` Designer). Пустой → self-closing.
pub fn emit_mobile_functionalities(
    ns: &str,
    tag: &str,
    app_ns: &str,
    value: &PropertyValue,
    version: FormatVersion,
) -> Result<OutElement, String> {
    let rows = match value {
        PropertyValue::List(l) => l,
        other => {
            return Err(format!(
                "MobileFunctionalities must be List, got {:?}",
                other.kind()
            ))
        }
    };
    // Designer (app_ns="app"): канонический sparse → полный dense-список по таблице
    // (обратная операция decode-редукции; см. модульный комментарий секции).
    let expanded;
    let rows = if app_ns == "app" && !rows.is_empty() {
        expanded = expand_mobile_functionalities(rows, version)?;
        &expanded
    } else {
        rows
    };
    if rows.is_empty() {
        return Ok(OutElement::self_closing(ns, tag));
    }
    let mut host = OutElement::branch(ns, tag);
    for row in rows {
        let cells = match row {
            PropertyValue::List(c) => c,
            other => {
                return Err(format!(
                    "MobileFunctionalities row must be List, got {:?}",
                    other.kind()
                ))
            }
        };
        if cells.len() != 2 {
            return Err(format!(
                "MobileFunctionalities row must be [name, use], got {}",
                cells.len()
            ));
        }
        let name = match &cells[0] {
            PropertyValue::Str(s) => s,
            other => {
                return Err(format!(
                    "functionality name must be Str, got {:?}",
                    other.kind()
                ))
            }
        };
        let use_val = match &cells[1] {
            PropertyValue::Bool(b) => *b,
            other => {
                return Err(format!(
                    "functionality use must be Bool, got {:?}",
                    other.kind()
                ))
            }
        };
        let mut f = OutElement::branch(app_ns, "functionality");
        if !name.is_empty() {
            f.push(OutElement::leaf(app_ns, "functionality", name.clone()));
        }
        f.push(OutElement::leaf(
            app_ns,
            "use",
            if use_val { "true" } else { "false" },
        ));
        host.push(f);
    }
    Ok(host)
}
