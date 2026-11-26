//! Native extension schema facts measured on 8.3.27.2214 and 8.5.1.1150.
//! The synthetic one/two-job Coverage transitions are tracked by issue #582.

/// The scheduled-job service record and the table definition it names.
pub struct ExtensionScheduledJobsLayout;

impl ExtensionScheduledJobsLayout {
    pub const SERVICE_ID: &str = "b5156fba-6391-4a93-8888-7c063a551319";
    pub const PAYLOAD_TYPE: &str = "dc25cb71-037a-4e2a-a4fd-12362b81a772";
    pub const TABLE_KIND: &str = "ScheduledJobs";
    pub const FIELDS: &[(&str, &str, u64, u64)] = &[
        ("ID", "B", 16, 0),
        ("Description", "S", 0x8000_0080, 0),
        ("JobKey", "S", 0x8000_0080, 0),
        ("MetadataID", "B", 16, 0),
        ("Predefined", "L", 0, 0),
        ("Parameters", "B", 0x8000_0000, 0),
        ("UserName", "S", 0x8000_00ff, 0),
        ("ActivationCondition", "B", 0x8000_0000, 0),
        ("Use", "L", 0, 0),
        ("RestartCount", "N", 9, 0),
        ("RestartPeriod", "N", 9, 0),
        ("RestartAttemptNumber", "N", 9, 0),
        ("State", "N", 9, 0),
        ("StartTime", "T", 0, 0),
        ("FinishTime", "T", 0, 0),
        ("Version", "N", 9, 0),
    ];
}

/// Context columns shared by the extension registry and its service records.
pub struct ExtensionServiceScopeLayout;
impl ExtensionServiceScopeLayout {
    pub const SERVICE_TABLE: &str = "ExtensionsRestruct";
    pub const REGISTRY_TABLE: &str = "ExtensionsInfo";
    pub const USE_PREFIX: &str = "DataSeparationUse";
    pub const VALUE_PREFIX: &str = "Fld";
    pub fn is_context_field(name: &str) -> bool {
        [Self::USE_PREFIX, Self::VALUE_PREFIX].iter().any(|prefix| {
            name.strip_prefix(prefix).is_some_and(|suffix| {
                !suffix.is_empty() && suffix.bytes().all(|b| b.is_ascii_digit())
            })
        })
    }
}
