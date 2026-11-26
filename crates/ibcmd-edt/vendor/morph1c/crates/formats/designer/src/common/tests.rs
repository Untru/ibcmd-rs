//! Гейт версионной обёртки Designer-XML (§1.0/§3.2, FORMATS.md §1/§3): обёртка теперь
//! ЗНАЕТ обе версии — SSL 8.5.1 (формат 2.21, 18 xmlns с `pal`) и ERP 8.3.27 (формат
//! 2.20, 17 xmlns без `pal`). Здесь:
//!
//! 1. РЕЕСТРОВЫЕ инварианты (чистые, без фикстур): `version_value == format.to_string()`
//!    (замыкание к FORMATS.md §2, не второй хардкод); ERP-блок == SSL-блок минус `pal`;
//!    detect/profile-роундтрип; неизвестная версия → `None`.
//! 2. ENVELOPE byte-exact на РЕАЛЬНЫХ фикстурах ОБЕИХ версий: корневой тег
//!    `<MetaDataObject …>` (BOM+пролог+ns-блок+`version=`) РИДИТСЯ (`verify_root_envelope`
//!    принимает + детектит версию) И ПИШЕТСЯ (`emit_root_envelope`) БАЙТ-В-БАЙТ.
//!    Фикстуры gitignore'нуты → инфра-skip при отсутствии (единственный разрешённый
//!    пропуск, §3.2).

use super::*;
use formats_xml::parse;
use morph1c_core::version::{ERP, SSL};
use std::path::{Path, PathBuf};

// --- реестровые инварианты (чистые) ------------------------------------------

#[test]
fn profile_version_value_matches_format() {
    // `version_value` НЕ второй хардкод — он ОБЯЗАН равняться `format.to_string()`
    // (FormatVersion таблицы FORMATS.md §2). Иначе реестр разошёлся бы с FORMATS.md.
    for p in envelope_profiles() {
        assert_eq!(
            p.version_value,
            p.format.to_string(),
            "profile version_value must equal format.to_string() (FORMATS.md §2 sync)"
        );
    }
}

#[test]
fn erp_ns_block_is_ssl_without_pal() {
    // Единственное структурное отличие ns-блоков ERP↔SSL — отсутствие `xmlns:pal`.
    let ssl_without_pal: Vec<_> = NS_BLOCK_SSL
        .iter()
        .filter(|(name, _)| *name != "xmlns:pal")
        .copied()
        .collect();
    assert_eq!(
        NS_BLOCK_ERP.to_vec(),
        ssl_without_pal,
        "ERP ns-block must be exactly SSL ns-block minus xmlns:pal, same order"
    );
    assert_eq!(
        NS_BLOCK_SSL.len(),
        18,
        "SSL has 18 xmlns (17 prefixed + default, incl. pal)"
    );
    assert_eq!(NS_BLOCK_ERP.len(), 17, "ERP has 17 xmlns (no pal)");
    assert!(
        NS_BLOCK_SSL.iter().any(|(n, _)| *n == "xmlns:pal"),
        "SSL must have pal"
    );
    assert!(
        !NS_BLOCK_ERP.iter().any(|(n, _)| *n == "xmlns:pal"),
        "ERP must NOT have pal"
    );
}

#[test]
fn registry_lookup_and_detect_roundtrip() {
    // profile_for(version) и detect_profile(version_value) — согласованы и покрывают
    // ОБА корпуса. back-compat синонимы указывают на SSL-профиль.
    let ssl = profile_for(SSL).expect("SSL profile present");
    let erp = profile_for(ERP).expect("ERP profile present");
    assert_eq!(ssl.version_value, "2.21");
    assert_eq!(erp.version_value, "2.20");
    assert_eq!(ssl.ns_block, NS_BLOCK_SSL);
    assert_eq!(erp.ns_block, NS_BLOCK_ERP);

    assert_eq!(detect_profile("2.21").map(|p| p.format), Some(SSL));
    assert_eq!(detect_profile("2.20").map(|p| p.format), Some(ERP));

    // back-compat консты = SSL-профиль (lib.rs продолжает работать без изменений).
    assert_eq!(VERSION_VALUE, ssl.version_value);
    assert_eq!(ROOT_NS_BLOCK, NS_BLOCK_SSL);
}

#[test]
fn unknown_version_is_hard_none_not_default() {
    // §1.0: неизвестная версия/ns-набор — жёсткий None (коннектор → ошибка), не дефолт.
    assert!(
        detect_profile("2.19").is_none(),
        "2.19 not in registry → None (no silent default)"
    );
    assert!(detect_profile("").is_none());
    assert!(detect_profile("garbage").is_none());
    assert!(profile_for(FormatVersion::new(2, 5)).is_none());
    assert!(
        emit_root_envelope(FormatVersion::new(2, 5)).is_err(),
        "unknown target → Err"
    );
}

