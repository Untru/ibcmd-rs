//! Interface descriptor profile measured on native 8.3.27 and 8.5.1.
//! tool1cd supplied the independent layout hypothesis; native controls own
//! the admitted profile. Switchable's stored flag is inverted.

pub struct InterfaceLayout;

impl InterfaceLayout {
    pub const KIND: &'static str = "Interface";
    pub const FOLDER: &'static str = "Interfaces";
    pub const FAMILY_UUID: &'static str = "39bddf6a-0c3c-452b-921c-d99cfa1c2f1b";
    pub const RECORD_CODE: u32 = 7;
    pub const OUTER_ARITY: usize = 3;
    pub const BODY_ARITY: usize = 4;
    pub const PAYLOAD: usize = 1;
    pub const SWITCHABLE: usize = 1;
    pub const HEADER: usize = 2;
    pub const RESERVED: usize = 3;
    pub const CHILD_COUNT: usize = 2;

    pub const fn identity() -> (&'static str, &'static str) {
        (Self::KIND, Self::FOLDER)
    }

    pub fn switchable_from_code(code: &str) -> Option<bool> {
        match code {
            "0" => Some(true),
            "1" => Some(false),
            _ => None,
        }
    }

    pub const fn switchable_code(value: bool) -> i64 {
        if value { 0 } else { 1 }
    }
}
