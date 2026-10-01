//! Attach / emit the CommonPicture IMAGE BODY sidecar (`Picture.<ext>` EDT /
//! `Ext/Picture/<file>` + `Ext/Picture.xml` wrapper Designer) as `obj.picture` during whole-config
//! read/write (§1.0/§1.6). Sibling of [`crate::mxl_read`] / [`crate::cmi_read`] — "the descriptor
//! read is metadata-only, the image BODY is a sibling file the pipeline attaches" — with the §1.0
//! OPTIONAL-body shape (a CommonPicture MAY be an image-less stub; absence is honest, never a hard
//! error).
//!
//! # Layout beside the descriptor (RE: coverage/s4_common CommonPictures — raw PNG byte-identical
//! across formats)
//! * **EDT** (`CommonPictures/<Name>/<Name>.mdo`, dir-per-object): sibling image
//!   `CommonPictures/<Name>/Picture.<ext>` (raw, no wrapper).
//! * **Designer** (`CommonPictures/<Name>.xml`, file-per-object): image
//!   `CommonPictures/<Name>/Ext/Picture/<file>` (raw) PLUS an XML wrapper
//!   `CommonPictures/<Name>/Ext/Picture.xml` = `<ExtPicture …><Picture><xr:Abs><file></xr:Abs>
//!   <xr:LoadTransparent>false</xr:LoadTransparent></Picture></ExtPicture>` (BOM, tab indent, CRLF,
//!   NO trailing newline — same wrapper convention as `Ext/CommandInterface.xml`).
//!
//! The raw image bytes are IDENTICAL across formats (§1.6) → the canonical IR ([`PictureBody`])
//! carries them plus the file name.
//!
//! # `LoadTransparent` / `TransparentPixel` — a FIELD, not a wrapper constant
//! `LoadTransparent=false` was previously baked as a wrapper CONSTANT (witnessed-only on
//! SSL/coverage). The ERP corpus falsified that: 30/2458 wrappers carry
//! `<xr:LoadTransparent>true</…>` PLUS `<xr:TransparentPixel x="10" y="7"/>` (witness
//! `ВажностиНовостей`; `ВажностьНовостиОченьВажная` — `x="14" y="0"`), and
//! `LoadTransparent=true` ⟺ `TransparentPixel` present (30/30 and 2428/2428 — a
//! DENORMALIZATION of pixel presence, not an independent flag). The pixel is the SAME
//! property EDT carries INSIDE the descriptor (`<transparentPixel><x/><y/>` — sparse
//! leaves, engine codec `formats_xml::transparent_pixel`), so this pass reads/writes the
//! Designer wrapper into that spec field (`F_TRANSPARENT_PIXEL`, mirror of the
//! `predefined` sidecar pattern; the cf `.0` body envelope `{1,0,x,y}` is the third
//! projection — `formats_cf::picture_body`). §1.0: `true` WITHOUT a pixel node (or vice
//! versa) is unwitnessed → typed error.

use std::path::{Path, PathBuf};

use formats_xml::Element;
use formats_xml::registry::Format;
use formats_xml::transparent_pixel::pixel_of;
use morph1c_core::ir::value::PropertyValue;
use morph1c_core::ir::{MetadataObject, PictureBody};
use morph1c_core::spec::metadata::common_picture::F_TRANSPARENT_PIXEL;

use crate::ConvertError;

/// Kinds whose object carries an image body (currently only `CommonPicture`).
const PICTURE_KINDS: &[&str] = &["CommonPicture"];

/// The EDT/Designer image file's stem (the extension carries the image format, e.g. `Picture.png`).
const PICTURE_STEM: &str = "Picture";

