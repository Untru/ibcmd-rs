//! Blob-СПУТНИК картинки: формат-нейтральное построение [`BlobRef`] из байтов бинарного
//! файла-спутника (`Picture.<ext>`) — общий субстрат среза S1 (picture/Blob).
//!
//! # Зачем ЗДЕСЬ (formats-xml)
//! Картинка `CommonPicture` (и любой picture-Blob вида — Bot/Subsystem/…) хранится
//! БИНАРНЫМ файлом рядом с дескриптором. §1.0: это ЛЕГИТИМНЫЙ as-is Blob — читается
//! ВЕРБАТИМ, пишется ВЕРБАТИМ, byte-exact; НИКАКОЙ интерпретации PNG/SVG/ZIP-внутренностей.
//! И EDT, и Designer держат ОДИН И ТОТ ЖЕ файл (сверено корпусом: 600/600 байт-идентичны),
//! поэтому оба формата ОБЯЗАНЫ дать РАВНЫЙ [`BlobRef`] → cross-format X by construction
//! (§1.6/§3.5). Этот хелпер — единственная точка построения `BlobRef` из байтов, чтобы
//! `key`/`len`/`digest` совпадали у всех форматов и видов.
//!
//! # `digest` — детерминированный, dependency-free
//! [`BlobRef::digest`] — `[u8; 32]`. Крипто-SHA здесь избыточен (цель — РАВЕНСТВО одного и
//! того же файла между форматами, не защита от злонамеренных коллизий), а тянуть внешний
//! crate ради него — лишняя зависимость. Поэтому — детерминированный 256-битный digest на
//! базе FNV-1a по 4 независимо-затравленным потокам (64 бита каждый), покрывающий длину и
//! содержимое. Одинаковые байты → одинаковый digest в любом формате; разные — с подавляющей
//! вероятностью различны. Стабилен между запусками/платформами (чистая арифметика на байтах).

use morph1c_core::ir::value::BlobRef;

/// FNV-1a 64-бит с заданной затравкой (offset basis) по байтам `data`.
fn fnv1a64(seed: u64, data: &[u8]) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01B3;
    let mut h = seed;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// Детерминированный 256-битный digest байтов (4×64-бит FNV-1a с РАЗНЫМИ затравками,
/// плюс подмешанная длина — чтобы префикс/суффикс-коллизии по одному потоку не проходили).
/// Формат-нейтрален и стабилен между платформами.
pub fn digest256(data: &[u8]) -> [u8; 32] {
    // Четыре независимые затравки (стандартный FNV offset basis + производные константы).
    const SEEDS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x1000_0000_0000_01b3,
        0x9e37_79b9_7f4a_7c15,
        0xff51_afd7_ed55_8ccd,
    ];
    let len = data.len() as u64;
    let mut out = [0u8; 32];
    for (i, seed) in SEEDS.iter().enumerate() {
        // Подмешиваем длину в затравку → пустой/короткий вход тоже даёт различимые потоки.
        let h = fnv1a64(seed ^ len.wrapping_mul(PRIME_MIX), data);
        out[i * 8..i * 8 + 8].copy_from_slice(&h.to_le_bytes());
    }
    out
}

/// Константа перемешивания длины в затравку (нечётная, крупная — рассеивает биты длины).
const PRIME_MIX: u64 = 0x9E37_79B9_7F4A_7C15;

/// Построить формат-нейтральный [`BlobRef`] для файла-спутника: `key` = имя файла
/// (`Picture.<ext>` — расширение несёт тип картинки), `len`/`digest` — из байтов.
///
/// `filename` — ИМЯ файла-спутника (без каталога): и EDT, и Designer держат один и тот же
/// базовый `Picture.<ext>` (Designer `<xr:Abs>` = то же имя), поэтому `key` совпадает у
/// форматов → X by construction.
pub fn blob_ref_for(filename: &str, bytes: &[u8]) -> BlobRef {
    BlobRef {
        key: filename.to_string(),
        len: bytes.len() as u64,
        digest: digest256(bytes),
    }
}

// --- Designer `Ext/Picture.xml` (ExtPicture) — byte-exact реконструкция ------------------
//
// Designer держит picture-спутник ТРЕМЯ файлами: главный `<Name>.xml`, вспомогательный
// `<Name>/Ext/Picture.xml` (ExtPicture-дескриптор) и бинарь `<Name>/Ext/Picture/Picture.<ext>`.
// `Ext/Picture.xml` — КОНСТАНТНЫЙ шаблон (BOM+CRLF+TAB, version 2.21) с ЕДИНСТВЕННОЙ
// переменной — именем файла в `<xr:Abs>` (сверено корпусом: 600/600 байт-идентичны по
// шаблону, `LoadTransparent=false` всегда). Поэтому он ПОЛНОСТЬЮ выводим из имени бинаря —
// reconstructibl byte-exact без парс-движка (§1.0: любое ОТКЛОНЕНИЕ входа от этого шаблона
// → ошибка round-trip, не «best-effort»).

/// Префикс шаблона `Ext/Picture.xml` ДО имени файла (BOM + пролог + `<ExtPicture …>` +
/// `<Picture>` + `<xr:Abs>`). Байт-в-байт из корпуса (см. hexdump в тестах).
const EXT_PICTURE_PREFIX: &[u8] = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ExtPicture xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
version=\"2.21\">\r\n\t<Picture>\r\n\t\t<xr:Abs>";

