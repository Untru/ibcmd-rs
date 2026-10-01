//! ChildObjects — ИМЕНА всех объектов конфигурации (членство+порядок, НЕ рекурсия).

use super::*;

// ============================================================================
// ChildObjects — ИМЕНА всех объектов конфигурации (членство+порядок, НЕ рекурсия).
// EDT:      <commonModules>CommonModule.Имя</commonModules>  (сиблинги, kind-плюрал-тег,
//           имя с префиксом `Kind.`); Language — НЕ в ref-листах (inline <languages>).
// Designer: <ChildObjects><CommonModule>Имя</CommonModule>…<Language>Имя</Language>…
//           </ChildObjects>  (bare-имена, singular-тег = kind).
// IR: List([ List([Str(kind), Str(name)]) , … ]) в КАНОНИЧЕСКОМ порядке kind-групп.
// EDT синтезирует запись Language из inline <languages> (первой), чтобы X совпал с
// Designer (где Language — первая в ChildObjects).
// ============================================================================

/// Один kind-слот ref-листа childObjects: канонический kind (= Designer-тег) ↔ EDT
/// плюрал-тег. Таблица — СЛОВАРЬ соответствия тегов (валидация + трансляция); порядок
/// строк IR берётся из AUTHORED-порядка источника (см. `decode_child_objects_edt`).
#[derive(Debug, Clone, Copy)]
pub struct ChildKind {
    /// Канонический kind (= singular Designer-тег = EDT-префикс имени), напр.
    /// `"CommonModule"`.
    pub kind: &'static str,
    /// EDT плюрал-тег (`"commonModules"`); `""` для Language (inline-сущность, не ref-лист).
    pub edt_plural: &'static str,
}

