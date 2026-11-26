//! Attach the GraphicalSchema TEMPLATE-BODY SIDECAR (`Template.scheme` EDT / `Ext/Template.xml`
//! Designer) + the item PICTURE sidecars into the IR `templates` during whole-config read, and
//! re-emit them on write (§1.0/§1.3/§1.6). Sibling of [`crate::geos_read`] (GeographicalSchema)
//! but for `templateType == GraphicalSchema`; the XML dialect codec is SHARED with
//! [`crate::flowchart_read`] — the sidecar root is the same `<GraphicalSchema>` (xcf/scheme)
//! document a BusinessProcess flowchart carries, with the SAME dialect skew (RE ERP: 37
//! designer/edt sidecar pairs):
//! * **EDT** — `Templates/<T>/Template.scheme`: БЕЗ BOM, корень БЕЗ `version=`, CRLF и в
//!   разметке, и ВНУТРИ текстовых узлов;
//! * **Designer** — `Templates/<T>/Ext/Template.xml`: С BOM, корень несёт `version="2.20"`
//!   ПОСЛЕДНИМ атрибутом, текстовые узлы LF.
//!
//! КАНОН (== IR `Template::body`) = флоучартный: без BOM, без `version=`, разметка CRLF,
//! текстовые узлы LF ([`crate::flowchart_read::canonicalize`]); оба диалекта дают РАВНЫЙ канон
//! (проверено на ERP-паре `Доходы`: edt == designer-канон побайтно).
//!
//! # Картинки элементов (`Template::resources`)
//! Декорация с `<Picture><Abs>Picture.<ext></Abs>…` несёт СЫРОЙ файл-сайдкар:
//! * EDT `Templates/<T>/Items/<Имя>/Picture.<ext>`;
//! * Designer `Templates/<T>/Ext/Template/Items/<Имя>/Picture.<ext>`
//!
//! — байты между диалектами БАЙТ-ИДЕНТИЧНЫ (blob; проверено на ERP `Доходы` png+jpg). Read
//! собирает КАЖДЫЙ файл, на который ссылается схема (ключ `<Имя>/<файл>` — им же cf-энкодер
//! адресует инлайн-ячейку); отсутствующий файл — типизированный отказ (молчаливая потеря
//! картинки дала бы неверное тело). Write раскладывает их обратно в раскладку целевого диалекта.
//!
//! # §1.0
//! Тело ОПЦИОНАЛЬНО: GraphicalSchema-стаб БЕЗ сайдкара не присоединяет ничего (coverage s12 —
//! у его графсхемы-макета сайдкара НЕТ и эталонный cf хранит её descriptor-only; гейт «тело ⟺
//! сайдкар» не двигает s12). Присоединённый сайдкар обязан пере-сериализоваться в ИСХОДНЫЕ
//! байты (self-check flowchart_read) — иначе громкий отказ.

use std::path::{Path, PathBuf};

use formats_xml::registry::Format;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, Template};
use morph1c_core::spec::metadata::common_template::F_TEMPLATE_TYPE;

use crate::ConvertError;

/// Kinds whose OBJECT carries a graphical-schema template-body sidecar (currently only
/// `CommonTemplate` — зеркало geos/mxl; в ERP носители только subordinate, но раскладка
/// объектного уровня симметрична остальным типам макетов).
const GS_SIDECAR_KINDS: &[&str] = &["CommonTemplate"];

/// Литерал `templateType = GraphicalSchema`.
const TEMPLATE_TYPE_GRAPHICAL_SCHEMA: &str = "GraphicalSchema";

/// EDT sidecar file name (сиблинг `.mdo` / внутри `Templates/<T>/`).
const GS_SIDECAR_EDT: &str = "Template.scheme";
/// Designer sidecar file name (внутри `<T>/Ext/`; то же имя, что у MXL/DCS/geos —
/// дизамбигуация по `templateType`).
const GS_SIDECAR_DESIGNER: &str = "Template.xml";

/// Подкаталог картинок относительно КАТАЛОГА сайдкара: EDT `Items/…` (рядом с
/// `Template.scheme`), Designer `Template/Items/…` (рядом с `Ext/Template.xml`).
fn items_root(format: Format, sidecar: &Path) -> Option<PathBuf> {
    let dir = sidecar.parent()?;
    Some(match format {
        Format::Edt => dir.join("Items"),
        Format::Designer => dir.join("Template").join("Items"),
        Format::Cf => return None,
    })
}

