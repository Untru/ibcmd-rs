//! The active dynamic (online) configuration generation a storage table holds.
//!
//! An online configuration update does not rewrite the rows it changes. It
//! writes the new bodies under an alias -- `<base>_dynupdate_<generation>`,
//! with the storage suffix kept last -- and records the generation history in
//! the table's own `DynamicallyUpdated` row; the plain rows keep the previous
//! content. The infobase, and the native `ibcmd`, read the aliased rows as the
//! configuration, so an export that reads the plain ones publishes the
//! *previous* configuration for every object such an update touched.
//!
//! The history is ordered oldest first and the last entry is the active
//! generation -- the same reading `mssql_main_activation` performs when it
//! plans the next transition. A generation carries only the rows that
//! transition changed, so the configuration is the history applied in order:
//! for a given published name the newest generation that carries it wins, and
//! a name no generation carries keeps its plain row.
//!
//! A table with no `DynamicallyUpdated` row has no overlay and every query is
//! byte-for-byte the query this export built before.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{LazyLock, RwLock};

/// The infix an online update inserts before the storage suffix.
const DYNAMIC_UPDATE_INFIX: &str = "_dynupdate_";

/// How an active dynamic generation renames what a storage table publishes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct StorageGenerationOverlay {
    /// Alias row -> the name it is published under.
    renames: BTreeMap<String, String>,
    /// Plain rows an alias replaces.
    hidden: BTreeSet<String>,
}

impl StorageGenerationOverlay {
    pub(super) fn is_empty(&self) -> bool {
        self.renames.is_empty()
    }

    /// The name `file_name` is published under, when this overlay moves it.
    pub(super) fn published_name<'a>(&'a self, file_name: &'a str) -> Option<&'a str> {
        self.renames.get(file_name).map(String::as_str)
    }

    /// Whether the plain row `file_name` is replaced by an alias.
    pub(super) fn hides(&self, file_name: &str) -> bool {
        self.hidden.contains(file_name)
    }

    #[cfg(test)]
    pub(super) fn renames(&self) -> &BTreeMap<String, String> {
        &self.renames
    }
}

/// Whether a file name carries any generation's alias infix.
pub(super) fn is_dynamic_generation_alias(file_name: &str) -> bool {
    file_name.contains(DYNAMIC_UPDATE_INFIX)
}

/// The generation history a `DynamicallyUpdated` payload records, oldest
/// first, or `None` when the payload is not the shape this reader knows.
///
/// The shape is `{1,<count>,<generation>…}` with `count` entries, the same one
/// `mssql_main_activation::active_generation_from_markers` validates before it
/// plans a transition. A payload this reader cannot read is a refusal, not an
/// empty history: reading it as "no active generation" would publish the
/// previous configuration silently, which is the defect this exists to close.
pub(super) fn dynamic_generation_history(payload: &[u8]) -> Option<Vec<String>> {
    let text = std::str::from_utf8(payload.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(payload))
        .ok()?
        .trim();
    let fields = text
        .strip_prefix('{')?
        .strip_suffix('}')?
        .split(',')
        .map(str::trim)
        .collect::<Vec<_>>();
    if fields.first()? != &"1" {
        return None;
    }
    let count = fields.get(1)?.parse::<usize>().ok()?;
    if count == 0 || fields.len() != count + 2 {
        return None;
    }
    let history = fields[2..]
        .iter()
        .map(|value| {
            uuid::Uuid::parse_str(value)
                .ok()
                .map(|_| (*value).to_owned())
        })
        .collect::<Option<Vec<_>>>()?;
    Some(history)
}

/// The overlay a generation history and a table's own file names imply.
///
/// Generations are applied in history order, so a name several generations
/// carry is published from the newest one; the aliases of the older ones are
/// left out of the table altogether, exactly as a superseded generation's rows
/// already are.
pub(super) fn storage_generation_overlay<'a>(
    history: &[String],
    file_names: impl IntoIterator<Item = &'a str>,
) -> StorageGenerationOverlay {
    let rank_by_generation = history
        .iter()
        .enumerate()
        .map(|(rank, generation)| (format!("{DYNAMIC_UPDATE_INFIX}{generation}"), rank))
        .collect::<Vec<_>>();
    let mut best = BTreeMap::<String, (usize, String)>::new();
    for file_name in file_names {
        for (infix, rank) in &rank_by_generation {
            let Some(position) = file_name.find(infix.as_str()) else {
                continue;
            };
            let mut published = String::with_capacity(file_name.len() - infix.len());
            published.push_str(&file_name[..position]);
            published.push_str(&file_name[position + infix.len()..]);
            match best.get(&published) {
                Some((current, _)) if current >= rank => {}
                _ => {
                    best.insert(published, (*rank, file_name.to_owned()));
                }
            }
            break;
        }
    }
    let mut overlay = StorageGenerationOverlay::default();
    for (published, (_, alias)) in best {
        overlay.hidden.insert(published.clone());
        overlay.renames.insert(alias, published);
    }
    overlay
}

static STORAGE_GENERATION_OVERLAYS: LazyLock<RwLock<BTreeMap<String, StorageGenerationOverlay>>> =
    LazyLock::new(|| RwLock::new(BTreeMap::new()));

/// Makes every query this run builds on `table` read the configuration the
/// overlay describes. An empty overlay installs nothing.
pub(super) fn install_storage_generation_overlay(table: &str, overlay: StorageGenerationOverlay) {
    if overlay.is_empty() {
        return;
    }
    if let Ok(mut overlays) = STORAGE_GENERATION_OVERLAYS.write() {
        overlays.insert(table.to_owned(), overlay);
    }
}

pub(super) fn clear_storage_generation_overlays() {
    if let Ok(mut overlays) = STORAGE_GENERATION_OVERLAYS.write() {
        overlays.clear();
    }
}

