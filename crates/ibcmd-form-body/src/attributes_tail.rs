//! The physical layout of the Form Attributes tail of a packed form body: the
//! sub-tables, the `sourcePath -> resolvedPath` bindings and the Base64
//! storage document of the form-wide conditional appearance, moved unchanged
//! from `src/module_blob.rs`. The form body reader reads the conditional
//! appearance through the two readers at the end; the root's form packer
//! reads the existing value through the second and rewrites the binding block
//! the walk locates. The root reaches every item by its old name through
//! imports.

use std::ops::Range;

use anyhow::{Result, anyhow};
use ibcmd_core::dcs::DcsConditionalAppearance;
use ibcmd_schema::bundled_dcs_form_attributes_conditional_appearance_policy;
use ibcmd_xml::{
    DcsChildParseOutcome, parse_form_attributes_conditional_appearance_storage_document,
    parse_form_attributes_empty_storage_document,
};

use crate::record_text::{
    decode_base64_payload_field, parse_1c_quoted_string, scan_balanced_braces, scan_braced_fields,
};

/// The physical layout of everything a packed form body writes after its
/// declared attribute entries.
///
/// Read off the packed body of all 4 997 forms the UT 11.5.27.75
/// configuration carries, with no counterexample:
///
/// ```text
/// tail    = 4, attrCount, attribute x attrCount, EXTRAS
/// EXTRAS  = subCount, subTable x subCount,
///           bindingCount, bindingField x (4 * bindingCount),
///           storage document
/// binding = "sourcePath", "resolvedPath", resolvedChain, ownerChain
/// chain   = n, segment x n         segment = index | index, uuid
/// ```
///
/// The two leading fields the previous reading took for a pair of activity
/// markers are these two counts: `0,0` is a body with no sub-tables and no
/// bindings, `0,1` one with a single binding. Reading them as an enumerated
/// marker pair accounted for 4 538 of the 4 997 bodies and silently mis-read
/// the rest, because a body with sub-tables, or with more than one binding,
/// has no marker to match.
pub struct FormAttributesTailLayout {
    /// How many `sourcePath -> resolvedPath` bindings the tail declares.
    pub binding_count: usize,
    /// The trailing field carrying the Base64 storage document.
    payload: Range<usize>,
    /// Everything from the binding count through the storage document. The
    /// sub-tables sit before it and belong to properties this code does not
    /// own, so a rewrite replaces exactly this span and nothing earlier.
    pub binding_block: Range<usize>,
}