/// Канонический порядок ВСЕХ child-kind групп Configuration (включая ERP-only виды, не
/// witness'ящиеся в SSL — они структурно поддержаны, R/X их докажет на ERP). Порядок и
/// EDT-плюралы сверены по SSL-корпусу; ERP-плюралы — по конвенции 1С (lowerCamel + plural).
/// Language — особый: EDT хранит его inline-сущностью `<languages>` (edt_plural=""), а в
/// ChildObjects-список он входит ПЕРВЫМ (kind="Language").
pub const CHILD_KIND_TABLE: &[ChildKind] = &[
    ChildKind {
        kind: "Language",
        edt_plural: "",
    },
    ChildKind {
        kind: "Subsystem",
        edt_plural: "subsystems",
    },
    ChildKind {
        kind: "StyleItem",
        edt_plural: "styleItems",
    },
    ChildKind {
        kind: "Style",
        edt_plural: "styles",
    },
    ChildKind {
        kind: "CommonPicture",
        edt_plural: "commonPictures",
    },
    ChildKind {
        kind: "SessionParameter",
        edt_plural: "sessionParameters",
    },
    ChildKind {
        kind: "Role",
        edt_plural: "roles",
    },
    ChildKind {
        kind: "CommonTemplate",
        edt_plural: "commonTemplates",
    },
    ChildKind {
        kind: "FilterCriterion",
        edt_plural: "filterCriteria",
    },
    ChildKind {
        kind: "CommonModule",
        edt_plural: "commonModules",
    },
    ChildKind {
        kind: "CommonAttribute",
        edt_plural: "commonAttributes",
    },
    ChildKind {
        kind: "ExchangePlan",
        edt_plural: "exchangePlans",
    },
    ChildKind {
        kind: "XDTOPackage",
        edt_plural: "xDTOPackages",
    },
    ChildKind {
        kind: "WebService",
        edt_plural: "webServices",
    },
    ChildKind {
        kind: "WSReference",
        edt_plural: "wsReferences",
    },
    ChildKind {
        kind: "HTTPService",
        edt_plural: "httpServices",
    },
    ChildKind {
        kind: "EventSubscription",
        edt_plural: "eventSubscriptions",
    },
    ChildKind {
        kind: "ScheduledJob",
        edt_plural: "scheduledJobs",
    },
    ChildKind {
        kind: "SettingsStorage",
        edt_plural: "settingsStorages",
    },
    ChildKind {
        kind: "FunctionalOption",
        edt_plural: "functionalOptions",
    },
    ChildKind {
        kind: "FunctionalOptionsParameter",
        edt_plural: "functionalOptionsParameters",
    },
    ChildKind {
        kind: "DefinedType",
        edt_plural: "definedTypes",
    },
    ChildKind {
        kind: "CommonCommand",
        edt_plural: "commonCommands",
    },
    ChildKind {
        kind: "CommandGroup",
        edt_plural: "commandGroups",
    },
    ChildKind {
        kind: "Constant",
        edt_plural: "constants",
    },
    ChildKind {
        kind: "CommonForm",
        edt_plural: "commonForms",
    },
    ChildKind {
        kind: "Catalog",
        edt_plural: "catalogs",
    },
    ChildKind {
        kind: "Document",
        edt_plural: "documents",
    },
    ChildKind {
        kind: "DocumentNumerator",
        edt_plural: "documentNumerators",
    },
    ChildKind {
        kind: "Sequence",
        edt_plural: "sequences",
    },
    ChildKind {
        kind: "DocumentJournal",
        edt_plural: "documentJournals",
    },
    ChildKind {
        kind: "Enum",
        edt_plural: "enums",
    },
    ChildKind {
        kind: "Report",
        edt_plural: "reports",
    },
    ChildKind {
        kind: "DataProcessor",
        edt_plural: "dataProcessors",
    },
    ChildKind {
        kind: "InformationRegister",
        edt_plural: "informationRegisters",
    },
    ChildKind {
        kind: "AccumulationRegister",
        edt_plural: "accumulationRegisters",
    },
    ChildKind {
        kind: "AccountingRegister",
        edt_plural: "accountingRegisters",
    },
    ChildKind {
        kind: "CalculationRegister",
        edt_plural: "calculationRegisters",
    },
    ChildKind {
        kind: "ChartOfCharacteristicTypes",
        edt_plural: "chartsOfCharacteristicTypes",
    },
    ChildKind {
        kind: "ChartOfAccounts",
        edt_plural: "chartsOfAccounts",
    },
    ChildKind {
        kind: "ChartOfCalculationTypes",
        edt_plural: "chartsOfCalculationTypes",
    },
    ChildKind {
        kind: "BusinessProcess",
        edt_plural: "businessProcesses",
    },
    ChildKind {
        kind: "Task",
        edt_plural: "tasks",
    },
    ChildKind {
        kind: "ExternalDataSource",
        edt_plural: "externalDataSources",
    },
    ChildKind {
        kind: "IntegrationService",
        edt_plural: "integrationServices",
    },
    // Bot + WebSocketClient — новые лист-виды (coverage/s9_new). EDT `.mdo` эмитит их
    // ref-листы в порядке `<bots>` затем `<webSocketClients>` (сверено Configuration.mdo),
    // поэтому Bot ПЕРЕД WebSocketClient в каноническом порядке (EDT-R byte-exact).
    ChildKind {
        kind: "Bot",
        edt_plural: "bots",
    },
    ChildKind {
        kind: "WebSocketClient",
        edt_plural: "webSocketClients",
    },
];

fn child_kind_by_edt_plural(tag: &str) -> Option<&'static ChildKind> {
    CHILD_KIND_TABLE
        .iter()
        .find(|k| !k.edt_plural.is_empty() && k.edt_plural == tag)
}
fn child_kind_by_kind(kind: &str) -> Option<&'static ChildKind> {
    CHILD_KIND_TABLE.iter().find(|k| k.kind == kind)
}

/// Все EDT плюрал-теги (для claim/decode сканирования корня).
fn is_edt_child_list_tag(tag: &str) -> bool {
    child_kind_by_edt_plural(tag).is_some()
}

/// Имя inline-языка EDT (`<languages>/<name>` текст), если есть. Нужно childObjects-
/// кодеку EDT для синтеза записи Language (первой в каноническом списке).
fn edt_language_name(root: &Element) -> Option<String> {
    root.children
        .iter()
        .find(|c| c.local == "languages" && c.prefix.is_empty())
        .and_then(|el| el.child("name"))
        .map(|n| n.text.clone())
}

