//! Does a staged descriptor row differ from the stored one only by the
//! record format the staging platform writes?
//!
//! A native `config import` rewrites every descriptor it stages in the record
//! format of its own edition, whether the object changed or not: the version
//! number at the head of a record grows by one, a counted list gains the
//! default entries the new version introduced (and its counter grows by one),
//! a field the old version did not have appears with its default. The
//! metadata model reads a row by the fields it knows, so such a row decodes to
//! the same XML as the stored one although the bytes differ; a check that
//! trusted the decoding alone would call the row unchanged, and one that
//! trusted the bytes alone would call all 500 of them changes.
//!
//! This module settles it on the rows themselves. The two rows are read as
//! brace trees and aligned item by item; the rows are the same but for the
//! format when every difference is one of
//!
//! * the head of a list (its record version) is one greater;
//! * an integer inside a list, after its head, is one greater while an entry
//!   was inserted in that same list (a counter of the list);
//! * an entry the older version did not have was inserted, and it is one of
//!   the values [`FILLERS`] lists: nothing but the defaults the platform's
//!   importer was seen to write;
//! * an empty string holds the default of the new version ([`FILLS`]);
//! * in the row of the configuration, the extension compatibility mode
//!   names the edition of the platform that staged the row instead of the
//!   compatibility mode.
//!
//! Anything else is not proven, whichever way it decodes, and the caller
//! refuses the row with the first place that no rule explains
//! ([`Deviation`]). The tables are what one native import of the corpus
//! showed (`docs/apply/restructuring-check.md`, section 8.9): a row of
//! another shape is refused, never guessed.

use std::sync::OnceLock;

use crate::metadata_model::brace::{Brace, NIL_UUID, parse_row, serialize};

/// The most entries one list may gain.
const MAX_INSERTS: usize = 64;
/// The largest alignment table (items x insertions) one list may need.
const MAX_TABLE: usize = 4_000_000;

/// The class of records and atoms the importer inserts for a new record
/// version. A value not on this list is not an upgrade, whatever the row
/// means: a new attribute, a new predefined item, a changed flag.
static FILLERS: OnceLock<Vec<Brace>> = OnceLock::new();

/// (old field, new field): an empty string that got the default of the new
/// version. (The brace tree keeps no empty fields: `a,,b` is `a,b`.)
static FILLS: OnceLock<Vec<(Brace, Brace)>> = OnceLock::new();

const TYPED_VALUE_CLASS: &str = "502b7765-f89c-4fd0-924f-0a28d3dc09b7";
const PROPERTY_CLASS: &str = "3b10624f-1e3d-495d-8093-25225efc5313";

fn typed_default(value: &str) -> Brace {
    // `{"#",<class>,{<class>,<value>}}`
    Brace::list(vec![
        Brace::str("#"),
        Brace::atom(TYPED_VALUE_CLASS),
        Brace::list(vec![Brace::atom(TYPED_VALUE_CLASS), Brace::atom(value)]),
    ])
}

fn fillers() -> &'static [Brace] {
    FILLERS.get_or_init(|| {
        vec![
            Brace::atom("0"),
            Brace::atom("1"),
            Brace::atom("5"),
            Brace::atom(PROPERTY_CLASS),
            Brace::list(vec![Brace::atom("1"), Brace::atom(NIL_UUID)]),
            typed_default("0"),
            typed_default("2"),
            // Entries of the configuration row's own counted lists.
            Brace::list(vec![Brace::atom("41"), Brace::atom("0")]),
            Brace::list(vec![
                Brace::atom("a7641777-7813-45c6-96ef-9d51587a6ac6"),
                Brace::atom("0"),
            ]),
            Brace::list(vec![Brace::atom("-1036481104")]),
        ]
    })
}

fn fills() -> &'static [(Brace, Brace)] {
    FILLS.get_or_init(|| vec![(Brace::str(""), Brace::str("5"))])
}

/// What a proven upgrade consisted of.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Upgrade {
    /// Lists whose record version grew by one.
    pub head_bumps: usize,
    /// Counters that grew by one along with an inserted entry.
    pub counter_bumps: usize,
    /// Entries the importer inserted.
    pub inserted: usize,
    /// Empty fields that got a default.
    pub filled: usize,
    /// Extension compatibility modes moved to the staging edition.
    pub compatibility_moves: usize,
    /// The stored row is the one in the newer format (a native apply
    /// promoted it) and the staged row is the older: the same table read the
    /// other way round.
    pub reversed: bool,
}

