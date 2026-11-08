//! Designer XML input defaults measured by the extension dialog corpus on
//! 8.3.27.2214 and 8.5.1.1150. EDT omission rules are a different dialect.

/// An omitted Designer Button/Type becomes UsualButton, including buttons
/// inside a command bar. EDT's omitted type is CommandBarButton instead.
pub const DESIGNER_BUTTON_TYPE: &str = "UsualButton";

/// The 2.21 form projection derives ShowCommandBar from CommandBarLocation.
/// The 2.20 projection does not emit ShowCommandBar.
pub fn derived_show_command_bar(edition: &str, location: &str) -> Option<&'static str> {
    if edition != "2.21" {
        return None;
    }
    match location {
        "None" => Some("false"),
        "Top" | "Bottom" => Some("true"),
        _ => None,
    }
}

/// The two qualifier leaves of a binary form attribute's type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormBinaryDataQualifier {
    Length,
    AllowedLength,
}

impl FormBinaryDataQualifier {
    pub const CONTAINER_XML_NAME: &'static str = "BinaryDataQualifiers";

    pub fn from_xml_local_name(name: &str) -> Option<Self> {
        match name {
            "Length" => Some(Self::Length),
            "AllowedLength" => Some(Self::AllowedLength),
            _ => None,
        }
    }

    pub const fn xml_local_name(self) -> &'static str {
        match self {
            Self::Length => "Length",
            Self::AllowedLength => "AllowedLength",
        }
    }
}

/// A form attribute's binary type. The native corpus-form-settings forms of
/// 8.3.27.2214 and 8.5.1.1150 declare the unbounded variable type (`{"R"}`).
/// The qualified pattern uses the same length/allowed-length coding read by
/// the shared type-pattern reader: a positive length and 1 variable / 0 fixed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormBinaryDataType {
    pub length: u32,
    pub allowed_length_flag: u8,
}

impl FormBinaryDataType {
    /// Returns `None` for another type; malformed or lossy binary qualifiers
    /// refuse instead of silently becoming the unbounded type.
    pub fn from_xml(
        type_name: &str,
        length: Option<&str>,
        allowed_length: Option<&str>,
    ) -> Result<Option<Self>, String> {
        if type_name.trim() != "xs:base64Binary" {
            return Ok(None);
        }
        let length = length
            .map(|value| {
                value.trim().parse::<u32>().map_err(|_| {
                    format!("invalid Form Attribute BinaryDataQualifiers/Length: {value}")
                })
            })
            .transpose()?
            .unwrap_or(0);
        let allowed_length_flag = match allowed_length.map(str::trim).unwrap_or("Variable") {
            "Variable" => 1,
            "Fixed" => 0,
            other => {
                return Err(format!(
                    "unsupported Form Attribute BinaryDataQualifiers/AllowedLength: {other}"
                ));
            }
        };
        if length == 0 && allowed_length_flag == 0 {
            return Err(
                "Form Attribute fixed binary length zero has no lossless type pattern".to_string(),
            );
        }
        Ok(Some(Self {
            length,
            allowed_length_flag,
        }))
    }
}
