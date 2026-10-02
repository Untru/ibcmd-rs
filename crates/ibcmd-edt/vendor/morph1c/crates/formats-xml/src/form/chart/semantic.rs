//! Current typed chart state and source-only field layout. No previous values are retained.
use super::*;
use morph1c_core::ir::form::{
    ChartCompositeLayout, ChartLayoutSegment, ChartSourceFormat, ChartSourceLayout,
};

pub(super) fn from_source(
    kind: &str,
    fields: Vec<(String, ChartValue)>,
    format: ChartSourceFormat,
    source: &Element,
) -> Result<ChartSettings, FormError> {
    let mut composites = Vec::new();
    let fields = canonical_fields(
        fields,
        top_tbl(kind)?,
        format,
        &mut Vec::new(),
        &mut composites,
    )?;
    capture_primitives(
        source,
        top_tbl(kind)?,
        format,
        &mut Vec::new(),
        &mut composites,
    );
    Ok(ChartSettings {
        kind: kind.to_owned(),
        fields,
        source_layout: Some(ChartSourceLayout {
            format,
            composites,
            root_namespaces: if format == ChartSourceFormat::Edt {
                source
                    .attrs
                    .iter()
                    .filter(|a| a.name.starts_with("xmlns:"))
                    .map(|a| (a.name.clone(), a.value.clone()))
                    .collect()
            } else {
                Vec::new()
            },
            common_inline_paths: capture_common_namespace_paths(source),
        }),
    })
}

fn subtable(shape: Shape) -> Option<Tbl> {
    match shape {
        Shape::Nested(t) | Shape::Items(t) => Some(t),
        Shape::ChartTable => Some(Tbl::Chart),
        Shape::SeriesItem | Shape::SeriesItems => Some(Tbl::SeriesItem),
        Shape::PointItems => Some(Tbl::PointItem),
        _ => None,
    }
}

fn default_value(t: Tbl, row: &Row, format: ChartSourceFormat) -> Option<ChartValue> {
    if t == Tbl::SeriesItem && row.name == "info" {
        return Some(ChartValue::Absent);
    }
    if t == Tbl::SeriesCalc && matches!(row.name, "str" | "m_centerPoint") {
        return Some(ChartValue::Absent);
    }
    if t == Tbl::Interval && matches!(row.name, "leftDate" | "rightDate") {
        return Some(ChartValue::Absent);
    }
    if let Some(mut v) = super::sdk_defaults::scalar_default(t, row.name, format) {
        if let (Shape::Dec, ChartValue::Int(n)) = (row.shape, &mut v) {
            *n = designer_decimal(n);
        }
        if matches!(
            (row.shape, &v),
            (Shape::Bool, ChartValue::Bool(_))
                | (Shape::Int | Shape::Dec, ChartValue::Int(_))
                | (Shape::Str | Shape::DateTime, ChartValue::Str(_))
                | (Shape::Enum, ChartValue::Enum(_))
        ) {
            return Some(v);
        }
    }
    if matches!(t, Tbl::SeriesItem | Tbl::PointItem) && matches!(row.name, "valInfo" | "key") {
        return Some(ChartValue::Value(Box::new(
            morph1c_core::ir::form::ChartTypedValue::Scalar(
                morph1c_core::ir::value::PropertyValue::Value(morph1c_core::ir::value::ValueSpec {
                    kind: morph1c_core::ir::value::ValueScalarKind::Undefined,
                    scalar: None,
                }),
            ),
        )));
    }
    match row.shape {
        Shape::Picture => Some(ChartValue::Absent),
        Shape::Loc => Some(ChartValue::Localized(Vec::new())),
        Shape::Color => Some(ChartValue::Color("auto".into())),
        Shape::Font => Some(ChartValue::Font(auto_font())),
        Shape::SeriesItems
        | Shape::PointItems
        | Shape::Items(_)
        | Shape::DataItems
        | Shape::Colors => Some(ChartValue::Items(Vec::new())),
        // These fixed composite instances are created by the native versioned chart factory.
        // Their scalar defaults are decoded recursively, independently of source spelling.
        Shape::Nested(Tbl::Empty) => Some(ChartValue::Nested(Vec::new())),
        Shape::Nested(t2)
            if matches!(
                t2,
                Tbl::Axis | Tbl::Scale | Tbl::Cpd | Tbl::ReferenceLines | Tbl::ReferenceBands
            ) =>
        {
            Some(ChartValue::Nested(Vec::new()))
        }
        Shape::Nested(Tbl::Interval) => Some(ChartValue::Nested(vec![
            ("leftIsNum".into(), ChartValue::Bool(true)),
            ("rightIsNum".into(), ChartValue::Bool(true)),
        ])),
        _ => None,
    }
}