impl Upgrade {
    fn plus(self, other: Upgrade) -> Upgrade {
        Upgrade {
            head_bumps: self.head_bumps + other.head_bumps,
            counter_bumps: self.counter_bumps + other.counter_bumps,
            inserted: self.inserted + other.inserted,
            filled: self.filled + other.filled,
            compatibility_moves: self.compatibility_moves + other.compatibility_moves,
            reversed: self.reversed || other.reversed,
        }
    }
}

/// The first place the rows differ in a way no rule of the upgrade explains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Deviation {
    /// The item numbers from the root of the row down (`5/2/3`); empty for
    /// the row as a whole.
    pub path: String,
    pub old: String,
    pub new: String,
}

impl Deviation {
    /// `at 5/2/3: 24 -> 26`
    pub fn describe(&self) -> String {
        if self.path.is_empty() {
            format!("{} -> {}", self.old, self.new)
        } else {
            format!("at {}: {} -> {}", self.path, self.old, self.new)
        }
    }
}

/// What a caller that has both rows can say about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowProof {
    /// The staged row is the stored one in the record format of a later
    /// platform.
    Proven(Upgrade),
    /// It is not: the first place no rule explains.
    Refuted(Deviation),
    /// The caller has no raw rows to compare (a test double, a tree).
    Unavailable,
}

impl RowProof {
    /// [`prove`] as a `RowProof`.
    pub fn of(kind: &str, old: &[u8], new: &[u8]) -> RowProof {
        match prove(kind, old, new) {
            Ok(upgrade) => RowProof::Proven(upgrade),
            Err(deviation) => RowProof::Refuted(deviation),
        }
    }
}

/// What the alignment may accept besides equality.
#[derive(Clone, Copy)]
struct Rules {
    /// The row of the configuration.
    configuration: bool,
}

/// Proves that the rows are one in two record formats: `new` is `old` in the
/// format of a later platform (a native import staged it), or the other way
/// round (a native apply promoted a stage into `Config`, and what is staged
/// now is written for the compatibility mode). See the module. `kind` is the
/// object's kind (`Configuration` has one more rule). The rows are the
/// inflated bytes as stored.
pub fn prove(kind: &str, old: &[u8], new: &[u8]) -> Result<Upgrade, Deviation> {
    let read = |bytes: &[u8], side: &str| {
        parse_row(bytes).map_err(|error| Deviation {
            path: String::new(),
            old: String::new(),
            new: format!("the {side} row is not brace text: {error:#}"),
        })
    };
    let (old, new) = (read(old, "stored")?, read(new, "staged")?);
    prove_trees(kind, &old, &new)
}

pub(crate) fn prove_trees(kind: &str, old: &Brace, new: &Brace) -> Result<Upgrade, Deviation> {
    match prove_forward(kind, old, new) {
        Ok(upgrade) => Ok(upgrade),
        Err(first) => match prove_forward(kind, new, old) {
            Ok(upgrade) => Ok(Upgrade {
                reversed: true,
                ..upgrade
            }),
            // The message is about the direction a native import goes.
            Err(_) => Err(first),
        },
    }
}

/// `new` is `old` upgraded.
fn prove_forward(kind: &str, old: &Brace, new: &Brace) -> Result<Upgrade, Deviation> {
    let rules = Rules {
        configuration: kind == "Configuration",
    };
    match equivalent(old, new, 0, false, rules) {
        Some(tally) if tally.counter_bumps <= tally.inserted => Ok(tally),
        Some(_) => Err(Deviation {
            path: String::new(),
            old: "a counter grew".to_string(),
            new: "with no entry inserted in the row".to_string(),
        }),
        None => Err(explain(old, new, &mut Vec::new(), rules)),
    }
}

