//! The verdict of the restructuring check and its parts.

use std::collections::BTreeMap;

use serde::Serialize;

use super::rule_id::RuleId;
use super::tree_diff::{ChangeOp, Seg};

/// What kind of change makes the platform's own apply necessary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasonClass {
    /// Tables, columns or indexes change (or would).
    Structure,
    /// Data the platform derives from the configuration and stores changes:
    /// predefined items, route points, the change registration of an
    /// exchange plan.
    Data,
    /// The check cannot tell: an unknown row, a kind it cannot read, a
    /// property nobody has shown to be harmless. Treated as a restructuring.
    Unknown,
}

impl ReasonClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::Structure => "structure",
            Self::Data => "data",
            Self::Unknown => "unknown",
        }
    }
}

/// One change that needs the platform's own apply.
///
/// The text fields (`object`, `property`, `change`) are what a person reads.
/// The typed fields (`rule`, `kind`, `path`, `op`) are what a program acts
/// on: nobody should match words of the text, and the S1 classification
/// (`s1`) does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reason {
    pub class: ReasonClass,
    /// The object, by the name its XML file has (`Catalog._ДемоКассы`,
    /// `Catalog._ДемоКассы.Form.ФормаСписка`, `Configuration`); the row name
    /// when the check cannot place the row.
    pub object: String,
    /// The Config row (its `FileName`) or, in a tree comparison, the file.
    pub file_name: String,
    /// Where in the object: `Properties/CodeLength`,
    /// `ChildObjects/Attribute[Код]`, or the role of a body row
    /// (`Predefined`).
    pub property: String,
    /// What happened there: `9 -> 12`, `added`, `removed`, `row changed`.
    pub change: String,
    /// The rule or the step of the check that made the reason.
    pub rule: RuleId,
    /// The kind of the object (`Catalog`, `Document`, `Form`); empty when
    /// the check could not place it (a row it cannot read).
    pub kind: String,
    /// `property` as steps: the element names from the object down to the
    /// change, and for a child object or a named element its name. Empty for
    /// a reason that is not a change inside a descriptor (a row, an object
    /// that appeared or went).
    pub path: Vec<Seg>,
    /// What happened at `path`: `None` when the reason is not a change of a
    /// descriptor's element.
    pub op: Option<ChangeOp>,
}

impl Default for Reason {
    /// A reason with no class and no rule is `unknown`: the fail-closed
    /// default for whoever builds one field by field.
    fn default() -> Self {
        Self {
            class: ReasonClass::Unknown,
            object: String::new(),
            file_name: String::new(),
            property: String::new(),
            change: String::new(),
            rule: RuleId::Unspecified,
            kind: String::new(),
            path: Vec::new(),
            op: None,
        }
    }
}

impl Reason {
    /// A reason of a step of the check: no kind, no path.
    pub fn step(
        class: ReasonClass,
        rule: RuleId,
        object: &str,
        file_name: &str,
        property: &str,
        change: &str,
    ) -> Self {
        Self {
            class,
            object: object.to_string(),
            file_name: file_name.to_string(),
            property: property.to_string(),
            change: change.to_string(),
            rule,
            ..Self::default()
        }
    }

    /// `Catalog._ДемоКассы: Properties/CodeLength: 9 -> 12`
    pub fn summary(&self) -> String {
        if self.property.is_empty() {
            format!("{}: {}", self.object, self.change)
        } else {
            format!("{}: {}: {}", self.object, self.property, self.change)
        }
    }

    /// The names along `path` (`ChildObjects`, `Attribute`, `Properties`,
    /// `Type`, ...), without the labels.
    pub fn path_names(&self) -> Vec<&str> {
        self.path.iter().map(|seg| seg.name.as_str()).collect()
    }
}

/// A change the check saw and knows to be harmless (applying it copies rows).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Note {
    pub object: String,
    pub file_name: String,
    pub property: String,
    pub change: String,
}

/// What happened to an object between the two sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectOp {
    Added,
    Removed,
    Changed,
}