fn canonical_fields(
    fields: Vec<(String, ChartValue)>,
    t: Tbl,
    format: ChartSourceFormat,
    path: &mut Vec<ChartLayoutSegment>,
    layouts: &mut Vec<ChartCompositeLayout>,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    layouts.push(ChartCompositeLayout {
        path: path.clone(),
        fields: fields.iter().map(|(n, _)| n.clone()).collect(),
    });
    let mut remaining = fields;
    let mut out = Vec::new();
    let mut table: Vec<&Row> = rows(t).iter().collect();
    table.sort_by_key(|r| r.edt_rank);
    for row in table {
        let value = remaining
            .iter()
            .position(|(n, _)| n == row.name)
            .map(|i| remaining.remove(i).1)
            .or_else(|| default_value(t, row, format));
        let Some(mut value) = value else { continue };
        validate_primitive(row, &value)?;
        validate_number(t, row, &value)?;
        if let (Shape::Enum, ChartValue::Enum(v)) = (row.shape, &value) {
            if !super::sdk_defaults::enum_allowed(t, row.name, v) {
                return Err(frame(format!(
                    "chart: invalid enum literal for {}",
                    row.name
                )));
            }
        }
        if let ChartValue::Color(color) = &mut value {
            if color.starts_with('#') {
                *color = color.to_ascii_uppercase();
            }
        }
        if row.shape == Shape::Colors {
            let ChartValue::Items(items) = &mut value else {
                return Err(frame("chart: current palette must be typed Colors".into()));
            };
            for (i, fields) in items.iter_mut().enumerate() {
                let [(name, ChartValue::Color(color))] = fields.as_mut_slice() else {
                    return Err(frame(
                        "chart: current palette item requires one Color".into(),
                    ));
                };
                if name != "value" {
                    return Err(frame("chart: palette item has unknown field".into()));
                }
                check_color_canon(color, row.name)?;
                path.push(ChartLayoutSegment::Field(row.name.into()));
                path.push(ChartLayoutSegment::Item(i));
                let names = if color.starts_with('#') {
                    color
                        .bytes()
                        .skip(1)
                        .enumerate()
                        .filter(|(_, c)| c.is_ascii_lowercase())
                        .map(|(i, _)| format!("hexLower{i}"))
                        .collect()
                } else {
                    Vec::new()
                };
                layouts.push(ChartCompositeLayout {
                    path: path.clone(),
                    fields: names,
                });
                path.pop();
                path.pop();
                if color.starts_with('#') {
                    *color = color.to_ascii_uppercase();
                }
            }
        }
        if let ChartValue::Font(font) = &mut value {
            let mut fields = Vec::new();
            if font.face_name.is_some() {
                fields.push("faceName".into());
            }
            if font.height.is_some() {
                fields.push("height".into());
            }
            for (n, v) in [
                ("bold", font.bold),
                ("italic", font.italic),
                ("underline", font.underline),
                ("strikeout", font.strikeout),
            ] {
                if v.is_some() {
                    fields.push(n.into());
                }
            }
            if font.scale.is_some() {
                fields.push("scale".into());
            }
            path.push(ChartLayoutSegment::Field(row.name.into()));
            layouts.push(ChartCompositeLayout {
                path: path.clone(),
                fields,
            });
            path.pop();
            *font = normalize_chart_font(font.clone());
        }
        if let Some(sub) = subtable(row.shape) {
            path.push(ChartLayoutSegment::Field(row.name.to_owned()));
            value = match value {
                ChartValue::Nested(f) => {
                    ChartValue::Nested(canonical_fields(f, sub, format, path, layouts)?)
                }
                ChartValue::Items(items) => {
                    let mut out = Vec::new();
                    for (i, f) in items.into_iter().enumerate() {
                        path.push(ChartLayoutSegment::Item(i));
                        out.push(canonical_fields(f, sub, format, path, layouts)?);
                        path.pop();
                    }
                    ChartValue::Items(out)
                }
                other => other,
            };
            path.pop();
        }
        if matches!(row.shape, Shape::Dec) {
            if let ChartValue::Int(v) = &mut value {
                *v = designer_decimal(v);
            }
        }
        out.push((row.name.to_owned(), value));
    }
    if !remaining.is_empty() {
        return Err(frame(format!(
            "chart: unknown or duplicate current field {:?}",
            remaining[0].0
        )));
    }
    Ok(out)
}

