//! Measured WebSocketClient storage facts (Windows 8.3.27.2214).
//! The family occupies an existing configuration section; it adds no group.

pub struct WebSocketClientLayout;

impl WebSocketClientLayout {
    pub const KIND: &'static str = "WebSocketClient";
    pub const FOLDER: &'static str = "WebSocketClients";
    pub const FAMILY_UUID: &'static str = "a7641777-7813-45c6-96ef-9d51587a6ac6";
    pub const OUTER_ARITY: usize = 3;
    pub const PAYLOAD_ARITY: usize = 11;
    pub const HEADER: usize = 1;
    pub const PREDEFINED: usize = 2;
    pub const SERVER_URL: usize = 3;
    pub const USER: usize = 4;
    pub const PASSWORD: usize = 5;
    pub const HEADERS: usize = 6;
    pub const OS_PROXY: usize = 7;
    pub const OS_AUTHENTICATION: usize = 8;
    pub const TIMEOUT: usize = 9;
    pub const AUTO_CONNECT: usize = 10;
    pub const MODULE_SUFFIX: &'static str = ".0";
    pub const MODULE_SOURCE: &'static str = "Ext/Module.bsl";

    pub const fn identity() -> (&'static str, &'static str) {
        (Self::KIND, Self::FOLDER)
    }

    /// A discriminator, not validation: malformed flags/counts must still be
    /// diagnosed as this family rather than an IntegrationService or Form.
    /// IntegrationService carries UUIDs in these positions, not three strings.
    pub fn recognizes_fields(fields: &[&str]) -> bool {
        fields.len() > Self::PASSWORD
            && fields.first().is_some_and(|field| field.trim() == "0")
            && [Self::SERVER_URL, Self::USER, Self::PASSWORD]
                .iter()
                .all(|index| fields[*index].trim().starts_with('"'))
    }

    pub fn headers_arity(count: usize) -> Option<usize> {
        count.checked_mul(2)?.checked_add(1)
    }
}