/// Load the CommonPicture image body (from its sidecar) into `obj.picture`.
///
/// `descriptor_path` — path of the descriptor just read (`.mdo`/`.xml`). §1.0: the body is OPTIONAL
/// (an image-less stub attaches nothing). Non-CommonPicture kinds and cf (container) are no-ops.
pub fn attach_picture_body(
    format: Format,
    kind: &str,
    descriptor_path: &Path,
    obj: &mut MetadataObject,
) -> Result<(), ConvertError> {
    if !PICTURE_KINDS.contains(&kind) {
        return Ok(());
    }
    let (file_name, image_path, pixel) = match picture_source(format, descriptor_path, &obj.name)? {
        Some(x) => x,
        None => return Ok(()), // cf: container, or an image-less stub — no-op.
    };
    let bytes = std::fs::read(&image_path).map_err(|e| ConvertError::Io {
        path: image_path.display().to_string(),
        reason: e.to_string(),
    })?;
    // §1.0: never silently overwrite an already-attached picture (the descriptor read must not
    // populate it — only this pass does).
    if obj.picture.is_some() {
        return Err(ConvertError::Read {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason: "object already carries a picture before the sidecar attach (unexpected — the \
                     descriptor projection must not populate it)"
                .into(),
        });
    }
    // Designer wrapper carried a transparent pixel → the SAME spec field the EDT descriptor
    // codec fills inline (mirror of the `predefined` sidecar pattern). §1.0: the Designer
    // DESCRIPTOR cannot populate it — an already-present field is a bug, not a merge.
    if let Some((x, y)) = pixel {
        if obj.get(F_TRANSPARENT_PIXEL).is_some() {
            return Err(ConvertError::Read {
                kind: kind.to_string(),
                object: obj.name.clone(),
                reason: "object already carries transparentPixel before the wrapper attach \
                         (unexpected — the Designer descriptor does not carry it)"
                    .into(),
            });
        }
        obj.properties.push((
            F_TRANSPARENT_PIXEL,
            PropertyValue::List(vec![PropertyValue::Int(x), PropertyValue::Int(y)]),
        ));
    }
    obj.picture = Some(PictureBody { file_name, bytes });
    Ok(())
}

/// Write-side mirror of [`attach_picture_body`]: emit the CommonPicture image body `obj` carries
/// beside its just-written descriptor `descriptor_out`, in the target format's layout. EDT writes
/// the raw image as `<obj-dir>/<file>`; Designer writes `<dir>/<Name>/Ext/Picture/<file>` (raw) +
/// the byte-exact `<dir>/<Name>/Ext/Picture.xml` wrapper. No-op for non-CommonPicture kinds, cf,
/// and image-less objects. §1.0: typed [`ConvertError`] on I/O.
pub fn write_picture_body(
    format: Format,
    kind: &str,
    descriptor_out: &Path,
    obj: &MetadataObject,
) -> Result<(), ConvertError> {
    if !PICTURE_KINDS.contains(&kind) {
        return Ok(());
    }
    // The transparent pixel (spec field, sparse bag) — the Designer wrapper is its ONLY
    // Designer-side carrier (see the module doc). §1.0: a malformed IR node is refused.
    let pixel = match obj.get(F_TRANSPARENT_PIXEL) {
        None => None,
        Some(v) => Some(pixel_of(v).map_err(|reason| ConvertError::Write {
            kind: kind.to_string(),
            object: obj.name.clone(),
            reason,
        })?),
    };
    let pic = match &obj.picture {
        Some(p) => p,
        None => {
            // §1.0: a pixel WITHOUT an image is unwitnessed, and on the Designer side the
            // wrapper (its only carrier) is not emitted for an image-less stub — dropping
            // it silently would lose data.
            if format == Format::Designer && pixel.is_some() {
                return Err(ConvertError::Write {
                    kind: kind.to_string(),
                    object: obj.name.clone(),
                    reason: "object carries transparentPixel but no image — unwitnessed, and \
                             the Designer wrapper (the pixel's only Designer carrier) is not \
                             emitted for an image-less stub (§1.0)"
                        .into(),
                });
            }
            return Ok(()); // OPTIONAL body — nothing attached, nothing to emit.
        }
    };
    match format {
        Format::Edt => {
            let obj_dir = descriptor_out
                .parent()
                .ok_or_else(|| no_parent(kind, obj))?;
            crate::form_write::write_file(&obj_dir.join(&pic.file_name), &pic.bytes)
        }
        Format::Designer => {
            let dir = descriptor_out
                .parent()
                .ok_or_else(|| no_parent(kind, obj))?;
            let stem = descriptor_out
                .file_stem()
                .ok_or_else(|| no_parent(kind, obj))?;
            let ext = dir.join(stem).join("Ext");
            crate::form_write::write_file(
                &ext.join(PICTURE_STEM).join(&pic.file_name),
                &pic.bytes,
            )?;
            crate::form_write::write_file(
                &ext.join("Picture.xml"),
                &serialize_wrapper(&pic.file_name, pixel),
            )
        }
        Format::Cf => Ok(()), // container — no file-per-object sidecar.
    }
}