/// One object whose descriptor differs between the two sides, whether or not
/// the difference matters. This is the index a caller (the import deciding
/// what to stage, a check of the platform's own table entries) walks: the
/// reasons and notes say what changed inside, this says where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectChange {
    /// `Catalog._ДемоКассы`, the row name when the check cannot place it.
    pub object: String,
    /// The object's uuid: what its Config row is named by.
    pub id: String,
    /// The Config row (its `FileName`) or, in a tree comparison, the file.
    pub file_name: String,
    /// `Catalog`, `Form`, ... ; empty when the check cannot tell.
    pub kind: String,
    pub op: ObjectOp,
    /// The most serious class among the object's changes (`unknown` before
    /// `data` before `structure`); `None` when every change is harmless.
    pub class: Option<ReasonClass>,
    /// Property-level changes found (one for an added or removed object).
    pub changes: usize,
}

/// What the check looked at.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Stats {
    /// Files of the configuration now (rows of `versions`, or files of the
    /// new tree).
    pub old_files: usize,
    pub new_files: usize,
    /// Rows the ConfigSave holds (rows mode) or files that differ (trees).
    pub staged_rows: usize,
    pub added_files: usize,
    pub removed_files: usize,
    /// Descriptors whose content differs and that were compared property by
    /// property.
    pub descriptors_compared: usize,
    /// Body rows (modules, forms, templates, help, ...) by role; their
    /// content is not compared, applying them copies rows.
    pub body_rows_by_role: BTreeMap<String, usize>,
    /// Body rows whose content was compared (the roles that carry data).
    pub body_rows_compared: usize,
    /// Rows or files nobody could read: each also gives a reason.
    pub unreadable: usize,
    pub notes_dropped: usize,
    /// Files of a source tree that carry data (predefined items, a
    /// flowchart, the content of an exchange plan, aggregates, additional
    /// indexes) and were not compared with the database: only descriptors
    /// are (see `dbtree`).
    pub body_files_not_compared: usize,
    /// Descriptor rows that differ from the stored ones in bytes only by
    /// the record format the staging platform writes (`upgrade`): proven
    /// harmless, neither a reason nor a note each.
    pub format_upgrades: usize,
}

/// The answer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Verdict {
    /// The staged configuration changes the data structure (or the check
    /// cannot exclude it): only the platform's `config apply` may apply it.
    pub needs_restructuring: bool,
    pub reasons: Vec<Reason>,
    pub notes: Vec<Note>,
    /// Every object whose descriptor differs, harmless or not (no cap).
    pub objects: Vec<ObjectChange>,
    pub stats: Stats,
    /// `rows` (the ConfigSave of a database), `trees` (two XML trees) or
    /// `tree-db` (a source tree against the active Config of a database).
    pub source: String,
    /// Files that carry data were left out of the comparison
    /// (`stats.body_files_not_compared`): `needs_restructuring: false` says
    /// nothing about them. Only a tree against a database is incomplete.
    pub incomplete: bool,
}

/// How many notes a verdict keeps (the rest are counted).
pub const MAX_NOTES: usize = 500;