/// `templateType` объекта/ребёнка — GraphicalSchema?
fn is_graphical(obj: &MetadataObject) -> bool {
    matches!(obj.get(F_TEMPLATE_TYPE), Some(PropertyValue::Enum(t)) if t.as_str() == TEMPLATE_TYPE_GRAPHICAL_SCHEMA)
}

/// Load the GraphicalSchema template bodies (object + `*.TemplateRef` children) from their
/// sidecars — канон схемы в `Template::body`, картинки в `Template::resources`. Оба XML-диалекта;
/// cf — no-op (контейнер). NB (call-order): Designer-источникам нужен
/// [`crate::template_ref_read`]-энричмент ПЕРЕД этим проходом (bare-ref ребёнок не несёт
/// `templateType`).
pub fn attach_graph_template_bodies(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    // (a) `*.TemplateRef` children.
    for child in obj.children.iter_mut() {
        if !crate::template_read::is_template_ref_child(child) || !is_graphical(child) {
            continue;
        }
        let path = match crate::template_read::child_sidecar_path(
            format,
            descriptor_path,
            &child.name,
            GS_SIDECAR_EDT,
            GS_SIDECAR_DESIGNER,
        ) {
            Some(p) => p,
            None => continue, // no parent/stem (defensive).
        };
        attach_one(format, &path, child)?;
    }
    // (b) CommonTemplate object-level body.
    if !GS_SIDECAR_KINDS.contains(&kind) || !is_graphical(obj) {
        return Ok(());
    }
    let path = match object_sidecar_path(format, descriptor_path) {
        Some(p) => p,
        None => return Ok(()),
    };
    attach_one(format, &path, obj)
}

/// Присоединить ОДИН сайдкар (объект ИЛИ ребёнок) в `target.templates`. Отсутствующий файл —
/// no-op (bodyless-стаб, s12). §1.0: self-check пере-сериализации + обязательные байты каждой
/// картинки, на которую ссылается схема.
fn attach_one(
    format: Format,
    sidecar: &Path,
    target: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !sidecar.is_file() {
        return Ok(()); // OPTIONAL body — descriptor-only stub (s12).
    }
    let kind = target.kind.as_str().to_string();
    let read_err = |reason: String| ConvertError::Read {
        kind: kind.clone(),
        object: target.name.clone(),
        reason,
    };
    let bytes = std::fs::read(sidecar).map_err(|e| ConvertError::Io {
        path: sidecar.display().to_string(),
        reason: e.to_string(),
    })?;
    let (canon, src_version) = crate::flowchart_read::canonicalize(format, &bytes)
        .map_err(|e| read_err(format!("graphical template {}: {e}", sidecar.display())))?;
    // §1.0-самопроверка: канон обязан пере-сериализоваться в ИСХОДНЫЕ байты (Designer — версией
    // ИСТОЧНИКА; версия — свойство файла, §1.6) — как у flowchart_read.
    let back = match (format, src_version) {
        (Format::Designer, Some(v)) => crate::flowchart_read::serialize_designer(&canon, v),
        _ => crate::flowchart_read::serialize(format, &canon),
    };
    if back != bytes {
        return Err(read_err(format!(
            "graphical template {} does not round-trip byte-exactly through the IR (the sidecar \
             carries a byte-shape this codec does not model — refusing to silently normalize it, \
             §1.0)",
            sidecar.display()
        )));
    }
    let resources = read_referenced_pictures(format, sidecar, &canon, &kind, &target.name)?;
    // §1.0: только этот проход населяет templates графсхемы.
    if !target.templates.is_empty() {
        return Err(read_err(
            "object already carries templates before the graphical-schema sidecar attach \
             (unexpected — only this pass populates a GraphicalSchema body)"
                .into(),
        ));
    }
    target.templates.push(Template {
        pages: Vec::new(),
        resources,
        name: target.name.clone(),
        properties: Vec::new(),
        body: Some(canon),
    });
    Ok(())
}