fn no_parent(kind: &str, obj: &MetadataObject) -> ConvertError {
    ConvertError::Write {
        kind: kind.to_string(),
        object: obj.name.clone(),
        reason: "CommonPicture descriptor path has no parent directory (unexpected)".into(),
    }
}

/// `(file_name, raw-image path, transparent pixel)` beside the descriptor, per format. `None`
/// for cf and for an image-less stub (no image file / no Designer wrapper on disk). Designer
/// additionally validates the wrapper shape and yields its transparent pixel (§1.0); EDT
/// carries the pixel INSIDE the descriptor (engine codec) → always `None` here.
#[allow(clippy::type_complexity)]
fn picture_source(
    format: Format,
    descriptor_path: &Path,
    obj_name: &str,
) -> Result<Option<(String, PathBuf, Option<(i64, i64)>)>, ConvertError> {
    match format {
        // EDT: the image is the sibling file whose stem is `Picture` (extension = image format).
        Format::Edt => {
            let obj_dir = match descriptor_path.parent() {
                Some(d) => d,
                None => return Ok(None),
            };
            let mut found: Option<PathBuf> = None;
            let rd = match std::fs::read_dir(obj_dir) {
                Ok(rd) => rd,
                Err(_) => return Ok(None), // no dir ⇒ no image (honest).
            };
            for entry in rd {
                let path = entry
                    .map_err(|e| ConvertError::Io {
                        path: obj_dir.display().to_string(),
                        reason: e.to_string(),
                    })?
                    .path();
                if path.is_file() && path.file_stem().and_then(|s| s.to_str()) == Some(PICTURE_STEM)
                {
                    if found.is_some() {
                        return Err(ConvertError::Read {
                            kind: "CommonPicture".into(),
                            object: obj_name.into(),
                            reason: format!(
                                "multiple `{PICTURE_STEM}.*` image files beside the descriptor in {} \
                                 (only one witnessed, §1.0)",
                                obj_dir.display()
                            ),
                        });
                    }
                    found = Some(path);
                }
            }
            Ok(found.map(|p| (file_name_of(&p), p, None)))
        }
        // Designer: read the wrapper `<Name>/Ext/Picture.xml` for the referenced file name (and
        // the transparent pixel, if any), then the raw image `<Name>/Ext/Picture/<file>`.
        Format::Designer => {
            let dir = match descriptor_path.parent() {
                Some(d) => d,
                None => return Ok(None),
            };
            let stem = match descriptor_path.file_stem() {
                Some(s) => s,
                None => return Ok(None),
            };
            let ext = dir.join(stem).join("Ext");
            let wrapper = ext.join("Picture.xml");
            if !wrapper.is_file() {
                return Ok(None); // image-less stub.
            }
            let (file_name, pixel) = parse_wrapper(&wrapper, obj_name)?;
            Ok(Some((
                file_name.clone(),
                ext.join(PICTURE_STEM).join(file_name),
                pixel,
            )))
        }
        Format::Cf => Ok(None),
    }
}

