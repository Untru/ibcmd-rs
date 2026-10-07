//! Original StandardPeriodValue projection: a nonnull period, nullable dates.
use super::*;
use morph1c_core::ir::form::ChartTypedValue;
const VARIANTS: &[&str] = &[
    "Custom",
    "Today",
    "ThisWeek",
    "ThisTenDays",
    "ThisMonth",
    "ThisQuarter",
    "ThisHalfYear",
    "ThisYear",
    "FromBeginningOfThisWeek",
    "FromBeginningOfThisTenDays",
    "FromBeginningOfThisMonth",
    "FromBeginningOfThisQuarter",
    "FromBeginningOfThisHalfYear",
    "FromBeginningOfThisYear",
    "Yesterday",
    "LastWeek",
    "LastTenDays",
    "LastMonth",
    "LastQuarter",
    "LastHalfYear",
    "LastYear",
    "LastWeekTillSameWeekDay",
    "LastTenDaysTillSameDayNumber",
    "LastMonthTillSameDate",
    "LastQuarterTillSameDate",
    "LastHalfYearTillSameDate",
    "LastYearTillSameDate",
    "Tomorrow",
    "NextWeek",
    "NextTenDays",
    "NextMonth",
    "NextQuarter",
    "NextHalfYear",
    "NextYear",
    "NextWeekTillSameWeekDay",
    "NextTenDaysTillSameDayNumber",
    "NextMonthTillSameDate",
    "NextQuarterTillSameDate",
    "NextHalfYearTillSameDate",
    "NextYearTillSameDate",
    "TillEndOfThisWeek",
    "TillEndOfThisTenDays",
    "TillEndOfThisMonth",
    "TillEndOfThisQuarter",
    "TillEndOfThisHalfYear",
    "TillEndOfThisYear",
    "Last7Days",
    "Next7Days",
    "Month",
];

fn checked(
    variant: &str,
    start: Option<&str>,
    end: Option<&str>,
    path: &str,
) -> Result<(), FormError> {
    if !VARIANTS.contains(&variant) {
        return Err(frame(format!(
            "chart {path}: unknown StandardPeriodVariant"
        )));
    }
    if start.is_some_and(str::is_empty) || end.is_some_and(str::is_empty) {
        return Err(frame(format!("chart {path}: empty StandardPeriod date")));
    }
    Ok(())
}
pub(super) fn read(el: &Element, path: &str, native: bool) -> Result<ChartTypedValue, FormError> {
    el.claim();
    claim_xsi(
        el,
        if native {
            "v8:StandardPeriod"
        } else {
            "core:StandardPeriodValue"
        },
        path,
    )?;
    expect_attrs_claimed(el, path)?;
    if !el.text.is_empty() {
        return Err(frame(format!("chart {path}: text outside StandardPeriod")));
    }
    let container = if native {
        el
    } else {
        let [value] = el.children.as_slice() else {
            return Err(frame(format!(
                "chart {path}: StandardPeriodValue requires nonnull value"
            )));
        };
        if !value.prefix.is_empty() || value.local != "value" {
            return Err(frame(format!(
                "chart {path}: invalid StandardPeriod value role"
            )));
        }
        value.claim();
        expect_attrs_claimed(value, path)?;
        if !value.text.is_empty() {
            return Err(frame(format!(
                "chart {path}: text in StandardPeriod container"
            )));
        }
        value
    };
    let mut fields: [Option<String>; 3] = [None, None, None];
    for child in &container.children {
        if child.prefix != if native { "v8" } else { "" } {
            return Err(frame(format!(
                "chart {path}: wrong StandardPeriod field namespace"
            )));
        }
        let index = match child.local.as_str() {
            "variant" => 0,
            "startDate" => 1,
            "endDate" => 2,
            _ => return Err(frame(format!("chart {path}: unknown StandardPeriod field"))),
        };
        if fields[index].is_some() {
            return Err(frame(format!(
                "chart {path}: duplicate StandardPeriod field"
            )));
        }
        if native && index == 0 {
            claim_xsi(child, "v8:StandardPeriodVariant", path)?;
        }
        fields[index] = Some(leaf_text(child, path)?);
    }
    let [variant, start, end] = fields;
    let variant = variant.unwrap_or_else(|| "Custom".into());
    checked(&variant, start.as_deref(), end.as_deref(), path)?;
    Ok(ChartTypedValue::StandardPeriod {
        variant,
        start,
        end,
    })
}
pub(super) fn write(
    name: &str,
    value: &ChartTypedValue,
    path: &str,
    native: bool,
) -> Result<OutElement, FormError> {
    let ChartTypedValue::StandardPeriod {
        variant,
        start,
        end,
    } = value
    else {
        return Err(frame(format!(
            "chart {path}: expected current StandardPeriod"
        )));
    };
    checked(variant, start.as_deref(), end.as_deref(), path)?;
    let mut inner = OutElement::branch("", "value");
    if native || variant != "Custom" {
        let mut field = OutElement::leaf(if native { "v8" } else { "" }, "variant", variant);
        if native {
            field = field.attr("xsi:type", "v8:StandardPeriodVariant");
        }
        inner.push(field);
    }
    for (name, date) in [("startDate", start), ("endDate", end)] {
        if let Some(date) = date {
            inner.push(OutElement::leaf(if native { "v8" } else { "" }, name, date));
        }
    }
    let mut outer = OutElement::branch(if native { "d4p1" } else { "" }, name).attr(
        "xsi:type",
        if native {
            "v8:StandardPeriod"
        } else {
            "core:StandardPeriodValue"
        },
    );
    if native {
        outer.children = inner.children;
    } else {
        inner.self_closing = inner.children.is_empty();
        outer.push(inner);
    }
    Ok(outer)
}
