//! A form body record and its reader (from `src/module_blob.rs`, which
//! re-exports the two types under their old public paths, reaches the reader
//! and the container by their old names through imports, and keeps the
//! inflating entry point `parse_form_body_blob`). The reader moved unchanged
//! but for one mechanical clippy fix (`needless_borrow` in
//! `parse_form_body_plain`) and the revision reader, a root trait over this
//! crate's type there and a private method here.

use std::ops::Range;

use anyhow::{Context, Result, anyhow};

use crate::form_root_42;
use crate::record_text::{parse_1c_quoted_string, scan_braced_fields};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ParsedFormBodyBlob {
    /// The container revision the record declared in its own first slot.
    pub revision: FormBodyRevision,
    pub layout: String,
    pub module_text: String,
    pub trailing: Vec<String>,
    pub trailing_fields: usize,
}

/// The container revision a Form body declares in its own first slot.
///
/// Both admitted revisions carry the identical ten-slot container -- marker,
/// layout, module text, five blocks and two scalars. Measured over every form
/// of all eight stand corpora (22,646 forms): revision `4` on 22,535 managed
/// forms of all eight, revision `3` on 102 managed forms of ERP УХ 3.2.12.6
/// only, and no other value anywhere. Layout revision `49` occurs under both
/// container revisions, layout `50` only under `4`. See
/// `docs/evidence/uh-form-body-container-revision-20260826.md`.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FormBodyRevision {
    V3,
    V4,
}

impl FormBodyRevision {
    pub const fn declared_marker(self) -> &'static str {
        match self {
            Self::V3 => "3",
            Self::V4 => "4",
        }
    }
}

pub fn parse_form_body_plain(plain: &str) -> Result<ParsedFormBodyBlob> {
    // An older platform's root-`42` form is read as 8.3.27 re-saves it.
    let upgraded = form_root_42::upgrade_root_42_body(plain);
    let plain = upgraded.as_deref().unwrap_or(plain);
    let container = FormBodyContainer::parse(plain)?;
    let layout = plain[container.layout_range.clone()].trim().to_string();
    let module_text = parse_1c_quoted_string(plain[container.module_range.clone()].trim())
        .context("Form body module text field is not a quoted string")?;
    Ok(ParsedFormBodyBlob {
        revision: container.revision,
        layout,
        module_text,
        trailing: container
            .trailing_ranges
            .into_iter()
            .map(|range| plain[range].trim().to_string())
            .collect(),
        trailing_fields: container.trailing_fields,
    })
}

/// Reading the revision a record declares is a step of the container check
/// alone, so it is not public.
impl FormBodyRevision {
    /// Reads the revision the record itself declares. Fail-closed: an
    /// undeclared or unnamed revision is refused, never assumed.
    fn parse(declared: Option<&str>) -> Result<Self> {
        match declared {
            Some("3") => Ok(Self::V3),
            Some("4") => Ok(Self::V4),
            Some(other) => Err(anyhow!(
                "Form body declares unsupported container revision {other}"
            )),
            None => Err(anyhow!("Form body declares no container revision")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormBodyContainer {
    revision: FormBodyRevision,
    pub layout_range: Range<usize>,
    pub module_range: Range<usize>,
    pub trailing_ranges: Vec<Range<usize>>,
    trailing_fields: usize,
}

impl FormBodyContainer {
    pub fn parse(plain: &str) -> Result<Self> {
        let body_start = plain
            .find('{')
            .ok_or_else(|| anyhow!("Form body has no braced payload"))?;
        let fields = scan_braced_fields(plain, body_start)?;
        let revision =
            FormBodyRevision::parse(fields.first().map(|range| plain[range.clone()].trim()))?;
        let layout_range = fields
            .get(1)
            .ok_or_else(|| anyhow!("Form body has no layout field"))?
            .clone();
        validate_form_body_layout(plain[layout_range.clone()].trim())?;
        let module_range = fields
            .get(2)
            .ok_or_else(|| anyhow!("Form body has no module text field"))?
            .clone();
        if parse_1c_quoted_string(plain[module_range.clone()].trim()).is_err() {
            return Err(anyhow!(
                "Form body module text field is not a quoted string"
            ));
        }
        Ok(Self {
            revision,
            layout_range,
            module_range,
            trailing_ranges: fields.iter().skip(3).cloned().collect(),
            trailing_fields: fields.len().saturating_sub(3),
        })
    }
}

fn validate_form_body_layout(layout: &str) -> Result<()> {
    let fields = scan_braced_fields(layout, 0)?;
    if !fields.first().is_some_and(|range| {
        layout[range.clone()]
            .trim()
            .chars()
            .all(|ch| ch.is_ascii_digit())
    }) {
        return Err(anyhow!(
            "Form body layout field does not start with a numeric marker"
        ));
    }
    Ok(())
}
