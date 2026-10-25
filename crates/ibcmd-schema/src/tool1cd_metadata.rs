//! Metadata scalar coordinates independently described by tool1cd MetaTree and
//! confirmed by native-built Coverage single-property variants on 8.3.27/8.5.
//! Modern history tails are established by Coverage, not inferred from the older grammar.

pub struct CommonAttributeLayout;
impl CommonAttributeLayout {
    pub const OWNER_ARITY: usize = 15;
    pub const AUTO_USE: usize = 6;
    pub const AUTHENTICATION_SEPARATION: usize = 11;
    pub const TYPED: usize = 1;
    pub const PASSWORD_MODE: usize = 2;
    pub const MARK_NEGATIVES: usize = 5;
    pub const MULTI_LINE: usize = 7;
    pub const FILL_CHECKING: usize = 13;
    pub const EXTENDED_EDIT: usize = 17;
    pub const FILL_FROM_FILLING_VALUE: usize = 20;
    pub const TYPED_ARITY: usize = 23;
    pub const TYPED_CODE: &'static str = "27";

    pub fn auto_use_from_code(code: &str) -> Option<&'static str> {
        match code {
            "0" => Some("Use"),
            "1" => Some("DontUse"),
            _ => None,
        }
    }

    pub fn fill_checking_from_code(code: &str) -> Option<&'static str> {
        match code {
            "0" => Some("DontCheck"),
            "1" => Some("ShowError"),
            _ => None,
        }
    }
}

pub struct ConstantHistoryLayout;
impl ConstantHistoryLayout {
    pub const LEGACY_ARITY: usize = 15;
    pub const ARITY: usize = 17;
    pub const UPDATE_IMMEDIATELY: usize = 15;
    pub const AFTER_VERSION: usize = 16;
}

pub struct BusinessProcessScalarLayout;
impl BusinessProcessScalarLayout {
    pub const CHECK_UNIQUE: usize = 19;
    pub const AUTONUMBERING: usize = 20;
    pub const CREATE_ON_INPUT: usize = 21;
    pub const NUMBER_PERIODICITY: usize = 29;
    pub const CREATE_TASK_PRIVILEGED: usize = 31;
}

pub struct TaskScalarLayout;
impl TaskScalarLayout {
    /// Native XML omits these validated UUID carriers (tool1cd mystery properties).
    pub const INTERNAL_UUID_FIELDS: [usize; 2] = [13, 14];
    pub const CREATE_ON_INPUT: usize = 45;
    pub fn create_on_input_profile_is_known(fields: &[&str]) -> bool {
        fields
            .get(Self::CREATE_ON_INPUT)
            .is_some_and(|value| value.trim() == "1")
    }

    pub const CHECK_UNIQUE: usize = 20;
    pub const AUTONUMBERING: usize = 21;
    pub const EDIT_TYPE: usize = 23;
    pub const DATA_HISTORY: usize = 49;
    pub const UPDATE_IMMEDIATELY: usize = 50;
    pub const AFTER_VERSION: usize = 51;
}

pub struct ExchangePlanHistoryLayout;
impl ExchangePlanHistoryLayout {
    pub fn reserved_profile_is_known(fields: &[&str]) -> bool {
        [
            (16, "0"),
            (21, "1"),
            (22, "2"),
            (25, "1"),
            (41, "1"),
            (49, "0"),
        ]
        .iter()
        .all(|&(slot, code)| fields.get(slot).is_some_and(|v| v.trim() == code))
    }

    pub const DATA_HISTORY: usize = 46;
    pub const UPDATE_IMMEDIATELY: usize = 47;
    pub const AFTER_VERSION: usize = 48;
}

pub struct CalculationTypeScalarLayout;
impl CalculationTypeScalarLayout {
    pub const USE_STANDARD_COMMANDS: usize = 35;
    pub const EDIT_TYPE: usize = 24;
    pub const CREATE_ON_INPUT: usize = 55;
    pub const QUICK_CHOICE: usize = 39;
}

pub struct CalculationRegisterScalarLayout;
impl CalculationRegisterScalarLayout {
    pub fn matches_kind(kind: &str) -> bool {
        kind == "CalculationRegister"
    }

    pub const USE_STANDARD_COMMANDS: usize = 24;
    pub const INCLUDE_HELP: usize = 25;
}

/// Closed EditType vocabulary for Task and ChartOfCalculationTypes.
pub fn edit_type_from_code(code: &str) -> Option<&'static str> {
    match code.trim() {
        "0" => Some("InList"),
        "1" => Some("InDialog"),
        "2" => Some("BothWays"),
        _ => None,
    }
}
