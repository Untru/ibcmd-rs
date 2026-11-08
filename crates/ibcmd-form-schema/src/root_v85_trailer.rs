/// The root's three internal generated command bars are presence records,
/// not XML ChildItems. Their native empty payload is measured by the public
/// stored-panels recipe and retained by native load/save/load.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FormRootV85TrailerMember {
    Absent,
    Present,
    Revision,
    GeneratedPanel(usize),
    Other,
}

impl FormRootV85TrailerMember {
    pub fn scalar(raw: &str) -> Self {
        match raw.trim() {
            "0" => Self::Absent,
            "1" => Self::Present,
            _ => Self::Other,
        }
    }

    pub fn record(fields: &[&str]) -> Self {
        if fields.len() >= 2 && fields.first().map(|value| value.trim()) == Some("59") {
            return Self::Revision;
        }
        // An empty native revision-22 auto command bar has no child pairs.
        // All scalar/primitive defaults are checked; a styled or configured
        // payload is outside this profile, even if its first field is 22.
        const EMPTY: [&str; 29] = [
            "22",
            "",
            "1",
            "0",
            "0",
            "9",
            "",
            "{1,0}",
            "{1,0}",
            "0",
            "1",
            "0",
            "0",
            "0",
            "2",
            "2",
            "{4,4,{0},4}",
            "{8,3,0,1,100}",
            "{0,0,0}",
            "1",
            "{1,0,1,0}",
            "0",
            "1",
            "0",
            "0",
            "0",
            "3",
            "3",
            "0",
        ];
        if fields.len() != EMPTY.len() {
            return Self::Other;
        }
        for (index, expected) in EMPTY.iter().enumerate() {
            if expected.is_empty() {
                continue;
            }
            let compact = fields[index]
                .chars()
                .filter(|ch| !ch.is_ascii_whitespace())
                .collect::<String>();
            if compact != *expected {
                return Self::Other;
            }
        }
        let name = fields[6].trim();
        if name.len() < 2 || !name.starts_with('"') || !name.ends_with('"') {
            return Self::Other;
        }
        let mut chars = name[1..name.len() - 1].chars();
        while let Some(ch) = chars.next() {
            if ch == '"' && chars.next() != Some('"') {
                return Self::Other;
            }
        }
        let Some(identity) = fields[1]
            .trim()
            .strip_prefix('{')
            .and_then(|value| value.strip_suffix('}'))
        else {
            return Self::Other;
        };
        let Some((id, class)) = identity.split_once(',') else {
            return Self::Other;
        };
        if uuid::Uuid::parse_str(class.trim()).ok()
            != uuid::Uuid::parse_str("02023637-7868-4a5f-8576-835a76e0c9ba").ok()
        {
            return Self::Other;
        }
        match id.trim() {
            "-2" => Self::GeneratedPanel(0),
            "-3" => Self::GeneratedPanel(1),
            "-4" => Self::GeneratedPanel(2),
            _ => Self::Other,
        }
    }
}

/// A unique physical boundary and the twelve logical appended members.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct FormRootV85TrailerSchema {
    revision_index: usize,
    appended_start: usize,
    logical: [usize; Self::LOGICAL_MEMBERS],
    panels: Vec<usize>,
}

impl FormRootV85TrailerSchema {
    pub const REVISION: &'static str = "59";
    pub const LOGICAL_MEMBERS: usize = 12;
    const RETAINED_PREFIX: usize = 2;
    const PANEL_SLOTS: [usize; 3] = [2, 3, 9];

    pub fn from_raw_layout(
        revision: Option<&str>,
        member_count: usize,
        mut classify: impl FnMut(usize) -> FormRootV85TrailerMember,
    ) -> Option<Self> {
        if revision.map(str::trim) != Some(Self::REVISION) {
            return None;
        }
        let mut found = None;
        for panel_count in 0..=Self::PANEL_SLOTS.len() {
            let Some(revision_index) = member_count
                .checked_sub(Self::LOGICAL_MEMBERS + Self::RETAINED_PREFIX + panel_count)
            else {
                continue;
            };
            if classify(revision_index) != FormRootV85TrailerMember::Revision {
                continue;
            }
            let appended_start = revision_index + Self::RETAINED_PREFIX;
            let mut cursor = appended_start;
            let mut logical = [0; Self::LOGICAL_MEMBERS];
            let mut panels = Vec::new();
            let mut valid = true;
            for (slot, physical) in logical.iter_mut().enumerate() {
                if cursor >= member_count {
                    valid = false;
                    break;
                }
                *physical = cursor - appended_start;
                let member = classify(cursor);
                cursor += 1;
                if let Some(role) = Self::PANEL_SLOTS.iter().position(|known| *known == slot) {
                    match member {
                        FormRootV85TrailerMember::Absent => {}
                        FormRootV85TrailerMember::Present
                            if cursor < member_count
                                && classify(cursor)
                                    == FormRootV85TrailerMember::GeneratedPanel(role) =>
                        {
                            panels.push(cursor - appended_start);
                            cursor += 1;
                        }
                        _ => {
                            valid = false;
                            break;
                        }
                    }
                }
            }
            if !valid || cursor != member_count || panels.len() != panel_count {
                continue;
            }
            let profile = Self {
                revision_index,
                appended_start,
                logical,
                panels,
            };
            if found.replace(profile).is_some() {
                return None;
            }
        }
        found
    }

    pub const fn revision_index(&self) -> usize {
        self.revision_index
    }
    pub const fn appended_start(&self) -> usize {
        self.appended_start
    }
    pub const fn logical_members(&self) -> &[usize; Self::LOGICAL_MEMBERS] {
        &self.logical
    }
    pub fn generated_panels(&self) -> &[usize] {
        &self.panels
    }
}
