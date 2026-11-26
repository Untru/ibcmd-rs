//! Type descriptions the extension writer (`crate::extension`) renders on its
//! own: the extend value of an adopted property whose type the extension
//! widens, kept in the object's adoption header rather than in a field the
//! pipeline reads.

use std::collections::BTreeMap;

use super::metadata::{MetadataTextRowAudit, metadata_text_row_audit_from_blob};
use super::{
    ConstantValueType, build_metadata_type_index_from_texts, format_metadata_types_xml_with_indent,
    parse_metadata_type_pattern, split_1c_braced_fields,
};

/// The type index of an extension's metadata rows.
pub(crate) struct ExtensionTypes {
    index: BTreeMap<String, String>,
}

/// The `TypeDescription` class of a `{"#",<class>,<value>}` wrapper.
const TYPE_DESCRIPTION: &str = "f5c65050-3bbb-11d5-b988-0050bae0a95d";

impl ExtensionTypes {
    /// `(name, packed payload)` rows of the container; module and form
    /// bodies (names with a suffix) are skipped.
    pub(crate) fn from_entries(entries: &[(String, Vec<u8>)]) -> Self {
        let rows = entries
            .iter()
            .filter(|(name, _)| name.len() == 36 && !name.contains('.'))
            .filter_map(
                |(name, payload)| match metadata_text_row_audit_from_blob(name, payload) {
                    MetadataTextRowAudit::Extracted(row)
                    | MetadataTextRowAudit::ExtractedWithWarning(row, _) => Some(row),
                    MetadataTextRowAudit::Miss(_) => None,
                },
            )
            .collect::<Vec<_>>();
        Self {
            index: build_metadata_type_index_from_texts(&rows),
        }
    }

    /// The `<v8:Type>`… lines of `value` (`{"Pattern",…}`, or it wrapped as
    /// `{"#",<TypeDescription>,{"Pattern",…}}`) at `indent`, as a metadata
    /// `<Type>` holds them; `None` when a member is not known.
    pub(crate) fn render(&self, value: &str, indent: &str) -> Option<String> {
        let value = value.trim();
        let fields = split_1c_braced_fields(value, 0)?;
        let pattern = if fields.first()?.trim() == r##""#""## {
            if fields.get(1)?.trim() != TYPE_DESCRIPTION || fields.len() != 3 {
                return None;
            }
            fields[2].trim()
        } else {
            value
        };
        let types = parse_metadata_type_pattern(pattern, &self.index)?
            .into_iter()
            .map(|value_type| match value_type {
                // A checked `AnyIBRef` is a type set (a real extension's
                // dimension; no configuration dump in the corpora prints it
                // in metadata, so the shared writer is left alone).
                ConstantValueType::Reference { reference } if reference == "cfg:AnyIBRef" => {
                    ConstantValueType::ReferenceTypeSet { reference }
                }
                other => other,
            })
            .collect::<Vec<_>>();
        let outer = indent.strip_suffix('\t')?;
        let xml = format_metadata_types_xml_with_indent(&types, outer);
        let inner = xml
            .strip_prefix(&format!("{outer}<Type>\r\n"))?
            .strip_suffix(&format!("{outer}</Type>\r\n"))?;
        Some(inner.to_owned())
    }
}