/// Whether `new` is `old` upgraded; `index` is the item's place in its list
/// (0 is the record version) and `counted` says that entries were inserted
/// in that same list, which is what lets a counter in it grow.
fn equivalent(
    old: &Brace,
    new: &Brace,
    index: usize,
    counted: bool,
    rules: Rules,
) -> Option<Upgrade> {
    if old == new {
        return Some(Upgrade::default());
    }
    match (old, new) {
        (Brace::List(old_items), Brace::List(new_items)) => align(old_items, new_items, rules),
        (Brace::Atom(before), Brace::Atom(after)) => {
            if let (Ok(before), Ok(after)) = (before.parse::<i64>(), after.parse::<i64>())
                && after == before + 1
                && (index == 0 || counted)
            {
                return Some(if index == 0 {
                    Upgrade {
                        head_bumps: 1,
                        ..Upgrade::default()
                    }
                } else {
                    Upgrade {
                        counter_bumps: 1,
                        ..Upgrade::default()
                    }
                });
            }
            if rules.configuration
                && let (Ok(before), Ok(after)) = (before.parse::<u32>(), after.parse::<u32>())
                && (80300..80400).contains(&before)
                && (80300..80400).contains(&after)
                && after > before
            {
                return Some(Upgrade {
                    compatibility_moves: 1,
                    ..Upgrade::default()
                });
            }
            filled(old, new)
        }
        _ => filled(old, new),
    }
}

fn filled(old: &Brace, new: &Brace) -> Option<Upgrade> {
    fills()
        .iter()
        .any(|(before, after)| before == old && after == new)
        .then_some(Upgrade {
            filled: 1,
            ..Upgrade::default()
        })
}

/// Two lists that may differ by inserted entries: every entry of `old`
/// pairs, in order, with an entry of `new` that is equivalent to it, and the
/// entries of `new` left over are fillers.
fn align(old: &[Brace], new: &[Brace], rules: Rules) -> Option<Upgrade> {
    let (n, m) = (old.len(), new.len());
    if m < n {
        return None;
    }
    let extra = m - n;
    if extra > MAX_INSERTS || (n + 1).saturating_mul(extra + 1) > MAX_TABLE {
        return None;
    }
    // reach[i][k]: the first i entries of `old` paired with the first i + k
    // of `new`, k of them insertions.
    let mut reach: Vec<Vec<Option<Upgrade>>> = vec![vec![None; extra + 1]; n + 1];
    reach[0][0] = Some(Upgrade::default());
    for i in 0..=n {
        for k in 0..=extra {
            let Some(so_far) = reach[i][k] else { continue };
            let j = i + k;
            if j >= m {
                continue;
            }
            if i < n
                && reach[i + 1][k].is_none()
                && let Some(step) = equivalent(&old[i], &new[j], i, extra > 0, rules)
            {
                reach[i + 1][k] = Some(so_far.plus(step));
            }
            if k < extra && reach[i][k + 1].is_none() && fillers().contains(&new[j]) {
                reach[i][k + 1] = Some(so_far.plus(Upgrade {
                    inserted: 1,
                    ..Upgrade::default()
                }));
            }
        }
    }
    reach[n][extra]
}

/// Where the rows part, for a message: the innermost list whose items the
/// upgrade does not explain, with what is different in it.
fn explain(old: &Brace, new: &Brace, path: &mut Vec<usize>, rules: Rules) -> Deviation {
    let here = |path: &[usize], old: String, new: String| Deviation {
        path: path
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join("/"),
        old,
        new,
    };
    let (Brace::List(old_items), Brace::List(new_items)) = (old, new) else {
        return here(path, show(old), show(new));
    };
    let prefix = old_items
        .iter()
        .zip(new_items)
        .take_while(|(a, b)| a == b)
        .count();
    let limit = old_items.len().min(new_items.len()) - prefix;
    let suffix = old_items
        .iter()
        .rev()
        .zip(new_items.iter().rev())
        .take(limit)
        .take_while(|(a, b)| a == b)
        .count();
    let old_middle = &old_items[prefix..old_items.len() - suffix];
    let new_middle = &new_items[prefix..new_items.len() - suffix];
    if old_middle.len() == new_middle.len() {
        // The same number of entries on both sides: the first pair that the
        // upgrade does not explain is the place.
        for (offset, (a, b)) in old_middle.iter().zip(new_middle).enumerate() {
            let at = prefix + offset;
            if equivalent(a, b, at, new_items.len() > old_items.len(), rules).is_some() {
                continue;
            }
            path.push(at);
            let found = if matches!((a, b), (Brace::List(_), Brace::List(_))) {
                explain(a, b, path, rules)
            } else {
                here(path, show(a), show(b))
            };
            path.pop();
            return found;
        }
    }
    let join = |items: &[Brace]| {
        if items.is_empty() {
            "nothing".to_string()
        } else {
            items.iter().map(show).collect::<Vec<_>>().join(",")
        }
    };
    let mut found = here(path, join(old_middle), join(new_middle));
    found.path = if found.path.is_empty() {
        format!("items {}..", prefix)
    } else {
        format!("{} items {}..", found.path, prefix)
    };
    found
}