pub(super) fn emission_fields(
    cs: &ChartSettings,
    format: ChartSourceFormat,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    project_fields(
        &cs.fields,
        top_tbl(&cs.kind)?,
        format,
        &mut Vec::new(),
        cs.source_layout.as_ref(),
    )
}

fn project_fields(
    fields: &[(String, ChartValue)],
    t: Tbl,
    format: ChartSourceFormat,
    path: &mut Vec<ChartLayoutSegment>,
    layout: Option<&ChartSourceLayout>,
) -> Result<Vec<(String, ChartValue)>, FormError> {
    let mut seen = std::collections::HashSet::new();
    for (n, v) in fields {
        if !seen.insert(n) {
            return Err(frame(format!("chart: duplicate current field {n:?}")));
        }
        let row = row_by_name(t, n)
            .ok_or_else(|| frame(format!("chart: unknown current field {n:?}")))?;
        validate_primitive(row, v)?;
        validate_number(t, row, v)?;
        if let (Shape::Enum, ChartValue::Enum(value)) = (row.shape, v) {
            if !super::sdk_defaults::enum_allowed(t, n, value) {
                return Err(frame(format!(
                    "chart: invalid current enum literal for {n}"
                )));
            }
        }
    }
    let source_layout = layout.and_then(|l| l.composites.iter().find(|c| c.path == *path));
    let original = if layout.is_some_and(|l| l.format == format) {
        source_layout
    } else {
        None
    };
    let mut names: Vec<&str> = original
        .map(|o| o.fields.iter().map(String::as_str).collect())
        .unwrap_or_default();
    for (n, _) in fields {
        if !names.contains(&n.as_str()) {
            names.push(n);
        }
    }
    if original.is_none() {
        names.sort_by_key(|n| match format {
            ChartSourceFormat::Edt => row_by_name(t, n).map_or(u16::MAX, |r| r.edt_rank) as usize,
            ChartSourceFormat::Designer => super::sdk_defaults::native_rank(t, n),
        });
    }
    let mut out = Vec::new();
    for n in names {
        let row = row_by_name(t, n)
            .ok_or_else(|| frame(format!("chart: unknown current field {n:?}")))?;
        let current = fields
            .iter()
            .find(|(name, _)| name == n)
            .map(|(_, v)| v.clone());
        // A deleted field can only regain its known default spelling, never an old value.
        let Some(mut value) = current.or_else(|| default_value(t, row, format)) else {
            continue;
        };
        if format == ChartSourceFormat::Designer
            && row.shape == Shape::Value
            && matches!(
                n,
                "semitransparencyPercent" | "borderSemitransparencyPercent"
            )
        {
            use morph1c_core::ir::form::ChartTypedValue;
            use morph1c_core::ir::value::{PropertyValue, ValueScalarKind};
            let undefined = matches!(&value,ChartValue::Value(v) if matches!(v.as_ref(),ChartTypedValue::Scalar(PropertyValue::Value(spec)) if spec.kind==ValueScalarKind::Undefined&&spec.scalar.is_none()));
            if undefined && layout.is_some_and(|l| l.format == ChartSourceFormat::Designer) {
                path.push(ChartLayoutSegment::Field(n.into()));
                let spelling = layout
                    .and_then(|l| l.composites.iter().find(|c| c.path == *path))
                    .and_then(|c| {
                        if c.fields == ["PositiveInfinity"] {
                            Some("Infinity")
                        } else if c.fields == ["NegativeInfinity"] {
                            Some("-Infinity")
                        } else {
                            None
                        }
                    });
                path.pop();
                if let Some(spelling) = spelling {
                    value = ChartValue::Value(Box::new(ChartTypedValue::ProjectedNumber(
                        spelling.into(),
                    )));
                } else {
                    continue;
                }
            }
        }
        if value == ChartValue::Absent && row.shape != Shape::Picture {
            continue;
        }
        if t == Tbl::SeriesItem && n == "info" && format == ChartSourceFormat::Designer {
            return Err(frame(
                "chart: current SeriesCalcInfo requires bound semantic transport".into(),
            ));
        }
        if t == Tbl::GaugeBand
            && format == ChartSourceFormat::Designer
            && matches!(n, "useTextString" | "useToolTipString")
        {
            match value {
                ChartValue::Bool(false) => continue,
                ChartValue::Bool(true) => {
                    return Err(frame(format!(
                        "chart: current gauge item flag {n} requires bound semantic transport"
                    )));
                }
                _ => return Err(frame("chart: gauge item flag must be boolean".into())),
            }
        }
        if matches!(t, Tbl::SeriesItem | Tbl::PointItem)
            && format == ChartSourceFormat::Designer
            && matches!(n, "valInfo" | "key")
            && original.is_none()
            && default_value(t, row, format).as_ref() == Some(&value)
        {
            continue;
        }
        if format == ChartSourceFormat::Designer
            && original.is_none()
            && matches!(&value,ChartValue::Items(v) if v.is_empty())
        {
            continue;
        }
        if format == ChartSourceFormat::Designer && original.is_none() {
            if matches!(&value,ChartValue::Localized(v) if v.is_empty())
                && !native_forces_localized(t, n)
            {
                continue;
            }
            if matches!(&value,ChartValue::Color(c) if c=="auto")
                && matches!(t, Tbl::Scale | Tbl::Cpd)
            {
                continue;
            }
            if matches!(&value,ChartValue::Color(c) if c=="auto")
                && t == Tbl::Chart
                && matches!(n, "gradientPaletteStartColor" | "gradientPaletteEndColor")
            {
                continue;
            }
            if matches!(&value,ChartValue::Font(f) if f==&auto_font()) && t == Tbl::Scale {
                continue;
            }
            if t == Tbl::GInterval && n == "textColor" {
                if value == ChartValue::Color("auto".into()) {
                    continue;
                }
                return Err(frame(
                    "chart: current interval textColor requires bound semantic transport".into(),
                ));
            }
            if t == Tbl::PointItem && matches!(n, "intAdd" | "doubleAdd" | "endX") {
                if matches!(&value,ChartValue::Int(v) if designer_decimal(v)=="0") {
                    continue;
                }
                return Err(frame(format!(
                    "chart: current point field {n} requires bound semantic transport"
                )));
            }
        }
        let was_present = original.is_some_and(|o| o.fields.iter().any(|s| s == n));
        if !was_present {
            let default = match default_value(t, row, format) {
                Some(ChartValue::Nested(f)) => subtable(row.shape)
                    .map(|s| canonical_fields(f, s, format, &mut Vec::new(), &mut Vec::new()))
                    .transpose()?
                    .map(ChartValue::Nested),
                other => other,
            };
            let omitted = if original.is_some() {
                default.as_ref() == Some(&value)
            } else {
                match format {
                    ChartSourceFormat::Designer => {
                        super::sdk_defaults::native_omitted(t, n, &value)
                            && !(t == Tbl::Chart
                                && matches!(
                                    n,
                                    "startAnglePieChart" | "finishAnglePieChart" | "isometricDepth"
                                )
                                && source_layout.is_some_and(|source| {
                                    source.fields.iter().any(|name| name == n)
                                }))
                    }
                    ChartSourceFormat::Edt => default.as_ref() == Some(&value),
                }
            };
            if omitted {
                continue;
            }
        }
        if format == ChartSourceFormat::Designer {
            if let ChartValue::Color(color) = &mut value {
                path.push(ChartLayoutSegment::Field(n.into()));
                if let Some(original) = layout
                    .filter(|l| l.format == format)
                    .and_then(|l| l.composites.iter().find(|c| c.path == *path))
                {
                    if color.starts_with('#') && color.len() == 7 && color.is_ascii() {
                        let mut bytes = color.as_bytes().to_vec();
                        for i in 0..6 {
                            if original.fields.iter().any(|n| n == &format!("hexLower{i}")) {
                                bytes[i + 1] = bytes[i + 1].to_ascii_lowercase();
                            }
                        }
                        *color = String::from_utf8(bytes).expect("validated ASCII RGB");
                    }
                }
                path.pop();
            }
        }
        if row.shape == Shape::Colors {
            let ChartValue::Items(items) = &mut value else {
                return Err(frame("chart: current palette must be typed Colors".into()));
            };
            for (i, fields) in items.iter_mut().enumerate() {
                let [(name, ChartValue::Color(color))] = fields.as_mut_slice() else {
                    return Err(frame(
                        "chart: current palette item requires one Color".into(),
                    ));
                };
                if name != "value" {
                    return Err(frame("chart: palette item has unknown field".into()));
                }
                check_color_canon(color, row.name)?;
                if color.starts_with('#') {
                    *color = color.to_ascii_uppercase();
                }
                if format == ChartSourceFormat::Designer && color.starts_with('#') {
                    path.push(ChartLayoutSegment::Field(n.into()));
                    path.push(ChartLayoutSegment::Item(i));
                    if let Some(original) = layout
                        .filter(|l| l.format == format)
                        .and_then(|l| l.composites.iter().find(|c| c.path == *path))
                    {
                        let mut bytes = color.as_bytes().to_vec();
                        for j in 0..6 {
                            if original.fields.iter().any(|n| n == &format!("hexLower{j}")) {
                                bytes[j + 1] = bytes[j + 1].to_ascii_lowercase();
                            }
                        }
                        *color = String::from_utf8(bytes).expect("validated ASCII RGB");
                    }
                    path.pop();
                    path.pop();
                }
            }
        }
        if let ChartValue::Font(font) = &mut value {
            *font = normalize_chart_font(font.clone());
            if !font.auto && font.font_ref.is_none() {
                path.push(ChartLayoutSegment::Field(n.into()));
                if let Some(original) = layout
                    .filter(|l| l.format == format)
                    .and_then(|l| l.composites.iter().find(|c| c.path == *path))
                {
                    for name in &original.fields {
                        match name.as_str() {
                            "faceName" if font.face_name.is_none() => {
                                font.face_name = Some(String::new())
                            }
                            "height" if font.height.is_none() => font.height = Some("0.0".into()),
                            "bold" if font.bold.is_none() => font.bold = Some(false),
                            "italic" if font.italic.is_none() => font.italic = Some(false),
                            "underline" if font.underline.is_none() => font.underline = Some(false),
                            "strikeout" if font.strikeout.is_none() => font.strikeout = Some(false),
                            "scale" if font.scale.is_none() => font.scale = Some("100".into()),
                            _ => {}
                        }
                    }
                }
                path.pop();
            }
        }
        if let Some(sub) = subtable(row.shape) {
            path.push(ChartLayoutSegment::Field(n.to_owned()));
            value = match value {
                ChartValue::Nested(f) => {
                    ChartValue::Nested(project_fields(&f, sub, format, path, layout)?)
                }
                ChartValue::Items(items) => {
                    let mut projected = Vec::new();
                    for (i, f) in items.into_iter().enumerate() {
                        path.push(ChartLayoutSegment::Item(i));
                        projected.push(project_fields(&f, sub, format, path, layout)?);
                        path.pop();
                    }
                    ChartValue::Items(projected)
                }
                other => other,
            };
            path.pop();
        }
        if format == ChartSourceFormat::Designer
            && original.is_none()
            && matches!(
                row.shape,
                Shape::Nested(Tbl::Cpd | Tbl::ReferenceLines | Tbl::ReferenceBands)
            )
            && matches!(&value,ChartValue::Nested(f) if f.is_empty())
        {
            continue;
        }
        out.push((n.to_owned(), value));
    }
    Ok(out)
}