/// Декодировать ChildObjects (по диалекту). EDT — синтезирует Language из inline
/// `<languages>` (его имя), вставляя её ПЕРВОЙ в каноническом порядке.
pub fn decode_child_objects(dialect: ConfigDialect, root: &Element) -> Decoded {
    match dialect {
        ConfigDialect::Edt => {
            let lang = edt_language_name(root);
            decode_child_objects_edt(root, lang.as_deref())
        }
        ConfigDialect::Designer => decode_child_objects_designer(root),
    }
}

/// Claim ChildObjects (то же, что читает decode). EDT не клеймит inline `<languages>`
/// (его клеймит `languages`-кодек); клеймит лишь ref-листы.
pub fn claim_child_objects(dialect: ConfigDialect, root: &Element) {
    match dialect {
        ConfigDialect::Edt => {
            for ch in &root.children {
                if ch.prefix.is_empty() && is_edt_child_list_tag(&ch.local) {
                    ch.claim_with_text();
                }
            }
        }
        ConfigDialect::Designer => {
            let _ = decode_child_objects_designer(root);
        }
    }
}

fn decode_child_objects_edt(root: &Element, lang_name: Option<&str>) -> Decoded {
    // AUTHORED-порядок документа, НЕ регруппировка по слотам таблицы: платформа эмитит
    // ref-листы EDT и Designer-<ChildObjects> ОДНИМ обходом модели, поэтому физический
    // порядок обоих диалектов СОВПАДАЕТ (сверено: все 15 coverage-стадий + SSL, 2660
    // строк; ЕДИНСТВЕННОЕ расхождение — Bot/WebSocketClient в s9, где диалекты реально
    // разнонаправлены). Прежняя регруппировка по CHILD_KIND_TABLE ПЕРЕУПОРЯДОЧИВАЛА
    // interleave-группы (s2/s15: планы видов расчёта/счетов вперемешку с регистрами) и
    // ломала edt→designer byte-exactness корня. Language (inline `<languages>`, не
    // ref-лист) синтезируется ПЕРВОЙ — её позиция в Designer-ChildObjects (witnessed).
    let mut rows: Vec<Vec<String>> = Vec::new();
    if let Some(ln) = lang_name {
        rows.push(vec!["Language".to_string(), ln.to_string()]);
    }
    for ch in &root.children {
        if !ch.prefix.is_empty() {
            continue;
        }
        let Some(ck) = child_kind_by_edt_plural(&ch.local) else {
            continue;
        };
        if !ch.attrs.is_empty() || !ch.children.is_empty() {
            return Decoded::Error(format!(
                "<{}> child ref must be a plain text leaf (§1.0)",
                ch.local
            ));
        }
        // EDT-текст: `Kind.Name` → снять префикс `Kind.`.
        let prefix = format!("{}.", ck.kind);
        let name = match ch.text.strip_prefix(&prefix) {
            Some(n) => n.to_string(),
            None => {
                return Decoded::Error(format!(
                    "<{}> ref {:?} must start with {:?} (§1.0)",
                    ch.local, ch.text, prefix
                ))
            }
        };
        ch.claim_with_text();
        rows.push(vec![ck.kind.to_string(), name]);
    }
    Decoded::Present(str_list(rows))
}

