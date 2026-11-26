//! The stored layout of managed-form bodies a platform writes ([`FormLayout`]):
//! the 8.3 one, or the one 8.5.1 introduced, which the 8.5 down-conversion
//! (`layout_8_5_1`) and the 2.21 form loader read.
//!
//! It moved from the root's platform registry (`src/platform/mod.rs`), which
//! re-exports it at `platform::FormLayout` and keeps reading the profile
//! constant that names it.

use std::fmt;

use serde::Serialize;

/// The stored layout of managed-form bodies, and of the colours and fonts in
/// the other rows of a configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub enum FormLayout {
    /// The layout 8.3 platforms store: form root revision 50, colours
    /// `{3,...}`, fonts `{7,...}`.
    #[serde(rename = "8.3")]
    V8_3,
    /// The layout 8.5.1 introduced: form root revision 59, members appended
    /// to the 8.3 records, colours `{4,...}`, fonts `{8,...}`.
    #[serde(rename = "8.5.1")]
    V8_5_1,
}

impl FormLayout {
    /// The version that introduced the layout, as profiles declare it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V8_3 => "8.3",
            Self::V8_5_1 => "8.5.1",
        }
    }

    /// The layout a configuration's rows take on a platform whose own layout
    /// is `self`: a configuration kept in a compatibility mode older than 8.5
    /// (packed, `80327`) keeps the 8.3 layout -- every form and style of the
    /// ERP УХ 8.5 clone, compatibility 8.3.27, is stored the 8.3 way -- while
    /// compatibility 8.5 or later takes the platform's own.
    pub fn stored(self, compatibility: u32) -> Self {
        if compatibility >= 80500 {
            self
        } else {
            Self::V8_3
        }
    }
}

impl fmt::Display for FormLayout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::FormLayout;

    #[test]
    fn a_layout_is_named_by_the_version_that_introduced_it() {
        for (layout, name) in [(FormLayout::V8_3, "8.3"), (FormLayout::V8_5_1, "8.5.1")] {
            assert_eq!(layout.as_str(), name);
            assert_eq!(layout.to_string(), name);
            assert_eq!(
                serde_json::to_string(&layout).unwrap(),
                format!("\"{name}\"")
            );
        }
        assert!(FormLayout::V8_3 < FormLayout::V8_5_1);
    }
}
