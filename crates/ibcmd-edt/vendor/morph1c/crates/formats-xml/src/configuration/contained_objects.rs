//! containedObjects — платформенные пары (classId, objectId) корня Configuration.

use super::*;

// ============================================================================
// containedObjects — платформенные пары (classId, objectId).
// EDT:      <containedObjects classId="…" objectId="…"/>  (сиблинги корня)
// Designer: <InternalInfo><xr:ContainedObject><xr:ClassId>…</xr:ClassId>
//             <xr:ObjectId>…</xr:ObjectId></xr:ContainedObject>…</InternalInfo>
// IR: List([ List([Str(classId), Str(objectId)]) , … ]). Пустой → List([]).
// ============================================================================

const CONTAINED_OBJECTS_EDT_TAG: &str = "containedObjects";

/// Декодировать containedObjects (по диалекту). Claim'ит РОВНО прочитанное.
pub fn decode_contained_objects(dialect: ConfigDialect, root: &Element) -> Decoded {
    match dialect {
        ConfigDialect::Edt => decode_contained_objects_edt(root),
        ConfigDialect::Designer => decode_contained_objects_designer(root),
    }
}

/// Claim containedObjects (то же, что читает decode).
pub fn claim_contained_objects(dialect: ConfigDialect, root: &Element) {
    let _ = decode_contained_objects(dialect, root);
}

fn decode_contained_objects_edt(root: &Element) -> Decoded {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for ch in root
        .children
        .iter()
        .filter(|c| c.local == CONTAINED_OBJECTS_EDT_TAG && c.prefix.is_empty())
    {
        if !ch.children.is_empty() || !ch.text.is_empty() {
            return Decoded::Error("<containedObjects> must be a self-closing leaf (§1.0)".into());
        }
        let class_id = match ch.attr("classId") {
            Some(a) => a,
            None => return Decoded::Error("<containedObjects> missing @classId".into()),
        };
        let object_id = match ch.attr("objectId") {
            Some(a) => a,
            None => return Decoded::Error("<containedObjects> missing @objectId".into()),
        };
        class_id.claimed.set(true);
        object_id.claimed.set(true);
        if let Some(extra) = ch.attrs.iter().find(|a| !a.claimed.get()) {
            return Decoded::Error(format!(
                "<containedObjects> unexpected attribute {:?} (§1.0)",
                extra.name
            ));
        }
        ch.claim();
        rows.push(vec![class_id.value.clone(), object_id.value.clone()]);
    }
    Decoded::Present(str_list(rows))
}

fn decode_contained_objects_designer(root: &Element) -> Decoded {
    let Some(cfg) = designer_config_wrapper(root) else {
        return Decoded::Present(str_list(Vec::new()));
    };
    // блок — <InternalInfo> под <Configuration>.
    let info = match cfg.child("InternalInfo") {
        Some(b) => b,
        None => return Decoded::Present(str_list(Vec::new())),
    };
    if !info.prefix.is_empty() || !info.attrs.is_empty() || !info.text.is_empty() {
        return Decoded::Error(
            "<InternalInfo> must be an unprefixed attribute-less container (§1.0)".into(),
        );
    }
    info.claim();
    let mut rows: Vec<Vec<String>> = Vec::new();
    for co in &info.children {
        if co.local != "ContainedObject" || co.prefix != "xr" {
            return Decoded::Error(format!(
                "<InternalInfo> child must be <xr:ContainedObject>, got <{}>",
                qname(co)
            ));
        }
        if !co.attrs.is_empty() || !co.text.is_empty() {
            return Decoded::Error(
                "<xr:ContainedObject> must be attribute-less, no text (§1.0)".into(),
            );
        }
        co.claim();
        let mut it = co.children.iter();
        let class_id = match take_xr_leaf(it.next(), "ClassId") {
            Ok(s) => s,
            Err(e) => return Decoded::Error(e),
        };
        let object_id = match take_xr_leaf(it.next(), "ObjectId") {
            Ok(s) => s,
            Err(e) => return Decoded::Error(e),
        };
        if let Some(extra) = it.next() {
            return Decoded::Error(format!(
                "<xr:ContainedObject> unexpected extra child <{}> (§1.0)",
                qname(extra)
            ));
        }
        rows.push(vec![class_id, object_id]);
    }
    Decoded::Present(str_list(rows))
}

fn take_xr_leaf(el: Option<&Element>, local: &str) -> Result<String, String> {
    let el = el.ok_or_else(|| format!("<xr:ContainedObject> missing <xr:{local}>"))?;
    if el.local != local || el.prefix != "xr" {
        return Err(format!("expected <xr:{local}>, got <{}>", qname(el)));
    }
    if !el.attrs.is_empty() || !el.children.is_empty() {
        return Err(format!("<xr:{local}> must be a plain text leaf"));
    }
    el.claim_with_text();
    Ok(el.text.clone())
}

/// Эмитировать containedObjects (по диалекту) — несколько узлов (EDT) либо один
/// `<InternalInfo>` (Designer; ПУСТОЙ блок НЕ эмитится — корень без contained → нет
/// `<InternalInfo>`, сверено).
pub fn emit_contained_objects(
    dialect: ConfigDialect,
    value: &PropertyValue,
) -> Result<Vec<OutElement>, String> {
    let rows = unpack_rows(value)?;
    for r in &rows {
        if r.len() != 2 {
            return Err(format!(
                "containedObjects row must be [classId, objectId], got {} cells",
                r.len()
            ));
        }
    }
    match dialect {
        ConfigDialect::Edt => Ok(rows
            .into_iter()
            .map(|r| {
                OutElement::self_closing("", CONTAINED_OBJECTS_EDT_TAG)
                    .attr("classId", r[0].to_string())
                    .attr("objectId", r[1].to_string())
            })
            .collect()),
        ConfigDialect::Designer => {
            if rows.is_empty() {
                return Ok(Vec::new());
            }
            let mut info = OutElement::branch("", "InternalInfo");
            for r in rows {
                let mut co = OutElement::branch("xr", "ContainedObject");
                co.push(OutElement::leaf("xr", "ClassId", r[0].to_string()));
                co.push(OutElement::leaf("xr", "ObjectId", r[1].to_string()));
                info.push(co);
            }
            Ok(vec![info])
        }
    }
}