/// Собрать байты КАЖДОЙ картинки, на которую ссылается схема (`<Picture><Abs>файл</Abs>`),
/// ключ = `<ИмяЭлемента>/<файл>`. Отсутствующий файл — типизированный отказ (§1.0).
fn read_referenced_pictures(
    format: Format,
    sidecar: &Path,
    canon: &[u8],
    kind: &str,
    object: &str,
) -> Result<Vec<(String, Vec<u8>)>, ConvertError> {
    let refs = picture_refs(canon).map_err(|reason| ConvertError::Read {
        kind: kind.to_string(),
        object: object.to_string(),
        reason: format!("graphical template {}: {reason}", sidecar.display()),
    })?;
    if refs.is_empty() {
        return Ok(Vec::new());
    }
    let root = items_root(format, sidecar).ok_or_else(|| ConvertError::Read {
        kind: kind.to_string(),
        object: object.to_string(),
        reason: "graphical template sidecar path has no parent to anchor Items/ at".into(),
    })?;
    let mut out = Vec::with_capacity(refs.len());
    for (item, file) in refs {
        let path = root.join(&item).join(&file);
        let bytes = std::fs::read(&path).map_err(|e| ConvertError::Read {
            kind: kind.to_string(),
            object: object.to_string(),
            reason: format!(
                "graphical template item {item:?} references picture {file:?}, but its sidecar \
                 {} is unreadable ({e}) — dropping it silently would emit a wrong body (§1.0)",
                path.display()
            ),
        })?;
        out.push((format!("{item}/{file}"), bytes));
    }
    Ok(out)
}

/// Пары `(<ИмяЭлемента>, <файл из <Abs>>)` схемы в порядке элементов.
fn picture_refs(canon: &[u8]) -> Result<Vec<(String, String)>, String> {
    let doc = formats_xml::parse(canon).map_err(|e| format!("sidecar XML does not parse: {e}"))?;
    let mut out = Vec::new();
    let Some(items) = doc.root.child("Items") else {
        return Ok(out);
    };
    for it in &items.children {
        let Some(props) = it.child("Properties") else {
            continue;
        };
        let Some(pic) = props.child("Picture") else {
            continue;
        };
        let Some(abs) = pic.child("Abs") else {
            continue;
        };
        let file = abs.text.trim();
        if file.is_empty() {
            return Err(format!(
                "item <{}> carries an EMPTY <Picture>/<Abs> file ref (§1.0)",
                it.local
            ));
        }
        let name = props
            .child("Name")
            .map(|n| n.text.trim().to_string())
            .ok_or_else(|| format!("item <{}> has no <Name> (§1.0)", it.local))?;
        out.push((name, file.to_string()));
    }
    Ok(out)
}

/// Write-side mirror of [`attach_graph_template_bodies`]: emit each attached GraphicalSchema
/// body beside the just-written descriptor in the TARGET dialect (схема —
/// [`crate::flowchart_read::serialize`], Designer штампуется версией амбиентного round-trip
/// таргета) + каждую картинку в раскладке целевого диалекта. No-op для bodyless и cf.
pub fn write_graph_template_bodies(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if format == Format::Cf {
        return Ok(());
    }
    // (a) `*.TemplateRef` children.
    for child in &obj.children {
        if !crate::template_read::is_template_ref_child(child) || !is_graphical(child) {
            continue;
        }
        let Some(template) = child.templates.first() else {
            continue; // bodyless stub — mirrors the read's optional attach.
        };
        if template.body.is_none() {
            continue;
        }
        if let Some(path) = crate::template_read::child_sidecar_path(
            format,
            descriptor_out,
            &child.name,
            GS_SIDECAR_EDT,
            GS_SIDECAR_DESIGNER,
        ) {
            write_one(format, &path, template)?;
        }
    }
    // (b) CommonTemplate object-level body.
    if !GS_SIDECAR_KINDS.contains(&kind) || !is_graphical(obj) {
        return Ok(());
    }
    let Some(template) = obj.templates.first() else {
        return Ok(());
    };
    if template.body.is_none() {
        return Ok(());
    }
    let path = match object_sidecar_path(format, descriptor_out) {
        Some(p) => p,
        None => return Ok(()),
    };
    write_one(format, &path, template)
}