// --- envelope byte-exact на реальных фикстурах ОБЕИХ версий -------------------

/// Локатор корпуса (тот же контракт, что roundtrip.rs / testkit): env
/// `MORPH1C_FIXTURES` (abs путь к `.fixtures`) имеет приоритет, иначе — `.fixtures` от
/// корня воркспейса. CARGO_MANIFEST_DIR = crates/formats/designer → nth(3) = корень.
fn fixtures_root() -> PathBuf {
    if let Some(p) = std::env::var_os("MORPH1C_FIXTURES") {
        return PathBuf::from(p);
    }
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("workspace root")
        .join(".fixtures")
}

/// Первый `.xml` в каталоге (стабильно — сортировка по имени), если каталог есть.
fn first_xml_in(dir: &Path) -> Option<PathBuf> {
    let mut xs: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("xml"))
        .collect();
    xs.sort();
    xs.into_iter().next()
}

/// Точные байты открывающего тега `<MetaDataObject …>` (от BOM включительно до первого
/// `>`) — это ВСЯ версионная обёртка на записи (BOM, пролог, EOL, корень, ns-блок,
/// `version=`). Сверяем именно её byte-exact.
fn opening_tag_bytes(full: &[u8]) -> &[u8] {
    let end = full
        .iter()
        .position(|&b| b == b'>')
        .expect("descriptor must contain a '>'");
    &full[..=end]
}

/// Envelope byte-exact для одного фикстур-файла ожидаемой версии `want`:
/// READ (`verify_root_envelope` принимает + детектит версию) и WRITE
/// (`emit_root_envelope` эмитит тот же корневой тег байт-в-байт).
fn assert_envelope_byte_exact(path: &Path, want: FormatVersion) {
    let bytes = std::fs::read(path).expect("read fixture bytes");

    // --- READ: детект + сверка версионной обёртки ---
    let descriptor = parse(&bytes).expect("fixture must tokenize");
    let detected =
        verify_root_envelope(&descriptor.root).expect("verify_root_envelope must accept fixture");
    assert_eq!(
        detected,
        want,
        "detected format version must match corpus ({})",
        path.display()
    );

    // Обёртка ПОЛНОСТЬЮ востребована: все ns + version claim'нуты (кроме uuid/детей,
    // которых у нас нет заботы — считаем только атрибуты корня).
    let unclaimed_root_attrs = descriptor
        .root
        .attrs
        .iter()
        .filter(|a| !a.claimed.get())
        .count();
    assert_eq!(
        unclaimed_root_attrs,
        0,
        "all root ns + version attrs must be claimed by verify_root_envelope ({})",
        path.display()
    );

    // --- WRITE: эмиссия корневого тега той же версии, byte-exact ---
    let mut root = emit_root_envelope(want).expect("emit_root_envelope for known version");
    // Даём корню одного ребёнка, чтобы render эмитил ветку (открывающий тег + EOL),
    // а не пустой `<tag></tag>`; сравниваем ТОЛЬКО открывающий тег.
    root.push(OutElement::self_closing("", "Marker"));
    let rendered = formats_xml::emit::render(&DESIGNER_ENVELOPE, &root);

    assert_eq!(
        opening_tag_bytes(&rendered),
        opening_tag_bytes(&bytes),
        "written <MetaDataObject …> envelope must be byte-exact vs fixture ({})",
        path.display()
    );
}

#[test]
fn envelope_byte_exact_ssl_8_5_1() {
    let dir = fixtures_root().join("SSL/designer_8.5.1/cf/CommonModules");
    let Some(f) = first_xml_in(&dir) else {
        eprintln!(
            "INFRA-SKIP: SSL corpus absent ({}). Fetch fixtures to run.",
            dir.display()
        );
        return;
    };
    assert_envelope_byte_exact(&f, SSL);
    eprintln!("SSL envelope byte-exact OK: {}", f.display());
}

#[test]
fn envelope_byte_exact_erp_8_3_27() {
    // ERP CommonModule сначала; если каталога нет — Constants (та же обёртка 2.20).
    let base = fixtures_root().join("ERP/designer_8.3.27");
    let f =
        first_xml_in(&base.join("CommonModules")).or_else(|| first_xml_in(&base.join("Constants")));
    let Some(f) = f else {
        eprintln!(
            "INFRA-SKIP: ERP corpus absent ({}). Fetch fixtures to run.",
            base.display()
        );
        return;
    };
    assert_envelope_byte_exact(&f, ERP);
    eprintln!("ERP envelope byte-exact OK: {}", f.display());
}
