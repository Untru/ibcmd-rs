/// Platform standard command group UUIDs and the names the platform exports
/// them under; these are not configuration-specific ids. The command-interface
/// writer reads the same table backwards.
pub const COMMON_COMMAND_GROUPS: [(&str, &str); 11] = [
    (
        "77ea1b8f-dd79-4717-9dba-5628e7f348cf",
        "NavigationPanelOrdinary",
    ),
    (
        "bc80566a-86a5-4e87-acd4-872239385a2e",
        "NavigationPanelSeeAlso",
    ),
    (
        "1af6d528-0b86-4fba-ab95-bd7475db03ba",
        "NavigationPanelImportant",
    ),
    ("4f499c31-050b-47c5-aa84-d0366c0a0da8", "ActionsPanelCreate"),
    (
        "5b360bff-01a1-49b6-93d2-26e7e8e3a038",
        "ActionsPanelReports",
    ),
    ("aabb34e1-98c1-4bd0-bf7f-243f95437b44", "ActionsPanelTools"),
    (
        "dc2ade0f-383e-4c78-85f2-c0dabc0e2dc0",
        "FormCommandBarCreateBasedOn",
    ),
    (
        "cb50f5c0-8013-4262-93a2-f0db379d6b6b",
        "FormCommandBarImportant",
    ),
    (
        "eacad741-96b9-4b3a-bf79-dde9ecead1a1",
        "FormNavigationPanelGoTo",
    ),
    (
        "8ab1540c-0bfa-4fa6-a1e1-5d5069efc7d8",
        "FormNavigationPanelSeeAlso",
    ),
    (
        "dc11a6be-de1f-4b64-a7a5-9b17bf4ec9f2",
        "FormNavigationPanelImportant",
    ),
];
