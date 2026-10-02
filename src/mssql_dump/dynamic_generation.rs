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
//! A row carries a generation when the 36 characters after the first
//! `_dynupdate_` of its name are an entry of the history; any other name with
//! that infix belongs to a generation the history does not list and is left
//! out.
//!
//! An online update that removes an object does not remove its rows: they stay
//! in the table, under the plain name or under the alias of the generation that
//! wrote them, while the `versions` row of the newer generation stops listing
//! the object and the platform stops reading it. The published configuration is
//! what the active `versions` row lists, so a published name it does not list
//! is dropped ([`StorageGenerationOverlay::dropping`]).
//!
//! The platform's export publishes the *main* configuration, which is not
//! always the one `Config` holds: a completed import stages it in `ConfigSave`
//! -- a `versions` row that lists all of it and the rows of the objects that
//! changed -- and it stays there until an apply moves it into `Config`. The
//! export reads a name from the staged rows when they hold it and from `Config`
//! (as the generations above publish it) when they do not
//! ([`StorageGenerationOverlay::staging`]).
//!
//! A table with no `DynamicallyUpdated` row and no staged rows has no overlay
//! and every query is byte-for-byte the query this export built before.
//!
//! # How the queries read it
//!
//! The overlay itself (alias -> published name, and the plain rows the aliases
//! replace) is kept for the Rust side: the offline row folder and the row
//! headers use it. The SQL side must not: a database that was updated online
//! many times holds a hundred thousand aliases, and a statement that lists
//! them (a `CASE` branch and two `IN` names each) exhausts the optimizer's
//! stack (error 8621). [`storage_table_expression`] therefore builds a derived
//! table whose text depends only on the *history* -- a few dozen entries --
//! and lets the server find the aliases by their name:
//!
//! 1. every row gets the name it is published under (`STUFF` removes
//!    `_dynupdate_<generation>`) and a rank, the position of its generation in
//!    the history (0 for a plain row); a row whose generation is not listed is
//!    dropped;
//! 2. per published name the row with the highest rank is kept -- as the
//!    maximum of `rank || stored name` in a `GROUP BY`, a single pass over
//!    narrow columns;
//! 3. the parts of that stored row are read back by its name, on the clustered
//!    key;
//! 4. the names the overlay drops -- a short list, one entry per removed object
//!    -- and the names the staged rows hold leave after the aggregate, once per
//!    published name; the staged rows are appended (`UNION ALL`).
//!
//! The published name leaves the aggregate as `MAX(...)`, so a filter on it
//! (`IN` lists, ranges, `LIKE`) runs once per name on a materialised column
//! and cannot be pushed below the aggregate; pushed into the scan it would
//! recompute the name for every row and every list entry.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};

/// The infix an online update inserts before the storage suffix.
const DYNAMIC_UPDATE_INFIX: &str = "_dynupdate_";

/// The length of a generation: a hyphenated UUID.
const GENERATION_LEN: usize = 36;

/// The most row constructors one `VALUES` list may hold.
const VALUES_ROWS_MAX: usize = 1000;

/// The rows a completed import staged in `ConfigSave`, which the platform's
/// export publishes instead of the rows of `Config` that carry the same names.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct StagedRows {
    /// The qualified `ConfigSave` table.
    table: String,
    /// The names it holds a row for.
    names: BTreeSet<String>,
}

/// How an active dynamic generation renames what a storage table publishes.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct StorageGenerationOverlay {
    /// The generations of the table's history, oldest first.
    history: Vec<String>,
    /// Whether some alias of a listed generation exists, so the rows have to be
    /// picked generation by generation.
    aliased: bool,
    /// Alias row -> the name it is published under.
    renames: BTreeMap<String, String>,
    /// Plain rows that are not published: those an alias replaces, those a
    /// staged row replaces and those of a dropped name.
    hidden: BTreeSet<String>,
    /// Published names left out although a row carries them: what the active
    /// `versions` row does not list ([`Self::dropping`]).
    dropped: BTreeSet<String>,
    /// The staged main configuration, when the export publishes it
    /// ([`Self::staging`]).
    staged: Option<StagedRows>,
}

impl StorageGenerationOverlay {
    pub(super) fn is_empty(&self) -> bool {
        !self.aliased && self.dropped.is_empty() && self.staged.is_none()
    }

    /// The names of the staged rows this overlay publishes.
    pub(super) fn staged_names(&self) -> Option<&BTreeSet<String>> {
        self.staged.as_ref().map(|staged| &staged.names)
    }

    /// The overlay that publishes the rows staged in `table` (`ConfigSave`)
    /// instead of the rows of the table it reads that carry the same names.
    ///
    /// A completed import stages the main configuration there: `versions` lists
    /// all of it and the table holds the rows of the objects that changed. The
    /// platform's export publishes that configuration, so a name the staged
    /// rows hold is read from them and every other name from the table.
    pub(super) fn staging(mut self, table: String, names: BTreeSet<String>) -> Self {
        self.renames
            .retain(|_, published| !names.contains(published));
        self.hidden.extend(names.iter().cloned());
        self.staged = Some(StagedRows { table, names });
        self
    }

    /// The name `file_name` is published under, when this overlay moves it.
    pub(super) fn published_name<'a>(&'a self, file_name: &'a str) -> Option<&'a str> {
        self.renames.get(file_name).map(String::as_str)
    }

    /// Whether the plain row `file_name` is not published: an alias replaces
    /// it, or its name is dropped.
    pub(super) fn hides(&self, file_name: &str) -> bool {
        self.hidden.contains(file_name)
    }

    /// The names this overlay publishes out of the stored ones `file_names`.
    pub(super) fn published_names<'a>(
        &'a self,
        file_names: impl IntoIterator<Item = &'a str> + 'a,
    ) -> impl Iterator<Item = &'a str> + 'a {
        file_names.into_iter().filter_map(|file_name| {
            self.published_name(file_name).or_else(|| {
                (!is_dynamic_generation_alias(file_name) && !self.hides(file_name))
                    .then_some(file_name)
            })
        })
    }

    /// The stored row the published `name` is read from: its newest alias, or
    /// the plain row when no alias carries it.
    pub(super) fn stored_name<'a>(&'a self, name: &'a str) -> &'a str {
        self.renames
            .iter()
            .find(|(_, published)| published.as_str() == name)
            .map_or(name, |(alias, _)| alias.as_str())
    }

    /// The overlay without the published names `dropped`: no row is published
    /// under them, be it an alias or a plain row.
    ///
    /// An online update that removes an object does not remove its rows; the
    /// `versions` row of the generation stops listing it and the platform stops
    /// reading it. The names such a row does not list are what this drops.
    pub(super) fn dropping(mut self, dropped: BTreeSet<String>) -> Self {
        self.renames
            .retain(|_, published| !dropped.contains(published));
        self.hidden.extend(dropped.iter().cloned());
        self.dropped.extend(dropped);
        self
    }

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