fn capture_primitives(
    el: &Element,
    t: Tbl,
    format: ChartSourceFormat,
    path: &mut Vec<ChartLayoutSegment>,
    out: &mut Vec<ChartCompositeLayout>,
) {
    let mut occurrences = std::collections::HashMap::new();
    for child in &el.children {
        let row = rows(t).iter().find(|r| {
            if format == ChartSourceFormat::Edt {
                r.e_name() == child.local
            } else {
                r.d_name() == child.local
            }
        });
        let Some(row) = row else { continue };
        path.push(ChartLayoutSegment::Field(row.name.into()));
        if let Some(sub) = subtable(row.shape) {
            if matches!(
                row.shape,
                Shape::Items(_) | Shape::SeriesItems | Shape::PointItems
            ) {
                let index = occurrences.entry(row.name).or_insert(0);
                path.push(ChartLayoutSegment::Item(*index));
                *index += 1;
            }
            let actual = if row.shape == Shape::SeriesItems {
                child
                    .children
                    .iter()
                    .find(|e| e.local == "properties")
                    .unwrap_or(child)
            } else {
                child
            };
            capture_primitives(actual, sub, format, path, out);
            if matches!(
                row.shape,
                Shape::Items(_) | Shape::SeriesItems | Shape::PointItems
            ) {
                path.pop();
            }
        } else if format == ChartSourceFormat::Designer
            && row.shape == Shape::Value
            && matches!(
                row.name,
                "semitransparencyPercent" | "borderSemitransparencyPercent"
            )
            && matches!(child.text.as_str(), "Infinity" | "-Infinity")
        {
            out.push(ChartCompositeLayout {
                path: path.clone(),
                fields: vec![if child.text == "Infinity" {
                    "PositiveInfinity".into()
                } else {
                    "NegativeInfinity".into()
                }],
            });
        } else if matches!(
            row.shape,
            Shape::Line | Shape::Color | Shape::Rect | Shape::Border | Shape::Picture
        ) {
            let mut names: Vec<String> = child.children.iter().map(|e| e.local.clone()).collect();
            if row.shape == Shape::Picture {
                for point in child
                    .children
                    .iter()
                    .filter(|e| e.local == "TransparentPixel")
                {
                    path.push(ChartLayoutSegment::Field("TransparentPixel".into()));
                    out.push(ChartCompositeLayout {
                        path: path.clone(),
                        fields: point.attrs.iter().map(|a| a.name.clone()).collect(),
                    });
                    path.pop();
                }
            }

            if row.shape == Shape::Color && child.text.starts_with('#') && child.text.len() == 7 {
                for (i, c) in child.text.bytes().skip(1).enumerate() {
                    if c.is_ascii_lowercase() {
                        names.push(format!("hexLower{i}"));
                    }
                }
            }
            out.push(ChartCompositeLayout {
                path: path.clone(),
                fields: names,
            });
        }
        path.pop();
    }
}

