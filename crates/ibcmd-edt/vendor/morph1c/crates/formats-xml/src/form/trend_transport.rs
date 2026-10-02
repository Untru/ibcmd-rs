//! Current trend-array topology and only lines that native XML cannot encode.
use super::{FormError, chart};
use morph1c_core::ir::form::{ChartSettings, ChartValue};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TrendTopology {
    arrays: Vec<Array>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Array {
    series_id: i32,
    lines: Lines,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Lines {
    Native(usize),
    Unprojected(Vec<Vec<(String, ChartValue)>>),
}
fn error() -> FormError {
    FormError::Frame("chart semantics: invalid current trend topology".into())
}
fn arrays(fields: &[(String, ChartValue)]) -> Result<&[Vec<(String, ChartValue)>], FormError> {
    let mut found = fields.iter().filter(|(n, _)| n == "trendLinesArray");
    let Some((_, ChartValue::Items(value))) = found.next() else {
        return Err(error());
    };
    if found.next().is_some() {
        return Err(error());
    }
    Ok(value)
}
fn components(
    fields: &[(String, ChartValue)],
) -> Result<(i32, &Vec<Vec<(String, ChartValue)>>), FormError> {
    if fields.len() != 2 {
        return Err(error());
    }
    let Some((_, ChartValue::Int(id))) = fields.iter().find(|(n, _)| n == "seriesId") else {
        return Err(error());
    };
    let Some((_, ChartValue::Items(lines))) = fields.iter().find(|(n, _)| n == "line") else {
        return Err(error());
    };
    Ok((id.parse().map_err(|_| error())?, lines))
}
pub(super) fn projected(
    fields: &[(String, ChartValue)],
) -> Result<Vec<(String, ChartValue)>, FormError> {
    chart::project_native_trends(&ChartSettings {
        kind: "Chart".into(),
        fields: fields.to_vec(),
        source_layout: None,
    })
    .map(|c| c.fields)
}
impl TrendTopology {
    pub(super) fn collect(fields: &[(String, ChartValue)]) -> Result<Option<Self>, FormError> {
        let current = arrays(fields)?;
        let projected = projected(fields)?;
        let native = arrays(&projected)?;
        if current == native {
            return Ok(None);
        }
        let native: Vec<_> = native
            .iter()
            .map(|a| components(a).map(|(id, lines)| (id, lines.clone())))
            .collect::<Result<_, _>>()?;
        let mut seen = std::collections::HashSet::new();
        let mut result = Vec::new();
        for array in current {
            let (id, lines) = components(array)?;
            let reference = seen
                .insert(id)
                .then(|| native.iter().position(|(n, _)| *n == id))
                .flatten();
            result.push(Array {
                series_id: id,
                lines: reference.map_or_else(|| Lines::Unprojected(lines.clone()), Lines::Native),
            });
        }
        Ok(Some(Self { arrays: result }))
    }
    pub(super) fn validate(&self) -> Result<(), FormError> {
        for array in &self.arrays {
            if let Lines::Unprojected(lines) = &array.lines {
                // Reuse the closed field tables and lexical validators, including nested lines.
                let chart = ChartSettings {
                    kind: "Chart".into(),
                    fields: vec![(
                        "trendLinesArray".into(),
                        ChartValue::Items(vec![vec![
                            (
                                "seriesId".into(),
                                ChartValue::Int(array.series_id.to_string()),
                            ),
                            ("line".into(), ChartValue::Items(lines.clone())),
                        ]]),
                    )],
                    source_layout: None,
                };
                let xml = chart::write_chart_sidecar(&chart)?;
                chart::read_chart_sidecar(&xml)?;
            }
        }
        Ok(())
    }
    pub(super) fn restore(&self, fields: &mut [(String, ChartValue)]) -> Result<(), FormError> {
        let native = arrays(fields)?.to_vec();
        let mut result = Vec::new();
        for array in &self.arrays {
            let lines = match &array.lines {
                Lines::Native(index) => {
                    let (id, lines) = components(native.get(*index).ok_or_else(error)?)?;
                    if id != array.series_id {
                        return Err(error());
                    }
                    lines.clone()
                }
                Lines::Unprojected(lines) => lines.clone(),
            };
            result.push(vec![
                (
                    "seriesId".into(),
                    ChartValue::Int(array.series_id.to_string()),
                ),
                ("line".into(), ChartValue::Items(lines)),
            ]);
        }
        fields
            .iter_mut()
            .find(|(n, _)| n == "trendLinesArray")
            .ok_or_else(error)?
            .1 = ChartValue::Items(result);
        Ok(())
    }
}
pub(super) fn binding(fields: &[(String, ChartValue)]) -> Result<Vec<ChartValue>, FormError> {
    let mut result = vec![ChartValue::Items(arrays(fields)?.to_vec())];
    for name in ["realSeriesData", "realExSeriesData"] {
        let Some((_, value)) = fields.iter().find(|(n, _)| n == name) else {
            continue;
        };
        let rows: Vec<_> = match value {
            ChartValue::Items(rows) if name == "realSeriesData" => rows.iter().collect(),
            ChartValue::Nested(row) if name == "realExSeriesData" => vec![row],
            _ => return Err(error()),
        };
        for row in rows {
            let id = row
                .iter()
                .find(|(n, _)| n == "id")
                .map(|(_, v)| v.clone())
                .unwrap_or_else(|| ChartValue::Int("1".into()));
            result.push(id);
        }
    }
    Ok(result)
}
