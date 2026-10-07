//! Чтение/запись СОСТАВА АВТОНОМНОЙ (мобильной) КОНФИГУРАЦИИ — структурного конфиг-сайдкара
//! КОРНЯ (`Ext/StandaloneConfigurationContent.bin` Designer /
//! `src/Configuration/MobileApplicationContent.scc` EDT) в IR
//! [`morph1c_core::ir::StandaloneContent`]. Сосед [`crate::ext_read`] (тот вызывает attach/write
//! отсюда на своём конфиг-Ext-проходе) и [`crate::flowchart_read`] (тот же приём §1.0-
//! самопроверки: пере-сериализация канона ОБЯЗАНА воспроизвести исходные байты).
//!
//! # Диалекты (витнесс ERP — единственный носитель корпуса; SSL/coverage файла НЕ имеют)
//! В отличие от прочих xml-сайдкаров диалекты здесь СТРУКТУРНО РАЗНЫЕ (не «минус BOM/version»):
//! * **Designer** — BOM + пролог + элементный
//!   `<StandaloneContent xmlns="http://v8.1c.ru/8.3/xcf/extrnprops" … version="2.20">`:
//!   `<UsedItem><Metadata>Кind.Name</Metadata></UsedItem>`… `<UnusedItem>`… `<PriorityItem>`
//!   (+`<Priority>`)… `<DataExchangeSettings>` с 4 полями (`InactiveNodesCleanupTimeout` —
//!   ВНУТРИ настроек), разметка `\t`+CRLF, БЕЗ завершающего перевода строки.
//! * **EDT** — БЕЗ BOM, атрибутный `<scc:StandaloneContent xmlns:scc="http://g5.1c.ru/v8/dt/scc">`:
//!   `<usedItem metadata="…"/>`… `<unusedItem…/>`… `<priorityItem metadata="…" priority="…"/>`…
//!   `<dataExchangeSettings exchangeOnChangeData="…" exchangePeriod="…" transactionCount="…"/>`,
//!   отступ 2 пробела, `InactiveNodesCleanupTimeout`-поля НЕТ (witnessed-значение 0 = дефолт),
//!   С завершающим CRLF после корня.
//!
//! Канон ОДИН (§1.6, сверено ERP: 1692 used + 596 unused + 3 priority + настройки — списки и
//! ПОРЯДОК байт-в-байт эквивалентны между диалектами). Порядок IR = документный порядок XML
//! (о его связи с порядком эталонного cf-тела — док [`morph1c_core::ir::StandaloneContent`]).
//!
//! # §1.0
//! * пере-сериализация в диалект источника ОБЯЗАНА воспроизвести исходные байты (иначе файл
//!   несёт не смоделированную кодеком байт-форму — громкий отказ, не тихая нормализация);
//! * Designer-версия корня — witnessed-реестр ([`crate::sidecar_version`]), запись — версией
//!   амбьентного round-trip-таргета;
//! * EDT-запись ненулевого `inactive_nodes_cleanup_timeout` — типизированный отказ (кодировка
//!   не witnessed: у EDT-диалекта поля нет вовсе).

use std::path::Path;

use formats_xml::registry::Format;
use morph1c_core::ir::{MetadataObject, StandaloneContent, StandalonePriority};
use morph1c_core::version::FormatVersion;

use crate::ConvertError;

/// Имя Designer-сайдкара (в корневом `Ext/`).
pub(crate) const DESIGNER_FILE: &str = "StandaloneConfigurationContent.bin";
/// Имя EDT-сайдкара (сиблинг `Configuration.mdo`; ⚠️ имя ДРУГОЕ — `MobileApplicationContent`).
pub(crate) const EDT_FILE: &str = "MobileApplicationContent.scc";
/// UTF-8 BOM — Designer-файл его несёт, EDT нет (witnessed ERP).
const BOM: &[u8] = &[0xEF, 0xBB, 0xBF];

/// Designer: пролог + голова корневого тега ДО значения `version="…"` (witnessed ERP —
/// порядок атрибутов фиксирован, `version` последний).
const DESIGNER_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<StandaloneContent \
                             xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
                             xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
                             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"";
/// EDT: пролог + корневой тег целиком (версии нет — witnessed ERP).
const EDT_HEAD: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<scc:StandaloneContent \
                        xmlns:scc=\"http://g5.1c.ru/v8/dt/scc\">\r\n";

