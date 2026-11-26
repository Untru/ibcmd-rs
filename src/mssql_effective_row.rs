//! Which stored row the platform reads for a published name when an online
//! generation is active (#416, F-18).
//!
//! An online update writes the rows it changes under
//! `<base>_dynupdate_<generation><suffix>` and leaves the plain rows as they
//! were; the history in the table's `DynamicallyUpdated` row lists the
//! generations, oldest first, and for a published name the newest generation
//! that carries it wins. A stage that patches a base row -- and above all the
//! `versions` row it builds the next generation's from -- has to start from the
//! row the platform reads, not from the plain one, or everything the earlier
//! generations changed goes back to what it was before them.
//!
//! The rule itself is the export's (`mssql_dump::dynamic_generation`, reached
//! through `mssql_dump::stored_row_name`); the stage that reads the whole table
//! applies it through `mssql_dump::dynamic_generation_aliases`. This module adds
//! what the row-by-row stage needs and the other readers do not: the `LIKE`
//! pattern that finds the candidates with one seek.

/// The infix an online update inserts before the storage suffix.
const DYNAMIC_UPDATE_INFIX: &str = "_dynupdate_";

/// The length of a generation: a hyphenated UUID.
const GENERATION_LEN: usize = 36;

/// The name `published` is stored under in `generation`: the storage suffix
/// (`.0`, `.1`, ...) stays last.
#[cfg(test)]
fn alias_name(published: &str, generation: &str) -> String {
    match published.split_once('.') {
        Some((base, suffix)) => format!("{base}{DYNAMIC_UPDATE_INFIX}{generation}.{suffix}"),
        None => format!("{published}{DYNAMIC_UPDATE_INFIX}{generation}"),
    }
}

/// A `LIKE ... ESCAPE N'\'` pattern that matches the alias of `published` in
/// every generation (and nothing else): the stem is a constant prefix, so the
/// clustered key seeks.
pub fn alias_pattern(published: &str) -> String {
    let escape = |text: &str| {
        let mut escaped = String::with_capacity(text.len());
        for character in text.chars() {
            if matches!(character, '\\' | '%' | '_' | '[') {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        escaped
    };
    let (stem, suffix) = match published.split_once('.') {
        Some((base, suffix)) => (base, format!(".{suffix}")),
        None => (published, String::new()),
    };
    format!(
        "{}\\_dynupdate\\_{}{}",
        escape(stem),
        "_".repeat(GENERATION_LEN),
        escape(&suffix)
    )
}

/// Of the stored rows that carry `published` -- the plain one and the aliases
/// of any generation -- the one the platform reads: the alias of the newest
/// generation in `history` (oldest first), else the plain row. An alias of a
/// generation the history does not list is left out.
pub fn pick<T>(
    published: &str,
    history: &[String],
    rows: impl IntoIterator<Item = (String, T)>,
) -> Option<(String, T)> {
    let rows = rows.into_iter().collect::<Vec<_>>();
    let stored = crate::mssql_dump::stored_row_name(
        history,
        published,
        rows.iter().map(|(name, _)| name.as_str()),
    );
    rows.into_iter().find(|(name, _)| *name == stored)
}

#[cfg(test)]
mod tests {
    use super::*;

    const G1: &str = "06cb0442-0c47-4fad-986a-f08f28287c1b";
    const G2: &str = "17894f1a-0404-4132-9792-15816a396671";
    const G3: &str = "4b094372-d4b4-4387-8acf-03f18697d7a4";

    fn rows(names: &[String]) -> Vec<(String, usize)> {
        names
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, n)| (n, i))
            .collect()
    }

    #[test]
    fn the_storage_suffix_stays_last() {
        assert_eq!(
            alias_name("versions", G1),
            format!("versions_dynupdate_{G1}")
        );
        assert_eq!(
            alias_name("a627e390-8fad-4a95-afe6-674f54813188.0", G1),
            format!("a627e390-8fad-4a95-afe6-674f54813188_dynupdate_{G1}.0")
        );
    }

    #[test]
    fn the_pattern_matches_the_aliases_of_the_name_only() {
        assert_eq!(
            alias_pattern("versions"),
            format!("versions\\_dynupdate\\_{}", "_".repeat(36))
        );
        assert_eq!(
            alias_pattern("ab_c%d.0"),
            format!("ab\\_c\\%d\\_dynupdate\\_{}.0", "_".repeat(36))
        );
    }

    #[test]
    fn the_newest_generation_that_carries_the_name_wins() {
        let history = vec![G1.to_owned(), G2.to_owned(), G3.to_owned()];
        let names = vec![
            "versions".to_owned(),
            alias_name("versions", G1),
            alias_name("versions", G3),
            // A generation the history does not list is not read.
            alias_name("versions", "99999999-9999-4999-8999-999999999999"),
        ];
        let picked = pick("versions", &history, rows(&names)).unwrap();
        assert_eq!(picked.0, alias_name("versions", G3));
        assert_eq!(picked.1, 2);
        // Only an older generation carries it: that one.
        let names = vec!["versions".to_owned(), alias_name("versions", G1)];
        assert_eq!(
            pick("versions", &history, rows(&names)).unwrap().0,
            alias_name("versions", G1)
        );
        // No alias: the plain row. No row at all: nothing.
        let names = vec!["versions".to_owned()];
        assert_eq!(
            pick("versions", &history, rows(&names)).unwrap().0,
            "versions"
        );
        assert!(pick("versions", &history, rows(&[])).is_none());
        // An alias the history does not list, and no plain row: nothing.
        let names = vec![alias_name(
            "versions",
            "99999999-9999-4999-8999-999999999999",
        )];
        assert!(pick("versions", &history, rows(&names)).is_none());
    }

    #[test]
    fn a_row_of_another_name_is_never_picked() {
        let history = vec![G1.to_owned()];
        let names = vec![
            "versions.0".to_owned(),
            alias_name("versions.0", G1),
            alias_name("other", G1),
        ];
        assert!(pick("versions", &history, rows(&names)).is_none());
    }

    #[test]
    fn the_row_by_row_pick_and_the_whole_table_aliases_are_one_rule() {
        let history = vec![G1.to_owned(), G2.to_owned(), G3.to_owned()];
        let marker = format!("\u{feff}{{1,3,{G1},{G2},{G3}}}");
        let unlisted = "99999999-9999-4999-8999-999999999999";
        let stored = vec![
            "a".to_owned(),
            alias_name("a", G1),
            alias_name("a", G3),
            "a.0".to_owned(),
            alias_name("a.0", G2),
            "b".to_owned(),
            alias_name("b", unlisted),
            "versions".to_owned(),
            alias_name("versions", G2),
            // An object an update added: no plain row.
            alias_name("added", G2),
            alias_name("added.0", G3),
        ];
        let aliases = crate::mssql_dump::dynamic_generation_aliases(
            Some(marker.as_bytes()),
            stored.iter().map(String::as_str),
        )
        .unwrap();
        for published in ["a", "a.0", "b", "versions", "added", "added.0", "missing"] {
            let picked = pick(published, &history, rows(&stored)).map(|(name, _)| name);
            let expected = aliases
                .get(published)
                .cloned()
                .or_else(|| stored.iter().find(|name| *name == published).cloned());
            assert_eq!(picked, expected, "{published}");
        }
    }
}