/// Restore only known default presence/order and hexadecimal spelling from CURRENT values.
pub(super) fn restore_edt_primitive_layout(
    outputs: &mut [OutElement],
    cs: &ChartSettings,
) -> Result<(), FormError> {
    let Some(layout) = cs
        .source_layout
        .as_ref()
        .filter(|l| l.format == ChartSourceFormat::Edt)
    else {
        return Ok(());
    };
    restore_primitive_children(outputs, top_tbl(&cs.kind)?, layout, &mut Vec::new());
    Ok(())
}
fn restore_primitive_children(
    elements: &mut [OutElement],
    t: Tbl,
    layout: &ChartSourceLayout,
    path: &mut Vec<ChartLayoutSegment>,
) {
    let mut occurrences = std::collections::HashMap::new();
    for el in elements {
        let Some(row) = rows(t).iter().find(|r| r.e_name() == el.local) else {
            continue;
        };
        path.push(ChartLayoutSegment::Field(row.name.into()));
        if let Some(sub) = subtable(row.shape) {
            if matches!(
                row.shape,
                Shape::Items(_) | Shape::SeriesItems | Shape::PointItems
            ) {
                let i = occurrences.entry(row.name).or_insert(0);
                path.push(ChartLayoutSegment::Item(*i));
                *i += 1;
            }
            if row.shape == Shape::SeriesItems {
                if let Some(prop) = el.children.iter_mut().find(|e| e.local == "properties") {
                    restore_primitive_children(&mut prop.children, sub, layout, path)
                }
            } else {
                restore_primitive_children(&mut el.children, sub, layout, path)
            }
            if matches!(
                row.shape,
                Shape::Items(_) | Shape::SeriesItems | Shape::PointItems
            ) {
                path.pop();
            }
        } else if row.shape == Shape::Line {
            if let Some(original) = layout.composites.iter().find(|c| c.path == *path) {
                el.children.retain(|e| {
                    original.fields.contains(&e.local)
                        || !matches!(
                            (e.local.as_str(), e.text.as_deref()),
                            ("width", Some("0")) | ("style", Some("None")) | ("gap", Some("false"))
                        )
                });
                el.self_closing = el.children.is_empty();
            }
        }
        path.pop();
    }
}

