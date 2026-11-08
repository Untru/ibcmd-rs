//! Canonical empty DCS picture values of the current XML 2.21 writer.
//!
//! Native 8.5.1.1529 writes a self-closing readable Picture at the value's
//! depth, while its stored primitive and the 2.20 spelling remain v8ui:Picture.

pub const VALUE_TAG: &str = "dcscor:value";
pub const PICTURE_TYPE: &str = "xsi:type=\"v8ui:Picture\"";
pub const READABLE_NAMESPACE: &str = "http://v8.1c.ru/8.3/xcf/readable";

/// The native readable empty value. Numeric depth keeps the generated prefix
/// independent of object names and cannot introduce an arbitrary XML name.
pub fn empty_readable_value(depth: usize) -> String {
    format!(
        "<{VALUE_TAG} xmlns:d{depth}p1=\"{READABLE_NAMESPACE}\" xsi:type=\"d{depth}p1:Picture\"/>\r\n"
    )
}

/// The unreferenced, empty primitive stored by both measured platforms.
pub fn empty_stored_value() -> String {
    format!("<{VALUE_TAG} {PICTURE_TYPE}/>\r\n")
}
