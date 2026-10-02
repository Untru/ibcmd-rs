//! Typed current SeriesCalcInfo, persisted by EDT and omitted by native SDK XML.
use super::FormError;
use morph1c_core::ir::form::ChartValue;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeriesInfo {
    enabled: bool,
    min: String,
    max: String,
    abs_min: String,
    abs_max: String,
    start_angle: String,
    stop_angle: String,
    percent: String,
    text: Option<String>,
    expanded: bool,
    center: Option<Center>,
    end_x: i32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Center {
    x: i32,
    y: i32,
}
fn error() -> FormError {
    FormError::Frame("chart semantics: malformed current SeriesCalcInfo".into())
}

impl SeriesInfo {
    pub(super) fn read(value: &ChartValue) -> Result<Self, FormError> {
        let ChartValue::Nested(fields) = value else {
            return Err(error());
        };
        let mut result = Self {
            enabled: false,
            min: "0".into(),
            max: "0".into(),
            abs_min: "0".into(),
            abs_max: "0".into(),
            start_angle: "0".into(),
            stop_angle: "0".into(),
            percent: "0".into(),
            text: None,
            expanded: false,
            center: None,
            end_x: 0,
        };
        let mut seen = std::collections::HashSet::new();
        for (name, value) in fields {
            if !seen.insert(name) {
                return Err(error());
            }
            match (name.as_str(), value) {
                ("enabled", ChartValue::Bool(v)) => result.enabled = *v,
                ("m_isExpand", ChartValue::Bool(v)) => result.expanded = *v,
                ("str", ChartValue::Absent) => {}
                ("str", ChartValue::Str(v)) => result.text = Some(v.clone()),
                ("endX", ChartValue::Int(v)) => result.end_x = v.parse().map_err(|_| error())?,
                ("m_centerPoint", ChartValue::Absent) => {}
                ("m_centerPoint", ChartValue::Nested(point)) => {
                    let mut center = Center { x: 0, y: 0 };
                    let mut seen = std::collections::HashSet::new();
                    for (name, value) in point {
                        if !seen.insert(name) {
                            return Err(error());
                        }
                        let ChartValue::Int(value) = value else {
                            return Err(error());
                        };
                        match name.as_str() {
                            "x" => center.x = value.parse().map_err(|_| error())?,
                            "y" => center.y = value.parse().map_err(|_| error())?,
                            _ => return Err(error()),
                        }
                    }
                    result.center = Some(center);
                }
                (
                    name @ ("min" | "max" | "absMin" | "absMax" | "startAngle" | "stopAngle"
                    | "percent"),
                    ChartValue::Int(v),
                ) => {
                    if !matches!(v.as_str(), "Infinity" | "-Infinity") && v.parse::<f64>().is_err()
                    {
                        return Err(error());
                    }
                    let target = match name {
                        "min" => &mut result.min,
                        "max" => &mut result.max,
                        "absMin" => &mut result.abs_min,
                        "absMax" => &mut result.abs_max,
                        "startAngle" => &mut result.start_angle,
                        "stopAngle" => &mut result.stop_angle,
                        _ => &mut result.percent,
                    };
                    *target = v.clone();
                }
                _ => return Err(error()),
            }
        }
        Ok(result)
    }
    pub(super) fn current(&self) -> ChartValue {
        ChartValue::Nested(vec![
            ("enabled".into(), ChartValue::Bool(self.enabled)),
            ("min".into(), ChartValue::Int(self.min.clone())),
            ("max".into(), ChartValue::Int(self.max.clone())),
            ("absMin".into(), ChartValue::Int(self.abs_min.clone())),
            ("absMax".into(), ChartValue::Int(self.abs_max.clone())),
            (
                "startAngle".into(),
                ChartValue::Int(self.start_angle.clone()),
            ),
            ("stopAngle".into(), ChartValue::Int(self.stop_angle.clone())),
            ("percent".into(), ChartValue::Int(self.percent.clone())),
            (
                "str".into(),
                self.text
                    .as_ref()
                    .map_or(ChartValue::Absent, |v| ChartValue::Str(v.clone())),
            ),
            ("m_isExpand".into(), ChartValue::Bool(self.expanded)),
            (
                "m_centerPoint".into(),
                self.center.as_ref().map_or(ChartValue::Absent, |v| {
                    ChartValue::Nested(vec![
                        ("x".into(), ChartValue::Int(v.x.to_string())),
                        ("y".into(), ChartValue::Int(v.y.to_string())),
                    ])
                }),
            ),
            ("endX".into(), ChartValue::Int(self.end_x.to_string())),
        ])
    }
    pub(super) fn validate(&self) -> Result<(), FormError> {
        Self::read(&self.current()).map(|_| ())
    }
}