/// One entry on one line, cut to a readable length.
fn show(item: &Brace) -> String {
    const LIMIT: usize = 90;
    let flat = serialize(item)
        .replace(['\r', '\n'], "")
        .trim_start_matches('\u{feff}')
        .to_string();
    if flat.chars().count() > LIMIT {
        format!("{}...", flat.chars().take(LIMIT).collect::<String>())
    } else {
        flat
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(text: &str) -> Brace {
        parse_row(text.as_bytes()).unwrap()
    }

    fn proof(old: &str, new: &str) -> Result<Upgrade, Deviation> {
        prove_trees("Catalog", &tree(old), &tree(new))
    }

    #[test]
    fn equal_rows_are_the_same_with_nothing_to_prove() {
        assert_eq!(
            proof("{56,\"a\",{1,2}}", "{56,\"a\",{1,2}}").unwrap(),
            Upgrade::default()
        );
    }

    #[test]
    fn a_record_version_one_greater_is_an_upgrade() {
        let upgrade = proof("{56,\"a\",{7,x}}", "{57,\"a\",{8,x}}").unwrap();
        assert_eq!(upgrade.head_bumps, 2);
        assert_eq!((upgrade.head_bumps, upgrade.counter_bumps), (2, 0));
    }

    #[test]
    fn a_version_two_greater_or_a_changed_head_is_not() {
        let refused = proof("{56,\"a\"}", "{58,\"a\"}").unwrap_err();
        assert_eq!(refused.old, "56");
        assert_eq!(refused.new, "58");
        // One lower is the older format, read the other way round; two lower is not.
        assert!(proof("{56,\"a\"}", "{55,\"a\"}").unwrap().reversed);
        assert!(proof("{56,\"a\"}", "{54,\"a\"}").is_err());
    }

    #[test]
    fn default_entries_may_be_inserted_and_others_may_not() {
        let upgrade = proof(
            "{56,\"a\",{3,x}}",
            "{57,\"a\",0,{3,x},{1,00000000-0000-0000-0000-000000000000}}",
        )
        .unwrap();
        assert_eq!(upgrade.inserted, 2);
        // A new attribute is not a default.
        let refused = proof("{56,\"a\",{3,x}}", "{57,\"a\",{3,x},{9,\"Name\"}}").unwrap_err();
        assert!(refused.new.contains("Name"), "{}", refused.describe());
        // Nor is a value that merely looks like one.
        assert!(proof("{56,\"a\"}", "{57,\"a\",7}").is_err());
        assert!(proof("{56,\"a\"}", "{57,\"a\",\"0\"}").is_err());
    }

    #[test]
    fn a_counter_grows_only_with_an_inserted_entry() {
        let upgrade = proof(
            "{56,{9,2,a,b}}",
            "{57,{10,3,a,b,3b10624f-1e3d-495d-8093-25225efc5313}}",
        )
        .unwrap();
        assert_eq!(upgrade.head_bumps, 2);
        assert_eq!(upgrade.counter_bumps, 1);
        assert_eq!(upgrade.inserted, 1);
        // The same +1 with nothing inserted is a changed number.
        let refused = proof("{56,{9,2,a,b}}", "{57,{10,3,a,b}}").unwrap_err();
        assert_eq!(refused.describe(), "at 1/1: 2 -> 3");
        // Nor does an entry inserted in another list of the row explain it.
        let elsewhere = proof(
            "{56,{9,2,a},{9,7}}",
            "{57,{10,3,a},{10,7,3b10624f-1e3d-495d-8093-25225efc5313}}",
        )
        .unwrap_err();
        assert_eq!(elsewhere.describe(), "at 1/1: 2 -> 3");
        // Two counters for one inserted entry are one too many.
        let twice = proof(
            "{56,{9,2,3,a}}",
            "{57,{10,3,4,a,3b10624f-1e3d-495d-8093-25225efc5313}}",
        )
        .unwrap_err();
        assert_eq!(twice.old, "a counter grew");
    }

    #[test]
    fn an_empty_field_may_get_the_default_of_the_new_version() {
        let upgrade = proof("{1,\"\",{2},\"\"}", "{2,\"\",{2},\"5\"}").unwrap();
        assert_eq!(upgrade.filled, 1);
        assert!(proof("{1,\"\",{2},\"\"}", "{2,\"\",{2},\"6\"}").is_err());
        // A filled field that was not empty is a change.
        assert!(proof("{1,\"x\"}", "{2,\"5\"}").is_err());
    }

    #[test]
    fn removed_entries_are_never_an_upgrade() {
        assert!(proof("{56,a,b}", "{57,a}").is_err());
        assert!(proof("{57,a,b}", "{56,a}").is_err());
    }

    #[test]
    fn a_staged_row_in_the_older_format_is_proven_the_other_way_round() {
        // The stored row was promoted by a native apply (57); what is staged
        // now is written for the compatibility mode (56).
        let upgrade = proof("{57,\"a\",0,{3,x}}", "{56,\"a\",{3,x}}").unwrap();
        assert!(upgrade.reversed);
        assert_eq!(upgrade.inserted, 1);
        // Not a licence for anything: an entry the older row has and the
        // newer lacks, or a value that is not a default, is still refused.
        assert!(proof("{57,\"a\",{3,x}}", "{56,\"a\",{3,y}}").is_err());
        assert!(proof("{57,\"a\",7,{3,x}}", "{56,\"a\",{3,x}}").is_err());
        assert!(!proof("{56,\"a\"}", "{57,\"a\",0}").unwrap().reversed);
    }

    #[test]
    fn the_extension_compatibility_moves_only_in_the_configuration_row() {
        let old = tree("{67,80324,80324}");
        let new = tree("{68,80324,80327}");
        let upgrade = prove_trees("Configuration", &old, &new).unwrap();
        assert_eq!(upgrade.compatibility_moves, 1);
        assert!(prove_trees("Catalog", &old, &new).is_err());
        // Read the other way round (the stored row is the newer): the same.
        assert!(prove_trees("Configuration", &new, &old).unwrap().reversed);
        assert!(prove_trees("Catalog", &new, &old).is_err());
    }

    #[test]
    fn a_deviation_names_the_innermost_list_and_the_entries() {
        let refused = proof("{56,{1,{2,a}},b}", "{57,{1,{2,a},{9,\"X\"}},b}").unwrap_err();
        assert_eq!(refused.path, "1 items 2..");
        assert_eq!(refused.old, "nothing");
        assert!(refused.new.contains("\"X\""), "{}", refused.describe());
        let single = proof("{56,{1,7}}", "{57,{1,9}}").unwrap_err();
        assert_eq!(single.path, "1/1");
        assert_eq!(single.describe(), "at 1/1: 7 -> 9");
    }

    #[test]
    fn rows_that_are_not_brace_text_are_not_proven() {
        let refused = prove("Catalog", b"<Catalog/>", b"{1}").unwrap_err();
        assert!(
            refused.new.contains("not brace text"),
            "{}",
            refused.describe()
        );
    }

    #[test]
    fn an_alignment_pairs_entries_in_order_around_inserted_ones() {
        // The inserted default sits between two equal entries.
        let upgrade = proof("{5,0,0,1}", "{6,0,0,0,1}").unwrap();
        assert_eq!(upgrade.inserted, 1);
        // Too many insertions are refused rather than searched.
        let mut long = String::from("{5");
        for _ in 0..(MAX_INSERTS + 2) {
            long.push_str(",0");
        }
        long.push('}');
        assert!(proof("{5}", &long).is_err());
    }
}