pub(super) fn storage_generation_overlay_for(table: &str) -> Option<StorageGenerationOverlay> {
    STORAGE_GENERATION_OVERLAYS
        .read()
        .ok()?
        .get(table)
        .cloned()
}

/// The table expression every query reads from: the table itself when no
/// generation is active, and otherwise a derived table that publishes the
/// active generation's rows under their own names, hides the plain rows they
/// replace and drops every other generation's alias.
pub(super) fn storage_table_expression(
    qualified_table: &str,
    overlay: Option<&StorageGenerationOverlay>,
) -> String {
    let Some(overlay) = overlay.filter(|overlay| !overlay.is_empty()) else {
        return qualified_table.to_owned();
    };
    let quote = |value: &str| format!("N'{}'", value.replace('\'', "''"));
    let cases = overlay
        .renames
        .iter()
        .map(|(alias, published)| format!(" WHEN {} THEN {}", quote(alias), quote(published)))
        .collect::<String>();
    let aliases = overlay
        .renames
        .keys()
        .map(|alias| quote(alias))
        .collect::<Vec<_>>()
        .join(", ");
    let hidden = overlay
        .hidden
        .iter()
        .map(|published| quote(published))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "(SELECT CASE FileName{cases} ELSE FileName END AS FileName,\n\
         \x20              PartNo, DataSize, BinaryData\n\
         \x20       FROM {qualified_table}\n\
         \x20       WHERE FileName IN ({aliases})\n\
         \x20          OR (CHARINDEX(N'{DYNAMIC_UPDATE_INFIX}', FileName) = 0\n\
         \x20              AND FileName NOT IN ({hidden}))) AS storage"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_history_a_marker_records() {
        assert_eq!(
            dynamic_generation_history(
                "{1,1,06cb0442-0c47-4fad-986a-f08f28287c1b}".as_bytes()
            ),
            Some(vec!["06cb0442-0c47-4fad-986a-f08f28287c1b".to_owned()])
        );
        assert_eq!(
            dynamic_generation_history(
                "\u{feff}{1,2,06cb0442-0c47-4fad-986a-f08f28287c1b,17894f1a-0404-4132-9792-15816a396671}"
                    .as_bytes()
            ),
            Some(vec![
                "06cb0442-0c47-4fad-986a-f08f28287c1b".to_owned(),
                "17894f1a-0404-4132-9792-15816a396671".to_owned(),
            ])
        );
        // A count that does not match its payload, a foreign tag and an entry
        // that is not a uuid are refusals, not an empty history.
        assert_eq!(
            dynamic_generation_history("{1,2,06cb0442-0c47-4fad-986a-f08f28287c1b}".as_bytes()),
            None
        );
        assert_eq!(
            dynamic_generation_history("{0,1,06cb0442-0c47-4fad-986a-f08f28287c1b}".as_bytes()),
            None
        );
        assert_eq!(dynamic_generation_history("{1,1,not-a-uuid}".as_bytes()), None);
    }

    #[test]
    fn publishes_the_newest_generation_and_drops_the_older_one() {
        const OLD: &str = "15bcc426-54ca-410a-9543-768987b832ac";
        const NEW: &str = "17894f1a-0404-4132-9792-15816a396671";
        let history = vec![OLD.to_owned(), NEW.to_owned()];
        let names = vec![
            "a627e390-8fad-4a95-afe6-674f54813188".to_owned(),
            format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{OLD}"),
            format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{NEW}"),
            format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{NEW}.0"),
            "versions".to_owned(),
            format!("versions_dynupdate_{OLD}"),
            "untouched".to_owned(),
        ];
        let overlay = storage_generation_overlay(&history, names.iter().map(String::as_str));

        assert_eq!(
            overlay.renames(),
            &BTreeMap::from([
                (
                    format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{NEW}"),
                    "a627e390-8fad-4a95-afe6-674f54813188".to_owned(),
                ),
                (
                    format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{NEW}.0"),
                    "a627e390-8fad-4a95-afe6-674f54813188.0".to_owned(),
                ),
                (format!("versions_dynupdate_{OLD}"), "versions".to_owned()),
            ])
        );
        assert!(overlay.hides("a627e390-8fad-4a95-afe6-674f54813188"));
        assert!(overlay.hides("versions"));
        assert!(!overlay.hides("untouched"));
        assert_eq!(
            overlay.published_name(&format!(
                "a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{OLD}"
            )),
            None,
            "the superseded alias is dropped, not published"
        );
    }

    #[test]
    fn a_table_without_a_generation_keeps_its_own_name() {
        assert_eq!(
            storage_table_expression("[db].dbo.[Config]", None),
            "[db].dbo.[Config]"
        );
        assert_eq!(
            storage_table_expression(
                "[db].dbo.[Config]",
                Some(&StorageGenerationOverlay::default())
            ),
            "[db].dbo.[Config]"
        );
    }

    #[test]
    fn an_active_generation_reads_through_a_derived_table() {
        let overlay = storage_generation_overlay(
            &["06cb0442-0c47-4fad-986a-f08f28287c1b".to_owned()],
            ["versions".into(), "versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b"],
        );
        let expression = storage_table_expression("[db].dbo.[Config]", Some(&overlay));

        assert!(expression.starts_with("(SELECT CASE FileName"));
        assert!(expression.ends_with(") AS storage"));
        assert!(expression.contains(
            "WHEN N'versions_dynupdate_06cb0442-0c47-4fad-986a-f08f28287c1b' THEN N'versions'"
        ));
        assert!(expression.contains("FROM [db].dbo.[Config]"));
        assert!(expression.contains("AND FileName NOT IN (N'versions')"));
    }
}
