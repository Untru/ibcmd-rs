//! Ordered, complete five-field AllowedIncomingShareRequestType records.
use super::*;

const FIELDS: [&str; 5] = ["mime", "uti", "ext", "processingVariant", "isCustom"];

fn row(el: &Element, dialect: ConfigDialect) -> Result<Vec<String>, String> {
    if !el.text.is_empty() {
        return Err("share type record cannot contain mixed text".into());
    }
    let designer = dialect == ConfigDialect::Designer;
    if designer {
        if el.local != "Value"
            || el.prefix != "v8"
            || el.attrs.len() != 1
            || el
                .attr("xsi:type")
                .is_none_or(|a| a.value != "app:AllowedIncomingShareRequestType")
        {
            return Err(
                "share type requires v8:Value xsi:type=app:AllowedIncomingShareRequestType".into(),
            );
        }
        el.attr("xsi:type").unwrap().claimed.set(true);
    } else if !el.attrs.is_empty() {
        return Err("EDT share type record cannot have attributes".into());
    }
    let mut values = vec![
        String::new(),
        String::new(),
        String::new(),
        "0".into(),
        "false".into(),
    ];
    let mut seen = [false; 5];
    for ch in &el.children {
        let Some(i) = FIELDS.iter().position(|f| *f == ch.local) else {
            return Err("unknown share type field".into());
        };
        if seen[i] || ch.prefix != if designer { "app" } else { "" } || !ch.children.is_empty() {
            return Err("duplicate, qualified or nested share type field".into());
        }
        seen[i] = true;
        if designer && i == 3 {
            if ch.attrs.len() != 1 || ch.attr("xsi:type").is_none_or(|a| a.value != "xs:decimal") {
                return Err("share processingVariant requires xsi:type=xs:decimal".into());
            }
            ch.attr("xsi:type").unwrap().claimed.set(true);
        } else if !ch.attrs.is_empty() {
            return Err("unexpected share type field attribute".into());
        }
        values[i] = match (i, designer, ch.text.as_str()) {
            (3, false, "View") | (3, true, "0") => "0".into(),
            (3, false, "Edit") | (3, true, "1") => "1".into(),
            (3, _, _) => return Err("unknown share processingVariant".into()),
            (4, _, "true" | "false") => ch.text.clone(),
            (4, _, _) => return Err("invalid share isCustom boolean".into()),
            _ => ch.text.clone(),
        };
        ch.claim_with_text();
    }
    if designer && seen != [true; 5] {
        return Err("native share type must contain all five fields".into());
    }
    el.claim();
    Ok(values)
}

/// Decode all EDT siblings or the single native Configuration property container.
pub fn decode_share_types(dialect: ConfigDialect, root: &Element) -> Decoded {
    let result = (|| {
        let mut rows = Vec::new();
        if dialect == ConfigDialect::Edt {
            for el in root
                .children
                .iter()
                .filter(|e| e.local == "allowedIncomingShareRequestTypes" && e.prefix.is_empty())
            {
                rows.push(row(el, dialect)?);
            }
        } else {
            let owners: Vec<_> = root
                .children
                .iter()
                .filter(|e| e.local == "Configuration" && e.prefix.is_empty())
                .collect();
            if owners.len() != 1 {
                return Err("share types require single Configuration owner".into());
            }
            let props: Vec<_> = owners[0]
                .children
                .iter()
                .filter(|e| e.local == "Properties" && e.prefix.is_empty())
                .collect();
            if props.len() != 1 {
                return Err("share types require single Properties container".into());
            }
            let hosts: Vec<_> = props[0]
                .children
                .iter()
                .filter(|e| e.local == "AllowedIncomingShareRequestTypes" && e.prefix.is_empty())
                .collect();
            if hosts.len() > 1 {
                return Err("duplicate native share type container".into());
            }
            if let Some(host) = hosts.first() {
                if !host.attrs.is_empty() || !host.text.is_empty() {
                    return Err("invalid native share type container".into());
                }
                host.claim();
                for el in &host.children {
                    rows.push(row(el, dialect)?);
                }
            }
        }
        Ok(str_list(rows))
    })();
    match result {
        Ok(value) => Decoded::Present(value),
        Err(error) => Decoded::Error(error),
    }
}

/// Emit in authored record order; the native serializer materializes all five fields.
pub fn emit_share_types(
    dialect: ConfigDialect,
    value: &PropertyValue,
) -> Result<Vec<OutElement>, String> {
    let mut records = Vec::new();
    for cells in unpack_rows(value)? {
        if cells.len() != 5
            || !matches!(cells[3], "0" | "1")
            || !matches!(cells[4], "true" | "false")
        {
            return Err(
                "share type requires five fields and a valid processingVariant/isCustom".into(),
            );
        }
        let designer = dialect == ConfigDialect::Designer;
        let mut record = if designer {
            OutElement::branch("v8", "Value")
                .attr("xsi:type", "app:AllowedIncomingShareRequestType")
        } else {
            OutElement::branch("", "allowedIncomingShareRequestTypes")
        };
        for (i, field) in FIELDS.iter().enumerate() {
            if !designer
                && ((i < 3 && cells[i].is_empty())
                    || (i == 3 && cells[i] == "0")
                    || (i == 4 && cells[i] == "false"))
            {
                continue;
            }
            let text = if i == 3 && !designer {
                if cells[i] == "0" { "View" } else { "Edit" }
            } else {
                cells[i]
            };
            let mut leaf = if text.is_empty() {
                OutElement::self_closing(if designer { "app" } else { "" }, *field)
            } else {
                OutElement::leaf(if designer { "app" } else { "" }, *field, text)
            };
            if designer && i == 3 {
                leaf = leaf.attr("xsi:type", "xs:decimal");
            }
            record.push(leaf);
        }
        if record.children.is_empty() {
            record.self_closing = true;
        }
        records.push(record);
    }
    if dialect == ConfigDialect::Edt {
        return Ok(records);
    }
    let mut host = if records.is_empty() {
        OutElement::self_closing("", "AllowedIncomingShareRequestTypes")
    } else {
        OutElement::branch("", "AllowedIncomingShareRequestTypes")
    };
    for record in records {
        host.push(record);
    }
    Ok(vec![host])
}