/// The generation a stored name carries, as its rank in the history, and the
/// position of its `_dynupdate_<generation>` part.
///
/// Only the first `_dynupdate_` of a name counts, the same reading the SQL
/// side makes ([`storage_table_expression`]).
fn alias_generation(
    file_name: &str,
    rank_by_generation: &HashMap<&str, usize>,
) -> Option<(usize, usize)> {
    let position = file_name.find(DYNAMIC_UPDATE_INFIX)?;
    let start = position + DYNAMIC_UPDATE_INFIX.len();
    let generation = file_name.get(start..start + GENERATION_LEN)?;
    rank_by_generation
        .get(generation)
        .map(|rank| (*rank, position))
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
    // A generation listed twice keeps its first rank.
    let mut rank_by_generation = HashMap::<&str, usize>::with_capacity(history.len());
    for (rank, generation) in history.iter().enumerate() {
        rank_by_generation
            .entry(generation.as_str())
            .or_insert(rank);
    }
    let mut best = BTreeMap::<String, (usize, String)>::new();
    for file_name in file_names {
        let Some((rank, position)) = alias_generation(file_name, &rank_by_generation) else {
            continue;
        };
        let end = position + DYNAMIC_UPDATE_INFIX.len() + GENERATION_LEN;
        let mut published = String::with_capacity(file_name.len() - (end - position));
        published.push_str(&file_name[..position]);
        published.push_str(&file_name[end..]);
        match best.get(&published) {
            Some((current, _)) if *current >= rank => {}
            _ => {
                best.insert(published, (rank, file_name.to_owned()));
            }
        }
    }
    let mut overlay = StorageGenerationOverlay {
        history: history.to_vec(),
        aliased: !best.is_empty(),
        ..StorageGenerationOverlay::default()
    };
    for (published, (_, alias)) in best {
        overlay.hidden.insert(published.clone());
        overlay.renames.insert(alias, published);
    }
    overlay
}

/// The table of a database an overlay describes: (database, table).
type StorageKey = (String, String);

/// The most names a bounded read may name for its selection to reach the scan
/// under the table expression: a longer list is read the way an unbounded read
/// is.
const SELECTION_MAX: usize = 64;