impl Verdict {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            ..Self::default()
        }
    }

    pub fn push_reason(&mut self, reason: Reason) {
        if !self.reasons.contains(&reason) {
            self.reasons.push(reason);
        }
        self.needs_restructuring = true;
    }

    pub fn push_note(&mut self, note: Note) {
        if self.notes.len() < MAX_NOTES {
            self.notes.push(note);
        } else {
            self.stats.notes_dropped += 1;
        }
    }

    /// Whether "no restructuring" can be trusted: `false` for a verdict
    /// that left files with data out (see `incomplete`). A restructuring is
    /// always conclusive.
    pub fn is_conclusive(&self) -> bool {
        self.needs_restructuring || !self.incomplete
    }

    /// The refusal `apply` prints when it cannot apply: `None` when the
    /// staged configuration needs no restructuring.
    pub fn refusal(&self) -> Option<String> {
        if !self.needs_restructuring {
            return None;
        }
        const SHOWN: usize = 8;
        let mut parts = self
            .reasons
            .iter()
            .take(SHOWN)
            .map(Reason::summary)
            .collect::<Vec<_>>();
        if self.reasons.len() > SHOWN {
            parts.push(format!("и ещё {}", self.reasons.len() - SHOWN));
        }
        Some(format!(
            "требуется штатный config apply: {}",
            parts.join("; ")
        ))
    }

    /// The report `mssql-apply-check` prints.
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        if self.needs_restructuring {
            out.push_str("Реструктуризация нужна: применять штатным config apply.\n");
        } else if self.incomplete {
            out.push_str(
                "По описаниям объектов реструктуризация не нужна; файлы с данными не сравнивались.\n",
            );
        } else {
            out.push_str("Реструктуризация не нужна.\n");
        }
        let stats = &self.stats;
        let (what, staged) = match self.source.as_str() {
            "rows" => ("Файлов в конфигурации", "строк в ConfigSave"),
            "tree-db" => ("Файлов в базе -> в дереве", "различающихся описаний"),
            _ => ("Файлов в деревьях", "различающихся файлов"),
        };
        out.push_str(&format!(
            "{what}: {} -> {}; {staged}: {}; добавлено {}, удалено {}; \
             описаний сравнено {}, тел с данными сравнено {}.\n",
            stats.old_files,
            stats.new_files,
            stats.staged_rows,
            stats.added_files,
            stats.removed_files,
            stats.descriptors_compared,
            stats.body_rows_compared,
        ));
        if !self.objects.is_empty() {
            let flagged = self
                .objects
                .iter()
                .filter(|object| object.class.is_some())
                .count();
            out.push_str(&format!(
                "Объектов с отличиями: {} (из них с причинами: {flagged}).\n",
                self.objects.len()
            ));
        }
        if stats.body_files_not_compared > 0 {
            out.push_str(&format!(
                "Не сравнивались с базой (данные, а не описания): {} файлов; \
                 их проверяют выгрузкой базы и apply-check-trees.\n",
                stats.body_files_not_compared
            ));
        }
        if !stats.body_rows_by_role.is_empty() {
            let roles = stats
                .body_rows_by_role
                .iter()
                .map(|(role, count)| format!("{role} {count}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("Тела без влияния на структуру: {roles}.\n"));
        }
        if !self.reasons.is_empty() {
            out.push_str(&format!("Причины ({}):\n", self.reasons.len()));
            for reason in &self.reasons {
                out.push_str(&format!(
                    "  [{}] {}  ({})\n",
                    reason.class.label(),
                    reason.summary(),
                    reason.file_name
                ));
            }
        }
        if !self.notes.is_empty() {
            out.push_str(&format!(
                "Безопасные изменения ({}{}):\n",
                self.notes.len(),
                if stats.notes_dropped > 0 {
                    format!(", ещё {} не показано", stats.notes_dropped)
                } else {
                    String::new()
                }
            ));
            for note in self.notes.iter().take(60) {
                let what = if note.property.is_empty() {
                    note.change.clone()
                } else {
                    format!("{}: {}", note.property, note.change)
                };
                out.push_str(&format!("  {}: {}\n", note.object, what));
            }
            if self.notes.len() > 60 {
                out.push_str(&format!("  ... и ещё {}\n", self.notes.len() - 60));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reason(object: &str, property: &str, change: &str) -> Reason {
        Reason::step(
            ReasonClass::Structure,
            RuleId::PropertyNotCovered,
            object,
            "f",
            property,
            change,
        )
    }

    #[test]
    fn a_reason_makes_the_verdict_a_refusal_with_the_reasons_in_the_message() {
        let mut verdict = Verdict::new("trees");
        assert!(verdict.refusal().is_none());
        verdict.push_reason(reason("Catalog.A", "Properties/CodeLength", "9 -> 12"));
        verdict.push_reason(reason("Catalog.A", "Properties/CodeLength", "9 -> 12"));
        assert_eq!(verdict.reasons.len(), 1, "the same reason is kept once");
        let message = verdict.refusal().unwrap();
        assert_eq!(
            message,
            "требуется штатный config apply: Catalog.A: Properties/CodeLength: 9 -> 12"
        );
    }

    #[test]
    fn a_long_list_of_reasons_is_cut_in_the_message() {
        let mut verdict = Verdict::new("rows");
        for index in 0..12 {
            verdict.push_reason(reason(&format!("Catalog.C{index}"), "", "added"));
        }
        let message = verdict.refusal().unwrap();
        assert!(message.ends_with("и ещё 4"), "{message}");
        assert_eq!(message.matches("Catalog.C").count(), 8);
    }

    #[test]
    fn notes_beyond_the_limit_are_counted() {
        let mut verdict = Verdict::new("rows");
        for index in 0..(MAX_NOTES + 3) {
            verdict.push_note(Note {
                object: format!("Catalog.C{index}"),
                file_name: String::new(),
                property: String::new(),
                change: "x".to_string(),
            });
        }
        assert_eq!(verdict.notes.len(), MAX_NOTES);
        assert_eq!(verdict.stats.notes_dropped, 3);
        assert!(!verdict.needs_restructuring);
    }
}
