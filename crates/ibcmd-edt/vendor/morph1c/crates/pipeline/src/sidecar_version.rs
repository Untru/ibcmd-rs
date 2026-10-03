//! Версия формата Designer-САЙДКАРОВ (`Ext/Schedule.xml` / `Ext/Content.xml` /
//! `Ext/CommandInterface.xml` / `Ext/Flowchart.xml`) — ЦЕНТРАЛЬНАЯ пара «распознать
//! witnessed-версию входа / выбрать версию таргета» (FORMATS.md §1: формат входа
//! детектится, формат выхода — параметр; версия — свойство ФАЙЛА, а не IR, §1.6).
//!
//! Каждый Designer-сайдкар несёт в корневом теге `version="2.20"|"2.21"` — ту же версию
//! формата выгрузки, что и дескрипторы (witnessed: ERP 8.3.27 → 2.20 на всех 258
//! Schedule.xml / 16 Content.xml / 245 CommandInterface.xml / 19 Flowchart.xml; SSL
//! 8.5.1 → 2.21). Ридер принимает ЛЮБУЮ версию из реестра witnessed-обёрток
//! ([`formats_designer::common::detect_profile`] — единственный источник правды, не
//! второй хардкод), НЕЗНАКОМАЯ версия — типизированный отказ (§1.0: молча переписать
//! её таргетом значило бы тихо перештамповать дамп). Плотность ТЕЛА сайдкара при этом
//! охраняет §1.0-самопроверка byte-round-trip'а самих кодеков.
//!
//! Райтер эмитит версию АМБЬЕНТНОГО round-trip-таргета
//! ([`morph1c_core::version::current_roundtrip_target`] — его ставит `pipeline::convert`
//! из `--v8version` либо версии источника), дефолт — SSL (весь исторический SSL-путь
//! byte-exact без изменений).

use morph1c_core::version::FormatVersion;

/// Распознать witnessed-версию Designer-сайдкара из значения корневого `version=`.
/// §1.0: версия вне реестра witnessed-обёрток → типизированный отказ (не дефолт).
pub(crate) fn parse_witnessed(version: &str, ctx: &str) -> Result<FormatVersion, String> {
    formats_designer::common::detect_profile(version)
        .map(|p| p.format)
        .ok_or_else(|| {
            format!(
                "{ctx}: root version attribute {version:?} is not a witnessed designer format \
                 version (see the ENVELOPE_PROFILES registry / FORMATS.md §2; an unwitnessed \
                 schema version would otherwise be silently restamped) — §1.0"
            )
        })
}

/// Версия ТАРГЕТА для записи Designer-сайдкара: амбьентный round-trip-таргет
/// (установлен `pipeline::convert`), вне scope — SSL (обратная совместимость).
pub(crate) fn write_target() -> FormatVersion {
    morph1c_core::version::current_roundtrip_target().unwrap_or(morph1c_core::version::SSL)
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::version::{ERP, SSL};

    #[test]
    fn witnessed_versions_parse() {
        assert_eq!(parse_witnessed("2.20", "t").unwrap(), ERP);
        assert_eq!(parse_witnessed("2.21", "t").unwrap(), SSL);
    }

    #[test]
    fn unwitnessed_version_is_refused() {
        let err = parse_witnessed("2.19", "t").unwrap_err();
        assert!(err.contains("not a witnessed"), "{err}");
    }

    #[test]
    fn default_write_target_is_ssl() {
        assert_eq!(write_target(), SSL);
        morph1c_core::version::with_roundtrip_target(ERP, || {
            assert_eq!(write_target(), ERP);
        });
    }
}