fn validate_primitive(row: &Row, value: &ChartValue) -> Result<(), FormError> {
    if let ChartValue::Color(color) = value {
        check_color_canon(color, row.name)?;
    }
    if let (Shape::Line, ChartValue::Line { style, width, .. }) = (row.shape, value) {
        if !matches!(
            style.as_str(),
            "None" | "Solid" | "Dotted" | "Dashed" | "DashDotted" | "DashDottedDotted"
        ) {
            return Err(frame(format!(
                "chart: invalid ChartLineType in {}",
                row.name
            )));
        }
        width
            .parse::<i32>()
            .map_err(|_| frame(format!("chart: invalid EInt line width in {}", row.name)))?;
    }
    Ok(())
}

// Original ChartXmlWriter/derived feature writers force these LocalString references.
fn native_forces_localized(t: Tbl, name: &str) -> bool {
    matches!(t, Tbl::Chart | Tbl::Gantt)
        || matches!(
            (t, name),
            (Tbl::GDimPoint | Tbl::GDimSeries, "title") | (Tbl::TsLevel, "format")
        )
}

fn validate_number(t: Tbl, row: &Row, value: &ChartValue) -> Result<(), FormError> {
    if let ChartValue::Int(number) = value {
        let valid = match super::sdk_defaults::numeric_type(t, row.name) {
            Some("EInt") => number.parse::<i32>().is_ok(),
            Some("ELong") => number.parse::<i64>().is_ok(),
            Some("EFloat" | "EDouble") => number.parse::<f64>().is_ok(),
            Some("EBigDecimal") => valid_big_decimal(number),
            _ => true,
        };
        if !valid {
            return Err(frame(format!(
                "chart: invalid current numeric primitive for {}",
                row.name
            )));
        }
    }
    Ok(())
}