fn decode_child_objects_designer(root: &Element) -> Decoded {
    let Some(cfg) = designer_config_wrapper(root) else {
        return Decoded::Present(str_list(Vec::new()));
    };
    let block = match cfg.child("ChildObjects") {
        Some(b) => b,
        None => return Decoded::Present(str_list(Vec::new())),
    };
    if !block.prefix.is_empty() || !block.attrs.is_empty() || !block.text.is_empty() {
        return Decoded::Error(
            "<ChildObjects> must be an unprefixed attribute-less container (§1.0)".into(),
        );
    }
    block.claim();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for ch in &block.children {
        if !ch.prefix.is_empty() {
            return Decoded::Error(format!(
                "<ChildObjects> child <{}> must be unprefixed (§1.0)",
                qname(ch)
            ));
        }
        if child_kind_by_kind(&ch.local).is_none() {
            return Decoded::Error(format!(
                "<ChildObjects>: unknown child kind <{}> (§1.0)",
                ch.local
            ));
        }
        if !ch.attrs.is_empty() || !ch.children.is_empty() {
            return Decoded::Error(format!(
                "<{}> child ref must be a bare name leaf (§1.0)",
                ch.local
            ));
        }
        ch.claim_with_text();
        rows.push(vec![ch.local.clone(), ch.text.clone()]);
    }
    Decoded::Present(str_list(rows))
}

/// ЕДИНСТВЕННАЯ витнесснутая пара видов, которую диалекты упорядочивают ПО-РАЗНОМУ:
/// EDT-дамп кладёт `bots` ПЕРЕД `webSocketClients`, Designer-дамп — `WebSocketClient` перед
/// `Bot` (витнесс `.fixtures/coverage/{edt,designer}/s9_new`; прочие 2660 строк состава на
/// 15 стадиях + SSL у диалектов СОВПАДАЮТ — см. `decode_child_objects_edt`).
///
/// IR несёт АВТОРСКИЙ порядок источника, поэтому при записи в ЧУЖОЙ диалект эта пара должна
/// быть переставлена — иначе Designer-источник протекал бы своим порядком в EDT-дамп (видно
/// побайтным диффом; платформе это безразлично, `cf_compare` пары не различает).
const EDT_DIVERGENT_PAIR: (&str, &str) = ("Bot", "WebSocketClient");

/// Переставить витнесснутую расходящуюся пару в EDT-порядок: непрерывный прогон `Bot` ставится
/// ПЕРЕД непрерывным прогоном `WebSocketClient`, если источник (Designer) дал их наоборот.
///
/// Осознанно ТОЧЕЧНО, а не «пересортировать состав по канону видов»: общая перегруппировка
/// ломает interleave-группы реальных дампов (s2/s15 — планы видов расчёта/счетов вперемешку с
/// регистрами) и уже однажды уронила byte-exactness корня (см. `decode_child_objects_edt`).
fn edt_swap_divergent_pair(rows: Vec<Vec<&str>>) -> Vec<Vec<&str>> {
    let (first, second) = EDT_DIVERGENT_PAIR;
    let pos = |kind: &str| -> Option<(usize, usize)> {
        let start = rows.iter().position(|r| r[0] == kind)?;
        let end = rows.iter().rposition(|r| r[0] == kind)?;
        // Прогон обязан быть НЕПРЕРЫВНЫМ — иначе это не витнесснутая форма, и перестановка
        // была бы догадкой: оставляем как есть (§1.0).
        rows[start..=end].iter().all(|r| r[0] == kind).then_some((start, end))
    };
    let (Some((bs, be)), Some((ws, we))) = (pos(first), pos(second)) else {
        return rows;
    };
    if bs < ws {
        return rows; // уже EDT-порядок (источник — EDT).
    }
    // Designer-порядок: …, WebSocketClient-прогон, Bot-прогон, … — меняем прогоны местами.
    if we + 1 != bs {
        return rows; // между прогонами что-то есть — форма не витнесснута, не трогаем.
    }
    let mut out: Vec<Vec<&str>> = Vec::with_capacity(rows.len());
    out.extend_from_slice(&rows[..ws]);
    out.extend_from_slice(&rows[bs..=be]);
    out.extend_from_slice(&rows[ws..=we]);
    out.extend_from_slice(&rows[be + 1..]);
    out
}