/// Подгрузить состав автономной конфигурации (если сайдкар существует) в
/// `obj.standalone_content`. Зовётся из [`crate::ext_read::attach_config_ext`] с уже
/// вычисленным конфиг-Ext-каталогом (`ext_dir`); отсутствие файла — честное отсутствие
/// (SSL/coverage), no-op. §1.0: файл, чья пере-сериализация не воспроизводит исходные
/// байты → громкий отказ.
pub(crate) fn attach_standalone_content(
    format: Format,
    ext_dir: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    let file = match format {
        Format::Designer => DESIGNER_FILE,
        Format::Edt => EDT_FILE,
        Format::Cf => return Ok(()), // контейнер — не файловый сайдкар.
    };
    let path = ext_dir.join(file);
    if !path.is_file() {
        return Ok(()); // нет файла → честно нет состава (SSL/coverage).
    }
    let bytes = std::fs::read(&path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: "Configuration".to_string(),
        object: obj.name.clone(),
        reason: format!("standalone content {}: {reason}", path.display()),
    };
    let (sc, src_version) = match format {
        Format::Designer => {
            let (sc, v) = parse_designer(&bytes).map_err(&read_err)?;
            (sc, Some(v))
        }
        Format::Edt => (parse_edt(&bytes).map_err(&read_err)?, None),
        Format::Cf => unreachable!("cf returned above"),
    };
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (Designer —
    // ВЕРСИЕЙ ИСТОЧНИКА: версия — свойство файла, §1.6).
    let back = match (format, src_version) {
        (Format::Designer, Some(v)) => serialize_designer(&sc, v),
        _ => serialize_edt(&sc).map_err(&read_err)?,
    };
    if back != bytes {
        return Err(read_err(
            "does not round-trip byte-exactly through the IR (the sidecar carries a byte-shape \
             this codec does not model — refusing to silently normalize it, §1.0)"
                .into(),
        ));
    }
    if obj.standalone_content.is_some() {
        return Err(read_err(
            "object already carries standalone content before the sidecar attach (unexpected — \
             the descriptor projection must not populate it)"
                .into(),
        ));
    }
    obj.standalone_content = Some(sc);
    Ok(())
}

/// Write-side mirror of [`attach_standalone_content`]: эмитить `obj.standalone_content` в
/// конфиг-Ext-каталог целевого формата (пер-форматное ИМЯ и ДИАЛЕКТ). Объект без состава —
/// no-op. §1.0: EDT-запись ненулевого `inactive_nodes_cleanup_timeout` — типизированный отказ.
pub(crate) fn write_standalone_content(
    format: Format,
    ext_dir: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    let Some(sc) = obj.standalone_content.as_ref() else {
        return Ok(());
    };
    let write_err = |reason: String| ConvertError::Write {
        kind: "Configuration".to_string(),
        object: obj.name.clone(),
        reason,
    };
    let (file, bytes) = match format {
        Format::Designer => (
            DESIGNER_FILE,
            serialize_designer(sc, crate::sidecar_version::write_target()),
        ),
        Format::Edt => (EDT_FILE, serialize_edt(sc).map_err(&write_err)?),
        Format::Cf => return Ok(()), // контейнер — тело `<host>.f` собирает cf-ассемблер.
    };
    crate::form_write::write_file(&ext_dir.join(file), &bytes)
}

// ---------------------------------------------------------------------------
// Designer-диалект
// ---------------------------------------------------------------------------

/// Разобрать Designer-байты → (канон, witnessed-версия формата источника). Строгий
/// последовательный парс witnessed-формы (никакого общего XML: §1.0 — неожиданная форма
/// означает не смоделированный конструкт и обязана отказать громко, не угадываться).
fn parse_designer(bytes: &[u8]) -> Result<(StandaloneContent, FormatVersion), String> {
    let body = bytes
        .strip_prefix(BOM)
        .ok_or_else(|| "Designer sidecar carries no BOM (witnessed: it DOES) — §1.0".to_string())?;
    let text = std::str::from_utf8(body).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(DESIGNER_HEAD)?;
    let version = cur.until("\"")?;
    let version = crate::sidecar_version::parse_witnessed(&version, "standalone content")?;
    cur.expect(">\r\n")?;

    let mut sc = StandaloneContent {
        used: Vec::new(),
        unused: Vec::new(),
        priority: Vec::new(),
        exchange_on_change_data: false,
        exchange_period: 0,
        transaction_count: 0,
        inactive_nodes_cleanup_timeout: 0,
    };
    while cur.try_expect("\t<UsedItem>\r\n\t\t<Metadata>") {
        sc.used.push(name_text(cur.until("</Metadata>\r\n\t</UsedItem>\r\n")?)?);
    }
    while cur.try_expect("\t<UnusedItem>\r\n\t\t<Metadata>") {
        sc.unused
            .push(name_text(cur.until("</Metadata>\r\n\t</UnusedItem>\r\n")?)?);
    }
    while cur.try_expect("\t<PriorityItem>\r\n\t\t<Metadata>") {
        let metadata = name_text(cur.until("</Metadata>\r\n\t\t<Priority>")?)?;
        let priority = name_text(cur.until("</Priority>\r\n\t</PriorityItem>\r\n")?)?;
        sc.priority.push(StandalonePriority { metadata, priority });
    }
    cur.expect("\t<DataExchangeSettings>\r\n\t\t<ExchangeOnChangeData>")?;
    sc.exchange_on_change_data = parse_bool(&cur.until("</ExchangeOnChangeData>\r\n\t\t<ExchangePeriod>")?)?;
    sc.exchange_period = parse_u64(&cur.until("</ExchangePeriod>\r\n\t\t<TransactionCount>")?)?;
    sc.transaction_count =
        parse_u64(&cur.until("</TransactionCount>\r\n\t\t<InactiveNodesCleanupTimeout>")?)?;
    sc.inactive_nodes_cleanup_timeout = parse_u64(
        &cur.until("</InactiveNodesCleanupTimeout>\r\n\t</DataExchangeSettings>\r\n")?,
    )?;
    cur.expect("</StandaloneContent>")?;
    cur.expect_eof()?;
    Ok((sc, version))
}

/// Designer-байты из канона под ЗАДАННОЙ версией формата (обратная [`parse_designer`]).
fn serialize_designer(sc: &StandaloneContent, version: FormatVersion) -> Vec<u8> {
    let mut out = String::new();
    out.push_str(DESIGNER_HEAD);
    out.push_str(&version.to_string());
    out.push_str("\">\r\n");
    for name in &sc.used {
        out.push_str("\t<UsedItem>\r\n\t\t<Metadata>");
        out.push_str(name);
        out.push_str("</Metadata>\r\n\t</UsedItem>\r\n");
    }
    for name in &sc.unused {
        out.push_str("\t<UnusedItem>\r\n\t\t<Metadata>");
        out.push_str(name);
        out.push_str("</Metadata>\r\n\t</UnusedItem>\r\n");
    }
    for p in &sc.priority {
        out.push_str("\t<PriorityItem>\r\n\t\t<Metadata>");
        out.push_str(&p.metadata);
        out.push_str("</Metadata>\r\n\t\t<Priority>");
        out.push_str(&p.priority);
        out.push_str("</Priority>\r\n\t</PriorityItem>\r\n");
    }
    out.push_str("\t<DataExchangeSettings>\r\n\t\t<ExchangeOnChangeData>");
    out.push_str(bool_text(sc.exchange_on_change_data));
    out.push_str("</ExchangeOnChangeData>\r\n\t\t<ExchangePeriod>");
    out.push_str(&sc.exchange_period.to_string());
    out.push_str("</ExchangePeriod>\r\n\t\t<TransactionCount>");
    out.push_str(&sc.transaction_count.to_string());
    out.push_str("</TransactionCount>\r\n\t\t<InactiveNodesCleanupTimeout>");
    out.push_str(&sc.inactive_nodes_cleanup_timeout.to_string());
    out.push_str("</InactiveNodesCleanupTimeout>\r\n\t</DataExchangeSettings>\r\n");
    out.push_str("</StandaloneContent>");
    let mut bytes = BOM.to_vec();
    bytes.extend_from_slice(out.as_bytes());
    bytes
}

// ---------------------------------------------------------------------------
// EDT-диалект
// ---------------------------------------------------------------------------

/// Разобрать EDT-байты → канон. Тот же строгий последовательный парс witnessed-формы.
fn parse_edt(bytes: &[u8]) -> Result<StandaloneContent, String> {
    if bytes.starts_with(BOM) {
        return Err("EDT sidecar carries a BOM (witnessed: it does NOT) — §1.0".into());
    }
    let text = std::str::from_utf8(bytes).map_err(|e| format!("not UTF-8: {e}"))?;
    let mut cur = Cursor::new(text);
    cur.expect(EDT_HEAD)?;
    let mut sc = StandaloneContent {
        used: Vec::new(),
        unused: Vec::new(),
        priority: Vec::new(),
        exchange_on_change_data: false,
        exchange_period: 0,
        transaction_count: 0,
        // У EDT-диалекта поля НЕТ вовсе — witnessed-значение 0 (дефолт, см. IR-док).
        inactive_nodes_cleanup_timeout: 0,
    };
    while cur.try_expect("  <usedItem metadata=\"") {
        sc.used.push(name_text(cur.until("\"/>\r\n")?)?);
    }
    while cur.try_expect("  <unusedItem metadata=\"") {
        sc.unused.push(name_text(cur.until("\"/>\r\n")?)?);
    }
    while cur.try_expect("  <priorityItem metadata=\"") {
        let metadata = name_text(cur.until("\" priority=\"")?)?;
        let priority = name_text(cur.until("\"/>\r\n")?)?;
        sc.priority.push(StandalonePriority { metadata, priority });
    }
    cur.expect("  <dataExchangeSettings exchangeOnChangeData=\"")?;
    sc.exchange_on_change_data = parse_bool(&cur.until("\" exchangePeriod=\"")?)?;
    sc.exchange_period = parse_u64(&cur.until("\" transactionCount=\"")?)?;
    sc.transaction_count = parse_u64(&cur.until("\"/>\r\n")?)?;
    cur.expect("</scc:StandaloneContent>\r\n")?;
    cur.expect_eof()?;
    Ok(sc)
}

/// EDT-байты из канона (обратная [`parse_edt`]). §1.0: ненулевой
/// `inactive_nodes_cleanup_timeout` не имеет witnessed-кодировки в EDT-диалекте — отказ.
fn serialize_edt(sc: &StandaloneContent) -> Result<Vec<u8>, String> {
    if sc.inactive_nodes_cleanup_timeout != 0 {
        return Err(format!(
            "standalone content carries inactiveNodesCleanupTimeout={} but the EDT dialect has \
             no witnessed encoding for a non-zero value (the witnessed EDT file carries no such \
             attribute at all) — §1.0",
            sc.inactive_nodes_cleanup_timeout
        ));
    }
    let mut out = String::new();
    out.push_str(EDT_HEAD);
    for name in &sc.used {
        out.push_str("  <usedItem metadata=\"");
        out.push_str(name);
        out.push_str("\"/>\r\n");
    }
    for name in &sc.unused {
        out.push_str("  <unusedItem metadata=\"");
        out.push_str(name);
        out.push_str("\"/>\r\n");
    }
    for p in &sc.priority {
        out.push_str("  <priorityItem metadata=\"");
        out.push_str(&p.metadata);
        out.push_str("\" priority=\"");
        out.push_str(&p.priority);
        out.push_str("\"/>\r\n");
    }
    out.push_str("  <dataExchangeSettings exchangeOnChangeData=\"");
    out.push_str(bool_text(sc.exchange_on_change_data));
    out.push_str("\" exchangePeriod=\"");
    out.push_str(&sc.exchange_period.to_string());
    out.push_str("\" transactionCount=\"");
    out.push_str(&sc.transaction_count.to_string());
    out.push_str("\"/>\r\n");
    out.push_str("</scc:StandaloneContent>\r\n");
    Ok(out.into_bytes())
}

// ---------------------------------------------------------------------------
// Помощники строгого последовательного парса
// ---------------------------------------------------------------------------

/// Курсор строгого последовательного парса witnessed-XML-формы. `pub(crate)`: тем же
/// приёмом парсятся конфиг-уровневые интерфейс-сайдкары (`crate::config_interface_read`).
pub(crate) struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        Cursor { rest: text }
    }

    /// Обязательный префикс — иначе §1.0-ошибка с контекстом.
    pub(crate) fn expect(&mut self, prefix: &str) -> Result<(), String> {
        if let Some(rest) = self.rest.strip_prefix(prefix) {
            self.rest = rest;
            Ok(())
        } else {
            let got: String = self.rest.chars().take(60).collect();
            Err(format!(
                "expected {prefix:?}, got {got:?}… (unwitnessed byte-shape, §1.0)"
            ))
        }
    }

    /// Опциональный префикс (для повторяющихся блоков).
    pub(crate) fn try_expect(&mut self, prefix: &str) -> bool {
        if let Some(rest) = self.rest.strip_prefix(prefix) {
            self.rest = rest;
            true
        } else {
            false
        }
    }

    /// Текст ДО (и включая потребление) `delim`.
    pub(crate) fn until(&mut self, delim: &str) -> Result<String, String> {
        match self.rest.find(delim) {
            Some(at) => {
                let text = self.rest[..at].to_string();
                self.rest = &self.rest[at + delim.len()..];
                Ok(text)
            }
            None => Err(format!("missing {delim:?} (unwitnessed byte-shape, §1.0)")),
        }
    }

    pub(crate) fn expect_eof(&self) -> Result<(), String> {
        if self.rest.is_empty() {
            Ok(())
        } else {
            let got: String = self.rest.chars().take(60).collect();
            Err(format!("trailing bytes after root close: {got:?}… (§1.0)"))
        }
    }
}