/// Parse the Designer `Ext/Picture.xml` wrapper → `(referenced image file name, transparent
/// pixel)`. §1.0: the shape must be exactly `<ExtPicture><Picture><xr:Abs>NAME</xr:Abs>
/// <xr:LoadTransparent>false|true</xr:LoadTransparent>[<xr:TransparentPixel x="N" y="M"/>]
/// </Picture></ExtPicture>` with `LoadTransparent=true` ⟺ `TransparentPixel` present
/// (witnessed ERP 30/30 pixel-carriers + 2428/2428 plain — the flag is a denormalization of
/// pixel presence, not an independent bit). Shared with the config-level Ext pictures
/// (`crate::ext_read`: `Ext/<Slot>.xml` wrappers of `Splash`/`MainSectionPicture` carry the
/// IDENTICAL shape — verified byte-equal on s15; those refuse a pixel, unwitnessed there).
pub(crate) fn parse_wrapper(
    path: &Path,
    obj_name: &str,
) -> Result<(String, Option<(i64, i64)>), ConvertError> {
    let bytes = std::fs::read(path).map_err(|e| ConvertError::Io {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    let read_err = |reason: String| ConvertError::Read {
        kind: "CommonPicture".into(),
        object: obj_name.into(),
        reason,
    };
    let doc = formats_xml::parse(&bytes).map_err(|e| read_err(format!("Picture.xml XML: {e}")))?;
    let root = &doc.root;
    if root.local != "ExtPicture" {
        return Err(read_err(format!(
            "Picture.xml root is <{}>, expected <ExtPicture> (§1.0)",
            root.local
        )));
    }
    let picture = child(root, "Picture")
        .ok_or_else(|| read_err("Picture.xml has no <Picture> (§1.0)".into()))?;
    let abs =
        child(picture, "Abs").ok_or_else(|| read_err("<Picture> has no <xr:Abs> (§1.0)".into()))?;
    if abs.text.is_empty() {
        return Err(read_err("<xr:Abs> is empty (§1.0)".into()));
    }
    let lt = child(picture, "LoadTransparent")
        .ok_or_else(|| read_err("<Picture> has no <xr:LoadTransparent> (§1.0)".into()))?;
    let transparent = match lt.text.as_str() {
        "true" => true,
        "false" => false,
        other => {
            return Err(read_err(format!(
                "<xr:LoadTransparent> is {other:?}, expected true|false (§1.0)"
            )));
        }
    };
    let pixel_el = child(picture, "TransparentPixel");
    // §1.0: the flag is a denormalization of pixel presence — a contradicting pair is
    // unwitnessed (ERP 2458/2458) and refused, never guessed.
    let pixel = match (transparent, pixel_el) {
        (false, None) => None,
        (true, Some(px)) => Some(parse_pixel(px, &read_err)?),
        (true, None) => {
            return Err(read_err(
                "<xr:LoadTransparent>true</…> without <xr:TransparentPixel> — unwitnessed \
                 (the flag denormalizes pixel presence, §1.0)"
                    .into(),
            ));
        }
        (false, Some(_)) => {
            return Err(read_err(
                "<xr:TransparentPixel> with <xr:LoadTransparent>false</…> — unwitnessed \
                 (the flag denormalizes pixel presence, §1.0)"
                    .into(),
            ));
        }
    };
    Ok((abs.text.clone(), pixel))
}

/// `<xr:TransparentPixel x="N" y="M"/>` → `(x, y)`. §1.0: both attributes required (DENSE —
/// witnessed `x="14" y="0"`), strict integer parse, no children/text.
fn parse_pixel(
    el: &Element,
    read_err: &impl Fn(String) -> ConvertError,
) -> Result<(i64, i64), ConvertError> {
    if !el.children.is_empty() || !el.text.is_empty() {
        return Err(read_err(
            "<xr:TransparentPixel> must be an empty element (§1.0)".into(),
        ));
    }
    let coord = |name: &str| -> Result<i64, ConvertError> {
        let a = el.attr(name).ok_or_else(|| {
            read_err(format!(
                "<xr:TransparentPixel> has no @{name} (DENSE attrs witnessed, §1.0)"
            ))
        })?;
        a.value.parse().map_err(|e| {
            read_err(format!(
                "<xr:TransparentPixel> @{name}={:?} is not an integer: {e} (§1.0)",
                a.value
            ))
        })
    };
    Ok((coord("x")?, coord("y")?))
}

/// First child of `el` by local name (prefix-agnostic — mirrors `Element::child`).
fn child<'a>(el: &'a Element, local: &str) -> Option<&'a Element> {
    el.children.iter().find(|c| c.local == local)
}