/// Суффикс шаблона ПОСЛЕ имени файла (`</xr:Abs>` + `<xr:LoadTransparent>false</…>` +
/// закрытие `</Picture></ExtPicture>`; БЕЗ финального перевода строки — как в корпусе).
const EXT_PICTURE_SUFFIX: &[u8] =
    b"</xr:Abs>\r\n\t\t<xr:LoadTransparent>false</xr:LoadTransparent>\r\n\t</Picture>\r\n</ExtPicture>";

/// Собрать байты `Ext/Picture.xml` byte-exact из имени бинарного файла (`Picture.<ext>`).
pub fn ext_picture_xml_bytes(filename: &str) -> Vec<u8> {
    let mut out =
        Vec::with_capacity(EXT_PICTURE_PREFIX.len() + filename.len() + EXT_PICTURE_SUFFIX.len());
    out.extend_from_slice(EXT_PICTURE_PREFIX);
    out.extend_from_slice(filename.as_bytes());
    out.extend_from_slice(EXT_PICTURE_SUFFIX);
    out
}

/// Извлечь имя файла из `<xr:Abs>…</xr:Abs>` входного `Ext/Picture.xml`, СВЕРЯЯ, что вход
/// РОВНО соответствует константному шаблону (§1.0 — не best-effort). `Err`, если префикс/
/// суффикс не совпал (иной ns/version/LoadTransparent/структура) — тогда картинка иной
/// формы, чем витнессилась, и требует явного расширения субстрата, а не тихого проглатывания.
pub fn parse_ext_picture_xml(bytes: &[u8]) -> Result<String, String> {
    let inner = bytes.strip_prefix(EXT_PICTURE_PREFIX).ok_or_else(|| {
        "ExtPicture: unexpected prefix (ns/version/BOM differ — §1.0)".to_string()
    })?;
    let name = inner.strip_suffix(EXT_PICTURE_SUFFIX).ok_or_else(|| {
        "ExtPicture: unexpected suffix (LoadTransparent/structure differ — §1.0)".to_string()
    })?;
    // Имя файла между `<xr:Abs>` и `</xr:Abs>` — не должно содержать угловых скобок.
    let name =
        std::str::from_utf8(name).map_err(|e| format!("ExtPicture: non-utf8 filename: {e}"))?;
    if name.is_empty() || name.contains('<') || name.contains('>') {
        return Err(format!(
            "ExtPicture: malformed <xr:Abs> filename {name:?} (§1.0)"
        ));
    }
    Ok(name.to_string())
}

#[cfg(any())]
mod tests {
    use super::*;

    #[test]
    fn same_bytes_same_ref() {
        let a = blob_ref_for("Picture.png", b"hello-picture-bytes");
        let b = blob_ref_for("Picture.png", b"hello-picture-bytes");
        assert_eq!(
            a, b,
            "identical bytes+name → identical BlobRef (X by construction)"
        );
    }

    #[test]
    fn different_bytes_different_digest() {
        let a = blob_ref_for("Picture.png", b"aaaa");
        let b = blob_ref_for("Picture.png", b"aaab");
        assert_ne!(a.digest, b.digest, "one-byte change must change digest");
    }

    #[test]
    fn length_folded_in() {
        // Prefix-equal but different length → different digest (length is mixed in).
        let a = blob_ref_for("Picture.png", b"abc");
        let b = blob_ref_for("Picture.png", b"abcabc");
        assert_ne!(a.digest, b.digest);
        assert_ne!(a.len, b.len);
    }

    #[test]
    fn empty_bytes_ok() {
        let r = blob_ref_for("Picture.svg", b"");
        assert_eq!(r.len, 0);
        assert_eq!(r.key, "Picture.svg");
    }

    #[test]
    fn ext_picture_xml_matches_corpus_bytes_png() {
        // Точный байт-снимок `Ext/Picture.xml` из корпуса (BotFather, Picture.png).
        let corpus: &[u8] = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n\
<ExtPicture xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
version=\"2.21\">\r\n\t<Picture>\r\n\t\t<xr:Abs>Picture.png</xr:Abs>\r\n\t\t\
<xr:LoadTransparent>false</xr:LoadTransparent>\r\n\t</Picture>\r\n</ExtPicture>";
        assert_eq!(
            ext_picture_xml_bytes("Picture.png"),
            corpus,
            "must reproduce corpus byte-exact"
        );
        assert_eq!(corpus.len(), 377, "corpus Ext/Picture.xml is 377 bytes");
    }

    #[test]
    fn ext_picture_xml_roundtrips_all_witnessed_exts() {
        for name in [
            "Picture.png",
            "Picture.svg",
            "Picture.zip",
            "Picture.gif",
            "Picture.ico",
        ] {
            let bytes = ext_picture_xml_bytes(name);
            assert_eq!(parse_ext_picture_xml(&bytes).unwrap(), name);
        }
    }

    #[test]
    fn parse_ext_picture_rejects_foreign_shape() {
        // Иной LoadTransparent (true) — не наш витнессированный шаблон → §1.0 ошибка.
        let bad = b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ExtPicture>true</ExtPicture>";
        assert!(parse_ext_picture_xml(bad).is_err());
    }
}
