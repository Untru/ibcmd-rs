//! StyleItem declaration and independently typed value (tool1cd MetaTree 59992ff).
//! Native Coverage preserves a colour value when the declared type becomes Font.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StyleItemKind {
    Color,
    Font,
    Border,
}
impl StyleItemKind {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Color" => Some(Self::Color),
            "Font" => Some(Self::Font),
            "Border" => Some(Self::Border),
            _ => None,
        }
    }
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "0" => Some(Self::Color),
            "1" => Some(Self::Font),
            "2" => Some(Self::Border),
            _ => None,
        }
    }
    pub const fn code(self) -> i64 {
        match self {
            Self::Color => 0,
            Self::Font => 1,
            Self::Border => 2,
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Color => "Color",
            Self::Font => "Font",
            Self::Border => "Border",
        }
    }
}
pub struct StyleItemLayout;
impl StyleItemLayout {
    pub const DECLARED_KIND: usize = 1;
    pub const VALUE: usize = 2;
    pub const HEADER: usize = 3;
    pub const TYPED_VALUE_TYPE: usize = 1;
    pub const COLOR_TYPE: &'static str = "9cd510c7-abfc-11d4-9434-004095e12fc7";
    pub const FONT_TYPE: &'static str = "9cd510c8-abfc-11d4-9434-004095e12fc7";
    pub const BORDER_TYPE: &'static str = "4d10ca00-111a-4d43-9c96-92cd773716de";
}
