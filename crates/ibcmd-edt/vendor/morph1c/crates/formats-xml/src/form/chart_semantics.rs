//! Closed transport of current chart state omitted by native SDK XML.
//! Item bindings contain identities and a digest, never previous chart values.
use super::series_info::SeriesInfo;
use super::trend_transport::{self, TrendTopology};
use super::FormError;
use morph1c_core::ir::form::{ChartSettings, ChartTypedValue, ChartValue, FormDataAttribute};
use morph1c_core::ir::{FormBody, Uuid};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::io::Write;

pub const CHART_SEMANTICS_RESOURCE: &str = "ibcmd-chart-semantics.v1.json";
const SCHEMA: &str = "urn:ibcmd:source-extension:chart-semantics:1";
fn error(text: impl Into<String>) -> FormError {
    FormError::Frame(format!("chart semantics: {}", text.into()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "edge", deny_unknown_fields)]
enum AttributeEdge {
    Attribute {
        name: String,
        id: i64,
    },
    Column {
        name: String,
        id: i64,
    },
    AdditionalColumn {
        table_path: String,
        group: usize,
        name: String,
        id: i64,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
enum Step {
    Field(String),
    Item(usize),
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum Kind {
    GaugeBand,
    DataItem,
    SeriesItem,
    PointItem,
    GanttInterval,
    AxisInterval,
}
impl Kind {
    fn fields(&self) -> &'static [&'static str] {
        match self {
            Self::GaugeBand => &["useTextString", "useToolTipString"],
            Self::DataItem => &["isToolTipFormatted"],
            Self::SeriesItem | Self::PointItem | Self::GanttInterval | Self::AxisInterval => &[],
        }
    }
    fn omitted(&self) -> &'static [&'static str] {
        match self {
            Self::SeriesItem => &["info"],
            Self::PointItem => &["intAdd", "doubleAdd", "endX"],
            Self::GanttInterval => &["textColor"],
            _ => &[],
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    attribute: Vec<AttributeEdge>,
    chart_kind: String,
    path: Vec<Step>,
    kind: Kind,
    current_item_sha256: String,
    flags: Vec<(String, bool)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    series_info: Option<SeriesInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    point_aux: Option<PointAux>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    interval_text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    axis_inactive: Option<AxisInactive>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AxisInactive {
    left: Option<AxisLoss>,
    right: Option<AxisLoss>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum AxisLoss {
    Date(String),
    Number(String),
    EmptyDateMode { number: String },
}
fn axis_inactive(
    fields: &[(String, ChartValue)],
    kind: &Kind,
) -> Result<Option<AxisInactive>, FormError> {
    if !matches!(kind, Kind::AxisInterval) {
        return Ok(None);
    }
    let side = |side: &str| -> Result<Option<AxisLoss>, FormError> {
        let ChartValue::Bool(is_number) = field(fields, &format!("{side}IsNum"))? else {
            return Err(error("wrong current axis boundary kind"));
        };
        let ChartValue::Int(number) = field(fields, &format!("{side}Num"))? else {
            return Err(error("wrong current axis number kind"));
        };
        if number.parse::<f64>().is_err() {
            return Err(error("invalid current axis EDouble"));
        }
        let date = match field(fields, &format!("{side}Date"))? {
            ChartValue::Absent => None,
            ChartValue::Str(v) => Some(v),
            _ => return Err(error("wrong current axis date kind")),
        };
        Ok(match (*is_number, date) {
            (true, Some(date)) => Some(AxisLoss::Date(date.clone())),
            (false, None) => Some(AxisLoss::EmptyDateMode {
                number: number.clone(),
            }),
            (false, Some(_)) if number != "0" => Some(AxisLoss::Number(number.clone())),
            _ => None,
        })
    };
    let value = AxisInactive {
        left: side("left")?,
        right: side("right")?,
    };
    Ok((value.left.is_some() || value.right.is_some()).then_some(value))
}
fn axis_project(fields: &mut [(String, ChartValue)], value: &AxisInactive) {
    for (side, loss) in [("left", &value.left), ("right", &value.right)] {
        let Some(loss) = loss else {
            continue;
        };
        for (name, value) in fields.iter_mut() {
            if matches!(loss, AxisLoss::Date(_)) && name == &format!("{side}Date") {
                *value = ChartValue::Absent;
            } else if matches!(loss, AxisLoss::Number(_) | AxisLoss::EmptyDateMode { .. })
                && name == &format!("{side}Num")
            {
                *value = ChartValue::Int("0".into());
            } else if matches!(loss, AxisLoss::EmptyDateMode { .. })
                && name == &format!("{side}IsNum")
            {
                *value = ChartValue::Bool(true);
            }
        }
    }
}
fn axis_restore(fields: &mut [(String, ChartValue)], payload: &AxisInactive) {
    for (side, loss) in [("left", &payload.left), ("right", &payload.right)] {
        let Some(loss) = loss else {
            continue;
        };
        for (name, value) in fields.iter_mut() {
            if let AxisLoss::Date(date) = loss {
                if name == &format!("{side}Date") {
                    *value = ChartValue::Str(date.clone());
                }
            } else if name == &format!("{side}Num") {
                *value = ChartValue::Int(match loss {
                    AxisLoss::Number(n) | AxisLoss::EmptyDateMode { number: n } => n.clone(),
                    _ => unreachable!(),
                });
            } else if matches!(loss, AxisLoss::EmptyDateMode { .. })
                && name == &format!("{side}IsNum")
            {
                *value = ChartValue::Bool(false);
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PointAux {
    int_add: i32,
    double_add: String,
    end_x: i32,
}
fn field<'a>(fields: &'a [(String, ChartValue)], name: &str) -> Result<&'a ChartValue, FormError> {
    let mut values = fields.iter().filter(|(n, _)| n == name);
    let value = values
        .next()
        .ok_or_else(|| error("missing current chart property"))?;
    if values.next().is_some() {
        return Err(error("duplicate current chart property"));
    }
    Ok(&value.1)
}
fn point_aux(fields: &[(String, ChartValue)], kind: &Kind) -> Result<Option<PointAux>, FormError> {
    if !matches!(kind, Kind::PointItem) {
        return Ok(None);
    }
    let integer = |name| match field(fields, name)? {
        ChartValue::Int(v) => v
            .parse::<i32>()
            .map_err(|_| error("invalid current point EInt")),
        _ => Err(error("wrong current point property kind")),
    };
    let ChartValue::Int(double) = field(fields, "doubleAdd")? else {
        return Err(error("wrong current point double kind"));
    };
    if double.parse::<f64>().is_err() {
        return Err(error("invalid current point EDouble"));
    }
    let value = PointAux {
        int_add: integer("intAdd")?,
        double_add: double.clone(),
        end_x: integer("endX")?,
    };
    Ok((value.int_add != 0 || value.double_add != "0" || value.end_x != 0).then_some(value))
}
fn interval_color(
    fields: &[(String, ChartValue)],
    kind: &Kind,
) -> Result<Option<String>, FormError> {
    if !matches!(kind, Kind::GanttInterval) {
        return Ok(None);
    }
    let value = field(fields, "textColor")?;
    if value == &ChartValue::Absent { return Ok(None); }
    let ChartValue::Color(color) = value else {
        return Err(error("wrong current interval color kind"));
    };
    super::chart::check_color_canon(color, "GanttChart interval textColor")?;
    Ok((color != "auto").then(|| color.clone()))
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    schema: String,
    version: u32,
    form_uuid: Uuid,
    records: Vec<Record>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    values: Vec<ValueRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    trends: Vec<TrendRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    designs: Vec<DesignRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    numbers: Vec<NumberRecord>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NumberRecord {
    attribute: Vec<AttributeEdge>,
    chart_kind: String,
    current_native_sha256: String,
    current: Vec<(Vec<Step>, String)>,
}
fn number_paths(
    chart: &mut ChartSettings,
) -> Result<Vec<(Vec<Step>, String)>, FormError> {
    use morph1c_core::ir::form::ChartLayoutSegment;
    Ok(super::chart::project_native_double_fields(chart)?.into_iter().map(|(path, current)| {
        (path.into_iter().map(|segment| match segment {
            ChartLayoutSegment::Field(name) => Step::Field(name),
            ChartLayoutSegment::Item(index) => Step::Item(index),
        }).collect(), current)
    }).collect())
}
fn restore_numbers(chart: &mut ChartSettings, row: &NumberRecord) -> Result<(), FormError> {
    fn set(fields: &mut [(String, ChartValue)], path: &[Step], current: &str) -> Result<(), FormError> {
        let Some(Step::Field(name)) = path.first() else { return Err(error("invalid numeric field path")); };
        let value = &mut fields.iter_mut().find(|(n, _)| n == name)
            .ok_or_else(|| error("orphan current numeric field"))?.1;
        match &path[1..] {
            [] => match value {
                ChartValue::Int(number) => { *number = current.into(); Ok(()) },
                _ => Err(error("wrong current numeric field kind")),
            },
            [Step::Item(index), rest @ ..] => match value {
                ChartValue::Items(items) => set(items.get_mut(*index)
                    .ok_or_else(|| error("orphan current numeric item"))?, rest, current),
                _ => Err(error("wrong current numeric item kind")),
            },
            rest => match value {
                ChartValue::Nested(children) => set(children, rest, current),
                _ => Err(error("wrong current numeric container")),
            },
        }
    }
    for (path, current) in &row.current { set(&mut chart.fields, path, current)?; }
    Ok(())
}
/// CURRENT typed collections suppressed by the SDK's explicit design flags.
/// No previous chart body or projected values are stored in this payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DesignRecord {
    attribute: Vec<AttributeEdge>,
    chart_kind: String,
    current_native_sha256: String,
    current: Vec<(String, ChartValue)>,
}
fn project_design(
    chart: &mut ChartSettings,
    current: &[(String, ChartValue)],
) -> Result<(), FormError> {
    for (name, _) in current {
        let replacement = super::chart::native_design_default(name)?;
        chart
            .fields
            .iter_mut()
            .find(|(n, _)| n == name)
            .ok_or_else(|| error("missing current design field"))?
            .1 = replacement;
        // This transport projection no longer has the source's explicit hidden spelling.
        if let Some(layout) = &mut chart.source_layout {
            for composite in &mut layout.composites {
                if composite.path.is_empty() {
                    composite.fields.retain(|n| n != name);
                }
            }
            layout.composites.retain(|c| !matches!(c.path.first(),Some(morph1c_core::ir::form::ChartLayoutSegment::Field(n)) if n == name));
        }
    }
    Ok(())
}
fn design_digest(chart: &ChartSettings, native: bool) -> Result<String, FormError> {
    let mut chart = chart.clone();
    super::chart::project_native_double_fields(&mut chart)?;
    let mut fields = chart.fields;
    if !native {
        project_values(&mut fields, &mut Vec::new(), false);
        fields = trend_transport::projected(&fields)?;
    }
    project_values(&mut fields, &mut Vec::new(), true);
    let mut out = DigestWriter(Sha256::new());
    serde_json::to_writer(&mut out, &fields).map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", out.0.finalize()))
}
fn restore_design(chart: &mut ChartSettings, row: &DesignRecord) -> Result<(), FormError> {
    for (name, value) in &row.current {
        chart
            .fields
            .iter_mut()
            .find(|(n, _)| n == name)
            .ok_or_else(|| error("missing native design counterpart"))?
            .1 = value.clone();
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TrendRecord {
    attribute: Vec<AttributeEdge>,
    chart_kind: String,
    path: Vec<Step>,
    current_native_sha256: String,
    current: TrendTopology,
}
fn trend_digest(fields: &[(String, ChartValue)], native: bool) -> Result<String, FormError> {
    let projected = if native {
        fields.to_vec()
    } else {
        trend_transport::projected(fields)?
    };
    let mut out = DigestWriter(Sha256::new());
    serde_json::to_writer(&mut out, &trend_transport::binding(&projected)?)
        .map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", out.0.finalize()))
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValueRecord {
    attribute: Vec<AttributeEdge>,
    chart_kind: String,
    path: Vec<Step>,
    field: String,
    field_rank: usize,
    current_parent_sha256: String,
    current: ChartTypedValue,
}
/// Native XML cannot represent a nullable reference or recover EEnum package identity.
/// Ordered collection projection uses the SDK's omitted-member rule.
fn projected_value(value: &ChartTypedValue) -> Option<ChartTypedValue> {
    match value {
        ChartTypedValue::Reference(None) => None,
        ChartTypedValue::Enum {
            enum_type, literal, ..
        } => Some(ChartTypedValue::ProjectedEnum {
            enum_type: enum_type.clone(),
            literal: literal.clone(),
        }),
        ChartTypedValue::SysEnum(value) => value.as_ref().and_then(|value| {
            let mut parts = value.split('.');
            let name = parts.next()?;
            let literal = parts.next()?;
            (super::chart::is_native_enum_type(name) && !literal.is_empty()).then(|| {
                if name == "AccountType" {
                    ChartTypedValue::ProjectedEnum {
                        enum_type: name.into(),
                        literal: literal.into(),
                    }
                } else {
                    ChartTypedValue::SysEnum(Some(format!("{name}.{literal}")))
                }
            })
        }),
        ChartTypedValue::ValueList(values) => Some(ChartTypedValue::ValueList(
            values.iter().filter_map(projected_value).collect(),
        )),
        ChartTypedValue::FixedArray(values) => Some(ChartTypedValue::FixedArray(
            values.iter().filter_map(projected_value).collect(),
        )),
        value => Some(value.clone()),
    }
}
fn binding_value(value: &ChartTypedValue) -> ChartTypedValue {
    match value {
        ChartTypedValue::ProjectedNumber(text) => {
            use morph1c_core::ir::value::{PropertyValue, ValueScalarKind, ValueSpec};
            ChartTypedValue::Scalar(PropertyValue::Value(
                if matches!(text.as_str(), "Infinity" | "-Infinity" | "NaN") {
                    ValueSpec {
                        kind: ValueScalarKind::Undefined,
                        scalar: None,
                    }
                } else {
                    ValueSpec {
                        kind: ValueScalarKind::Number,
                        scalar: Some(Box::new(PropertyValue::Str(
                            text.strip_suffix(".0").unwrap_or(text).into(),
                        ))),
                    }
                },
            ))
        }
        ChartTypedValue::ProjectedEnum { enum_type, literal } => {
            super::chart::native_enum_binding(enum_type, literal)
        }
        ChartTypedValue::ValueList(values) => {
            ChartTypedValue::ValueList(values.iter().map(binding_value).collect())
        }
        ChartTypedValue::FixedArray(values) => {
            ChartTypedValue::FixedArray(values.iter().map(binding_value).collect())
        }
        value => value.clone(),
    }
}
fn projected_field_value(name: &str, value: &ChartTypedValue) -> Option<ChartTypedValue> {
    if matches!(value, ChartTypedValue::ProjectedNumber(_)) {
        return Some(value.clone());
    }
    if matches!(
        name,
        "semitransparencyPercent" | "borderSemitransparencyPercent"
    ) {
        use morph1c_core::ir::value::{PropertyValue, ValueScalarKind};
        let ChartTypedValue::Scalar(PropertyValue::Value(spec)) = value else {
            return None;
        };
        if spec.kind != ValueScalarKind::Number {
            return None;
        }
        if let Some(PropertyValue::Str(number)) = spec.scalar.as_deref() {
            if let Ok((text, _)) = super::chart::native_percentage_projection(number) {
                let projected = ChartTypedValue::ProjectedNumber(text);
                return Some(if binding_value(&projected) == *value {
                    value.clone()
                } else {
                    projected
                });
            }
        }
    }
    projected_value(value)
}
fn value_default(path: &[Step], name: &str) -> Option<ChartValue> {
    match (item_kind(path), name) {
        (Some(Kind::SeriesItem | Kind::PointItem), "valInfo" | "key") => {
            Some(ChartValue::Value(Box::new(ChartTypedValue::Scalar(
                morph1c_core::ir::value::PropertyValue::Value(morph1c_core::ir::value::ValueSpec {
                    kind: morph1c_core::ir::value::ValueScalarKind::Undefined,
                    scalar: None,
                }),
            ))))
        }
        (Some(Kind::DataItem), "dataValue" | "infoValue") => Some(ChartValue::Absent),
        _ => None,
    }
}
fn project_values(fields: &mut Vec<(String, ChartValue)>, path: &mut Vec<Step>, binding: bool) {
    let kind = item_kind(path);
    if let Some(kind) = &kind {
        if let Ok(Some(value)) = axis_inactive(fields, kind) {
            axis_project(fields, &value);
        }
    }
    fields.retain_mut(|(name, value)| {
        if let Some(kind) = &kind {
            if kind.fields().contains(&name.as_str()) {
                *value = ChartValue::Bool(false);
            } else if kind.omitted().contains(&name.as_str()) {
                *value = match kind {
                    Kind::SeriesItem => ChartValue::Absent,
                    Kind::PointItem => ChartValue::Int("0".into()),
                    Kind::GanttInterval => ChartValue::Absent,
                    _ => unreachable!(),
                };
            }
        }
        if let ChartValue::Value(current) = value {
            if let Some(mut current) = if binding {
                Some(current.as_ref().clone())
            } else {
                projected_field_value(name, current)
            } {
                if binding {
                    current = binding_value(&current);
                }
                *value = ChartValue::Value(Box::new(current));
            } else if let Some(default) = value_default(path, name) {
                *value = default;
            } else {
                return false;
            }
        }
        path.push(Step::Field(name.clone()));
        match value {
            ChartValue::Nested(children) => project_values(children, path, binding),
            ChartValue::Items(items) => {
                for (index, item) in items.iter_mut().enumerate() {
                    path.push(Step::Item(index));
                    project_values(item, path, binding);
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
        true
    });
}
fn parent_digest(
    fields: &[(String, ChartValue)],
    path: &[Step],
    native: bool,
) -> Result<String, FormError> {
    let mut fields = fields.to_vec();
    if !native {
        project_values(&mut fields, &mut path.to_vec(), false);
    }
    project_values(&mut fields, &mut path.to_vec(), true);
    let mut out = DigestWriter(Sha256::new());
    serde_json::to_writer(&mut out, &fields).map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", out.0.finalize()))
}
fn all_fields<F>(
    fields: &[(String, ChartValue)],
    path: &mut Vec<Step>,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[Step], &[(String, ChartValue)]) -> Result<(), FormError>,
{
    action(path, fields)?;
    for (name, value) in fields {
        path.push(Step::Field(name.clone()));
        match value {
            ChartValue::Nested(children) => all_fields(children, path, action)?,
            ChartValue::Items(items) => {
                for (index, item) in items.iter().enumerate() {
                    path.push(Step::Item(index));
                    all_fields(item, path, action)?;
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}
fn all_fields_mut<F>(
    fields: &mut Vec<(String, ChartValue)>,
    path: &mut Vec<Step>,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[Step], &mut Vec<(String, ChartValue)>) -> Result<(), FormError>,
{
    action(path, fields)?;
    for (name, value) in fields {
        path.push(Step::Field(name.clone()));
        match value {
            ChartValue::Nested(children) => all_fields_mut(children, path, action)?,
            ChartValue::Items(items) => {
                for (index, item) in items.iter_mut().enumerate() {
                    path.push(Step::Item(index));
                    all_fields_mut(item, path, action)?;
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}

fn item_kind(path: &[Step]) -> Option<Kind> {
    match path {
        [.., Step::Field(collection), Step::Item(_)] if collection == "realDataItems" => {
            Some(Kind::DataItem)
        }
        [.., Step::Field(collection), Step::Field(items), Step::Item(_)]
            if collection == "gaugeQualityBands" && items == "items" =>
        {
            Some(Kind::GaugeBand)
        }
        [.., Step::Field(collection), Step::Item(_)] if collection == "realSeriesData" => {
            Some(Kind::SeriesItem)
        }
        [.., Step::Field(collection)] if collection == "realExSeriesData" => Some(Kind::SeriesItem),
        [.., Step::Field(collection), Step::Item(_)] if collection == "realPointData" => {
            Some(Kind::PointItem)
        }
        [.., Step::Field(collection), Step::Item(_)] if collection == "interval" => {
            Some(Kind::GanttInterval)
        }
        [.., Step::Field(collection)] if collection == "interval" => Some(Kind::AxisInterval),
        _ => None,
    }
}
fn flags(fields: &[(String, ChartValue)], kind: &Kind) -> Result<Vec<(String, bool)>, FormError> {
    kind.fields()
        .iter()
        .map(|name| {
            let current: Vec<_> = fields.iter().filter(|(n, _)| n == name).collect();
            match current.as_slice() {
                [(_, ChartValue::Bool(value))] => Ok(((*name).into(), *value)),
                _ => Err(error("missing/duplicate/wrong-kind chart flag")),
            }
        })
        .collect()
}
struct DigestWriter(Sha256);
impl Write for DigestWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn item_digest(
    fields: &[(String, ChartValue)],
    kind: &Kind,
    path: &[Step],
    native: bool,
) -> Result<String, FormError> {
    let mut fields = fields.to_vec();
    if !native {
        project_values(&mut fields, &mut path.to_vec(), false);
    }
    project_values(&mut fields, &mut path.to_vec(), true);
    let current: Vec<_> = fields
        .iter()
        .filter(|(name, _)| {
            !kind.fields().contains(&name.as_str()) && !kind.omitted().contains(&name.as_str())
        })
        .collect();
    let mut out = DigestWriter(Sha256::new());
    serde_json::to_writer(&mut out, &current).map_err(|e| error(e.to_string()))?;
    Ok(format!("{:x}", out.0.finalize()))
}
fn series_info(
    fields: &[(String, ChartValue)],
    kind: &Kind,
) -> Result<Option<SeriesInfo>, FormError> {
    if !matches!(kind, Kind::SeriesItem) {
        return Ok(None);
    }
    match field(fields, "info")? {
        ChartValue::Absent => Ok(None),
        value => SeriesInfo::read(value).map(Some),
    }
}
fn key(attribute: &[AttributeEdge], path: &[Step]) -> Result<String, FormError> {
    serde_json::to_string(&(attribute, path)).map_err(|e| error(e.to_string()))
}

fn visit_fields<F>(
    fields: &mut [(String, ChartValue)],
    path: &mut Vec<Step>,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[Step], &Kind, &mut [(String, ChartValue)]) -> Result<(), FormError>,
{
    if let Some(kind) = item_kind(path) {
        action(path, &kind, fields)?;
    }
    for (name, value) in fields {
        path.push(Step::Field(name.clone()));
        match value {
            ChartValue::Nested(children) => visit_fields(children, path, action)?,
            ChartValue::Items(items) => {
                for (index, item) in items.iter_mut().enumerate() {
                    path.push(Step::Item(index));
                    visit_fields(item, path, action)?;
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}
fn visit_attributes<F>(
    attrs: &mut [FormDataAttribute],
    path: &mut Vec<AttributeEdge>,
    edge: &dyn Fn(&FormDataAttribute) -> AttributeEdge,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[AttributeEdge], &mut ChartSettings) -> Result<(), FormError>,
{
    for attr in attrs {
        path.push(edge(attr));
        if let Some(chart) = &mut attr.chart_settings {
            if !["Chart", "GanttChart"].contains(&chart.kind.as_str()) {
                return Err(error("unknown current chart kind"));
            }
            action(path, chart)?;
        }
        visit_attributes(
            &mut attr.columns,
            path,
            &|a| AttributeEdge::Column {
                name: a.name.clone(),
                id: a.id,
            },
            action,
        )?;
        for (index, group) in attr.additional_columns.iter_mut().enumerate() {
            let table = group.table_path.primary();
            visit_attributes(
                &mut group.columns,
                path,
                &|a| AttributeEdge::AdditionalColumn {
                    table_path: table.clone(),
                    group: index,
                    name: a.name.clone(),
                    id: a.id,
                },
                action,
            )?;
        }
        path.pop();
    }
    Ok(())
}
fn inspect_fields<F>(
    fields: &[(String, ChartValue)],
    path: &mut Vec<Step>,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[Step], &Kind, &[(String, ChartValue)]) -> Result<(), FormError>,
{
    if let Some(kind) = item_kind(path) {
        action(path, &kind, fields)?;
    }
    for (name, value) in fields {
        path.push(Step::Field(name.clone()));
        match value {
            ChartValue::Nested(children) => inspect_fields(children, path, action)?,
            ChartValue::Items(items) => {
                for (index, item) in items.iter().enumerate() {
                    path.push(Step::Item(index));
                    inspect_fields(item, path, action)?;
                    path.pop();
                }
            }
            _ => {}
        }
        path.pop();
    }
    Ok(())
}
fn inspect_attributes<F>(
    attrs: &[FormDataAttribute],
    path: &mut Vec<AttributeEdge>,
    edge: &dyn Fn(&FormDataAttribute) -> AttributeEdge,
    action: &mut F,
) -> Result<(), FormError>
where
    F: FnMut(&[AttributeEdge], &ChartSettings) -> Result<(), FormError>,
{
    for attr in attrs {
        path.push(edge(attr));
        if let Some(chart) = &attr.chart_settings {
            if !["Chart", "GanttChart"].contains(&chart.kind.as_str()) {
                return Err(error("unknown current chart kind"));
            }
            action(path, chart)?;
        }
        inspect_attributes(
            &attr.columns,
            path,
            &|a| AttributeEdge::Column {
                name: a.name.clone(),
                id: a.id,
            },
            action,
        )?;
        for (index, group) in attr.additional_columns.iter().enumerate() {
            inspect_attributes(
                &group.columns,
                path,
                &|a| AttributeEdge::AdditionalColumn {
                    table_path: group.table_path.primary(),
                    group: index,
                    name: a.name.clone(),
                    id: a.id,
                },
                action,
            )?;
        }
        path.pop();
    }
    Ok(())
}
fn collect(body: &FormBody, form_uuid: Uuid) -> Result<Resource, FormError> {
    let mut designs = Vec::new();
    let mut projected = None;
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            let current = super::chart::suppressed_design_fields(chart)?;
            if !current.is_empty() {
                let mut counterpart = chart.clone();
                project_design(&mut counterpart, &current)?;
                designs.push(DesignRecord {
                    attribute: attribute.to_vec(),
                    chart_kind: chart.kind.clone(),
                    current_native_sha256: design_digest(&counterpart, false)?,
                    current,
                });
            }
            Ok(())
        },
    )?;
    if !designs.is_empty() {
        let mut copy = body.clone();
        visit_attributes(
            &mut copy.data_attributes,
            &mut Vec::new(),
            &|a| AttributeEdge::Attribute {
                name: a.name.clone(),
                id: a.id,
            },
            &mut |attribute, chart| {
                if let Some(row) = designs.iter().find(|r| r.attribute == attribute) {
                    project_design(chart, &row.current)?;
                }
                Ok(())
            },
        )?;
        projected = Some(copy);
    }
    let body = projected.as_ref().unwrap_or(body);
    let mut numeric_body = body.clone();
    let mut numbers = Vec::new();
    visit_attributes(
        &mut numeric_body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute { name: a.name.clone(), id: a.id },
        &mut |attribute, chart| {
            let current = number_paths(chart)?;
            if !current.is_empty() {
                numbers.push(NumberRecord {
                    attribute: attribute.to_vec(),
                    chart_kind: chart.kind.clone(),
                    current_native_sha256: design_digest(chart, false)?,
                    current,
                });
            }
            Ok(())
        },
    )?;
    let body = &numeric_body;
    let mut records = Vec::new();
    let mut values = Vec::new();
    let mut trends = Vec::new();
    let mut seen = HashSet::new();
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            inspect_fields(&chart.fields, &mut Vec::new(), &mut |path, kind, item| {
                let current_flags = flags(item, kind)?;
                let series_info = series_info(item, kind)?;
                let point_aux = point_aux(item, kind)?;
                let interval_text_color = interval_color(item, kind)?;
                let axis_inactive = axis_inactive(item, kind)?;
                if current_flags.iter().all(|(_, value)| !*value)
                    && series_info.is_none()
                    && point_aux.is_none()
                    && interval_text_color.is_none()
                    && axis_inactive.is_none()
                {
                    return Ok(());
                }
                if !seen.insert(key(attribute, path)?) {
                    return Err(error("ambiguous current chart item identity"));
                }
                records.push(Record {
                    attribute: attribute.to_vec(),
                    chart_kind: chart.kind.clone(),
                    path: path.to_vec(),
                    kind: kind.clone(),
                    current_item_sha256: item_digest(item, kind, path, false)?,
                    flags: current_flags,
                    series_info,
                    point_aux,
                    interval_text_color,
                    axis_inactive,
                });
                Ok(())
            })
        },
    )?;
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            all_fields(&chart.fields, &mut Vec::new(), &mut |path, fields| {
                if fields.iter().any(|(n, _)| n == "trendLinesArray") {
                    if let Some(current) = TrendTopology::collect(fields)? {
                        if !seen.insert(format!("trend:{}", key(attribute, path)?)) {
                            return Err(error("ambiguous current trend identity"));
                        }
                        trends.push(TrendRecord {
                            attribute: attribute.to_vec(),
                            chart_kind: chart.kind.clone(),
                            path: path.to_vec(),
                            current_native_sha256: trend_digest(fields, false)?,
                            current,
                        });
                    }
                }
                for (field_rank, (name, value)) in fields.iter().enumerate() {
                    if let ChartValue::Value(current) = value {
                        if projected_field_value(name, current).as_ref() != Some(current.as_ref()) {
                            if !seen.insert(format!("value:{}:{name}", key(attribute, path)?)) {
                                return Err(error("ambiguous current value identity"));
                            }
                            values.push(ValueRecord {
                                attribute: attribute.to_vec(),
                                chart_kind: chart.kind.clone(),
                                path: path.to_vec(),
                                field: name.clone(),
                                field_rank,
                                current_parent_sha256: parent_digest(fields, path, false)?,
                                current: current.as_ref().clone(),
                            });
                        }
                    }
                }
                Ok(())
            })
        },
    )?;
    Ok(Resource {
        schema: SCHEMA.into(),
        version: 1,
        form_uuid,
        records,
        values,
        trends,
        numbers,
        designs,
    })
}
fn decode(bytes: &[u8]) -> Result<Resource, FormError> {
    let value: Resource = super::strict_resource::parse(bytes).map_err(|e| error(e.to_string()))?;
    if value.schema != SCHEMA
        || value.version != 1
        || (value.records.is_empty()
            && value.values.is_empty()
            && value.trends.is_empty()
            && value.designs.is_empty()
            && value.numbers.is_empty())
    {
        return Err(error("unknown/empty chart resource schema"));
    }
    let mut seen = HashSet::new();
    for row in &value.numbers {
        if row.attribute.is_empty()
            || !["Chart", "GanttChart"].contains(&row.chart_kind.as_str())
            || row.current.is_empty()
            || row.current_native_sha256.len() != 64
            || !row.current_native_sha256.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(format!("number:{}", key(&row.attribute, &[])?))
        {
            return Err(error("invalid/duplicate current number binding"));
        }
        let mut paths = HashSet::new();
        for (path, current) in &row.current {
            if path.is_empty()
                || !matches!(path.last(), Some(Step::Field(_)))
                || !paths.insert(serde_json::to_string(path).map_err(|e| error(e.to_string()))?)
                || ibcmd_number_format::parse_binary64(current).is_none()
            {
                return Err(error("invalid/duplicate current number payload"));
            }
        }
    }
    for row in &value.records {
        if row.attribute.is_empty()
            || !["Chart", "GanttChart"].contains(&row.chart_kind.as_str())
            || item_kind(&row.path) != Some(row.kind.clone())
            || row.current_item_sha256.len() != 64
            || !row
                .current_item_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(key(&row.attribute, &row.path)?)
        {
            return Err(error("invalid/duplicate chart resource identity"));
        }
        if row.flags.len() != row.kind.fields().len()
            || row
                .flags
                .iter()
                .zip(row.kind.fields())
                .any(|((name, _), expected)| name != expected)
            || (matches!(row.kind, Kind::SeriesItem) != row.series_info.is_some())
            || (matches!(row.kind, Kind::PointItem) != row.point_aux.is_some())
            || (matches!(row.kind, Kind::GanttInterval) != row.interval_text_color.is_some())
            || (matches!(row.kind, Kind::AxisInterval) != row.axis_inactive.is_some())
            || (row.flags.iter().all(|(_, value)| !*value)
                && row.series_info.is_none()
                && row.point_aux.is_none()
                && row.interval_text_color.is_none()
                && row.axis_inactive.is_none())
        {
            return Err(error("unknown/duplicate/default chart payload"));
        }
        if let Some(info) = &row.series_info {
            info.validate()?;
        }
        if let Some(point) = &row.point_aux {
            if point.double_add.parse::<f64>().is_err()
                || (point.int_add == 0 && point.double_add == "0" && point.end_x == 0)
            {
                return Err(error("invalid/default current point payload"));
            }
        }
        if let Some(color) = &row.interval_text_color {
            super::chart::check_color_canon(color, "GanttChart interval textColor")?;
            if color == "auto" {
                return Err(error("default current interval color payload"));
            }
        }
        if let Some(axis) = &row.axis_inactive {
            if axis.left.is_none() && axis.right.is_none() {
                return Err(error("default current axis payload"));
            }
            for loss in [&axis.left, &axis.right].into_iter().flatten() {
                match loss {
                    AxisLoss::Number(n) if n == "0" || n.parse::<f64>().is_err() => {
                        return Err(error("invalid/default current axis number payload"));
                    }
                    AxisLoss::EmptyDateMode { number } if number.parse::<f64>().is_err() => {
                        return Err(error("invalid current empty date mode"));
                    }
                    _ => {}
                }
            }
        }
    }
    for row in &value.values {
        if row.attribute.is_empty()
            || !["Chart", "GanttChart"].contains(&row.chart_kind.as_str())
            || ![
                "dataValue",
                "infoValue",
                "valInfo",
                "key",
                "value",
                "begin",
                "end",
                "semitransparencyPercent",
                "details",
                "borderSemitransparencyPercent",
            ]
            .contains(&row.field.as_str())
            || row.current_parent_sha256.len() != 64
            || !row
                .current_parent_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(format!(
                "value:{}:{}",
                key(&row.attribute, &row.path)?,
                row.field
            ))
            || projected_field_value(&row.field, &row.current).as_ref() == Some(&row.current)
        {
            return Err(error("invalid/duplicate/default current value binding"));
        }
        super::chart::validate_current_value(&row.current)?;
    }
    for row in &value.designs {
        if row.attribute.is_empty()
            || row.chart_kind != "Chart"
            || row.current.is_empty()
            || row.current_native_sha256.len() != 64
            || !row
                .current_native_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(format!("design:{}", key(&row.attribute, &[])?))
        {
            return Err(error("invalid/duplicate current design identity"));
        }
        let mut fields = HashSet::new();
        for (name, current) in &row.current {
            let default = super::chart::native_design_default(name)?;
            if !fields.insert(name)
                || current == &default
                || !matches!(
                    (name.as_str(), current),
                    ("realSeriesData" | "realPointData", ChartValue::Items(_))
                        | ("realExSeriesData", ChartValue::Nested(_) | ChartValue::Absent)
                )
            {
                return Err(error("invalid/default current design payload"));
            }
        }
    }
    for row in &value.trends {
        if row.attribute.is_empty()
            || !["Chart", "GanttChart"].contains(&row.chart_kind.as_str())
            || row.current_native_sha256.len() != 64
            || !row
                .current_native_sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || !seen.insert(format!("trend:{}", key(&row.attribute, &row.path)?))
        {
            return Err(error("invalid/duplicate current trend identity"));
        }
        row.current.validate()?;
    }
    Ok(value)
}

/// Preserve current properties that the native SDK does not serialize.
pub fn project_chart_semantics(
    body: &FormBody,
    form_uuid: Uuid,
) -> Result<Option<(FormBody, Vec<u8>)>, FormError> {
    let resource = collect(body, form_uuid)?;
    if resource.records.is_empty()
        && resource.values.is_empty()
        && resource.trends.is_empty()
        && resource.designs.is_empty()
        && resource.numbers.is_empty()
    {
        return Ok(None);
    }
    let mut body = body.clone();
    visit_attributes(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            super::chart::project_native_double_fields(chart)?;
            if let Some(row) = resource.designs.iter().find(|r| r.attribute == attribute) {
                project_design(chart, &row.current)?;
            }
            project_values(&mut chart.fields, &mut Vec::new(), false);
            all_fields_mut(&mut chart.fields, &mut Vec::new(), &mut |_, fields| {
                if fields.iter().any(|(n, _)| n == "trendLinesArray") {
                    *fields = trend_transport::projected(fields)?;
                }
                Ok(())
            })
        },
    )?;
    let bytes = serde_json::to_vec_pretty(&resource).map_err(|e| error(e.to_string()))?;
    Ok(Some((body, bytes)))
}
pub fn apply_chart_semantics_resource(
    body: &mut FormBody,
    form_uuid: Uuid,
    bytes: &[u8],
) -> Result<(), FormError> {
    let resource = decode(bytes)?;
    if resource.form_uuid != form_uuid {
        return Err(error("chart resource differs from declared form UUID"));
    }
    let mut pending_numbers: HashSet<_> = resource.numbers.iter()
        .map(|r| key(&r.attribute, &[])).collect::<Result<_, FormError>>()?;
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute { name: a.name.clone(), id: a.id },
        &mut |attribute, chart| {
            let Some(row) = resource.numbers.iter().find(|r| r.attribute == attribute) else { return Ok(()); };
            if !pending_numbers.remove(&key(attribute, &[])?)
                || chart.kind != row.chart_kind
                || design_digest(chart, true)? != row.current_native_sha256
            {
                return Err(error("stale current numeric chart binding"));
            }
            let mut restored = chart.clone();
            restore_numbers(&mut restored, row)?;
            if number_paths(&mut restored)? != row.current || restored.fields != chart.fields {
                return Err(error("current numeric payload conflicts with native projection"));
            }
            Ok(())
        },
    )?;
    if !pending_numbers.is_empty() { return Err(error("orphan current numeric chart binding")); }
    let mut pending_designs: HashSet<_> = resource
        .designs
        .iter()
        .map(|r| key(&r.attribute, &[]))
        .collect::<Result<_, FormError>>()?;
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            if let Some(row) = resource.designs.iter().find(|r| r.attribute == attribute) {
                if !pending_designs.remove(&key(attribute, &[])?)
                    || chart.kind != row.chart_kind
                    || design_digest(chart, true)? != row.current_native_sha256
                    || !super::chart::suppressed_design_fields(chart)?.is_empty()
                {
                    return Err(error("stale/conflicting current native design binding"));
                }
                let mut restored = chart.clone();
                restore_design(&mut restored, row)?;
                if super::chart::suppressed_design_fields(&restored)? != row.current {
                    return Err(error("current design payload conflicts with native flags"));
                }
                project_design(&mut restored, &row.current)?;
                if design_digest(&restored, false)? != row.current_native_sha256 {
                    return Err(error(
                        "current design payload conflicts with native counterpart",
                    ));
                }
            }
            Ok(())
        },
    )?;
    if !pending_designs.is_empty() {
        return Err(error("orphan current design binding"));
    }
    let mut remaining: HashMap<_, _> = resource
        .records
        .iter()
        .map(|row| Ok((key(&row.attribute, &row.path)?, row)))
        .collect::<Result<_, FormError>>()?;
    let mut seen = HashSet::new();
    // Verify the complete binding set before changing even one current flag.
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            inspect_fields(&chart.fields, &mut Vec::new(), &mut |path, kind, item| {
                let identity = key(attribute, path)?;
                if !seen.insert(identity.clone()) {
                    return Err(error("ambiguous current chart item identity"));
                }
                let Some(row) = remaining.remove(&identity) else {
                    return Ok(());
                };
                if row.kind != *kind
                    || row.chart_kind != chart.kind
                    || row.current_item_sha256 != item_digest(item, kind, path, true)?
                {
                    return Err(error("stale or wrong-kind current chart item binding"));
                }
                if flags(item, kind)?.iter().any(|(_, value)| *value) {
                    return Err(error("chart resource conflicts with current native flags"));
                }
                if series_info(item, kind)?.is_some() {
                    return Err(error(
                        "chart resource conflicts with current native SeriesCalcInfo",
                    ));
                }
                if point_aux(item, kind)?.is_some() || interval_color(item, kind)?.is_some() {
                    return Err(error(
                        "chart resource conflicts with current native properties",
                    ));
                }
                if let Some(axis) = &row.axis_inactive {
                    let mut restored = item.to_vec();
                    axis_restore(&mut restored, axis);
                    if axis_inactive(&restored, kind)?.as_ref() != Some(axis) {
                        return Err(error("current axis payload conflicts with boundary mode"));
                    }
                    axis_project(&mut restored, axis);
                    if restored != item {
                        return Err(error(
                            "current axis payload conflicts with active native boundaries",
                        ));
                    }
                }
                Ok(())
            })
        },
    )?;
    if !remaining.is_empty() {
        return Err(error("orphan current chart item binding"));
    }
    let mut value_groups: HashMap<String, Vec<&ValueRecord>> = HashMap::new();
    for row in &resource.values {
        value_groups
            .entry(key(&row.attribute, &row.path)?)
            .or_default()
            .push(row);
    }
    let mut pending: HashSet<_> = value_groups.keys().cloned().collect();
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            all_fields(&chart.fields, &mut Vec::new(), &mut |path, fields| {
                let identity = key(attribute, path)?;
                let Some(rows) = value_groups.get(&identity) else {
                    return Ok(());
                };
                if !pending.remove(&identity) {
                    return Err(error("ambiguous current value parent identity"));
                }
                let digest = parent_digest(fields, path, true)?;
                if rows
                    .iter()
                    .any(|r| r.chart_kind != chart.kind || r.current_parent_sha256 != digest)
                {
                    return Err(error("stale current value parent binding"));
                }
                let mut restored = fields.to_vec();
                restore_values(&mut restored, rows)?;
                project_values(&mut restored, &mut path.to_vec(), false);
                project_values(&mut restored, &mut path.to_vec(), true);
                let mut native = fields.to_vec();
                project_values(&mut native, &mut path.to_vec(), true);
                if restored != native {
                    return Err(error(
                        "current value resource conflicts with current native values",
                    ));
                }
                Ok(())
            })
        },
    )?;
    if !pending.is_empty() {
        return Err(error("orphan current value parent binding"));
    }
    let mut trend_rows: HashMap<_, _> = resource
        .trends
        .iter()
        .map(|r| Ok((key(&r.attribute, &r.path)?, r)))
        .collect::<Result<_, FormError>>()?;
    let mut pending: HashSet<_> = trend_rows.keys().cloned().collect();
    inspect_attributes(
        &body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            all_fields(&chart.fields, &mut Vec::new(), &mut |path, fields| {
                let identity = key(attribute, path)?;
                let Some(row) = trend_rows.get(&identity) else {
                    return Ok(());
                };
                if !pending.remove(&identity)
                    || row.chart_kind != chart.kind
                    || row.current_native_sha256 != trend_digest(fields, true)?
                {
                    return Err(error("stale/ambiguous current trend binding"));
                }
                let mut restored = fields.to_vec();
                row.current.restore(&mut restored)?;
                if TrendTopology::collect(&restored)?.as_ref() != Some(&row.current)
                    || trend_digest(&restored, false)? != row.current_native_sha256
                {
                    return Err(error("current trend resource conflicts with native lines"));
                }
                Ok(())
            })
        },
    )?;
    if !pending.is_empty() {
        return Err(error("orphan current trend binding"));
    }
    let records: HashMap<_, _> = resource
        .records
        .iter()
        .map(|row| Ok((key(&row.attribute, &row.path)?, row)))
        .collect::<Result<_, FormError>>()?;
    visit_attributes(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            visit_fields(&mut chart.fields, &mut Vec::new(), &mut |path, _, item| {
                if let Some(row) = records.get(&key(attribute, path)?) {
                    for (name, value) in &row.flags {
                        item.iter_mut().find(|(n, _)| n == name).unwrap().1 =
                            ChartValue::Bool(*value);
                    }
                    if let Some(info) = &row.series_info {
                        item.iter_mut().find(|(name, _)| name == "info").unwrap().1 =
                            info.current();
                    }
                    if let Some(point) = &row.point_aux {
                        for (name, value) in item.iter_mut() {
                            *value = match name.as_str() {
                                "intAdd" => ChartValue::Int(point.int_add.to_string()),
                                "doubleAdd" => ChartValue::Int(point.double_add.clone()),
                                "endX" => ChartValue::Int(point.end_x.to_string()),
                                _ => continue,
                            };
                        }
                    }
                    if let Some(color) = &row.interval_text_color {
                        item.iter_mut()
                            .find(|(name, _)| name == "textColor")
                            .unwrap()
                            .1 = ChartValue::Color(color.clone());
                    }
                    if let Some(axis) = &row.axis_inactive {
                        axis_restore(item, axis);
                    }
                }
                Ok(())
            })
        },
    )?;
    visit_attributes(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            all_fields_mut(&mut chart.fields, &mut Vec::new(), &mut |path, fields| {
                let identity = key(attribute, path)?;
                if let Some(rows) = value_groups.get(&identity) {
                    restore_values(fields, rows)?;
                }
                if let Some(row) = trend_rows.remove(&identity) {
                    row.current.restore(fields)?;
                }
                Ok(())
            })
        },
    )?;
    visit_attributes(
        &mut body.data_attributes,
        &mut Vec::new(),
        &|a| AttributeEdge::Attribute {
            name: a.name.clone(),
            id: a.id,
        },
        &mut |attribute, chart| {
            if let Some(row) = resource.designs.iter().find(|r| r.attribute == attribute) {
                restore_design(chart, row)?;
            }
            if let Some(row) = resource.numbers.iter().find(|r| r.attribute == attribute) {
                restore_numbers(chart, row)?;
            }
            Ok(())
        },
    )?;
    Ok(())
}
fn restore_values(
    fields: &mut Vec<(String, ChartValue)>,
    rows: &[&ValueRecord],
) -> Result<(), FormError> {
    let mut rows = rows.to_vec();
    rows.sort_by_key(|r| r.field_rank);
    for row in rows {
        let value = ChartValue::Value(Box::new(row.current.clone()));
        if let Some(index) = fields.iter().position(|(name, _)| name == &row.field) {
            if index != row.field_rank {
                return Err(error("stale current value field order"));
            }
            fields[index].1 = value;
        } else if row.field_rank <= fields.len() {
            fields.insert(row.field_rank, (row.field.clone(), value));
        } else {
            return Err(error("invalid current value field order"));
        }
    }
    Ok(())
}
pub fn same_chart_semantics_resource(a: &[u8], b: &[u8]) -> bool {
    match (decode(a), decode(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
pub fn chart_semantics_resource_count(body: &FormBody) -> Result<Option<usize>, FormError> {
    let resource = collect(body, Uuid([0; 16]))?;
    let count = resource
        .records
        .len()
        .checked_add(resource.values.len())
        .and_then(|n| n.checked_add(resource.trends.len()))
        .and_then(|n| n.checked_add(resource.designs.len()))
        .and_then(|n| n.checked_add(resource.numbers.len()))
        .ok_or_else(|| error("chart reference count overflow"))?;
    Ok((count > 0).then_some(count))
}