/// Эмитить ОДИН сайдкар (схема + картинки) в целевом диалекте.
fn write_one(format: Format, sidecar: &Path, template: &Template) -> Result<(), ConvertError> {
    let canon = template
        .body
        .as_deref()
        .expect("write_one is only called with an attached body");
    crate::form_write::write_file(sidecar, &crate::flowchart_read::serialize(format, canon))?;
    if template.resources.is_empty() {
        return Ok(());
    }
    let root = items_root(format, sidecar).ok_or_else(|| ConvertError::Io {
        path: sidecar.display().to_string(),
        reason: "graphical template sidecar path has no parent to anchor Items/ at".into(),
    })?;
    for (key, bytes) in &template.resources {
        // Ключ `<ИмяЭлемента>/<файл>` — ровно два компонента (см. read).
        let mut path = root.clone();
        for part in key.split('/') {
            path.push(part);
        }
        crate::form_write::write_file(&path, bytes)?;
    }
    Ok(())
}

/// Object-level sidecar path beside the descriptor, per format layout (== geos/mxl):
/// EDT `<obj-dir>/Template.scheme`, Designer `<dir>/<Name>/Ext/Template.xml`. `None` для cf.
fn object_sidecar_path(format: Format, descriptor_path: &Path) -> Option<PathBuf> {
    match format {
        Format::Edt => Some(descriptor_path.parent()?.join(GS_SIDECAR_EDT)),
        Format::Designer => {
            let dir = descriptor_path.parent()?;
            let stem = descriptor_path.file_stem()?;
            Some(dir.join(stem).join("Ext").join(GS_SIDECAR_DESIGNER))
        }
        Format::Cf => None,
    }
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::value::Token;
    use morph1c_core::ir::{ObjectKind, Uuid};

    /// Витнессированный канон-минимум (выжимка ERP `Доходы`): одна декорация с картинкой.
    const CANON: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
        "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\">\r\n",
        "\t<Items>\r\n",
        "\t\t<Decoration id=\"1\">\r\n",
        "\t\t\t<Properties>\r\n",
        "\t\t\t\t<Name>Декорация1</Name>\r\n",
        "\t\t\t\t<Picture>\r\n",
        "\t\t\t\t\t<Abs xmlns=\"http://v8.1c.ru/8.3/xcf/readable\">Picture.png</Abs>\r\n",
        "\t\t\t\t</Picture>\r\n",
        "\t\t\t</Properties>\r\n",
        "\t\t</Decoration>\r\n",
        "\t</Items>\r\n",
        "</GraphicalSchema>"
    );

    fn designer_bytes() -> Vec<u8> {
        let with_version = CANON.replacen(
            "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\">",
            "<GraphicalSchema xmlns=\"http://v8.1c.ru/8.3/xcf/scheme\" version=\"2.20\">",
            1,
        );
        let mut out = vec![0xEF, 0xBB, 0xBF];
        out.extend_from_slice(with_version.as_bytes());
        out
    }

    fn graphical_child(name: &str) -> MetadataObject {
        let mut c = MetadataObject::new(
            ObjectKind::new("Report.TemplateRef"),
            name,
            Uuid([9; 16]),
        );
        c.properties.push((
            F_TEMPLATE_TYPE,
            PropertyValue::Enum(Token::new(TEMPLATE_TYPE_GRAPHICAL_SCHEMA)),
        ));
        c
    }

    fn temp_base(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "morph1c-graphtmpl-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    #[test]
    fn designer_child_attach_canonicalizes_and_collects_pictures() {
        let base = temp_base("dz");
        let tdir = base
            .join("Reports")
            .join("Отчет")
            .join("Templates")
            .join("Макет");
        let ext = tdir.join("Ext");
        std::fs::create_dir_all(ext.join("Template").join("Items").join("Декорация1")).unwrap();
        crate::fsio::write(ext.join("Template.xml"), designer_bytes()).unwrap();
        crate::fsio::write(
            ext.join("Template/Items/Декорация1/Picture.png"),
            b"PNGBYTES",
        )
        .unwrap();
        let host = base.join("Reports").join("Отчет.xml");

        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет", Uuid([1; 16]));
        obj.children.push(graphical_child("Макет"));
        attach_graph_template_bodies(Format::Designer, "Report", &host, &mut obj).unwrap();
        let t = &obj.children[0].templates[0];
        assert_eq!(
            String::from_utf8(t.body.clone().unwrap()).unwrap(),
            CANON,
            "designer sidecar canonicalizes (BOM/version снят)"
        );
        assert_eq!(
            t.resources,
            vec![("Декорация1/Picture.png".to_string(), b"PNGBYTES".to_vec())]
        );

        // Write-side: designer→designer round-trips byte-exact (амбиентный таргет ERP=2.20).
        let out_host = base.join("out").join("Reports").join("Отчет.xml");
        morph1c_core::version::with_roundtrip_target(morph1c_core::version::ERP, || {
            write_graph_template_bodies(Format::Designer, "Report", &out_host, &obj)
        })
        .unwrap();
        let written =
            std::fs::read(base.join("out/Reports/Отчет/Templates/Макет/Ext/Template.xml"))
                .unwrap();
        assert_eq!(written, designer_bytes(), "designer write byte-exact");
        let pic = std::fs::read(
            base.join("out/Reports/Отчет/Templates/Макет/Ext/Template/Items/Декорация1/Picture.png"),
        )
        .unwrap();
        assert_eq!(pic, b"PNGBYTES");

        // …и EDT-запись кладёт схему+картинку в EDT-раскладку (CRLF в текст-узлах здесь нет).
        let edt_host = base.join("edt").join("Reports").join("Отчет").join("Отчет.mdo");
        write_graph_template_bodies(Format::Edt, "Report", &edt_host, &obj).unwrap();
        let edt = std::fs::read(
            base.join("edt/Reports/Отчет/Templates/Макет/Template.scheme"),
        )
        .unwrap();
        assert_eq!(String::from_utf8(edt).unwrap(), CANON, "EDT = канон (BOM/version нет)");
        assert!(base
            .join("edt/Reports/Отчет/Templates/Макет/Items/Декорация1/Picture.png")
            .is_file());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn edt_child_attach_yields_the_same_canon() {
        let base = temp_base("edt");
        let tdir = base
            .join("Reports")
            .join("Отчет")
            .join("Templates")
            .join("Макет");
        std::fs::create_dir_all(tdir.join("Items").join("Декорация1")).unwrap();
        crate::fsio::write(tdir.join("Template.scheme"), CANON.as_bytes()).unwrap();
        crate::fsio::write(tdir.join("Items/Декорация1/Picture.png"), b"PNGBYTES").unwrap();
        let host = base.join("Reports").join("Отчет").join("Отчет.mdo");

        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет", Uuid([1; 16]));
        obj.children.push(graphical_child("Макет"));
        attach_graph_template_bodies(Format::Edt, "Report", &host, &mut obj).unwrap();
        let t = &obj.children[0].templates[0];
        assert_eq!(String::from_utf8(t.body.clone().unwrap()).unwrap(), CANON);
        assert_eq!(t.resources.len(), 1, "картинка собрана и в EDT-раскладке");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn bodyless_graphical_child_attaches_nothing() {
        // s12-гейт: GraphicalSchema БЕЗ сайдкара остаётся descriptor-only (нет ошибки).
        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет", Uuid([1; 16]));
        obj.children.push(graphical_child("Макет"));
        let host = Path::new("/nonexistent-root/Reports/Отчет.xml");
        attach_graph_template_bodies(Format::Designer, "Report", host, &mut obj).unwrap();
        assert!(obj.children[0].templates.is_empty());
    }

    #[test]
    fn missing_referenced_picture_is_refused() {
        let base = temp_base("nopic");
        let ext = base
            .join("Reports")
            .join("Отчет")
            .join("Templates")
            .join("Макет")
            .join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        crate::fsio::write(ext.join("Template.xml"), designer_bytes()).unwrap();
        // Картинку НЕ кладём — схема на неё ссылается.
        let host = base.join("Reports").join("Отчет.xml");
        let mut obj = MetadataObject::new(ObjectKind::new("Report"), "Отчет", Uuid([1; 16]));
        obj.children.push(graphical_child("Макет"));
        let err =
            attach_graph_template_bodies(Format::Designer, "Report", &host, &mut obj).unwrap_err();
        assert!(
            err.to_string().contains("Picture.png"),
            "missing picture must refuse loudly, got: {err}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
