//! The compatibility-mode rules of the form writer: what a form spells
//! by the configuration's `CompatibilityMode` (from `src/mssql_dump/refs.rs`;
//! the readers of the mode off the configuration's rows stay there).

/// The first compatibility mode that writes the AnyIBRef type set
/// (`280f5f0e-…`) as `cfg:AnyIBRef`; below it 8.3.27.2214 writes `cfg:AnyRef`.
/// Probed on an extension's form attribute: 8.3.10, 8.3.21, 8.3.22 AnyRef;
/// 8.3.23, 8.3.24, 8.3.27 AnyIBRef (fixtures `form_events`,
/// `form_events_8_3_22`); ИТК (8.3.10) and 1Cv8 (8.3.21) write AnyRef
/// throughout, and an external object in a configuration of 8.3.21 too.
const ANY_IB_REF_FROM: (u32, u32, u32) = (8, 3, 23);

/// Whether a configuration of `compatibility_mode` writes the AnyIBRef type
/// set as `cfg:AnyRef`. With nothing read, the platform's own edition: no.
pub fn compatibility_mode_spells_any_ref(compatibility_mode: Option<&str>) -> bool {
    compatibility_mode
        .and_then(compatibility_version)
        .is_some_and(|version| version < ANY_IB_REF_FROM)
}

/// The first compatibility mode whose forms write a `UsualGroup`'s
/// `<Behavior>Usual</Behavior>`. 8.3.27.2214 dumps one form holding a group
/// loaded with an explicit `Usual` (stored as code `0`, not the default `3`)
/// without the element under `CompatibilityMode` 8.3.17, 8.3.18 and 8.3.19,
/// and with it under 8.3.20 and 8.3.21; `Collapsible` and `PopUp` are written
/// under every one of them (fixture `group_behavior`). Real configurations
/// agree: three under 8.3.14/8.3.17 never write `Usual`, two under 8.3.21 do.
const USUAL_GROUP_BEHAVIOR_FROM: (u32, u32, u32) = (8, 3, 20);

/// Whether forms written under `compatibility_mode` spell a group's explicit
/// `Usual` behavior. With nothing read, the platform's own edition: yes.
pub fn forms_write_usual_group_behavior(compatibility_mode: Option<&str>) -> bool {
    compatibility_mode
        .and_then(compatibility_version)
        .is_none_or(|version| version >= USUAL_GROUP_BEHAVIOR_FROM)
}

/// The first compatibility mode under which a dynamic list that declares no
/// main table has no `DefaultPicture` field of its own, so the platform marks
/// the path `~List.DefaultPicture`. 8.3.27.2214 loads a one-form configuration
/// whose table names `Список.DefaultPicture` of such a list under
/// `CompatibilityMode` 8.3.17 and 8.3.18 and dumps it unmarked, refuses the
/// same path under 8.3.19, 8.3.20 and 8.3.21 ("wrong data path"), and dumps
/// `~Список.DefaultPicture` there (fixture `dynamic_list_default_picture`).
/// Real configurations agree: 23 such paths under 8.3.17 and one under 8.3.14
/// are unmarked, 37 under 8.3.21 marked.
const NO_MAIN_TABLE_DEFAULT_PICTURE_MARKED_FROM: (u32, u32, u32) = (8, 3, 19);

/// Whether a no-main-table dynamic list's `DefaultPicture` path carries `~`
/// under `compatibility_mode`. With nothing read, the platform's own edition:
/// yes.
pub fn forms_mark_no_main_table_default_picture(compatibility_mode: Option<&str>) -> bool {
    compatibility_mode
        .and_then(compatibility_version)
        .is_none_or(|version| version >= NO_MAIN_TABLE_DEFAULT_PICTURE_MARKED_FROM)
}

/// What a form's spelling depends on in the configuration's compatibility
/// mode, read once per export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormCompatibility {
    pub usual_group_behavior: bool,
    pub no_main_table_default_picture_marked: bool,
}

impl Default for FormCompatibility {
    /// The platform's own edition.
    fn default() -> Self {
        Self::of(None)
    }
}

impl FormCompatibility {
    pub fn of(compatibility_mode: Option<&str>) -> Self {
        Self {
            usual_group_behavior: forms_write_usual_group_behavior(compatibility_mode),
            no_main_table_default_picture_marked: forms_mark_no_main_table_default_picture(
                compatibility_mode,
            ),
        }
    }
}

/// `Version8_3_21` -> (8, 3, 21).
fn compatibility_version(mode: &str) -> Option<(u32, u32, u32)> {
    let mut parts = mode
        .strip_prefix("Version")?
        .split('_')
        .map(str::parse::<u32>);
    let (Some(Ok(major)), Some(Ok(minor))) = (parts.next(), parts.next()) else {
        return None;
    };
    let patch = match parts.next() {
        None => 0,
        Some(Ok(patch)) => patch,
        Some(Err(_)) => return None,
    };
    Some((major, minor, patch))
}
