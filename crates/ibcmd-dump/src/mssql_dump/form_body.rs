//! Root-only bounded diagnostics over the shared form reader. The legacy body
//! parser stays in the root; the reader and its production policies live in
//! `ibcmd-form-body`.
use ibcmd_1c_text::{parse_1c_quoted_string_with_len, split_1c_braced_fields};
pub(crate) use ibcmd_form_body::form_body::*;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
mod field_map_observer;
mod query_identity_observer;
pub use field_map_observer::{
    FormFieldMapObservationError, FormFieldMapObservationReport,
    MAX_FORM_OBSERVER_COMPRESSED_BYTES, observe_form_field_map_blob,
    observe_form_field_map_blob_with_context,
};

// An owned diagnostic snapshot keeps observer controls out of the reader's
// mutable IR API. Production selection parsing remains the shared reader's.
#[derive(Debug, Default, Clone, Eq, PartialEq)]
struct ObserverQuerySelection {
    aliases: BTreeSet<String>,
    stars: Vec<Option<String>>,
    #[cfg(test)]
    extension: Option<BTreeSet<String>>,
    sources: Vec<(Option<String>, Option<String>)>,
    #[cfg(test)]
    paths: BTreeMap<(String, String), String>,
    plain_path_occurrences: Vec<(String, String, String)>,
    unclassified_selection_expression_count: usize,
}
fn observe_query_selection(text: &str, diagnostic: bool) -> Option<ObserverQuerySelection> {
    let selected = if diagnostic {
        ibcmd_form_body::form_body::parse_form_dynamic_list_query_selection_diagnostic(text)?
    } else {
        ibcmd_form_body::form_body::parse_form_dynamic_list_query_selection(text)?
    };
    Some(ObserverQuerySelection {
        aliases: selected.aliases().clone(),
        stars: selected.stars().clone(),
        #[cfg(test)]
        extension: selected.extension().clone(),
        sources: selected.sources().clone(),
        #[cfg(test)]
        paths: selected.paths().clone(),
        plain_path_occurrences: selected.plain_path_occurrences().clone(),
        unclassified_selection_expression_count: selected.unclassified_selection_expression_count(),
    })
}
fn parse_form_dynamic_list_query_selection_diagnostic(
    text: &str,
) -> Option<ObserverQuerySelection> {
    observe_query_selection(text, true)
}