/// Native associates the first array with each current series ID and emits lines in series order.
/// This projection is used only by the closed transport binder; callers preserve lost topology.
#[doc(hidden)]
pub fn project_native_trends(cs: &ChartSettings) -> Result<ChartSettings, FormError> {
    if top_tbl(&cs.kind)? != Tbl::Chart {
        return Ok(cs.clone());
    }
    let mut names = std::collections::HashSet::new();
    for (name, _) in &cs.fields {
        if !names.insert(name) {
            return Err(frame("chart: duplicate current root field".into()));
        }
    }
    let Some((_, ChartValue::Items(arrays))) =
        cs.fields.iter().find(|(n, _)| n == "trendLinesArray")
    else {
        if cs.fields.iter().any(|(n, _)| n == "trendLinesArray") {
            return Err(frame("chart: trendLinesArray must be typed Items".into()));
        }
        return Ok(cs.clone());
    };
    let mut associations = Vec::new();
    for array in arrays {
        let mut seen = std::collections::HashSet::new();
        for (name, _) in array {
            if !matches!(name.as_str(), "seriesId" | "line") || !seen.insert(name) {
                return Err(frame("chart: malformed current trend association".into()));
            }
        }
        let Some(ChartValue::Int(id)) = array.iter().find(|(n, _)| n == "seriesId").map(|(_, v)| v)
        else {
            return Err(frame(
                "chart: trend association requires current series ID".into(),
            ));
        };
        id.parse::<i32>()
            .map_err(|_| frame("chart: invalid current trend series ID".into()))?;
        let Some(ChartValue::Items(lines)) =
            array.iter().find(|(n, _)| n == "line").map(|(_, v)| v)
        else {
            return Err(frame(
                "chart: trend association requires ordered current lines".into(),
            ));
        };
        associations.push((id, lines));
    }
    let mut projected = Vec::new();
    // This is also the order used by collect_associated_trendlines after native read.
    for name in ["realSeriesData", "realExSeriesData"] {
        let Some((_, value)) = cs.fields.iter().find(|(n, _)| n == name) else {
            continue;
        };
        let groups: Vec<&Vec<(String, ChartValue)>> = match value {
            ChartValue::Items(items) if name == "realSeriesData" => items.iter().collect(),
            ChartValue::Nested(fields) if name == "realExSeriesData" => vec![fields],
            _ => return Err(frame("chart: invalid current series shape".into())),
        };
        for fields in groups {
            let id = match fields.iter().find(|(n, _)| n == "id").map(|(_, v)| v) {
                Some(ChartValue::Int(id)) => id.as_str(),
                None => "1",
                _ => return Err(frame("chart: invalid current series ID kind".into())),
            };
            if fields.iter().filter(|(n, _)| n == "id").count() > 1 {
                return Err(frame("chart: duplicate current series ID field".into()));
            }
            id.parse::<i32>()
                .map_err(|_| frame("chart: invalid current series ID".into()))?;
            if let Some((_, lines)) = associations.iter().find(|(key, _)| key.as_str() == id) {
                if !lines.is_empty() {
                    projected.push(vec![
                        ("seriesId".into(), ChartValue::Int(id.into())),
                        ("line".into(), ChartValue::Items((*lines).clone())),
                    ]);
                }
            }
        }
    }
    let mut result = cs.clone();
    result
        .fields
        .iter_mut()
        .find(|(n, _)| n == "trendLinesArray")
        .expect("checked field")
        .1 = ChartValue::Items(projected);
    if result.fields != cs.fields {
        if let Some(layout) = &mut result.source_layout {
            layout.composites.retain(|c| !matches!(c.path.first(),Some(ChartLayoutSegment::Field(name)) if name == "trendLinesArray"));
        }
    }
    Ok(result)
}