/// What a query on the table expression is going to keep, so that the scan under
/// the aggregate can be limited to the stored rows that can publish it.
#[derive(Debug, Clone, Copy)]
pub(super) enum Selection<'a> {
    /// Every row: the scan reads the whole table.
    All,
    /// Rows with these published names (`FileName IN (...)`).
    Names(&'a BTreeSet<String>),
    /// The rows of these owners: the owner's own row and its `<owner>.<n>` rows
    /// (`FileName = owner OR FileName LIKE 'owner.%'`).
    Owners(&'a BTreeSet<String>),
}

/// `LIKE` with the wildcards of `text` escaped (`ESCAPE N'\'`).
fn like_escaped(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if matches!(character, '\\' | '%' | '_' | '[') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

/// The condition on the stored rows (`alias.FileName`) that keeps every row that
/// can publish a selected name, or `None` when the selection is not limited.
///
/// A published name is stored under itself and, in a generation, under
/// `<stem>_dynupdate_<generation><suffix>` (the storage suffix stays last); the
/// stem is what precedes the first dot. Each stem is one prefix range of the
/// clustered key, each name one point of it: the scan seeks, instead of reading
/// the table to throw most of it away after the aggregate (#409 F-15: 34.5 s
/// under load against 1 ms for the plain read). The condition is only a
/// narrowing: the query still filters on the published name.
fn selection_condition(alias: &str, selection: Selection<'_>) -> Option<String> {
    let names = match selection {
        Selection::All => return None,
        Selection::Names(names) | Selection::Owners(names) => names,
    };
    if names.is_empty() || names.len() > SELECTION_MAX {
        return None;
    }
    let mut terms = Vec::new();
    let mut stems = BTreeSet::new();
    match selection {
        Selection::Names(_) => {
            let list = names.iter().map(|name| quote(name)).collect::<Vec<_>>();
            terms.push(format!("{alias}.FileName IN ({})", list.join(", ")));
        }
        Selection::Owners(_) => {
            for name in names {
                terms.push(format!("{alias}.FileName = {}", quote(name)));
                terms.push(format!(
                    "{alias}.FileName LIKE {} ESCAPE N'\\'",
                    quote(&format!("{}.%", like_escaped(name)))
                ));
            }
        }
        Selection::All => unreachable!("returned above"),
    }
    for name in names {
        stems.insert(name.split('.').next().unwrap_or(name.as_str()));
    }
    for stem in stems {
        terms.push(format!(
            "{alias}.FileName LIKE {} ESCAPE N'\\'",
            quote(&format!("{}\\_dynupdate\\_%", like_escaped(stem)))
        ));
    }
    Some(terms.join(" OR "))
}

/// The overlays are shared, not copied: a query builder asks for the table's
/// overlay once per statement and an overlay may name a hundred thousand rows.
///
/// An overlay is the view of one table of one database, so it is kept under
/// both names, and whoever installs one does it inside a [`StorageViewScope`],
/// which takes it away again. A step of a run that must read the rows as they
/// are stored -- the activation compares them with the table inside its
/// transaction -- and a run that handles another database never read it by
/// accident (#409 F-2: the overlay was once process-global and outlived the
/// export that installed it).
static STORAGE_GENERATION_OVERLAYS: LazyLock<
    RwLock<BTreeMap<StorageKey, Arc<StorageGenerationOverlay>>>,
> = LazyLock::new(|| RwLock::new(BTreeMap::new()));

fn storage_key(database: &str, table: &str) -> StorageKey {
    (database.to_owned(), table.to_owned())
}

/// Makes every query this run builds on `table` of `database` read the
/// configuration the overlay describes, until the enclosing
/// [`StorageViewScope`] ends. An empty overlay installs nothing.
pub(super) fn install_storage_generation_overlay(
    database: &str,
    table: &str,
    overlay: Arc<StorageGenerationOverlay>,
) {
    if overlay.is_empty() {
        return;
    }
    STORAGE_GENERATION_OVERLAYS
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(storage_key(database, table), overlay);
}

/// The overlay installed for `table` of `database`, if there is one.
pub(super) fn storage_generation_overlay_for(
    database: &str,
    table: &str,
) -> Option<Arc<StorageGenerationOverlay>> {
    STORAGE_GENERATION_OVERLAYS
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&storage_key(database, table))
        .cloned()
}

/// What the reads of one database see while the scope lives: the rows as they
/// are stored, plus the overlays installed since it began.
///
/// Beginning a scope suspends the overlays already installed for the database
/// -- an enclosing scope's -- and ending it takes away what was installed
/// inside and gives the suspended ones back. An export begins one for its
/// database, so the overlay it resolves is gone when it returns; the reads of
/// the activation begin one to be sure they see the stored rows.
#[must_use = "the scope ends when it is dropped"]
pub(super) struct StorageViewScope {
    database: String,
    suspended: Vec<(String, Arc<StorageGenerationOverlay>)>,
}

impl StorageViewScope {
    pub(super) fn begin(database: &str) -> Self {
        let mut overlays = STORAGE_GENERATION_OVERLAYS
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        let tables = overlays
            .keys()
            .filter(|(installed, _)| installed == database)
            .map(|(_, table)| table.clone())
            .collect::<Vec<_>>();
        let suspended = tables
            .into_iter()
            .filter_map(|table| {
                overlays
                    .remove(&storage_key(database, &table))
                    .map(|overlay| (table, overlay))
            })
            .collect();
        Self {
            database: database.to_owned(),
            suspended,
        }
    }
}

impl Drop for StorageViewScope {
    fn drop(&mut self) {
        let mut overlays = STORAGE_GENERATION_OVERLAYS
            .write()
            .unwrap_or_else(PoisonError::into_inner);
        overlays.retain(|(installed, _), _| installed != &self.database);
        for (table, overlay) in std::mem::take(&mut self.suspended) {
            overlays.insert(storage_key(&self.database, &table), overlay);
        }
    }
}

fn quote(value: &str) -> String {
    format!("N'{}'", value.replace('\'', "''"))
}

/// `SELECT Gen, Rk` over the history: one row per generation with its rank
/// (its 1-based place in the history; a generation listed twice keeps the first
/// one), in `VALUES` lists of at most [`VALUES_ROWS_MAX`] rows.
fn generation_ranks(history: &[String]) -> String {
    let mut seen = BTreeSet::new();
    let rows = history
        .iter()
        .enumerate()
        .filter(|(_, generation)| seen.insert(generation.as_str()))
        .map(|(index, generation)| format!("({}, {})", quote(generation), index + 1))
        .collect::<Vec<_>>();
    rows.chunks(VALUES_ROWS_MAX)
        .map(|chunk| {
            format!(
                "SELECT v.Gen, v.Rk FROM (VALUES {}) v(Gen, Rk)",
                chunk.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
}

/// The names an overlay leaves out although a row carries them, one column
/// `Name`, in `VALUES` lists of at most [`VALUES_ROWS_MAX`] rows.
fn dropped_names(dropped: &BTreeSet<String>) -> String {
    let rows = dropped
        .iter()
        .map(|name| format!("({})", quote(name)))
        .collect::<Vec<_>>();
    rows.chunks(VALUES_ROWS_MAX)
        .map(|chunk| format!("SELECT v.Name FROM (VALUES {}) v(Name)", chunk.join(", ")))
        .collect::<Vec<_>>()
        .join(" UNION ALL ")
}

/// The columns of the storage table every expression publishes.
const COLUMNS: [&str; 7] = [
    "FileName",
    "PartNo",
    "DataSize",
    "BinaryData",
    "Attributes",
    "Creation",
    "Modified",
];

/// The columns of one table alias, in the order of [`COLUMNS`].
fn columns_of(alias: &str) -> String {
    COLUMNS
        .iter()
        .map(|column| format!("{alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The table expression every query reads from: the table itself when nothing
/// is overlaid, and otherwise a derived table that publishes, for every name,
/// the row of the newest generation that carries it (or the plain row when none
/// does) under that name, drops every other row, and takes the staged rows in
/// place of the rows of the same names.
///
/// The text depends on the generation history, on the names the overlay drops
/// -- the objects an update removed -- and on the staged table; it does not
/// depend on the number of rows or aliases. See the module documentation for
/// the reading it implements. It has the columns of the storage table:
/// `FileName`, `PartNo`, `DataSize`, `BinaryData`, `Attributes`, `Creation`,
/// `Modified`.
pub(super) fn storage_table_expression(
    qualified_table: &str,
    overlay: Option<&StorageGenerationOverlay>,
) -> String {
    storage_table_expression_for(qualified_table, overlay, Selection::All)
}

/// [`storage_table_expression`] for a query that keeps only `selection`: the same
/// rows, and the scan under the aggregate limited to the stored rows that can
/// publish them.
pub(super) fn storage_table_expression_for(
    qualified_table: &str,
    overlay: Option<&StorageGenerationOverlay>,
    selection: Selection<'_>,
) -> String {
    let Some(overlay) = overlay.filter(|overlay| !overlay.is_empty()) else {
        return qualified_table.to_owned();
    };
    let bin = "COLLATE Latin1_General_BIN2";
    // What leaves the table's own side: the names a staged row replaces, and
    // the dropped names. Both leave once per published name.
    let leaving = |name: &str| {
        let mut conditions = Vec::new();
        if let Some(staged) = &overlay.staged {
            conditions.push(format!(
                "NOT EXISTS (SELECT 1 FROM {} k WHERE k.FileName = {name})",
                staged.table
            ));
        }
        if !overlay.dropped.is_empty() {
            conditions.push(format!(
                "NOT EXISTS (SELECT 1 FROM ({}) d WHERE d.Name {bin} = {name} {bin})",
                dropped_names(&overlay.dropped)
            ));
        }
        if conditions.is_empty() {
            String::new()
        } else {
            format!("\n\x20 WHERE {}", conditions.join(" AND "))
        }
    };
    let own_side = if overlay.aliased {
        generation_table(qualified_table, overlay, &leaving("w.FileName"), selection)
    } else {
        format!(
            "(SELECT {} FROM {qualified_table} p{})",
            columns_of("p"),
            leaving("p.FileName")
        )
    };
    match &overlay.staged {
        None => format!("{own_side} AS storage"),
        Some(staged) => format!(
            "(SELECT {} FROM {own_side} x\n\x20 UNION ALL SELECT {} FROM {} s) AS storage",
            columns_of("x"),
            columns_of("s"),
            staged.table
        ),
    }
}

/// The rows of the newest generation that carries each name, as a parenthesised
/// table expression; `leaving` is the `WHERE` that takes names out after the
/// aggregate.
fn generation_table(
    qualified_table: &str,
    overlay: &StorageGenerationOverlay,
    leaving: &str,
    selection: Selection<'_>,
) -> String {
    let narrowing = selection_condition("c", selection)
        .map(|condition| format!("\n\x20                         WHERE {condition}"))
        .unwrap_or_default();
    let infix = quote(DYNAMIC_UPDATE_INFIX);
    let strip = DYNAMIC_UPDATE_INFIX.len() + GENERATION_LEN;
    let after_infix = DYNAMIC_UPDATE_INFIX.len();
    // The rank is compared as text: its width is that of the longest rank.
    let width = overlay.history.len().to_string().len();
    let zeros = "0".repeat(width);
    let ranks = generation_ranks(&overlay.history);
    let bin = "COLLATE Latin1_General_BIN2";
    format!(
        "(SELECT w.FileName, t.PartNo, t.DataSize, t.BinaryData, t.Attributes, t.Creation, t.Modified\n\
         \x20  FROM (SELECT MAX(y.Pub) AS FileName, SUBSTRING(MAX(y.Tag), {tag_start}, 4000) AS Src\n\
         \x20          FROM (SELECT x.Pub, RIGHT(N'{zeros}' + CONVERT(nvarchar({width}), x.Rk), {width}) + x.SrcName AS Tag\n\
         \x20                  FROM (SELECT c.FileName AS SrcName,\n\
         \x20                               CASE WHEN a.Pos = 0 THEN c.FileName ELSE STUFF(c.FileName, a.Pos, {strip}, N'') END AS Pub,\n\
         \x20                               CASE WHEN a.Pos = 0 THEN 0 ELSE g.Rk END AS Rk\n\
         \x20                          FROM {qualified_table} c\n\
         \x20                         CROSS APPLY (SELECT CHARINDEX({infix}, c.FileName {bin}) AS Pos) a\n\
         \x20                          LEFT JOIN ({ranks}) g\n\
         \x20                            ON a.Pos > 0 AND g.Gen {bin} = SUBSTRING(c.FileName, a.Pos + {after_infix}, {GENERATION_LEN}) {bin}{narrowing}) x\n\
         \x20                 WHERE x.Rk IS NOT NULL) y\n\
         \x20         GROUP BY y.Pub {bin}) w\n\
         \x20  JOIN {qualified_table} t ON t.FileName = w.Src{leaving})",
        tag_start = width + 1,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const G1: &str = "06cb0442-0c47-4fad-986a-f08f28287c1b";
    const G2: &str = "17894f1a-0404-4132-9792-15816a396671";
    const G3: &str = "4b094372-d4b4-4387-8acf-03f18697d7a4";

    fn alias(base: &str, generation: &str, suffix: &str) -> String {
        format!("{base}{DYNAMIC_UPDATE_INFIX}{generation}{suffix}")
    }

    #[test]
    fn reads_the_history_a_marker_records() {
        assert_eq!(
            dynamic_generation_history("{1,1,06cb0442-0c47-4fad-986a-f08f28287c1b}".as_bytes()),
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
        assert_eq!(
            dynamic_generation_history("{1,1,not-a-uuid}".as_bytes()),
            None
        );
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
    fn a_name_only_an_older_generation_carries_is_published_from_it() {
        // G3 changes `b`, G2 changed `a` and `b`; the newest carrier of each name wins.
        let history = vec![G1.to_owned(), G2.to_owned(), G3.to_owned()];
        let names = [
            "a".to_owned(),
            "b".to_owned(),
            alias("a", G2, ""),
            alias("b", G2, ""),
            alias("b", G3, ""),
            alias("c", G1, ".0"),
        ];
        let overlay = storage_generation_overlay(&history, names.iter().map(String::as_str));
        assert_eq!(overlay.published_name(&alias("a", G2, "")), Some("a"));
        assert_eq!(overlay.published_name(&alias("b", G3, "")), Some("b"));
        assert_eq!(overlay.published_name(&alias("b", G2, "")), None);
        assert_eq!(overlay.published_name(&alias("c", G1, ".0")), Some("c.0"));
        assert!(overlay.hides("a") && overlay.hides("b") && overlay.hides("c.0"));
    }

    #[test]
    fn a_generation_the_history_does_not_list_is_left_out() {
        let history = vec![G1.to_owned()];
        let names = [
            "a".to_owned(),
            alias("a", G2, ""),
            alias("z", G2, ""),
            // Too short to hold a generation after the infix.
            format!("q{DYNAMIC_UPDATE_INFIX}short"),
            // Not on a character boundary: must not panic.
            format!("я{DYNAMIC_UPDATE_INFIX}яяяяяяяяяяяяяяяяяяяяяяяяяяя"),
        ];
        let overlay = storage_generation_overlay(&history, names.iter().map(String::as_str));
        assert!(overlay.is_empty(), "no listed generation carries a row");
        assert!(!overlay.hides("a"));
    }

    #[test]
    fn only_the_first_infix_of_a_name_counts() {
        let history = vec![G1.to_owned(), G2.to_owned()];
        // G2 follows the second infix only: the SQL side reads the first one, so does this.
        let name = format!(
            "x{DYNAMIC_UPDATE_INFIX}not-a-generation-of-the-history-1234{DYNAMIC_UPDATE_INFIX}{G2}"
        );
        let overlay = storage_generation_overlay(&history, [name.as_str()]);
        assert!(overlay.is_empty());
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

    fn overlay_of(history: &[&str], names: &[String]) -> StorageGenerationOverlay {
        let history = history
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        storage_generation_overlay(&history, names.iter().map(String::as_str))
    }

    #[test]
    fn an_active_generation_reads_through_a_derived_table() {
        let overlay = overlay_of(&[G1], &["versions".into(), alias("versions", G1, "")]);
        let expression = storage_table_expression("[db].dbo.[Config]", Some(&overlay));

        assert!(expression.starts_with("(SELECT w.FileName, t.PartNo, t.DataSize, t.BinaryData"));
        assert!(expression.ends_with(") AS storage"));
        assert!(expression.contains("FROM [db].dbo.[Config] c"));
        assert!(expression.contains("JOIN [db].dbo.[Config] t ON t.FileName = w.Src"));
        assert!(expression.contains(&format!("VALUES (N'{G1}', 1)")));
        assert!(
            expression
                .contains("CHARINDEX(N'_dynupdate_', c.FileName COLLATE Latin1_General_BIN2)")
        );
        // 11 characters of infix and 36 of generation are cut out of the name.
        assert!(expression.contains("STUFF(c.FileName, a.Pos, 47, N'')"));
        // One generation: a one-digit rank.
        assert!(expression.contains("RIGHT(N'0' + CONVERT(nvarchar(1), x.Rk), 1)"));
        assert!(expression.contains("SUBSTRING(MAX(y.Tag), 2, 4000)"));
    }

    #[test]
    fn the_text_does_not_grow_with_the_number_of_aliases() {
        let few = overlay_of(
            &[G1],
            &[
                "a".to_owned(),
                "b".to_owned(),
                alias("a", G1, ""),
                alias("b", G1, ""),
            ],
        );
        let names = (0..120_000)
            .flat_map(|n| {
                [
                    format!("{n:08x}-0000-0000-0000-000000000000"),
                    alias(&format!("{n:08x}-0000-0000-0000-000000000000"), G1, ".0"),
                ]
            })
            .collect::<Vec<_>>();
        let many = overlay_of(&[G1], &names);
        assert_eq!(many.renames().len(), 120_000);
        let small = storage_table_expression("[db].dbo.[Config]", Some(&few));
        let big = storage_table_expression("[db].dbo.[Config]", Some(&many));
        assert_eq!(small, big, "the expression depends on the history only");
        assert!(big.len() < 2_000, "{} characters", big.len());
    }

    #[test]
    fn the_history_is_listed_once_per_generation_in_chunks_of_a_thousand() {
        let generations = (0..2_500)
            .map(|n| format!("{n:08x}-0000-4000-8000-000000000000"))
            .collect::<Vec<_>>();
        // A generation listed twice keeps its first rank.
        let mut history = generations.clone();
        history.push(generations[3].clone());
        let names = vec!["x".to_owned(), alias("x", &generations[0], "")];
        let overlay = storage_generation_overlay(&history, names.iter().map(String::as_str));
        let expression = storage_table_expression("[db].dbo.[Config]", Some(&overlay));
        assert_eq!(
            expression.matches("(VALUES ").count(),
            3,
            "2500 rows in lists of 1000"
        );
        assert_eq!(expression.matches("UNION ALL").count(), 2);
        assert_eq!(
            expression.matches("-0000-4000-8000-000000000000'").count(),
            2_500,
            "the repeated generation is listed once"
        );
        assert!(expression.contains(&format!("(N'{}', 4)", generations[3])));
        // 2 501 entries: a four-digit rank.
        assert!(expression.contains("RIGHT(N'0000' + CONVERT(nvarchar(4), x.Rk), 4)"));
    }

    #[test]
    fn a_dropped_name_is_published_from_no_row() {
        let names = [
            "kept".to_owned(),
            alias("kept", G2, ""),
            "gone".to_owned(),
            alias("gone", G1, ""),
            alias("gone", G2, ".0"),
            "gone-plain-only".to_owned(),
            "untouched".to_owned(),
        ];
        let overlay = overlay_of(&[G1, G2], &names).dropping(BTreeSet::from([
            "gone".to_owned(),
            "gone.0".to_owned(),
            "gone-plain-only".to_owned(),
        ]));

        assert_eq!(overlay.published_name(&alias("kept", G2, "")), Some("kept"));
        assert_eq!(overlay.published_name(&alias("gone", G1, "")), None);
        assert_eq!(overlay.published_name(&alias("gone", G2, ".0")), None);
        assert!(overlay.hides("gone") && overlay.hides("gone-plain-only"));
        assert!(
            overlay.hides("kept"),
            "an alias still replaces its plain row"
        );
        let published = overlay
            .published_names(names.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        assert_eq!(published, BTreeSet::from(["kept", "untouched"]));
        assert_eq!(overlay.stored_name("kept"), alias("kept", G2, ""));
        assert_eq!(overlay.stored_name("untouched"), "untouched");
        assert!(!overlay.is_empty());
    }

    #[test]
    fn an_overlay_that_only_drops_is_not_empty() {
        let overlay =
            StorageGenerationOverlay::default().dropping(BTreeSet::from(["x".to_owned()]));
        assert!(!overlay.is_empty());
        assert!(StorageGenerationOverlay::default().is_empty());
    }

    #[test]
    fn only_the_dropped_names_make_the_text_longer() {
        let overlay = overlay_of(&[G1], &["a".into(), alias("a", G1, "")]);
        let plain = storage_table_expression("[db].dbo.[Config]", Some(&overlay));
        assert!(!plain.contains("NOT EXISTS"), "no name is dropped");

        let one = storage_table_expression(
            "[db].dbo.[Config]",
            Some(
                &overlay
                    .clone()
                    .dropping(BTreeSet::from(["it's".to_owned()])),
            ),
        );
        assert!(one.contains(
            "WHERE NOT EXISTS (SELECT 1 FROM (SELECT v.Name FROM (VALUES (N'it''s')) v(Name)) d"
        ));
        assert!(one.contains(
            "d.Name COLLATE Latin1_General_BIN2 = w.FileName COLLATE Latin1_General_BIN2)"
        ));
        assert!(one.ends_with(") AS storage"));

        let many = (0..2_500)
            .map(|n| format!("{n:08x}-0000-4000-8000-000000000000.0"))
            .collect::<BTreeSet<_>>();
        let text = storage_table_expression("[db].dbo.[Config]", Some(&overlay.dropping(many)));
        assert_eq!(
            text.matches("SELECT v.Name FROM (VALUES ").count(),
            3,
            "2500 names in lists of 1000"
        );
        assert_eq!(text.matches("UNION ALL").count(), 2);
    }

    #[test]
    fn an_overlay_is_the_view_of_one_table_of_one_database() {
        let overlay = Arc::new(overlay_of(&[G1], &["a".into(), alias("a", G1, "")]));
        let _scope = StorageViewScope::begin("keyed_a");
        install_storage_generation_overlay("keyed_a", "Config", overlay.clone());
        assert_eq!(
            storage_generation_overlay_for("keyed_a", "Config"),
            Some(overlay)
        );
        assert!(storage_generation_overlay_for("keyed_b", "Config").is_none());
        assert!(storage_generation_overlay_for("keyed_a", "ConfigSave").is_none());
    }

    #[test]
    fn a_scope_takes_away_what_it_installed_and_gives_back_what_it_suspended() {
        let outer_overlay = Arc::new(overlay_of(&[G1], &["a".into(), alias("a", G1, "")]));
        let inner_overlay = Arc::new(overlay_of(&[G2], &["b".into(), alias("b", G2, "")]));
        let outer = StorageViewScope::begin("nested");
        install_storage_generation_overlay("nested", "Config", outer_overlay.clone());
        {
            let _inner = StorageViewScope::begin("nested");
            assert!(
                storage_generation_overlay_for("nested", "Config").is_none(),
                "the inner scope reads the rows as they are stored"
            );
            install_storage_generation_overlay("nested", "Config", inner_overlay.clone());
            assert_eq!(
                storage_generation_overlay_for("nested", "Config"),
                Some(inner_overlay)
            );
        }
        assert_eq!(
            storage_generation_overlay_for("nested", "Config"),
            Some(outer_overlay),
            "the suspended view is back"
        );
        drop(outer);
        assert!(
            storage_generation_overlay_for("nested", "Config").is_none(),
            "nothing outlives the outermost scope"
        );
    }

    #[test]
    fn a_scope_leaves_the_views_of_other_databases_alone() {
        let overlay = Arc::new(overlay_of(&[G1], &["a".into(), alias("a", G1, "")]));
        let _other = StorageViewScope::begin("untouched");
        install_storage_generation_overlay("untouched", "Config", overlay.clone());
        drop(StorageViewScope::begin("another"));
        assert_eq!(
            storage_generation_overlay_for("untouched", "Config"),
            Some(overlay)
        );
    }

    fn selecting(names: &[&str], owners: bool) -> String {
        let overlay = overlay_of(&[G1], &["a".into(), alias("a", G1, "")]);
        let names = names
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>();
        let selection = if owners {
            Selection::Owners(&names)
        } else {
            Selection::Names(&names)
        };
        storage_table_expression_for("[db].dbo.[Config]", Some(&overlay), selection)
    }

    #[test]
    fn a_selection_narrows_the_scan_under_the_aggregate_to_the_rows_that_can_publish_it() {
        let text = selecting(
            &["a627e390-8fad-4a95-afe6-674f54813188.0", "versions"],
            false,
        );
        // The stored names: the published ones, and every alias of their stems.
        assert!(
            text.contains(
                "WHERE c.FileName IN (N'a627e390-8fad-4a95-afe6-674f54813188.0', N'versions') OR c.FileName LIKE N'a627e390-8fad-4a95-afe6-674f54813188\\_dynupdate\\_%' ESCAPE N'\\' OR c.FileName LIKE N'versions\\_dynupdate\\_%' ESCAPE N'\\'"
            ),
            "{text}"
        );
        // Under the scan (the innermost select), not after the aggregate.
        let narrowing = text.find("WHERE c.FileName IN").unwrap();
        assert!(text.find("FROM [db].dbo.[Config] c").unwrap() < narrowing);
        assert!(narrowing < text.find("GROUP BY").unwrap());
        assert!(text.ends_with(") AS storage"));
    }

    #[test]
    fn an_owner_selection_takes_the_owner_row_its_numbered_rows_and_its_aliases() {
        let text = selecting(&["ab132638-5188-470d-9432-de85f2b2c7d8"], true);
        assert!(
            text.contains(
                "WHERE c.FileName = N'ab132638-5188-470d-9432-de85f2b2c7d8' OR c.FileName LIKE N'ab132638-5188-470d-9432-de85f2b2c7d8.%' ESCAPE N'\\' OR c.FileName LIKE N'ab132638-5188-470d-9432-de85f2b2c7d8\\_dynupdate\\_%' ESCAPE N'\\'"
            ),
            "{text}"
        );
    }

    #[test]
    fn no_selection_a_long_one_and_a_table_without_aliases_read_as_before() {
        let plain = storage_table_expression(
            "[db].dbo.[Config]",
            Some(&overlay_of(&[G1], &["a".into(), alias("a", G1, "")])),
        );
        assert_eq!(
            selecting(&[], false),
            plain,
            "an empty selection is no selection"
        );
        let many = (0..=SELECTION_MAX)
            .map(|n| format!("{n:08x}-0000-0000-0000-000000000000"))
            .collect::<Vec<_>>();
        let many = many.iter().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(
            selecting(&many, false),
            plain,
            "a long list is read the way an unbounded read is"
        );
        // Without an alias the expression has no aggregate to narrow.
        let names = BTreeSet::from(["x".to_owned()]);
        let no_alias =
            StorageGenerationOverlay::default().dropping(BTreeSet::from(["gone".to_owned()]));
        assert_eq!(
            storage_table_expression_for(
                "[db].dbo.[Config]",
                Some(&no_alias),
                Selection::Names(&names)
            ),
            storage_table_expression("[db].dbo.[Config]", Some(&no_alias))
        );
    }

    #[test]
    fn a_wildcard_in_a_name_is_escaped_in_the_prefix() {
        let text = selecting(&["50%_off[1]"], false);
        assert!(
            text.contains("LIKE N'50\\%\\_off\\[1]\\_dynupdate\\_%' ESCAPE N'\\'"),
            "{text}"
        );
    }

    const SAVED: &str = "[db].dbo.[ConfigSave]";

    #[test]
    fn staged_rows_replace_the_rows_of_their_names() {
        let names = [
            "a".to_owned(),
            alias("a", G1, ""),
            "b".to_owned(),
            alias("b", G1, ".0"),
            "c".to_owned(),
        ];
        let staged = BTreeSet::from(["a".to_owned(), "b.0".to_owned(), "new".to_owned()]);
        let overlay = overlay_of(&[G1], &names).staging(SAVED.to_owned(), staged.clone());

        assert_eq!(overlay.published_name(&alias("a", G1, "")), None);
        assert_eq!(overlay.published_name(&alias("b", G1, ".0")), None);
        assert!(overlay.hides("a") && overlay.hides("b.0"));
        let published = overlay
            .published_names(names.iter().map(String::as_str))
            .collect::<BTreeSet<_>>();
        assert_eq!(published, BTreeSet::from(["b", "c"]));
        assert_eq!(overlay.staged_names(), Some(&staged));
        assert!(!overlay.is_empty());
    }

    #[test]
    fn the_staged_rows_are_appended_to_the_generations() {
        let overlay = overlay_of(&[G1], &["a".into(), alias("a", G1, "")])
            .staging(SAVED.to_owned(), BTreeSet::from(["x".to_owned()]));
        let text = storage_table_expression("[db].dbo.[Config]", Some(&overlay));

        assert!(text.starts_with(
            "(SELECT x.FileName, x.PartNo, x.DataSize, x.BinaryData, x.Attributes, x.Creation, x.Modified FROM (SELECT w.FileName"
        ));
        assert!(text.contains(
            "NOT EXISTS (SELECT 1 FROM [db].dbo.[ConfigSave] k WHERE k.FileName = w.FileName)"
        ));
        assert!(text.ends_with(
            "UNION ALL SELECT s.FileName, s.PartNo, s.DataSize, s.BinaryData, s.Attributes, s.Creation, s.Modified FROM [db].dbo.[ConfigSave] s) AS storage"
        ));
    }

    #[test]
    fn staged_rows_without_an_alias_read_the_table_itself() {
        let overlay = StorageGenerationOverlay::default()
            .staging(SAVED.to_owned(), BTreeSet::from(["x".to_owned()]))
            .dropping(BTreeSet::from(["gone".to_owned()]));
        let text = storage_table_expression("[db].dbo.[Config]", Some(&overlay));

        assert!(text.starts_with("(SELECT x.FileName, x.PartNo"));
        assert!(text.contains("FROM [db].dbo.[Config] p"));
        assert!(text.contains("k.FileName = p.FileName"));
        assert!(text.contains(
            "d.Name COLLATE Latin1_General_BIN2 = p.FileName COLLATE Latin1_General_BIN2"
        ));
        assert!(!text.contains("MAX(y.Pub)"), "no generation is read");
    }

    #[test]
    fn dropped_names_without_an_alias_read_the_table_itself() {
        let overlay =
            StorageGenerationOverlay::default().dropping(BTreeSet::from(["gone".to_owned()]));
        let text = storage_table_expression("[db].dbo.[Config]", Some(&overlay));

        assert!(text.starts_with("(SELECT p.FileName, p.PartNo"));
        assert!(text.ends_with(") AS storage"));
        assert!(!text.contains("UNION ALL"));
    }

    #[test]
    fn the_staged_names_do_not_lengthen_the_text() {
        let base = overlay_of(&[G1], &["a".into(), alias("a", G1, "")]);
        let few = base
            .clone()
            .staging(SAVED.to_owned(), BTreeSet::from(["x".to_owned()]));
        let many = base.staging(
            SAVED.to_owned(),
            (0..50_000)
                .map(|n| format!("{n:08x}-0000-0000-0000-000000000000"))
                .collect(),
        );
        assert_eq!(
            storage_table_expression("[db].dbo.[Config]", Some(&few)),
            storage_table_expression("[db].dbo.[Config]", Some(&many)),
            "the staged table is read on the server"
        );
    }

    /// The selection the SQL expression makes, written the plain way, against the
    /// overlay the Rust side computes: the same rows must survive under the same names.
    fn selection_of_the_sql(history: &[String], stored: &[String]) -> BTreeMap<String, String> {
        let rank_of = |generation: &str| {
            history
                .iter()
                .position(|value| value == generation)
                .map(|rank| rank + 1)
        };
        // published name -> (rank, stored name); plain rows have rank 0
        let mut best = BTreeMap::<String, (usize, String)>::new();
        for name in stored {
            let (published, rank) = match name.find(DYNAMIC_UPDATE_INFIX) {
                None => (name.clone(), 0),
                Some(position) => {
                    let start = position + DYNAMIC_UPDATE_INFIX.len();
                    let Some(generation) = name.get(start..start + GENERATION_LEN) else {
                        continue;
                    };
                    let Some(rank) = rank_of(generation) else {
                        continue;
                    };
                    (
                        format!("{}{}", &name[..position], &name[start + GENERATION_LEN..]),
                        rank,
                    )
                }
            };
            if best
                .get(&published)
                .is_none_or(|(current, _)| *current < rank)
            {
                best.insert(published, (rank, name.clone()));
            }
        }
        best.into_iter()
            .map(|(published, (_, name))| (published, name))
            .collect()
    }

    #[test]
    fn the_rust_overlay_and_the_sql_selection_agree() {
        let history = vec![G1.to_owned(), G2.to_owned(), G3.to_owned()];
        let mut stored = Vec::new();
        for n in 0..40 {
            let base = format!("{n:08x}-1111-2222-3333-444444444444");
            stored.push(base.clone());
            stored.push(format!("{base}.0"));
            // Some names are carried by one generation, some by several, some by an unlisted one.
            if n % 2 == 0 {
                stored.push(alias(&base, G1, ""));
                stored.push(alias(&base, G1, ".0"));
            }
            if n % 3 == 0 {
                stored.push(alias(&base, G3, ".0"));
            }
            if n % 5 == 0 {
                stored.push(alias(&base, "99999999-9999-4999-8999-999999999999", ""));
            }
            if n % 7 == 0 {
                // An object an update added: no plain row.
                stored.push(alias(&format!("added-{n}"), G2, ""));
            }
        }
        stored.push("versions".to_owned());
        stored.push(alias("versions", G2, ""));
        let overlay = storage_generation_overlay(&history, stored.iter().map(String::as_str));

        let expected = selection_of_the_sql(&history, &stored);
        let mut from_overlay = BTreeMap::new();
        for name in &stored {
            if let Some(published) = overlay.published_name(name) {
                from_overlay.insert(published.to_owned(), name.clone());
            } else if !is_dynamic_generation_alias(name) && !overlay.hides(name) {
                from_overlay.insert(name.clone(), name.clone());
            }
        }
        assert_eq!(from_overlay, expected);
    }
}

/// The overlay the export installs, run against a real server: the rows the
/// derived table publishes and the headers the export lists must be what the
/// overlay describes, row by row (SHA-256 of the stored bytes).
///
/// Set `IBCMD_RS_DYNGEN_DB` to a lab database (it is only read). With
/// `IBCMD_RS_DYNGEN_MAIN=1` the run publishes the main configuration, as the
/// platform's export does, staged `ConfigSave` rows included:
///
/// ```text
/// IBCMD_RS_DYNGEN_DB=ibcmd_rs_04_rcheck_bsp_a IBCMD_RS_DYNGEN_MAIN=1 cargo test --locked \
///     -p ibcmd-rs --lib --no-default-features --features mssql-live-tests \
///     dynamic_generation::live -- --ignored --nocapture
/// ```
#[cfg(all(test, feature = "mssql-live-tests"))]
mod live {
    use super::*;
    use crate::mssql_dump::fetch::fetch_row_headers;
    use crate::sql::{SqlBackend, SqlClient, SqlExec, SqlOptions};

    type Rows = BTreeMap<(String, i64), (i64, Vec<u8>)>;

    fn stored_rows(client: &dyn SqlClient, table: &str) -> anyhow::Result<Rows> {
        let mut rows = Rows::new();
        for row in client.query_rows(
            &format!(
                "SELECT FileName, PartNo, DataSize, HASHBYTES('SHA2_256', BinaryData) FROM {table}"
            ),
            &[],
        )? {
            let key = (row.text(0)?.to_owned(), row.i64(1)?);
            assert!(
                rows.insert(key.clone(), (row.i64(2)?, row.binary(3)?.to_vec()))
                    .is_none(),
                "{key:?} is stored twice"
            );
        }
        Ok(rows)
    }

    #[test]
    #[ignore = "reads a lab database: set IBCMD_RS_DYNGEN_DB"]
    fn the_expression_publishes_the_rows_the_overlay_describes() -> anyhow::Result<()> {
        let Some(database) = std::env::var_os("IBCMD_RS_DYNGEN_DB") else {
            return Ok(());
        };
        let database = database.to_string_lossy().into_owned();
        let main = std::env::var_os("IBCMD_RS_DYNGEN_MAIN").is_some();
        let sql = SqlExec::from_options(SqlOptions::integrated("localhost", None))?;
        let SqlBackend::Client(client) = sql.backend() else {
            anyhow::bail!("the built-in client is expected");
        };
        let config = format!("[{}].dbo.[Config]", database.replace(']', "]]"));
        let saved = format!("[{}].dbo.[ConfigSave]", database.replace(']', "]]"));

        let stored = stored_rows(client, &config)?;
        let staged = stored_rows(client, &saved)?;

        // What the export installs, exactly as `dump_table_rows_streamed` asks for it.
        let _view = StorageViewScope::begin(&database);
        let headers = fetch_row_headers(&sql, &database, "Config", &BTreeSet::new())?;
        let listed = crate::mssql_dump::install_storage_overlay(
            &sql,
            &database,
            "Config",
            &BTreeSet::new(),
            headers,
            main,
        )?;
        let overlay =
            storage_generation_overlay_for(&database, "Config").expect("an overlay is installed");

        // What the overlay says is published.
        let mut expected = Rows::new();
        for ((name, part), value) in &stored {
            if let Some(published) = overlay.published_name(name) {
                expected.insert((published.to_owned(), *part), value.clone());
            } else if !is_dynamic_generation_alias(name) && !overlay.hides(name) {
                expected.insert((name.clone(), *part), value.clone());
            }
        }
        if let Some(names) = overlay.staged_names() {
            for ((name, part), value) in &staged {
                assert!(names.contains(name), "{name} is staged but not listed");
                expected.insert((name.clone(), *part), value.clone());
            }
        }

        // What the server publishes.
        let expression = crate::mssql_dump::qualified_storage_table(&database, "Config");
        let published = stored_rows(client, &expression)?;
        eprintln!(
            "{database}: {} stored rows, {} staged rows, {} aliases published, {} published rows, {} listed, expression of {} characters",
            stored.len(),
            staged.len(),
            overlay.renames().len(),
            published.len(),
            listed.len(),
            expression.len()
        );
        assert_eq!(published.len(), expected.len());
        assert!(published == expected, "the server and the overlay disagree");

        // The same rows through a selection (#409 F-15): a few aliased names, plain
        // names, and a name that does not exist.
        if !main {
            let mut picked = BTreeSet::new();
            for published in overlay.renames().values().take(6) {
                picked.insert(published.clone());
            }
            for (name, _) in stored
                .keys()
                .filter(|(name, _)| !is_dynamic_generation_alias(name))
                .take(6)
            {
                picked.insert(name.clone());
            }
            picked.insert("00000000-0000-0000-0000-000000000000.0".to_owned());
            let read = |selection: Selection<'_>, filter: &str| -> anyhow::Result<Rows> {
                let expression =
                    crate::mssql_dump::qualified_storage_table_for(&database, "Config", selection);
                stored_rows(
                    client,
                    &format!("(SELECT * FROM {expression} WHERE {filter}) sel"),
                )
            };
            let values = picked
                .iter()
                .map(|name| quote(name))
                .collect::<Vec<_>>()
                .join(", ");
            let filter = format!("FileName IN ({values})");
            let by_names = read(Selection::Names(&picked), &filter)?;
            let unbounded = read(Selection::All, &filter)?;
            assert!(
                by_names == unbounded,
                "a selection changed the rows it keeps"
            );
            assert!(!by_names.is_empty());
            let owners = picked
                .iter()
                .filter(|name| !name.contains('.'))
                .cloned()
                .collect::<BTreeSet<_>>();
            if !owners.is_empty() {
                let filter = owners
                    .iter()
                    .map(|owner| {
                        format!(
                            "FileName = {} OR FileName LIKE N'{}.%'",
                            quote(owner),
                            owner.replace('\'', "''")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let by_owners = read(Selection::Owners(&owners), &filter)?;
                let all = read(Selection::All, &filter)?;
                assert!(
                    by_owners == all,
                    "an owner selection changed the rows it keeps"
                );
            }
            eprintln!(
                "{database}: {} rows through a selection of {} names and {} owners",
                by_names.len(),
                picked.len(),
                owners.len()
            );
        }

        // What the export lists: the same names, parts and sizes.
        let listed = listed
            .iter()
            .map(|header| {
                (
                    (header.file_name.clone(), i64::from(header.part_no)),
                    header.data_size,
                )
            })
            .collect::<BTreeMap<_, _>>();
        let sizes = expected
            .iter()
            .map(|(key, (size, _))| (key.clone(), *size))
            .collect::<BTreeMap<_, _>>();
        assert!(
            listed == sizes,
            "the headers and the published rows disagree"
        );
        Ok(())
    }
}
