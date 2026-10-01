//! `.dcss` сайдкар: цельный `<Settings>`-документ настроек динсписка (byte-exact read/write).

use super::*;

/// Полный фиксированный ns-блок корня `<Settings>` (11 объявлений; settings-ns — ДЕФОЛТНЫЙ).
/// Порядок и URI сверены по корпусу — строка идентична во всех 227 сайдкарах SSL.
pub(crate) const DCSS_ROOT_NS: &[(&str, &str)] = &[
    ("xmlns", DCSSET_NS_URI),
    ("xmlns:xsi", XSI_NS_URI),
    ("xmlns:xs", "http://www.w3.org/2001/XMLSchema"),
    ("xmlns:v8", "http://v8.1c.ru/8.1/data/core"),
    ("xmlns:v8ui", "http://v8.1c.ru/8.1/data/ui"),
    ("xmlns:pal", "http://v8.1c.ru/8.1/data/ui/colors/palette"),
    ("xmlns:style", "http://v8.1c.ru/8.1/data/ui/style"),
    ("xmlns:sys", "http://v8.1c.ru/8.1/data/ui/fonts/system"),
    ("xmlns:web", "http://v8.1c.ru/8.1/data/ui/colors/web"),
    ("xmlns:win", "http://v8.1c.ru/8.1/data/ui/colors/windows"),
    (
        "xmlns:dcscor",
        "http://v8.1c.ru/8.1/data-composition-system/core",
    ),
];

/// Прочитать байты `ListSettings.dcss` → [`DcsListSettings`] (byte-exact-обратимо).
/// §1.0: envelope/корень/ns сверяются, каждый узел claim'ится или ошибка (не Blob, не skip).
pub fn read_list_settings_dcss(bytes: &[u8]) -> Result<DcsListSettings, FormError> {
    let descriptor = parse(bytes)?;
    let env = descriptor.bytes_env;
    if env.bom {
        return Err(FormError::Envelope("dcss: unexpected BOM".into()));
    }
    if env.eol != EolStyle::Crlf {
        return Err(FormError::Envelope(format!(
            "dcss: must be CRLF, found {:?}",
            env.eol
        )));
    }
    match descriptor.decl.as_deref() {
        Some(d) if d == DCSS_DECL => {}
        other => {
            return Err(FormError::Envelope(format!(
                "dcss: unexpected decl: {other:?}"
            )))
        }
    }
    let mut root = descriptor.root;
    if !root.prefix.is_empty() || root.local != "Settings" {
        return Err(FormError::Envelope(format!(
            "dcss: unexpected root <{}{}{}>",
            root.prefix,
            if root.prefix.is_empty() { "" } else { ":" },
            root.local
        )));
    }
    root.claim();
    // `xmlns:pal` — ОПЦИОНАЛЕН: SSL несёт его 227/227, ERP-сайдкары идут БЕЗ него (witness
    // Международный.ФормаСписка). Присутствие переносится presence-битом
    // (`DcsListSettings::envelope_without_pal`) — re-emit byte-exact в обе стороны;
    // отсутствующий НЕ выдумываем. Остальные 10 объявлений — обязательный каркас.
    let required: Vec<(&str, &str)> = DCSS_ROOT_NS
        .iter()
        .copied()
        .filter(|(name, _)| *name != "xmlns:pal")
        .collect();
    claim_root_ns(&root, &required)?;
    let pal_uri = DCSS_ROOT_NS
        .iter()
        .find(|(name, _)| *name == "xmlns:pal")
        .expect("DCSS_ROOT_NS carries xmlns:pal")
        .1;
    let envelope_without_pal = match root.attr("xmlns:pal") {
        Some(a) => {
            if a.value != pal_uri {
                return Err(FormError::Envelope(format!(
                    "root xmlns:pal={:?} want {pal_uri:?}",
                    a.value
                )));
            }
            a.claimed.set(true);
            false
        }
        None => true,
    };
    // ЕДИНСТВЕННОЕ отличие от Designer-инлайна: settings-ns здесь ДЕФОЛТНЫЙ. Переписываем его на
    // `dcsset:` и делегируем ОДНОМУ типизированному ридеру (см. модульный docstring).
    default_ns_to_dcsset(&mut root);
    let mut ls = read_designer_list_settings(&root)?;
    ls.envelope_without_pal = envelope_without_pal;
    // §1.0 тотальность по всему дереву сайдкара.
    let leftover = root.unclaimed_count();
    if leftover != 0 {
        return Err(FormError::Frame(format!(
            "{leftover} unconsumed node(s) in ListSettings.dcss (no passthrough/Raw — §1.0): {:?}",
            unclaimed_labels(&root)
        )));
    }
    Ok(ls)
}

/// Записать [`DcsListSettings`] в байты `ListSettings.dcss` (byte-exact к платформенному
/// экспорту — 227/227 сайдкаров SSL регенерируются в точности).
///
/// ПУСТЫЕ настройки сайдкара НЕ ИМЕЮТ (платформа файл не создаёт — witness ×2); вызывающая
/// сторона (`pipeline::form_write`) обязана проверить [`DcsListSettings::is_empty`] ДО вызова.
pub fn write_list_settings_dcss(ls: &DcsListSettings) -> Vec<u8> {
    // Строим Designer-инлайн `<ListSettings>` тем же writer'ом и снимаем префикс `dcsset:`.
    let mut inline = designer_list_settings(ls);
    dcsset_to_default_ns(&mut inline);
    let mut root = OutElement::branch("", "Settings");
    for (name, uri) in DCSS_ROOT_NS {
        // presence-точный re-emit: ERP-flavored сайдкар (без pal) не получает выдуманного
        // объявления; SSL-flavor (default false) — прежний 11-ns блок byte-exact.
        if ls.envelope_without_pal && *name == "xmlns:pal" {
            continue;
        }
        root = root.attr(*name, *uri);
    }
    for c in inline.children {
        root.push(c);
    }
    render(&dcss_envelope(), &root)
}