/// The file-name component of `path` as an owned `String` (lossy is impossible here — the corpus
/// names are UTF-8; a non-UTF-8 name degrades to the lossy form rather than panicking).
fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Serialize the Designer `Ext/Picture.xml` wrapper BYTE-EXACT (RE: s4 `Picture.xml`, 377 B;
/// ERP `ВажностиНовостей` for the pixel-carrying shape): leading UTF-8 BOM, tab indent, CRLF,
/// NO trailing newline, four namespaces + `version=` of the ambient round-trip target
/// (`crate::sidecar_version::write_target` — SSL `2.21` by default, ERP target → `2.20`;
/// FORMATS.md §1: the output format is a parameter). `LoadTransparent` = pixel presence
/// (denormalization, witnessed 2458/2458); the pixel itself is the DENSE
/// `<xr:TransparentPixel x="N" y="M"/>` attr pair. Shared with the config-level Ext pictures
/// (`crate::ext_read` — `Ext/<Slot>.xml` wrappers are byte-identical in shape, RE s15;
/// those always pass `pixel=None`).
pub(crate) fn serialize_wrapper(file_name: &str, pixel: Option<(i64, i64)>) -> Vec<u8> {
    let version = formats_designer::common::profile_for(crate::sidecar_version::write_target())
        .map(|p| p.version_value)
        // Реестр всегда несёт таргет (`write_target` возвращает только witnessed-версии);
        // страховочный дефолт — SSL (историческое поведение этой обёртки).
        .unwrap_or("2.21");
    let mut s = String::new();
    s.push('\u{FEFF}'); // UTF-8 BOM.
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n");
    s.push_str(
        "<ExtPicture xmlns=\"http://v8.1c.ru/8.3/xcf/extrnprops\" \
         xmlns:xr=\"http://v8.1c.ru/8.3/xcf/readable\" \
         xmlns:xs=\"http://www.w3.org/2001/XMLSchema\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" version=\"",
    );
    s.push_str(version);
    s.push_str("\">\r\n");
    s.push_str("\t<Picture>\r\n\t\t<xr:Abs>");
    s.push_str(file_name);
    s.push_str("</xr:Abs>\r\n\t\t<xr:LoadTransparent>");
    s.push_str(if pixel.is_some() { "true" } else { "false" });
    s.push_str("</xr:LoadTransparent>");
    if let Some((x, y)) = pixel {
        s.push_str(&format!(
            "\r\n\t\t<xr:TransparentPixel x=\"{x}\" y=\"{y}\"/>"
        ));
    }
    s.push_str("\r\n\t</Picture>\r\n</ExtPicture>");
    s.into_bytes()
}

#[cfg(any())]
mod tests {
    use super::*;
    use morph1c_core::ir::{ObjectKind, Uuid};

    #[test]
    fn non_picture_kind_is_noop() {
        let mut obj = MetadataObject::new(ObjectKind::new("Constant"), "C", Uuid([0; 16]));
        attach_picture_body(
            Format::Edt,
            "Constant",
            Path::new("/nope/C/C.mdo"),
            &mut obj,
        )
        .unwrap();
        assert!(obj.picture.is_none());
    }

    #[test]
    fn wrapper_round_trips_through_parse() {
        // serialize → write to a temp Designer layout → parse back → same file name + shape ok.
        let base = std::env::temp_dir().join(format!(
            "morph1c-pic-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let ext = base.join("CommonPictures").join("Кар").join("Ext");
        std::fs::create_dir_all(&ext).unwrap();
        crate::fsio::write(
            ext.join("Picture.xml"),
            serialize_wrapper("Picture.png", None),
        )
        .unwrap();
        let (name, pixel) = parse_wrapper(&ext.join("Picture.xml"), "Кар").unwrap();
        assert_eq!(name, "Picture.png");
        assert_eq!(pixel, None);
        let _ = std::fs::remove_dir_all(&base);
    }

    /// ERP-witnessed pixel-carrying shape (`ВажностиНовостей` x=10,y=7;
    /// `ВажностьНовостиОченьВажная` y=0 stays DENSE in the wrapper) round-trips.
    #[test]
    fn wrapper_with_pixel_round_trips() {
        let base = std::env::temp_dir().join(format!("morph1c-pic-px-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        for px in [(10i64, 7i64), (14, 0)] {
            crate::fsio::write(
                base.join("Picture.xml"),
                serialize_wrapper("Picture.png", Some(px)),
            )
            .unwrap();
            let (name, pixel) = parse_wrapper(&base.join("Picture.xml"), "Кар").unwrap();
            assert_eq!(name, "Picture.png");
            assert_eq!(pixel, Some(px));
        }
        // The emitted wrapper carries the witnessed markup verbatim.
        let bytes = serialize_wrapper("Picture.png", Some((10, 7)));
        let text = String::from_utf8(bytes).unwrap();
        assert!(
            text.contains("<xr:LoadTransparent>true</xr:LoadTransparent>\r\n\t\t<xr:TransparentPixel x=\"10\" y=\"7\"/>"),
            "got {text}"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    /// §1.0: `true` WITHOUT a pixel (and a pixel with `false`) are unwitnessed — the flag
    /// denormalizes pixel presence (ERP 2458/2458) — refused, not guessed.
    #[test]
    fn wrapper_rejects_contradicting_transparency() {
        let base = std::env::temp_dir().join(format!("morph1c-pic-lt-{}", std::process::id()));
        std::fs::create_dir_all(&base).unwrap();
        let xml = b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ExtPicture xmlns:xr=\"x\"><Picture><xr:Abs>P.png</xr:Abs><xr:LoadTransparent>true</xr:LoadTransparent></Picture></ExtPicture>";
        crate::fsio::write(base.join("Picture.xml"), xml).unwrap();
        let err = parse_wrapper(&base.join("Picture.xml"), "Кар").unwrap_err();
        match err {
            ConvertError::Read { reason, .. } => {
                assert!(reason.contains("LoadTransparent"), "got {reason}")
            }
            other => panic!("expected Read, got {other:?}"),
        }
        let xml2 = b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<ExtPicture xmlns:xr=\"x\"><Picture><xr:Abs>P.png</xr:Abs><xr:LoadTransparent>false</xr:LoadTransparent><xr:TransparentPixel x=\"1\" y=\"2\"/></Picture></ExtPicture>";
        crate::fsio::write(base.join("Picture.xml"), xml2).unwrap();
        let err = parse_wrapper(&base.join("Picture.xml"), "Кар").unwrap_err();
        match err {
            ConvertError::Read { reason, .. } => {
                assert!(reason.contains("TransparentPixel"), "got {reason}")
            }
            other => panic!("expected Read, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}

/// Resolve form reference transparency only after every CommonPicture descriptor
/// and image wrapper has been typed. Designer-origin per-use flags/pixels are
/// preserved; the carrier is consulted only by EDT projection validation.
pub(crate) fn resolve_form_picture_transparency(
    format: Format,
    cfg: &mut morph1c_core::ir::Configuration,
) -> Result<(), ConvertError> {
    use std::collections::BTreeMap;
    fn collect(
        objects: &[MetadataObject],
        out: &mut BTreeMap<String, bool>,
    ) -> Result<(), ConvertError> {
        for object in objects {
            if object.kind.as_str() == "CommonPicture" {
                let pixel = object
                    .get(F_TRANSPARENT_PIXEL)
                    .map(pixel_of)
                    .transpose()
                    .map_err(|reason| ConvertError::Read {
                        kind: "CommonPicture".into(),
                        object: object.name.clone(),
                        reason,
                    })?;
                let reference = format!("CommonPicture.{}", object.name);
                if out.insert(reference, pixel.is_some()).is_some() {
                    return Err(ConvertError::Read {
                        kind: "CommonPicture".into(),
                        object: object.name.clone(),
                        reason: "duplicate CommonPicture reference identity".into(),
                    });
                }
            }
            collect(&object.children, out)?;
        }
        Ok(())
    }
    fn bind(
        objects: &mut [MetadataObject],
        defaults: &BTreeMap<String, bool>,
        edt: bool,
    ) -> Result<(), ConvertError> {
        for object in objects {
            let declared: Vec<_> = object
                .form_bodies
                .iter()
                .map(|form| crate::form_read::declared_form_uuid(object, &form.name))
                .collect::<Result<_, _>>()?;
            for (form, uuid) in object.form_bodies.iter_mut().zip(declared) {
                if form.ordinary_body.is_none() {
                    formats_xml::form::resolve_common_picture_transparency(
                        &mut form.body,
                        defaults,
                        edt,
                    )
                    .map_err(|error| ConvertError::Read {
                        kind: object.kind.as_str().into(),
                        object: format!("{}.{}", object.name, form.name),
                        reason: error.to_string(),
                    })?;
                    formats_xml::form::bind_picture_semantics(&mut form.body, uuid, edt).map_err(
                        |error| ConvertError::Read {
                            kind: object.kind.as_str().into(),
                            object: format!("{}.{}", object.name, form.name),
                            reason: error.to_string(),
                        },
                    )?;
                }
            }
            bind(&mut object.children, defaults, edt)?;
        }
        Ok(())
    }
    let mut defaults = BTreeMap::new();
    collect(&cfg.objects, &mut defaults)?;
    bind(&mut cfg.objects, &defaults, format == Format::Edt)
}
