//! Shared registry-text fixture helper.

use crate::mssql_config_apply::si::SiMain;

/// Not used by the writers: the registry text of a subtree, to compare with `object_records`.
pub(crate) fn subtree_text(text: &[u8], main: &SiMain, index: usize) -> String {
    let end = main.subtree_end(index);
    let bytes = &text[main.records[index].start..main.records[end - 1].end];
    String::from_utf8_lossy(bytes).into_owned()
}