/// Walks the packed tail of a form body and reports where the storage
/// document sits, or `Ok(None)` when the body carries no such tail at all.
///
/// Every field is accounted for: a tail whose counts do not consume it
/// exactly is refused rather than read past, so a layout this walk does not
/// describe can never be mistaken for one it does.
pub fn scan_form_attributes_tail_layout(text: &str) -> Result<Option<FormAttributesTailLayout>> {
    let fields = scan_braced_fields(text, 0)?;
    let policy = bundled_dcs_form_attributes_conditional_appearance_policy()
        .map_err(|error| anyhow!(error.to_string()))?;
    let container_marker = fields.first().map(|range| text[range.clone()].trim());
    if container_marker != Some(policy.storage_container_marker()) {
        return Ok(None);
    }
    let count = fields
        .get(1)
        .and_then(|range| text[range.clone()].trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Form Attributes container has no valid declared count"))?;
    let extras_start = 2usize
        .checked_add(count)
        .ok_or_else(|| anyhow!("Form Attributes declared count overflows"))?;
    if fields.len() < extras_start {
        return Err(anyhow!(
            "Form Attributes container has fewer entries than its declared count"
        ));
    }
    let extras = &fields[extras_start..];
    if extras.is_empty() {
        return Ok(None);
    }
    let sub_count = text[extras[0].clone()]
        .trim()
        .parse::<usize>()
        .map_err(|_| anyhow!("Form Attributes tail has no valid sub-table count"))?;
    let mut index = 1usize
        .checked_add(sub_count)
        .ok_or_else(|| anyhow!("Form Attributes tail sub-table count overflows"))?;
    let binding_block_start = extras
        .get(index)
        .ok_or_else(|| anyhow!("Form Attributes tail sub-tables overrun the container"))?
        .start;
    let binding_count = text[extras[index].clone()]
        .trim()
        .parse::<usize>()
        .map_err(|_| anyhow!("Form Attributes tail has no valid binding count"))?;
    index += 1;
    for _ in 0..binding_count {
        let binding = extras
            .get(index..index + FORM_ATTRIBUTES_TAIL_BINDING_FIELDS)
            .ok_or_else(|| anyhow!("Form Attributes tail bindings overrun the container"))?;
        validate_form_attributes_tail_binding(text, binding)?;
        index += FORM_ATTRIBUTES_TAIL_BINDING_FIELDS;
    }
    if index + 1 != extras.len() {
        return Err(anyhow!(
            "Form Attributes tail carries fields its declared counts do not account for"
        ));
    }
    let payload = extras[index].clone();
    if !text[payload.clone()].trim().starts_with("{#base64:") {
        // The trailing field is not a storage document at all. All 4 997 UT
        // bodies do carry one -- a conditional appearance has nowhere else to
        // live -- so this is a body that has none rather than one whose
        // document failed to read, and only a tail that binds a path can
        // contradict that.
        if binding_count != 0 {
            return Err(anyhow!(
                "Form Attributes tail declares bindings but carries no storage document"
            ));
        }
        return Ok(None);
    }
    Ok(Some(FormAttributesTailLayout {
        binding_count,
        binding_block: binding_block_start..payload.end,
        payload,
    }))
}

/// `"sourcePath"`, `"resolvedPath"`, `resolvedChain`, `ownerChain`.
const FORM_ATTRIBUTES_TAIL_BINDING_FIELDS: usize = 4;

/// Authenticates one binding against the two relations that hold across all
/// 349 bindings the UT configuration carries.
///
/// Field 2 is the path as the settings document spells it and field 3 is the
/// same path in its storage spelling (`ПометкаУдаления` -> `DeletionMark`),
/// empty when it does not resolve. The chain in field 4 carries one segment
/// per dot-separated step of that resolved path, and the owner chain in
/// field 5 is either empty or a prefix of it. Both relations are checked
/// here, so a tail split on the wrong field boundaries cannot pass.
fn validate_form_attributes_tail_binding(text: &str, fields: &[Range<usize>]) -> Result<()> {
    let [source_path, resolved_path, resolved_chain, owner_chain] = fields else {
        return Err(anyhow!("Form Attributes tail binding is not four fields"));
    };
    parse_1c_quoted_string(&text[source_path.clone()])?;
    let resolved = parse_1c_quoted_string(&text[resolved_path.clone()])?;
    let resolved_chain = parse_form_attributes_tail_chain(text, resolved_chain)?;
    let owner_chain = parse_form_attributes_tail_chain(text, owner_chain)?;
    let steps = if resolved.is_empty() {
        0
    } else {
        resolved.split('.').count()
    };
    if resolved_chain.len() != steps {
        return Err(anyhow!(
            "Form Attributes tail binding chain does not match its resolved path"
        ));
    }
    if owner_chain.len() > resolved_chain.len()
        || owner_chain != resolved_chain[..owner_chain.len()]
    {
        return Err(anyhow!(
            "Form Attributes tail binding owner chain is not a prefix of its resolved chain"
        ));
    }
    Ok(())
}

/// Reads one `{n, segment x n}` chain, returning its segments verbatim so the
/// owner/resolved prefix relation can be compared without interpreting them.
fn parse_form_attributes_tail_chain(text: &str, range: &Range<usize>) -> Result<Vec<String>> {
    if scan_balanced_braces(text, range.start)? != range.end {
        return Err(anyhow!(
            "Form Attributes tail chain does not span its own field"
        ));
    }
    let fields = scan_braced_fields(text, range.start)?;
    let count = fields
        .first()
        .and_then(|field| text[field.clone()].trim().parse::<usize>().ok())
        .ok_or_else(|| anyhow!("Form Attributes tail chain has no valid segment count"))?;
    if fields.len() != count + 1 {
        return Err(anyhow!(
            "Form Attributes tail chain declares a segment count it does not carry"
        ));
    }
    let mut segments = Vec::with_capacity(count);
    for field in fields.iter().skip(1) {
        let segment = scan_braced_fields(text, field.start)?;
        let index = text[segment
            .first()
            .ok_or_else(|| anyhow!("Form Attributes tail chain segment is empty"))?
            .clone()]
        .trim();
        if index.parse::<i64>().is_err() {
            return Err(anyhow!(
                "Form Attributes tail chain segment has no numeric index"
            ));
        }
        match segment.len() {
            1 => segments.push(index.to_string()),
            2 => {
                let uuid = text[segment[1].clone()].trim();
                if uuid.is_empty()
                    || !uuid
                        .bytes()
                        .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
                {
                    return Err(anyhow!(
                        "Form Attributes tail chain segment reference is not a UUID"
                    ));
                }
                segments.push(format!("{index}|{uuid}"));
            }
            _ => {
                return Err(anyhow!(
                    "Form Attributes tail chain segment is outside the evidenced shape"
                ));
            }
        }
    }
    Ok(segments)
}

/// The storage document the packed tail carries, or `Ok(None)` when the body
/// has no tail. Callers that cannot type the document re-spell it instead.
pub(crate) fn form_attributes_conditional_appearance_tail_storage_document(
    text: &str,
) -> Result<Option<Vec<u8>>> {
    let Some(layout) = scan_form_attributes_tail_layout(text)? else {
        return Ok(None);
    };
    Ok(Some(decode_base64_payload_field(
        text[layout.payload].trim(),
    )?))
}

pub fn parse_form_attributes_conditional_appearance_tail(
    text: &str,
) -> Result<DcsChildParseOutcome<DcsConditionalAppearance>> {
    let Some(layout) = scan_form_attributes_tail_layout(text)? else {
        return Ok(DcsChildParseOutcome::Absent);
    };
    let bytes = decode_base64_payload_field(text[layout.payload].trim())?;
    if layout.binding_count == 0 {
        // A tail that binds no path carries the empty settings document, and
        // that pairing holds for all 4 871 such UT bodies. Requiring it here
        // keeps a document with content from being dropped as if absent.
        parse_form_attributes_empty_storage_document(&bytes)
            .map_err(|error| anyhow!(error.to_string()))?;
        return Ok(DcsChildParseOutcome::Absent);
    }
    match parse_form_attributes_conditional_appearance_storage_document(&bytes)
        .map_err(|error| anyhow!(error.to_string()))?
    {
        // The converse pairing: all 126 UT bodies that bind a path carry a
        // conditional appearance. A document that reads as absent against a
        // tail that declares bindings is not understood, so it is handed on
        // as unsupported rather than silently dropping the property.
        DcsChildParseOutcome::Absent => Ok(DcsChildParseOutcome::Unsupported(
            "Form Attributes tail declares bindings but its storage document reads as absent",
        )),
        outcome => Ok(outcome),
    }
}