/// Текстовое значение имени/enum'а: XML-разметка и escape'ы в 1С-идентификаторах не
/// witnessed — символ, требующий escape'а, означает не смоделированную кодировку (§1.0).
pub(crate) fn name_text(raw: String) -> Result<String, String> {
    if raw.contains(['<', '>', '&', '"']) {
        return Err(format!(
            "value {raw:?} contains XML markup/escape characters — unwitnessed for standalone \
             content names (§1.0)"
        ));
    }
    Ok(raw)
}

pub(crate) fn parse_bool(raw: &str) -> Result<bool, String> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(format!("boolean {other:?} is neither true nor false (§1.0)")),
    }
}

pub(crate) fn bool_text(v: bool) -> &'static str {
    if v {
        "true"
    } else {
        "false"
    }
}

pub(crate) fn parse_u64(raw: &str) -> Result<u64, String> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("number {raw:?} is not a plain decimal (§1.0)"));
    }
    raw.parse::<u64>()
        .map_err(|e| format!("number {raw:?}: {e} (§1.0)"))
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::version::{ERP, SSL};

    /// Микро-канон (витнесс-эхо ERP в миниатюре: те же формы блоков, 2 used / 1 unused
    /// дочерний путь / 1 priority / настройки true,300,1000,0).
    fn micro() -> StandaloneContent {
        StandaloneContent {
            used: vec![
                "Role.РольА".to_string(),
                "Catalog.Товары".to_string(),
            ],
            unused: vec!["Document.Заказ.Attribute.Номер".to_string()],
            priority: vec![StandalonePriority {
                metadata: "Constant.ПринятоЗаписей".to_string(),
                priority: "LocalServer".to_string(),
            }],
            exchange_on_change_data: true,
            exchange_period: 300,
            transaction_count: 1000,
            inactive_nodes_cleanup_timeout: 0,
        }
    }

    fn micro_designer() -> Vec<u8> {
        serialize_designer(&micro(), ERP)
    }

    /// Оба диалекта микрофикстуры → РАВНЫЙ канон (§1.6) + byte-exact re-serialize (§1.0).
    #[test]
    fn both_dialects_roundtrip_and_agree() {
        let dsn = micro_designer();
        let (from_dsn, v) = parse_designer(&dsn).expect("designer parse");
        assert_eq!(v, ERP, "detected 2.20");
        assert_eq!(from_dsn, micro());
        assert_eq!(serialize_designer(&from_dsn, ERP), dsn, "designer byte-exact");

        let edt = serialize_edt(&micro()).expect("edt serialize");
        let from_edt = parse_edt(&edt).expect("edt parse");
        assert_eq!(from_edt, micro(), "§1.6: единый канон");
        assert_eq!(serialize_edt(&from_edt).unwrap(), edt, "edt byte-exact");
    }

    /// Designer под SSL-версией (2.21) — параметр версии, не константа.
    #[test]
    fn designer_version_is_a_parameter() {
        let ssl_bytes = serialize_designer(&micro(), SSL);
        let (sc, v) = parse_designer(&ssl_bytes).expect("parse 2.21");
        assert_eq!(v, SSL);
        assert_eq!(sc, micro());
        assert!(ssl_bytes.windows(6).any(|w| w == b"\"2.21\""));
    }

    /// §1.0: Designer без BOM / EDT с BOM / невитнесснутая версия — громкие отказы.
    #[test]
    fn dialect_envelope_violations_are_refused() {
        let dsn = micro_designer();
        let err = parse_designer(&dsn[3..]).unwrap_err();
        assert!(err.contains("BOM"), "{err}");

        let mut edt_with_bom = b"\xEF\xBB\xBF".to_vec();
        edt_with_bom.extend_from_slice(&serialize_edt(&micro()).unwrap());
        let err = parse_edt(&edt_with_bom).unwrap_err();
        assert!(err.contains("BOM"), "{err}");

        let bad = String::from_utf8(micro_designer())
            .unwrap()
            .replace("version=\"2.20\"", "version=\"2.19\"");
        let err = parse_designer(bad.as_bytes()).unwrap_err();
        assert!(err.contains("witnessed"), "{err}");
    }

    /// §1.0: EDT-запись ненулевого InactiveNodesCleanupTimeout — типизированный отказ
    /// (у EDT-диалекта поля нет; Designer пишет значение честно).
    #[test]
    fn edt_nonzero_inactive_timeout_is_refused() {
        let mut sc = micro();
        sc.inactive_nodes_cleanup_timeout = 60;
        let err = serialize_edt(&sc).unwrap_err();
        assert!(err.contains("inactiveNodesCleanupTimeout=60"), "{err}");
        let dsn = serialize_designer(&sc, ERP);
        let (back, _) = parse_designer(&dsn).expect("designer roundtrips non-zero");
        assert_eq!(back.inactive_nodes_cleanup_timeout, 60);
    }

    /// §1.0: перекошенная разметка (чужой тег в середине) — громкий отказ, не тихий скип.
    #[test]
    fn unwitnessed_markup_is_refused() {
        let bad = String::from_utf8(micro_designer())
            .unwrap()
            .replace("<DataExchangeSettings>", "<DataExchangeSettingsX>");
        let err = parse_designer(bad.as_bytes()).unwrap_err();
        assert!(err.contains("§1.0"), "{err}");
    }

    /// КЛЮЧЕВОЙ ИНТЕГРАЦИОННЫЙ: НАСТОЯЩИЕ ERP-фикстуры обоих диалектов — парс, счётчики
    /// витнесса (1692/596/3 + true/300/1000/0), §1.6-равенство канонов designer==edt И
    /// byte-exact re-serialize обоих файлов (самопроверка на реальном корпусе).
    #[test]
    fn erp_fixture_files_parse_equal_and_roundtrip() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dsn_path = root.join(".fixtures/ERP/designer_8.3.27/Ext/StandaloneConfigurationContent.bin");
        let edt_path = root.join(".fixtures/ERP/edt/src/Configuration/MobileApplicationContent.scc");
        if !dsn_path.is_file() || !edt_path.is_file() {
            eprintln!("skipping: ERP fixtures not present");
            return;
        }
        let dsn_bytes = std::fs::read(&dsn_path).unwrap();
        let edt_bytes = std::fs::read(&edt_path).unwrap();

        let (from_dsn, v) = parse_designer(&dsn_bytes).expect("ERP designer parses");
        assert_eq!(v, ERP, "ERP dump format 2.20");
        let from_edt = parse_edt(&edt_bytes).expect("ERP edt parses");
        assert_eq!(from_dsn, from_edt, "§1.6: designer == edt canon (весь ERP-состав)");

        assert_eq!(from_dsn.used.len(), 1692, "witnessed used count");
        assert_eq!(from_dsn.unused.len(), 596, "witnessed unused count");
        assert_eq!(from_dsn.priority.len(), 3, "witnessed priority count");
        assert!(from_dsn
            .priority
            .iter()
            .all(|p| p.priority == "LocalServer"));
        assert!(from_dsn.exchange_on_change_data);
        assert_eq!(from_dsn.exchange_period, 300);
        assert_eq!(from_dsn.transaction_count, 1000);
        assert_eq!(from_dsn.inactive_nodes_cleanup_timeout, 0);

        assert_eq!(
            serialize_designer(&from_dsn, ERP),
            dsn_bytes,
            "ERP designer byte-exact"
        );
        assert_eq!(
            serialize_edt(&from_edt).unwrap(),
            edt_bytes,
            "ERP edt byte-exact"
        );
    }
}