/// Эмитировать ChildObjects (по диалекту). EDT — несколько сиблингов `<plural>Kind.Name`
/// (БЕЗ Language: её эмитит inline `<languages>`; витнесснутая пара Bot/WebSocketClient
/// приводится к EDT-порядку — см. [`edt_swap_divergent_pair`]). Designer — один
/// `<ChildObjects>` со ВСЕМИ записями (включая Language) в порядке IR.
pub fn emit_child_objects(
    dialect: ConfigDialect,
    value: &PropertyValue,
) -> Result<Vec<OutElement>, String> {
    let rows = unpack_rows(value)?;
    for r in &rows {
        if r.len() != 2 {
            return Err(format!(
                "ChildObjects row must be [kind, name], got {} cells",
                r.len()
            ));
        }
    }
    match dialect {
        ConfigDialect::Edt => {
            let rows = edt_swap_divergent_pair(rows);
            let mut out = Vec::new();
            for r in rows {
                let (kind, name) = (r[0], r[1]);
                if kind == "Language" {
                    continue; // Language эмитится inline-сущностью <languages>, не ref-листом.
                }
                let ck = child_kind_by_kind(kind)
                    .ok_or_else(|| format!("ChildObjects: unknown kind {kind:?}"))?;
                if ck.edt_plural.is_empty() {
                    return Err(format!(
                        "ChildObjects: kind {kind:?} has no EDT ref-list tag"
                    ));
                }
                out.push(OutElement::leaf(
                    "",
                    ck.edt_plural,
                    format!("{kind}.{name}"),
                ));
            }
            Ok(out)
        }
        ConfigDialect::Designer => {
            if rows.is_empty() {
                return Ok(Vec::new());
            }
            let mut block = OutElement::branch("", "ChildObjects");
            for r in rows {
                let (kind, name) = (r[0], r[1]);
                if child_kind_by_kind(kind).is_none() {
                    return Err(format!("ChildObjects: unknown kind {kind:?}"));
                }
                block.push(OutElement::leaf("", kind, name.to_string()));
            }
            Ok(vec![block])
        }
    }
}

#[cfg(any())]
mod divergent_pair_tests {
    use super::*;

    fn rows<'a>(pairs: &[(&'a str, &'a str)]) -> Vec<Vec<&'a str>> {
        pairs.iter().map(|(k, n)| vec![*k, *n]).collect()
    }

    /// Designer-порядок пары (WebSocketClient → Bot) приводится к EDT-порядку; всё вокруг
    /// остаётся НА МЕСТЕ (порядок прочих видов — авторский, его не трогаем).
    #[test]
    fn designer_pair_order_is_swapped_for_edt() {
        let src = rows(&[
            ("Language", "Русский"),
            ("WebSocketClient", "W1"),
            ("WebSocketClient", "W2"),
            ("Bot", "B1"),
            ("Bot", "B2"),
            ("Catalog", "C1"),
        ]);
        let got = edt_swap_divergent_pair(src);
        let kinds: Vec<&str> = got.iter().map(|r| r[0]).collect();
        assert_eq!(
            kinds,
            vec!["Language", "Bot", "Bot", "WebSocketClient", "WebSocketClient", "Catalog"]
        );
        // Порядок ВНУТРИ прогона сохраняется.
        assert_eq!(got[1][1], "B1");
        assert_eq!(got[3][1], "W1");
    }

    /// EDT-источник уже в своём порядке — перестановки НЕ происходит (идемпотентность).
    #[test]
    fn edt_pair_order_is_left_alone() {
        let src = rows(&[("Bot", "B1"), ("WebSocketClient", "W1")]);
        let got = edt_swap_divergent_pair(src.clone());
        assert_eq!(got, src);
        // И повторное применение ничего не меняет.
        assert_eq!(edt_swap_divergent_pair(got.clone()), got);
    }

    /// Не-витнесснутая форма (прогоны не смежны / вид один) — НЕ трогаем (§1.0: не догадываемся).
    #[test]
    fn unwitnessed_shapes_are_untouched() {
        let split = rows(&[
            ("WebSocketClient", "W1"),
            ("Catalog", "C1"),
            ("Bot", "B1"),
        ]);
        assert_eq!(edt_swap_divergent_pair(split.clone()), split);
        let only_bots = rows(&[("Bot", "B1"), ("Catalog", "C1")]);
        assert_eq!(edt_swap_divergent_pair(only_bots.clone()), only_bots);
    }
}