/// BigDecimal grammar shared by current numeric primitive and native percentage projections.
/// Normalize exactly the original Java17 Character.digit(char,10) decimal domain.
/// Supplementary characters are not digits here: BigDecimal parses UTF16 chars.
#[doc(hidden)]
pub fn normalize_big_decimal(number: &str) -> Option<String> {
    const BASES: [u32; 37] = [
        0x0030, 0x0660, 0x06f0, 0x07c0, 0x0966, 0x09e6, 0x0a66, 0x0ae6, 0x0b66, 0x0be6, 0x0c66,
        0x0ce6, 0x0d66, 0x0de6, 0x0e50, 0x0ed0, 0x0f20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946,
        0x19d0, 0x1a80, 0x1a90, 0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0,
        0xa9f0, 0xaa50, 0xabf0, 0xff10,
    ];
    let normalized: String = number
        .chars()
        .map(|c| {
            let code = u32::from(c);
            BASES
                .iter()
                .find_map(|base| code.checked_sub(*base).filter(|d| *d < 10))
                .map_or(c, |d| char::from(b'0' + d as u8))
        })
        .collect();
    let unsigned = normalized.strip_prefix(['-', '+']).unwrap_or(&normalized);
    let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
        Some((m, e)) => (m, i64::from(e.parse::<i32>().ok()?)),
        None => (unsigned, 0),
    };
    let mut dot = false;
    let mut digits = 0usize;
    for c in mantissa.bytes() {
        if c.is_ascii_digit() {
            digits = digits.checked_add(1)?;
        } else if c == b'.' && !dot {
            dot = true;
        } else {
            return None;
        }
    }
    if digits == 0 {
        return None;
    }
    let fractional = mantissa.split_once('.').map_or(0, |(_, f)| f.len());
    let scale = i64::try_from(fractional).ok()?.checked_sub(exponent)?;
    i32::try_from(scale).ok()?;
    Some(normalized)
}
pub(super) fn valid_big_decimal(number: &str) -> bool {
    normalize_big_decimal(number).is_some()
}

fn capture_common_namespace_paths(source: &Element) -> Vec<Vec<(String, usize)>> {
    fn walk(
        element: &Element,
        path: &mut Vec<(String, usize)>,
        out: &mut Vec<Vec<(String, usize)>>,
    ) {
        if element.attr("xmlns:common").is_some() && !path.is_empty() {
            out.push(path.clone());
        }
        let mut counts = std::collections::HashMap::new();
        for child in &element.children {
            let index = counts.entry(child.local.clone()).or_insert(0);
            path.push((child.local.clone(), *index));
            *index += 1;
            walk(child, path, out);
            path.pop();
        }
    }
    let mut result = Vec::new();
    walk(source, &mut Vec::new(), &mut result);
    result
}

pub(super) fn restore_native_picture_layout(
    elements: &mut [OutElement],
    cs: &ChartSettings,
) -> Result<(), FormError> {
    let Some(layout) = cs
        .source_layout
        .as_ref()
        .filter(|l| l.format == ChartSourceFormat::Designer)
    else {
        return Ok(());
    };
    fn walk(
        elements: &mut [OutElement],
        t: Tbl,
        layout: &ChartSourceLayout,
        path: &mut Vec<ChartLayoutSegment>,
    ) -> Result<(), FormError> {
        let mut counts = std::collections::HashMap::new();
        for element in elements {
            let Some(row) = rows(t).iter().find(|r| r.d_name() == element.local) else {
                continue;
            };
            path.push(ChartLayoutSegment::Field(row.name.into()));
            if let Some(sub) = subtable(row.shape) {
                let repeated = matches!(
                    row.shape,
                    Shape::Items(_) | Shape::SeriesItems | Shape::PointItems
                );
                if repeated {
                    let index = counts.entry(row.name).or_insert(0);
                    path.push(ChartLayoutSegment::Item(*index));
                    *index += 1;
                }
                walk(&mut element.children, sub, layout, path)?;
                if repeated {
                    path.pop();
                }
            } else if row.shape == Shape::Picture {
                if let Some(original) = layout.composites.iter().find(|c| c.path == *path) {
                    if original.fields.iter().any(|n| n == "TransparentPixel")
                        && element.children.iter().any(|c| c.local == "Abs")
                        && element.children.iter().any(|c| {
                            c.local == "LoadTransparent" && c.text.as_deref() == Some("true")
                        })
                        && !element
                            .children
                            .iter()
                            .any(|c| c.local == "TransparentPixel")
                    {
                        element.push(super::picture::native_point(
                            "TransparentPixel",
                            (-1, -1),
                            "current sentinel",
                        )?);
                    }
                    path.push(ChartLayoutSegment::Field("TransparentPixel".into()));
                    if let Some(point_layout) = layout.composites.iter().find(|c| c.path == *path) {
                        if let Some(point) = element
                            .children
                            .iter_mut()
                            .find(|c| c.local == "TransparentPixel")
                        {
                            point.attrs.retain(|(name, value)| {
                                value != "0" || point_layout.fields.contains(name)
                            });
                        }
                    }
                    path.pop();
                }
            }
            path.pop();
        }
        Ok(())
    }
    walk(elements, top_tbl(&cs.kind)?, layout, &mut Vec::new())
}
