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
