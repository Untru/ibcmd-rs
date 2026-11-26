//! CURRENT logical DataPath values. Extra paths are ordered semantic data;
//! unresolved spelling belongs to the source-only form layout instead.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPathSpec {
    pub segments: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_paths: Vec<String>,
}

impl DataPathSpec {
    /// The original FormXmlHelper uses Java String.split("\\.") with limit 0.
    /// Empty input has one empty segment; trailing empty segments are dropped.
    pub fn from_form_text(value: &str) -> Self {
        let mut segments: Vec<String> = value.split('.').map(str::to_owned).collect();
        if !value.is_empty() {
            while segments.last().is_some_and(String::is_empty) {
                segments.pop();
            }
        }
        for segment in &mut segments {
            if let Some(current) = segment.strip_prefix('~') {
                *segment = current.to_owned();
            }
        }
        Self {
            segments,
            extra_paths: Vec::new(),
        }
    }

    pub fn primary(&self) -> String {
        self.segments.join(".")
    }
}

impl From<String> for DataPathSpec {
    fn from(value: String) -> Self {
        Self {
            segments: value.split('.').map(str::to_owned).collect(),
            extra_paths: Vec::new(),
        }
    }
}
impl From<&str> for DataPathSpec {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl PartialEq<&str> for DataPathSpec {
    fn eq(&self, value: &&str) -> bool {
        self.extra_paths.is_empty() && self.primary() == *value
    }
}
